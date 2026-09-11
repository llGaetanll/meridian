//! Meridian's own server, and the refusals a client has to tell apart.
//!
//! Nothing here is reactive: plain async functions over plain data, like the
//! price source beside it.

use api_common::Timestamp;
use api_trace::ApiCall;
use api_trace::Source;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use server_wire::WireUnpaid;

/// What this client is called in the logs.
const SOURCE: Source = Source::new("meridian");

/// Where the server is when the build does not name one. `MERIDIAN_SERVER` at
/// build time points it elsewhere, as the same variable does on the phone.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8082/";

/// Where the browser keeps the session token.
///
/// `localStorage` and not a cookie: the token is sent as a bearer header, and
/// the server sets nothing for a browser to keep.
#[cfg(target_arch = "wasm32")]
const TOKEN_KEY: &str = "meridian.token";

/// Why the server would not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// **401.** No session, or one the server has stopped honouring. The
    /// answer is a sign-in screen.
    Unauthorized,

    /// **402.** The session is good and has not paid. The answer is the
    /// paywall. Told apart from the 401 on purpose: a client that conflated
    /// the two would sign somebody out for not having paid.
    ///
    /// `None` is a 402 whose body did not parse, which draws the screen that
    /// claims nothing about what came before.
    PaymentRequired(Option<WireUnpaid>),

    /// A status this client has no rule about. The code and not a sentence
    /// about it, so a rule about one of them is a `match` rather than a
    /// comparison against a spelling.
    Answered(u16),

    /// A call that produced no status to read — a request that never arrived,
    /// or an answer that would not parse. Under whatever the library that
    /// failed called it.
    Failed(String),
}

/// What `GET /billing/status` answers with.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BillingStatus {
    /// Whether the app may be used. Decided on the server from the date
    /// against the clock, which is why [`BillingStatus::status`] does not
    /// decide it here either.
    pub entitled: bool,

    /// Why not, present exactly when [`entitled`](BillingStatus::entitled) is
    /// false. Our vocabulary and never a provider's — see [`WireUnpaid`].
    #[serde(default)]
    pub unpaid: Option<WireUnpaid>,

    /// The end of the period that has been paid for.
    pub good_until: Option<Timestamp>,

    /// Whether it stops at the end of that period.
    pub cancel_at_period_end: bool,
}

pub struct Meridian {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl Meridian {
    pub fn new() -> Result<Self, String> {
        let base_url = option_env!("MERIDIAN_SERVER").unwrap_or(DEFAULT_BASE_URL);

        // A base without the trailing slash swallows its last segment when a
        // path is joined onto it, which is a 404 nobody can see the cause of.
        let base_url = match base_url.ends_with('/') {
            true => base_url.to_string(),
            false => format!("{base_url}/"),
        };

        Ok(Self {
            client: reqwest::Client::new(),
            base_url: reqwest::Url::parse(&base_url).map_err(|e| e.to_string())?,
        })
    }

    /// What this session's subscription is.
    ///
    /// One of the two routes a session without one may reach, so a 402 from
    /// here means the token names nobody rather than that nobody has paid.
    pub async fn billing_status(&self, token: &str) -> Result<BillingStatus, Refusal> {
        self.get("billing/status", token).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, token: &str) -> Result<T, Refusal> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| Refusal::Failed(e.to_string()))?;

        let call = ApiCall::get(module_path!(), SOURCE, &url);

        let sent = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await;

        let response = match sent {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);
                return Err(Refusal::Failed(e.to_string()));
            }
        };

        let status = response.status().as_u16();

        let body = match response.text().await {
            Ok(body) => body,
            Err(e) => {
                call.failed(&e);
                return Err(Refusal::Failed(e.to_string()));
            }
        };

        let response = call.response(status, &body);

        // Read before the body is: a refusal carries no body worth parsing,
        // and which one it is decides which screen the app draws.
        match status {
            200 => {}
            401 => return Err(Refusal::Unauthorized),
            402 => return Err(Refusal::PaymentRequired(serde_json::from_str(&body).ok())),
            other => return Err(Refusal::Answered(other)),
        }

        match serde_json::from_str(&body) {
            Ok(value) => Ok(value),
            Err(e) => {
                response.parse_error(&e);
                Err(Refusal::Failed(e.to_string()))
            }
        }
    }
}

/// The session token the browser is holding, if it is holding one.
#[cfg(target_arch = "wasm32")]
pub fn token() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(TOKEN_KEY)
        .ok()?
        .filter(|token| !token.is_empty())
}

/// Nothing to hold it in: there is no session outside a browser.
#[cfg(not(target_arch = "wasm32"))]
pub fn token() -> Option<String> {
    None
}
