//! A call in flight, and the route that wants its answer.

use api_common::Timestamp;

use crate::RequestId;

/// One call out to a source or the store, and who is parked on it.
#[derive(Debug)]
pub struct Waiting {
    pub request_id: RequestId,

    /// When the call went out, so one that never answers can be given up on.
    pub since: Timestamp,
}
