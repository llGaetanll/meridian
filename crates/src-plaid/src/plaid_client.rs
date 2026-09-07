use api_trace::ApiCall;
use api_trace::Source;
use api_trace::scrub;
use serde_json::Value;

use crate::PlaidEnv;
use crate::PlaidError;
use crate::PlaidHttpRequest;
use crate::PlaidRequest;

/// What this client is called in the logs, and how its bodies get there.
///
/// Redaction first: every request carries the account-wide `secret`, and an
/// exchange answers with a bank's access token, so a log file was a credential
/// until this line. It runs before the reformatting so that no rule after it
/// is handed one.
///
/// Then compaction, because Plaid pretty-prints: a response arrives as
/// four-space-indented JSON and spends most of the 400 characters a line is
/// cut to on indentation — a `/accounts/balance/get` body got as far as the
/// second account's name.
const SOURCE: Source =
    Source::new("plaid").through(&[scrub::redact_credentials, scrub::compact_json]);

/// Plaid's credentials, and the environment they belong to.
///
/// `secret` differs between Sandbox and Production, so the two travel together
/// — a Sandbox secret pointed at Production is a confusing 400 rather than an
/// obvious mistake.
pub struct PlaidCredentials {
    pub client_id: String,
    pub secret: String,
    pub env: PlaidEnv,
}

impl PlaidCredentials {
    /// Credentials from `PLAID_CLIENT_ID`, `PLAID_SECRET` and `PLAID_ENV`.
    ///
    /// All three are required. None of them has a sensible default — two are
    /// secrets and the third decides whether this talks to real money — so an
    /// environment that has not set them is one this cannot guess for.
    pub fn from_env() -> Result<Self, PlaidError> {
        let var = |key: &str| {
            std::env::var(key).map_err(|_| PlaidError::Config(format!("{key} is not set")))
        };

        Ok(Self {
            client_id: var("PLAID_CLIENT_ID")?,
            secret: var("PLAID_SECRET")?,
            env: PlaidEnv::from_env()?,
        })
    }
}

pub struct PlaidClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
    client_id: String,
    secret: String,

    /// Which Plaid this talks to. Kept so that a caller holding only the
    /// client can still say which one it is — the alternative is keeping the
    /// credentials alive next to it for the sake of one field.
    env: PlaidEnv,
}

impl PlaidClient {
    /// A client for the environment the credentials name.
    pub fn new(credentials: PlaidCredentials) -> Result<Self, PlaidError> {
        let base_url = credentials.env.base_url();

        // Which environment a session ran against is the first thing worth
        // knowing when a number looks wrong, and the last thing anyone
        // remembers to write down.
        tracing::info!(env = credentials.env.as_str(), base_url, "plaid client");

        Self::with_base_url(base_url, credentials)
    }

    /// A client whose requests are resolved against `base_url` rather than
    /// Plaid's own host. The base must end in a `/`, and requests keep their
    /// paths, e.g. `<base>/transactions/sync`.
    ///
    /// This is for tests pointing at a local server. Unlike the price sources
    /// there is no proxy case to serve: Plaid is only ever called from the
    /// server, which can reach it directly.
    pub fn with_base_url(
        base_url: &str,
        credentials: PlaidCredentials,
    ) -> Result<Self, PlaidError> {
        let base_url =
            reqwest::Url::parse(base_url).map_err(|e| PlaidError::InvalidUrl(e.to_string()))?;

        let builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(30));

        Ok(Self {
            client: builder.build()?,
            base_url,
            client_id: credentials.client_id,
            secret: credentials.secret,
            env: credentials.env,
        })
    }

    /// Which Plaid this client talks to.
    pub fn env(&self) -> PlaidEnv {
        self.env
    }

    /// Build a request without sending it.
    ///
    /// Public because it is the half worth testing: the path, the base it
    /// resolves against — which is what picks Sandbox or Production — and
    /// which fields the body ends up carrying are all decided here, and none
    /// of them needs a network to be checked.
    pub fn build_request(&self, req: &dyn PlaidRequest) -> Result<PlaidHttpRequest, PlaidError> {
        let url = self
            .base_url
            .join(&req.path())
            .map_err(|e| PlaidError::InvalidUrl(e.to_string()))?;

        Ok(PlaidHttpRequest {
            url,
            body: self.authenticated_body(req)?,
        })
    }

    /// `&(dyn PlaidRequest + Sync)` rather than `&dyn PlaidRequest`: an `async
    /// fn` holds its arguments for the whole body, and without the `Sync`
    /// bound the returned future is not `Send` — which a multi-threaded
    /// runtime (the server's) cannot hold across an await. Still dynamic, so
    /// this stays one function rather than one per request type.
    pub async fn send_request<Response: serde::de::DeserializeOwned>(
        &self,
        req: &(dyn PlaidRequest + Sync),
    ) -> Result<Response, PlaidError> {
        self.send(self.build_request(req)?).await
    }

    /// Put an already-built request on the wire.
    async fn send<Response: serde::de::DeserializeOwned>(
        &self,
        request: PlaidHttpRequest,
    ) -> Result<Response, PlaidError> {
        let PlaidHttpRequest { url, body } = request;

        // The body is logged as it is sent, field for field, with the
        // credentials in it replaced by their length — `SOURCE`'s rules. See
        // `ApiCall::post` for why the rest of it is worth logging.
        let call = ApiCall::post(module_path!(), SOURCE, &url, &body.to_string());

        let response = match self.client.post(url).json(&body).send().await {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        let status = response.status();

        let body = match response.text().await {
            Ok(body) => body,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        let logged = call.response(status.as_u16(), &body);

        if status == 429 {
            return Err(PlaidError::TooManyRequests);
        }

        // Plaid puts the reason in the body, so an error is parsed rather than
        // read off the status. Checked before the success type: a 400 body
        // would fail to parse as one, and the parse error would say far less
        // than Plaid already has.
        if !status.is_success() {
            return Err(parse_error_body(&body));
        }

        match serde_json::from_str(&body) {
            Ok(res) => Ok(res),
            Err(e) => {
                logged.parse_error(&e);
                Err(e.into())
            }
        }
    }

    /// The request's body with `client_id` and `secret` added.
    ///
    /// Credentials go in here rather than on the request types, so a
    /// `LinkTokenRequest` and its friends stay plain values that can be built,
    /// cloned and asserted on with no secret anywhere near them.
    ///
    /// They do reach [`PlaidHttpRequest`], which is both what gets sent and
    /// what gets logged — the log keeps the field names and drops the values,
    /// which is `SOURCE` above and `docs/LOGGING.md`.
    fn authenticated_body(&self, req: &dyn PlaidRequest) -> Result<Value, PlaidError> {
        let mut body = req.body();

        let map = body
            .as_object_mut()
            .ok_or_else(|| PlaidError::InvalidUrl("request body is not a JSON object".into()))?;

        map.insert("client_id".into(), Value::String(self.client_id.clone()));
        map.insert("secret".into(), Value::String(self.secret.clone()));

        Ok(body)
    }
}

/// Plaid's error body, as far as we care about it.
fn parse_error_body(body: &str) -> PlaidError {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        // A non-JSON error body is a gateway or a proxy talking, not Plaid.
        Err(e) => return PlaidError::JsonError(e),
    };

    let field = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string()
    };

    PlaidError::Plaid {
        error_code: field("error_code"),
        error_message: field("error_message"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sync;

    impl PlaidRequest for Sync {
        fn path(&self) -> String {
            String::from("transactions/sync")
        }

        fn body(&self) -> Value {
            serde_json::json!({ "access_token": "access-sandbox-1" })
        }
    }

    fn client(base: &str) -> PlaidClient {
        PlaidClient::with_base_url(
            base,
            PlaidCredentials {
                client_id: "test-id".into(),
                secret: "test-secret".into(),
                env: PlaidEnv::Sandbox,
            },
        )
        .unwrap()
    }

    /// Request paths must survive the base being swapped, which is the whole
    /// mechanism for choosing an environment.
    #[test]
    fn resolves_paths_against_the_base() {
        let resolve = |base: &str| {
            reqwest::Url::parse(base)
                .unwrap()
                .join(&Sync.path())
                .unwrap()
                .to_string()
        };

        assert_eq!(
            resolve(crate::SANDBOX_BASE_URL),
            "https://sandbox.plaid.com/transactions/sync"
        );
        assert_eq!(
            resolve(crate::PRODUCTION_BASE_URL),
            "https://production.plaid.com/transactions/sync"
        );
        assert_eq!(
            resolve("http://localhost:3000/"),
            "http://localhost:3000/transactions/sync"
        );
    }

    /// The whole point of building being its own step: what would go on the
    /// wire is a value, and a value can be asserted on with no network.
    #[test]
    fn a_built_request_carries_the_url_and_the_credentials() {
        let built = client(crate::SANDBOX_BASE_URL)
            .build_request(&Sync)
            .unwrap();

        assert_eq!(
            built.url.as_str(),
            "https://sandbox.plaid.com/transactions/sync",
        );

        assert_eq!(built.body["client_id"], "test-id");
        assert_eq!(built.body["secret"], "test-secret");
        // The request's own fields survive.
        assert_eq!(built.body["access_token"], "access-sandbox-1");
    }

    /// The base is what chooses the environment, so it is what a built request
    /// has to carry. Getting this wrong is how a Sandbox secret reaches
    /// Production and comes back as a puzzling 400.
    #[test]
    fn a_built_request_names_the_environment_it_is_for() {
        let built = client(crate::PRODUCTION_BASE_URL)
            .build_request(&Sync)
            .unwrap();

        assert_eq!(
            built.url.as_str(),
            "https://production.plaid.com/transactions/sync",
        );
    }

    #[test]
    fn plaid_errors_carry_their_code() {
        let err = parse_error_body(
            r#"{"error_code":"ITEM_LOGIN_REQUIRED","error_message":"the login details have changed"}"#,
        );

        match err {
            PlaidError::Plaid {
                error_code,
                error_message,
            } => {
                assert_eq!(error_code, "ITEM_LOGIN_REQUIRED");
                assert_eq!(error_message, "the login details have changed");
            }
            other => panic!("expected a Plaid error, got {other:?}"),
        }
    }
}
