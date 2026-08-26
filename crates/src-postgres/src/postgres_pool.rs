use std::time::Duration;

use sqlx::Pool;
use sqlx::Postgres;
use sqlx::postgres::PgPoolOptions;

/// A handle to the database. Cheap to clone — it is a pool, not a connection.
pub type Db = Pool<Postgres>;

/// How long to wait for the database to accept us before giving up.
///
/// Generous, because the common case for a failure here is the container still
/// starting: `docker compose` brings both up at once and Postgres takes a
/// second or two to be ready even when everything is right. The compose file
/// has a healthcheck so this should not be load-bearing, but a server that
/// dies because it was three seconds early is a bad first impression.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Open the pool named by `DATABASE_URL`.
///
/// The URL is the whole configuration: host, credentials, database. That is
/// deliberate — it is one thing to set in an env file, one thing to override
/// in compose, and one thing for a script to check before it does something
/// destructive.
pub async fn connect(url: &str) -> Result<Db, PostgresError> {
    // The credentials are in the URL, so the URL never reaches a log line.
    // What is worth knowing is which host and which database, and those are
    // recoverable from the connection itself.
    tracing::info!("connecting to postgres");

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(CONNECT_TIMEOUT)
        .connect(url)
        .await?;

    Ok(pool)
}

/// Whether the database is actually answering.
///
/// `SELECT 1` rather than a pool metric: a pool can hold a connection that the
/// server on the other end has since closed, so the only honest answer comes
/// from asking it something.
pub async fn ping(db: &Db) -> Result<(), PostgresError> {
    sqlx::query("SELECT 1").execute(db).await?;

    Ok(())
}

/// What can go wrong down here.
///
/// One variant for now. It exists rather than leaking `sqlx::Error` because
/// the callers above are state machines and runtimes that should not have to
/// know which database this is.
#[derive(Debug)]
pub enum PostgresError {
    Sql(sqlx::Error),

    /// The schema could not be brought up to date. Most often an
    /// already-applied migration whose file has since been edited, which sqlx
    /// refuses on purpose — see [`migrate`](crate::migrate).
    Migrate(sqlx::migrate::MigrateError),
}

impl From<sqlx::Error> for PostgresError {
    fn from(err: sqlx::Error) -> Self {
        PostgresError::Sql(err)
    }
}

impl std::fmt::Display for PostgresError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PostgresError::Sql(e) => write!(f, "postgres error: {e}"),
            PostgresError::Migrate(e) => write!(f, "migration failed: {e}"),
        }
    }
}

impl std::error::Error for PostgresError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PostgresError::Sql(e) => Some(e),
            PostgresError::Migrate(e) => Some(e),
        }
    }
}
