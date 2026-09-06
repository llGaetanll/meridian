//! One module per screen, drawing the view value the core built.

mod account;
mod balances;
mod ledger;
mod recurring;

pub use account::AccountScreen;
pub use balances::BalancesScreen;
pub use ledger::LedgerScreen;
pub use recurring::RecurringScreen;
