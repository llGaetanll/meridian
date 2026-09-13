//! What Stripe has been asked for on one user's behalf.

/// The customer everything hangs off, and the subscription against it once
/// there is one.
///
/// One of these per user, which is what makes "one subscription per person" a
/// shape rather than a rule somebody has to remember. Pressing Subscribe
/// again replaces what is here; it does not add to it.
///
/// `subscription_id` is `None` between the two calls that start a
/// subscription, and stays `None` for a user whose customer was created and
/// whose subscription Stripe then refused.
#[derive(Debug, Clone)]
pub struct StripePayer {
    /// `cus_…`, as Stripe named them.
    pub customer_id: String,

    /// `sub_…`, once there is one.
    pub subscription_id: Option<String>,
}
