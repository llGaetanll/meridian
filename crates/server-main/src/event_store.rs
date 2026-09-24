//! The store answered, or failed to.
//!
//! A read carries what it found; a write carries only that it landed.

use crate::StoredCode;

#[derive(Debug)]
pub enum EventStore {
    /// Who a session token signs in. `None` is a token that names nobody —
    /// wrong, expired, or signed out, deliberately not told apart.
    SessionResolved { user_id: Option<uuid::Uuid> },

    /// One bank a webhook named, as the store holds it. `None` is a bank we
    /// have no row for — Plaid keeps sending after one is removed.
    ItemLoaded { item: Option<crate::ItemFacts> },

    /// The bank a Link session produced, as it was written down.
    ///
    /// `None` is a write that found no session to attach it to or no token to
    /// attach — both logged where they happen, and neither leaves anything to
    /// sync.
    ItemSaved { item: Option<crate::ItemFacts> },

    /// This user's banks, as the store holds them. Empty is a first run, not a
    /// failure.
    ItemsLoaded { items: Vec<crate::ItemFacts> },

    /// The Link sessions this user has open. Empty is a sheet closed on a
    /// session we have no record of, or one already collected.
    PendingLinksLoaded { links: Vec<crate::PendingLinkFacts> },

    /// Which Item a Link session repairs. `None` is an ordinary session that
    /// attaches, or one we have no row for — neither may skip the exchange.
    LinkRepairLoaded { repair_of: Option<String> },

    /// The ledger a snapshot is about to be answered with, as a value: nothing
    /// outside the machine reads the store.
    SnapshotLoaded { snapshot: server_wire::WireSnapshot },

    /// What stands against the address a sign-in named. `None` is an address
    /// with no code, which is a rejection like any other.
    CodeLoaded { stored: Option<StoredCode> },

    /// A read the store would not answer. Its own event rather than an empty
    /// answer: "no code" and "we could not look" are different sentences.
    ReadError { message: String },

    /// The write landed.
    WriteSuccess,

    /// The write did not land, and why. Recoverable because the cursor did not
    /// move: the next sync re-reads the same range.
    WriteError { message: String },

    /// Every subscription the store holds, at startup.
    SubscriptionsLoaded {
        subscriptions: Vec<src_postgres::StoredSubscription>,
    },

    /// The address a user is written to. `None` is a user that is not there,
    /// which a live session should make impossible.
    UserLoaded { email: Option<String> },

    /// The access tokens this user's banks were reached with, and the bank
    /// each one opens. Empty is an account with no banks, which deletes fine.
    AccessTokensLoaded { tokens: Vec<(String, String)> },

    /// What deletion code stands against the user a deletion named. `None` is
    /// an account with no code, which is a rejection like any other.
    DeleteCodeLoaded { stored: Option<StoredCode> },

    /// The stretches of time a series is known over, sorted and disjoint.
    /// Empty is a series nobody has fetched.
    BarCoverageLoaded {
        spans: Vec<std::ops::Range<api_common::Timestamp>>,
    },

    /// The bars a series holds over the window that was asked, in time order.
    BarsLoaded { bars: Vec<api_common::Bar> },
}
