//! Where a subscription stands, as far as anything decides about it.

/// What Stripe says a subscription is.
///
/// A value rather than a string, so a rule about one of them is a `match` and
/// not a comparison against a spelling. Unlike [`PlaidFault`], which names two
/// codes out of hundreds, the whole published set is named here: it is eight
/// words, four of them already have rules, and the app's answer to "why can
/// this person not get in" is built out of the rest.
///
/// [`Other`](StripeStatus::Other) is still there because the set is Stripe's
/// and they extend it without asking. It carries the word that arrived, which
/// is what the row stores and what the log line says.
///
/// [`PlaidFault`]: ../src_plaid/enum.PlaidFault.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripeStatus {
    /// Paid and running.
    Active,

    /// Inside a trial that has not been billed yet.
    Trialing,

    /// A first payment that has not landed. The subscription exists and has
    /// bought nothing yet.
    Incomplete,

    /// That first payment never landed within Stripe's 23 hours. Terminal,
    /// and it never bought anything.
    IncompleteExpired,

    /// A renewal failed and Stripe is still retrying. The period already paid
    /// for is still running.
    PastDue,

    /// Stripe has given up retrying. It keeps issuing invoices nobody pays,
    /// so the period rolls forward on a subscription earning nothing.
    Unpaid,

    /// Stopped. Whether access stops with it is the period end's to say.
    Canceled,

    /// Collection is suspended, with the subscription still in place.
    Paused,

    /// A word Stripe has added since, kept as it arrived.
    Other(String),
}

impl StripeStatus {
    /// The status one of Stripe's names means.
    pub fn from_name(name: &str) -> Self {
        match name {
            "active" => StripeStatus::Active,
            "trialing" => StripeStatus::Trialing,
            "incomplete" => StripeStatus::Incomplete,
            "incomplete_expired" => StripeStatus::IncompleteExpired,
            "past_due" => StripeStatus::PastDue,
            "unpaid" => StripeStatus::Unpaid,
            "canceled" => StripeStatus::Canceled,
            "paused" => StripeStatus::Paused,
            other => StripeStatus::Other(other.to_string()),
        }
    }

    /// Stripe's name for this status, for the row it is stored in and for a
    /// log line.
    pub fn as_str(&self) -> &str {
        match self {
            StripeStatus::Active => "active",
            StripeStatus::Trialing => "trialing",
            StripeStatus::Incomplete => "incomplete",
            StripeStatus::IncompleteExpired => "incomplete_expired",
            StripeStatus::PastDue => "past_due",
            StripeStatus::Unpaid => "unpaid",
            StripeStatus::Canceled => "canceled",
            StripeStatus::Paused => "paused",
            StripeStatus::Other(name) => name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names are Stripe's and the rows are written with them, so a
    /// variant renamed on one side and not the other stores a word Stripe
    /// never said.
    #[test]
    fn every_status_round_trips_through_its_name() {
        let all = [
            StripeStatus::Active,
            StripeStatus::Trialing,
            StripeStatus::Incomplete,
            StripeStatus::IncompleteExpired,
            StripeStatus::PastDue,
            StripeStatus::Unpaid,
            StripeStatus::Canceled,
            StripeStatus::Paused,
        ];

        for status in all {
            assert_eq!(StripeStatus::from_name(status.as_str()), status);
        }
    }

    /// `incomplete` and `incomplete_expired` differ by a suffix and decide
    /// opposite things: one is a payment still in flight, the other is one
    /// that never landed.
    #[test]
    fn the_two_incompletes_stay_apart() {
        assert_eq!(
            StripeStatus::from_name("incomplete"),
            StripeStatus::Incomplete,
        );
        assert_eq!(
            StripeStatus::from_name("incomplete_expired"),
            StripeStatus::IncompleteExpired,
        );
    }

    /// A word nobody has a rule about survives whole: it is what the row is
    /// written with and what the log line says.
    #[test]
    fn anything_else_keeps_its_word() {
        let status = StripeStatus::from_name("in_arbitration");

        assert_eq!(status, StripeStatus::Other("in_arbitration".into()));
        assert_eq!(status.as_str(), "in_arbitration");
    }
}
