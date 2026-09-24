//! One line of the market universe: a symbol, and how much attention it earns.

/// A symbol the machine may fetch on its own initiative, and its share of
/// the idle budget.
///
/// The weight is a prior, not a reading: coarse relative trading volume,
/// good enough to say SPY before a small-cap, and stale rankings cost only
/// a suboptimal fetch order. Scale is free — only ratios matter.
#[derive(Debug, Clone)]
pub struct MarketWeight {
    pub symbol: String,
    pub weight: f64,
}

/// The universe file, parsed: one `SYMBOL WEIGHT` per line, `#` comments and
/// blank lines skipped.
///
/// All or nothing. A file that half-parses is a distribution silently
/// missing the symbols that failed, and the error names the line instead.
pub fn parse_universe(text: &str) -> Result<Vec<MarketWeight>, String> {
    let mut universe = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();

        let (Some(symbol), Some(weight), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!(
                "line {}: expected `SYMBOL WEIGHT`, got {line:?}",
                index + 1,
            ));
        };

        let weight: f64 = weight
            .parse()
            .map_err(|_| format!("line {}: {weight:?} is not a number", index + 1))?;

        if !weight.is_finite() || weight <= 0.0 {
            return Err(format!(
                "line {}: a weight must be finite and positive, got {weight}",
                index + 1,
            ));
        }

        universe.push(MarketWeight {
            symbol: symbol.to_string(),
            weight,
        });
    }

    Ok(universe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_universe_parses_line_by_line() {
        let universe = parse_universe("# the majors\nSPY 100\n\nAAPL 40.5\n").expect("it parses");

        assert_eq!(universe.len(), 2);
        assert_eq!(universe[0].symbol, "SPY");
        assert_eq!(universe[0].weight, 100.0);
        assert_eq!(universe[1].symbol, "AAPL");
        assert_eq!(universe[1].weight, 40.5);
    }

    #[test]
    fn a_bad_line_refuses_the_whole_file() {
        assert!(parse_universe("SPY 100\nAAPL\n").is_err());
        assert!(parse_universe("SPY one-hundred\n").is_err());
        assert!(parse_universe("SPY -3\n").is_err());
        assert!(parse_universe("SPY 100 extra\n").is_err());
    }
}
