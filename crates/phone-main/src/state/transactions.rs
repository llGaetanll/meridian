use api_ledger::Tx;
use leptos::prelude::*;

use crate::fake;

/// Something that happened on the transactions screen.
///
/// A stand-in for the `UiEvent` arm the state machine will own — see
/// `docs/ARCHITECTURE.md`. Kept as an enum from the start so that moving it
/// there is a move, not a rewrite.
#[derive(Debug, Clone)]
pub enum TxEvent {
    /// The user asked to record a transaction.
    AddRequested,
}

#[derive(Clone, Copy)]
pub struct Transactions {
    txs: RwSignal<Vec<Tx>>,
}

impl Transactions {
    /// The list, newest first.
    pub fn txs(&self) -> RwSignal<Vec<Tx>> {
        self.txs
    }

    /// The only way state changes. Today it logs; tomorrow it hands the event
    /// to the machine and writes the resulting state back into the signals.
    pub fn dispatch(&self, event: TxEvent) {
        tracing::info!(target: "phone", ?event, "tx event");

        match event {
            // Nothing to add against yet — the screen this opens is the next
            // piece of work.
            TxEvent::AddRequested => {}
        }
    }
}

pub fn provide_transactions() {
    provide_context(Transactions {
        txs: RwSignal::new(fake::txs()),
    });
}

pub fn use_transactions() -> Transactions {
    expect_context()
}
