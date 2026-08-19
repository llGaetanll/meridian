//! The server's insides, as a library.
//!
//! `main.rs` is the binary that wires these together; everything worth testing
//! or reusing — the state machine, the projection, the Plaid IO — lives here so
//! that `examples/` and future tools can reach it without a `#[path]` include.

pub mod core;
pub mod plaid_io;
pub mod project;
