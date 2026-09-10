//! Something for the store to do.
//!
//! Reads as well as writes: anything the machine needs from the store it asks
//! for here, so nothing reads on the request path.
//!
//! Nothing here carries an access token — one lives in the runtime, and an
//! action carrying it would put a bank credential into the event log.

use crate::CallId;

use api_ledger::Account;
use api_ledger::Entry;
use api_ledger::Stream;

#[derive(Debug)]
pub enum ActionStore {
    /// Who this session token signs in, if anybody.
    ///
    /// Carries the clock, so the row this matches is the one the machine
    /// reckoned was still good.
    LoadSession {
        call_id: CallId,
        token_sha256: Vec<u8>,
        now: api_common::Timestamp,
    },

    /// One bank, by the id Plaid names it with.
    ///
    /// Unscoped, unlike [`LoadItems`](ActionStore::LoadItems): a webhook names
    /// a bank and not a person, so who owns it is what the read finds out.
    LoadItem { call_id: CallId, item_id: String },

    /// Which banks are this user's, and where each one's last sync reached.
    ///
    /// The facts a decision about going back to Plaid is made from.
    LoadItems {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// The Link sessions this user has open and has not collected.
    ///
    /// A session is collected by whatever *reports* it, so an open one is one
    /// nobody has been told about.
    LoadPendingLinks {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// Everything this user's ledger holds, projected for the wire.
    ///
    /// Asked for only once everything the request set going has landed, so the
    /// rows read are the ones those calls just wrote.
    LoadSnapshot {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// What code stands against this address, if any.
    ///
    /// Asked for rather than read at the edge, which is what makes verifying
    /// one reachable from an `eval!` test.
    LoadCode { call_id: CallId, email: String },

    /// Hold a sign-in code against an address, replacing any that stood.
    ///
    /// The hash, never the code. The plaintext goes only to
    /// [`ActionEmail::SendCode`](crate::ActionEmail).
    SaveCode {
        call_id: CallId,
        email: String,
        code_sha256: Vec<u8>,
        expires_at: api_common::Timestamp,
    },

    /// Count a wrong guess against the code standing for this address.
    RecordAttempt { call_id: CallId, email: String },

    /// Spend the code. What makes a replayed one worth nothing.
    DeleteCode { call_id: CallId, email: String },

    /// Sign somebody in. The runtime resolves the address to a user, which is
    /// a lookup rather than a decision — the machine has no use for a uuid.
    CreateSession {
        call_id: CallId,
        email: String,
        token_sha256: Vec<u8>,
        expires_at: api_common::Timestamp,
    },

    /// Drop every session that has already expired.
    ///
    /// Housekeeping rather than correctness — the lookup filters on expiry
    /// either way — but without it the table only ever grows. Carries the
    /// clock, so a sweep drops the rows the machine reckoned were expired.
    SweepSessions {
        call_id: CallId,
        now: api_common::Timestamp,
    },

    /// Remember that this user opened this Link session.
    ///
    /// Written before the URL reaches them, so no session can be finished that
    /// we have no record of asking for — which is what stops one account
    /// collecting another's bank.
    SaveLink {
        call_id: CallId,
        link_token: String,
        user_id: uuid::Uuid,
    },

    /// Mark a Link session done, whatever it came to. Set on an empty one too:
    /// a session that stays open forever is one we would poll forever.
    CollectLink { call_id: CallId, link_token: String },

    /// Store a newly linked bank.
    SaveItem {
        call_id: CallId,
        item_id: String,
        link_token: String,
    },

    /// Write what a bank's accounts hold, and what the bank is called.
    ///
    /// One Plaid answer, one write — which is what makes a sync that stops
    /// halfway leave behind what it did get rather than nothing.
    SaveAccounts {
        call_id: CallId,
        item_id: String,

        /// What the bank is called, so a cold start has a name to draw before
        /// the next fetch comes back.
        institution_name: Option<String>,

        accounts: Vec<Account>,
    },

    /// Write one page of a sync.
    ///
    /// Carries the diff rather than asking the runtime to read it off state,
    /// because by the time this is performed the diff is gone — state holds
    /// the result of applying it.
    SaveTransactions {
        call_id: CallId,
        item_id: String,

        added: Vec<Entry>,
        modified: Vec<Entry>,
        removed: Vec<String>,

        /// Written last and separately, so a failure earlier in the page
        /// leaves it where it was and the page is read again rather than
        /// skipped.
        cursor: String,
    },

    /// Write the charges that come back on a bank.
    SaveStreams {
        call_id: CallId,
        item_id: String,
        streams: Vec<Stream>,
    },

    /// Remember whether a connection needs a person.
    ///
    /// Its own action because it outlives the process: a server that restarts
    /// into an `ITEM_LOGIN_REQUIRED` it has forgotten asks Plaid again and is
    /// told the same thing.
    SaveStatus {
        call_id: CallId,
        item_id: String,
        error_code: Option<String>,
    },

    /// Remember that this user is this provider's customer.
    ///
    /// Written when the customer is created rather than when a payment lands,
    /// because the webhook that says it landed names only the customer.
    SaveCustomer {
        call_id: CallId,
        user_id: uuid::Uuid,
        provider: src_postgres::BillingProvider,
        customer_id: String,
        subscription_id: Option<String>,
        status: String,
    },

    /// Write what a webhook said about a subscription.
    ///
    /// The query refuses a delivery that is a repeat or older than what is
    /// already there, so this is safe to send for every believed webhook.
    ApplySubscriptionEvent {
        call_id: CallId,
        subscription_id: String,
        status: String,
        current_period_end: Option<api_common::Timestamp>,
        cancel_at_period_end: bool,
        event_id: String,
        event_at: api_common::Timestamp,
    },

    /// Every subscription there is, for the read at startup.
    LoadSubscriptions { call_id: CallId },

    /// The address a user is written to, for Stripe's receipt.
    LoadUser {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// Every access token this user's banks were reached with, opened.
    ///
    /// Read before anything is deleted: the tokens are what the Plaid
    /// removals need, and deleting the user takes `items` with it.
    LoadAccessTokens {
        call_id: CallId,
        user_id: uuid::Uuid,
    },

    /// Delete the account and everything that hangs off it.
    DeleteUser {
        call_id: CallId,
        user_id: uuid::Uuid,
    },
}

impl ActionStore {
    /// Which call this write is.
    ///
    /// Every variant carries one, so the runtime can report the outcome
    /// against it without knowing which write it just did.
    pub fn call_id(&self) -> CallId {
        match self {
            Self::LoadSession { call_id, .. }
            | Self::LoadItem { call_id, .. }
            | Self::LoadItems { call_id, .. }
            | Self::LoadPendingLinks { call_id, .. }
            | Self::LoadSnapshot { call_id, .. }
            | Self::LoadCode { call_id, .. }
            | Self::SaveCode { call_id, .. }
            | Self::RecordAttempt { call_id, .. }
            | Self::DeleteCode { call_id, .. }
            | Self::CreateSession { call_id, .. }
            | Self::SaveLink { call_id, .. }
            | Self::CollectLink { call_id, .. }
            | Self::SaveItem { call_id, .. }
            | Self::SaveAccounts { call_id, .. }
            | Self::SaveTransactions { call_id, .. }
            | Self::SaveStreams { call_id, .. }
            | Self::SaveStatus { call_id, .. }
            | Self::SweepSessions { call_id, .. }
            | Self::SaveCustomer { call_id, .. }
            | Self::ApplySubscriptionEvent { call_id, .. }
            | Self::LoadSubscriptions { call_id, .. }
            | Self::LoadUser { call_id, .. }
            | Self::LoadAccessTokens { call_id, .. }
            | Self::DeleteUser { call_id, .. } => *call_id,
        }
    }
}
