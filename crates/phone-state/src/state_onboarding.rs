//! First run's own state.
//!
//! A sub-machine rather than fields on [`StateApp`](crate::StateApp): nothing
//! outside the flow reads any of it, and once the flow is through there is
//! nothing left to read.
//!
//! The order of the steps is forced: Plaid's `access_token` lives on the
//! server (`docs/PLAID.md`), so the account exists before the bank does.

use std::time::Duration;

use api_common::Timestamp;

use server_wire::SyncStage;

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

/// What the last thing the user did was wrong about. One at a time: a fault
/// belongs to the screen it happened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// Not an address.
    Address,

    /// The code did not work. One fault for every way of failing, because a
    /// rejection carries no reason — expired and wrong are indistinguishable.
    CodeWrong,

    /// The server stopped honouring the session this device was holding, so
    /// the app put first run back. Nothing the user did, which is why it needs
    /// saying: they arrive here having tapped nothing.
    SignedOut,
}

/// How much of the first sync has arrived. The stage and the counts are
/// separate facts: a count climbs while its stage is still running.
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

    /// What is in the address field. Kept across a step back, so fixing a typo
    /// does not mean typing it again.
    email: String,

    /// The digits entered so far, at most [`CODE_LENGTH`] of them.
    code: String,

    fault: Option<Fault>,

    /// When the last code went out. A timestamp rather than a remaining count,
    /// because the machine reads no clock.
    sent_at: Option<Timestamp>,

    /// The bank most recently connected, which is the one the sync is for.
    bank: Option<String>,

    /// Whether a bank has ever finished connecting in this flow. Never goes
    /// back to false, unlike [`Self::bank`], which is cleared while the next
    /// connection is being made.
    linked: bool,

    /// When the server last said it did not know how the sign-in ended.
    ///
    /// While it is set, a status saying nothing is running is the gap before
    /// Plaid has spoken rather than news that a sync finished. A timestamp
    /// because the wait is bounded and the machine reads no clock.
    awaiting: Option<Timestamp>,

    /// Whether a bank sign-in is being opened, from the tap until Plaid's page
    /// has been and gone.
    opening: bool,

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

    pub(crate) fn opening(&self) -> bool {
        self.opening
    }

    pub(crate) fn linked(&self) -> bool {
        self.linked
    }

    pub(crate) fn sync(&self) -> Progress {
        self.sync
    }

    pub(crate) fn choose_email(&mut self) {
        self.step = Step::Email;
        self.fault = None;
    }

    /// The session this device held is one the server does not know. The flow
    /// is already at its first screen; this is the line that says why.
    pub(crate) fn signed_out(&mut self) {
        self.fault = Some(Fault::SignedOut);
    }

    pub(crate) fn type_email(&mut self, text: &str) {
        self.email = text.to_string();

        // A field that stayed red while being fixed argues with the user.
        if self.fault == Some(Fault::Address) {
            self.fault = None;
        }
    }

    /// Continue, on the address screen. `None` if that is not an address.
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
    /// Takes what the field holds rather than one keystroke, so a paste and a
    /// backspace are the same event as a tap.
    pub(crate) fn type_code(&mut self, digits: &str) {
        let code: String = digits
            .chars()
            .filter(char::is_ascii_digit)
            .take(CODE_LENGTH)
            .collect();

        // A seventh digit typed into six full boxes arrives as the same six.
        // That is not an edit, so the rejection below stands.
        if code == self.code {
            return;
        }

        self.code = code;

        self.fault = None;
    }

    /// Continue, under the boxes. `None` on a code that is not six digits,
    /// which the absent button should have prevented — a stale tap, not an
    /// invariant worth asserting.
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

    /// Whether the countdown is still running. A code that was never sent can
    /// be sent, which makes the button live on a screen reached by replay.
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

            // From the bank screen on there is a connection behind it or an
            // account made, and neither unwinds.
            step => step,
        };
    }

    /// Connect a bank, Try again, and Add another bank all land here.
    ///
    /// `opening` is what the button reads while the server creates a Link
    /// session and iOS raises the sheet — around a second, and a button that
    /// looks untapped is a button people tap twice.
    pub(crate) fn connect_bank(&mut self) {
        self.step = Step::Bank;
        self.opening = true;
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

    /// A bank is attached. What it is called is not known yet;
    /// [`Self::bank_named`] fills it in.
    pub(crate) fn bank_linked(&mut self) {
        self.opening = false;
        self.linked = true;
        self.bank = None;
        self.sync = Progress::default();
        self.step = Step::Sync;
    }

    /// The name, once the sync has reported one.
    pub(crate) fn bank_named(&mut self, institution: &str) {
        self.bank = Some(institution.to_string());
    }

    pub(crate) fn bank_abandoned(&mut self) {
        self.opening = false;
        self.step = Step::Failed;
    }

    /// The server does not know yet, and neither do we. The sync screen rather
    /// than the failed one, because a bank still landing arrives on the next
    /// status; [`linked`](Self::linked) stays false until one has.
    pub(crate) fn bank_pending(&mut self, now: Timestamp) {
        self.opening = false;
        self.bank = None;
        self.sync = Progress::default();
        self.step = Step::Sync;
        self.awaiting = Some(now);
    }

    pub(crate) fn sync_progressed(&mut self, sync: Progress) {
        debug_assert_eq!(
            self.step,
            Step::Sync,
            "a sync reports progress only while one is running",
        );

        // A sync that is running is a bank that exists, which is the proof a
        // pending answer was waiting for.
        self.arrived();

        self.sync = sync;
    }

    /// The last reply, which carries the totals as well as the news that there
    /// will be no more of them. They come from this reply because a sync that
    /// finishes between two ticks has no reply before it.
    pub(crate) fn sync_finished(&mut self, sync: Progress, now: Timestamp) {
        debug_assert_eq!(
            self.step,
            Step::Sync,
            "a sync reports its end only while one is running",
        );

        // Nothing is running and nothing ever was: after a pending answer there
        // may be no bank yet, and this is that gap rather than an end.
        if let Some(since) = self.awaiting.filter(|_| nothing_arrived(&self.bank, sync)) {
            // The server had already waited before it answered pending, so by
            // here Plaid is late twice over.
            if now.since(since) >= AWAIT_GOOD_FOR {
                self.awaiting = None;
                self.step = Step::Failed;
            }

            return;
        }

        self.arrived();

        self.sync = sync;
        self.step = Step::Ready;
    }

    /// A bank has finished connecting, however roundabout the news was.
    fn arrived(&mut self) {
        self.awaiting = None;
        self.linked = true;
    }
}

/// How long the phone waits after the server has given up waiting. Twice what
/// the server allows itself, because a request cannot be held open that long
/// and a screen can.
pub const AWAIT_GOOD_FOR_SECS: u64 = 20;

const AWAIT_GOOD_FOR: Duration = Duration::from_secs(AWAIT_GOOD_FOR_SECS);

/// Whether a status says anything at all. The name is the strongest signal: it
/// exists nowhere before the balances call answers.
fn nothing_arrived(bank: &Option<String>, sync: Progress) -> bool {
    bank.is_none() && sync.accounts == 0 && sync.transactions == 0 && sync.streams == 0
}

/// Whether this could be an address.
///
/// Deliberately shallow: the code arriving is what really checks it, so
/// anything stricter would reject real addresses to catch typos it cannot
/// catch anyway.
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
