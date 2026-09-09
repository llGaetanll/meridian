//! What `POST /v1/customers` answers with.

use serde::Deserialize;

/// One customer, cut down to the field anything here uses.
#[derive(Debug, Clone, Deserialize)]
pub struct StripeCustomer {
    /// `cus_…`. What every later call about this person is keyed by.
    pub id: String,
}
