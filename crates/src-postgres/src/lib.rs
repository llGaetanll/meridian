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

mod postgres_audit;
mod postgres_delete_code;
mod postgres_item;
mod postgres_ledger;
mod postgres_link;
mod postgres_login;
mod postgres_migrate;
mod postgres_money;
mod postgres_pool;
mod postgres_secret;
mod postgres_session;
mod postgres_subscription;
mod postgres_user;

pub use postgres_audit::AuditScope;
pub use postgres_audit::StoredAccess;
pub use postgres_audit::load_accesses;
pub use postgres_audit::record_access;
pub use postgres_delete_code::load_delete_code;
pub use postgres_delete_code::record_delete_attempt;
pub use postgres_delete_code::save_delete_code;
pub use postgres_item::StoredItem;
pub use postgres_item::load_item;
pub use postgres_item::load_items;
pub use postgres_item::name_item;
pub use postgres_item::remove_item;
pub use postgres_item::save_cursor;
pub use postgres_item::save_error;
pub use postgres_item::save_item;
pub use postgres_ledger::apply_entries;
pub use postgres_ledger::load_accounts;
pub use postgres_ledger::load_entries;
pub use postgres_ledger::load_streams;
pub use postgres_ledger::save_accounts;
pub use postgres_ledger::save_streams;
pub use postgres_link::PendingLink;
pub use postgres_link::collect_link;
pub use postgres_link::pending_links;
pub use postgres_link::repair_for_link;
pub use postgres_link::save_link;
pub use postgres_link::user_for_link;
pub use postgres_login::StoredCode;
pub use postgres_login::delete_code;
pub use postgres_login::load_code;
pub use postgres_login::record_attempt;
pub use postgres_login::save_code;
pub use postgres_migrate::MIGRATOR;
pub use postgres_migrate::migrate;
pub use postgres_pool::Db;
pub use postgres_pool::PostgresError;
pub use postgres_pool::connect;
pub use postgres_pool::ping;
pub use postgres_secret::DbKey;
pub use postgres_session::create_session;
pub use postgres_session::delete_session;
pub use postgres_session::session_expiry;
pub use postgres_session::sweep_sessions;
pub use postgres_session::user_for_session;
pub use postgres_subscription::BillingProvider;
pub use postgres_subscription::StoredSubscription;
pub use postgres_subscription::apply_apple_event;
pub use postgres_subscription::apply_subscription_event;
pub use postgres_subscription::load_subscription;
pub use postgres_subscription::load_subscriptions;
pub use postgres_subscription::name_subscription;
pub use postgres_subscription::save_customer;
pub use postgres_subscription::user_for_customer;
pub use postgres_user::delete_user;
pub use postgres_user::load_access_tokens;
pub use postgres_user::load_email;
pub use postgres_user::upsert_user;
