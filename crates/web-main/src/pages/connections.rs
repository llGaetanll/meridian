//! Which logins the app has, and whether they are working.
//!
//! One column and one block per login: the accounts under a bank are the ones
//! it brings in, held and owed together. There is no rail, because a column of
//! shares beside a list of the banks they name was the page said twice.

use client_state::EventUi;
use client_state::ViewConnections;
use leptos::prelude::*;
use web_components::ConfirmRemove;
use web_components::ConnectionBlocks;
use web_ui::Glyph;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn ConnectionsPage(view: Signal<ViewConnections>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |field: fn(&ViewConnections) -> &String| {
        Signal::derive(move || view.with(|v| field(v).clone()))
    };

    let connections = Signal::derive(move || view.with(|v| v.connections.clone()));
    let empty = Signal::derive(move || view.with(|v| v.empty.clone()));
    let listing = move || empty.with(Option::is_none);

    let remove = Signal::derive(move || view.with(|v| v.remove.clone()));
    let adding = Signal::derive(move || view.with(|v| v.adding));
    let ui = Callback::new(move |event| runtime.ui(event));

    // Not the `Hero` every other page opens on: the page's name is the
    // heading here rather than a figure, so it takes no money face and what
    // the banks come to is set under the line rather than tucked over it as a
    // label.
    view! {
        <div class="page solo conn-page">
            <div class="page-col">
                <div class="hero">
                    <div class="hero-row">
                        <div class="v">{text(|v| &v.title)}</div>

                        <div class="hero-acts">
                            <button
                                class=move || {
                                    if adding.get() { "btn primary busy" } else { "btn primary" }
                                }
                                disabled=move || adding.get()
                                on:click=move |_| runtime.ui(EventUi::BankConnectRequested)
                            >
                                <Glyph name=Signal::derive(move || {
                                    if adding.get() { "spin" } else { "plus" }.to_string()
                                }) />
                                {text(|v| &v.add_label)}
                            </button>
                        </div>
                    </div>

                    <div class="k">{text(|v| &v.count)}</div>
                </div>

                <Show
                    when=listing
                    fallback=move || view! { <div class="list-empty">{move || empty.get()}</div> }
                >
                    <div class="accounts">
                        <ConnectionBlocks connections=connections ui=ui />
                    </div>
                </Show>
            </div>

            {move || {
                remove.get().map(|remove| view! { <ConfirmRemove remove=remove ui=ui /> })
            }}
        </div>
    }
}
