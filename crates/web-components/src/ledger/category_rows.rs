use client_state::ViewCategoryChip;
use leptos::html;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::wasm_bindgen::closure::Closure;
use leptos::web_sys::Element;
use leptos::web_sys::ResizeObserver;

use super::chip_pack::pack;

/// The gap between two chips, as `.chip-row` sets it.
const SPACING: f64 = 8.0;

/// The categories, set as a justified paragraph of chips.
///
/// Which line a chip goes on depends on how wide every chip draws, which only
/// the page knows, so the lines are measured rather than received: each chip's
/// width is read off the page and `pack` cuts them into lines. A change of
/// width anywhere — the rail, a count, the font arriving — measures again.
#[component]
pub fn CategoryRows(
    chips: Signal<Vec<ViewCategoryChip>>,
    #[prop(into)] on_toggle: Callback<String>,
) -> impl IntoView {
    let container = NodeRef::<html::Div>::new();
    let packed = RwSignal::new(Vec::<Vec<String>>::new());

    // Every chip on one line until they have been measured, and again whenever
    // the chips stop being the ones that were packed: a chip has to be drawn
    // before it can be measured.
    let lines = Memo::new(move |_| {
        let ids = chips.with(|chips| chips.iter().map(|chip| chip.id.clone()).collect::<Vec<_>>());

        packed.with(|packed| {
            if covers(packed, &ids) {
                packed.clone()
            } else {
                vec![ids]
            }
        })
    });

    let measure = move || {
        let Some(Some(container)) = container.try_get_untracked() else {
            return;
        };

        let Some(ids) = chips.try_with_untracked(|chips| {
            chips.iter().map(|chip| chip.id.clone()).collect::<Vec<_>>()
        }) else {
            return;
        };

        let drawn = drawn_widths(&container);

        let widths: Vec<f64> = ids
            .iter()
            .map(|id| {
                drawn
                    .iter()
                    .find(|(drawn, _)| drawn == id)
                    .map_or(0.0, |(_, width)| *width)
            })
            .collect();

        let next: Vec<Vec<String>> = pack(
            &widths,
            container.get_bounding_client_rect().width(),
            SPACING,
        )
        .into_iter()
        .map(|line| line.into_iter().map(|index| ids[index].clone()).collect())
        .collect();

        if packed.try_with_untracked(|packed| *packed != next) == Some(true) {
            packed.set(next);
        }
    };

    // A fresh observer on every redraw, because a chip that moved line is a new
    // element and the old one is gone. Observing reports a size straight away,
    // which is what takes the first measurement.
    Effect::new(move |_| {
        // Read, not tracked: `track` does not compute a memo, so an effect
        // that only tracks one never runs again.
        lines.with(|_| ());

        let Some(container) = container.get() else {
            return;
        };

        let callback = Closure::<dyn FnMut()>::new(measure).into_js_value();

        let Ok(observer) = ResizeObserver::new(callback.unchecked_ref()) else {
            return;
        };

        observer.observe(&container);

        for chip in chips_in(&container) {
            observer.observe(&chip);
        }

        let observer = StoredValue::new_local(observer);

        on_cleanup(move || {
            observer.try_with_value(|observer| observer.disconnect());
        });
    });

    view! {
        <div class="chip-rows" node_ref=container>
            {move || {
                let lines = lines.get();
                let count = lines.len();

                lines
                    .into_iter()
                    .enumerate()
                    .map(|(index, line)| {
                        // Every line but the last is flush at both edges, and
                        // never a line of one: a lone chip stretched across the
                        // measure is not a row.
                        let flush = index + 1 < count && line.len() > 1;

                        view! {
                            <div class="chip-row" class:flush=flush>
                                {line
                                    .into_iter()
                                    .map(|id| {
                                        view! { <CategoryChip id=id chips=chips on_toggle=on_toggle /> }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

/// One category, reading what it shows from `chips` by its id, so a render
/// that changes its count or its selection does not rebuild it.
#[component]
fn CategoryChip(
    id: String,
    chips: Signal<Vec<ViewCategoryChip>>,
    on_toggle: Callback<String>,
) -> impl IntoView {
    let chip = {
        let id = id.clone();

        Signal::derive(move || chips.with(|chips| chips.iter().find(|chip| chip.id == id).cloned()))
    };

    let text = move |read: fn(&ViewCategoryChip) -> &str| {
        move || {
            chip.with(|chip| {
                chip.as_ref()
                    .map(|chip| read(chip).to_string())
                    .unwrap_or_default()
            })
        }
    };

    let flag = move |read: fn(&ViewCategoryChip) -> bool| {
        move || chip.with(|chip| chip.as_ref().is_some_and(read))
    };

    view! {
        <button
            type="button"
            class="chip"
            class:on=flag(|chip| chip.selected)
            class:empty=flag(|chip| chip.empty)
            data-id=id.clone()
            on:click=move |_| on_toggle.run(id.clone())
        >
            <span class="in">
                <span class="n">{text(|chip| &chip.count)}</span>
                <span class="lb" data-label=text(|chip| &chip.label)>
                    {text(|chip| &chip.label)}
                </span>
            </span>
        </button>
    }
}

/// Whether a packing holds exactly these chips, whatever order it put them in.
fn covers(packed: &[Vec<String>], ids: &[String]) -> bool {
    let mut held: Vec<&String> = packed.iter().flatten().collect();
    let mut wanted: Vec<&String> = ids.iter().collect();

    held.sort();
    wanted.sort();

    held == wanted
}

fn chips_in(container: &Element) -> Vec<Element> {
    let Ok(nodes) = container.query_selector_all(".chip") else {
        return Vec::new();
    };

    (0..nodes.length())
        .filter_map(|index| nodes.item(index))
        .filter_map(|node| node.dyn_into::<Element>().ok())
        .collect()
}

/// Each chip's id, and how wide it draws.
fn drawn_widths(container: &Element) -> Vec<(String, f64)> {
    chips_in(container)
        .into_iter()
        .filter_map(|chip| {
            let id = chip.get_attribute("data-id")?;

            Some((id, chip.get_bounding_client_rect().width()))
        })
        .collect()
}
