//! First run, tested the way `docs/STYLE.md` says: events in with `<`, actions
//! out with `>`.
//!
//! This is the one test file that does *not* say who is signed in. The app
//! assumes a device it has never run on, so a machine straight out of
//! `StateApp::new()` is already on the first screen of the flow — which is
//! what these drive.
//!
//! `eval!` covers the actions; the assertions after it cover the screens those
//! actions carry.

use std::time::Duration;

use api_common::Timestamp;
use api_eval::eval;
use phone_state::*;

/// 2026-08-19 00:00 UTC, the same clock the rest of the tests are set to.
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

const EMAIL: &str = "al@example.com";

const BANK: &str = "First Platypus Bank";

/// A machine on a device the app has never run on, with its clock set.
fn first_run() -> StateApp {
    let mut state = StateApp::new();
    state.set_now(NOW);
    state
}

fn ui(event: EventUi) -> Event {
    Event::FromUi { event }
}

fn from(event: EventSource) -> Event {
    Event::FromSource { event }
}

fn typed(text: &str) -> Event {
    ui(EventUi::EmailTyped {
        text: text.to_string(),
    })
}

fn code(digits: &str) -> Event {
    ui(EventUi::CodeTyped {
        digits: digits.to_string(),
    })
}

/// Drive the machine and hand back the screen the last action carried.
fn screen(state: &mut StateApp, events: &[Event]) -> ViewScreen {
    let mut actions = Vec::new();

    state.handle_events(state.now(), events, &mut actions);

    actions
        .iter()
        .rev()
        .find_map(|a| match a {
            Action::ToUi {
                action: ActionUi::Render { screen, .. },
            } => Some(screen.clone()),
            _ => None,
        })
        .expect("something should have been rendered")
}

fn flow(state: &mut StateApp, events: &[Event]) -> ViewOnboarding {
    match screen(state, events) {
        ViewScreen::Onboarding { view } => view,
        other => panic!("expected first run, got {other:?}"),
    }
}

/// Everything up to the code screen, with a code already sent.
fn at_the_code_screen() -> StateApp {
    let mut state = first_run();

    screen(
        &mut state,
        &[
            ui(EventUi::EmailChosen),
            typed(EMAIL),
            ui(EventUi::EmailSubmitted),
            from(EventSource::CodeSent),
        ],
    );

    state
}

/// Connecting a bank is what starts the history arriving, so the ask follows
/// the news rather than the runtime deciding a sync must be next.
#[test]
fn a_linked_bank_asks_for_its_history() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < ui(EventUi::BankConnectRequested),
        > Action::ToSource { action: ActionSource::StartBankLink },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < from(EventSource::BankLinked { institution: BANK.to_string() }),
        > Action::ToSource { action: ActionSource::WatchSync },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// Everything up to the bank screen, which is the one the flow exists for.
fn at_the_bank_screen() -> StateApp {
    let mut state = at_the_code_screen();

    screen(
        &mut state,
        &[code("407193"), from(EventSource::CodeAccepted)],
    );

    state
}

#[test]
fn a_device_the_app_has_never_run_on_opens_on_welcome() {
    let mut state = first_run();

    let mut actions = Vec::new();
    state.handle_events(NOW, &[ui(EventUi::Started)], &mut actions);

    let rendered = actions.iter().find_map(|a| match a {
        Action::ToUi {
            action: ActionUi::Render { screen, dock },
        } => Some((screen.clone(), dock.clone())),
        _ => None,
    });

    let Some((ViewScreen::Onboarding { view }, dock)) = rendered else {
        panic!("first run should be what a new machine draws, got {rendered:?}")
    };

    assert!(
        matches!(view, ViewOnboarding::Welcome { .. }),
        "and it opens on the first screen of it",
    );

    // Its screens are not tabs. A dock under them would offer three empty ones.
    assert!(dock.is_none(), "first run has no dock");
}

/// No session means the machine stays where it already is. It is the *only*
/// answer that changes nothing, which is why it is worth pinning.
#[test]
fn no_session_leaves_the_flow_where_it_is() {
    let mut state = first_run();

    eval!(&mut state,
        < from(EventSource::SessionLoaded { email: None }),
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);

    assert!(matches!(view, ViewOnboarding::Welcome { .. }));
}

#[test]
fn an_address_that_is_not_one_is_never_sent() {
    let mut state = first_run();

    eval!(&mut state,
        < ui(EventUi::EmailChosen),
        > Action::ToUi { action: ActionUi::Render { .. } },

        // Nothing goes out, and the only thing that happens is the field
        // saying so.
        < typed("al@example"),
        < ui(EventUi::EmailSubmitted),
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);

    let ViewOnboarding::Email { field, .. } = view else {
        panic!("a rejected address leaves you on the address screen, got {view:?}")
    };

    assert!(field.fault.is_some(), "the field says what is wrong");
    assert_eq!(field.value, "al@example", "and keeps what was typed");
}

/// A field that stayed red while it was being fixed would be arguing with the
/// user.
#[test]
fn typing_clears_the_rejection() {
    let mut state = first_run();

    let view = flow(
        &mut state,
        &[
            ui(EventUi::EmailChosen),
            typed("al@example"),
            ui(EventUi::EmailSubmitted),
            typed("al@example.c"),
        ],
    );

    let ViewOnboarding::Email { field, .. } = view else {
        panic!("expected the address screen, got {view:?}")
    };

    assert!(field.fault.is_none());
}

#[test]
fn a_good_address_asks_for_a_code() {
    let mut state = first_run();

    eval!(&mut state,
        < ui(EventUi::EmailChosen),
        > Action::ToUi { action: ActionUi::Render { .. } },

        // Typing draws, and only Continue sends.
        < typed(EMAIL),
        < ui(EventUi::EmailSubmitted),
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::SendCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The address is on screen and one tap from being fixed — a typo in the step
/// before is otherwise a dead end.
#[test]
fn the_code_screen_says_where_the_code_went() {
    let mut state = at_the_code_screen();

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);

    let ViewOnboarding::Code { body, cells, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    assert!(
        body.iter().any(|run| run.text == EMAIL && run.strong),
        "the address is set strong in the sentence: {body:?}",
    );

    assert_eq!(cells.len(), 6, "a box per digit says how many are left");
    assert!(cells[0].live, "and the first one takes the next digit");
}

/// There is nothing to check before the sixth digit, so the sixth digit
/// submits and there is no Continue button on that screen.
#[test]
fn the_sixth_digit_submits() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < code("40719"),
        > Action::ToUi { action: ActionUi::Render { .. } },

        < code("407193"),
        > Action::ToSource { action: ActionSource::VerifyCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// A rejected code is one wrong value, not six wrong digits.
#[test]
fn a_wrong_code_reddens_every_box() {
    let mut state = at_the_code_screen();

    let view = flow(
        &mut state,
        &[
            code("407193"),
            from(EventSource::CodeRejected { expired: false }),
        ],
    );

    let ViewOnboarding::Code { cells, fault, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    assert!(cells.iter().all(|cell| cell.bad), "{cells:?}");
    assert!(cells.iter().all(|cell| !cell.live), "and none is waiting");
    assert_eq!(fault.as_deref(), Some("That code is not right."));
}

/// Wrong and too late are different problems, one sentence apart, and the way
/// out of each is different.
#[test]
fn an_expired_code_says_so_and_offers_a_new_one() {
    let mut state = at_the_code_screen();

    let view = flow(
        &mut state,
        &[
            code("407193"),
            from(EventSource::CodeRejected { expired: true }),
        ],
    );

    let ViewOnboarding::Code { fault, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    assert_eq!(
        fault.as_deref(),
        Some("That code has expired. They last ten minutes."),
    );
}

#[test]
fn the_resend_counts_down_before_it_offers() {
    let mut state = at_the_code_screen();

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);
    let ViewOnboarding::Code { resend, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    let ViewResend::Waiting { text, .. } = resend else {
        panic!("a code just went out, so it is still counting: {resend:?}")
    };

    assert_eq!(text, "Resend in 1:00");

    state.set_now(NOW + Duration::from_secs(42));

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);
    let ViewOnboarding::Code { resend, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    assert!(
        matches!(&resend, ViewResend::Waiting { text, .. } if text == "Resend in 0:18"),
        "{resend:?}",
    );

    state.set_now(NOW + Duration::from_secs(60));

    let view = flow(&mut state, &[ui(EventUi::Ticked)]);
    let ViewOnboarding::Code { resend, .. } = view else {
        panic!("expected the code screen, got {view:?}")
    };

    assert!(
        matches!(resend, ViewResend::Ready { .. }),
        "once there is nothing to wait for it is a button",
    );
}

/// The countdown is what stops a second code going out, so the event has to be
/// ignored rather than merely un-drawn.
#[test]
fn resending_early_sends_nothing() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < ui(EventUi::ResendRequested),
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    state.set_now(NOW + Duration::from_secs(60));

    eval!(&mut state,
        < ui(EventUi::ResendRequested),
        > Action::ToSource { action: ActionSource::SendCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

#[test]
fn wrong_address_goes_back_to_the_field_with_it_still_in() {
    let mut state = at_the_code_screen();

    let view = flow(&mut state, &[code("407"), ui(EventUi::BackPressed)]);

    let ViewOnboarding::Email { field, .. } = view else {
        panic!("back from the code screen is the address screen, got {view:?}")
    };

    assert_eq!(field.value, EMAIL, "and the address is still in it");
}

#[test]
fn a_verified_code_asks_for_a_bank() {
    let mut state = at_the_code_screen();

    let view = flow(
        &mut state,
        &[code("407193"), from(EventSource::CodeAccepted)],
    );

    assert!(matches!(view, ViewOnboarding::Bank { .. }), "{view:?}");
}

/// There is no version of this app without a bank in it — every screen is a
/// view of transactions — so nothing here offers a way past this one, back
/// included.
#[test]
fn there_is_no_way_back_out_of_the_bank_screen() {
    let mut state = at_the_bank_screen();

    let view = flow(&mut state, &[ui(EventUi::BackPressed)]);

    assert!(matches!(view, ViewOnboarding::Bank { .. }), "{view:?}");
}

#[test]
fn connecting_a_bank_opens_plaid() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < ui(EventUi::BankConnectRequested),
        > Action::ToSource { action: ActionSource::StartBankLink },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The common ending, not a rare one. Nothing was created, and the screen says
/// exactly that.
#[test]
fn an_abandoned_sign_in_says_nothing_was_connected() {
    let mut state = at_the_bank_screen();

    let view = flow(
        &mut state,
        &[
            ui(EventUi::BankConnectRequested),
            from(EventSource::BankAbandoned),
        ],
    );

    assert!(matches!(view, ViewOnboarding::Failed { .. }), "{view:?}");

    // And the way out of it is the same road, not a different one.
    let view = flow(&mut state, &[ui(EventUi::BankConnectRequested)]);

    assert!(matches!(view, ViewOnboarding::Bank { .. }), "{view:?}");
}

#[test]
fn the_sync_says_what_has_landed_so_far() {
    let mut state = at_the_bank_screen();

    let view = flow(
        &mut state,
        &[
            ui(EventUi::BankConnectRequested),
            from(EventSource::BankLinked {
                institution: BANK.to_string(),
            }),
            from(EventSource::SyncProgressed {
                stage: SyncStage::Transactions,
                accounts: 8,
                transactions: 48,
                streams: 0,
            }),
        ],
    );

    let ViewOnboarding::Sync { progress, rows, .. } = view else {
        panic!("a linked bank puts the flow on the sync screen, got {view:?}")
    };

    assert_eq!(rows[0].title, BANK, "the bank it is reading");
    assert_eq!(rows[2].value.as_deref(), Some("48 so far"));
    assert!(progress > 0.0 && progress < 1.0, "{progress}");
}

#[test]
fn a_finished_sync_counts_what_arrived() {
    let mut state = at_the_bank_screen();

    let view = flow(
        &mut state,
        &[
            ui(EventUi::BankConnectRequested),
            from(EventSource::BankLinked {
                institution: BANK.to_string(),
            }),
            from(EventSource::SyncProgressed {
                stage: SyncStage::Streams,
                accounts: 8,
                transactions: 48,
                streams: 8,
            }),
            from(EventSource::SyncFinished),
        ],
    );

    let ViewOnboarding::Ready { tally, body, .. } = view else {
        panic!("a finished sync is the last screen, got {view:?}")
    };

    assert_eq!(tally[0].value, "8");
    assert_eq!(tally[0].label, "accounts");
    assert_eq!(tally[1].value, "48");
    assert_eq!(tally[1].label, "transactions");
    assert_eq!(tally[2].value, "8");

    assert!(
        body.iter().any(|run| run.text == BANK),
        "and it names the bank: {body:?}",
    );
}

/// The end of the road: the flow is dropped, the address is remembered so the
/// next launch skips it, and the app goes and gets its data.
#[test]
fn opening_meridian_ends_the_flow() {
    let mut state = at_the_bank_screen();

    screen(
        &mut state,
        &[
            ui(EventUi::BankConnectRequested),
            from(EventSource::BankLinked {
                institution: BANK.to_string(),
            }),
            from(EventSource::SyncFinished),
        ],
    );

    eval!(&mut state,
        < ui(EventUi::OnboardingFinished),
        > Action::ToSource { action: ActionSource::SaveSession { .. } },
        > Action::ToSource { action: ActionSource::LoadSnapshot { force: false } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    let mut actions = Vec::new();
    state.handle_events(state.now(), &[ui(EventUi::Ticked)], &mut actions);

    let Some(Action::ToUi {
        action: ActionUi::Render { screen, dock },
    }) = actions.first()
    else {
        panic!("expected a render, got {actions:?}")
    };

    assert!(
        matches!(screen, ViewScreen::Ledger { .. }),
        "the app opens on the Ledger, got {screen:?}",
    );

    assert!(dock.is_some(), "and the dock is back");
}

/// A tap that arrives after the flow has gone is a stale tap, not a broken
/// invariant. It draws the app it is now in and nothing else happens.
#[test]
fn the_flow_ignores_events_once_it_is_over() {
    let mut state = at_the_bank_screen();

    screen(
        &mut state,
        &[
            ui(EventUi::BankConnectRequested),
            from(EventSource::BankLinked {
                institution: BANK.to_string(),
            }),
            from(EventSource::SyncFinished),
            ui(EventUi::OnboardingFinished),
        ],
    );

    eval!(&mut state,
        < ui(EventUi::BankConnectRequested),
        > Action::ToUi { action: ActionUi::Render { .. } },

        < ui(EventUi::OnboardingFinished),
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}
