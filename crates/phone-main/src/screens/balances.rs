//! Where the money sits.
//!
//! The headline is what you hold, not net worth. Plaid reports the mortgage
//! but never the house behind it, and the student loan but not the degree, so
//! a net figure is structurally too red — opening the app should not read as
//! "you are $77k in the hole". Debts get their own blocks below.

use leptos::prelude::*;
use phone_state::ViewBalances;

use crate::ui::ConnectionBlock;
use crate::ui::Masked;
use crate::ui::PhoneShell;

#[component]
pub fn BalancesScreen(view: ViewBalances) -> impl IntoView {
    let connections = view.connections.clone();

    view! {
        <PhoneShell title="Balances" subtitle=view.subtitle>
            <div class="networth">
                <div class="k">{view.headline_label}</div>
                <div class="v num">
                    <Masked value=view.headline masked=view.masked tappable=true />
                </div>
            </div>

            <div class="bal-list">
                <For
                    each=move || connections.clone()
                    key=|connection| connection.id.clone()
                    let:connection
                >
                    <ConnectionBlock connection=connection />
                </For>
            </div>
        </PhoneShell>
    }
}
