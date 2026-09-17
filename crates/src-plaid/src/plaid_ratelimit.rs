//! Every request we send Plaid, counted before it goes out.
//!
//! One variant per endpoint this app calls, and a budget for each. Plaid meters
//! per endpoint, per Item, per minute, and the endpoints this app uses do not
//! share a budget, so the shape is one sliding window per endpoint per Item.
//!
//! No IO here, and no clock: `can_send` is arithmetic over what we have already
//! sent, which is what lets it live inside a state machine. See
//! `api_common::ratelimit::RateLimiter`.

use std::collections::BTreeMap;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use api_common::ratelimit::Window;

/// The endpoints we call, and what each is metered at.
///
/// Plaid documents these per Item per minute. They are deliberately a little
/// under the published numbers: being throttled costs a user-visible failure,
/// and the headroom costs nothing at this scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlaidEndpoint {
    /// `/transactions/sync` — 30/min per Item.
    TransactionsSync,
    /// `/accounts/get` — accounts and cached balances. The free one; the app
    /// deliberately never calls `/accounts/balance/get`, which bills per call.
    AccountsGet,
    /// `/transactions/recurring/get`.
    RecurringGet,
    /// `/link/token/create` — the page somebody signs in to their bank on.
    LinkTokenCreate,
    /// `/link/token/get` — what a session came to, where Plaid does not call
    /// us. One request per sheet closed.
    LinkSessionGet,
    /// `/item/public_token/exchange` — the irreversible one. Each request that
    /// lands creates an Item, and the account may hold ten.
    PublicTokenExchange,
    /// `/webhook_verification_key/get` — the only request an unauthenticated
    /// POST can cause, which is why it is metered rather than merely cached.
    WebhookKeyGet,

    /// `/item/remove`
    ItemRemove,
}

impl PlaidEndpoint {
    /// Requests allowed per minute, per Item.
    pub fn per_minute(&self) -> u32 {
        match self {
            PlaidEndpoint::TransactionsSync => 25,
            // 15/min published; 10 leaves room for a retry without tipping over.
            PlaidEndpoint::AccountsGet => 10,
            PlaidEndpoint::RecurringGet => 10,
            // Both are one request per sheet, and a sheet is a person doing
            // something. Far more than that is something in a loop.
            PlaidEndpoint::LinkTokenCreate => 30,
            PlaidEndpoint::LinkSessionGet => 30,
            // One per bank attached, and somebody connects one or two in a
            // sitting. The lowest budget here, because it is the request
            // nothing can undo.
            PlaidEndpoint::PublicTokenExchange => 5,
            // Cached per `kid`, hits and misses both, so a minute that needs
            // ten of these is one where Plaid rotated every key it has.
            PlaidEndpoint::WebhookKeyGet => 10,
            // One per bank, on an account being deleted, and somebody has one
            // or two. Deliberately not stingy: a removal that a limit refuses
            // is a bank we keep paying for.
            PlaidEndpoint::ItemRemove => 30,
        }
    }
}

/// What the limiter needs in order to answer.
#[derive(Debug, Clone)]
pub struct PlaidRateCtx {
    pub endpoint: PlaidEndpoint,

    /// Which Item the request is about, where there is one. Limits are
    /// per-Item, so two banks do not share a budget. `None` is a request that
    /// names no bank, counted for the account as a whole.
    pub item_id: Option<String>,
}

impl PlaidRateCtx {
    /// A request about one Item.
    pub fn new(endpoint: PlaidEndpoint, item_id: impl Into<String>) -> Self {
        Self {
            endpoint,
            item_id: Some(item_id.into()),
        }
    }

    /// A request that names no Item, and so is counted for the whole account.
    pub fn account(endpoint: PlaidEndpoint) -> Self {
        Self {
            endpoint,
            item_id: None,
        }
    }
}

/// One sliding window per endpoint per Item.
#[derive(Debug, Default)]
pub struct PlaidRateLimiter {
    windows: BTreeMap<(PlaidEndpoint, Option<String>), Window>,
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
        let ctx = ctx(PlaidEndpoint::AccountsGet);

        for i in 0..10 {
            assert!(limiter.can_send(T0, &ctx), "call {i} should be allowed");
        }

        assert!(!limiter.can_send(T0, &ctx), "the 11th is over the limit");
    }

    /// The whole reason the context carries an endpoint: a burst of syncs must
    /// not use up the accounts budget.
    #[test]
    fn endpoints_have_separate_budgets() {
        let mut limiter = PlaidRateLimiter::new();

        for _ in 0..10 {
            limiter.can_send(T0, &ctx(PlaidEndpoint::AccountsGet));
        }

        assert!(!limiter.can_send(T0, &ctx(PlaidEndpoint::AccountsGet)));
        assert!(
            limiter.can_send(T0, &ctx(PlaidEndpoint::TransactionsSync)),
            "a different endpoint has its own window",
        );
    }

    /// Plaid meters per Item, so two banks do not share a budget.
    #[test]
    fn items_have_separate_budgets() {
        let mut limiter = PlaidRateLimiter::new();

        let first = PlaidRateCtx::new(PlaidEndpoint::AccountsGet, "item-1");
        let second = PlaidRateCtx::new(PlaidEndpoint::AccountsGet, "item-2");

        for _ in 0..10 {
            limiter.can_send(T0, &first);
        }

        assert!(!limiter.can_send(T0, &first));
        assert!(limiter.can_send(T0, &second));
    }

    #[test]
    fn the_budget_refills_over_a_minute() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = ctx(PlaidEndpoint::AccountsGet);

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
            let ctx = PlaidRateCtx::new(PlaidEndpoint::AccountsGet, format!("item-{i}"));

            limiter.can_send(T0, &ctx);
        }

        assert_eq!(limiter.kept(), 50);

        limiter.sweep(T0 + Duration::from_secs(1));

        assert_eq!(limiter.kept(), 50, "nothing has aged out yet");

        limiter.sweep(T0 + Duration::from_secs(61));

        assert_eq!(limiter.kept(), 0, "everything has aged out");
    }

    /// The irreversible one is metered like the rest, and it is the tightest:
    /// each request that lands creates an Item the account cannot give back.
    #[test]
    fn the_exchange_is_the_narrowest_budget() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = PlaidRateCtx::account(PlaidEndpoint::PublicTokenExchange);

        for i in 0..PlaidEndpoint::PublicTokenExchange.per_minute() {
            assert!(limiter.can_send(T0, &ctx), "exchange {i} should be allowed");
        }

        assert!(!limiter.can_send(T0, &ctx), "the next is over the budget");

        assert!(
            PlaidEndpoint::PublicTokenExchange.per_minute()
                < PlaidEndpoint::LinkTokenCreate.per_minute(),
            "a sheet may be opened more often than a bank may be attached",
        );
    }

    #[test]
    fn it_says_when_the_next_call_is_allowed() {
        let mut limiter = PlaidRateLimiter::new();
        let ctx = ctx(PlaidEndpoint::AccountsGet);

        for _ in 0..10 {
            limiter.can_send(T0, &ctx);
        }

        assert_eq!(
            limiter.next_allowed(T0, &ctx),
            Some(T0 + Duration::from_secs(60)),
        );
    }
}
