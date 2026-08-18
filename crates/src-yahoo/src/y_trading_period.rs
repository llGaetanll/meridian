use serde::Deserialize;

use crate::YTradingSession;

#[derive(Debug, Deserialize)]
pub struct YTradingPeriod {
    pub pre: Option<YTradingSession>,
    pub regular: Option<YTradingSession>,
    pub post: Option<YTradingSession>,
}
