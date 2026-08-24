//! One module per screen. Each takes the view value the core built and draws
//! it — see `phone_state::ViewScreen`.

mod balances;
mod ledger;
mod recurring;

pub use balances::BalancesScreen;
pub use ledger::LedgerScreen;
pub use recurring::RecurringScreen;
