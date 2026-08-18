//! State for the phone app.
//!
//! There is no state machine yet (`api-core` is still planned), so this is
//! signals. What it does keep is the shape: nothing mutates state directly,
//! everything goes through `dispatch` with an event value. When the machine
//! lands, `dispatch` calls into it and the callers do not change.

mod transactions;

pub use transactions::*;
