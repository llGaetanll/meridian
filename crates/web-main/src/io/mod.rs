//! Talking to the outside world. No reactive state lives here.

mod markets;
mod yahoo;

pub use markets::AssetQuote;
pub use markets::Markets;
pub use yahoo::ChartRange;
