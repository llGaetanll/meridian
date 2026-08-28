//! Money in, money out.
//!
//! The app computes in `f64` and the database stores whole minor units. This
//! is the only place that crosses between them, so the rounding rule is stated
//! once rather than guessed at each call site.

use api_common::Price;

/// Minor units in one major unit.
///
/// A constant rather than a per-currency lookup, because everything the app
/// holds is USD. A non-USD account needs the currency's own exponent — JPY has
/// none and KWD has three — and that is a column on `accounts` and a change
/// here, not something to fake now.
const MINOR: f64 = 100.0;

/// A price as whole minor units.
///
/// Rounds rather than truncates: a bank amount is already a whole number of
/// cents, and the float that carries it is off by a fraction of one at most.
/// Truncating would turn `4.33` stored as `4.329999…` into `432`, losing a
/// cent per transaction in a way that only shows up in a total.
///
/// A value too large for an `i64` saturates rather than wrapping — Rust's `as`
/// has done that since 1.45. There is no bank balance anywhere near it, and a
/// saturated number is a visibly wrong one rather than a silently negative one.
pub fn to_minor(amount: Price) -> i64 {
    (amount * MINOR).round() as i64
}

/// And back.
pub fn from_minor(minor: i64) -> Price {
    minor as Price / MINOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_survives_the_round_trip() {
        for amount in [0.0, 4.33, -25.0, 86_541.74, -163_705.89, 0.01, -0.01] {
            assert_eq!(from_minor(to_minor(amount)), amount, "{amount}");
        }
    }

    /// The case truncation gets wrong. `4.33` is not exactly representable, so
    /// the product is a hair under 433.
    #[test]
    fn a_hair_under_a_cent_rounds_to_the_cent() {
        assert_eq!(to_minor(4.33), 433);
        assert_eq!(to_minor(-4.33), -433);
        assert_eq!(to_minor(0.1 + 0.2), 30);
    }

    /// Adding minor units is exact where adding the floats is not, which is
    /// the whole reason for storing them this way.
    #[test]
    fn minor_units_add_up_exactly() {
        let cents: i64 = [0.1, 0.2, 0.3].iter().map(|a| to_minor(*a)).sum();

        assert_eq!(cents, 60);
        assert_eq!(from_minor(cents), 0.6);
    }
}
