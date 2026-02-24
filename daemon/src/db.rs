use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub async fn open(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA journal_mode=WAL;")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA synchronous=NORMAL;")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA foreign_keys=ON;")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA busy_timeout=5000;")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA temp_store=MEMORY;")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await?;
    run_migrations(&pool).await?;
    // NOTE(victor): We intentionally keep DB open limited to pragmas + migrations.
    // Opencode's DB init path stays limited to pragmas + migrations, and we mirror that
    // startup pattern to keep initialization fast and side-effect free.
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!().run(pool).await?;
    Ok(())
}

pub async fn reconcile_on_boot(pool: &SqlitePool) -> Result<()> {
    let now = now_ms();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE member_runs SET ended_at_ms = ?, end_reason = 'unknown' WHERE ended_at_ms IS NULL",
    )
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE tool_calls SET status = 'cancelled', ended_at_ms = ? WHERE status = 'started'",
    )
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE member_status_current SET alive = 0, state = 'offline'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_reconcile_open_rows_on_boot() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("teams.db");
        let pool = open(&db_path).await.unwrap();

        let now = now_ms();
        sqlx::query("INSERT INTO teams (id, name, description, created_at_ms, lead_agent_id) VALUES (?, ?, '', ?, ?)")
            .bind("t1")
            .bind("team-1")
            .bind(now)
            .bind("m1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO members (id, team_id, name, agent_type, model, backend_type, cwd, joined_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind("m1")
            .bind("t1")
            .bind("lead")
            .bind("team-lead")
            .bind("claude")
            .bind("claude")
            .bind("/")
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO member_runs (id, team_id, member_id, started_at_ms) VALUES (?, ?, ?, ?)",
        )
        .bind("r1")
        .bind("t1")
        .bind("m1")
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO tool_calls (id, team_id, member_id, backend_type, source, tool_name, input_summary, status, started_at_ms, ingested_at_ms) VALUES (?, ?, ?, 'claude', 'adapter_observer', 'read', '/tmp/a', 'started', ?, ?)")
            .bind("c1")
            .bind("t1")
            .bind("m1")
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO member_status_current (member_id, run_id, alive, state, last_heartbeat_ms) VALUES (?, ?, 1, 'thinking', ?)")
            .bind("m1")
            .bind("r1")
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();

        reconcile_on_boot(&pool).await.unwrap();

        let run_ended: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM member_runs WHERE ended_at_ms IS NOT NULL AND end_reason = 'unknown'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let cancelled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tool_calls WHERE status = 'cancelled' AND ended_at_ms IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
        let offline: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM member_status_current WHERE alive = 0 AND state = 'offline'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(run_ended, 1);
        assert_eq!(cancelled, 1);
        assert_eq!(offline, 1);
    }
}
