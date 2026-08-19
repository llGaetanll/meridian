//! What to do about it. Actions are the only IO the core asks for; a thin
//! runtime on each platform carries them out and feeds the result back in as
//! the next event.

use crate::view::DockView;
use crate::view::ScreenView;

/// `Render` is far larger than the other variants, which clippy notices. It is
/// deliberate: the whole screen crosses in one action so that the view holds no
/// state to apply a diff against (see the note on `UiAction::Render`). Boxing
/// it would not help — UniFFI copies the payload across the boundary either
/// way, so the indirection would cost an allocation and buy nothing.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design"
)]
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Action {
    ToUi { action: UiAction },
    ToSource { action: SourceAction },
}

/// Something for the view to do.
///
/// These carry finished values, not instructions to go and compute one. A
/// platform that receives `Render` draws exactly what it is given.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design"
)]
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum UiAction {
    /// Draw this screen. The whole screen, in the state it should be in.
    ///
    /// One action rather than a set of fine-grained ones — no `ShowTxs`,
    /// `SelectTab`, `OpenFilter`. A view that applies a diff has to hold the
    /// state the diff applies to, and holding state above the boundary is the
    /// thing this design exists to prevent. Sending the whole screen means the
    /// view can be a pure function of the last action it saw.
    Render { screen: ScreenView, dock: DockView },

    /// Tell the user something went wrong, in these words.
    ShowError { message: String },
}

/// Something for a source to do.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SourceAction {
    /// Go and get everything. `force` skips whatever cache sits in the way,
    /// which is what pull-to-refresh means.
    LoadSnapshot { force: bool },
}
