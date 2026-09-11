use client_state::ViewCalendar;
use client_state::ViewCalendarDay;
use leptos::prelude::*;
use web_ui::Glyph;

/// The month, centred between two arrows, with a mark on every day a stream
/// lands.
///
/// A whole month fits the rail, so the phone's sideways strip and its swipe
/// are not here: the arrows step the month, and only the current one is drawn.
#[component]
pub fn Calendar(
    #[prop(into)] calendar: Signal<ViewCalendar>,
    /// A day in the month was picked, by its id.
    on_pick: Callback<String>,
    /// The month was stepped, by −1 or 1.
    on_step: Callback<i32>,
) -> impl IntoView {
    let weekdays = move || {
        calendar.with(|calendar| {
            calendar
                .weekdays
                .iter()
                .cloned()
                .enumerate()
                .collect::<Vec<_>>()
        })
    };

    let days = move || {
        calendar.with(|calendar| {
            calendar
                .current
                .weeks
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    view! {
        <div class="rail-head cal-head">
            <button class="btn plain" on:click=move |_| on_step.run(-1)>
                <Glyph name="prev" />
            </button>
            <span class="cal-month">{move || calendar.with(|c| c.current.label.clone())}</span>
            <button class="btn plain" on:click=move |_| on_step.run(1)>
                <Glyph name="next" />
            </button>
        </div>

        <div class="cal-grid">
            // Tuesday and Thursday are both "T", so a heading is keyed by
            // where it sits as well as by what it says.
            <For each=weekdays key=|heading| heading.clone() let:heading>
                <div class="cal-wd">{heading.1}</div>
            </For>

            // Keyed by everything a day says, so turning the month redraws the
            // grid and picking a day redraws only the two it moved between.
            <For each=days key=|day| format!("{day:?}") let:day>
                <CalendarDay day=day on_pick=on_pick />
            </For>
        </div>
    }
}

/// One day: plain, marked, today, picked, or outside the month.
#[component]
fn CalendarDay(day: ViewCalendarDay, on_pick: Callback<String>) -> impl IntoView {
    let ViewCalendarDay {
        id,
        label,
        outside,
        today,
        picked,
        marks,
    } = day;

    // A day of the month either side is drawn for the shape of the grid, and
    // is not one of this month's to pick.
    let pick = move |_| {
        if !outside {
            on_pick.run(id.clone());
        }
    };

    view! {
        <div class="cal-day" class:outside=outside class:today=today class:picked=picked on:click=pick>
            <span class="n">{label}</span>
            <span class="marks">{marks.iter().map(|_| view! { <i></i> }).collect_view()}</span>
        </div>
    }
}
