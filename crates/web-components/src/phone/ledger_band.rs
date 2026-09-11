use client_state::ViewFilter;
use leptos::prelude::*;
use web_ui::Glyph;

/// What the filter is set to, once the filter itself has scrolled by: its two
/// halves in its own order, as the app's chips at their smaller size. Pressing
/// it goes back up to the filter.
#[component]
pub fn LedgerBand(
    #[prop(into)] filter: Signal<ViewFilter>,
    on_open: Callback<()>,
) -> impl IntoView {
    let chips = move || filter.with(|filter| filter.categories_summary.clone());
    let more = move || filter.with(|filter| filter.categories_overflow.clone());
    let range = move || filter.with(|filter| filter.range_summary.clone());

    view! {
        <div class="band opens" role="button" on:click=move |_| on_open.run(())>
            <Glyph name="chevron" />

            <div class="cats">
                <For each=chips key=|chip| chip.id.clone() let:chip>
                    <span class="chip sm on">
                        <span class="in">{chip.label}</span>
                    </span>
                </For>
                {move || more().map(|more| view! { <span class="more">{more}</span> })}
            </div>

            <span class="end chip sm">
                <span class="in">{range}</span>
            </span>
        </div>
    }
}
