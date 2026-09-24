//! Talking to the outside world. No reactive state lives here.

pub mod calm;
pub mod clock;
mod failure;
mod link_outcome;
mod markets;
mod meridian;
mod popup;
pub mod session;
mod sse;
mod yahoo;

pub use failure::Failure;
pub use link_outcome::LinkOutcome;
pub use markets::AssetQuote;
pub use markets::Markets;
pub use meridian::Meridian;
pub use popup::Popup;
pub use sse::Sse;
pub use yahoo::ChartRange;
