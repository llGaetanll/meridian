//! Asking "may I send this?" without doing any IO.
//!
//! Every external API has limits, and they are never the same shape: Plaid
//! meters per endpoint per Item, Yahoo meters per IP, CoinGecko meters per
//! minute with a burst. The one thing they have in common is that deciding
//! whether a call is allowed is *arithmetic over what we have already sent* —
//! no clock of its own, no socket, no sleeping.
//!
//! That makes a rate limiter something the state machine can own. It is a rule
//! about when to act, which is exactly what a state machine is for, and
//! keeping it out means the machine either guesses or asks the runtime a
//! question the runtime cannot answer deterministically.
//!
//! Each API's limiter lives in that API's crate, because the limits are that
//! API's business — `src-plaid` knows Plaid's, and nothing else should have to.
//! What lives here is only the shape they share.

use std::time::Duration;

use crate::Timestamp;

/// Whether a call may be sent, and the bookkeeping that comes with saying yes.
pub trait RateLimiter {
    /// Whatever this API needs in order to answer. Often an endpoint, since
    /// limits are usually per-endpoint; sometimes an Item or an account too.
    type Ctx;

    /// May this call go out now?
    ///
    /// **Saying yes counts.** The caller is expected to send, so the limiter
    /// records the call as it answers — which is why this takes `&mut self`.
    /// A caller that asks and then does not send will under-use its budget,
    /// which is the safe direction to be wrong in.
    ///
    /// `now` is handed in rather than read, so the whole thing stays testable
    /// and replayable. See `docs/ARCHITECTURE.md`.
    fn can_send(&mut self, now: Timestamp, ctx: &Self::Ctx) -> bool;

    /// When the next call would be allowed, if one is not allowed now.
    ///
    /// Lets a caller schedule rather than poll. `None` means "now".
    fn next_allowed(&self, now: Timestamp, ctx: &Self::Ctx) -> Option<Timestamp>;

    /// How long to wait before the `attempt`-th retry, and whether to bother.
    ///
    /// `attempt` counts from zero, so the waits are 1s, 2s, 4s, 8s, 16s and
    /// then `None` — five tries, about half a minute in total. Exponential
    /// because a fixed delay against a rate limit is just a slower version of
    /// the same burst, and capped because something that has failed five times
    /// is not going to come good on the sixth.
    ///
    /// The default suits most APIs. An implementation can override it where the
    /// service documents something specific — a `Retry-After` header, say.
    fn backoff(&self, attempt: u32) -> Option<Duration> {
        if attempt >= Self::MAX_ATTEMPTS {
            return None;
        }

        Some(Duration::from_secs(1 << attempt))
    }

    /// How many times to retry before giving up.
    const MAX_ATTEMPTS: u32 = 5;
}

/// A fixed number of calls per window, the shape most APIs document.
///
/// A sliding window rather than a fixed one: a fixed window lets twice the
/// limit through across a boundary, which is exactly when a burst happens.
#[derive(Debug, Clone)]
pub struct Window {
    limit: u32,
    window_secs: u64,

    /// When each call in the current window went out, oldest first.
    sent: Vec<Timestamp>,
}

impl Window {
    pub fn new(limit: u32, window_secs: u64) -> Self {
        Self {
            limit,
            window_secs,
            sent: Vec::new(),
        }
    }

    /// Drop everything that has aged out of the window.
    fn expire(&mut self, now: Timestamp) {
        let cutoff = now.saturating_sub_secs(self.window_secs);
        self.sent.retain(|ts| *ts > cutoff);
    }

    /// Whether there is room, counting this instant.
    pub fn allows(&mut self, now: Timestamp) -> bool {
        self.expire(now);
        (self.sent.len() as u32) < self.limit
    }

    /// Record a call. Only call this having decided to send one.
    pub fn record(&mut self, now: Timestamp) {
        self.sent.push(now);
    }

    /// Convenience: check and record in one step.
    pub fn take(&mut self, now: Timestamp) -> bool {
        if self.allows(now) {
            self.record(now);
            true
        } else {
            false
        }
    }

    /// When the oldest call ages out, which is the soonest there is room.
    pub fn next_allowed(&self, now: Timestamp) -> Option<Timestamp> {
        if (self.sent.len() as u32) < self.limit {
            return None;
        }

        self.sent
            .first()
            .map(|oldest| oldest.saturating_add_secs(self.window_secs))
            .filter(|at| *at > now)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const T0: Timestamp = Timestamp::from_secs(1_000);

    #[test]
    fn it_allows_up_to_the_limit() {
        let mut window = Window::new(3, 60);

        assert!(window.take(T0));
        assert!(window.take(T0));
        assert!(window.take(T0));
        assert!(!window.take(T0), "the fourth is over the limit");
    }

    #[test]
    fn room_opens_as_calls_age_out() {
        let mut window = Window::new(2, 60);

        assert!(window.take(T0));
        assert!(window.take(T0));
        assert!(!window.take(T0));

        // A minute later the first two have aged out.
        let later = T0 + Duration::from_secs(61);

        assert!(window.take(later));
    }

    /// A fixed window would let 4 through across the boundary here; a sliding
    /// one lets 2, which is what the limit actually means.
    #[test]
    fn the_window_slides_rather_than_resetting() {
        let mut window = Window::new(2, 60);

        assert!(window.take(T0 + Duration::from_secs(50)));
        assert!(window.take(T0 + Duration::from_secs(59)));

        // One second later a fixed window would have reset. This one has not.
        assert!(!window.take(T0 + Duration::from_secs(60)));
    }

    /// A stand-in limiter, so the default `backoff` can be exercised without
    /// dragging a real API's rules in.
    struct Bare;

    impl RateLimiter for Bare {
        type Ctx = ();

        fn can_send(&mut self, _now: Timestamp, _ctx: &()) -> bool {
            true
        }

        fn next_allowed(&self, _now: Timestamp, _ctx: &()) -> Option<Timestamp> {
            None
        }
    }

    #[test]
    fn backoff_doubles_and_then_gives_up() {
        let waits: Vec<Option<u64>> = (0..7)
            .map(|attempt| Bare.backoff(attempt).map(|d| d.as_secs()))
            .collect();

        assert_eq!(
            waits,
            [Some(1), Some(2), Some(4), Some(8), Some(16), None, None,],
            "five tries, doubling, then stop",
        );
    }

    #[test]
    fn it_says_when_there_will_be_room() {
        let mut window = Window::new(1, 60);

        assert_eq!(window.next_allowed(T0), None, "there is room now");

        window.record(T0);

        assert_eq!(window.next_allowed(T0), Some(T0 + Duration::from_secs(60)),);
    }
}
