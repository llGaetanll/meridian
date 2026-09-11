use client_state::ViewDayCard;
use leptos::prelude::*;

/// What lands on the day picked in the calendar, under the month it was picked
/// in. Nothing is drawn while no day is picked.
#[component]
pub fn DayCard(#[prop(into)] card: Signal<Option<ViewDayCard>>) -> impl IntoView {
    let shown = Memo::new(move |_| card.with(Option::is_some));

    let heading = move || card.with(|card| card.as_ref().map(|card| card.heading.clone()));
    let total = move || card.with(|card| card.as_ref().map(|card| card.total.clone()));
    let rows = move || {
        card.with(|card| {
            card.as_ref()
                .map(|card| card.rows.clone())
                .unwrap_or_default()
        })
    };

    view! {
        <Show when=move || shown.get()>
            <div class="daycard">
                <div class="daycard-head">
                    <span>{heading}</span>
                    <span class="t money">{total}</span>
                </div>
                <For each=rows key=|row| format!("{row:?}") let:row>
                    <div class="dayrow">
                        <span class="nm">{row.name}</span>
                        <span class="amt money">{row.amount}</span>
                    </div>
                </For>
            </div>
        </Show>
    }
}
