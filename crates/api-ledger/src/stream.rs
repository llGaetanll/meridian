//! Recurring streams — a charge that comes back.
//!
//! Plaid's `/transactions/recurring/get` does the detection: it returns a
//! frequency, an average amount, a predicted next date and a status per
//! stream. That is a real reason to use it rather than write a classifier for
//! the MVP, and it is why this type is close to Plaid's answer.

use api_common::Price;
use api_common::Timestamp;

use crate::Category;

/// One recurring charge or deposit.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub id: String,

    /// What to call it: the merchant where Plaid matched one, the bank
    /// description otherwise.
    pub label: String,

    pub logo_url: Option<String>,

    pub category: Category,

    /// The typical amount, in ledger convention: negative is money out.
    pub amount: Price,

    /// How often, already worded: "Monthly", "Every 2 weeks".
    pub frequency: String,

    /// When Plaid expects the next one.
    pub next_date: Option<Timestamp>,

    /// Whether this stream is still running. An ended subscription still comes
    /// back from Plaid, and counting it would overstate the commitment.
    pub is_active: bool,
}

impl Stream {
    pub fn is_outflow(&self) -> bool {
        self.amount < 0.0
    }

    /// The amount normalised to a month, so streams of different frequencies
    /// can be added up into one commitment figure.
    ///
    /// The multipliers are the plain arithmetic ones: a weekly charge happens
    /// about 4.33 times a month, not 4. Rounding that to 4 understates a
    /// weekly subscription by 8% and the error compounds across every stream.
    pub fn monthly_amount(&self) -> Price {
        let per_month = match self.frequency.as_str() {
            "Weekly" => 52.0 / 12.0,
            "Every 2 weeks" => 26.0 / 12.0,
            "Twice a month" => 2.0,
            "Monthly" => 1.0,
            "Yearly" => 1.0 / 12.0,
            // An unknown cadence is counted once. Better a figure that is
            // right for the common case than one silently dropped.
            _ => 1.0,
        };

        self.amount * per_month
    }

    /// Plaid's `frequency`, worded.
    pub fn frequency_label(frequency: &str) -> String {
        match frequency {
            "WEEKLY" => "Weekly",
            "BIWEEKLY" => "Every 2 weeks",
            "SEMI_MONTHLY" => "Twice a month",
            "MONTHLY" => "Monthly",
            "ANNUALLY" => "Yearly",
            _ => "Irregular",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(amount: Price, frequency: &str) -> Stream {
        Stream {
            id: "s".into(),
            label: "Something".into(),
            logo_url: None,
            category: Category::FoodAndDrink,
            amount,
            frequency: frequency.into(),
            next_date: None,
            is_active: true,
        }
    }

    #[test]
    fn monthly_streams_pass_through() {
        assert_eq!(stream(-500.0, "Monthly").monthly_amount(), -500.0);
    }

    /// The 4-vs-4.33 error, guarded: a weekly charge is not four a month.
    #[test]
    fn weekly_streams_are_normalised_by_the_real_multiplier() {
        let monthly = stream(-10.0, "Weekly").monthly_amount();

        assert!((monthly - -43.333).abs() < 0.01, "got {monthly}");
    }

    #[test]
    fn a_yearly_stream_is_a_twelfth() {
        assert!((stream(-120.0, "Yearly").monthly_amount() - -10.0).abs() < 1e-9);
    }

    #[test]
    fn direction_follows_the_sign() {
        assert!(stream(-500.0, "Monthly").is_outflow());
        assert!(!stream(4.22, "Monthly").is_outflow());
    }
}
