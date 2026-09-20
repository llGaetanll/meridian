//! The Account screen: which banks are attached, and the way out.
//!
//! Deliberately small — everything a setting could configure already lives on
//! the screen it affects. No address either: nothing the server sends says who
//! the device is signed in as.

/// Everything the Account tab shows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccount {
    /// `"Banks"`.
    pub banks_label: String,

    /// The same connections the Balances screen draws, one row each.
    pub banks: Vec<ViewAccountBank>,

    /// What the list says when there are none.
    pub banks_empty: Option<String>,

    /// `"Session"`.
    pub session_label: String,

    /// `"Sign out"`.
    pub sign_out_label: String,

    /// `"Delete my account"`.
    pub delete_label: String,

    /// The delete confirmation, while somebody is leaving. `None` draws
    /// nothing over the screen.
    pub delete: Option<ViewAccountDelete>,

    /// `"Meridian 0.1.0"`.
    pub build: String,
}

/// The delete confirmation, at whichever step it is on: the "are you sure"
/// popup, then the emailed code.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountDelete {
    pub title: String,
    pub body: String,

    /// The digits typed so far, present exactly when the code field is drawn.
    pub code: Option<String>,

    /// `"That code was not right. Check the email and try again."`, after a
    /// refused code.
    pub error: Option<String>,

    /// The destructive button, absent while a request is out or the code is
    /// not yet six digits.
    pub confirm_label: Option<String>,

    /// The safe way out. Absent only while the deletion itself is out, when
    /// there is nothing left to back out of.
    pub cancel_label: Option<String>,

    /// Whether a request is out, so the popup can read busy.
    pub busy: bool,
}

/// One bank, as this screen lists it: a name and whether it is still working.
///
/// Not [`ViewConnection`](crate::ViewConnection), which also carries the
/// account rows and their balances.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountBank {
    pub id: String,
    pub name: String,
    pub initials: String,

    /// `"8 accounts · synced 2m ago"`, or what is wrong with it.
    pub detail: String,

    /// Whether the bank has stopped syncing and wants a fresh sign-in.
    pub needs_reconnect: bool,
}
