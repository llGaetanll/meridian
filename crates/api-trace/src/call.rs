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

use std::fmt::Display;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use chrono::DateTime;
use chrono::Utc;

/// Everything about talking to the outside world logs under this target, so
/// `target=api` alone gives the full story of what we asked for and got back.
pub const TARGET: &str = "api";

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[must_use = "an ApiCall must be given an outcome"]
pub struct ApiCall {
    id: u64,
    source: &'static str,
    start: DateTime<Utc>,
    done: bool,
}

impl ApiCall {
    /// Log an outgoing request and start its clock.
    ///
    /// `source` names the API ("yahoo", "coingecko") and `url` is the request
    /// as sent, query string and all, so it can be replayed from the log.
    pub fn get(source: &'static str, url: impl Display) -> Self {
        Self::new(source, "GET", url)
    }

    /// The same, for an API that only answers POSTs.
    ///
    /// The body is deliberately not logged. A POST to Plaid carries the
    /// secret in its body, and a log is the one place it must never end up.
    /// What was asked for is recoverable from the path.
    pub fn post(source: &'static str, url: impl Display) -> Self {
        Self::new(source, "POST", url)
    }

    fn new(source: &'static str, method: &'static str, url: impl Display) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        tracing::info!(
            target: TARGET,
            id,
            source,
            method,
            url = %url,
            "-> request",
        );

        Self {
            id,
            source,
            start: crate::clock::instant(),
            done: false,
        }
    }

    /// A response arrived. Logs it as it came in, and hands back something to
    /// hang later trouble with it off.
    ///
    /// The body is logged, cut to the first few hundred characters. A size
    /// would tell us a response was wrong but never how; the head of the body
    /// usually tells us both, and it is what we would ask the user for anyway.
    pub fn response(mut self, status: u16, body: &str) -> Response {
        self.done = true;

        tracing::info!(
            target: TARGET,
            id = self.id,
            source = self.source,
            status,
            ms = crate::clock::millis_since(self.start),
            body,
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
            source = self.source,
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
            source = self.source,
            ms = crate::clock::millis_since(self.start),
            "request cancelled",
        );
    }
}

/// A response that arrived and has already been logged.
pub struct Response {
    id: u64,
    source: &'static str,
}

impl Response {
    /// The body did not parse. The body itself is on the response line above,
    /// under the same id, so this line only has to say what went wrong.
    pub fn parse_error(self, error: impl Display) {
        tracing::error!(
            target: TARGET,
            source = self.source,
            error = %error,
            "[parse error id={}]", self.id,
        );
    }
}
