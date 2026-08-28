//! The mechanism: that the schema applies and that the database answers.
//!
//! `#[sqlx::test]` has already run the migrator into a fresh database by the
//! time these start, so what they check is that it produced what it claims to
//! and that running it again is safe — which the server does on every boot.
//!
//! They need a server to create those databases on:
//!
//! ```sh
//! ./scripts/db-init.sh
//! cargo test
//! ```

use sqlx::PgPool;

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn the_schema_applies_and_the_database_answers(db: PgPool) {
    src_postgres::ping(&db).await.expect("it answers");

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

    // And the tables they describe are there to be queried. Written out rather
    // than looped: sqlx takes a `&'static str`, so a statement cannot be built
    // from anything at run time without saying so explicitly. Four literals is
    // a small price for that guard being on everywhere else.
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
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn migrating_twice_changes_nothing(db: PgPool) {
    src_postgres::migrate(&db).await.expect("a second run");

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
