//! View models for the markets page. They describe what a component shows, not
//! how the ledger stores it, so any data source can feed them.

use api_common::Asset;
use api_common::Price;
use api_common::Quantity;
use api_common::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TradeSide {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::Sell => "Sell",
        }
    }
}

/// What a past trade is worth today: still open, or closed out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TradeResult {
    Unrealized(Price),
    Realized(Price),
}

/// One row of the "trades involving X" table.
#[derive(Debug, Clone)]
pub struct AssetTrade {
    pub ts: Timestamp,
    pub side: TradeSide,
    pub portfolio: String,
    pub qty: Quantity,
    pub price: Price,
    pub value: Price,
    pub result: TradeResult,
}

/// The user's whole position in an asset, across every portfolio.
#[derive(Debug, Clone)]
pub struct AssetPosition {
    pub asset: Asset,
    pub qty: Quantity,
    pub value: Price,
    pub avg_cost: Price,
    pub unrealized: Price,
    pub realized: Price,
    pub net_invested: Price,
    pub today: Price,
}

/// The slice of one portfolio that a given asset accounts for.
#[derive(Debug, Clone)]
pub struct PortfolioHolding {
    pub portfolio: String,
    pub qty: Quantity,
    pub value: Price,
    /// `0.0..=1.0` of that portfolio's total value.
    pub share_of_portfolio: f64,
}

/// A compact price line for lists of assets.
#[derive(Debug, Clone)]
pub struct Quote {
    pub symbol: Asset,
    pub name: String,
    pub price: Price,
    /// Change over the displayed period, as a ratio.
    pub change: f64,
}
