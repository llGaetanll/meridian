use std::fmt::Display;
use std::ops::RangeBounds;
use std::str::FromStr;

use api_common::Timestamp;

use crate::Entry;
use crate::Positions;
use crate::Tx;

/// Everything that has happened, newest first.
///
/// One list, not two. `docs/ABOUT.md` argues the case: spending apps and
/// portfolio apps keep separate models and bolt a net worth number on top,
/// and every question that crosses between them then has to cross a seam.
/// Here a paycheck, a burrito and a share of AAPL are the same kind of row.
#[derive(Debug, Default, Clone)]
pub struct Ledger {
    entries: Vec<Entry>,
}

impl Ledger {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut ledger = Self { entries };
        ledger.sort();
        ledger
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Apply what a sync turned up.
    ///
    /// A sync is a diff, not a snapshot, so all three lists matter: `removed`
    /// has to actually remove, or a transaction the bank retracted stays on
    /// screen forever. Modified entries replace by id rather than joining the
    /// list a second time.
    pub fn apply(&mut self, added: Vec<Entry>, modified: Vec<Entry>, removed: &[String]) {
        self.entries
            .retain(|e| !removed.contains(&e.id) && !modified.iter().any(|m| m.id == e.id));

        self.entries.extend(modified);
        self.entries.extend(added);

        self.resolve_pending();
        self.sort();
    }

    /// Drop the pending entries that have since posted.
    ///
    /// Plaid does not update a pending transaction in place. A *new* one
    /// arrives with its own id and a `pending_transaction_id` pointing back at
    /// the old one, and unless the old one is dropped every purchase is
    /// counted twice for the days between authorisation and settlement.
    ///
    /// `docs/PLAID.md` calls this one of the two ways to be quietly wrong —
    /// quiet because the numbers stay plausible either way.
    fn resolve_pending(&mut self) {
        let superseded: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.supersedes.clone())
            .collect();

        if !superseded.is_empty() {
            self.entries.retain(|e| !superseded.contains(&e.id));
        }
    }

    /// Newest first, which is the order every screen shows.
    ///
    /// Ties break on id so the order is total: two transactions on the same
    /// day would otherwise sort differently run to run, and a list that
    /// reshuffles on refresh looks broken.
    fn sort(&mut self) {
        self.entries
            .sort_by(|a, b| b.ts().cmp(&a.ts()).then_with(|| a.id.cmp(&b.id)));
    }

    /// Compute the ledger's positions for any transaction in the given time
    /// range.
    pub fn compute_positions<R>(&self, range: R) -> Positions
    where
        R: RangeBounds<Timestamp>,
    {
        let mut positions = Positions::default();

        for entry in &self.entries {
            match &entry.tx {
                Tx::Trade { ts, buy, sell } => {
                    if range.contains(ts) {
                        positions.insert(buy);
                        positions.insert(sell);
                    }
                }
                Tx::Transfer { ts, pos } => {
                    if range.contains(ts) {
                        positions.insert(pos);
                    }
                }
            }
        }

        positions
    }
}

impl FromStr for Ledger {
    type Err = anyhow::Error;

    /// The text format, one transaction per line. Comments and blank lines are
    /// skipped.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let entries = s
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| l.parse::<Tx>().map(|tx| Entry::from_tx(i.to_string(), tx)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self::new(entries))
    }
}

impl Display for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for entry in &self.entries {
            writeln!(f, "{}", entry.tx)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Category;
    use crate::Position;

    fn entry(id: &str, secs: u64, qty: f64) -> Entry {
        Entry {
            id: id.to_string(),
            tx: Tx::Transfer {
                ts: Timestamp::from_secs(secs),
                pos: Position {
                    asset: "USD".into(),
                    qty,
                },
            },
            label: id.to_string(),
            account_id: None,
            logo_url: None,
            category: Category::FoodAndDrink,
            pending: false,
            supersedes: None,
        }
    }

    #[test]
    fn newest_comes_first() {
        let ledger = Ledger::new(vec![
            entry("a", 100, -1.0),
            entry("c", 300, -3.0),
            entry("b", 200, -2.0),
        ]);

        let ids: Vec<&str> = ledger.entries().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
    }

    /// Same timestamp, so only the id tiebreak keeps this stable.
    #[test]
    fn the_order_is_total() {
        let first = Ledger::new(vec![entry("b", 100, -1.0), entry("a", 100, -2.0)]);
        let second = Ledger::new(vec![entry("a", 100, -2.0), entry("b", 100, -1.0)]);

        assert_eq!(
            first.entries().iter().map(|e| &e.id).collect::<Vec<_>>(),
            second.entries().iter().map(|e| &e.id).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn a_sync_removes_what_the_bank_retracted() {
        let mut ledger = Ledger::new(vec![entry("a", 100, -1.0), entry("b", 200, -2.0)]);

        ledger.apply(vec![], vec![], &["a".to_string()]);

        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.entries()[0].id, "b");
    }

    #[test]
    fn a_modified_entry_replaces_rather_than_joins() {
        let mut ledger = Ledger::new(vec![entry("a", 100, -1.0)]);

        ledger.apply(vec![], vec![entry("a", 100, -9.0)], &[]);

        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.entries()[0].amount(), -9.0);
    }

    /// The pending-duplicate trap from `docs/PLAID.md`: the posted
    /// transaction points back at the pending one, which must then go.
    #[test]
    fn a_posted_transaction_drops_the_pending_one_it_replaces() {
        let mut ledger = Ledger::new(vec![Entry {
            pending: true,
            ..entry("pending-1", 100, -5.40)
        }]);

        let posted = Entry {
            supersedes: Some("pending-1".to_string()),
            ..entry("posted-1", 101, -5.40)
        };

        ledger.apply(vec![posted], vec![], &[]);

        assert_eq!(ledger.len(), 1, "the purchase must not be counted twice");
        assert_eq!(ledger.entries()[0].id, "posted-1");
    }

    /// A pending entry with nothing pointing at it yet stays: it is the only
    /// record of that purchase until it settles.
    #[test]
    fn an_unsettled_pending_entry_survives() {
        let mut ledger = Ledger::new(vec![]);

        ledger.apply(
            vec![Entry {
                pending: true,
                ..entry("pending-1", 100, -5.40)
            }],
            vec![],
            &[],
        );

        assert_eq!(ledger.len(), 1);
    }
}
