//! The view layer. Components take finished values and print them.
//!
//! Nothing here decides anything: no formatting, no sorting, no working out
//! what a row says. That is all in `api-core`, which is what makes this crate
//! a transcription target for SwiftUI and Compose rather than a thing to be
//! reimplemented.

mod icon;
mod rows;
mod shell;

pub use icon::BrandMark;
pub use icon::Icon;
pub use rows::Breakdown;
pub use rows::ConnectionBlock;
pub use rows::Masked;
pub use rows::StreamRowView;
pub use rows::TxRowView;
pub use shell::PhoneShell;
