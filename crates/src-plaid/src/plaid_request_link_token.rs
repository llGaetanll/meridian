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
    /// What Link calls us on its own page — [`CLIENT_NAME`], and the
    /// constraint that comes with it.
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

    /// Where **the bank's own app** hands control back, part way through.
    ///
    /// Easy to read as another spelling of `completion_redirect_uri` and it is
    /// not one. An OAuth bank does not take the sign-in on Plaid's page: it
    /// hands off to the bank's own app, and when that app is finished the only
    /// way it has of returning control is to open a URL. This is that URL, so
    /// it is opened by a different app while the session is still going,
    /// rather than by Plaid's page at the end of it.
    ///
    /// That difference is why it cannot be a custom scheme: anything can claim
    /// one, so it has to be an https Universal Link on a domain that vouches
    /// for the app, registered in the Plaid dashboard. `android_package_name`
    /// cannot stand in for it either.
    ///
    /// Its presence is what sends `is_mobile_app`, and the two are one
    /// decision rather than two. Saying a native app opened the session
    /// without giving the bank an address that reaches that app is the worst
    /// of the three: the user is stranded in their bank's app *after* signing
    /// in.
    pub redirect_uri: Option<String>,

    /// How long the hosted URL stays good for. Plaid's default is 30 minutes
    /// when it is not being delivered by SMS or email, which is the case here.
    pub url_lifetime_seconds: Option<u32>,
}

/// What Plaid's consent screen calls us — "Meridian would like to connect to
/// your account". It must match the app name in the Plaid dashboard profile
/// exactly, and no API exposes that profile, so keeping the two the same is a
/// hand check: a change here is a change there. Plaid caps it at 30
/// characters.
pub const CLIENT_NAME: &str = "Meridian";

/// The only product the app uses. Balances and recurring both come off the
/// Item that `transactions` creates, so naming them again would only widen
/// what the user is asked to consent to.
pub const TRANSACTIONS: &str = "transactions";

impl LinkTokenRequest {
    /// Attach a bank that is not attached yet.
    pub fn hosted(client_user_id: impl Into<String>) -> Self {
        Self {
            client_name: String::from(CLIENT_NAME),
            language: String::from("en"),
            country_codes: vec![String::from("US")],
            client_user_id: client_user_id.into(),
            products: vec![String::from(TRANSACTIONS)],
            webhook: None,
            access_token: None,
            completion_redirect_uri: None,
            redirect_uri: None,
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

    /// Where to send the browser when the flow ends.
    pub fn redirecting_to(mut self, uri: impl Into<String>) -> Self {
        self.completion_redirect_uri = Some(uri.into());
        self
    }

    /// Where the bank's own app hands control back to ours, and with it that a
    /// native app is what opened the session.
    ///
    /// One method for both because they are one claim. There is no way through
    /// this builder to say the second without the first.
    pub fn handing_back_to(mut self, uri: impl Into<String>) -> Self {
        self.redirect_uri = Some(uri.into());
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

        if self.redirect_uri.is_some() {
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

        // Top level rather than inside `hosted_link`, unlike its neighbour.
        // The two are different parameters answering different moments.
        if let Some(uri) = &self.redirect_uri {
            body["redirect_uri"] = Value::String(uri.clone());
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
            .redirecting_to("meridian://linked")
            .notifying("https://example.com/plaid/webhook")
            .body();

        assert_eq!(
            body["hosted_link"]["completion_redirect_uri"],
            "meridian://linked"
        );
        assert_eq!(body["webhook"], "https://example.com/plaid/webhook");
    }

    /// The pairing the flow depends on: a phone that cannot catch a handback
    /// must not be announced to Plaid as one that can.
    #[test]
    fn is_mobile_app_is_sent_only_alongside_a_handback() {
        let without = LinkTokenRequest::hosted("user-1")
            .redirecting_to("meridian://linked")
            .body();

        assert!(
            without["hosted_link"].get("is_mobile_app").is_none(),
            "a completion redirect alone must not claim a native app: {without}",
        );
        assert!(without.get("redirect_uri").is_none());

        let with = LinkTokenRequest::hosted("user-1")
            .redirecting_to("meridian://linked")
            .handing_back_to("https://api.meridiancash.io/link/oauth")
            .body();

        assert_eq!(with["hosted_link"]["is_mobile_app"], true);
        assert_eq!(
            with["redirect_uri"],
            "https://api.meridiancash.io/link/oauth"
        );
    }
}
