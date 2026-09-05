//! The whole app: one signal, matched on.
//!
//! There is no router and no page state. The core says which screen it is on
//! and hands over everything that screen shows, so this is a `match` over the
//! last thing it rendered.

use leptos::prelude::*;
use phone_state::ViewScreen;

use crate::runtime_app::RuntimeApp;
use crate::screens::AccountScreen;
use crate::screens::BalancesScreen;
use crate::screens::LedgerScreen;
use crate::screens::RecurringScreen;

#[component]
pub fn App() -> impl IntoView {
    let runtime = RuntimeApp::run();

    let screen = runtime.screen();

    view! {
        {move || match screen.get() {
            None => view! { <div class="boot">"Starting…"</div> }.into_any(),
            Some(ViewScreen::Ledger { view }) => view! { <LedgerScreen view=view /> }.into_any(),
            Some(ViewScreen::Recurring { view }) => {
                view! { <RecurringScreen view=view /> }.into_any()
            }
            Some(ViewScreen::Balances { view }) => {
                view! { <BalancesScreen view=view /> }.into_any()
            }
            Some(ViewScreen::Account { view }) => {
                view! { <AccountScreen view=view /> }.into_any()
            }
            // Reached only by signing out, which is what the Account tab's
            // one button does — the web has no session to forget, so the
            // machine's way back to first run is a road that stops here. It
            // is also what makes adding a screen in Rust keep failing to
            // compile until every front end has drawn it.
            Some(ViewScreen::Onboarding { .. }) => {
                view! { <div class="boot">"First run is on the phone."</div> }.into_any()
            }
        }}
    }
}
