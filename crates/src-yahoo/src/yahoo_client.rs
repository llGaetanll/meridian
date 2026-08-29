use api_trace::ApiCall;
use api_trace::Source;

use crate::YError;
use crate::YRequest;

/// What this client is called in the logs.
const SOURCE: Source = Source::new("yahoo");

/// Yahoo's own host. Reachable from native builds (desktop, mobile); browsers
/// have to go through a proxy instead — see [`YahooClient::with_base_url`].
pub const YAHOO_BASE_URL: &str = "https://query1.finance.yahoo.com/";

pub struct YahooClient {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl YahooClient {
    pub fn new() -> Result<Self, YError> {
        Self::with_base_url(YAHOO_BASE_URL)
    }

    /// A client whose requests are resolved against `base_url` instead of
    /// Yahoo's host, for talking to a proxy. The base must end in a `/`, and
    /// requests keep the same paths, e.g. `<base>/v8/finance/chart/SPY`.
    pub fn with_base_url(base_url: &str) -> Result<Self, YError> {
        let base_url =
            reqwest::Url::parse(base_url).map_err(|e| YError::InvalidUrl(e.to_string()))?;

        let builder = reqwest::Client::builder();

        // Yahoo answers 429 to requests without a User-Agent. In the browser
        // the header is set by the browser itself and cannot be overridden,
        // and neither timeouts nor default headers exist on that backend.
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

        Ok(Self {
            client: builder.build()?,
            base_url,
        })
    }

    pub async fn send_request<Response: serde::de::DeserializeOwned>(
        &self,
        req: &dyn YRequest,
    ) -> Result<Response, YError> {
        let url = self
            .base_url
            .join(&req.path())
            .map_err(|e| YError::InvalidUrl(e.to_string()))?;

        let call = ApiCall::get(SOURCE, &url);

        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);
                return Err(e.into());
            }
        };

        let status = response.status();

        // Read the body as text rather than straight into the response type:
        // it costs one copy and buys the byte count for the log, plus the body
        // itself to log when a response stops matching what we parse.
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
            return Err(YError::TooManyRequests);
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Chart;

    impl YRequest for Chart {
        fn path(&self) -> String {
            String::from("v8/finance/chart/SPY?range=1d&interval=1m")
        }
    }

    /// Request paths must survive being pointed at a proxy, base path and all.
    #[test]
    fn resolves_paths_against_the_base() {
        let resolve = |base: &str| {
            reqwest::Url::parse(base)
                .unwrap()
                .join(&Chart.path())
                .unwrap()
                .to_string()
        };

        assert_eq!(
            resolve(YAHOO_BASE_URL),
            "https://query1.finance.yahoo.com/v8/finance/chart/SPY?range=1d&interval=1m"
        );
        assert_eq!(
            resolve("http://localhost:8080/"),
            "http://localhost:8080/v8/finance/chart/SPY?range=1d&interval=1m"
        );
    }
}
