//! A call in flight, and the route that wants its answer.

use api_common::Timestamp;

use crate::RequestId;

/// One call out to a source or the store, and who is parked on it.
///
/// Keyed by the call rather than the route, because that is what an answer
/// names: this is how a call finds the route it was made for, and how a call
/// that never answers is found by the tick.
#[derive(Debug)]
pub struct Waiting {
    pub request_id: RequestId,

    /// When the call went out, so one that never answers can be given up on.
    pub since: Timestamp,
}
