//! First run's own state.
//!
//! A sub-machine rather than fields on [`StateApp`](crate::StateApp): the
//! address being typed, the digits so far, which code fault, how much of the
//! first sync has landed — nothing outside the flow reads any of it, and once
//! the flow is through there is nothing left to read. That is the split
//! `docs/STYLE.md` asks for, and the same one [`StateLedger`](crate::StateLedger)
//! is.
//!
//! It handles no events of its own. [`StateApp`](crate::StateApp) routes the
//! ones that are the flow's business here, which keeps a single
//! `handle_events` as the only way anything changes.
//!
//! The order of the steps is forced rather than chosen. Plaid's `access_token`
//! is permanent and lives on the server (`docs/PLAID.md`), so it has to belong
//! to somebody before it can be stored: the account exists before the bank
//! does.

use std::time::Duration;

use api_common::Timestamp;

use crate::SyncStage;

/// How long before another code can be asked for.
pub(crate) const RESEND_AFTER: Duration = Duration::from_secs(60);

/// How many digits a code has.
pub(crate) const CODE_LENGTH: usize = 6;

/// Which screen of the flow, and it is only ever one.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    #[default]
    Welcome,
    Email,
    Code,
    Bank,
    Sync,
    Ready,
    /// The bank sign-in ended with nothing — dismissed, failed, timed out.
    Failed,
}

/// What the last thing the user did was wrong about.
///
/// One at a time: a fault belongs to the screen it happened on, and moving off
/// that screen clears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Not an address.
    Address,

    /// The code did not work.
    ///
    /// One fault for every way of failing, because the server answers with
    /// one — see [`EventSource::CodeRejected`](crate::EventSource). A code
    /// that has expired reads as wrong and the resend is the way out of both,
    /// which is the same way out the user would have wanted anyway.
    CodeWrong,
}

/// How much of the first sync has arrived.
///
/// The counts and the stage are separate because they are separate facts: the
/// transactions row shows `48 so far` while it is still running, so a count is
/// not a claim that the stage is finished.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Progress {
    pub(crate) stage: SyncStage,
    pub(crate) accounts: u32,
    pub(crate) transactions: u32,
    pub(crate) streams: u32,
}

/// The flow, from the first screen to the bank being connected.
#[derive(Debug, Default)]
pub struct StateOnboarding {
    step: Step,

    /// What is in the address field. Kept across a step back, so returning to
    /// fix a typo does not mean typing it again.
    email: String,

    /// The digits entered so far, at most [`CODE_LENGTH`] of them.
    code: String,

    fault: Option<Fault>,

    /// When the last code went out, for the countdown. A timestamp rather than
    /// a remaining count, because the machine reads no clock — the difference
    /// is taken on the way out, against the `now` the runtime handed in.
    sent_at: Option<Timestamp>,

    /// The bank most recently connected, which is the one the sync is for.
    bank: Option<String>,

    sync: Progress,
}

impl StateOnboarding {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn step(&self) -> Step {
        self.step
    }

    pub(crate) fn email(&self) -> &str {
        &self.email
    }

    pub(crate) fn code(&self) -> &str {
        &self.code
    }

    pub(crate) fn fault(&self) -> Option<Fault> {
        self.fault
    }

    pub(crate) fn sent_at(&self) -> Option<Timestamp> {
        self.sent_at
    }

    pub(crate) fn bank(&self) -> Option<&str> {
        self.bank.as_deref()
    }

    pub(crate) fn sync(&self) -> Progress {
        self.sync
    }

    /// The Welcome button: there is one door, and this is it.
    pub(crate) fn choose_email(&mut self) {
        self.step = Step::Email;
        self.fault = None;
    }

    pub(crate) fn type_email(&mut self, text: &str) {
        self.email = text.to_string();

        // Typing is how a rejected address stops being rejected. A field that
        // stayed red while it was being fixed would be arguing with the user.
        if self.fault == Some(Fault::Address) {
            self.fault = None;
        }
    }

    /// Continue, on the address screen. The address to send a code to, or
    /// `None` if there is nothing to send one to.
    pub(crate) fn submit_email(&mut self) -> Option<String> {
        if !is_address(&self.email) {
            self.fault = Some(Fault::Address);
            return None;
        }

        self.fault = None;
        self.code.clear();
        self.step = Step::Code;

        Some(self.email.clone())
    }

    /// A digit typed, or the whole field filled by iOS from the notification.
    ///
    /// Takes the value the field now holds rather than one keystroke, so a
    /// paste and a backspace are the same event as a tap.
    pub(crate) fn type_code(&mut self, digits: &str) {
        let code: String = digits
            .chars()
            .filter(char::is_ascii_digit)
            .take(CODE_LENGTH)
            .collect();

        // Nothing changed, so nothing to clear. Worth checking because the
        // field sends its whole contents: a seventh digit typed into six full
        // boxes arrives here as the same six, and that is not an edit.
        if code == self.code {
            return;
        }

        self.code = code;

        // Any change at all clears the rejection. A screen that stayed red
        // while it was being corrected would be arguing with the user, which
        // is the same reason [`Self::type_email`] does this.
        self.fault = None;
    }

    /// Continue, under the boxes.
    ///
    /// `None` on a code that is not six digits long, which the button being
    /// absent should already have prevented — a stale tap, not an invariant
    /// worth asserting.
    pub(crate) fn submit_code(&mut self) -> Option<(String, String)> {
        if self.code.len() < CODE_LENGTH {
            return None;
        }

        Some((self.email.clone(), self.code.clone()))
    }

    /// Ask for another code. `None` while the last one is still young.
    pub(crate) fn resend(&mut self, now: Timestamp) -> Option<String> {
        if self.waiting_to_resend(now) {
            return None;
        }

        self.code.clear();
        self.fault = None;

        Some(self.email.clone())
    }

    /// Whether the countdown is still running.
    ///
    /// A code that was never sent can be sent, which is what makes the button
    /// live on a screen reached by replay rather than by tapping Continue.
    pub(crate) fn waiting_to_resend(&self, now: Timestamp) -> bool {
        match self.sent_at {
            Some(sent) => now.since(sent) < RESEND_AFTER,
            None => false,
        }
    }

    /// The chevron, and the code screen's way back to a mistyped address.
    pub(crate) fn back(&mut self) {
        self.fault = None;

        self.step = match self.step {
            Step::Email => Step::Welcome,
            Step::Code => {
                self.code.clear();
                Step::Email
            }

            // Everything from the bank screen on has a connection behind it or
            // an account made, and neither unwinds.
            step => step,
        };
    }

    /// Connect a bank — the button on the bank screen, Try again on the
    /// failure, and Add another bank on the last screen. One event, because it
    /// is one thing: the next tap opens Plaid.
    pub(crate) fn connect_bank(&mut self) {
        self.step = Step::Bank;
        self.sync = Progress::default();
    }

    pub(crate) fn code_sent(&mut self, now: Timestamp) {
        self.sent_at = Some(now);
        self.step = Step::Code;
    }

    pub(crate) fn code_accepted(&mut self) {
        self.fault = None;
        self.step = Step::Bank;
    }

    pub(crate) fn code_rejected(&mut self) {
        self.fault = Some(Fault::CodeWrong);
    }

    pub(crate) fn bank_linked(&mut self, institution: &str) {
        self.bank = Some(institution.to_string());
        self.sync = Progress::default();
        self.step = Step::Sync;
    }

    pub(crate) fn bank_abandoned(&mut self) {
        self.step = Step::Failed;
    }

    pub(crate) fn sync_progressed(&mut self, sync: Progress) {
        debug_assert_eq!(
            self.step,
            Step::Sync,
            "a sync reports progress only while one is running",
        );

        self.sync = sync;
    }

    pub(crate) fn sync_finished(&mut self) {
        self.step = Step::Ready;
    }
}

/// Whether this could be an address.
///
/// Deliberately shallow: something, an `@`, and a domain with a dot in it. The
/// only address that matters is the one the code arrives at, and the code
/// arriving is what checks that — so anything stricter here would reject real
/// addresses to catch typos it cannot catch anyway. It does reject
/// `al@example`, which is the one the design sheet draws wrong.
fn is_address(text: &str) -> bool {
    let text = text.trim();

    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };

    let Some((name, tld)) = domain.rsplit_once('.') else {
        return false;
    };

    !local.is_empty()
        && !name.is_empty()
        && !tld.is_empty()
        && !text.contains(char::is_whitespace)
        && !domain.contains('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_needs_a_domain_with_a_dot_in_it() {
        assert!(is_address("al@example.com"));
        assert!(is_address("al+meridian@mail.example.co.uk"));

        assert!(!is_address("al@example"));
        assert!(!is_address("al"));
        assert!(!is_address("@example.com"));
        assert!(!is_address("al@.com"));
        assert!(!is_address("al@example."));
        assert!(!is_address("al @example.com"));
        assert!(!is_address(""));
    }

    #[test]
    fn a_code_takes_six_digits_and_ignores_the_rest() {
        let mut flow = StateOnboarding::new();
        flow.email = "al@example.com".to_string();

        flow.type_code("40");
        assert_eq!(flow.code(), "40");
        assert!(flow.submit_code().is_none(), "two digits is not a code");

        // Whatever the keyboard sends, only digits land, and only six.
        flow.type_code("4-0-7-1-9-3-5");
        assert_eq!(flow.code(), "407193");
        assert!(flow.submit_code().is_some());
    }

    #[test]
    fn editing_at_all_clears_the_rejection() {
        let mut flow = StateOnboarding::new();

        flow.type_code("407193");
        flow.code_rejected();
        assert_eq!(flow.fault(), Some(Fault::CodeWrong));

        flow.type_code("40719");
        assert_eq!(flow.fault(), None);
    }

    /// The field sends its whole contents, so a seventh digit typed into six
    /// full boxes arrives as the same six. Nothing changed, so the rejection
    /// stands — clearing it would wipe the message the user has not acted on.
    #[test]
    fn a_seventh_digit_is_not_an_edit() {
        let mut flow = StateOnboarding::new();

        flow.type_code("407193");
        flow.code_rejected();

        flow.type_code("4071935");
        assert_eq!(flow.code(), "407193");
        assert_eq!(flow.fault(), Some(Fault::CodeWrong));
    }
}
