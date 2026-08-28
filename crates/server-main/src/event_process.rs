//! The process did something to itself.
//!
//! The counterpart of [`ActionProcess`](crate::ActionProcess): that one is
//! what the machine asks the process to do, this is what the process reports
//! having done. Distinct from [`EventTerminal`](crate::EventTerminal), which is
//! the process being *signalled* from outside.
//!
//! One variant, and it is the one that makes the boot sequence replayable:
//! without it, reading state back at startup would be something the runtime
//! did on its own initiative rather than something the machine asked for.

#[derive(Debug)]
pub enum EventProcess {
    /// The server came up. Nothing has been read or fetched yet.
    Started,
}
