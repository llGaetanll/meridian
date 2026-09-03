//! Whether a POST on the webhook route is really Plaid.
//!
//! The twin of [`RuntimeServer::authenticate`](crate::RuntimeServer): one
//! turns a bearer token into a user, this turns a body and a header into a
//! webhook worth acting on. Both are the runtime answering *who is this*, and
//! neither decides anything about what to do next — that is the machine's, on
//! the event that comes out the far side.
//!
//! Its own file because it owns something: the key cache. This is the first
//! unauthenticated route in the server that does work, and the cache is what
//! keeps an anonymous POST from becoming an outbound call to Plaid.

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::RwLockWriteGuard;

use api_common::Timestamp;
use axum::http::HeaderMap;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidJwk;
use src_plaid::PlaidRejection;
use src_plaid::PlaidWebhook;
use src_plaid::PlaidWebhookJwt;
use src_plaid::VERIFICATION_HEADER;

use crate::runtime_plaid;

/// How long a `kid` Plaid would not answer for is remembered as a miss.
///
/// Short, because the two reasons for a miss want opposite things. A `kid`
/// somebody invented should cost one call to Plaid however often they send it;
/// a real key we failed to fetch — Plaid briefly down — must not be refused
/// for the rest of this process's life. Five minutes serves both.
const MISS_GOOD_FOR_SECS: u64 = 5 * 60;

/// How many signing keys are remembered at once.
///
/// Generous next to the handful Plaid rotates through, and finite because the
/// ids come off tokens strangers send. Full means the misses are swept, and if
/// it is still full a webhook naming an unknown key is refused rather than
/// allowed to cause a call — which is the whole point of the cache, so it is
/// also what it does when it cannot hold any more.
const KEYS_AT_MOST: usize = 64;

/// A key, or the fact that there was not one.
///
/// The miss is the half that matters. Without it, a `kid` nobody has ever seen
/// is a Plaid request per POST, which is a denial of service anyone can point
/// at us — and a hit that is never remembered is the same thing at a slower
/// rate.
struct CachedKey {
    /// `None` is a miss: Plaid did not answer for this id.
    key: Option<PlaidJwk>,

    /// When it was fetched. Only a miss expires; a key for a given `kid` is
    /// the same key forever, because rotation gives the new one a new id.
    at: Timestamp,
}

#[derive(Default)]
pub(crate) struct WebhookVerifier {
    keys: RwLock<HashMap<String, CachedKey>>,
}

impl WebhookVerifier {
    /// The webhook this request is, or why it is not one.
    ///
    /// `body` is the bytes as they arrived and must stay that way all the way
    /// in. Plaid hashes what it put on the wire — pretty-printed, two spaces —
    /// so a body that has been through a JSON extractor and printed again
    /// hashes to something else, and every webhook is refused for a reason
    /// that reads like an attack.
    pub(crate) async fn believe(
        &self,
        credentials: &PlaidCredentials,
        headers: &HeaderMap,
        body: &[u8],
        now: Timestamp,
    ) -> Result<PlaidWebhook, PlaidRejection> {
        let token = headers
            .get(VERIFICATION_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or(PlaidRejection::Unsigned)?;

        // Everything free happens before anything expensive: this rejects the
        // wrong algorithm, which is what stops a made-up `kid` reaching the
        // fetch below.
        let jwt = PlaidWebhookJwt::parse(token)?;

        let key = self
            .key(credentials, jwt.kid(), now)
            .await
            .ok_or(PlaidRejection::Unusable)?;

        jwt.verify(&key, body, now.as_secs() as i64)?;

        // Believed, and only now parsed. A body read before the hash was
        // checked is a body a stranger wrote.
        serde_json::from_slice(body).map_err(|e| {
            tracing::error!(error = %e, "plaid signed a body that is not JSON");

            PlaidRejection::Malformed("body is not JSON")
        })
    }

    /// The key for one `kid`, from the cache or from Plaid.
    ///
    /// The one place in the server where an unauthenticated request can cause
    /// an outbound call, and everything about it is arranged so that it costs
    /// at most one per id: hits are kept for the life of the process, misses
    /// for [`MISS_GOOD_FOR_SECS`], and a cache with no room left refuses
    /// rather than calling.
    async fn key(
        &self,
        credentials: &PlaidCredentials,
        kid: &str,
        now: Timestamp,
    ) -> Option<PlaidJwk> {
        if let Some(known) = self.known(kid, now) {
            return known;
        }

        // Checked before the call rather than after it, because the point is
        // the call. No lock is held across the await below.
        if !self.has_room(now) {
            tracing::error!(
                kid,
                "refusing to fetch another webhook key: too many have been asked for",
            );

            return None;
        }

        let fetched = match runtime_plaid::webhook_key(credentials, kid).await {
            Ok(key) => Some(key),

            Err(e) => {
                tracing::warn!(kid, error = %e, "could not fetch a webhook key");

                None
            }
        };

        self.write_keys().insert(
            kid.to_string(),
            CachedKey {
                key: fetched.clone(),
                at: now,
            },
        );

        fetched
    }

    /// What is already known about this `kid`.
    ///
    /// Three answers, not two: `Some(Some(key))` is a hit, `Some(None)` is a
    /// miss still standing, and `None` is nothing known — which is the only
    /// one that goes on to ask Plaid.
    fn known(&self, kid: &str, now: Timestamp) -> Option<Option<PlaidJwk>> {
        let keys = match self.keys.read() {
            Ok(keys) => keys,

            Err(poisoned) => {
                tracing::error!(
                    "the webhook key cache lock is poisoned — a thread panicked while holding \
                     it; carrying on with the held data",
                );

                poisoned.into_inner()
            }
        };

        let cached = keys.get(kid)?;

        match &cached.key {
            Some(key) => Some(Some(key.clone())),

            None if now.since(cached.at).as_secs() < MISS_GOOD_FOR_SECS => Some(None),

            // The miss has expired, so this id is worth one more call.
            None => None,
        }
    }

    /// Whether there is room for another key, having first thrown out the
    /// misses that have expired.
    fn has_room(&self, now: Timestamp) -> bool {
        let mut keys = self.write_keys();

        if keys.len() < KEYS_AT_MOST {
            return true;
        }

        keys.retain(|_, cached| {
            cached.key.is_some() || now.since(cached.at).as_secs() < MISS_GOOD_FOR_SECS
        });

        keys.len() < KEYS_AT_MOST
    }

    fn write_keys(&self) -> RwLockWriteGuard<'_, HashMap<String, CachedKey>> {
        match self.keys.write() {
            Ok(keys) => keys,

            Err(poisoned) => {
                tracing::error!(
                    "the webhook key cache lock is poisoned — a thread panicked while holding \
                     it; carrying on with the held data",
                );

                poisoned.into_inner()
            }
        }
    }
}
