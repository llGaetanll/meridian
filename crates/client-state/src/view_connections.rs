//! The Connections screen: which logins the app has, and what they bring in.
//!
//! One block per login rather than one per side of the ledger: a bank is a
//! block, and the accounts under it are the ones it brings in, held and owed
//! together. What is owed is a minus sign on a row, not a section — the page
//! answers which logins we have and whether they are working, and splitting a
//! login in two answers it twice.

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewConnections {
    /// `"Connections"`. The page's name is its heading here rather than a
    /// figure, because there is no one figure this page is about.
    pub title: String,

    /// `"4 banks · 7 accounts"` — what the blocks come to, counted rather
    /// than added.
    pub count: String,

    /// `"Add connection"`.
    pub add_label: String,

    /// Whether a link is under way — the tap has happened and Plaid's page
    /// has not answered — so the button waits instead of taking another.
    pub adding: bool,

    /// One block per login, in the order they were connected.
    pub connections: Vec<ViewConnection>,

    /// What stands in place of the blocks when there are none, or when the
    /// snapshot they would be built from has not arrived.
    pub empty: Option<String>,
}

/// Removing one bank, at whichever step it is on: the "are you sure", the
/// wait, and what a refusal says.
///
/// The same shape as [`ViewAccountDelete`](crate::ViewAccountDelete) with the
/// code left out — what is lost is one connection's history rather than the
/// account, so the address is not asked to agree. It is asked in a card under
/// the button that opened it rather than over the page: one bank of four is
/// not something the whole window has to stop for.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewConnectionRemove {
    /// `"Remove Citi?"`.
    pub title: String,

    /// What goes with it.
    pub body: String,

    /// `"That did not work. Nothing was removed."`, after a refusal.
    pub error: Option<String>,

    /// The destructive button, absent while the request is out.
    pub confirm_label: Option<String>,

    /// The safe way out. Absent only while the request is out, when there is
    /// nothing left to back out of.
    pub cancel_label: Option<String>,

    /// Whether a request is out, so the popup can read busy.
    pub busy: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewConnection {
    pub id: String,

    /// Plaid's id for the connection, as a repair names it.
    pub item_id: String,

    pub name: String,

    /// `"Synced 2m ago"`, or empty when the connection needs attention — the
    /// button that fixes it takes the same place on the line.
    pub synced: String,

    /// Whether the user has to sign in again.
    pub needs_reconnect: bool,

    /// `"Sign in"` — what the button on an expired login says.
    pub reconnect_label: String,

    /// `"Remove bank"` — the name of the trash on this block, which is drawn
    /// as a glyph and so carries its words for a screen reader.
    pub remove_label: String,

    /// Whether a repair for this connection is under way — the tap has
    /// happened and Plaid's page has not answered — so the button waits
    /// instead of taking another.
    pub reconnecting: bool,

    pub accounts: Vec<ViewAccountRow>,

    /// The "are you sure" under this block's trash, while this is the bank
    /// being removed. On the block rather than on the page, because it is
    /// asked where it was pressed: the machine holds one removal at a time
    /// and this is how the view knows whose it is.
    pub remove: Option<ViewConnectionRemove>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountRow {
    pub id: String,
    pub name: String,

    /// `"Checking ·0000 · $100.00 available"` — the whole subtitle, assembled.
    pub subtitle: String,

    /// `"0000"` — the account's last four digits, which are what tell two
    /// checking accounts apart, for where they get a box of their own. Empty
    /// where the source did not send them.
    pub mask: String,

    /// `"Checking"` — what kind of account it is, on its own.
    pub kind: String,

    /// `"$100.00 available"` or `"4.10% APY"` — the rest of the subtitle, or
    /// empty.
    pub detail: String,

    /// `"$110.00"` or `"−$56,302.06"`.
    pub amount: String,

    /// Whether this is money owed, for colour.
    pub owed: bool,

    /// Whether the login behind it has expired, so the row is still true as
    /// far as anyone knows and no longer arriving.
    pub stale: bool,

    /// `"$410 of $2,000 limit used"`, where a limit exists.
    pub utilisation: Option<String>,
}
