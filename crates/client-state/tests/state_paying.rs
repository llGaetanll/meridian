//! Paying from the paywall, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`.
//!
//! A flow, so a file. Pay by card opens Stripe's own page in a window of its
//! own, the money is taken there or not at all, and the app opens once the
//! server has heard from Stripe — which nothing here is told, so the server
//! is asked until it has, for a minute, and then on a button.

use api_eval::eval;
use client_state::*;

/// 2026-08-19 00:00 UTC. It reaches the machine as a tick and nowhere else.
const NOW: u64 = 1_787_097_600;

/// A signed-in machine the server has refused for never having subscribed,
/// which is where every test here starts. What the paywall says for that
/// refusal is `state_app.rs`'s claim.
fn paywall() -> StateApp {
    let mut state = StateApp::new();

    state.handle_events(
        &[
            Event::FromRuntime {
                event: EventRuntime::Tick { now: NOW },
            },
            Event::FromSource {
                event: EventSource::SessionLoaded { signed_in: true },
            },
            Event::FromSource {
                event: EventSource::LoadFailed {
                    failure: LoadFailure::Unpaid {
                        reason: Unpaid::Never,
                    },
                },
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// The same machine, served: the snapshot arrived and there is no paywall.
fn served() -> StateApp {
    let mut state = StateApp::new();

    state.handle_events(
        &[
            Event::FromRuntime {
                event: EventRuntime::Tick { now: NOW },
            },
            Event::FromSource {
                event: EventSource::SessionLoaded { signed_in: true },
            },
            Event::FromSource {
                event: EventSource::SnapshotLoaded {
                    json: "{}".to_string(),
                },
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// The whole of it: Stripe's page, the sheet taking the money, and the
/// snapshot that opens the app once the server has heard.
#[test]
fn paying_on_stripes_page_opens_the_app() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            let ViewPaywallStep::Offer { action, opening } = &view.step else {
                panic!("still the offer, waiting: {:?}", view.step);
            };

            assert_eq!(action, "Opening Stripe…");
            assert!(opening);
        },

        // Stripe's window is up, and the flow waits behind it on a screen of
        // its own: none of the offer, and no way anywhere but the window.
        < Event::FromSource { event: EventSource::CheckoutOpened },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert_eq!(view.title, "Finish paying in Stripe's window");
            assert!(view.rows.is_empty(), "none of the offer: {:?}", view.rows);
            assert_eq!(view.sign_out, None);

            let ViewPaywallStep::Offer { action, opening } = &view.step else {
                panic!("the one busy button: {:?}", view.step);
            };

            assert_eq!(action, "Waiting on Stripe…");
            assert!(opening);
        },

        // Taken is not served: the server has not heard yet, so it is asked.
        < Event::FromSource { event: EventSource::PaymentTaken },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert_eq!(view.title, "Payment received");
        },

        < Event::FromSource { event: EventSource::SnapshotLoaded { json: "{}".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, dock: Some(_), .. } },
    );
}

/// Stripe's page closing without taking anything is the offer again, ready
/// for another press.
#[test]
fn a_page_closed_without_paying_offers_again() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::CheckoutOpened },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::CheckoutClosed },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            let ViewPaywallStep::Offer { action, opening } = &view.step else {
                panic!("the offer again: {:?}", view.step);
            };

            assert_eq!(action, "Pay by card");
            assert!(!opening);
        },

        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// A second press while the first is still out asks for nothing. The button
/// says it is waiting; a press that lands anyway is not a second window.
#[test]
fn a_press_while_waiting_asks_for_nothing() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The server not answering while Stripe's page is asked for puts Pay by
/// card back, with the reason in the banner.
#[test]
fn a_page_that_did_not_open_can_be_asked_for_again() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::LoadFailed { failure: LoadFailure::Offline } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert!(matches!(view.step, ViewPaywallStep::Offer { opening: false, .. }));
        },
        > Action::ToUi { action: ActionUi::ShowError { .. } },

        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// Once the sheet took the money the server is asked again every three
/// seconds, and a refusal in the meantime is the wait going on rather than
/// news.
#[test]
fn the_server_is_asked_until_it_has_heard() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::CheckoutOpened },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::PaymentTaken },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::LoadFailed { failure: LoadFailure::Unpaid { reason: Unpaid::Unconfirmed } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert_eq!(view.title, "Payment received");
        },

        // Two seconds on is too soon, and three is not: one ask between them.
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 2 } },
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 3 } },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
    );
}

/// A minute of asking is enough. After it the server is asked when somebody
/// presses Check again, which is a refresh — and pressing it asks on its own
/// for another minute.
#[test]
fn after_a_minute_asking_is_a_button() {
    let mut state = paywall();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToSource { action: ActionSource::StartCheckout },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::CheckoutOpened },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::PaymentTaken },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 60 } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert_eq!(view.title, "We can't confirm your payment yet");

            let ViewPaywallStep::Confirming { action, checking } = &view.step else {
                panic!("still confirming: {:?}", view.step);
            };

            assert_eq!(action, "Check again");
            assert!(!checking);
        },

        < Event::FromUi { event: EventUi::RefreshRequested },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert!(matches!(view.step, ViewPaywallStep::Confirming { checking: true, .. }));
        },
    );
}

/// Nothing is sold to a session the server is serving.
#[test]
fn a_served_session_is_offered_nothing() {
    let mut state = served();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::SubscribeRequested },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, .. } },
    );
}
