//! The process was signalled.
//!
//! Shutdown is a decision like any other: whether there is work to finish
//! first, whether a cursor still needs saving, whether an in-flight sync should
//! be waited on. Routing SIGINT through the machine means that decision is made
//! in the one place that can be replayed, rather than in a signal handler.

#[derive(Debug)]
pub enum EventTerminal {
    /// SIGINT or SIGTERM. The machine gets one chance to ask for whatever has
    /// to happen before the process goes away.
    ShutdownRequested,
}
