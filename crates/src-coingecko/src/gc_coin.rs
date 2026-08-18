use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GcCoin {
    pub id: String,
    pub symbol: String,
    pub name: String,
}
