//! Postgres, and the only place in the workspace that speaks SQL.
//!
//! An `src-*` crate like [`src_plaid`](../src_plaid/index.html): IO only, no
//! decisions. Every statement the app runs is a function in here, so a query
//! cannot be written at a call site and no other crate has to know what the
//! schema looks like — `server-main` calls functions, not SQL.
//!
//! ## Where this sits in the event loop
//!
//! A query is IO, so it is an action out and an event back, the same as a
//! trip to Plaid. What it is *not* is a lookup on the request path: the server
//! holds its ledger in memory and answers from what it holds, and Postgres is
//! where that survives a restart. So writes happen as they land, and reads
//! happen once at startup. `docs/ARCHITECTURE.md` has the reasoning.

mod postgres_migrate;
mod postgres_pool;

pub use postgres_migrate::MIGRATOR;
pub use postgres_migrate::migrate;
pub use postgres_pool::Db;
pub use postgres_pool::PostgresError;
pub use postgres_pool::connect;
pub use postgres_pool::ping;
