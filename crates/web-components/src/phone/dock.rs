use client_state::Screen;
use client_state::ViewDock;
use client_state::ViewDockItem;
use leptos::prelude::*;
use web_ui::Glyph;

/// The machine's tabs on a bar at the foot of the phone, and search on a
/// square of its own beside it. Glyphs only: each tab keeps its name for
/// anything that reads the page aloud, and the stylesheet clips it.
///
/// Which tab is lit is the machine's answer. The lit ground is one tile that
/// slides to whichever tab that is, rather than one per tab coming and going.
#[component]
pub fn Dock(
    #[prop(into)] dock: Signal<Option<ViewDock>>,
    on_screen: Callback<Screen>,
) -> impl IntoView {
    let items = move || {
        dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| dock.items.clone())
                .collect::<Vec<_>>()
        })
    };

    let count =
        Memo::new(move |_| dock.with(|dock| dock.as_ref().map_or(0, |dock| dock.items.len())));

    let lit = Memo::new(move |_| {
        dock.with(|dock| {
            dock.as_ref()
                .and_then(|dock| dock.items.iter().position(|item| item.selected))
        })
    });

    view! {
        <div class="dockrow">
            <nav class="dock">
                <div class="tabs" style=move || format!("--tabs: {}", count.get())>
                    <span
                        class="pill"
                        class:none=move || lit.get().is_none()
                        style=move || format!("--lit: {}", lit.get().unwrap_or_default())
                    />

                    // Keyed on the tab and not the whole item: `selected`
                    // changes on every tab, and keying on it would make each
                    // one a new button whenever the selection moved.
                    <For
                        each=items
                        key=|item| item.label.clone()
                        children=move |item: ViewDockItem| {
                            let screen = item.screen;
                            let label = item.label.clone();

                            let on = move || {
                                dock.with(|dock| {
                                    dock.iter()
                                        .flat_map(|dock| &dock.items)
                                        .any(|shown| shown.label == label && shown.selected)
                                })
                            };

                            view! {
                                <button
                                    type="button"
                                    class="tab"
                                    class:on=on
                                    on:click=move |_| on_screen.run(screen)
                                >
                                    <Glyph name=item.icon />
                                    <span>{item.label}</span>
                                </button>
                            }
                        }
                    />
                </div>
            </nav>

            // Drawn and dead, as it is on the phone: nothing has drawn the
            // screen behind it.
            <button type="button" class="dock-search" disabled aria-label="Search">
                <Glyph name="search" />
            </button>
        </div>
    }
}
