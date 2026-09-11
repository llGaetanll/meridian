use client_state::ViewDaySection;
use client_state::ViewMonthSection;
use leptos::prelude::*;

use super::TxRow;

/// Every transaction the filter lets through, under its month and its day.
///
/// Flat on purpose: months, days and rows are siblings, so a pinned day heading
/// holds until the next one arrives, and the rule between two days belongs to
/// the row above (`.tx + .day`) rather than to the heading.
///
/// Each level is keyed by what the machine calls it — the month's label, the
/// day's heading — and reads the rest from `months` by that key, so a render
/// that changes one day's rows redraws those rows and nothing else.
#[component]
pub fn LedgerList(
    months: Signal<Vec<ViewMonthSection>>,
    empty: Signal<Option<String>>,
    #[prop(into)] on_select: Callback<String>,
) -> impl IntoView {
    let labels = move || {
        months.with(|months| {
            months
                .iter()
                .map(|month| month.label.clone())
                .collect::<Vec<_>>()
        })
    };

    view! {
        <div class="list">
            {move || empty.get().map(|text| view! { <div class="list-empty">{text}</div> })}
            <For each=labels key=|label| label.clone() let:label>
                <Month label=label months=months on_select=on_select />
            </For>
        </div>
    }
}

/// A month's marker, and its days under it.
#[component]
fn Month(
    label: String,
    months: Signal<Vec<ViewMonthSection>>,
    on_select: Callback<String>,
) -> impl IntoView {
    let headings = {
        let label = label.clone();

        move || {
            months.with(|months| {
                month(months, &label)
                    .map(|month| {
                        month
                            .days
                            .iter()
                            .map(|day| day.heading.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
        }
    };

    view! {
        <div class="month">{label.clone()}</div>
        <For each=headings key=|heading| heading.clone() let:heading>
            <Day month=label.clone() heading=heading months=months on_select=on_select />
        </For>
    }
}

/// A day's heading, carrying its net, and its rows.
#[component]
fn Day(
    month: String,
    heading: String,
    months: Signal<Vec<ViewMonthSection>>,
    on_select: Callback<String>,
) -> impl IntoView {
    let total = {
        let (month, heading) = (month.clone(), heading.clone());

        move || {
            months.with(|months| {
                day(months, &month, &heading)
                    .map(|day| day.total.clone())
                    .unwrap_or_default()
            })
        }
    };

    let rows = {
        let heading = heading.clone();

        move || {
            months.with(|months| {
                day(months, &month, &heading)
                    .map(|day| day.rows.clone())
                    .unwrap_or_default()
            })
        }
    };

    // Keyed by everything the row shows, so a row that changed is redrawn and
    // one that did not is left where it is.
    view! {
        <div class="day">
            <span>{heading}</span>
            <span class="sum money">{total}</span>
        </div>
        <For each=rows key=|row| format!("{row:?}") let:row>
            <TxRow row=row on_select=on_select />
        </For>
    }
}

fn month<'a>(months: &'a [ViewMonthSection], label: &str) -> Option<&'a ViewMonthSection> {
    months.iter().find(|month| month.label == label)
}

fn day<'a>(
    months: &'a [ViewMonthSection],
    label: &str,
    heading: &str,
) -> Option<&'a ViewDaySection> {
    month(months, label)?
        .days
        .iter()
        .find(|day| day.heading == heading)
}
