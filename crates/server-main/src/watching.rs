//! A route parked on a Link session Plaid has not reported on yet.

use api_common::Timestamp;

use crate::RequestId;

/// One `/link/finish` waiting to be told how a bank sign-in ended.
#[derive(Debug)]
pub struct Watching {
    pub request_id: RequestId,

    /// When it parked, so it can be let go rather than held.
    pub since: Timestamp,
}
