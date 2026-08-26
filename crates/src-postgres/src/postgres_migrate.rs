use sqlx::migrate::MigrateError;
use sqlx::migrate::Migrator;

use crate::Db;
use crate::PostgresError;

/// Every migration, compiled into the binary.
///
/// Embedded rather than read off disk at run time: the binary is going into a
/// container, and a container that also needs a directory copied next to it is
/// a container with a second thing to get wrong. It is also what lets the
/// server apply pending migrations as it boots, which is the difference
/// between `docker compose up` and a sequence of commands to remember.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Bring the database up to the schema this binary expects.
///
/// Safe to run on every boot, and meant to be. sqlx records what it has
/// applied in a `_sqlx_migrations` table, one row per file with a checksum, so
/// this is a no-op against a database that is already current.
///
/// Against one where an already-applied file has been *edited*, it fails
/// loudly — and that is the feature, not an inconvenience. **A migration that
/// has run anywhere is immutable.** There is no way to know which databases
/// already ran the old version of a file, so a mistake is corrected by the
/// next migration and never by changing the last one. The checksum is what
/// makes that rule impossible to break by accident.
pub async fn migrate(db: &Db) -> Result<(), PostgresError> {
    let pending = MIGRATOR.iter().count();

    MIGRATOR.run(db).await?;

    tracing::info!(migrations = pending, "schema is current");

    Ok(())
}

impl From<MigrateError> for PostgresError {
    fn from(err: MigrateError) -> Self {
        PostgresError::Migrate(err)
    }
}
