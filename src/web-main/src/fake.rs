//! Stand-in data until the ledger and the price sources are wired up.

use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;
use web_components::AssetPosition;
use web_components::AssetTrade;
use web_components::PortfolioHolding;
use web_components::Quote;
use web_components::TradeResult;
use web_components::TradeSide;

/// 2026-08-03 09:30:00 UTC — the open of the session we are showing.
const SESSION_OPEN: Timestamp = 1_785_749_400;

const MINUTE: Timestamp = 60;
const WEEK: Timestamp = 7 * 86_400;

/// 2014-08-04 00:00:00 UTC — where the range selector's history starts.
const HISTORY_START: Timestamp = 1_407_110_400;

pub const SYMBOL: &str = "SPY";

/// Deterministic pseudo-random walk, so the charts look the same on every reload.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Uniform in 0.0..1.0
    fn next_unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in -1.0..1.0
    fn next_signed(&mut self) -> f64 {
        self.next_unit() * 2.0 - 1.0
    }
}

fn random_walk(
    seed: u64,
    start_ts: Timestamp,
    step: Timestamp,
    points: usize,
    start_px: Price,
    drift: Price,
    volatility: Price,
) -> ChartData {
    let mut rng = Lcg::new(seed);
    let mut px = start_px;

    (0..points)
        .map(|i| {
            px *= 1.0 + drift + rng.next_signed() * volatility;
            (start_ts + i as Timestamp * step, px)
        })
        .collect()
}

/// One trading session, minute by minute.
pub fn price_series() -> ChartData {
    random_walk(
        0x5EED_1234,
        SESSION_OPEN,
        MINUTE,
        390,
        639.99,
        0.00006,
        0.0009,
    )
}

/// Volume for the same session. Heaviest at the open and into the close.
pub fn volume_series() -> ChartData {
    let mut rng = Lcg::new(0xB0B0_CAFE);

    (0..390)
        .map(|i| {
            let t = i as f64 / 389.0;
            // U shaped session profile.
            let shape = 0.35 + 2.2 * (t - 0.5).powi(2) + if t > 0.97 { 1.4 } else { 0.0 };
            let volume = shape * (0.55 + rng.next_unit() * 0.9) * 900_000.0;

            (SESSION_OPEN + i as Timestamp * MINUTE, volume)
        })
        .collect()
}

/// Twelve years of weekly closes, for the range selector.
pub fn history_series() -> ChartData {
    random_walk(0xC0FF_EE42, HISTORY_START, WEEK, 626, 190.0, 0.0021, 0.018)
}

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
            ts: 1_781_222_400, // 2026-06-12
            side: TradeSide::Buy,
            portfolio: "Play money".to_string(),
            qty: 10.0,
            price: 612.40,
            value: 6_124.00,
            result: TradeResult::Unrealized(373.40),
        },
        AssetTrade {
            ts: 1_772_668_800, // 2026-03-05
            side: TradeSide::Buy,
            portfolio: "Retirement".to_string(),
            qty: 8.0,
            price: 588.15,
            value: 4_705.20,
            result: TradeResult::Unrealized(492.72),
        },
        AssetTrade {
            ts: 1_769_040_000, // 2026-01-22
            side: TradeSide::Sell,
            portfolio: "Core".to_string(),
            qty: 6.0,
            price: 601.30,
            value: 3_607.80,
            result: TradeResult::Realized(233.46),
        },
        AssetTrade {
            ts: 1_763_078_400, // 2025-11-14
            side: TradeSide::Buy,
            portfolio: "Core".to_string(),
            qty: 12.0,
            price: 542.60,
            value: 6_511.20,
            result: TradeResult::Unrealized(1_285.68),
        },
        AssetTrade {
            ts: 1_754_611_200, // 2025-08-08
            side: TradeSide::Buy,
            portfolio: "Core".to_string(),
            qty: 14.0,
            price: 528.90,
            value: 7_404.60,
            result: TradeResult::Unrealized(1_691.76),
        },
    ]
}

pub fn key_stats() -> Vec<(String, String)> {
    [
        ("Open", "$639.99"),
        ("Prev close", "$638.42"),
        ("Day range", "$638.69 – $652.53"),
        ("52w range", "$521.40 – $652.53"),
        ("Volume", "71.4M"),
        ("Avg volume", "58.9M"),
        ("Net assets", "$612.8B"),
        ("Expense ratio", "0.09%"),
        ("Div. yield", "1.21%"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
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
