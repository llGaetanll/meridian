//! What comes back: the monthly commitment, split by category, then every
//! stream grouped the same way.

use api_core::RecurringView;
use leptos::prelude::*;

use crate::ui::Breakdown;
use crate::ui::Masked;
use crate::ui::PhoneShell;
use crate::ui::StreamRowView;

#[component]
pub fn RecurringScreen(view: RecurringView) -> impl IntoView {
    let groups = view.groups.clone();

    view! {
        <PhoneShell title="Recurring" subtitle=view.subtitle>
            <div class="rec-hero">
                <div class="v num">
                    <Masked value=view.headline masked=view.masked tappable=true />
                    <span class="per">"/mo"</span>
                </div>
                <div class="d faint">{view.detail}</div>
            </div>

            <Breakdown slices=view.breakdown legend=true />

            <div class="rec-list">
                <For each=move || groups.clone() key=|group| group.name.clone() let:group>
                    <div class=format!("rec-head {}", group.token)>
                        <span class="nm">{group.name.clone()}</span>
                        <span class="sum num">{group.total.clone()}</span>
                    </div>

                    <For each=move || group.streams.clone() key=|row| row.id.clone() let:row>
                        <StreamRowView row=row />
                    </For>
                </For>
            </div>
        </PhoneShell>
    }
}
