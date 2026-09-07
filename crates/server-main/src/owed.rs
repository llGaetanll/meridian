//! What a request is owed, once everything it started has landed.

use uuid::Uuid;

use crate::ActionRouter;

/// An answer a route is owed, held until nothing it started is still out.
///
/// Two shapes, because a snapshot is not decided until it is read: the read
/// for it must not go out until the writes it would race are done.
#[derive(Debug)]
pub enum Owed {
    Answer(ActionRouter),

    /// A read first, and the answer is whatever it finds.
    Snapshot {
        user_id: Uuid,
    },
}
