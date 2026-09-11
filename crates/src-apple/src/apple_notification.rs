//! What Apple says happened to a subscription nobody was watching.
//!
//! The other half of [`AppleTransaction`](crate::AppleTransaction): the same
//! JWS format, arriving unprompted, because a renewal, a cancellation and a
//! refund all happen while the app is shut.

use serde::Deserialize;

/// One App Store Server Notification, as Apple signs it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleNotification {
    /// `DID_RENEW`, `EXPIRED`, `REFUND`, and a dozen more. Carried for the log
    /// line: what a notification does to an entitlement is decided from the
    /// transaction inside it, not from the name on the envelope.
    pub notification_type: String,

    /// What kind of that type — `BILLING_RECOVERY`, `VOLUNTARY`. Absent on
    /// most.
    #[serde(default)]
    pub subtype: Option<String>,

    /// Apple retries until it is answered 2xx, so this is what tells a repeat
    /// from a new fact.
    ///
    /// Named outright because it is the one field Apple does not spell in
    /// camel case: `notificationUUID`, where the rule would say
    /// `notificationUuid`.
    #[serde(rename = "notificationUUID")]
    pub notification_uuid: String,

    /// Absent on the notifications that carry a summary instead — a renewal
    /// date extended across many subscriptions at once, which names nobody.
    #[serde(default)]
    pub data: Option<AppleNotificationData>,
}

/// The subscription a notification is about.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleNotificationData {
    /// The app this happened in.
    pub bundle_id: String,

    /// Which store, as [`AppleEnvironment`](crate::AppleEnvironment) reads it.
    /// A notification from the sandbox naming a production subscription is the
    /// thing this field exists to catch.
    pub environment: String,

    /// Where the subscription stands, as
    /// [`AppleStatus`](crate::AppleStatus) numbers it.
    #[serde(default)]
    pub status: Option<i64>,

    /// The transaction, signed again inside this one.
    ///
    /// Verified in its own right and by the same [`verify`](crate::verify): a
    /// notification vouches for nothing but its own bytes, and this is another
    /// JWS rather than a field of this one.
    #[serde(default)]
    pub signed_transaction_info: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A notification as Apple sends one, with its two signed payloads cut
    /// down to something that is plainly not a JWS.
    const DID_RENEW: &str = r#"{
        "notificationType": "DID_RENEW",
        "notificationUUID": "d1f2b8a0-0000-4000-8000-000000000001",
        "version": "2.0",
        "signedDate": 1787097601000,
        "data": {
            "appAppleId": 6810780162,
            "bundleId": "com.meridian.phone",
            "bundleVersion": "1",
            "environment": "Sandbox",
            "status": 1,
            "signedTransactionInfo": "the.transaction.jws",
            "signedRenewalInfo": "the.renewal.jws"
        }
    }"#;

    #[test]
    fn a_renewal_names_the_transaction_that_proves_it() {
        let notification: AppleNotification = serde_json::from_str(DID_RENEW).unwrap();

        assert_eq!(notification.notification_type, "DID_RENEW");
        assert!(notification.subtype.is_none());

        let data = notification.data.unwrap();

        assert_eq!(data.environment, "Sandbox");
        assert_eq!(data.status, Some(1));
        assert_eq!(
            data.signed_transaction_info.as_deref(),
            Some("the.transaction.jws")
        );
    }

    /// The notifications that name nobody still parse. Refusing one is
    /// answering Apple with an error, and an error is a delivery that comes
    /// back every hour until it is not.
    #[test]
    fn a_notification_carrying_no_subscription_still_parses() {
        let summary = r#"{
            "notificationType": "RENEWAL_EXTENSION",
            "subtype": "SUMMARY",
            "notificationUUID": "d1f2b8a0-0000-4000-8000-000000000002",
            "version": "2.0",
            "signedDate": 1787097601000,
            "summary": { "environment": "Sandbox", "appAppleId": 6810780162 }
        }"#;

        let notification: AppleNotification = serde_json::from_str(summary).unwrap();

        assert_eq!(notification.subtype.as_deref(), Some("SUMMARY"));
        assert!(notification.data.is_none());
    }
}
