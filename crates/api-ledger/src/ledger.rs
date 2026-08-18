use std::fmt::Display;
use std::ops::RangeBounds;
use std::str::FromStr;

use api_common::Timestamp;

use crate::Positions;
use crate::Tx;

struct Ledger {
    txs: Vec<Tx>,
}

impl Ledger {
    /// Compute the ledger's positions for any transaction in the given time range
    pub fn compute_positions<R>(&self, range: R) -> Positions
    where
        R: RangeBounds<Timestamp>,
    {
        let mut positions = Positions::default();

        for tx in &self.txs {
            match tx {
                Tx::Trade { ts, buy, sell } => {
                    if range.contains(ts) {
                        positions.insert(buy);
                        positions.insert(sell);
                    }
                }
                Tx::Transfer { ts, pos } => {
                    if range.contains(ts) {
                        positions.insert(pos);
                    }
                }
            }
        }

        positions
    }
}

impl FromStr for Ledger {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let txs = s
            .lines()
            .filter(|l| l.starts_with('#') || l.trim().is_empty())
            .map(|l| l.parse())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { txs })
    }
}

impl Display for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for tx in &self.txs {
            writeln!(f, "{tx}")?;
        }

        Ok(())
    }
}

// fn generate_random_ledger() -> Ledger {
//     let mut rng = rand::rng();
//
//     let assets = ["BTC", "ETH", "XRP", "ADA", "SOL"].map(String::from);
//
//     let mut txs = vec![];
//
//     for i in 0..100 {
//         let ts = i;
//         let tx = if rng.random::<f64>() < 0.7 {
//             let buy_asset = assets[rng.random_range(0..assets.len())].clone();
//             let sell_asset = assets[rng.random_range(0..assets.len())].clone();
//
//             if buy_asset == sell_asset {
//                 continue;
//             }
//
//             let buy_qty = rng.random_range(0.01..10.0);
//             let sell_qty = rng.random_range(0.01..10.0);
//
//             Tx::Trade {
//                 ts,
//                 buy: Position {
//                     asset: buy_asset,
//                     qty: buy_qty,
//                 },
//                 sell: Position {
//                     asset: sell_asset,
//                     qty: -sell_qty,
//                 },
//             }
//         } else {
//             let pos_asset = assets[rng.random_range(0..assets.len())].clone();
//             let pos_qty = if rng.random::<f64>() < 0.5 {
//                 rng.random_range(0.01..100.0)
//             } else {
//                 -rng.random_range(0.01..100.0)
//             };
//
//             Tx::Transfer {
//                 ts,
//                 pos: Position {
//                     asset: pos_asset,
//                     qty: pos_qty,
//                 },
//             }
//         };
//
//         txs.push(tx);
//     }
//
//     Ledger { txs }
// }
