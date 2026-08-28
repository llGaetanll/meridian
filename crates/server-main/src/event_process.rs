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
//!
//! It carries nothing. It used to say whether there was a store and whether
//! the source was live, back when either could be absent. Neither can: the
//! store is required and so are the credentials, so the only fact left is
//! that the process started, which is the variant itself.

#[derive(Debug)]
pub enum EventProcess {
    /// The server came up, and this is what it came up with.
    ///
    /// `store` is a *fact the runtime observed* — it read the environment and
    /// either connected or did not — and not a decision. What to do about it
    /// is the machine's: with a store it asks to be filled, and without one it
    /// starts empty and fetches, which is what a first run does anyway.
    ///
    /// It is on the event rather than checked where the store is used, because
    /// a runtime that inspects its own state to decide which events to emit is
    /// a runtime with behaviour in it — behaviour no `eval!` test can reach and
    /// no event log records.
    Started,
}
