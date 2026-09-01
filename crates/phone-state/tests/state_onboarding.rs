//! First run, tested the way `docs/STYLE.md` says: events in with `<`, actions
//! out with `>`.
//!
//! This is the one test file that does *not* say who is signed in. The app
//! assumes a device it has never run on, so a machine straight out of
//! `StateApp::new()` is already on the first screen of the flow — which is
//! what these drive.
//!
//! The screens these are about are small enough to write out, so a test that
//! is about what a screen says pins the whole render with `@` — the sentence,
//! the field, the six boxes and the countdown, exactly as the machine built
//! them. Nothing here reads a view back out of an action list.

use api_eval::eval;
use phone_state::*;

/// 2026-08-19 00:00 UTC, the same clock the rest of the tests are set to.
const NOW: u64 = 1_787_097_600;

/// A machine on a device the app has never run on, told what time it is.
///
/// A tick is how the clock arrives, here and everywhere else: the machine
/// reads none of its own. This one draws nothing — a tick only redraws while
/// the resend countdown is running.
fn first_run() -> StateApp {
    let mut state = StateApp::new();

    state.handle_events(
        &[Event::FromRuntime {
            event: EventRuntime::Tick { now: NOW },
        }],
        &mut Vec::new(),
    );

    state
}

/// Everything up to the code screen, with a code already sent. The actions are
/// dropped rather than checked: the tests that are about them send these same
/// events themselves.
fn at_the_code_screen() -> StateApp {
    let mut state = first_run();

    state.handle_events(
        &[
            Event::FromUi {
                event: EventUi::EmailChosen,
            },
            Event::FromUi {
                event: EventUi::EmailTyped {
                    text: "al@example.com".to_string(),
                },
            },
            Event::FromUi {
                event: EventUi::EmailSubmitted,
            },
            Event::FromSource {
                event: EventSource::CodeSent,
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// The same, up to the bank screen, which is the one the flow exists for.
fn at_the_bank_screen() -> StateApp {
    let mut state = at_the_code_screen();

    state.handle_events(
        &[
            Event::FromUi {
                event: EventUi::CodeTyped {
                    digits: "407193".to_string(),
                },
            },
            Event::FromSource {
                event: EventSource::CodeAccepted,
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// Its screens are not tabs, so there is no dock under them — a dock here would
/// offer three empty ones. That is the `dock: None` at the end of the render.
#[test]
fn a_device_the_app_has_never_run_on_opens_on_welcome() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::Started },
        > @r#"ToSource { action: LoadSession }"#,
        > @r#"ToUi { action: Render { screen: Onboarding { view: Welcome { title: "Every account,\none ledger.", body: [ViewRun { text: "Meridian reads your banks and puts what you spend, hold and owe on one page.", strong: false }], action: "Continue with email", legal: "By continuing you agree to our Terms and Privacy Policy." } }, dock: None } }"#,
    );
}

/// No session means the machine stays where it already is. It is the *only*
/// answer that changes nothing, which is why it is worth pinning.
#[test]
fn no_session_leaves_the_flow_where_it_is() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SessionLoaded { signed_in: false } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Welcome { title: "Every account,\none ledger.", body: [ViewRun { text: "Meridian reads your banks and puts what you spend, hold and owe on one page.", strong: false }], action: "Continue with email", legal: "By continuing you agree to our Terms and Privacy Policy." } }, dock: None } }"#,

        // And a second passing moves nothing either: Welcome says nothing
        // about the clock, so there is nothing to redraw.
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 1 } },
    );
}

#[test]
fn an_address_that_is_not_one_is_never_sent() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::EmailChosen },
        > Action::ToUi { action: ActionUi::Render { .. } },

        // Nothing goes out, and the only thing that happens is the field
        // saying so — with what was typed still in it.
        < Event::FromUi { event: EventUi::EmailTyped { text: "al@example".to_string() } },
        < Event::FromUi { event: EventUi::EmailSubmitted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 3 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example", placeholder: "you@example.com", fault: Some("That doesn't look like an email address.") }, action: "Continue" } }, dock: None } }"#,
    );
}

/// A field that stayed red while it was being fixed would be arguing with the
/// user.
#[test]
fn typing_clears_the_rejection() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::EmailChosen },
        < Event::FromUi { event: EventUi::EmailTyped { text: "al@example".to_string() } },
        < Event::FromUi { event: EventUi::EmailSubmitted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::EmailTyped { text: "al@example.c".to_string() } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 3 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example.c", placeholder: "you@example.com", fault: None }, action: "Continue" } }, dock: None } }"#,
    );
}

#[test]
fn a_good_address_asks_for_a_code() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::EmailChosen },
        > Action::ToUi { action: ActionUi::Render { .. } },

        // Typing draws, and only Continue sends.
        < Event::FromUi { event: EventUi::EmailTyped { text: "al@example.com".to_string() } },
        < Event::FromUi { event: EventUi::EmailSubmitted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: SendCode { email: "al@example.com" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The address is on screen and one tap from being fixed — a typo in the step
/// before is otherwise a dead end. It is set strong in the sentence, there is a
/// box per digit, and the first one takes the next digit.
#[test]
fn the_code_screen_says_where_the_code_went() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );
}

/// There is nothing to check before the sixth digit, so the sixth digit
/// submits and there is no Continue button on that screen.
#[test]
fn the_sixth_digit_submits() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "40719".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// A rejected code is one wrong value, not six wrong digits: every box is bad,
/// none is waiting, and the sentence says which of the two problems it is.
#[test]
fn a_wrong_code_reddens_every_box() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromSource { event: EventSource::CodeRejected },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code is not right."), resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );
}

/// Wrong and too late are different problems, one sentence apart, and the way
/// out of each is different.
///
/// The server says only that the code was refused, so what makes this the
/// expired screen rather than the wrong one is the clock: eleven minutes have
/// passed since the code went out. Which is also why the resend is a button
/// here and a countdown in the test above — by the time a code can expire, a
/// new one has been available for ten minutes.
#[test]
fn an_expired_code_says_so_and_offers_a_new_one() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 11 * 60 } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromSource { event: EventSource::CodeRejected },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code has expired. They last ten minutes."), resend: Ready { label: "Send a new code" } } }, dock: None } }"#,
    );
}

#[test]
fn the_resend_counts_down_before_it_offers() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 42 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, resend: Waiting { text: "Resend in 0:18", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );

    // Once there is nothing left to wait for it is a button.
    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 60 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, resend: Ready { label: "Send a new code" } } }, dock: None } }"#,
    );
}

/// The countdown is what stops a second code going out, so the event has to be
/// ignored rather than merely un-drawn.
#[test]
fn resending_early_sends_nothing() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::ResendRequested },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 60 } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ResendRequested },
        > @r#"ToSource { action: SendCode { email: "al@example.com" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

#[test]
fn wrong_address_goes_back_to_the_field_with_it_still_in() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407".to_string() } },
        < Event::FromUi { event: EventUi::BackPressed },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 3 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example.com", placeholder: "you@example.com", fault: None }, action: "Continue" } }, dock: None } }"#,
    );
}

#[test]
fn a_verified_code_asks_for_a_bank() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

/// There is no version of this app without a bank in it — every screen is a
/// view of transactions — so nothing here offers a way past this one, back
/// included.
#[test]
fn there_is_no_way_back_out_of_the_bank_screen() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BackPressed },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

#[test]
fn connecting_a_bank_opens_plaid() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// Connecting a bank is what starts the history arriving, so the ask follows
/// the news rather than the runtime deciding a sync must be next.
#[test]
fn a_linked_bank_asks_for_its_history() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::BankLinked { institution: "First Platypus Bank".to_string() } },
        > @r#"ToSource { action: WatchSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The common ending, not a rare one. Nothing was created, and the screen says
/// exactly that — and the way out of it is the same road, not a different one.
#[test]
fn an_abandoned_sign_in_says_nothing_was_connected() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankAbandoned },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Failed { .. } }, .. } },

        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

/// The sync screen names the bank it is reading and counts what has landed so
/// far, which is the only thing on it that moves.
#[test]
fn the_sync_says_what_has_landed_so_far() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked { institution: "First Platypus Bank".to_string() } },
        < Event::FromSource { event: EventSource::SyncProgressed { stage: SyncStage::Transactions, accounts: 8, transactions: 48, streams: 0 } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 3 of 3 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.625, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "First Platypus Bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "tick-circle", tone: Done, title: "Accounts", sub: None, value: Some("8") }, ViewIconRow { icon: "spin", tone: Running, title: "Transactions", sub: None, value: Some("48 so far") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None } }"#,
    );
}

/// A finished sync counts what arrived and names the bank it came from.
#[test]
fn a_finished_sync_counts_what_arrived() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked { institution: "First Platypus Bank".to_string() } },
        < Event::FromSource { event: EventSource::SyncProgressed { stage: SyncStage::Streams, accounts: 8, transactions: 48, streams: 8 } },
        < Event::FromSource { event: EventSource::SyncFinished },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Ready { title: "You're set.", body: [ViewRun { text: "First Platypus Bank", strong: true }, ViewRun { text: " is connected and syncing.", strong: false }], tally: [ViewTally { value: "8", label: "accounts" }, ViewTally { value: "48", label: "transactions" }, ViewTally { value: "8", label: "recurring bills found" }], another: "Add another bank", open: "Open Meridian" } }, dock: None } }"#,
    );
}

/// The end of the road: the flow is dropped, the session the runtime is
/// holding is committed so the next launch skips first run, and the app goes
/// and gets its data. The Ledger it lands on is far too large to write out, so
/// the render says only that it is the Ledger and that the dock is back.
#[test]
fn opening_meridian_ends_the_flow() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::OnboardingFinished },
        > Action::ToSource { action: ActionSource::SaveSession },
        > @r#"ToSource { action: LoadSnapshot { force: false } }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, dock: Some(_) } },
    );
}

/// A tap that arrives after the flow has gone is a stale tap, not a broken
/// invariant. It draws the app it is now in and nothing else happens.
#[test]
fn the_flow_ignores_events_once_it_is_over() {
    let mut state = at_the_code_screen();

    state.handle_events(
        &[Event::FromUi {
            event: EventUi::OnboardingFinished,
        }],
        &mut Vec::new(),
    );

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::OnboardingFinished },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}
