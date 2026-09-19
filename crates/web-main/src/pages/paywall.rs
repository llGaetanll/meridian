//! What somebody sees when the session is good and has not paid.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. Every word on it is the
//! machine's, so the phone and the web say the same thing about one account.
//!
//! No card form: Pay by card opens Stripe's own page in a window of its own,
//! so the money is taken there and no card detail ever touches this app.

use client_state::EventUi;
use client_state::ViewPaywall;
use client_state::ViewPaywallStep;
use leptos::prelude::*;
use web_components::Brand;
use web_components::IconRow;
use web_ui::Glyph;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn PaywallPage(view: Signal<ViewPaywall>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let action = Memo::new(move |_| {
        view.with(|view| match &view.step {
            ViewPaywallStep::Offer { action, .. } | ViewPaywallStep::Confirming { action, .. } => {
                action.clone()
            }
        })
    });

    let busy = Memo::new(move |_| {
        view.with(|view| match view.step {
            ViewPaywallStep::Offer { opening, .. } => opening,
            ViewPaywallStep::Confirming { checking, .. } => checking,
        })
    });

    // Which event the one button sends is which step this is. It runs to the
    // end inside the click, which is what lets Stripe's window open: a
    // browser allows one only from inside the click that asked for it.
    let pressed = move |_| {
        let event = view.with(|view| match view.step {
            ViewPaywallStep::Offer { .. } => EventUi::SubscribeRequested,
            ViewPaywallStep::Confirming { .. } => EventUi::RefreshRequested,
        });

        runtime.ui(event);
    };

    view! {
        <div class="paywall ui">
            <div class="measure">
                <Brand />

                <div class="say-title">{move || view.with(|view| view.title.clone())}</div>
                <div class="say-body">
                    {move || {
                        view.with(|view| {
                            view.body
                                .iter()
                                .map(|run| match run.strong {
                                    true => view! { <b>{run.text.clone()}</b> }.into_any(),
                                    false => run.text.clone().into_any(),
                                })
                                .collect_view()
                        })
                    }}
                </div>

                <div class="facts">
                    // Redrawn whole: they change only when the reason does.
                    {move || {
                        view.with(|view| {
                            view.rows
                                .iter()
                                .map(|row| view! { <IconRow row=row.clone() /> })
                                .collect_view()
                        })
                    }}
                </div>

                <div class="acts">
                    <button
                        class="btn lg block"
                        on:click=move |_| runtime.ui(EventUi::SignOutRequested)
                    >
                        {move || view.with(|view| view.sign_out.clone())}
                    </button>

                    <button
                        type="button"
                        class=move || {
                            if busy.get() { "btn primary lg block busy" } else { "btn primary lg block" }
                        }
                        disabled=move || busy.get()
                        on:click=pressed
                    >
                        {move || {
                            if busy.get() {
                                Some(view! { <Glyph name="spin" /> }.into_any())
                            } else if view
                                .with(|view| matches!(view.step, ViewPaywallStep::Offer { .. }))
                            {
                                Some(view! { <Glyph name="card" /> }.into_any())
                            } else {
                                None
                            }
                        }}
                        {move || action.get()}
                    </button>
                </div>
            </div>
        </div>
    }
}
