//! What to do about it. Actions are the only IO the machine asks for.

use crate::ActionSource;
use crate::ActionUi;

/// What the machine asks for, and of whom.
///
/// `ToUi` is far larger than `ToSource` because the whole screen crosses in
/// one action (see [`ActionUi::Render`]). Boxing would not help — UniFFI
/// copies the payload across the boundary either way.
#[expect(
    clippy::large_enum_variant,
    reason = "the whole screen crosses at once, by design"
)]
#[derive(Debug, uniffi::Enum)]
pub enum Action {
    ToUi { action: ActionUi },
    ToSource { action: ActionSource },
}
