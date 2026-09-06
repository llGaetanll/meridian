//! Something to write to a stream that is already open.
//!
//! The first message is not here: it rides on
//! [`ActionRouter::Watching`](crate::ActionRouter), which is what makes it
//! impossible to accept a connection without telling it where things stand.

use crate::ConnId;

#[derive(Debug)]
pub enum ActionSocket {
    /// How far this caller's sync has got, whole rather than as a change since
    /// the last one. A client that connected late, reconnected, or missed a
    /// message is then in the same position as one that saw every message.
    Send {
        conn_id: ConnId,
        status: server_wire::WireSyncStatus,
    },
}
