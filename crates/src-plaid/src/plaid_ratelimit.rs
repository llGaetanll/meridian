//! Plaid's rate limits, as a limiter the state machine can consult.
//!
//! Plaid meters **per endpoint, per Item**, and the endpoints this app uses do
//! not share a budget. The published limits (Sandbox and Production alike) are
//! per-minute, so the shape is one sliding window per endpoint per Item.
//!
//! No IO here, and no clock: `can_send` is arithmetic over what we have already
//! sent, which is what lets it live inside a state machine. See
//! `api_common::ratelimit::RateLimiter`.

use std::collections::HashMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use api_common::ratelimit::Window;

/// The endpoints we call, and what each is metered at.
///
/// Plaid documents these per Item per minute. They are deliberately a little
/// under the published numbers: being throttled costs a user-visible failure,
/// and the headroom costs nothing at this scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaidEndpoint {
    /// `/transactions/sync` — 30/min per Item.
    TransactionsSync,
    /// `/accounts/balance/get` — the strict one. Plaid meters it hard because
    /// it goes to the bank rather than to a cache.
    BalanceGet,
    /// `/transactions/recurring/get`.
    RecurringGet,
}

impl PlaidEndpoint {
    /// Calls allowed per minute, per Item.
    fn per_minute(&self) -> u32 {
        match self {
            PlaidEndpoint::TransactionsSync => 25,
            // 15/min published; 10 leaves room for a retry without tipping over.
            PlaidEndpoint::BalanceGet => 10,
            PlaidEndpoint::RecurringGet => 10,
        }
    }
}

/// What the limiter needs in order to answer.
#[derive(Debug, Clone)]
pub struct PlaidRateCtx {
    pub endpoint: PlaidEndpoint,

    /// Which Item the call is for. Limits are per-Item, so two banks do not
    /// share a budget.
    pub item_id: String,
}

impl PlaidRateCtx {
    pub fn new(endpoint: PlaidEndpoint, item_id: impl Into<String>) -> Self {
        Self {
            endpoint,
            item_id: item_id.into(),
        }
    }
}

/// One sliding window per endpoint per Item.
#[derive(Debug, Default)]
pub struct PlaidRateLimiter {
    windows: HashMap<(PlaidEndpoint, String), Window>,
}

impl PlaidRateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop windows with nothing left in them.
    ///
    /// The keys are Items, so without this the map holds a window for every
    /// bank the process has ever fetched. An idle window and one that was
    /// never opened answer every question the same way, which is what makes
    /// dropping one free — see `Window::is_idle`.
    pub fn sweep(&mut self, now: Timestamp) {
        self.windows.retain(|_, window| !window.is_idle(now));
    }

    /// How many windows are being kept, one per endpoint per Item.
    pub fn kept(&self) -> usize {
        self.windows.len()
    }

    fn window_for(&mut self, ctx: &PlaidRateCtx) -> &mut Window {
        self.windows
            .entry((ctx.endpoint, ctx.item_id.clone()))
            .or_insert_with(|| Window::new(ctx.endpoint.per_minute(), 60))
    }
}

impl RateLimiter for PlaidRateLimiter {
    type Ctx = PlaidRateCtx;

    fn can_send(&mut self, now: Timestamp, ctx: &PlaidRateCtx) -> bool {
        self.window_for(ctx).take(now)
    }

    fn next_allowed(&self, now: Timestamp, ctx: &PlaidRateCtx) -> Option<Timestamp> {
        self.windows
            .get(&(ctx.endpoint, ctx.item_id.clone()))
            .and_then(|window| window.next_allowed(now))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const T0: Timestamp = Timestamp::from_secs(1_000);

    fn ctx(endpoint: PlaidEndpoint) -> PlaidRateCtx {
        PlaidRateCtx::new(endpoint, "item-1")
    }

    #[test]
    fn it_allows_calls_up_to_the_endpoint_limit() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = ctx(PlaidEndpoint::BalanceGet);

        for i in 0..10 {
            assert!(limiter.can_send(T0, &ctx), "call {i} should be allowed");
        }

        assert!(!limiter.can_send(T0, &ctx), "the 11th is over the limit");
    }

    /// The whole reason the context carries an endpoint: a burst of syncs must
    /// not use up the balance budget.
    #[test]
    fn endpoints_have_separate_budgets() {
        let mut limiter = PlaidRateLimiter::new();

        for _ in 0..10 {
            limiter.can_send(T0, &ctx(PlaidEndpoint::BalanceGet));
        }

        assert!(!limiter.can_send(T0, &ctx(PlaidEndpoint::BalanceGet)));
        assert!(
            limiter.can_send(T0, &ctx(PlaidEndpoint::TransactionsSync)),
            "a different endpoint has its own window",
        );
    }

    /// Plaid meters per Item, so two banks do not share a budget.
    #[test]
    fn items_have_separate_budgets() {
        let mut limiter = PlaidRateLimiter::new();

        let first = PlaidRateCtx::new(PlaidEndpoint::BalanceGet, "item-1");
        let second = PlaidRateCtx::new(PlaidEndpoint::BalanceGet, "item-2");

        for _ in 0..10 {
            limiter.can_send(T0, &first);
        }

        assert!(!limiter.can_send(T0, &first));
        assert!(limiter.can_send(T0, &second));
    }

    #[test]
    fn the_budget_refills_over_a_minute() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = ctx(PlaidEndpoint::BalanceGet);

        for _ in 0..10 {
            limiter.can_send(T0, &ctx);
        }
        assert!(!limiter.can_send(T0, &ctx));

        let later = T0 + Duration::from_secs(61);
        assert!(limiter.can_send(later, &ctx));
    }

    /// Without this the map holds three windows for every bank the process
    /// has ever fetched, and gives none of them back.
    #[test]
    fn the_sweep_drops_windows_nothing_is_left_in() {
        let mut limiter = PlaidRateLimiter::new();

        for i in 0..50 {
            let ctx = PlaidRateCtx::new(PlaidEndpoint::BalanceGet, format!("item-{i}"));

            limiter.can_send(T0, &ctx);
        }

        assert_eq!(limiter.kept(), 50);

        limiter.sweep(T0 + Duration::from_secs(1));

        assert_eq!(limiter.kept(), 50, "nothing has aged out yet");

        limiter.sweep(T0 + Duration::from_secs(61));

        assert_eq!(limiter.kept(), 0, "everything has aged out");
    }

    #[test]
    fn it_says_when_the_next_call_is_allowed() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = ctx(PlaidEndpoint::BalanceGet);

        for _ in 0..10 {
            limiter.can_send(T0, &ctx);
        }

        assert_eq!(
            limiter.next_allowed(T0, &ctx),
            Some(T0 + Duration::from_secs(60)),
        );
    }
}
