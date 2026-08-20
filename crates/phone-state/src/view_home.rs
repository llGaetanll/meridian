//! The Home screen: the highlight of each tab and nothing more.

use crate::ViewBreakdownSlice;
use crate::ViewConnection;
use crate::ViewStreamRow;
use crate::ViewTxRow;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewHome {
    pub balances: ViewHomeBalances,
    pub recurring: ViewHomeRecurring,

    /// The latest few ledger rows.
    pub ledger: Vec<ViewTxRow>,

    /// Connections needing attention. Empty when everything is syncing, and
    /// the block is not drawn at all then.
    pub attention: Vec<ViewConnection>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewHomeBalances {
    /// `"$86,541.74"`.
    pub headline: String,
    pub masked: bool,
    /// `"held across 14 accounts"`.
    pub detail: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewHomeRecurring {
    /// `"−$3,771.40"`.
    pub headline: String,
    pub masked: bool,
    /// `"3 due within the week"`.
    pub detail: String,
    pub breakdown: Vec<ViewBreakdownSlice>,
    /// Only what lands next.
    pub upcoming: Vec<ViewStreamRow>,
}
