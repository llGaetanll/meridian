//! The server, which exists because the phone cannot hold the Plaid secret.
//!
//! Every variable this server reads is required and matched exactly: a server
//! that starts without credentials and serves something plausible instead is a
//! server whose empty screen has two explanations.
//!
//! Split the way every program in this workspace is: a state machine that
//! decides ([`StateServer`]) and a runtime that acts ([`RuntimeServer`]).

mod action;
mod action_email;
mod action_process;
mod action_router;
mod action_socket;
mod action_source;
mod action_store;
mod call_id;
mod client;
mod conn_id;
mod email_address;
mod email_send;
mod event;
mod event_process;
mod event_router;
mod event_socket;
mod event_source;
mod event_store;
mod event_terminal;
mod item_facts;
mod link_poll;
mod login_code;
mod plaid_collect;
mod project;
mod public_url;
mod rate_limit;
mod request_id;
mod router_server;
mod router_trace;
mod runtime_plaid;
mod runtime_router;
mod runtime_server;
mod session_token;
mod source_pool;
mod state_server;
mod web_origins;
mod webhook_facts;

pub use action::Action;
pub use action_email::ActionEmail;
pub use action_process::ActionProcess;
pub use action_router::ActionRouter;
pub use action_socket::ActionSocket;
pub use action_source::ActionSource;
pub use action_store::ActionStore;
pub use call_id::CallId;
pub use client::Client;
pub use client::ClientIp;
pub use conn_id::ConnId;
pub use email_send::EmailSend;
pub use event::Event;
pub use event_process::EventProcess;
pub use event_router::EventRouter;
pub use event_socket::EventSocket;
pub use event_source::EventSource;
pub use event_store::EventStore;
pub use event_terminal::EventTerminal;
pub use item_facts::ItemFacts;
pub use link_poll::LinkPoll;
pub use login_code::StoredCode;
pub use login_code::Verdict;
/// Reachable so the machine's tests can build a stored code to submit against.
pub use login_code::hash as hash_code;
pub use plaid_collect::PlaidCollect;
pub use public_url::PublicUrl;
pub use public_url::WEBHOOK_PATH;
pub use rate_limit::Route;
pub use request_id::RequestId;
pub use runtime_server::RuntimeServer;
pub use server_wire::WireSnapshot;
/// Reachable so the machine's tests can build the token hash a request carries.
pub use session_token::hash as hash_token;
pub use state_server::CALL_GOOD_FOR_SECS;
pub use state_server::FRESH_FOR_SECS;
pub use state_server::LINK_ANSWER_GOOD_FOR_SECS;
pub use state_server::OUTCOME_GOOD_FOR_SECS;
pub use state_server::SWEEP_SESSIONS_EVERY_SECS;
pub use state_server::StateServer;
pub use state_server::WATCHERS_AT_MOST;
pub use state_server::WEBHOOK_KEY_MISS_GOOD_FOR_SECS;
pub use state_server::WEBHOOK_KEYS_AT_MOST;
pub use web_origins::WebOrigins;
pub use webhook_facts::WebhookFacts;

/// Reachable so that `examples/snapshot.rs` can generate the fixture the app's
/// tests read.
pub use runtime_plaid::Fetched;
pub use runtime_plaid::fetch_fixtures;
