//! The app at the width of a phone, drawn the way the iOS app draws it: a
//! title, one column, and the dock at the foot.
//!
//! The same machine and the same screens as the wide layout, and many of the
//! same components; what differs is the frame and the shape of each page.

mod account;
mod connections;
mod ledger;
mod narrow;
mod phone_app;
mod recurring;

use account::AccountPhone;
use connections::ConnectionsPhone;
use ledger::LedgerPhone;
pub use narrow::narrow;
pub use phone_app::PhoneApp;
use recurring::RecurringPhone;
