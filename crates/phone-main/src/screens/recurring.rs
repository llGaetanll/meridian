//! What comes back: the monthly commitment, split by category, then every
//! stream grouped the same way.

use leptos::prelude::*;
use phone_state::ViewCalendar;
use phone_state::ViewRecurring;

use crate::ui::Breakdown;
use crate::ui::Masked;
use crate::ui::PhoneShell;
use crate::ui::StreamRowView;

#[component]
pub fn RecurringScreen(view: ViewRecurring) -> impl IntoView {
    let groups = view.groups.clone();

    view! {
        <PhoneShell title="Recurring">
            <div class="rec-hero">
                <div class="v num">
                    <Masked value=view.headline masked=view.masked tappable=true />
                    <span class="per">"/mo"</span>
                </div>
                <div class="d faint">{view.detail}</div>
            </div>

            <Breakdown slices=view.breakdown legend=true />

            <CalendarGrid calendar=view.calendar />

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

/// The month, with a mark on every day a stream lands.
///
/// Six rows of seven, days either side of the month included — the grid is the
/// core's, and this places cells and colours marks.
#[component]
fn CalendarGrid(calendar: ViewCalendar) -> impl IntoView {
    let weekdays = calendar.weekdays.clone();
    let weeks = calendar.weeks.clone();

    view! {
        <div class="cal">
            <div class="cal-month">{calendar.month}</div>

            <div class="cal-grid">
                // Tuesday and Thursday are both "T", so the headings are keyed
                // by where they sit rather than by what they say.
                {weekdays
                    .into_iter()
                    .enumerate()
                    .map(|(index, day)| {
                        view! { <div class="cal-wd" data-col=index>{day}</div> }
                    })
                    .collect_view()}

                {weeks
                    .into_iter()
                    .flatten()
                    .enumerate()
                    .map(|(index, day)| {
                        let mut class = "cal-day".to_string();
                        if day.outside {
                            class.push_str(" outside");
                        }
                        if day.today {
                            class.push_str(" today");
                        }

                        view! {
                            <div class=class data-cell=index>
                                <span class="n">{day.label}</span>
                                <span class="marks">
                                    {day
                                        .marks
                                        .into_iter()
                                        .map(|token| view! { <i class=token></i> })
                                        .collect_view()}
                                </span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}
