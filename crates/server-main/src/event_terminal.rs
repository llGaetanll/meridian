//! The process was signalled.
//!
//! The variants are the signals themselves rather than an interpretation of
//! them: what a SIGINT *means* is the machine's business, and a variant named
//! `ShutdownRequested` would have decided that at the edge.

#[derive(Debug)]
pub enum EventTerminal {
    /// ^C, or whatever else sends SIGINT. Interactive: someone is watching.
    SigInt,

    /// SIGTERM — a supervisor or an orchestrator asking the process to stop.
    /// Usually followed by a SIGKILL if we take too long about it.
    SigTerm,
}
