//! What Apple says was bought.
//!
//! Parsed only after [`verify`](crate::verify) has said the bytes are Apple's.
//! Nothing here decides anything: what a transaction means for somebody's
//! entitlement is the state machine's business.

use serde::Deserialize;

/// One purchase or renewal, as Apple signs it.
///
/// Trimmed to what something reads. Apple's payload carries twenty-odd fields
/// — the storefront, the price, the offer it came from — and every one kept
/// here is one more that has to stay right.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleTransaction {
    /// The first transaction in the chain of renewals, and the only id that
    /// survives one. **This is the customer.** It is what the row is keyed by
    /// and how a renewal a year from now finds the person who bought this.
    pub original_transaction_id: String,

    /// This purchase or renewal in particular.
    pub transaction_id: String,

    /// What was bought. This app sells one thing, and a payload naming
    /// anything else was not meant for it.
    pub product_id: String,

    /// The app it was bought in.
    pub bundle_id: String,

    /// `Sandbox` or `Production`, as
    /// [`AppleEnvironment`](crate::AppleEnvironment) reads it. The word rather
    /// than the value: which store a build will act on is a rule, and rules
    /// live in the machine.
    pub environment: String,

    /// When the period paid for runs out, in **milliseconds**. Absent on
    /// something that does not renew, which this app does not sell.
    #[serde(default)]
    pub expires_date: Option<i64>,

    /// When Apple took the purchase back, in milliseconds — a refund, or a
    /// family member removed. Absent is the ordinary case.
    #[serde(default)]
    pub revocation_date: Option<i64>,

    /// When Apple signed this, in milliseconds.
    pub signed_date: i64,
}

impl AppleTransaction {
    /// The end of what has been paid for, in seconds.
    ///
    /// A revocation ends it there and not at the period end, because the money
    /// has gone back. `None` is a transaction that bought no period at all.
    pub fn good_until(&self) -> Option<i64> {
        match self.revocation_date {
            Some(revoked) => Some(revoked / 1000),
            None => self.expires_date.map(|expires| expires / 1000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transaction as Apple sends one, with the fields nothing reads left
    /// out.
    const RENEWAL: &str = r#"{
        "transactionId": "2000000000000002",
        "originalTransactionId": "2000000000000001",
        "bundleId": "com.meridian.phone",
        "productId": "com.meridian.phone.monthly",
        "purchaseDate": 1787097600000,
        "expiresDate": 1789776000000,
        "type": "Auto-Renewable Subscription",
        "inAppOwnershipType": "PURCHASED",
        "signedDate": 1787097601000,
        "environment": "Sandbox"
    }"#;

    #[test]
    fn a_renewal_is_good_until_its_period_ends() {
        let transaction: AppleTransaction = serde_json::from_str(RENEWAL).unwrap();

        assert_eq!(transaction.original_transaction_id, "2000000000000001");
        assert_eq!(transaction.environment, "Sandbox");
        assert_eq!(transaction.good_until(), Some(1_789_776_000));
    }

    /// Milliseconds are Apple's and seconds are everything else's. Getting
    /// this backwards entitles somebody until the year 58,660.
    #[test]
    fn the_date_arrives_in_milliseconds_and_leaves_in_seconds() {
        let transaction: AppleTransaction = serde_json::from_str(RENEWAL).unwrap();

        assert_eq!(transaction.expires_date, Some(1_789_776_000_000));
        assert_eq!(transaction.good_until(), Some(1_789_776_000));
    }

    /// A refund stops access when the money went back, not when the period
    /// would have ended.
    #[test]
    fn a_revoked_transaction_is_good_until_it_was_revoked() {
        let refunded = RENEWAL.replace(
            r#""environment": "Sandbox""#,
            r#""environment": "Sandbox", "revocationDate": 1787184000000, "revocationReason": 1"#,
        );

        let transaction: AppleTransaction = serde_json::from_str(&refunded).unwrap();

        assert_eq!(transaction.good_until(), Some(1_787_184_000));
    }
}
