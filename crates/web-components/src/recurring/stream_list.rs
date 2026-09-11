use client_state::ViewStreamGroup;
use client_state::ViewStreamRow;
use leptos::prelude::*;

/// Every stream, grouped the way the split groups them.
///
/// The groups are drawn flat — a heading, its rows, the next heading — so the
/// rules between rows run on unbroken from one group into the next.
#[component]
pub fn StreamList(#[prop(into)] groups: Signal<Vec<ViewStreamGroup>>) -> impl IntoView {
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

/// One stream: who, how often, when next, and how much. The phone joins the
/// middle two into one line; here each is a column.
#[component]
fn StreamRow(row: ViewStreamRow) -> impl IntoView {
    view! {
        <div class="rec">
            <span class="mark">{row.logo_url.map(|url| view! { <img src=url alt="" /> })}</span>
            <span class="rec-name">{row.title}</span>
            <span class="rec-freq">{row.frequency}</span>
            <span class="rec-when" class:soon=row.soon>
                {row.when}
            </span>
            <span class="rec-amt money">{row.amount}</span>
        </div>
    }
}
