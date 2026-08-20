//! The ledger: everything that has happened, newest first, and the filter
//! that narrows it.

use leptos::prelude::*;
use phone_state::EventUi;
use phone_state::FilterGroup;
use phone_state::RangeSelection;
use phone_state::RangeUnit;
use phone_state::ViewLedger;
use phone_state::ViewRangeKind;

use crate::runtime_app::use_runtime;
use crate::ui::Icon;
use crate::ui::PhoneShell;
use crate::ui::TxRowView;

#[component]
pub fn LedgerScreen(view: ViewLedger) -> impl IntoView {
    let filter = view.filter.clone();

    // The panel is drawn only when the core says it is open, and the core is
    // what the filter button toggles. Nothing here tracks whether it is
    // showing.
    let panel = move || {
        let filter = filter.clone();
        view! {
            <FilterBar active=filter.active open=filter.open />
            {filter.open.then(|| view! { <FilterPanel filter=filter.clone() /> })}
        }
    };

    let months = view.months.clone();
    let empty = view.empty.clone();

    view! {
        <PhoneShell title="Ledger" subtitle=view.subtitle panel=panel>
            <div class="list">
                {empty
                    .map(|message| {
                        view! { <div class="empty">{message}</div> }
                    })}

                <For each=move || months.clone() key=|month| format!("{}{}", month.name, month.year) let:month>
                    <div class="month">
                        <span class="name">{month.name.clone()}</span>
                        <span class="rule"></span>
                        <span class="year">{month.year.clone()}</span>
                    </div>

                    <For each=move || month.days.clone() key=|day| day.heading.clone() let:day>
                        <div class="day">{day.heading.clone()}</div>

                        <For each=move || day.rows.clone() key=|row| row.id.clone() let:row>
                            <TxRowView row=row />
                        </For>
                    </For>
                </For>
            </div>
        </PhoneShell>
    }
}

/// The control that opens the panel. A dot rather than a count: whether
/// anything is filtering is the useful bit, and the panel says what.
#[component]
fn FilterBar(active: bool, open: bool) -> impl IntoView {
    let runtime = use_runtime();

    view! {
        <div class="filterbar">
            <button
                class="filter-btn"
                class:on=open
                class:active=active
                on:click=move |_| runtime.ui(EventUi::FilterToggled)
            >
                <Icon name="filter" />
                "Filter"
                {active.then(|| view! { <i class="dot"></i> })}
            </button>

            {(active && open)
                .then(|| {
                    view! {
                        <button
                            class="filter-clear"
                            on:click=move |_| runtime.ui(EventUi::CategoriesCleared)
                        >
                            "Clear categories"
                        </button>
                    }
                })}
        </div>
    }
}

#[component]
fn FilterPanel(filter: phone_state::ViewFilter) -> impl IntoView {
    let runtime = use_runtime();

    let categories = filter.categories.clone();
    let summary = filter.categories_summary.clone();
    let overflow = filter.categories_overflow.clone();
    let range = filter.range.clone();

    view! {
        <div class="panel">
            <div class="filter-group" class:shut=!filter.categories_open>
                <div
                    class="filter-head"
                    on:click=move |_| {
                        runtime
                            .ui(EventUi::FilterGroupToggled {
                                group: FilterGroup::Categories,
                            })
                    }
                >
                    <Icon name="tag" />
                    <span class="filter-title">"Categories"</span>

                    {(!filter.categories_open)
                        .then(|| {
                            view! {
                                <span class="filter-sum">
                                    {summary
                                        .iter()
                                        .map(|chip| {
                                            view! {
                                                <span class=format!(
                                                    "cat {}",
                                                    chip.token,
                                                )>{chip.label.clone()}</span>
                                            }
                                        })
                                        .collect_view()}
                                    {overflow
                                        .clone()
                                        .map(|more| view! { <span class="more">{more}</span> })}
                                </span>
                            }
                        })}

                    <Icon name="chevron" class="caret" />
                </div>

                <div class="cat-grid">
                    <For each=move || categories.clone() key=|chip| chip.id.clone() let:chip>
                        {
                            let id = chip.id.clone();
                            view! {
                                <button
                                    class=format!("cat {}", chip.token)
                                    class:on=chip.selected
                                    class:empty=chip.empty
                                    on:click=move |_| {
                                        runtime.ui(EventUi::CategoryToggled { id: id.clone() })
                                    }
                                >
                                    <span class="n">{chip.count.clone()}</span>
                                    {chip.label.clone()}
                                </button>
                            }
                        }
                    </For>
                </div>
            </div>

            <div class="filter-group" class:shut=!filter.range_open>
                <div
                    class="filter-head"
                    on:click=move |_| {
                        runtime.ui(EventUi::FilterGroupToggled { group: FilterGroup::Range })
                    }
                >
                    <Icon name="clock" />
                    <span class="filter-title">"Time range"</span>

                    {(!filter.range_open)
                        .then(|| {
                            view! { <span class="filter-sum">{filter.range_summary.clone()}</span> }
                        })}

                    <Icon name="chevron" class="caret" />
                </div>

                <RangeRows range=range />
            </div>
        </div>
    }
}

/// The range, as the sentence the sketch draws. Each row is one of the four
/// forms; tapping one makes it live.
#[component]
fn RangeRows(range: phone_state::ViewRange) -> impl IntoView {
    let runtime = use_runtime();

    // The tokens keep their last value even when another form is live, so
    // switching back does not reset the sentence.
    let this_unit = RangeUnit::Month;
    let last_count = range.last_count.parse::<u32>().unwrap_or(3);

    view! {
        <div class="range">
            <div
                class="range-row"
                class:on=range.kind == ViewRangeKind::This
                on:click=move |_| {
                    runtime
                        .ui(EventUi::RangeSelected {
                            kind: RangeSelection::This { unit: this_unit },
                        })
                }
            >
                "This "
                <button class="sel">
                    {range.this_unit.clone()}
                    <Icon name="caret" />
                </button>
            </div>

            <div
                class="range-row"
                class:on=range.kind == ViewRangeKind::Last
                on:click=move |_| {
                    runtime
                        .ui(EventUi::RangeSelected {
                            kind: RangeSelection::Last {
                                count: last_count,
                                unit: RangeUnit::Month,
                            },
                        })
                }
            >
                "Last "
                <button class="sel n">
                    {range.last_count.clone()}
                    <Icon name="caret" />
                </button>
                <button class="sel">
                    {range.last_unit.clone()}
                    <Icon name="caret" />
                </button>
            </div>

            // Drawn because the sketch has it, and inert because picking a day
            // needs a date picker that is a screen of its own.
            <div class="range-row disabled" class:on=range.kind == ViewRangeKind::Between>
                "Between "
                <button class="sel" disabled>
                    {range.between_start.clone()}
                    <Icon name="caret" />
                </button>
                " and "
                <button class="sel" disabled>
                    {range.between_end.clone()}
                    <Icon name="caret" />
                </button>
            </div>

            <div
                class="range-row"
                class:on=range.kind == ViewRangeKind::All
                on:click=move |_| {
                    runtime.ui(EventUi::RangeSelected { kind: RangeSelection::All })
                }
            >
                <span class="plain">"All time"</span>
            </div>
        </div>
    }
}
