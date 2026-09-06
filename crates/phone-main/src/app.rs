//! The whole app: one signal, matched on. There is no router and no page
//! state — the core says which screen it is on and hands over everything that
//! screen shows.

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
            // Reached only by signing out. The web has no session to forget,
            // so the machine's way back to first run stops here.
            Some(ViewScreen::Onboarding { .. }) => {
                view! { <div class="boot">"First run is on the phone."</div> }.into_any()
            }
        }}
    }
}
