use api_trace::ApiCall;
use serde_json::Value;

use crate::PlaidEnv;
use crate::PlaidError;
use crate::PlaidRequest;

/// What this client is called in the logs.
const SOURCE: &str = "plaid";

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
    /// `PLAID_ENV` is the only optional one, and leaving it out means Sandbox.
    pub fn from_env() -> Result<Self, PlaidError> {
        let var = |key: &str| {
            std::env::var(key).map_err(|_| PlaidError::InvalidUrl(format!("{key} is not set")))
        };

        Ok(Self {
            client_id: var("PLAID_CLIENT_ID")?,
            secret: var("PLAID_SECRET")?,
            env: PlaidEnv::from_env(),
        })
    }
}

pub struct PlaidClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
    client_id: String,
    secret: String,
}

impl PlaidClient {
    /// A client for the environment the credentials name.
    pub fn new(credentials: PlaidCredentials) -> Result<Self, PlaidError> {
        let base_url = credentials.env.base_url();

        // Which environment a session ran against is the first thing worth
        // knowing when a number looks wrong, and the last thing anyone
        // remembers to write down.
        tracing::info!(
            target: "app",
            env = credentials.env.as_str(),
            base_url,
            "plaid client",
        );

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
        })
    }

    /// `&(dyn PlaidRequest + Sync)` rather than `&dyn PlaidRequest`: without the
    /// `Sync` bound the returned future is not `Send`, and a multi-threaded
    /// runtime (the server's) cannot hold it across an await. Still dynamic,
    /// so this stays one function rather than one per request type.
    pub async fn send_request<Response: serde::de::DeserializeOwned>(
        &self,
        req: &(dyn PlaidRequest + Sync),
    ) -> Result<Response, PlaidError> {
        let url = self
            .base_url
            .join(&req.path())
            .map_err(|e| PlaidError::InvalidUrl(e.to_string()))?;

        let body = self.authenticated_body(req)?;

        // POST rather than GET, and the body stays out of the log: it holds
        // the secret. The path says what was asked for.
        let call = ApiCall::post(SOURCE, &url);

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
    /// Credentials are put in here, at the last moment, rather than held on
    /// the request types — so nothing that could be logged or cloned around
    /// ever carries the secret.
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

    #[test]
    fn credentials_are_added_to_the_body() {
        let body = client(crate::SANDBOX_BASE_URL)
            .authenticated_body(&Sync)
            .unwrap();

        assert_eq!(body["client_id"], "test-id");
        assert_eq!(body["secret"], "test-secret");
        // The request's own fields survive.
        assert_eq!(body["access_token"], "access-sandbox-1");
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
