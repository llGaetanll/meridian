use serde_json::Value;

use crate::PlaidRequest;

/// `/link/token/create` — the token that opens Link, and the URL that hosts it.
///
/// One request type for the two things Link is ever asked to do: attach a bank
/// that is not attached yet, and repair one whose consent has lapsed. They are
/// the same call with one difference, and it is a difference Plaid rejects if
/// you get it wrong — an update carries the `access_token` of the Item being
/// repaired and **must not** name any products, because that Item already has
/// them. Hence two constructors and no way to build the wrong shape by hand.
///
/// Everything is Hosted Link: the bank sign-in happens on `secure.plaid.com`
/// in a session we open and do not draw, which is why nothing
/// platform-specific has to be integrated to connect a bank. See
/// `docs/PLAID.md`.
pub struct LinkTokenRequest {
    /// What Link calls us on its own page. Plaid caps it at 30 characters.
    pub client_name: String,

    pub language: String,

    pub country_codes: Vec<String>,

    /// Our own id for the person linking, which Plaid echoes back in webhooks.
    /// Stable per user and never an email — it ends up in Plaid's logs.
    pub client_user_id: String,

    /// Empty in update mode, and only in update mode.
    pub products: Vec<String>,

    /// Where Plaid should send `SESSION_FINISHED` and everything after it.
    /// Absent means we poll [`LinkSessionRequest`](crate::LinkSessionRequest)
    /// instead, which is the same information arriving later.
    pub webhook: Option<String>,

    /// The Item being repaired. Set only in update mode.
    pub access_token: Option<String>,

    /// Where Plaid sends the browser when the flow ends.
    ///
    /// On a phone this is a custom scheme whose only job is to close the web
    /// session — `meridian://linked`. **It carries no token**: the redirect
    /// says the sheet may close, and the `public_token` arrives by webhook or
    /// by polling. Those are two channels and they are independent.
    pub completion_redirect_uri: Option<String>,

    /// Whether the session is being opened inside a native app.
    ///
    /// **Setting this requires a top-level `redirect_uri` as well**, which
    /// Plaid rejects the request without — and that one must be an https
    /// address registered in the dashboard, unlike the custom scheme in
    /// `completion_redirect_uri`. It changes how an OAuth bank hands control
    /// back, so it is only needed once a real bank is linked from a phone.
    pub is_mobile_app: bool,

    /// How long the hosted URL stays good for. Plaid's default is 30 minutes
    /// when it is not being delivered by SMS or email, which is the case here.
    pub url_lifetime_seconds: Option<u32>,
}

/// The only product the app uses. Balances and recurring both come off the
/// Item that `transactions` creates, so naming them again would only widen
/// what the user is asked to consent to.
pub const TRANSACTIONS: &str = "transactions";

impl LinkTokenRequest {
    /// Attach a bank that is not attached yet.
    pub fn hosted(client_user_id: impl Into<String>) -> Self {
        Self {
            client_name: String::from("Meridian"),
            language: String::from("en"),
            country_codes: vec![String::from("US")],
            client_user_id: client_user_id.into(),
            products: vec![String::from(TRANSACTIONS)],
            webhook: None,
            access_token: None,
            completion_redirect_uri: None,
            is_mobile_app: false,
            url_lifetime_seconds: None,
        }
    }

    /// Repair an Item whose consent has lapsed — Plaid's update mode.
    ///
    /// The access token stays the same one afterwards; what changes is that
    /// the bank starts answering for it again. Nothing is re-fetched and no
    /// new Item is created, which is why this does not spend a slot.
    pub fn update(client_user_id: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            // No products. Plaid errors on an update that names any, because
            // the Item being repaired already has the ones it was created with.
            products: Vec::new(),
            access_token: Some(access_token.into()),
            ..Self::hosted(client_user_id)
        }
    }

    /// Where to send the browser when the flow ends, and whether a native app
    /// is what opened it.
    pub fn redirecting_to(mut self, uri: impl Into<String>, mobile: bool) -> Self {
        self.completion_redirect_uri = Some(uri.into());
        self.is_mobile_app = mobile;
        self
    }

    /// Where Plaid should tell us the session finished.
    pub fn notifying(mut self, webhook: impl Into<String>) -> Self {
        self.webhook = Some(webhook.into());
        self
    }
}

impl PlaidRequest for LinkTokenRequest {
    fn path(&self) -> String {
        String::from("link/token/create")
    }

    fn body(&self) -> Value {
        let mut hosted = serde_json::Map::new();

        if let Some(uri) = &self.completion_redirect_uri {
            hosted.insert("completion_redirect_uri".into(), Value::String(uri.clone()));
        }

        if self.is_mobile_app {
            hosted.insert("is_mobile_app".into(), Value::Bool(true));
        }

        if let Some(seconds) = self.url_lifetime_seconds {
            hosted.insert("url_lifetime_seconds".into(), Value::from(seconds));
        }

        // An empty object is how Hosted Link is asked for with no options, and
        // is what the docs specify — absent would give us the SDK flow, which
        // is the one that needs a universal link before OAuth banks work.
        let mut body = serde_json::json!({
            "client_name": self.client_name,
            "language": self.language,
            "country_codes": self.country_codes,
            "user": { "client_user_id": self.client_user_id },
            "hosted_link": Value::Object(hosted),
        });

        // Absent rather than empty for both of these: an update names no
        // products, and a create carries no access token, and Plaid reads the
        // presence of each as which one it is being asked for.
        if !self.products.is_empty() {
            body["products"] = Value::from(self.products.clone());
        }

        if let Some(token) = &self.access_token {
            body["access_token"] = Value::String(token.clone());
        }

        if let Some(webhook) = &self.webhook {
            body["webhook"] = Value::String(webhook.clone());
        }

        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_link_names_its_products_and_no_token() {
        let body = LinkTokenRequest::hosted("user-1").body();

        assert_eq!(body["products"][0], TRANSACTIONS);
        assert_eq!(body["user"]["client_user_id"], "user-1");
        assert!(body.get("access_token").is_none());

        // Hosted Link is asked for even with nothing to configure.
        assert!(body["hosted_link"].is_object());
    }

    /// The one shape Plaid rejects, and the reason `update` exists.
    #[test]
    fn an_update_carries_a_token_and_names_no_products() {
        let body = LinkTokenRequest::update("user-1", "access-sandbox-1").body();

        assert_eq!(body["access_token"], "access-sandbox-1");
        assert!(
            body.get("products").is_none(),
            "an update must not name products: {body}",
        );
    }

    #[test]
    fn a_phone_says_where_to_send_the_browser_back_to() {
        let body = LinkTokenRequest::hosted("user-1")
            .redirecting_to("meridian://linked", true)
            .notifying("https://example.com/plaid/webhook")
            .body();

        assert_eq!(
            body["hosted_link"]["completion_redirect_uri"],
            "meridian://linked"
        );
        assert_eq!(body["hosted_link"]["is_mobile_app"], true);
        assert_eq!(body["webhook"], "https://example.com/plaid/webhook");
    }
}
