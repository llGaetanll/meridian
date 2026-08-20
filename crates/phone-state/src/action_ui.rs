//! Something for the view to do.

use crate::ViewDock;
use crate::ViewScreen;

/// These carry finished values, not instructions to go and compute one. A
/// platform that receives `Render` draws exactly what it is given.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design — see Render"
)]
#[derive(Debug, uniffi::Enum)]
pub enum ActionUi {
    /// Draw this screen. The whole screen, in the state it should be in.
    ///
    /// One action rather than a set of fine-grained ones — no `ShowTxs`,
    /// `SelectTab`, `OpenFilter`. A view that applies a diff has to hold the
    /// state the diff applies to, and holding state above the boundary is the
    /// thing this design exists to prevent. Sending the whole screen means the
    /// view can be a pure function of the last action it saw.
    Render { screen: ViewScreen, dock: ViewDock },

    /// Tell the user something went wrong, in these words.
    ShowError { message: String },
}
