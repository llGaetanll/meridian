use std::time::Duration;

use client_state::ViewCalendar;
use client_state::ViewCalendarDay;
use client_state::ViewCalendarMonth;
use leptos::ev::PointerEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::wasm_bindgen::closure::Closure;
use leptos::web_sys::Element;
use leptos::web_sys::ResizeObserver;

/// How wide a month is. It gives up a little of the measure so the months
/// either side have somewhere to be: 310 across seven still leaves half a
/// day's width around each 32px disc.
const MONTH: f64 = 310.0;

/// The months are pages, not columns of one grid, so they sit further from
/// each other than the days inside them do.
const GAP: f64 = 14.0;

/// How far one month is from the next, which is how far the strip travels to
/// turn a page.
const PITCH: f64 = MONTH + GAP;

/// How far apart the three names sit. They turn with the grid, pulled in to
/// the edges so they can be read, and so travel at a fraction of its rate.
const NAME_PITCH: f64 = 165.0;

/// How far a finger travels before it is dragging the month rather than
/// tapping a day.
const SLOP: f64 = 14.0;

/// How far it has to travel for letting go to turn the month. Short enough to
/// feel like a flick, long enough that a scroll begun over the calendar is not
/// one.
const COMMIT: f64 = 56.0;

/// How long a page takes to come in, and the grid to grow or shrink a row.
const TURN: Duration = Duration::from_millis(280);

/// The month, with a mark on every day a stream lands, run sideways because
/// months do. `swift/Sources/MeridianApp/Ui/Calendar.swift` is what it draws.
///
/// Turning it is a drag rather than a redraw: all three of the machine's months
/// are drawn a page apart and a swipe moves the lot, so only a column of each
/// month either side shows at the edges. Which month is in the middle is the
/// machine's; where the finger has dragged it to is this strip's.
#[component]
pub fn CalendarStrip(
    #[prop(into)] calendar: Signal<ViewCalendar>,
    /// A day was picked, by its id.
    on_pick: Callback<String>,
    /// The month was turned, by −1 or 1.
    on_step: Callback<i32>,
) -> impl IntoView {
    let month = move |pick: fn(&ViewCalendar) -> &ViewCalendarMonth| -> Signal<ViewCalendarMonth> {
        Memo::new_with_compare(
            move |_| calendar.with(|calendar| pick(calendar).clone()),
            |_, _| true,
        )
        .into()
    };

    let weekdays: Signal<Vec<String>> = Memo::new_with_compare(
        move |_| calendar.with(|calendar| calendar.weekdays.clone()),
        |_, _| true,
    )
    .into();

    // Where the finger has dragged the strip to, and how it gets back from
    // there: `settle` springs back from a drag too short to turn anything,
    // `turn` carries the month over.
    let drag = RwSignal::new(0.0_f64);
    let motion = RwSignal::new("");

    let start = StoredValue::new(None::<(f64, f64)>);
    let swiping = StoredValue::new(false);
    let turning = StoredValue::new(false);

    // As tall as the month in the middle, held rather than left to the content
    // so a month with a row more or less is moved to rather than jumped to.
    let middle = NodeRef::<html::Div>::new();
    let height = RwSignal::new(None::<f64>);
    let grown = RwSignal::new(false);

    Effect::new(move |_| {
        let Some(grid) = middle.get() else {
            return;
        };

        let measure = move || {
            let Some(grid) = middle.get_untracked() else {
                return;
            };

            let tall = grid.get_bounding_client_rect().height();
            let held = height.get_untracked();

            if tall > 0.0 && held != Some(tall) {
                // The first month is however tall it is. Every one after that
                // is a month arriving, and the grid grows to it.
                grown.set(held.is_some());
                height.set(Some(tall));
            }
        };

        let callback = Closure::<dyn FnMut()>::new(measure).into_js_value();

        let Ok(observer) = ResizeObserver::new(callback.unchecked_ref()) else {
            return;
        };

        observer.observe(&grid);

        let observer = StoredValue::new_local(observer);

        on_cleanup(move || {
            observer.try_with_value(|observer| observer.disconnect());
        });
    });

    // A drag that ends over a day is not a tap on it.
    let pick = Callback::new(move |id: String| {
        if !swiping.get_value() {
            on_pick.run(id);
        }
    });

    let down = move |event: PointerEvent| {
        if turning.get_value() || event.button() != 0 {
            return;
        }

        start.set_value(Some((
            f64::from(event.client_x()),
            f64::from(event.client_y()),
        )));
        swiping.set_value(false);
    };

    let moved = move |event: PointerEvent| {
        let Some((x, y)) = start.get_value() else {
            return;
        };

        let across = f64::from(event.client_x()) - x;
        let down = f64::from(event.client_y()) - y;

        if !swiping.get_value() {
            if across.abs() < SLOP {
                return;
            }

            // Up and down is the page scrolling, not the month turning.
            if down.abs() > across.abs() {
                start.set_value(None);
                return;
            }

            swiping.set_value(true);

            if let Some(strip) = event
                .current_target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            {
                let _ = strip.set_pointer_capture(event.pointer_id());
            }
        }

        motion.set("");
        drag.set(across);
    };

    let settle = move || {
        motion.set("settle");
        drag.set(0.0);
    };

    let up = move |_: PointerEvent| {
        if start.get_value().is_none() {
            return;
        }

        start.set_value(None);

        if !swiping.get_value() {
            return;
        }

        let travelled = drag.get_untracked();

        if travelled.abs() <= COMMIT {
            settle();
            return;
        }

        let forward = travelled < 0.0;

        turning.set_value(true);
        motion.set("turn");
        drag.set(if forward { -PITCH } else { PITCH });

        // Carry the month over first, and only then tell the machine. The
        // month that was beside is then the middle one, drawn where the
        // beside one has just finished — the same pixels — so the two go in
        // one breath and neither animates.
        set_timeout(
            move || {
                motion.set("");
                on_step.run(if forward { 1 } else { -1 });
                drag.set(0.0);
                turning.set_value(false);
            },
            TURN,
        );
    };

    let cancel = move |_: PointerEvent| {
        if start.get_value().is_some() {
            start.set_value(None);

            if swiping.get_value() {
                settle();
            }
        }
    };

    // Nearest the middle is brightest and largest, falling off with the
    // distance, so a name brightens as it arrives rather than when it lands.
    let name = move |pick: fn(&ViewCalendar) -> &ViewCalendarMonth, rest: f64| {
        let label = move || calendar.with(|calendar| pick(calendar).label.clone());

        let style = move || {
            let x = rest + drag.get() * NAME_PITCH / PITCH;
            let near = (1.0 - x.abs() / NAME_PITCH).max(0.0);

            format!(
                "transform: translateX(calc(-50% + {x:.1}px)) scale({:.3}); opacity: {:.3}",
                0.87 + 0.13 * near,
                0.3 + 0.7 * near
            )
        };

        view! { <span style=style>{label}</span> }
    };

    view! {
        <div
            class="strip"
            style=format!("--month: {MONTH}px; --gap: {GAP}px; --turn: {}ms", TURN.as_millis())
        >
            <div class=move || format!("strip-names {}", motion.get())>
                {name(|calendar| &calendar.prev, -NAME_PITCH)}
                {name(|calendar| &calendar.current, 0.0)}
                {name(|calendar| &calendar.next, NAME_PITCH)}
            </div>

            <div
                class="strip-months"
                class:grown=move || grown.get()
                style=move || height.get().map(|height| format!("height: {height}px")).unwrap_or_default()
                on:pointerdown=down
                on:pointermove=moved
                on:pointerup=up
                on:pointercancel=cancel
            >
                <div
                    class=move || format!("strip-track {}", motion.get())
                    style=move || format!("transform: translateX({:.1}px)", drag.get())
                >
                    <Grid
                        month=month(|calendar| &calendar.prev)
                        weekdays=weekdays
                        on_pick=pick
                        place="prev"
                        grid=NodeRef::new()
                    />
                    <Grid
                        month=month(|calendar| &calendar.current)
                        weekdays=weekdays
                        on_pick=pick
                        place="middle"
                        grid=middle
                    />
                    <Grid
                        month=month(|calendar| &calendar.next)
                        weekdays=weekdays
                        on_pick=pick
                        place="next"
                        grid=NodeRef::new()
                    />
                </div>
            </div>
        </div>
    }
}

/// One month's grid, under the letters of the week.
#[component]
fn Grid(
    month: Signal<ViewCalendarMonth>,
    weekdays: Signal<Vec<String>>,
    on_pick: Callback<String>,
    place: &'static str,
    grid: NodeRef<html::Div>,
) -> impl IntoView {
    let count = move || month.with(|month| month.weeks.iter().map(Vec::len).sum::<usize>());

    // Tuesday and Thursday are both "T", so a heading is keyed by where it
    // sits as well as by what it says.
    let headings = move || weekdays.get().into_iter().enumerate().collect::<Vec<_>>();

    view! {
        <div class=format!("strip-month {place}") node_ref=grid>
            <div class="strip-wd">
                <For each=headings key=|heading| heading.clone() let:heading>
                    <span>{heading.1}</span>
                </For>
            </div>

            // Keyed by where a cell sits and not by which day it is. The
            // months overlap at their edges, and a day keyed by itself would be
            // matched across a change of month and walked from where it was to
            // where it now is.
            <div class="strip-days">
                <For each=move || 0..count() key=|index| *index let:index>
                    <Day month=month index=index on_pick=on_pick />
                </For>
            </div>
        </div>
    }
}

/// One day: plain, marked, today, picked, or outside the month.
#[component]
fn Day(month: Signal<ViewCalendarMonth>, index: usize, on_pick: Callback<String>) -> impl IntoView {
    let with = move |read: fn(&ViewCalendarDay) -> bool| {
        move || month.with(|month| month.weeks.iter().flatten().nth(index).is_some_and(read))
    };

    let label = move || {
        month.with(|month| {
            month
                .weeks
                .iter()
                .flatten()
                .nth(index)
                .map(|day| day.label.clone())
                .unwrap_or_default()
        })
    };

    let marks = move || {
        month.with(|month| {
            month
                .weeks
                .iter()
                .flatten()
                .nth(index)
                .map_or(0, |day| day.marks.len())
        })
    };

    // A day of the month either side is drawn for the shape of the grid, and
    // is not one of this month's to pick.
    let pick = move |_| {
        let day = month.with_untracked(|month| {
            month
                .weeks
                .iter()
                .flatten()
                .nth(index)
                .map(|day| (day.id.clone(), day.outside))
        });

        if let Some((id, false)) = day {
            on_pick.run(id);
        }
    };

    view! {
        <div
            class="cal-day"
            class:outside=with(|day| day.outside)
            class:today=with(|day| day.today)
            class:picked=with(|day| day.picked)
            on:click=pick
        >
            <span class="n">{label}</span>
            <span class="marks">{move || (0..marks()).map(|_| view! { <i></i> }).collect_view()}</span>
        </div>
    }
}
