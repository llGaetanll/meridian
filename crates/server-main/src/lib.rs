//! The server, which exists because the phone cannot hold the Plaid secret.
//!
//! `docs/PLAID.md`: the secret grants full access to every Item on the account,
//! so it cannot ship inside an app anyone can unzip. Every call that carries it
//! happens here, and the only thing the phone ever sees is the transactions
//! that come back.
//!
//! **Sandbox only.** `PLAID_ENV` is required and matched exactly — there is no
//! default and nothing is inferred — and [`runtime_plaid::fetch_all`] checks
//! the environment again before it calls out.
//!
//! Every variable this server reads is required. A server that starts without
//! credentials and serves something plausible instead is a server whose empty
//! screen has two explanations, and the log says the same thing for both.
//!
//! The crate is split the way every program in this workspace is: a state
//! machine that decides ([`StateServer`]) and a runtime that acts
//! ([`RuntimeServer`]). `main.rs` does nothing but start the latter.

mod action;
mod action_email;
mod action_process;
mod action_router;
mod action_source;
mod action_store;
mod email_send;
mod event;
mod event_process;
mod event_router;
mod event_source;
mod event_store;
mod event_terminal;
mod item_facts;
mod link_poll;
mod login_code;
mod plaid_collect;
mod project;
mod router_server;
mod runtime_plaid;
mod runtime_server;
mod session_token;
mod state_server;

pub use action::Action;
pub use action_email::ActionEmail;
pub use action_process::ActionProcess;
pub use action_router::ActionRouter;
pub use action_source::ActionSource;
pub use action_store::ActionStore;
pub use email_send::EmailSend;
pub use event::Event;
pub use event_process::EventProcess;
pub use event_router::EventRouter;
pub use event_source::EventSource;
pub use event_store::EventStore;
pub use event_terminal::EventTerminal;
pub use item_facts::ItemFacts;
pub use link_poll::LinkPoll;
pub use login_code::StoredCode;
pub use login_code::Verdict;
/// Reachable so the machine's tests can build a stored code to submit
/// against. Not part of the server's interface.
pub use login_code::hash as hash_code;
pub use plaid_collect::PlaidCollect;
pub use runtime_server::RuntimeServer;
pub use server_wire::WireSnapshot;
pub use state_server::FRESH_FOR_SECS;
pub use state_server::StateServer;

/// Reachable so that `examples/snapshot.rs` can generate the fixture the app's
/// tests read. Not part of the server's interface.
pub use runtime_plaid::Fetched;
pub use runtime_plaid::fetch_fixtures;
