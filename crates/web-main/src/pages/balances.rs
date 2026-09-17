//! Where the money sits.
//!
//! The headline is what you hold, not net worth: Plaid reports the mortgage
//! but never the house behind it, so a net figure is structurally too red.
//! What is owed gets its own section under it, and the rail says how much of
//! each half every bank is holding.

use client_state::EventUi;
use client_state::ViewBalances;
use client_state::ViewConnection;
use leptos::prelude::*;
use web_components::ConnectionBlocks;
use web_components::Hero;
use web_components::MaskButton;
use web_components::Sec;
use web_components::Split;
use web_ui::Glyph;
use web_ui::Tone;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn BalancesPage(view: Signal<ViewBalances>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |field: fn(&ViewBalances) -> &String| {
        Signal::derive(move || view.with(|v| field(v).clone()))
    };

    let held = side(view, false);
    let owed = side(view, true);
    let held_split = Signal::derive(move || view.with(|v| v.held_split.clone()));
    let owed_split = Signal::derive(move || view.with(|v| v.owed_split.clone()));

    let masked = Signal::derive(move || view.with(|v| v.masked));
    let toggle = Callback::new(move |()| runtime.ui(EventUi::MaskToggled));
    let ui = Callback::new(move |event| runtime.ui(event));

    view! {
        <div class="page">
            <div class="page-col">
                <Hero label=text(|v| &v.headline_label) value=text(|v| &v.headline)>
                    <div class="hero-acts">
                        <MaskButton masked=masked label=text(|v| &v.mask_label) on_toggle=toggle />
                        // Drawn and not built: outside first run the machine
                        // has no event that opens Plaid.
                        <button class="btn primary dead" disabled>
                            <Glyph name="plus" />
                            "Add a bank"
                        </button>
                    </div>
                </Hero>

                <div class="accounts">
                    <ConnectionBlocks connections=held ui=ui />
                </div>

                <Show when=move || owed.with(|owed| !owed.is_empty())>
                    <Sec title=text(|v| &v.owed_label) value=text(|v| &v.owed_total) tone=Tone::Down>
                        <ConnectionBlocks connections=owed ui=ui />
                    </Sec>
                </Show>
            </div>

            <aside class="rail">
                <div class="rail-sec">
                    <div class="rail-head">
                        <span class="rail-title">{text(|v| &v.held_split_label)}</span>
                        <span class="rail-title">
                            <span class="v money">{text(|v| &v.headline)}</span>
                        </span>
                    </div>
                    <Split slices=held_split />
                </div>

                <Show when=move || owed_split.with(|split| !split.is_empty())>
                    <div class="rail-sec">
                        <div class="rail-head">
                            <span class="rail-title">{text(|v| &v.owed_split_label)}</span>
                            <span class="rail-title">
                                <span class="v money down">{text(|v| &v.owed_total)}</span>
                            </span>
                        </div>
                        <Split slices=owed_split />
                    </div>
                </Show>
            </aside>
        </div>
    }
}

/// The blocks on one side of the page: what is held, or what is owed.
fn side(view: Signal<ViewBalances>, debts: bool) -> Signal<Vec<ViewConnection>> {
    Signal::derive(move || {
        view.with(|v| {
            v.connections
                .iter()
                .filter(|c| c.is_debts == debts)
                .cloned()
                .collect()
        })
    })
}
