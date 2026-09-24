use client_state::EventUi;
use client_state::ViewConnections;
use leptos::prelude::*;
use web_components::PhoneConnectionBlocks;

use crate::runtime_app::RuntimeApp;

/// Which logins the app has, as the phone stacks them: one block per bank,
/// with the accounts it brings in under it.
///
/// The frame's title already says the page's name, so what stands at the top
/// here is only the count and the way to add another.
#[component]
pub fn ConnectionsPhone(view: Signal<ViewConnections>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |read: fn(&ViewConnections) -> &String| {
        Signal::derive(move || view.with(|view| read(view).clone()))
    };

    let connections = Signal::derive(move || view.with(|view| view.connections.clone()));
    let empty = Signal::derive(move || view.with(|view| view.empty.clone()));
    let adding = Signal::derive(move || view.with(|view| view.adding));
    let ui = Callback::new(move |event| runtime.ui(event));

    view! {
        <div class="phone-page conn-phone">
            <div class="conn-top">
                <div class="k">{text(|view| &view.count)}</div>

                <button
                    class=move || if adding.get() { "btn busy" } else { "btn" }
                    disabled=move || adding.get()
                    on:click=move |_| runtime.ui(EventUi::BankConnectRequested)
                >
                    {text(|view| &view.add_label)}
                </button>
            </div>

            <Show
                when=move || empty.with(Option::is_none)
                fallback=move || view! { <div class="list-empty">{move || empty.get()}</div> }
            >
                <PhoneConnectionBlocks connections=connections ui=ui />
            </Show>
        </div>
    }
}
