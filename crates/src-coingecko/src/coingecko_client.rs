use api_trace::ApiCall;

use crate::GcError;
use crate::GcRequest;

/// What this client is called in the logs.
const SOURCE: &str = "coingecko";

pub struct CoinGeckoClient {
    client: reqwest::Client,
}

impl CoinGeckoClient {
    pub fn new() -> Result<Self, GcError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::USER_AGENT,
                    reqwest::header::HeaderValue::from_static(
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                    ),
                );
                headers
            })
            .build()?;

        Ok(Self { client })
    }

    pub async fn send_request<Response: serde::de::DeserializeOwned>(
        &self,
        req: &dyn GcRequest,
    ) -> Result<Response, GcError> {
        let url = req.url();
        let call = ApiCall::get(SOURCE, &url);

        let response = match self.client.get(url).send().await {
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

        let response = call.response(status.as_u16(), &body);

        // Check for rate limiting
        if status == 429 {
            return Err(GcError::TooManyRequests);
        }

        match serde_json::from_str(&body) {
            Ok(res) => Ok(res),
            Err(e) => {
                response.parse_error(&e);
                Err(e.into())
            }
        }
    }
}
