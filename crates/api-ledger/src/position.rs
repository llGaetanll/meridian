use anyhow::anyhow;
use api_common::Asset;
use api_common::Quantity;

use std::collections::HashMap;
use std::fmt::Display;
use std::ops::AddAssign;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub asset: Asset,
    pub qty: Quantity,
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

#[derive(Default)]
pub struct Positions {
    positions: HashMap<Asset, Quantity>,
}

impl Positions {
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
