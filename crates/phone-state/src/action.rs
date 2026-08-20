//! What to do about it. Actions are the only IO the machine asks for; a
//! runtime on each platform carries them out and feeds the result back in as
//! the next event.

use crate::ActionSource;
use crate::ActionUi;

/// `ToUi` is far larger than `ToSource`, which clippy notices. It is
/// deliberate: the whole screen crosses in one action so that the view holds
/// no state to apply a diff against (see [`ActionUi::Render`]). Boxing would
/// not help — UniFFI copies the payload across the boundary either way, so the
/// indirection would cost an allocation and buy nothing.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design"
)]
#[derive(Debug, uniffi::Enum)]
pub enum Action {
    ToUi { action: ActionUi },
    ToSource { action: ActionSource },
}
