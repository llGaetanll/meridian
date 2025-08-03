use std::fmt::Display;
use std::str::FromStr;

use anyhow::anyhow;
use api_common::Timestamp;

use crate::Position;

#[derive(Debug, Clone)]
pub enum Tx {
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
