//! What somebody sees when the session is good and has not paid.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. Every word on it is the
//! machine's, so the phone and the web say the same thing about one account.

use client_state::EventUi;
use client_state::ViewPaywall;
use client_state::ViewPaywallStep;
use leptos::prelude::*;
use leptos::task::spawn_local;
use web_components::Brand;
use web_components::CardForm;
use web_components::CardWells;
use web_components::IconRow;
use web_ui::Glyph;

use crate::io::stripe;
use crate::runtime_app::RuntimeApp;

#[component]
pub fn PaywallPage(view: Signal<ViewPaywall>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let card =
        Memo::new(move |_| view.with(|view| matches!(view.step, ViewPaywallStep::Card { .. })));

    let paying = Memo::new(move |_| {
        view.with(|view| matches!(view.step, ViewPaywallStep::Card { paying: true, .. }))
    });

    let fault = Memo::new(move |_| {
        view.with(|view| match &view.step {
            ViewPaywallStep::Card { fault, .. } => fault.clone(),
            _ => None,
        })
    });

    let action = Memo::new(move |_| {
        view.with(|view| match &view.step {
            ViewPaywallStep::Offer { action, .. }
            | ViewPaywallStep::Card { action, .. }
            | ViewPaywallStep::Confirming { action, .. } => action.clone(),
        })
    });

    let busy = Memo::new(move |_| {
        view.with(|view| match view.step {
            ViewPaywallStep::Offer { opening, .. } => opening,
            ViewPaywallStep::Card { paying, .. } => paying,
            ViewPaywallStep::Confirming { checking, .. } => checking,
        })
    });

    // Which event the one button sends is which step this is.
    let pressed = move |_| {
        let event = view.with(|view| match view.step {
            ViewPaywallStep::Offer { .. } => EventUi::SubscribeRequested,
            ViewPaywallStep::Card { .. } => EventUi::PaymentRequested,
            ViewPaywallStep::Confirming { .. } => EventUi::RefreshRequested,
        });

        runtime.ui(event);
    };

    // Stripe's frames, in the wells the card form draws.
    let mounted = Callback::new(|wells: CardWells| {
        spawn_local(async move {
            stripe::mount(&wells.number, &wells.expiry, &wells.cvc).await;
        });
    });

    // The card cannot change under a payment that is out.
    Effect::new(move || stripe::lock(paying.get()));

    on_cleanup(stripe::unmount);

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

                // Built once each time the card step begins, and taken down with
                // its frames when it ends: a render that rebuilt it would put
                // new frames in, empty.
                {move || {
                    card.get()
                        .then(|| {
                            on_cleanup(stripe::unmount);

                            view! {
                                <div class="pay">
                                    <CardForm mounted locked=paying fault=fault />
                                </div>
                            }
                        })
                }}

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
                        {move || busy.get().then(|| view! { <Glyph name="spin" /> })}
                        {move || action.get()}
                    </button>
                </div>
            </div>
        </div>
    }
}
