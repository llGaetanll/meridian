//! An outcome nobody has asked about yet.

use api_common::Timestamp;

use crate::LinkOutcome;

/// What one Link session came to, kept for the route that will ask.
#[derive(Debug)]
pub struct Settled {
    pub outcome: LinkOutcome,

    /// When it was recorded, so one nobody collects does not sit here.
    pub at: Timestamp,
}
