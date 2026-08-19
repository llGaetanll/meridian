//! One module per screen. Each takes the view value the core built and draws
//! it — see `api_core::ScreenView`.

mod balances;
mod home;
mod ledger;
mod recurring;

pub use balances::BalancesScreen;
pub use home::HomeScreen;
pub use ledger::LedgerScreen;
pub use recurring::RecurringScreen;
