//! Logging one request somebody made of us.
//!
//! The mirror of [`crate::ApiCall`], and the other half of the log's rule that
//! everything crossing the network is written down. A call we make is `->`
//! then `<-`; a call made of us is `<-` then `->`, because the arrows only
//! ever say which way the traffic went. The two kinds are told apart by which
//! arrow comes first and by `client=`, which names the far end of a request we
//! did not start.
//!
//! Both lines carry the same `id`, drawn from the same counter as outgoing
//! calls, so an id is unique across everything the process talks to and a
//! server handling several requests at once still reads as pairs.
//!
//! # No bodies
//!
//! Neither line carries one, which is the one place this departs from
//! [`crate::ApiCall`]. A request body here is at most an address and a sign-in
//! code, and both are already logged by name at the point something is decided
//! about them. A response body is somebody's ledger, and the first 400
//! characters of it is enough to be personal data and not enough to debug
//! from — the server's own lines already say what it decided to serve.
//! `bytes` is what is actually worth knowing about an answer: a snapshot that
//! left as 300 bytes rather than 300 kilobytes is the bug.

use std::fmt::Display;

use chrono::DateTime;
use chrono::Utc;

/// A request being served, from the moment it arrived.
///
/// Like [`crate::ApiCall`], the point of the type is that a request cannot go
/// unaccounted for: one dropped without an answer says so rather than leaving
/// a line with nothing under it.
#[must_use = "a Served must be given an answer"]
pub struct Served {
    id: u64,
    /// The caller's `module_path!()`, printed where a target would be. Carried
    /// rather than read off the callsite because the callsite is in here.
    module: &'static str,
    /// Who asked, as far as the socket knows. A dev server is talked to by a
    /// simulator on loopback and a phone on the LAN at the same time, and this
    /// is what tells their lines apart.
    client: String,
    start: DateTime<Utc>,
    done: bool,
}

impl Served {
    /// Log a request arriving and start its clock.
    ///
    /// `module` is the caller's own `module_path!()`, which is what the line
    /// is printed under. `url` is the path and query as asked for, not the
    /// whole address: the host is this process, which the log already belongs
    /// to.
    pub fn received(
        module: &'static str,
        client: impl Display,
        method: impl Display,
        url: impl Display,
    ) -> Self {
        let id = crate::call::next_id();

        // Rendered once, and as `&str` so they reach the line the same way
        // every other field does.
        let client = client.to_string();
        let method = method.to_string();
        let url = url.to_string();

        tracing::info!(
            module,
            id,
            client = client.as_str(),
            method = method.as_str(),
            url = url.as_str(),
            "<-",
        );

        Self {
            id,
            module,
            client,
            start: crate::clock::instant(),
            done: false,
        }
    }

    /// The answer went back out. `bytes` is the length of the body where it is
    /// known before sending, which for this server is always.
    pub fn answered(mut self, status: u16, bytes: Option<u64>) {
        self.done = true;

        // Two calls rather than one, because `tracing` fixes an event's fields
        // at the callsite and a `bytes=0` on an answer whose length we never
        // knew reads as an empty body.
        match bytes {
            Some(bytes) => tracing::info!(
                module = self.module,
                id = self.id,
                client = self.client.as_str(),
                status,
                ms = crate::clock::millis_since(self.start),
                bytes,
                "->",
            ),
            None => tracing::info!(
                module = self.module,
                id = self.id,
                client = self.client.as_str(),
                status,
                ms = crate::clock::millis_since(self.start),
                "->",
            ),
        }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if self.done {
            return;
        }

        // Nothing went back: the client hung up mid-request, or the task
        // serving it went away. Ordinary enough to be DEBUG and the same
        // reading as a cancelled call, but a request line with no answer
        // under it would read like a hang.
        tracing::debug!(
            module = self.module,
            id = self.id,
            client = self.client.as_str(),
            ms = crate::clock::millis_since(self.start),
            "answered nothing",
        );
    }
}
