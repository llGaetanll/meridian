use serde::Deserialize;

use crate::YIndicators;
use crate::YMeta;

#[derive(Debug, Deserialize)]
pub struct YQuoteBlock {
    pub meta: YMeta,
    pub timestamp: Vec<u64>,
    pub indicators: YIndicators,
}
