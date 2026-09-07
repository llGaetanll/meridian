//! A Link session between Plaid saying what it came to and a route asking.

use api_common::Timestamp;

use crate::LinkOutcome;
use crate::RequestId;

/// One Link session, and which half of the race arrived first.
///
/// Plaid's report and the route that asks for it have no ordering between
/// them, so whichever arrives first waits here for the other. One entry per
/// `link_token`, so a session is in one state at a time: reported and waiting
/// to be asked about, or asked about and waiting to be reported.
#[derive(Debug)]
pub enum LinkSession {
    /// Plaid has said, and nobody has asked. Swept once it is older than the
    /// sheet that opened it.
    Settled { outcome: LinkOutcome, at: Timestamp },

    /// A route has asked, and Plaid has not said. `since` is when it parked,
    /// so it can be let go rather than held.
    Watched {
        request_id: RequestId,
        since: Timestamp,
    },
}
