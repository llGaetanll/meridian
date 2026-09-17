//! One open Link session, as a sheet-close finds it.

/// What the machine needs to know about one uncollected Link session: which
/// it is, and whether it repairs an Item rather than attaches one.
///
/// A projection of the store's row. Who opened it and when stay behind — the
/// read was already scoped to the caller, and the machine decides nothing
/// from a timestamp here.
#[derive(Debug, Clone)]
pub struct PendingLinkFacts {
    pub link_token: String,

    /// The Item this session repairs — Plaid's update mode. `None` is the
    /// ordinary session that attaches, whose finish is exchanged.
    pub repair_of: Option<String>,
}
