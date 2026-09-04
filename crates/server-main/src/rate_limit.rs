//! What this server will answer, and how often.
//!
//! The other half of `src_plaid::PlaidRateLimiter`: that one is what *we* may
//! send, this is what we will be asked. Same shape and the same reason for it
//! — deciding is arithmetic over what has already happened, with no clock and
//! no IO, so it lives in the machine where an `eval!` test can reach it. See
//! `api_common::ratelimit`.
//!
//! A request is counted against every window it falls in, and has to be under
//! all of them. Two come from the halves of [`Client`](crate::Client):
//! counting only the address hands a botnet the whole server, and counting
//! only the session hands it to anyone willing to sign up twice. The third is
//! the inbox a sign-in code would go to, which is not a fact about the caller
//! at all — see [`ServerRateCtx::mailing`].

use std::collections::HashMap;
use std::hash::Hash;

use api_common::Timestamp;
use api_common::ratelimit::RateLimiter;
use api_common::ratelimit::Window;
use uuid::Uuid;

use crate::Client;
use crate::ClientIp;

/// Calls a minute on the route the phone polls.
///
/// An order of magnitude above everything else, because the phone asks
/// `/sync/status` once a second while a first run is going (#41) — so sixty a
/// minute is the *normal* rate and a limit anywhere near it would break the
/// one flow that has to work. Double that leaves room for a retry and a second
/// device.
///
/// It can afford to be this loose: the answer is a read of one map inside the
/// machine, with no store and no bank behind it. The polling is the thing
/// worth fixing, and #41 is where.
const POLLED_PER_MINUTE: u32 = 120;

/// Calls a minute on every other route.
///
/// Twenty is far above what the app does and far below what a loop does.
/// `/snapshot` is the expensive one — it reads a whole ledger out of Postgres
/// and projects it — but it is already damped by `FRESH_FOR_SECS`, so this
/// number is about the read rather than about Plaid.
const CALLS_PER_MINUTE: u32 = 20;

/// Sign-in emails per caller per quarter hour.
///
/// The only route that spends money and puts something in a stranger's inbox,
/// and the only one with no damping of its own — `/snapshot` at least serves a
/// cache. Issue #23 opens on it for a reason: unlimited, it is a mailbomb
/// aimed at any address the caller likes, from us.
///
/// The window matters more than the count here. Five a minute would still be
/// three hundred an hour at one address; five a quarter hour is twenty, which
/// is more than anyone signing in legitimately will ever need.
const EMAILS_PER_QUARTER_HOUR: u32 = 5;

/// A minute, in seconds.
const MINUTE: u64 = 60;

/// A quarter of an hour, in seconds.
const QUARTER_HOUR: u64 = 15 * 60;

/// What a caller is metered on.
///
/// One variant per route that dispatches, so the bucket is derived from the
/// event rather than named by the handler — see
/// [`EventRouter::route`](crate::EventRouter::route). A route that grows a new
/// variant cannot forget to be limited, because it will not compile until it
/// says which of these it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// `GET /snapshot`
    Snapshot,

    /// `POST /auth/request` — the one that sends an email.
    AuthRequest,

    /// `POST /auth/verify`
    AuthVerify,

    /// `POST /link/start`
    LinkStart,

    /// `POST /link/finish`
    LinkFinish,

    /// `GET /sync/status` — the one the phone polls.
    SyncStatus,
}

impl Route {
    /// What the logs call it, which is the path a caller asked for.
    ///
    /// The path rather than the variant name, because the line is read next to
    /// the `url=` on the request it refused and the two should say the same
    /// thing.
    pub fn as_str(self) -> &'static str {
        match self {
            Route::Snapshot => "/snapshot",
            Route::AuthRequest => "/auth/request",
            Route::AuthVerify => "/auth/verify",
            Route::LinkStart => "/link/start",
            Route::LinkFinish => "/link/finish",
            Route::SyncStatus => "/sync/status",
        }
    }

    /// How much of it, over how long.
    fn limit(self) -> (u32, u64) {
        match self {
            Route::SyncStatus => (POLLED_PER_MINUTE, MINUTE),
            Route::AuthRequest => (EMAILS_PER_QUARTER_HOUR, QUARTER_HOUR),

            Route::Snapshot | Route::AuthVerify | Route::LinkStart | Route::LinkFinish => {
                (CALLS_PER_MINUTE, MINUTE)
            }
        }
    }
}

/// What the limiter needs in order to answer: which route, who is asking, and
/// who would be written to.
#[derive(Debug, Clone)]
pub struct ServerRateCtx {
    pub route: Route,
    pub client: Client,

    /// The inbox a sign-in code would land in, on the one route that sends
    /// one. Already reduced to the inbox rather than the address written —
    /// see [`mailing`](Self::mailing).
    inbox: Option<String>,
}

impl ServerRateCtx {
    pub fn new(route: Route, client: Client) -> Self {
        Self {
            route,
            client,
            inbox: None,
        }
    }

    /// Also count this against the inbox a code would be sent to.
    ///
    /// The one axis here that is not a fact about the caller, and #23 wants it
    /// for a reason the other two cannot cover: a botnet is a thousand
    /// addresses and one victim, so every address window has room while the
    /// same inbox takes every message. What is metered is the mail we send,
    /// and mail is addressed to a person.
    ///
    /// `None` on every route that sends nothing, which is all of them but one.
    pub fn mailing(mut self, email: Option<&str>) -> Self {
        self.inbox = email.map(inbox_of);
        self
    }
}

/// Which inbox an address delivers to, as far as can be told from outside.
///
/// Lowercased, and a `+tag` dropped from the local part. Both are free for a
/// caller to vary and neither changes where the mail lands, so counting them
/// apart is the same bug as counting a v6 caller by their address: one
/// recipient, unlimited mail, a different hat each time.
///
/// It is a guess, and deliberately the cheap direction of one. A provider that
/// treats `+` as an ordinary character has two real addresses sharing one
/// budget of five per quarter hour; the other way round is a mailbomb. Only
/// the bucket is reduced — what gets stored and written to is the address as
/// the caller typed it.
fn inbox_of(email: &str) -> String {
    let email = email.trim().to_lowercase();

    let Some((local, domain)) = email.split_once('@') else {
        return email;
    };

    let local = local.split_once('+').map_or(local, |(before, _)| before);

    format!("{local}@{domain}")
}

/// One sliding window per route per caller, and a caller is two things.
///
/// Keyed separately rather than by a pair, because a request from an
/// authenticated caller has to be counted against their address *and* their
/// account, and the two windows have different lifetimes: an address stops
/// calling, an account signs in from somewhere else.
#[derive(Debug, Default)]
pub struct ServerRateLimiter {
    by_ip: HashMap<(Route, ClientIp), Window>,
    by_user: HashMap<(Route, Uuid), Window>,
    by_inbox: HashMap<(Route, String), Window>,
}

impl ServerRateLimiter {
    /// Drop windows with nothing left in them.
    ///
    /// The keys come from whoever calls us, so without this the maps grow for
    /// the life of the process and the rate limiter is itself the thing that
    /// exhausts the server. Called on the tick, which is the only turn that
    /// happens because nothing happened. See #40.
    pub fn sweep(&mut self, now: Timestamp) {
        self.by_ip.retain(|_, window| !window.is_idle(now));
        self.by_user.retain(|_, window| !window.is_idle(now));
        self.by_inbox.retain(|_, window| !window.is_idle(now));
    }

    /// How many windows are being kept, so a test can say the sweep works.
    #[cfg(test)]
    fn kept(&self) -> usize {
        self.by_ip.len() + self.by_user.len() + self.by_inbox.len()
    }
}

impl RateLimiter for ServerRateLimiter {
    type Ctx = ServerRateCtx;

    /// **Every window is asked before any of them is told.**
    ///
    /// The two-step is the point: recording as we go would spend a slot in the
    /// address window on a request the session window then refuses, so a
    /// caller who tripped one limit would slowly use up the others. Asking
    /// first means a refusal costs nothing.
    fn can_send(&mut self, now: Timestamp, ctx: &ServerRateCtx) -> bool {
        let (limit, window_secs) = ctx.route.limit();

        let key_ip = (ctx.route, ctx.client.ip.clone());
        let key_user = ctx.client.user.map(|user| (ctx.route, user));
        let key_inbox = ctx.inbox.clone().map(|inbox| (ctx.route, inbox));

        if !allows(&mut self.by_ip, key_ip.clone(), limit, window_secs, now) {
            return false;
        }

        if let Some(key) = key_user
            && !allows(&mut self.by_user, key, limit, window_secs, now)
        {
            return false;
        }

        if let Some(key) = key_inbox.clone()
            && !allows(&mut self.by_inbox, key, limit, window_secs, now)
        {
            return false;
        }

        // All of them said yes, so all of them are told. Reached through the
        // maps again rather than held across the checks above, because two
        // `&mut` into one map is not a thing, and the alternative is a lookup
        // that has already happened this turn.
        if let Some(window) = self.by_ip.get_mut(&key_ip) {
            window.record(now);
        }

        if let Some(window) = key_user.and_then(|key| self.by_user.get_mut(&key)) {
            window.record(now);
        }

        if let Some(window) = key_inbox.and_then(|key| self.by_inbox.get_mut(&key)) {
            window.record(now);
        }

        true
    }

    /// The soonest of the windows' answers.
    ///
    /// Implemented because the trait asks for it, and nothing here puts it on
    /// the wire: a 429 from this server says only that there were too many,
    /// because when the next one is allowed is a fact about our state that a
    /// caller behaving well does not need and one behaving badly should not be
    /// handed.
    fn next_allowed(&self, now: Timestamp, ctx: &ServerRateCtx) -> Option<Timestamp> {
        let by_ip = self
            .by_ip
            .get(&(ctx.route, ctx.client.ip.clone()))
            .and_then(|window| window.next_allowed(now));

        let by_user = ctx
            .client
            .user
            .and_then(|user| self.by_user.get(&(ctx.route, user)))
            .and_then(|window| window.next_allowed(now));

        let by_inbox = ctx
            .inbox
            .clone()
            .and_then(|inbox| self.by_inbox.get(&(ctx.route, inbox)))
            .and_then(|window| window.next_allowed(now));

        [by_ip, by_user, by_inbox].into_iter().flatten().min()
    }
}

/// Whether one window has room, opening it if this key has not called before.
fn allows<K: Eq + Hash>(
    windows: &mut HashMap<K, Window>,
    key: K,
    limit: u32,
    window_secs: u64,
    now: Timestamp,
) -> bool {
    windows
        .entry(key)
        .or_insert_with(|| Window::new(limit, window_secs))
        .allows(now)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const T0: Timestamp = Timestamp::from_secs(1_000);

    fn ip(address: &str) -> ClientIp {
        ClientIp::new(address)
    }

    fn anonymous(address: &str) -> Client {
        Client::anonymous(ip(address))
    }

    fn signed_in(address: &str, user: Uuid) -> Client {
        Client::new(ip(address), Some(user))
    }

    #[test]
    fn it_allows_calls_up_to_the_route_limit() {
        let mut limiter = ServerRateLimiter::default();
        let ctx = ServerRateCtx::new(Route::Snapshot, anonymous("203.0.113.7"));

        for i in 0..CALLS_PER_MINUTE {
            assert!(limiter.can_send(T0, &ctx), "call {i} should be allowed");
        }

        assert!(!limiter.can_send(T0, &ctx), "one past the limit");
    }

    #[test]
    fn routes_have_separate_budgets() {
        let mut limiter = ServerRateLimiter::default();
        let client = anonymous("203.0.113.7");

        for _ in 0..CALLS_PER_MINUTE {
            limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, client.clone()));
        }

        assert!(!limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, client.clone())));
        assert!(
            limiter.can_send(T0, &ServerRateCtx::new(Route::LinkStart, client)),
            "a different route has its own window",
        );
    }

    #[test]
    fn callers_have_separate_budgets() {
        let mut limiter = ServerRateLimiter::default();

        let first = ServerRateCtx::new(Route::Snapshot, anonymous("203.0.113.7"));
        let second = ServerRateCtx::new(Route::Snapshot, anonymous("198.51.100.4"));

        for _ in 0..CALLS_PER_MINUTE {
            limiter.can_send(T0, &first);
        }

        assert!(!limiter.can_send(T0, &first));
        assert!(limiter.can_send(T0, &second));
    }

    /// An IPv6 caller is not handed an address, they are handed a /64 — and
    /// every one of the eighteen quintillion addresses in it is free to pick,
    /// on a timer, by the operating system. Counting the address whole hands
    /// them a budget per request, on the one route that mails a stranger.
    #[test]
    fn one_ipv6_prefix_shares_a_budget() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..EMAILS_PER_QUARTER_HOUR {
            let client = anonymous(&format!("2001:db8:1234:5678::{i}"));

            assert!(limiter.can_send(T0, &ServerRateCtx::new(Route::AuthRequest, client)));
        }

        let rotated = anonymous("2001:db8:1234:5678:aaaa:bbbb:cccc:dddd");

        assert!(
            !limiter.can_send(T0, &ServerRateCtx::new(Route::AuthRequest, rotated)),
            "another address out of the same /64 is the same caller",
        );
    }

    /// The prefix is where it stops. Two /64s are two allocations and two
    /// callers, and sharing a budget between them lets either lock the other
    /// out.
    #[test]
    fn separate_ipv6_prefixes_have_separate_budgets() {
        let mut limiter = ServerRateLimiter::default();

        for _ in 0..EMAILS_PER_QUARTER_HOUR {
            let client = anonymous("2001:db8:1234:5678::1");

            limiter.can_send(T0, &ServerRateCtx::new(Route::AuthRequest, client));
        }

        let elsewhere = anonymous("2001:db8:1234:9999::1");

        assert!(
            limiter.can_send(T0, &ServerRateCtx::new(Route::AuthRequest, elsewhere)),
            "a different prefix is a different caller",
        );
    }

    /// The whole reason a session is counted as well as an address: spreading
    /// the same session across a thousand machines must not buy a thousand
    /// budgets.
    #[test]
    fn one_session_across_many_addresses_shares_a_budget() {
        let mut limiter = ServerRateLimiter::default();
        let user = Uuid::from_u128(1);

        // Every call from its own address, so no address window is ever near
        // its limit.
        for i in 0..CALLS_PER_MINUTE {
            let client = signed_in(&format!("203.0.113.{i}"), user);

            assert!(
                limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, client)),
                "call {i} is under both limits",
            );
        }

        let fresh = signed_in("198.51.100.4", user);

        assert!(
            !limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, fresh)),
            "a new address does not buy the session a new budget",
        );
    }

    /// The other direction: one machine signing up repeatedly must not buy
    /// itself a budget per account.
    #[test]
    fn many_sessions_from_one_address_share_a_budget() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..CALLS_PER_MINUTE {
            let client = signed_in("203.0.113.7", Uuid::from_u128(u128::from(i)));

            assert!(limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, client)));
        }

        let fresh = signed_in("203.0.113.7", Uuid::from_u128(999));

        assert!(!limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, fresh)));
    }

    /// A refusal must cost nothing, or a caller who trips one limit slowly
    /// spends the other.
    #[test]
    fn a_refused_call_spends_no_budget() {
        let mut limiter = ServerRateLimiter::default();
        let user = Uuid::from_u128(1);

        // Fill this address's window with calls signed in as somebody else, so
        // the address is at its limit and the user has spent nothing.
        for i in 0..CALLS_PER_MINUTE {
            let other = signed_in("203.0.113.7", Uuid::from_u128(u128::from(i) + 100));

            limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, other));
        }

        let blocked = signed_in("203.0.113.7", user);

        assert!(!limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, blocked)));

        // Same user, an address with room. Their own window must be untouched
        // by the refusals above.
        for i in 0..CALLS_PER_MINUTE {
            let elsewhere = signed_in("198.51.100.4", user);

            assert!(
                limiter.can_send(T0, &ServerRateCtx::new(Route::Snapshot, elsewhere)),
                "call {i} — the refusals cost this session nothing",
            );
        }
    }

    /// A botnet is a thousand addresses and one victim. Every address window
    /// has room, so the only thing between one inbox and a thousand sign-in
    /// codes is a window on the inbox itself.
    #[test]
    fn one_inbox_shares_a_budget_across_callers() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..EMAILS_PER_QUARTER_HOUR {
            let client = anonymous(&format!("203.0.113.{i}"));
            let ctx =
                ServerRateCtx::new(Route::AuthRequest, client).mailing(Some("al@example.com"));

            assert!(limiter.can_send(T0, &ctx), "code {i} from its own address");
        }

        let client = anonymous("198.51.100.4");
        let ctx = ServerRateCtx::new(Route::AuthRequest, client).mailing(Some("al@example.com"));

        assert!(
            !limiter.can_send(T0, &ctx),
            "a fresh address does not buy a fresh inbox",
        );
    }

    /// The other direction, or one person signing in would silence the next.
    #[test]
    fn separate_inboxes_have_separate_budgets() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..EMAILS_PER_QUARTER_HOUR {
            let client = anonymous(&format!("203.0.113.{i}"));
            let ctx =
                ServerRateCtx::new(Route::AuthRequest, client).mailing(Some("al@example.com"));

            limiter.can_send(T0, &ctx);
        }

        let client = anonymous("198.51.100.4");
        let ctx = ServerRateCtx::new(Route::AuthRequest, client).mailing(Some("sam@example.com"));

        assert!(
            limiter.can_send(T0, &ctx),
            "somebody else's inbox is untouched"
        );
    }

    /// A tag is free to add, so counting tagged addresses apart is the /64
    /// bug with an `@` in it: one inbox, unlimited mail.
    #[test]
    fn a_tagged_address_is_the_same_inbox() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..EMAILS_PER_QUARTER_HOUR {
            let ctx = ServerRateCtx::new(Route::AuthRequest, anonymous("203.0.113.7"))
                .mailing(Some(&format!("al+{i}@example.com")));

            limiter.can_send(T0, &ctx);
        }

        let ctx = ServerRateCtx::new(Route::AuthRequest, anonymous("198.51.100.4"))
            .mailing(Some("AL@example.com"));

        assert!(
            !limiter.can_send(T0, &ctx),
            "a tag and a capital letter are the same inbox",
        );
    }

    /// A refusal on the inbox must not spend the caller's own budget either.
    #[test]
    fn a_refused_email_spends_no_address_budget() {
        let mut limiter = ServerRateLimiter::default();

        for i in 0..EMAILS_PER_QUARTER_HOUR {
            let client = anonymous(&format!("203.0.113.{i}"));
            let ctx =
                ServerRateCtx::new(Route::AuthRequest, client).mailing(Some("al@example.com"));

            limiter.can_send(T0, &ctx);
        }

        let flooder = anonymous("198.51.100.4");

        for _ in 0..EMAILS_PER_QUARTER_HOUR {
            let ctx = ServerRateCtx::new(Route::AuthRequest, flooder.clone())
                .mailing(Some("al@example.com"));

            assert!(!limiter.can_send(T0, &ctx));
        }

        let ctx = ServerRateCtx::new(Route::AuthRequest, flooder).mailing(Some("sam@example.com"));

        assert!(
            limiter.can_send(T0, &ctx),
            "the refusals above cost this address nothing",
        );
    }

    #[test]
    fn the_email_route_is_metered_over_a_quarter_hour() {
        let mut limiter = ServerRateLimiter::default();
        let ctx = ServerRateCtx::new(Route::AuthRequest, anonymous("203.0.113.7"));

        for _ in 0..EMAILS_PER_QUARTER_HOUR {
            assert!(limiter.can_send(T0, &ctx));
        }

        assert!(!limiter.can_send(T0, &ctx));

        // A minute later a per-minute window would have opened. This one is
        // still shut, which is the whole point of it.
        assert!(!limiter.can_send(T0 + Duration::from_secs(MINUTE + 1), &ctx));
        assert!(limiter.can_send(T0 + Duration::from_secs(QUARTER_HOUR + 1), &ctx));
    }

    /// The phone's own polling rate must be comfortably inside the limit, or
    /// first run breaks. See #41.
    #[test]
    fn a_minute_of_the_phone_polling_is_under_the_limit() {
        let mut limiter = ServerRateLimiter::default();
        let user = Uuid::from_u128(1);

        // One a second for a minute, which is what `polls_on_a_tick` does.
        for second in 0..60 {
            let client = signed_in("203.0.113.7", user);
            let ctx = ServerRateCtx::new(Route::SyncStatus, client);

            assert!(
                limiter.can_send(T0 + Duration::from_secs(second), &ctx),
                "second {second} of an ordinary first run",
            );
        }
    }

    #[test]
    fn the_budget_refills_over_the_window() {
        let mut limiter = ServerRateLimiter::default();
        let ctx = ServerRateCtx::new(Route::Snapshot, anonymous("203.0.113.7"));

        for _ in 0..CALLS_PER_MINUTE {
            limiter.can_send(T0, &ctx);
        }
        assert!(!limiter.can_send(T0, &ctx));

        assert!(limiter.can_send(T0 + Duration::from_secs(MINUTE + 1), &ctx));
    }

    /// Without this the maps grow for the life of the process, keyed by
    /// whatever addresses call us. See #40.
    #[test]
    fn the_sweep_drops_windows_nothing_is_left_in() {
        let mut limiter = ServerRateLimiter::default();
        let user = Uuid::from_u128(1);

        for i in 0..50 {
            let client = signed_in(&format!("203.0.113.{i}"), user);

            limiter.can_send(T0, &ServerRateCtx::new(Route::SyncStatus, client));
        }

        assert_eq!(limiter.kept(), 51, "fifty addresses and one session");

        limiter.sweep(T0 + Duration::from_secs(1));

        assert_eq!(limiter.kept(), 51, "nothing has aged out yet");

        limiter.sweep(T0 + Duration::from_secs(MINUTE + 1));

        assert_eq!(limiter.kept(), 0, "everything has aged out");
    }

    /// A swept caller starts again from a full budget, which is correct: the
    /// window they had is empty, so keeping it would only cost memory.
    #[test]
    fn a_swept_caller_is_served_again() {
        let mut limiter = ServerRateLimiter::default();
        let ctx = ServerRateCtx::new(Route::Snapshot, anonymous("203.0.113.7"));

        for _ in 0..CALLS_PER_MINUTE {
            limiter.can_send(T0, &ctx);
        }

        let later = T0 + Duration::from_secs(MINUTE + 1);

        limiter.sweep(later);

        assert!(limiter.can_send(later, &ctx));
    }
}
