//! Something for the view to do.

use crate::ViewDock;
use crate::ViewScreen;
use crate::ViewWho;

/// Something for the view to do. These carry finished values, not
/// instructions to go and compute one.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design — see Render"
)]
#[derive(Debug, uniffi::Enum)]
pub enum ActionUi {
    /// Draw this screen. The whole screen, in the state it should be in.
    ///
    /// One action rather than a set of fine-grained ones: a view that applies
    /// a diff has to hold the state the diff applies to. The dock is `None` on
    /// a screen that is not one of its tabs, rather than an empty dock, which
    /// would still be a strip of ground at the bottom of the phone.
    Render {
        screen: ViewScreen,
        dock: Option<ViewDock>,

        /// Who the device is signed in as, for the account chip. Present
        /// exactly when the dock is: both belong to the app rather than to
        /// the road in, and neither is drawn over first run or the paywall.
        who: Option<ViewWho>,
    },

    /// Tell the user something went wrong, in these words.
    ShowError { message: String },
}
