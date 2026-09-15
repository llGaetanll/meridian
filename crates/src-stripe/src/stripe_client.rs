use api_trace::ApiCall;
use api_trace::Source;
use api_trace::scrub;
use serde::de::DeserializeOwned;

use crate::StripeEnv;
use crate::StripeError;

/// What this client is called in the logs.
///
/// `redact_credentials` because Stripe's errors quote the request back, and a
/// request that failed on the key is one whose body carries it. `compact_json`
/// for the same reason Plaid has it: the responses are pretty-printed and a
/// subscription is a long object.
const SOURCE: Source =
    Source::new("stripe").through(&[scrub::redact_credentials, scrub::compact_json]);

const BASE_URL: &str = "https://api.stripe.com/v1/";

/// The version of Stripe's API this crate is written against.
///
/// Pinned rather than taking whatever the account defaults to: Stripe changes
/// the shape of its objects between versions, and a response that grew a field
/// is not the thing that should decide whether somebody is subscribed. Moving
/// it is a deliberate edit with the changelog open.
///
/// **A webhook endpoint has a version of its own, and it must be this one.**
/// Registered without one it sends whatever the account currently defaults to,
/// so the objects this crate parses would come from two versions at once — the
/// pinned one for a reply to a call we made, the account's for a delivery we
/// did not. Both shapes agree today, which is exactly what makes the drift
/// worth pinning rather than watching: nothing would fail until the day it
/// does. #88 registers the endpoint with `api_version` set to this.
pub const API_VERSION: &str = "2026-08-26.dahlia";

/// The key, the price it sells, and the mode both belong to.
///
/// The mode travels with the key rather than being read off it. A key alone
/// would make a live key in a test `.env` a server that charges real cards
/// and says nothing; the mode beside it makes that a server that does not
/// start.
pub struct StripeCredentials {
    /// `sk_test_…` or `sk_live_…`, and it has to agree with `env`.
    pub secret_key: String,

    /// The recurring price a subscription is created against. `price_…`, not
    /// `prod_…` — a product can carry several prices, so the product names no
    /// particular amount.
    pub price_id: String,

    /// Which Stripe these belong to.
    pub env: StripeEnv,
}

impl StripeCredentials {
    /// Credentials from `STRIPE_SECRET_KEY`, `STRIPE_PRICE_ID` and
    /// `STRIPE_ENV`.
    ///
    /// All three are required, and the key's prefix has to agree with the
    /// mode. A price belongs to one mode too, but its id does not say which,
    /// so that one is Stripe's to refuse.
    pub fn from_env() -> Result<Self, StripeError> {
        let var = |key: &str| {
            std::env::var(key).map_err(|_| StripeError::Config(format!("{key} is not set")))
        };

        let env = StripeEnv::from_env()?;
        let secret_key = var("STRIPE_SECRET_KEY")?;

        Self::check_key("STRIPE_SECRET_KEY", &secret_key, env)?;

        Ok(Self {
            secret_key,
            price_id: var("STRIPE_PRICE_ID")?,
            env,
        })
    }

    /// Whether a key is one of `env`'s. A key from the other mode, or a
    /// string that is not a key, is a configuration error naming `name`.
    pub fn check_key(name: &str, key: &str, env: StripeEnv) -> Result<(), StripeError> {
        match StripeEnv::of_key(key) {
            Some(of_key) if of_key == env => Ok(()),
            Some(of_key) => Err(StripeError::Config(format!(
                "{name} is a {} key and STRIPE_ENV is `{}`",
                of_key.as_str(),
                env.as_str(),
            ))),
            None => Err(StripeError::Config(format!("{name} is not a Stripe key",))),
        }
    }
}

pub struct StripeClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
    secret_key: String,
    price_id: String,

    /// Which Stripe this talks to. Kept so that a caller holding only the
    /// client can still say which one it is.
    env: StripeEnv,
}

impl StripeClient {
    /// A client for the mode the credentials name.
    pub fn new(credentials: &StripeCredentials) -> Result<Self, StripeError> {
        // Which mode a session ran in is the first thing worth knowing when
        // a charge looks wrong, and the last thing anyone remembers to write
        // down.
        tracing::info!(
            env = credentials.env.as_str(),
            base_url = BASE_URL,
            "stripe client"
        );

        Self::with_base_url(BASE_URL, credentials)
    }

    /// A client pointed somewhere other than Stripe, for a test that wants to
    /// see what would have been sent.
    pub fn with_base_url(
        base_url: &str,
        credentials: &StripeCredentials,
    ) -> Result<Self, StripeError> {
        let base_url = reqwest::Url::parse(base_url)
            .map_err(|e| StripeError::Config(format!("base url: {e}")))?;

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            base_url,
            secret_key: credentials.secret_key.clone(),
            price_id: credentials.price_id.clone(),
            env: credentials.env,
        })
    }

    /// Which Stripe this client talks to.
    pub fn env(&self) -> StripeEnv {
        self.env
    }

    /// The price a subscription is created against.
    pub fn price_id(&self) -> &str {
        &self.price_id
    }

    /// POST a form to one endpoint. Stripe takes
    /// `application/x-www-form-urlencoded` and answers JSON, including for the
    /// errors.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &[(String, String)],
    ) -> Result<T, StripeError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| StripeError::Config(format!("url: {e}")))?;

        let body = serde_urlencoded::to_string(form).unwrap_or_default();

        let call = ApiCall::post(module_path!(), SOURCE, &url, &body);

        let response = match self
            .client
            .post(url)
            .basic_auth(&self.secret_key, None::<&str>)
            .header("Stripe-Version", API_VERSION)
            .form(form)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        self.read(call, response).await
    }

    /// GET one object.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, StripeError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| StripeError::Config(format!("url: {e}")))?;

        let call = ApiCall::get(module_path!(), SOURCE, &url);

        let response = match self
            .client
            .get(url)
            .basic_auth(&self.secret_key, None::<&str>)
            .header("Stripe-Version", API_VERSION)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        self.read(call, response).await
    }

    /// The half both verbs share: read the body, log it, and parse it or say
    /// why it could not be.
    async fn read<T: DeserializeOwned>(
        &self,
        call: ApiCall,
        response: reqwest::Response,
    ) -> Result<T, StripeError> {
        let status = response.status();

        let body = match response.text().await {
            Ok(body) => body,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        let logged = call.response(status.as_u16(), &body);

        if !status.is_success() {
            return Err(crate::stripe_rejection::rejection(status.as_u16(), &body));
        }

        match serde_json::from_str(&body) {
            Ok(value) => Ok(value),
            Err(e) => {
                logged.parse_error(&e);

                Err(StripeError::Stripe {
                    status: status.as_u16(),
                    kind: None,
                    message: format!("could not read the reply: {e}"),
                })
            }
        }
    }
}
