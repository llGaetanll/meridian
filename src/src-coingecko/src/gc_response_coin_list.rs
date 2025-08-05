use serde::Deserialize;

use crate::GcCoin;

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct GcResponseCoinList {
    pub coins: Vec<GcCoin>,
}
