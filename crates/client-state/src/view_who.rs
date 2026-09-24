//! Who the device is signed in as, for the corner of the app that says so.
//!
//! Its own value rather than a field on a screen: the account chip is drawn
//! beside every screen, the way the dock is, so it rides with the dock rather
//! than with whichever page happens to be up.

/// The two lines the account chip prints, and the words of the menu behind it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewWho {
    /// The line it leads with: the name. Never empty — an account is seeded
    /// with one when it is made, so there is no state where the chip has
    /// nothing to lead with.
    pub title: String,

    /// The line under it: the address.
    pub subtitle: String,

    /// `"Settings"`.
    pub settings_label: String,

    /// `"Sign out"`.
    pub sign_out_label: String,

    /// `"Delete my account"`.
    pub delete_label: String,
}
