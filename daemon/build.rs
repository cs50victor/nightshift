fn main() {
    println!("cargo:rerun-if-changed=migrations/0001_teams_db.sql");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    let db_path = std::path::PathBuf::from(out_dir).join("sqlx-compile.db");

    let conn = rusqlite::Connection::open(&db_path).expect("failed to open compile-time sqlite db");
    let migration = include_str!("migrations/0001_teams_db.sql");
    conn.execute_batch(migration)
        .expect("failed to apply compile-time sqlite schema");

    println!(
        "cargo:rustc-env=DATABASE_URL=sqlite://{}",
        db_path.display()
    );
}
