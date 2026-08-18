use leptos::prelude::*;
use web_components::NavItem;
use web_components::Shell;

use crate::pages::MarketsPage;
use crate::state::provide_markets;

#[component]
pub fn App() -> impl IntoView {
    provide_markets();

    view! {
        <Shell
            active=NavItem::Markets
            account_name="Gaetan Almela"
            account_status="4 connections live"
        >
            <MarketsPage />
        </Shell>
    }
}
