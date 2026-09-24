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
use client_state::*;

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
/// The empty snapshot is what puts it there: an address with no banks on it
/// is the one that walks the road.
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

/// Its screens are not tabs, so there is no dock under them — a dock here would
/// offer three empty ones. That is the `dock: None` at the end of the render.
#[test]
fn a_device_the_app_has_never_run_on_opens_on_welcome() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::Started },
        > @r#"ToSource { action: LoadSession }"#,
        > @r#"ToUi { action: Render { screen: Onboarding { view: Welcome { title: "Every account,\none ledger.", body: [ViewRun { text: "Meridian reads your banks and puts what you spend, hold and owe on one page.", strong: false }], action: "Continue with email", fault: None, legal: "By continuing you agree to our Terms and Privacy Policy." } }, dock: None, who: None } }"#,
    );
}

/// No session means the machine stays where it already is. It is the *only*
/// answer that changes nothing, which is why it is worth pinning.
#[test]
fn no_session_leaves_the_flow_where_it_is() {
    let mut state = first_run();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SessionLoaded { signed_in: false } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Welcome { title: "Every account,\none ledger.", body: [ViewRun { text: "Meridian reads your banks and puts what you spend, hold and owe on one page.", strong: false }], action: "Continue with email", fault: None, legal: "By continuing you agree to our Terms and Privacy Policy." } }, dock: None, who: None } }"#,

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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 4 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example", placeholder: "you@example.com", fault: Some("That doesn't look like an email address.") }, action: "Continue" } }, dock: None, who: None } }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 4 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example.c", placeholder: "you@example.com", fault: None }, action: "Continue" } }, dock: None, who: None } }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None, who: None } }"#,
    );
}

/// Typing never submits, however many digits land. The sixth digit only
/// brings the button, and the tap is what spends the guess.
///
/// It used to submit on the sixth. The typo is what that missed: a wrong
/// digit was gone before the user had seen what they had typed, and the six
/// on screen could then only be edited backwards.
#[test]
fn typing_the_sixth_digit_offers_a_button_rather_than_submitting() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "40719".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Code { action: None, .. } }, .. } },

        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Code { action: Some(_), .. } }, .. } },

        < Event::FromUi { event: EventUi::CodeSubmitted },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// A rejected code can be corrected in place. Every box is editable, the red
/// clears on the first change, and the button is back the moment six digits
/// are in again — so a typo costs one tap, not a new code.
#[test]
fn a_rejected_code_can_be_fixed_and_sent_again() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeRejected },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::VerifyCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Code { fault: Some(_), .. } }, .. } },

        // Taking the last digit back clears the red and takes the button away.
        < Event::FromUi { event: EventUi::CodeTyped { digits: "40719".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Code { fault: None, action: None, .. } }, .. } },

        // A different sixth digit, and it can go again.
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407194".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Code { fault: None, action: Some(_), .. } }, .. } },

        < Event::FromUi { event: EventUi::CodeSubmitted },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407194" } }"#,
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
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeRejected },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code is not right."), action: Some("Continue"), resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None, who: None } }"#,
    );
}

/// A code that has certainly expired says exactly what a mistyped one says.
///
/// Deliberate, and worth a test of its own because the obvious "improvement"
/// is to tell the user which it was. The server answers alike for wrong,
/// expired and too many guesses so that nobody can ask it whether an address
/// has a code standing against it, and a screen saying "that one expired"
/// would hand back what answering alike was protecting.
///
/// Eleven minutes have passed here, so the resend is a button rather than a
/// countdown — which is the real way out, and the same one either way.
#[test]
fn an_expired_code_reads_as_a_wrong_one() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 11 * 60 } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeRejected },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code is not right."), action: Some("Continue"), resend: Ready { label: "Send a new code" } } }, dock: None, who: None } }"#,
    );
}

#[test]
fn the_resend_counts_down_before_it_offers() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None, who: None } }"#,
    );

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 42 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 0:18", wrong_address: "Wrong address?" } } }, dock: None, who: None } }"#,
    );

    // Once there is nothing left to wait for it is a button.
    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 60 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 4 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Ready { label: "Send a new code" } } }, dock: None, who: None } }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Email { step: "Step 1 of 4 · Account", title: "What's your email?", body: [ViewRun { text: "We'll send a six-digit code. Nothing to make up and nothing to forget.", strong: false }], field: ViewField { icon: "mail", value: "al@example.com", placeholder: "you@example.com", fault: None }, action: "Continue" } }, dock: None, who: None } }"#,
    );
}

/// A verified code asks the server for the snapshot and waits on it, because
/// that snapshot is what says which of two screens comes next: an address
/// with banks on it has been here before. Neither is drawn until it lands —
/// a screen swapped under the user a moment after it appears is the thing
/// this wait exists to stop.
///
/// Whether payment is settled is the same ask's to answer: refused unpaid,
/// the paywall covers the flow here — between the account and the bank — and
/// somebody who has paid walks straight on.
#[test]
fn a_verified_code_waits_for_the_snapshot_before_drawing_either_screen() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: LoadSnapshot }"#,
        > @r#"ToUi { action: Render { screen: Onboarding { view: Waiting { step: "Step 1 of 4 · Account", title: "Signing you in", body: [ViewRun { text: "One moment while we look up your account.", strong: false }], action: "Signing in…" } }, dock: None, who: None } }"#,

        // No banks on it, so the road is the one that gets walked.
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: "{}".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

/// A snapshot that never comes cannot decide anything, and a loader nothing
/// ends is worse than the wrong guess: the flow takes the bank step, which
/// is the one with a button on it.
#[test]
fn a_snapshot_that_fails_still_lands_the_flow_somewhere() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::VerifyCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::LoadFailed { failure: LoadFailure::Offline } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
        > Action::ToUi { action: ActionUi::ShowError { .. } },
    );
}

/// The unpaid refusal that ask comes back with is the paywall, drawn over
/// first run — the board's road runs account, payment, bank.
#[test]
fn an_unpaid_account_meets_the_paywall_before_the_bank() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: VerifyCode { email: "al@example.com", code: "407193" } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: LoadSnapshot }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource {
            event: EventSource::LoadFailed {
                failure: LoadFailure::Unpaid { reason: Unpaid::Never },
            },
        },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { view }, .. } } => {
            assert_eq!(view.title, "Meridian is a subscription");
        },

        // Paid for — the sheet took the money, the webhook said active, and
        // the snapshot is served — the paywall comes down onto the bank step.
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: "{}".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

/// The code screen, then a code accepted for an address whose account
/// already has banks attached: the snapshot the acceptance asks for carries
/// them, and the flow stands aside on Welcome back.
fn at_welcome_back() -> StateApp {
    let mut state = at_the_code_screen();

    state.handle_events(
        &[
            Event::FromUi {
                event: EventUi::CodeTyped {
                    digits: "407193".to_string(),
                },
            },
            Event::FromUi {
                event: EventUi::CodeSubmitted,
            },
            Event::FromSource {
                event: EventSource::CodeAccepted,
            },
            Event::FromSource {
                event: EventSource::SnapshotLoaded {
                    json: r#"{"connections":[
                        {"id":"c1","name":"First Platypus Bank","initials":"FP",
                         "status":"ok","accounts":[{"id":"a1","name":"Checking",
                         "balance":100.0,"group":"depository"}]}]}"#
                        .to_string(),
                },
            },
        ],
        &mut Vec::new(),
    );

    state
}

/// An address that already has an account with banks on it does not walk the
/// road again: the snapshot answering the code carries them, and the flow
/// lands on Welcome back — the banks as proof, worded like the Account tab's
/// list, with the app one tap away.
#[test]
fn an_account_with_banks_lands_on_welcome_back() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::CodeTyped { digits: "407193".to_string() } },
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::VerifyCode { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource {
            event: EventSource::SnapshotLoaded {
                json: r#"{"connections":[
                    {"id":"c1","name":"First Platypus Bank","initials":"FP",
                     "status":"ok","accounts":[{"id":"a1","name":"Checking",
                     "balance":100.0,"group":"depository"}]},
                    {"id":"c2","name":"Tattersall Federal","initials":"TF",
                     "status":"ok","accounts":[{"id":"a2","name":"Checking",
                     "balance":50.0,"group":"depository"},{"id":"a3",
                     "name":"Savings","balance":50.0,"group":"depository"}]}]}"#
                    .to_string(),
            },
        },
        > @r#"ToUi { action: Render { screen: Onboarding { view: WelcomeBack { title: "You already have an account", body: [ViewRun { text: "al@example.com", strong: true }, ViewRun { text: " is already set up. Your banks are still connected.", strong: false }], banks: [ViewAccountBank { id: "c1", name: "First Platypus Bank", initials: "FP", detail: "1 account", needs_reconnect: false }, ViewAccountBank { id: "c2", name: "Tattersall Federal", initials: "TF", detail: "2 accounts", needs_reconnect: false }], another: "Add another bank", open: "Open Meridian" } }, dock: None, who: None } }"#,
    );
}

/// Add another bank, from Welcome back, is the ordinary road: Plaid opens,
/// and the bank screen waits behind it with its button busy.
#[test]
fn welcome_back_goes_through_plaid_for_one_more_bank() {
    let mut state = at_welcome_back();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > Action::ToSource { action: ActionSource::StartBankLink },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Bank { opening: Some(_), .. } }, .. } },
    );
}

/// A sign-in abandoned from Welcome back is not a fresh install's failure:
/// the banks this account already has are untouched, so the screen says
/// nothing was added and keeps the way through to the app.
#[test]
fn a_sign_in_abandoned_from_welcome_back_keeps_the_way_out() {
    let mut state = at_welcome_back();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankAbandoned },
        > Action::ToSource { action: ActionSource::StartBankLink },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Failed { step: "Step 3 of 4 · Bank", title: "That didn't finish", body: [ViewRun { text: "The bank sign-in closed before it was done, so nothing was added.", strong: false }], rows: [ViewIconRow { icon: "tick-circle", tone: Plain, title: "Your bank is still connected", sub: Some("Nothing about it changed. This attempt is the only thing that did not finish."), value: None }], action: "Try again", opening: None, open: Some("Open Meridian") } }, dock: None, who: None } }"#,
    );
}

/// Open Meridian, from Welcome back, is the same door as Ready's: the flow
/// is over, the session is kept, and the app opens on the ledger the account
/// already has.
#[test]
fn welcome_back_opens_meridian() {
    let mut state = at_welcome_back();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::OnboardingFinished },
        > Action::ToSource { action: ActionSource::SaveSession },
        > Action::ToSource { action: ActionSource::LoadSnapshot },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. },
            dock: Some(_), .. } },
    );
}

/// Banks landing while a sign-in is under way move nothing: the screen under
/// the user is theirs, and the window about to open would be orphaned by a
/// flow that walked off.
#[test]
fn banks_landing_mid_sign_in_do_not_move_the_screen() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > Action::ToSource { action: ActionSource::StartBankLink },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource {
            event: EventSource::SnapshotLoaded {
                json: r#"{"connections":[
                    {"id":"c1","name":"First Platypus Bank","initials":"FP",
                     "status":"ok","accounts":[{"id":"a1","name":"Checking",
                     "balance":100.0,"group":"depository"}]}]}"#
                    .to_string(),
            },
        },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Bank { opening: Some(_), .. } }, .. } },
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

/// Plaid's page coming up is a screen of its own behind the window: nothing
/// to press, and the window going away is what moves the flow on — here by
/// closing early, which is the failed screen.
#[test]
fn plaids_window_up_is_a_screen_of_its_own() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::BankLinkOpened },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Waiting { step: "Step 3 of 4 · Bank", title: "Finish in your bank's window", body: [ViewRun { text: "Plaid opened your bank's sign-in in a window of its own. This screen moves on by itself when that window closes.", strong: false }], action: "Waiting on Plaid…" } }, dock: None, who: None } }"#,

        < Event::FromSource { event: EventSource::BankAbandoned },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Failed { .. } }, .. } },
    );
}

/// Connecting a bank is what starts the history arriving, so the stream
/// follows the news rather than the runtime deciding a sync must be next.
#[test]
fn a_linked_bank_opens_a_stream_for_its_history() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::BankLinked },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Failed { step: "Step 3 of 4 · Bank", title: "That didn't finish", body: [ViewRun { text: "The bank sign-in closed before it was done, so no bank was connected.", strong: false }], rows: [ViewIconRow { icon: "tick-circle", tone: Plain, title: "Your account is made", sub: Some("Close the app if you need to. Signing in again comes back to here."), value: None }], action: "Try again", opening: None, open: None } }, dock: None, who: None } }"#,

        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Bank { .. } }, .. } },
    );
}

/// A browser shows Plaid's page in a window of its own, and it can refuse to
/// open one. Nothing was tried, so the screen does not say a sign-in closed: it
/// says what stopped it, and the same button tries again — which, once it has
/// tried, is an ordinary attempt that says what an ordinary one says.
#[test]
fn a_blocked_window_says_the_browser_stopped_it() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankBlocked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Failed { step: "Step 3 of 4 · Bank", title: "That didn't finish", body: [ViewRun { text: "Your browser blocked the window the bank sign-in opens in, so no bank was connected. Allow pop-ups for Meridian and try again.", strong: false }], rows: [ViewIconRow { icon: "tick-circle", tone: Plain, title: "Your account is made", sub: Some("Close the app if you need to. Signing in again comes back to here."), value: None }], action: "Try again", opening: None, open: None } }, dock: None, who: None } }"#,

        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankAbandoned },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Failed { body, .. } }, .. } } => {
            assert_eq!(body[0].text, "The bank sign-in closed before it was done, so no bank was connected.");
        },
    );
}

/// A session nobody has paid for is refused a bank sign-in before one opens,
/// and the paywall takes the screen. Once it is paid for, first run is back on
/// the bank step with Connect a bank live — nobody opened a sign-in, so none
/// closed, and a window cannot be opened without the next tap.
#[test]
fn paying_in_the_middle_of_first_run_comes_back_to_the_bank() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        // What a runtime reports when the call that opens a sign-in is
        // refused: the refusal, then that no bank came of it.
        < Event::FromSource { event: EventSource::LoadFailed { failure: LoadFailure::Unpaid { reason: Unpaid::Never } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { .. }, dock: None, .. } },

        < Event::FromSource { event: EventSource::BankAbandoned },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Paywall { .. }, dock: None, .. } },

        < Event::FromSource { event: EventSource::SnapshotLoaded { json: "{}".to_string() } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Bank { step: "Step 3 of 4 · Bank", title: "Just so you know", body: [ViewRun { text: "Meridian uses ", strong: false }, ViewRun { text: "Plaid", strong: true }, ViewRun { text: " to connect. You sign in on your bank's own page.", strong: false }], rows: [ViewIconRow { icon: "lock", tone: Plain, title: "Your bank password goes to Plaid", sub: Some("Never to Meridian. We are handed a key to read with, not your sign-in."), value: None }, ViewIconRow { icon: "tick-circle", tone: Plain, title: "Read-only", sub: Some("Balances, transactions and recurring bills. Nothing can move money."), value: None }, ViewIconRow { icon: "ledger", tone: Plain, title: "Meridian keeps your transactions", sub: Some("They are stored on our server. That is what the ledger is made of."), value: None }, ViewIconRow { icon: "trash", tone: Plain, title: "Removing a bank deletes them", sub: Some("One tap in Account, and its history goes with it."), value: None }], action: "Connect a bank", opening: None, legal: "Plaid handles the connection under its end-user privacy policy." } }, dock: None, who: None } }"#,
    );
}

/// The same screen, reached from Add another bank, is a different situation:
/// there is a bank connected, a ledger behind it, and nothing about either was
/// touched by the attempt that just closed. So it says so, and it carries a
/// second button — a screen whose only way on reopens the page somebody just
/// closed is a trap, and the flow has no back out of the bank step to fall
/// back on.
#[test]
fn a_second_sign_in_that_did_not_finish_leaves_a_way_out() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Ready { .. } }, .. } },

        // Add another bank, and then a change of mind on Plaid's page.
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankAbandoned },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Failed { step: "Step 3 of 4 · Bank", title: "That didn't finish", body: [ViewRun { text: "The bank sign-in closed before it was done, so nothing was added.", strong: false }], rows: [ViewIconRow { icon: "tick-circle", tone: Plain, title: "First Platypus Bank is still connected", sub: Some("Nothing about it changed. This attempt is the only thing that did not finish."), value: None }], action: "Try again", opening: None, open: Some("Open Meridian") } }, dock: None, who: None } }"#,
    );
}

/// The server waited and Plaid was still late, so nobody knows yet. That is
/// not the same sentence as "nothing was connected", and it does not get the
/// screen that says so — it gets the one that keeps asking.
#[test]
fn a_pending_sign_in_watches_for_the_bank_rather_than_failing() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::BankPending },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Sync { .. } }, .. } },
        > @r#"ToSource { action: WatchSync }"#,
    );
}

/// Nothing is running, and after a pending answer that is the gap rather than
/// the end: the sync this screen is watching may not have started. Reading it
/// as "finished" is how somebody who is about to get a bank sees `You're set.`
/// over nought accounts.
#[test]
fn a_status_that_says_nothing_is_not_the_end_of_a_pending_sign_in() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankPending },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"accounts","institution":null,"accounts":0,"transactions":0,"streams":0,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Sync { .. } }, .. } },
        > @r#"ToSource { action: WatchSync }"#,
    );
}

/// The bank was late and then it landed, which is the whole reason the pending
/// answer exists rather than a guess between the other two.
#[test]
fn a_bank_that_lands_after_a_pending_answer_carries_the_flow_on() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankPending },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"accounts","institution":null,"accounts":0,"transactions":0,"streams":0,"finished":true}"#.to_string() } },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding { view: ViewOnboarding::Ready { .. } }, .. } },
    );
}

/// Waiting is bounded. The server had already waited before it answered
/// pending, so past this the bank is not coming and saying so beats a spinner
/// that never stops — and it says the true thing, that no bank was connected.
#[test]
fn a_pending_sign_in_that_never_lands_says_nothing_was_connected() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankPending },
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + AWAIT_GOOD_FOR_SECS } },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"accounts","institution":null,"accounts":0,"transactions":0,"streams":0,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Failed { step: "Step 3 of 4 · Bank", title: "That didn't finish", body: [ViewRun { text: "The bank sign-in closed before it was done, so no bank was connected.", strong: false }], rows: [ViewIconRow { icon: "tick-circle", tone: Plain, title: "Your account is made", sub: Some("Close the app if you need to. Signing in again comes back to here."), value: None }], action: "Try again", opening: None, open: None } }, dock: None, who: None } }"#,
    );
}

/// The sync screen names the bank it is reading and counts what has landed so
/// far, which is the only thing on it that moves.
#[test]
fn the_sync_says_what_has_landed_so_far() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"transactions","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":0,"finished":false}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 4 of 4 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.625, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "First Platypus Bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "tick-circle", tone: Done, title: "Accounts", sub: None, value: Some("8") }, ViewIconRow { icon: "spin", tone: Running, title: "Transactions", sub: None, value: Some("48 so far") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None, who: None } }"#,
        > @r#"ToSource { action: WatchSync }"#,
    );
}

/// The button says what it is doing while Plaid's page is being opened.
///
/// Around a second passes between the tap and the sheet — the server creates a
/// Link session, then iOS raises a web view over the app — and nothing moved
/// on screen during it, which reads as a button that missed the tap. It is the
/// flow's state and not the view's, because what a screen says is a decision.
///
/// It clears on either ending, so Try again is a live button rather than one
/// still claiming to be opening something that closed.
#[test]
fn the_button_says_it_is_opening_plaid() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Bank { opening: Some(opening), .. } }, .. } } => {
            assert_eq!(opening, "Opening Plaid…");
        },

        < Event::FromSource { event: EventSource::BankAbandoned },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Failed { opening: None, .. } }, .. } },

        // And again from the failed screen, which opens the very same page.
        < Event::FromUi { event: EventUi::BankConnectRequested },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Onboarding {
            view: ViewOnboarding::Bank { opening: Some(_), .. } }, .. } },
    );
}

/// The sync screen opens before anyone knows what the bank is called.
///
/// Nothing has the name at the moment a bank is attached — it is in neither
/// the Link session nor the token exchange, and the server first sees it in
/// the balances call inside the sync that follows. So the first row reads
/// "Your bank" and the first status that carries a name corrects it. Better
/// than holding the bank sign-in sheet open for a string.
#[test]
fn the_sync_screen_names_the_bank_once_the_sync_says() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 4 of 4 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.375, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "Your bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "spin", tone: Running, title: "Accounts", sub: None, value: Some("reading") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Transactions", sub: None, value: Some("waiting") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None, who: None } }"#,
        > @r#"ToSource { action: WatchSync }"#,

        // The first message back from a sync that has got past balances.
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"transactions","institution":"First Platypus Bank","accounts":14,"transactions":0,"streams":0,"finished":false}"#.to_string() } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 4 of 4 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.625, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "First Platypus Bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "tick-circle", tone: Done, title: "Accounts", sub: None, value: Some("14") }, ViewIconRow { icon: "spin", tone: Running, title: "Transactions", sub: None, value: Some("reading") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None, who: None } }"#,
    );
}

/// One stream, asked for once. The server writes to it whenever the sync
/// moves, so the clock has nothing to go and look for on this screen.
#[test]
fn the_stream_is_opened_once_and_not_again_on_every_tick() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 1 } },

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 2 } },
    );
}

/// A connection that goes is opened again, because the screen it belongs to is
/// still up — iOS drops it every time the app is backgrounded, and coming back
/// to a sync that has stopped moving is the failure this prevents.
///
/// Backed off rather than reopened on the instant, or a server refusing them
/// is asked once a tick, which is a poll wearing a different name.
#[test]
fn a_stream_that_goes_is_opened_again_once_the_backoff_is_up() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,

        // Gone, and nothing drawn about it: one dropped connection is not a
        // banner over somebody's first run.
        < Event::FromSource { event: EventSource::SyncWatchEnded },

        // Too soon.
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 1 } },

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 3 } },
        > @r#"ToSource { action: WatchSync }"#,
    );
}

/// Leaving the screen lets the connection go. Its own action, because a stream
/// nobody closes is a slot the server holds for a screen that is not there.
#[test]
fn the_stream_is_let_go_when_the_sync_screen_is() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: WatchSync }"#,

        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: StopWatchingSync }"#,

        // On Ready now, so the clock has nothing to open.
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 1 } },
    );
}

/// A finished sync counts what arrived and names the bank it came from.
#[test]
fn a_finished_sync_counts_what_arrived() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":false}"#.to_string() } },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Ready { title: "You're set.", body: [ViewRun { text: "First Platypus Bank", strong: true }, ViewRun { text: " is connected and syncing.", strong: false }], tally: [ViewTally { value: "8", label: "accounts" }, ViewTally { value: "48", label: "transactions" }, ViewTally { value: "8", label: "recurring bills found" }], another: "Add another bank", open: "Open Meridian" } }, dock: None, who: None } }"#,
    );
}

/// The counts are read off the reply that says the sync is over, and not only
/// off the ones before it.
///
/// A sync can finish between two ticks — a Sandbox one nearly always does — so
/// the first status the phone reads is already the last. Taking the totals
/// only from a reply that said `finished: false` is how a real link of
/// fourteen accounts and forty-eight transactions landed on a "You're set."
/// that counted nought of each.
#[test]
fn a_sync_already_over_on_its_first_reply_still_counts() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"Citibank Online","accounts":14,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Ready { title: "You're set.", body: [ViewRun { text: "Citibank Online", strong: true }, ViewRun { text: " is connected and syncing.", strong: false }], tally: [ViewTally { value: "14", label: "accounts" }, ViewTally { value: "48", label: "transactions" }, ViewTally { value: "8", label: "recurring bills found" }], another: "Add another bank", open: "Open Meridian" } }, dock: None, who: None } }"#,
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
        > @r#"ToSource { action: LoadSnapshot }"#,
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, dock: Some(_), .. } },
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
