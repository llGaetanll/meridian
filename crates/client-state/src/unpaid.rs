//! Why somebody who is signed in is not being served.

/// What the server said about a subscription it is not honouring.
///
/// Our three words rather than the payment provider's. Stripe says `past_due`
/// and Apple says something else again, so a client matching on either
/// vocabulary is a client that has learned one provider's — and a second
/// provider is then a second set of arms in every app.
///
/// There is no "paid" arm. This is carried by a refusal, and a session being
/// served never produces one.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum Unpaid {
    /// Nobody has ever subscribed on this account.
    Never,

    /// A payment did not go through. The date is when the period it had paid
    /// for ended, where there was one — a first payment that never landed
    /// bought no period at all.
    Declined {
        /// When the period that was paid for ended, in seconds. Absent where
        /// the server has no date for it.
        ran_out: Option<u64>,
    },

    /// A subscription that was ended, and the period it paid for has passed.
    Ended { ran_out: Option<u64> },

    /// The server cannot tell. A payment still in flight, or a provider that
    /// believes this person is paying while the date says otherwise.
    ///
    /// Its own arm rather than folded into one of the three above, because
    /// each of those names a cause and this is the case where none of them is
    /// known to be true. Somebody mid-payment told their subscription has
    /// ended is being lied to.
    Unconfirmed,
}
