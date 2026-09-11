use client_state::ViewStreamGroup;
use client_state::ViewStreamRow;
use leptos::prelude::*;

/// Every stream as the phone lists them: grouped the way the split groups
/// them, each a name over when it comes back, and what it costs.
#[component]
pub fn StreamGroups(#[prop(into)] groups: Signal<Vec<ViewStreamGroup>>) -> impl IntoView {
    view! {
        // Keyed by everything a group says, so a group that changes is drawn
        // again and the rest are left where they are.
        <For each=move || groups.get() key=|group| format!("{group:?}") let:group>
            <div class="rec-head">
                <span class="nm">{group.name}</span>
                <span class="sum">{group.count_label}</span>
            </div>
            {group.streams.into_iter().map(|row| view! { <StreamRow row=row /> }).collect_view()}
        </For>
    }
}

/// One stream: what it is called, when it comes back, and what it costs. How
/// often and when next are one phrase here, as the machine joined them.
#[component]
fn StreamRow(row: ViewStreamRow) -> impl IntoView {
    view! {
        <div class="stream">
            <div class="t">
                <div class="nm">{row.title}</div>
                <div class="sub">{row.sub}</div>
            </div>
            <span class="amt money">{row.amount}</span>
        </div>
    }
}
