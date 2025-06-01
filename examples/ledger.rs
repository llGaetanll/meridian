use anyhow::anyhow;
use rand::Rng;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::AddAssign;
use std::ops::RangeBounds;
use std::str::FromStr;

// Number of seconds since unix epoch
type Timestamp = u64;
type Asset = String;
type Quantity = f64;

struct Position {
    asset: Asset,
    qty: Quantity,
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.asset, self.qty)
    }
}

impl FromStr for Position {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let [asset, qty] = s
            .split_whitespace()
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| anyhow!("Failed to split position into asset and qty"))?;

        let qty = qty.parse()?;

        Ok(Self {
            asset: String::from(asset),
            qty,
        })
    }
}

enum Tx {
    Trade {
        ts: Timestamp,
        buy: Position,
        sell: Position,
    },
    Transfer {
        ts: Timestamp,
        pos: Position,
    },
}

impl Display for Tx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tx::Trade { ts, buy, sell } => write!(f, "t {ts} {buy} {sell}"),
            Tx::Transfer { ts, pos } => write!(f, "T {ts} {pos}"),
        }
    }
}

impl FromStr for Tx {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut toks = s.split_whitespace();

        match toks.next() {
            Some("t") => {
                let ts = toks.next().ok_or(anyhow!("missing timestamp"))?.parse()?;
                let buy = toks
                    .next()
                    .ok_or(anyhow!("missing buy position"))?
                    .parse()?;
                let sell = toks
                    .next()
                    .ok_or(anyhow!("missing sell position"))?
                    .parse()?;

                Ok(Tx::Trade { ts, buy, sell })
            }
            Some("T") => {
                let ts = toks.next().ok_or(anyhow!("missing timestamp"))?.parse()?;
                let pos = toks.next().ok_or(anyhow!("missing position"))?.parse()?;

                Ok(Tx::Transfer { ts, pos })
            }
            _ => Err(anyhow!("unknown transaction type")),
        }
    }
}

struct Positions {
    positions: HashMap<Asset, Quantity>,
}

impl Positions {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    pub fn insert(&mut self, position: &Position) {
        let Position { asset, qty } = position;

        self.positions
            .entry(asset.clone())
            .and_modify(|q| q.add_assign(*qty))
            .or_insert(*qty);
    }
}

impl Display for Positions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (asset, qty) in &self.positions {
            writeln!(f, "{asset}: {qty}")?;
        }

        Ok(())
    }
}

struct Ledger {
    txs: Vec<Tx>,
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

impl Ledger {
    /// Compute the ledger's positions for any transaction in the given time range
    pub fn compute_positions<R>(&self, range: R) -> Positions
    where
        R: RangeBounds<Timestamp>,
    {
        let mut positions = Positions::new();

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

fn generate_random_ledger() -> Ledger {
    let mut rng = rand::rng();

    let assets = ["BTC", "ETH", "XRP", "ADA", "SOL"].map(String::from);

    let mut txs = vec![];

    for i in 0..100 {
        let ts = i;
        let tx = if rng.random::<f64>() < 0.7 {
            let buy_asset = assets[rng.random_range(0..assets.len())].clone();
            let sell_asset = assets[rng.random_range(0..assets.len())].clone();

            if buy_asset == sell_asset {
                continue;
            }

            let buy_qty = rng.random_range(0.01..10.0);
            let sell_qty = rng.random_range(0.01..10.0);

            Tx::Trade {
                ts,
                buy: Position {
                    asset: buy_asset,
                    qty: buy_qty,
                },
                sell: Position {
                    asset: sell_asset,
                    qty: -sell_qty,
                },
            }
        } else {
            let pos_asset = assets[rng.random_range(0..assets.len())].clone();
            let pos_qty = if rng.random::<f64>() < 0.5 {
                rng.random_range(0.01..100.0)
            } else {
                -rng.random_range(0.01..100.0)
            };

            Tx::Transfer {
                ts,
                pos: Position {
                    asset: pos_asset,
                    qty: pos_qty,
                },
            }
        };

        txs.push(tx);
    }

    Ledger { txs }
}

fn main() {
    let ledger = generate_random_ledger();

    println!("{}", ledger);

    let positions = ledger.compute_positions(..);

    println!("positions");
    println!("{positions}");
}
