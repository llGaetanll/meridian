//! The process was signalled.
//!
//! Shutdown is a decision like any other: whether there is work to finish
//! first, whether a cursor still needs saving, whether an in-flight sync should
//! be waited on. Routing signals through the machine means that decision is
//! made in the one place that can be replayed, rather than in a signal handler.
//!
//! The variants are the signals themselves rather than an interpretation of
//! them. What a SIGINT *means* is the machine's business, and naming the
//! variant `ShutdownRequested` would have decided that at the edge — which is
//! precisely the sort of decision the runtime is not supposed to make. It also
//! keeps the door open: SIGHUP means "reload your config" to most daemons, and
//! that arm can be added without reopening what the others were called.

#[derive(Debug)]
pub enum EventTerminal {
    /// ^C, or whatever else sends SIGINT. Interactive: someone is watching.
    SigInt,

    /// SIGTERM — a supervisor or an orchestrator asking the process to stop.
    /// Usually followed by a SIGKILL if we take too long about it.
    SigTerm,
}
