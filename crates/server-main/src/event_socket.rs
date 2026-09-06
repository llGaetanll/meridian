//! Something happened to a stream that nobody asked about.
//!
//! Opening one is not here: a stream is opened by an ordinary request, on
//! [`EventRouter::SyncWatchRequested`](crate::EventRouter), so that a caller
//! with no session is refused before any of the response has gone out. What is
//! left is the half a request cannot carry.

use crate::ConnId;

#[derive(Debug)]
pub enum EventSocket {
    /// The other end went away — backgrounded, out of signal, or shut. The
    /// runtime reports it; whether to stop holding anything for that caller is
    /// the machine's to decide.
    Closed { conn_id: ConnId },
}
