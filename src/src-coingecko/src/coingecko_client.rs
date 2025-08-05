use crate::GcError;
use crate::GcRequest;

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
        let response = self.client.get(url).send().await?;

        // Check for rate limiting
        if response.status() == 429 {
            return Err(GcError::TooManyRequests);
        }

        let res = response.json().await?;

        Ok(res)
    }
}
