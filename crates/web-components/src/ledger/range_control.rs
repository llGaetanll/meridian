use client_state::RangeSelection;
use client_state::RangeUnit;
use client_state::ViewRange;
use client_state::ViewRangeKind;
use leptos::prelude::*;
use web_ui::Glyph;

/// What "Last" sends when the count it reads is not a number.
const LAST_COUNT: u32 = 3;

/// The time range: one outline, the four options inside it, the live one
/// filled.
///
/// "Between" is drawn and dead, as it is on the phone: picking a day needs a
/// date picker, and nothing draws one yet.
#[component]
pub fn RangeControl(
    range: Signal<ViewRange>,
    #[prop(into)] on_select: Callback<RangeSelection>,
) -> impl IntoView {
    let on = move |kind: ViewRangeKind| move || range.with(|range| range.kind == kind);

    // The tokens keep their last value while another form is live, so going
    // back to "Last" picks up the count it showed rather than resetting it.
    let last = move |_| {
        let count = range.with(|range| range.last_count.parse().unwrap_or(LAST_COUNT));

        on_select.run(RangeSelection::Last {
            count,
            unit: RangeUnit::Month,
        });
    };

    view! {
        <div class="range">
            <div
                class="opt"
                class:on=on(ViewRangeKind::This)
                on:click=move |_| {
                    on_select
                        .run(RangeSelection::This {
                            unit: RangeUnit::Month,
                        })
                }
            >
                "This"
                <Token text=Signal::derive(move || range.with(|range| range.this_unit.clone())) />
            </div>

            <div class="opt" class:on=on(ViewRangeKind::Last) on:click=last>
                "Last"
                <Token
                    text=Signal::derive(move || range.with(|range| range.last_count.clone()))
                    fixed=true
                />
                <Token text=Signal::derive(move || range.with(|range| range.last_unit.clone())) />
            </div>

            <div class="opt dead" class:on=on(ViewRangeKind::Between)>
                "Between"
                <Token text=Signal::derive(move || range.with(|range| range.between_start.clone())) />
                "and"
                <Token text=Signal::derive(move || range.with(|range| range.between_end.clone())) />
            </div>

            <div
                class="opt"
                class:on=on(ViewRangeKind::All)
                on:click=move |_| on_select.run(RangeSelection::All)
            >
                "All time"
            </div>
        </div>
    }
}

/// One editable word inside an option — "This [month]".
#[component]
fn Token(text: Signal<String>, #[prop(optional)] fixed: bool) -> impl IntoView {
    view! {
        <i class="tok" class:fix=fixed>
            {text}
            <Glyph name="caret" />
        </i>
    }
}
