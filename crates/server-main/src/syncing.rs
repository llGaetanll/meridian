//! A sync that is running, and whose it is.

use api_common::Timestamp;
use server_wire::SyncStage;
use server_wire::WireSyncStatus;
use uuid::Uuid;

/// One bank's fetch, from the first call to the last.
///
/// The owner is held rather than looked up: a sync outlives the request that
/// started it, and both places one starts have an Item and no session.
#[derive(Debug)]
pub struct Syncing {
    pub user_id: Uuid,

    /// Where this sync resumes from, and where its next page starts. Carried
    /// because the balances call goes first and this is not wanted until
    /// after it.
    pub cursor: Option<String>,

    /// Which of the calls is out.
    pub stage: SyncStage,

    /// Whether there is still a call out. A finished sync is kept rather than
    /// dropped: it is what a screen watching one reads to say it is done, and
    /// the totals under it.
    pub finished: bool,

    /// What the bank is called, once the balances call has said. It is in
    /// neither the Link session nor the exchange.
    pub institution: Option<String>,

    /// What this sync has turned up. Counted here because it exists nowhere
    /// else once the writes have gone out: these are what the sync brought,
    /// not what the store now holds.
    pub accounts: u32,
    pub transactions: u32,
    pub streams: u32,

    /// When to ask for the transactions again, set when Plaid answered
    /// `PRODUCT_NOT_READY` — a fresh Item generates its history
    /// asynchronously, so the first ask can come too early. The tick is what
    /// reads it.
    pub retry_at: Option<Timestamp>,

    /// How many of those retries have gone out, which is what decides the
    /// next wait — and when there is no next.
    pub retries: u32,
}

impl Syncing {
    /// Nothing is known yet. What matters is that it is *not* finished.
    pub fn started(user_id: Uuid, cursor: Option<String>) -> Self {
        Self {
            user_id,
            cursor,
            stage: SyncStage::Accounts,
            finished: false,
            institution: None,
            accounts: 0,
            transactions: 0,
            streams: 0,
            retry_at: None,
            retries: 0,
        }
    }

    /// What a screen watching this sync is told.
    pub fn status(&self) -> WireSyncStatus {
        let mut status = WireSyncStatus::new(
            self.stage,
            self.institution.clone(),
            self.accounts,
            self.transactions,
            self.streams,
        );

        status.finished = self.finished;

        status
    }

    /// Where a stage sits in the order they run in, so two can be compared.
    pub fn stage_index(stage: SyncStage) -> usize {
        SyncStage::ALL
            .iter()
            .position(|s| *s == stage)
            .unwrap_or_default()
    }
}
