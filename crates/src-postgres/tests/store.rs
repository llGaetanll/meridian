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

    // And the tables the migrations describe are there to be queried. Note
    // what this does *not* assert: how many rows are in them. Cargo runs each
    // test binary as its own process, in parallel, against this one database —
    // so `tests/ledger.rs` may be writing while this reads, and a count here
    // would fail on scheduling rather than on anything being wrong.
    // Written out rather than looped: sqlx refuses a query built with
    // `format!` — `SqlSafeStr` takes a `&'static str`, so a statement cannot
    // be assembled from anything at run time without saying so explicitly.
    // Four literals is a small price for that guard being on everywhere else.
    sqlx::query("SELECT * FROM items LIMIT 0")
        .fetch_all(&db)
        .await
        .expect("0001 creates items");

    for query in [
        sqlx::query("SELECT * FROM accounts LIMIT 0"),
        sqlx::query("SELECT * FROM entries LIMIT 0"),
        sqlx::query("SELECT * FROM streams LIMIT 0"),
    ] {
        query.fetch_all(&db).await.expect("0002 creates the ledger");
    }
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
