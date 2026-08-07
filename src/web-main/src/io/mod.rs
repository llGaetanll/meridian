//! Talking to the outside world. No reactive state lives here.

mod yahoo;

pub use yahoo::AssetQuote;
pub use yahoo::ChartRange;
pub use yahoo::fetch_quote;
