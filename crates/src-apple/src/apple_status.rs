//! Where a subscription stands, as Apple numbers it.

/// What Apple says a subscription is, as a value rather than the integer it
/// arrives as.
///
/// The five codes Apple publishes, each named, so a rule about one of them is
/// a `match` and not a comparison against a number nobody can read. The words
/// are ours: Apple sends `1`, and `1` in a row six months from now is a fact
/// somebody has to go and look up.
///
/// [`Other`](AppleStatus::Other) keeps a code Apple has added since, so a row
/// is still written and a log line still says what arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppleStatus {
    /// Paid and running.
    Active,

    /// The period ran out and nothing renewed it.
    Expired,

    /// A renewal failed and Apple is still retrying. The period already paid
    /// for may still be running.
    BillingRetry,

    /// A renewal failed and Apple is serving the app anyway while it retries.
    GracePeriod,

    /// Apple took the purchase back — a refund, or a family member removed.
    /// Access stops now rather than at the end of the period.
    Revoked,

    /// A code Apple has added since, kept as the word it is stored as.
    Other(String),
}

impl AppleStatus {
    /// The status one of Apple's codes means.
    pub fn from_code(code: i64) -> Self {
        match code {
            1 => AppleStatus::Active,
            2 => AppleStatus::Expired,
            3 => AppleStatus::BillingRetry,
            4 => AppleStatus::GracePeriod,
            5 => AppleStatus::Revoked,
            other => AppleStatus::Other(other.to_string()),
        }
    }

    /// The word this is stored and logged as.
    ///
    /// One column holds both rails' statuses, so these sit beside Stripe's
    /// eight and must not collide with them: `expired` is Apple's and
    /// `canceled` is Stripe's, and neither provider's reader is ever handed
    /// the other's row.
    pub fn as_str(&self) -> &str {
        match self {
            AppleStatus::Active => "active",
            AppleStatus::Expired => "expired",
            AppleStatus::BillingRetry => "billing_retry",
            AppleStatus::GracePeriod => "grace_period",
            AppleStatus::Revoked => "revoked",
            AppleStatus::Other(code) => code,
        }
    }

    /// The status a stored word means, which is the way back from a row.
    pub fn from_name(name: &str) -> Self {
        match name {
            "active" => AppleStatus::Active,
            "expired" => AppleStatus::Expired,
            "billing_retry" => AppleStatus::BillingRetry,
            "grace_period" => AppleStatus::GracePeriod,
            "revoked" => AppleStatus::Revoked,
            other => AppleStatus::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows are written with these words and read back with them, so a
    /// variant renamed on one side and not the other loses what a row said.
    #[test]
    fn every_status_round_trips_through_its_word() {
        let all = [
            AppleStatus::Active,
            AppleStatus::Expired,
            AppleStatus::BillingRetry,
            AppleStatus::GracePeriod,
            AppleStatus::Revoked,
        ];

        for status in all {
            assert_eq!(AppleStatus::from_name(status.as_str()), status);
        }
    }

    #[test]
    fn the_five_codes_apple_publishes_are_named() {
        assert_eq!(AppleStatus::from_code(1), AppleStatus::Active);
        assert_eq!(AppleStatus::from_code(2), AppleStatus::Expired);
        assert_eq!(AppleStatus::from_code(3), AppleStatus::BillingRetry);
        assert_eq!(AppleStatus::from_code(4), AppleStatus::GracePeriod);
        assert_eq!(AppleStatus::from_code(5), AppleStatus::Revoked);
    }

    /// A code nobody has a rule about survives whole, both ways.
    #[test]
    fn anything_else_keeps_its_code() {
        let status = AppleStatus::from_code(9);

        assert_eq!(status, AppleStatus::Other("9".to_string()));
        assert_eq!(status.as_str(), "9");
        assert_eq!(AppleStatus::from_name("9"), status);
    }
}
