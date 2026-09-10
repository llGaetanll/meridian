//! Reactive state. It calls into [`crate::io`] but performs no requests itself.

mod billing;
mod markets;

use leptos::prelude::*;

pub use billing::BillingState;
pub use billing::Standing;
pub use markets::MarketsState;

use crate::io::Markets;

/// The cache every page pulls market data through.
///
/// It is provided once at the root rather than made per page, which is what
/// lets a symbol looked at, navigated away from, and come back to cost nothing
/// the second time.
type Cache = StoredValue<Markets, LocalStorage>;

pub fn provide_markets() {
    provide_context::<Cache>(StoredValue::new_local(Markets::new()));
}

fn markets_cache() -> Cache {
    expect_context::<Cache>()
}
