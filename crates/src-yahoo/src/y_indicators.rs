use serde::Deserialize;

use crate::YQuoteData;

#[derive(Debug, Deserialize)]
pub struct YIndicators {
    pub quote: Vec<YQuoteData>,
}
