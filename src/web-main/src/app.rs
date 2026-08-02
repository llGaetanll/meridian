use leptos::prelude::*;
use web_components::NavItem;
use web_components::Shell;

use crate::pages::MarketsPage;

#[component]
pub fn App() -> impl IntoView {
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
