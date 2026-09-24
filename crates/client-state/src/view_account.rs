//! The Account screen: who you are, which banks are attached, and the way out.
//!
//! Deliberately small — everything a setting could configure already lives on
//! the screen it affects. What is here is what belongs to the account rather
//! than to a screen: the name the app calls you by, the banks, and the two
//! doors out.

use crate::ViewField;

/// Everything the Account tab shows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccount {
    /// The name the app calls them by, and the field it is changed in.
    ///
    /// Absent until a snapshot has said who this is — the same moment the
    /// account chip gets its two lines.
    pub name: Option<ViewAccountName>,

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

/// The name, and what it takes to change it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewAccountName {
    /// `"Your name"`.
    pub label: String,

    /// What is in the field — the kept name until somebody starts typing over
    /// it, and what they have typed after that.
    pub field: ViewField,

    /// `Some("Save")` once the field holds something that is not what is
    /// already saved. Absent while it is blank, and absent while it is
    /// unchanged: a button that writes what is already there does nothing and
    /// says it did something.
    pub action: Option<String>,
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
