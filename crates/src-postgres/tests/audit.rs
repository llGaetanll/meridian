//! The audit log, against a real Postgres.
//!
//! Two properties have to be true of the table itself rather than of any
//! caller: a row cannot be changed or removed once written, and deleting an
//! account leaves its rows standing. Both are what make the log an account of
//! who read customer data rather than a table somebody can tidy.

use api_common::Timestamp;
use sqlx::PgPool;
use src_postgres::AuditScope;

const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_recorded_access_reads_back(db: PgPool) {
    let user_id = src_postgres::upsert_user(&db, "al@example.com")
        .await
        .expect("the user is written");

    src_postgres::record_access(&db, user_id, AuditScope::Snapshot, "203.0.113.7", NOW)
        .await
        .expect("the access is written");

    let accesses = src_postgres::load_accesses(&db, user_id)
        .await
        .expect("the read works");

    assert_eq!(accesses.len(), 1);
    assert_eq!(accesses[0].at, NOW);
    assert_eq!(accesses[0].user_id, Some(user_id));
    assert_eq!(accesses[0].scope, "snapshot");
    assert_eq!(accesses[0].client, "203.0.113.7");
}

/// The claim the word "append-only" makes. The grant keeps an ordinary role
/// out; the trigger is what refuses the table's owner and a superuser too,
/// which is what the connecting role is in the containers.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_row_cannot_be_changed_or_removed(db: PgPool) {
    let user_id = src_postgres::upsert_user(&db, "al@example.com")
        .await
        .expect("the user is written");

    src_postgres::record_access(&db, user_id, AuditScope::Billing, "203.0.113.7", NOW)
        .await
        .expect("the access is written");

    let update = sqlx::query("UPDATE audit_log SET scope = 'nothing'")
        .execute(&db)
        .await;
    let delete = sqlx::query("DELETE FROM audit_log").execute(&db).await;
    let truncate = sqlx::query("TRUNCATE audit_log").execute(&db).await;

    for refused in [update, delete, truncate] {
        let error = refused.expect_err("the table refuses");

        assert!(
            error.to_string().contains("append-only"),
            "refused for the right reason: {error}"
        );
    }

    let accesses = src_postgres::load_accesses(&db, user_id)
        .await
        .expect("the read works");

    assert_eq!(accesses.len(), 1, "the row stands");
}

/// Deleting an account takes the data and leaves the record of who read it.
/// No cascade reaches this table, and that is on purpose.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn deletion_leaves_the_audit_rows(db: PgPool) {
    let user_id = src_postgres::upsert_user(&db, "gone@example.com")
        .await
        .expect("the user is written");

    src_postgres::record_access(&db, user_id, AuditScope::Snapshot, "203.0.113.7", NOW)
        .await
        .expect("the access is written");

    src_postgres::delete_user(&db, user_id)
        .await
        .expect("the deletion works");

    let accesses = src_postgres::load_accesses(&db, user_id)
        .await
        .expect("the read works");

    assert_eq!(
        accesses.len(),
        1,
        "the access survives the account it was of"
    );
}
