//! Stand-in transactions, until the ledger is loaded from somewhere real.
//!
//! These are `api_ledger::Tx` values rather than a view-model of their own, so
//! the list is already rendering the type the app will hand it later.

use api_ledger::Position;
use api_ledger::Tx;

pub fn txs() -> Vec<Tx> {
    vec![
        Tx::Trade {
            ts: 1_781_222_400, // 2026-06-12
            buy: pos("SPY", 10.0),
            sell: pos("USD", -6_124.00),
        },
        Tx::Transfer {
            ts: 1_780_704_000, // 2026-06-06
            pos: pos("USD", 4_000.00),
        },
        Tx::Trade {
            ts: 1_779_408_000, // 2026-05-22
            buy: pos("BTC", 0.05),
            sell: pos("USD", -4_710.50),
        },
        Tx::Transfer {
            ts: 1_778_803_200, // 2026-05-15
            pos: pos("USD", -82.40),
        },
        Tx::Trade {
            ts: 1_772_668_800, // 2026-03-05
            buy: pos("AAPL", 8.0),
            sell: pos("USD", -4_705.20),
        },
        Tx::Transfer {
            ts: 1_769_040_000, // 2026-01-22
            pos: pos("USD", -1_240.00),
        },
    ]
}

fn pos(asset: &str, qty: f64) -> Position {
    Position {
        asset: asset.to_string(),
        qty,
    }
}
