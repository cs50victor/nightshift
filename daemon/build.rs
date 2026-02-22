fn main() {
    println!("cargo:rerun-if-changed=migrations/0001_teams_db.sql");
    println!("cargo:rerun-if-changed=migrations/0002_member_baseline_commit.sql");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    let db_path = std::path::PathBuf::from(out_dir).join("sqlx-compile.db");
    let _ = std::fs::remove_file(&db_path);

    let conn = rusqlite::Connection::open(&db_path).expect("failed to open compile-time sqlite db");
    let migrations = [
        include_str!("migrations/0001_teams_db.sql"),
        include_str!("migrations/0002_member_baseline_commit.sql"),
    ];
    for migration in migrations {
        conn.execute_batch(migration)
            .expect("failed to apply compile-time sqlite schema");
    }

    println!(
        "cargo:rustc-env=DATABASE_URL=sqlite://{}",
        db_path.display()
    );
}
