//! The whole app: one signal, matched on.
//!
//! There is no router and no page state. The core says which screen it is on
//! and hands over everything that screen shows, so this is a `match` over the
//! last thing it rendered.

use api_core::ScreenView;
use api_core::UiEvent;
use leptos::prelude::*;

use crate::runtime::provide_runtime;
use crate::screens::BalancesScreen;
use crate::screens::HomeScreen;
use crate::screens::LedgerScreen;
use crate::screens::RecurringScreen;

#[component]
pub fn App() -> impl IntoView {
    let runtime = provide_runtime();

    // The clock is read here, at the edge, and handed in. Nothing below this
    // line reads one — which is what makes a replay of the event log land
    // where it did the first time.
    runtime.ui(UiEvent::Started { now: now() });

    let screen = runtime.screen();

    view! {
        {move || match screen.get() {
            None => view! { <div class="boot">"Starting…"</div> }.into_any(),
            Some(ScreenView::Home { view }) => view! { <HomeScreen view=view /> }.into_any(),
            Some(ScreenView::Ledger { view }) => view! { <LedgerScreen view=view /> }.into_any(),
            Some(ScreenView::Recurring { view }) => {
                view! { <RecurringScreen view=view /> }.into_any()
            }
            Some(ScreenView::Balances { view }) => {
                view! { <BalancesScreen view=view /> }.into_any()
            }
        }}
    }
}

/// The wall clock, in seconds. The snapshot carries the server's own clock and
/// overrides this — a phone with a wrong date should not render a confidently
/// wrong "in 3 days" — but the app has to draw something before the first
/// snapshot lands.
fn now() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}
