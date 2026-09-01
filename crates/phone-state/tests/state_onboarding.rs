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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code is not right."), action: Some("Continue"), resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
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
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "4", live: false, bad: true }, ViewCodeCell { digit: "0", live: false, bad: true }, ViewCodeCell { digit: "7", live: false, bad: true }, ViewCodeCell { digit: "1", live: false, bad: true }, ViewCodeCell { digit: "9", live: false, bad: true }, ViewCodeCell { digit: "3", live: false, bad: true }], fault: Some("That code is not right."), action: Some("Continue"), resend: Ready { label: "Send a new code" } } }, dock: None } }"#,
    );
}

#[test]
fn the_resend_counts_down_before_it_offers() {
    let mut state = at_the_code_screen();

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 1:00", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );

    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 42 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Waiting { text: "Resend in 0:18", wrong_address: "Wrong address?" } } }, dock: None } }"#,
    );

    // Once there is nothing left to wait for it is a button.
    eval!(&mut state,
        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 60 } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Code { step: "Step 1 of 3 · Account", title: "Check your email", body: [ViewRun { text: "We sent a code to ", strong: false }, ViewRun { text: "al@example.com", strong: true }, ViewRun { text: ".", strong: false }], cells: [ViewCodeCell { digit: "", live: true, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }, ViewCodeCell { digit: "", live: false, bad: false }], fault: None, action: None, resend: Ready { label: "Send a new code" } } }, dock: None } }"#,
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
        < Event::FromUi { event: EventUi::CodeSubmitted },
        < Event::FromSource { event: EventSource::CodeAccepted },
        > Action::ToUi { action: ActionUi::Render { .. } },
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

        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: PollSync }"#,
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
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"transactions","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":0,"finished":false}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: PollSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 3 of 3 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.625, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "First Platypus Bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "tick-circle", tone: Done, title: "Accounts", sub: None, value: Some("8") }, ViewIconRow { icon: "spin", tone: Running, title: "Transactions", sub: None, value: Some("48 so far") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None } }"#,
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
        > @r#"ToSource { action: PollSync }"#,
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 3 of 3 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.375, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "Your bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "spin", tone: Running, title: "Accounts", sub: None, value: Some("reading") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Transactions", sub: None, value: Some("waiting") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None } }"#,

        // The first poll back from a sync that has got past balances.
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"transactions","institution":"First Platypus Bank","accounts":14,"transactions":0,"streams":0,"finished":false}"#.to_string() } },
        > @r#"ToUi { action: Render { screen: Onboarding { view: Sync { step: "Step 3 of 3 · Sync", title: "Reading your history", body: [ViewRun { text: "First time only. Every refresh after this one takes a moment.", strong: false }], progress: 0.625, rows: [ViewIconRow { icon: "tick-circle", tone: Done, title: "First Platypus Bank", sub: None, value: Some("connected") }, ViewIconRow { icon: "tick-circle", tone: Done, title: "Accounts", sub: None, value: Some("14") }, ViewIconRow { icon: "spin", tone: Running, title: "Transactions", sub: None, value: Some("reading") }, ViewIconRow { icon: "dot", tone: Waiting, title: "Recurring bills", sub: None, value: Some("waiting") }] } }, dock: None } }"#,
    );
}

/// The clock is what goes and looks. A sync runs on the server and finishes
/// whether or not anybody asks, so while that screen is up every tick asks
/// again — and because it is the machine deciding, every ask is an action in
/// the log and a replay makes the same ones.
///
/// It is a poll because the server can only answer questions it is asked;
/// #30 is the push channel that replaces it, and this rule is the whole of
/// what goes away.
#[test]
fn every_tick_asks_how_the_sync_is_going() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: PollSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 1 } },
        > @r#"ToSource { action: PollSync }"#,

        < Event::FromRuntime { event: EventRuntime::Tick { now: NOW + 2 } },
        > @r#"ToSource { action: PollSync }"#,
    );
}

/// Leaving the screen ends the polling, and nothing is cancelled to do it —
/// the machine simply stops asking. That is the whole of stopping a poll that
/// was never a subscription.
#[test]
fn the_asking_stops_when_the_sync_screen_does() {
    let mut state = at_the_bank_screen();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::BankConnectRequested },
        < Event::FromSource { event: EventSource::BankLinked },
        < Event::FromSource { event: EventSource::SyncStatusLoaded { json: r#"{"stage":"streams","institution":"First Platypus Bank","accounts":8,"transactions":48,"streams":8,"finished":true}"#.to_string() } },
        > @r#"ToSource { action: StartBankLink }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > @r#"ToSource { action: PollSync }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        // On Ready now, so the clock has nothing to ask about.
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
        > @r#"ToSource { action: PollSync }"#,
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
