use client_state::EventUi;
use client_state::ViewAccountBank;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use crate::BankRow;

use super::Runs;
use super::read;
use super::read::part;

/// The address already has an account with banks attached, so first run
/// stands aside: the banks listed as proof, and the app one tap away.
///
/// The rows are the Account tab's, so the two lists read as the same banks.
#[component]
pub(super) fn WelcomeBack(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let title = part(view, |view| read::title(view).to_string());
    let count = part(view, |view| read::banks(view).len());
    let another = part(view, |view| {
        read::aside(view).unwrap_or_default().to_string()
    });
    let open = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });

    view! {
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        <div class="rows">
            <For each=move || 0..count.get() key=|i| *i let:i>
                <BankRow bank=Signal::derive(move || {
                    view.with(|view| read::banks(view).get(i).cloned().unwrap_or_else(blank))
                }) />
            </For>
        </div>

        // The quieter way back onto the road above, the button the screen is
        // about below — the same pair as Ready's.
        <div class="acts">
            <button
                type="button"
                class="btn lg block"
                on:click=move |_| ui.run(EventUi::BankConnectRequested)
            >
                {move || another.get()}
            </button>
            <button
                type="button"
                class="btn primary lg block"
                on:click=move |_| ui.run(EventUi::OnboardingFinished)
            >
                {move || open.get()}
            </button>
        </div>
    }
}

/// A bank with nothing in it, for the moment a render has taken the list
/// away and the row has not yet come down.
fn blank() -> ViewAccountBank {
    ViewAccountBank {
        id: String::new(),
        name: String::new(),
        initials: String::new(),
        detail: String::new(),
        needs_reconnect: false,
    }
}
