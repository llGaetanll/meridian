//! The store, against a real Postgres.
//!
//! Ignored by default, because `cargo test` has to pass on a clean checkout
//! with no Docker running — the same reason the server treats `DATABASE_URL`
//! as optional. Run them with a database up:
//!
//! ```sh
//! ./scripts/db-init.sh
//! cargo test -p src-postgres -- --ignored
//! ```
//!
//! These are not tests of SQL we wrote. They are tests that the *mechanism*
//! works: that the schema applies, that applying it twice is safe, and that a
//! pool which exists is a pool that answers. Those three are what "one command
//! and it comes up" actually rests on.

const DEFAULT_URL: &str = "postgres://meridian:meridian@localhost:5432/meridian";

fn url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn the_schema_applies_and_the_database_answers() {
    let db = src_postgres::connect(&url())
        .await
        .expect("postgres should be up — ./scripts/db-init.sh");

    src_postgres::migrate(&db).await.expect("migrations run");

    src_postgres::ping(&db).await.expect("and it answers");

    // Every migration in the directory left a row behind. This is what
    // separates "the connection worked" from "the schema is actually there".
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&db)
        .await
        .expect("the migrations table exists once anything has run");

    assert_eq!(
        applied as usize,
        src_postgres::MIGRATOR.iter().count(),
        "every migration in the directory should have been applied",
    );

    // And the one migration there is did what it says.
    let items: i64 = sqlx::query_scalar("SELECT count(*) FROM items")
        .fetch_one(&db)
        .await
        .expect("0001 creates the items table");

    assert_eq!(items, 0, "a fresh database holds no connections");
}

/// The server runs this on every boot, so it has to be a no-op against a
/// database that is already current rather than an error.
#[tokio::test]
#[ignore = "needs a database: ./scripts/db-init.sh"]
async fn migrating_twice_changes_nothing() {
    let db = src_postgres::connect(&url())
        .await
        .expect("postgres should be up — ./scripts/db-init.sh");

    src_postgres::migrate(&db).await.expect("first run");
    src_postgres::migrate(&db)
        .await
        .expect("second run is a no-op");

    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&db)
        .await
        .expect("count");

    assert_eq!(
        applied as usize,
        src_postgres::MIGRATOR.iter().count(),
        "a second run should not apply anything again",
    );
}
