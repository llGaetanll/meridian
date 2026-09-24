//! One trip to a market data source, from planned to written down.

use api_common::Timestamp;
use src_postgres::BarInterval;

use crate::CallId;
use crate::RequestId;

/// One fetch in flight, and the step it is at.
///
/// The variant is the step, so the table of these is the list of what the
/// machine owes the sources — an entry still here after its bars are written
/// is a leak by definition. Requests park on a fetch rather than owning one:
/// two clients asking overlapping windows wait on the same entry, which is
/// what makes "never ask twice" hold for requests in flight and not only for
/// windows already stored.
#[derive(Debug)]
pub struct MarketFetch {
    pub symbol: String,
    pub interval: BarInterval,

    /// Half-open, what will be asked of the source.
    pub start: Timestamp,
    pub end: Timestamp,

    /// The routes held until this fetch is settled, and since when. Empty is
    /// the machine fetching for itself, with nobody to answer.
    pub waiters: Vec<MarketWaiter>,

    /// Failures so far, so a source that keeps failing is given up on rather
    /// than asked forever. A 429 does not count: it is the pace that was
    /// wrong, not the ask.
    pub attempts: u32,

    pub step: MarketFetchStep,
}

/// Where a fetch is.
#[derive(Debug)]
pub enum MarketFetchStep {
    /// Waiting its turn behind the pace the source is being asked at.
    Queued,

    /// Out at the source, on this call. `since` is what lets one that never
    /// answers be given up on.
    Out { call_id: CallId, since: Timestamp },

    /// Answered, and waiting on the write that makes it knowledge. The
    /// waiters are held through this step: a route answered before the write
    /// lands would read the store the moment before its bars arrive.
    Saving { call_id: CallId },
}

/// One route parked on a fetch, and since when — so a request is answered
/// with what is held rather than held forever.
#[derive(Debug)]
pub struct MarketWaiter {
    pub request_id: RequestId,
    pub since: Timestamp,
}
