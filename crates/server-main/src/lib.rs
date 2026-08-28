//! The server, which exists because the phone cannot hold the Plaid secret.
//!
//! `docs/PLAID.md`: the secret grants full access to every Item on the account,
//! so it cannot ship inside an app anyone can unzip. Every call that carries it
//! happens here, and the only thing the phone ever sees is the transactions
//! that come back.
//!
//! **Sandbox only.** `PlaidEnv` defaults to Sandbox and needs a deliberate
//! `PLAID_ENV=production` to be anything else; nothing here sets it, and
//! [`runtime_plaid::fetch_all`] checks the environment before it calls out.
//!
//! The crate is split the way every program in this workspace is: a state
//! machine that decides ([`StateServer`]) and a runtime that acts
//! ([`RuntimeServer`]). `main.rs` does nothing but start the latter.

mod action;
mod action_client;
mod action_process;
mod action_source;
mod action_store;
mod event;
mod event_client;
mod event_process;
mod event_source;
mod event_store;
mod event_terminal;
mod project;
mod runtime_plaid;
mod runtime_server;
mod state_server;

pub use action::Action;
pub use action_client::ActionClient;
pub use action_process::ActionProcess;
pub use action_source::ActionSource;
pub use action_store::ActionStore;
pub use event::Event;
pub use event_client::EventClient;
pub use event_process::EventProcess;
pub use event_source::EventSource;
pub use event_store::EventStore;
pub use event_terminal::EventTerminal;
pub use runtime_server::RuntimeServer;
pub use server_wire::WireSnapshot;
pub use server_wire::WireSource;
pub use state_server::FRESH_FOR_SECS;
pub use state_server::StateServer;

/// Reachable so that `examples/snapshot.rs` can generate the fixture the app's
/// tests read. Not part of the server's interface.
pub use runtime_plaid::Fetched;
pub use runtime_plaid::fetch_fixtures;
