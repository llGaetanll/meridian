//! Who the device is signed in as, for the corner of the app that says so.
//!
//! Its own value rather than a field on a screen: the account chip is drawn
//! beside every screen, the way the dock is, so it rides with the dock rather
//! than with whichever page happens to be up.

/// The two lines the account chip prints, and the words of the menu behind it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewWho {
    /// The line it leads with: the name, or the address for an account that
    /// has never been asked for one.
    pub title: String,

    /// The line under it, which is the address — absent when the title is
    /// already the address, so the chip never prints it twice.
    pub subtitle: Option<String>,

    /// `"Settings"`.
    pub settings_label: String,

    /// `"Sign out"`.
    pub sign_out_label: String,

    /// `"Delete my account"`.
    pub delete_label: String,
}
