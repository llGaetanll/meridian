//! Which origins a browser may call this server from.
//!
//! The only reason this server has any CORS configuration at all. The iOS app
//! is not a browser, Plaid is a server, and a developer with `curl` is neither
//! — none of them ask a browser for permission, and none of them are affected
//! by anything here. This exists for the web app and for nothing else.

use anyhow::anyhow;
use anyhow::bail;
use axum::http::HeaderValue;

/// The origins the web app is served from.
///
/// Required, with no default and no empty case, like every other variable this
/// server reads. A server with nothing here is one whose web app is blocked by
/// every browser, and that failure is invisible from this side: the browser
/// refuses the request before sending it, so nothing arrives and no line in
/// this log says anything went wrong. Refusing to start is the only version of
/// that mistake anybody finds.
#[derive(Debug, Clone)]
pub struct WebOrigins {
    /// What the log prints, comma separated.
    named: String,

    /// What the CORS layer matches an `Origin` header against.
    values: Vec<HeaderValue>,
}

impl WebOrigins {
    /// The origins named by `WEB_ORIGINS`, comma separated.
    pub fn from_env() -> anyhow::Result<Self> {
        let value = std::env::var("WEB_ORIGINS").map_err(|_| {
            anyhow!("WEB_ORIGINS is not set — it names the origins the web app is served from")
        })?;

        Self::parse(&value)
    }

    /// The origins a string names, checked on the way up.
    ///
    /// Separate from [`from_env`](WebOrigins::from_env) so the parse is
    /// testable without the process environment, which every test in the
    /// binary shares.
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let mut named = Vec::new();
        let mut values = Vec::new();

        // An empty variable splits to one empty entry and is refused by
        // `one`, which is the answer we want: `WEB_ORIGINS=` is somebody who
        // has not decided rather than somebody who means no browsers.
        for origin in value.split(',') {
            let origin = Self::one(origin)?;

            let header = HeaderValue::from_str(&origin)
                .map_err(|_| anyhow!("WEB_ORIGINS cannot be sent as a header: `{origin}`"))?;

            named.push(origin);
            values.push(header);
        }

        Ok(Self {
            named: named.join(","),
            values,
        })
    }

    /// One origin, normalised and checked.
    ///
    /// An origin is a scheme and a host and nothing else — that is exactly
    /// what a browser puts in `Origin`, and the comparison against it is byte
    /// for byte. Anything more matches nothing, and matching nothing is the
    /// failure with no log line, so each of these is refused here instead.
    ///
    /// Lowercased for the same reason: a browser sends `https://app.example`
    /// however the address was typed, and `https://App.example` in this
    /// variable would be a server that starts, looks configured, and blocks
    /// every request the web app makes.
    fn one(value: &str) -> anyhow::Result<String> {
        // A trailing slash is what copying an address out of a browser leaves
        // behind, the same as `PUBLIC_URL`. Taken off rather than refused: it
        // is unambiguous, and the origin under it is the one meant.
        let value = value.trim().trim_end_matches('/').to_lowercase();

        let refuse = |why: &str| anyhow!("WEB_ORIGINS {why}: `{value}`");

        let https = value.strip_prefix("https://");
        let http = value.strip_prefix("http://");

        let Some(host) = https.or(http) else {
            bail!(refuse("must start with https:// or http://"));
        };

        if host.is_empty() {
            bail!(refuse("names no host"));
        }

        if host.contains('/') {
            bail!(refuse("must be a scheme and a host, with no path"));
        }

        if host.contains('?') || host.contains('#') {
            bail!(refuse("must be a bare origin, with no query or fragment"));
        }

        if host.contains(char::is_whitespace) {
            bail!(refuse("has whitespace in it"));
        }

        // Plain http is a development answer and only ever a local one. On the
        // box it would be a web app served over a network anybody can read,
        // and the shape that mistake takes is a production `.env` that still
        // holds whatever was pasted into it on a laptop.
        if https.is_none() && !is_loopback(host) {
            bail!(refuse("may only be http:// on localhost"));
        }

        Ok(value)
    }

    /// What the CORS layer matches an `Origin` header against.
    pub fn values(&self) -> Vec<HeaderValue> {
        self.values.clone()
    }

    /// What the log calls them. Not a secret — these are the public addresses
    /// the web app is served from.
    pub fn as_str(&self) -> &str {
        &self.named
    }
}

/// Whether a host is this machine, port and all.
///
/// The port is taken off by hand rather than by a URL parser, because a v6
/// literal carries colons of its own: `[::1]:8080` splits at the last one and
/// bare `[::1]` must not be split at all.
fn is_loopback(host: &str) -> bool {
    let bare = if host.ends_with(']') {
        host
    } else {
        host.rsplit_once(':').map_or(host, |(host, _)| host)
    };

    matches!(bare, "localhost" | "127.0.0.1" | "[::1]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_origin_is_what_a_browser_would_send() {
        let origins = WebOrigins::parse("https://app.meridiancash.io").unwrap();

        assert_eq!(origins.as_str(), "https://app.meridiancash.io");
    }

    /// The web app and a laptop, which is what the two deployments hold.
    #[test]
    fn several_origins_are_taken_in_order() {
        let origins =
            WebOrigins::parse("https://app.meridiancash.io, http://127.0.0.1:8080").unwrap();

        assert_eq!(
            origins.as_str(),
            "https://app.meridiancash.io,http://127.0.0.1:8080",
        );
        assert_eq!(origins.values().len(), 2);
    }

    /// Both are values that would start a server and then match no request a
    /// browser ever makes.
    #[test]
    fn a_trailing_slash_and_a_capital_are_normalised() {
        let origins = WebOrigins::parse("HTTPS://App.MeridianCash.io/").unwrap();

        assert_eq!(origins.as_str(), "https://app.meridiancash.io");
    }

    #[test]
    fn http_is_allowed_on_this_machine_and_nowhere_else() {
        for value in [
            "http://localhost:8080",
            "http://127.0.0.1:8080",
            "http://localhost",
            "http://[::1]:8080",
        ] {
            assert!(
                WebOrigins::parse(value).is_ok(),
                "`{value}` is this machine"
            );
        }

        for value in ["http://app.meridiancash.io", "http://192.168.1.24:8080"] {
            assert!(WebOrigins::parse(value).is_err(), "`{value}` is a network");
        }
    }

    /// Each of these is a server that would start, look configured, and refuse
    /// every request the web app makes — with nothing in this log to say so.
    #[test]
    fn anything_that_is_not_an_origin_is_refused() {
        for value in [
            "",
            "   ",
            "app.meridiancash.io",
            "https://",
            "https://app.meridiancash.io/snapshot",
            "https://app.meridiancash.io?x=1",
            "https://app meridiancash.io",
            "https://a.io,",
            "https://a.io,,https://b.io",
        ] {
            assert!(
                WebOrigins::parse(value).is_err(),
                "`{value}` must not be accepted",
            );
        }
    }
}
