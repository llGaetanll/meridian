use leptos::prelude::*;
use web_components::NavItem;
use web_components::Shell;

use crate::pages::MarketsPage;
use crate::pages::PaywallPage;
use crate::state::BillingState;
use crate::state::Standing;
use crate::state::provide_markets;

#[component]
pub fn App() -> impl IntoView {
    provide_markets();

    let billing = BillingState::new();

    view! {
        {move || match billing.standing() {
            // Every gated route answers 402 for this session, so there is no
            // app behind the sidebar to draw.
            Standing::Unpaid(status) => view! { <PaywallPage billing=status /> }.into_any(),
            _ => {
                view! {
                    <Shell
                        active=NavItem::Markets
                        account_name="Gaetan Almela"
                        account_status="4 connections live"
                    >
                        <MarketsPage />
                    </Shell>
                }
                    .into_any()
            }
        }}
    }
}
