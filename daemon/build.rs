use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};
use std::str::FromStr;

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime for build script");
    runtime.block_on(async_main());
}

async fn async_main() {
    println!("cargo:rerun-if-changed=migrations/0001_teams_db.sql");
    println!("cargo:rerun-if-changed=migrations/0002_member_baseline_commit.sql");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    let db_path = std::path::PathBuf::from(out_dir).join("sqlx-compile.db");
    let _ = std::fs::remove_file(&db_path);

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))
        .expect("failed to parse compile-time sqlite db path")
        .create_if_missing(true);
    let mut conn = options
        .connect()
        .await
        .expect("failed to open compile-time sqlite db");
    let migrations = [
        include_str!("migrations/0001_teams_db.sql"),
        include_str!("migrations/0002_member_baseline_commit.sql"),
    ];
    for migration in migrations {
        sqlx::raw_sql(migration)
            .execute(&mut conn)
            .await
            .expect("failed to apply compile-time sqlite schema");
    }
    conn.close()
        .await
        .expect("failed to close compile-time sqlite db");

    println!(
        "cargo:rustc-env=DATABASE_URL=sqlite://{}",
        db_path.display()
    );
}
