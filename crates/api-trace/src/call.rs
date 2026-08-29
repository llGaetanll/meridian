//! Logging one call to an external API.
//!
//! `->` is a request leaving, `<-` is a response arriving, and that is all the
//! arrows mean. They carry the same `id`, so a response can be tied to its
//! request even when several are in flight.
//!
//! The two lines record what crossed the network and nothing else. What we
//! then make of a response — whether it parses, whether it holds what we
//! wanted — is a separate thing that happened afterwards, and gets its own
//! line. A response that arrived is logged as one whatever we do with it next.
//!
//! The point of the type is that a request cannot go unaccounted for. A call
//! dropped without an outcome — what a cancelled request looks like from here
//! — says so instead of leaving a request line with no answer.

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use chrono::DateTime;
use chrono::Utc;

use crate::Source;
use crate::scrub::Context;
use crate::scrub::Direction;

/// Everything about talking to the outside world logs under this target, so
/// `target=api` alone gives the full story of what we asked for and got back.
pub const TARGET: &str = "api";

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[must_use = "an ApiCall must be given an outcome"]
pub struct ApiCall {
    id: u64,
    source: Source,
    /// Kept for the response's rules, which are told the endpoint the body
    /// came back from. The call already knows it, so asking for it again at
    /// the outcome would be asking the caller to hold it for us.
    url: String,
    start: DateTime<Utc>,
    done: bool,
}

impl ApiCall {
    /// Log an outgoing request and start its clock.
    ///
    /// `source` names the API ("yahoo", "coingecko") and `url` is the request
    /// as sent, query string and all, so it can be replayed from the log.
    pub fn get(source: Source, url: impl Display) -> Self {
        Self::new(source, "GET", url, None)
    }

    /// The same, for an API that only answers POSTs, and the body it carries.
    ///
    /// **The body is logged as it was sent, credentials and all**, minus
    /// whatever the source's rules take out of it. A response tells
    /// you what the far end thought you asked for; it never tells you what you
    /// actually sent, so a request carrying a field we did not intend is
    /// invisible from the other half of the pair. That is the failure that
    /// cannot be debugged after the fact, and it is why this is worth what it
    /// costs.
    ///
    /// What it costs is that a log file is now a credential: Plaid's secret
    /// rides in every body, and it is account-wide rather than per-user.
    /// `docs/LOGGING.md` says so, and says what has to change before anything
    /// runs against Production.
    pub fn post(source: Source, url: impl Display, body: &str) -> Self {
        Self::new(source, "POST", url, Some(body))
    }

    fn new(source: Source, method: &'static str, url: impl Display, body: Option<&str>) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        // Rendered once: `url` is only `Display`, and the rules are told which
        // endpoint they are looking at.
        let url = url.to_string();

        let body = body.map(|body| {
            source.scrub().apply(
                &Context {
                    source: source.name(),
                    direction: Direction::Request,
                    url: &url,
                },
                Cow::Borrowed(body),
            )
        });

        // Two calls rather than one with an empty field. `tracing` fixes an
        // event's fields at the callsite, so the alternative is every GET
        // carrying a `body=""` that reads as a request that sent one.
        match body {
            Some(body) => tracing::info!(
                target: TARGET,
                id,
                source = source.name(),
                method,
                url = url.as_str(),
                body = body.as_ref(),
                "-> request",
            ),
            None => tracing::info!(
                target: TARGET,
                id,
                source = source.name(),
                method,
                url = url.as_str(),
                "-> request",
            ),
        }

        Self {
            id,
            source,
            url,
            start: crate::clock::instant(),
            done: false,
        }
    }

    /// A response arrived. Logs it as it came in, and hands back something to
    /// hang later trouble with it off.
    ///
    /// The body goes through the source's rules and is then cut to the first
    /// few hundred characters. A size would tell us a response was wrong but
    /// never how; the head of the body usually tells us both, and it is what
    /// we would ask the user for anyway. Which is most of why the rules are
    /// worth having — those few hundred characters are all we get, and a
    /// pretty-printed body spends them on indentation.
    pub fn response(mut self, status: u16, body: &str) -> Response {
        self.done = true;

        let body = self.source.scrub().apply(
            &Context {
                source: self.source.name(),
                direction: Direction::Response,
                url: &self.url,
            },
            Cow::Borrowed(body),
        );

        tracing::info!(
            target: TARGET,
            id = self.id,
            source = self.source.name(),
            status,
            ms = crate::clock::millis_since(self.start),
            body = body.as_ref(),
            "<- response",
        );

        Response {
            id: self.id,
            source: self.source,
        }
    }

    /// No response: the request never made it there or never made it back.
    pub fn failed(mut self, error: impl Display) {
        self.done = true;

        tracing::warn!(
            target: TARGET,
            id = self.id,
            source = self.source.name(),
            ms = crate::clock::millis_since(self.start),
            error = %error,
            "request failed",
        );
    }
}

impl Drop for ApiCall {
    fn drop(&mut self) {
        if self.done {
            return;
        }

        // Dropped before an outcome: the future was cancelled. Common and not
        // an error — a range switch abandons the request for the old range —
        // but a request line with no answer would read like a hang.
        tracing::debug!(
            target: TARGET,
            id = self.id,
            source = self.source.name(),
            ms = crate::clock::millis_since(self.start),
            "request cancelled",
        );
    }
}

/// A response that arrived and has already been logged.
pub struct Response {
    id: u64,
    source: Source,
}

impl Response {
    /// The body did not parse. The body itself is on the response line above,
    /// under the same id, so this line only has to say what went wrong.
    pub fn parse_error(self, error: impl Display) {
        tracing::error!(
            target: TARGET,
            source = self.source.name(),
            error = %error,
            "[parse error id={}]", self.id,
        );
    }
}
