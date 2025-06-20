use serde::Deserialize;

use crate::YQuoteBlock;

#[derive(Debug, Deserialize)]
pub struct YChart {
    pub result: Vec<YQuoteBlock>,
}
