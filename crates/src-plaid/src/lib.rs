//! Plaid, the bank connection.
//!
//! A stub. Nothing here talks to anything yet — it exists so the shape is
//! written down and the work has somewhere to go.
//!
//! Like `src-yahoo` and `src-coingecko` this is IO only: plain async functions
//! over plain data, no signals and no decisions. Requests will carry a path
//! resolved against a base, so the browser can point at a proxy while native
//! builds go direct.

mod transaction;

pub use transaction::*;
