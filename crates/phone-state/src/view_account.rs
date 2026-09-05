//! The Account screen: which banks are attached, and the way out.
//!
//! Deliberately small. Everything a setting could configure — the range, the
//! categories, what is covered — already lives on the screen it affects, so
//! what is left for this tab is the connections and the session.
//!
//! There is no address on it. Nothing the server sends says who the device is
//! signed in as, and a coloured initial would be a face invented for somebody
//! who never gave us a name. When `/snapshot` carries the address, it goes
//! here.

/// Everything the Account tab shows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccount {
    /// `"Banks"`.
    pub banks_label: String,

    /// The same connections the Balances screen draws, one row each.
    pub banks: Vec<ViewAccountBank>,

    /// What the list says when there are none — which is three different
    /// nothings, and they need different words.
    pub banks_empty: Option<String>,

    /// `"Session"`.
    pub session_label: String,

    /// `"Sign out"`.
    pub sign_out_label: String,

    /// `"Meridian 0.1.0"`. Nobody reads it until something is wrong, and then
    /// it is the first thing we ask them for.
    pub build: String,
}

/// One bank, as this screen lists it.
///
/// Not [`ViewConnection`](crate::ViewConnection): that one carries the account
/// rows and their balances, which is the Balances screen's business. Here a
/// bank is a name and whether it is still working.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountBank {
    pub id: String,
    pub name: String,
    pub initials: String,

    /// `"8 accounts · synced 2m ago"`, or what is wrong with it.
    pub detail: String,

    /// Whether the bank has stopped syncing and wants a fresh sign-in. The one
    /// thing in the app that stops the numbers being true, so the row says so
    /// rather than hiding it.
    pub needs_reconnect: bool,
}
