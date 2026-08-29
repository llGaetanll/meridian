use api_trace::ApiCall;
use api_trace::Source;
use serde::Deserialize;

use crate::Email;
use crate::ResendError;

/// What this client is called in the logs.
///
/// No scrub rule, and that is a decision rather than an omission: the body
/// carries the address and the sign-in code, and while both are already in the
/// line above it there is nothing here an attacker with the log does not
/// already have. It wants revisiting the first time this sends anything else.
const SOURCE: Source = Source::new("resend");

const BASE_URL: &str = "https://api.resend.com/";

pub struct ResendCredentials {
    pub api_key: String,

    /// Who the message comes from. On a domain verified with Resend, or every
    /// send is a 403.
    pub from: String,
}

impl ResendCredentials {
    pub fn from_env() -> Result<Self, ResendError> {
        let var = |key: &str| {
            std::env::var(key).map_err(|_| ResendError::Config(format!("{key} is not set")))
        };

        Ok(Self {
            api_key: var("RESEND_API_KEY")?,
            from: var("RESEND_FROM")?,
        })
    }
}

pub struct ResendClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
    api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct Sent {
    pub id: String,
}

impl ResendClient {
    pub fn new(credentials: &ResendCredentials) -> Result<Self, ResendError> {
        Self::with_base_url(BASE_URL, credentials)
    }

    /// A client pointed somewhere other than Resend, for a test that wants to
    /// see what would have been sent.
    pub fn with_base_url(
        base_url: &str,
        credentials: &ResendCredentials,
    ) -> Result<Self, ResendError> {
        let base_url = reqwest::Url::parse(base_url)
            .map_err(|e| ResendError::Config(format!("base url: {e}")))?;

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            base_url,
            api_key: credentials.api_key.clone(),
        })
    }

    pub(crate) async fn send(&self, email: &Email) -> Result<Sent, ResendError> {
        let url = self
            .base_url
            .join("emails")
            .map_err(|e| ResendError::Config(format!("url: {e}")))?;

        let body = serde_json::to_string(email).unwrap_or_default();

        // The key is a header rather than part of the body, so unlike Plaid
        // the logged body carries no credential.
        let call = ApiCall::post(SOURCE, &url, &body);

        let response = match self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .json(email)
            .send()
            .await
        {
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

        if !status.is_success() {
            return Err(ResendError::Resend {
                status: status.as_u16(),
                message: body,
            });
        }

        match serde_json::from_str(&body) {
            Ok(sent) => Ok(sent),
            Err(e) => {
                logged.parse_error(&e);

                Err(ResendError::Resend {
                    status: status.as_u16(),
                    message: format!("could not read the reply: {e}"),
                })
            }
        }
    }
}
