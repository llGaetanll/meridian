use client_state::ViewBalances;
use client_state::ViewConnection;
use leptos::prelude::*;
use web_components::Hero;
use web_components::PhoneConnectionBlocks;
use web_components::Sec;
use web_ui::Tone;

/// Where the money sits, as the phone draws it: what is held under the figure
/// it comes to, and what is owed in a section of its own.
///
/// The headline is what you hold, not net worth: Plaid reports the mortgage
/// but never the house behind it, so a net figure is structurally too red.
#[component]
pub fn BalancesPhone(view: Signal<ViewBalances>) -> impl IntoView {
    let text = move |read: fn(&ViewBalances) -> &String| {
        Signal::derive(move || view.with(|view| read(view).clone()))
    };

    let held = side(view, false);
    let owed = side(view, true);

    view! {
        <div class="phone-page balances-phone">
            <div data-band-anchor="">
                <Hero label=text(|view| &view.headline_label) value=text(|view| &view.headline) />
            </div>

            <PhoneConnectionBlocks connections=held />

            <Show when=move || owed.with(|owed| !owed.is_empty())>
                <Sec title=text(|view| &view.owed_label) value=text(|view| &view.owed_total) tone=Tone::Down>
                    <PhoneConnectionBlocks connections=owed />
                </Sec>
            </Show>
        </div>
    }
}

/// The blocks on one side of the page: what is held, or what is owed.
fn side(view: Signal<ViewBalances>, debts: bool) -> Signal<Vec<ViewConnection>> {
    Signal::derive(move || {
        view.with(|view| {
            view.connections
                .iter()
                .filter(|connection| connection.is_debts == debts)
                .cloned()
                .collect()
        })
    })
}
