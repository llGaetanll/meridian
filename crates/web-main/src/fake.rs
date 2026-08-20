//! Stand-in data for the ledger side of the page: what the user holds and
//! traded. Prices come from Yahoo — see `crate::io`.

use api_common::Timestamp;
use web_components::AssetPosition;
use web_components::AssetTrade;
use web_components::PortfolioHolding;
use web_components::Quote;
use web_components::TradeResult;
use web_components::TradeSide;

/// The asset the page opens on, until it can be picked.
pub const SYMBOL: &str = "SPY";

pub fn position() -> AssetPosition {
    AssetPosition {
        asset: SYMBOL.to_string(),
        qty: 38.0,
        value: 24_690.12,
        avg_cost: 562.39,
        unrealized: 3_319.30,
        realized: 233.46,
        net_invested: 21_370.82,
        today: 430.16,
    }
}

pub fn holdings() -> Vec<PortfolioHolding> {
    vec![
        PortfolioHolding {
            portfolio: "Core".to_string(),
            qty: 20.0,
            value: 12_994.80,
            share_of_portfolio: 0.382,
        },
        PortfolioHolding {
            portfolio: "Retirement".to_string(),
            qty: 8.0,
            value: 5_197.92,
            share_of_portfolio: 0.264,
        },
        PortfolioHolding {
            portfolio: "Play money".to_string(),
            qty: 10.0,
            value: 6_497.40,
            share_of_portfolio: 0.225,
        },
    ]
}

pub fn trades() -> Vec<AssetTrade> {
    vec![
        AssetTrade {
            ts: Timestamp::from_secs(1_781_222_400), // 2026-06-12
            side: TradeSide::Buy,
            portfolio: "Play money".to_string(),
            qty: 10.0,
            price: 612.40,
            value: 6_124.00,
            result: TradeResult::Unrealized(373.40),
        },
        AssetTrade {
            ts: Timestamp::from_secs(1_772_668_800), // 2026-03-05
            side: TradeSide::Buy,
            portfolio: "Retirement".to_string(),
            qty: 8.0,
            price: 588.15,
            value: 4_705.20,
            result: TradeResult::Unrealized(492.72),
        },
        AssetTrade {
            ts: Timestamp::from_secs(1_769_040_000), // 2026-01-22
            side: TradeSide::Sell,
            portfolio: "Core".to_string(),
            qty: 6.0,
            price: 601.30,
            value: 3_607.80,
            result: TradeResult::Realized(233.46),
        },
        AssetTrade {
            ts: Timestamp::from_secs(1_763_078_400), // 2025-11-14
            side: TradeSide::Buy,
            portfolio: "Core".to_string(),
            qty: 12.0,
            price: 542.60,
            value: 6_511.20,
            result: TradeResult::Unrealized(1_285.68),
        },
        AssetTrade {
            ts: Timestamp::from_secs(1_754_611_200), // 2025-08-08
            side: TradeSide::Buy,
            portfolio: "Core".to_string(),
            qty: 14.0,
            price: 528.90,
            value: 7_404.60,
            result: TradeResult::Unrealized(1_691.76),
        },
    ]
}

pub fn recently_seen() -> Vec<Quote> {
    vec![
        Quote {
            symbol: "NVDA".to_string(),
            name: "NVIDIA".to_string(),
            price: 182.05,
            change: -0.008,
        },
        Quote {
            symbol: "BTC".to_string(),
            name: "Bitcoin".to_string(),
            price: 94_210.0,
            change: 0.034,
        },
        Quote {
            symbol: "VXUS".to_string(),
            name: "Vanguard Total Intl".to_string(),
            price: 71.86,
            change: 0.004,
        },
        Quote {
            symbol: "EUR".to_string(),
            name: "Euro".to_string(),
            price: 1.09,
            change: -0.001,
        },
    ]
}
