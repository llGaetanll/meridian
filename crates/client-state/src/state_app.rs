//! The app's state machine. Synchronous, reads no clock, opens no socket,
//! awaits nothing — which is what makes a log of events a recording of the
//! session.

use std::time::Duration;

use api_common::Timestamp;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use server_wire::WireSnapshot;
use server_wire::WireSyncStatus;

use crate::Action;
use crate::ActionSource;
use crate::ActionUi;
use crate::Event;
use crate::EventRuntime;
use crate::EventSource;
use crate::EventUi;
use crate::LoadFailure;
use crate::Screen;
use crate::StateDelete;
use crate::StateLedger;
use crate::StateOnboarding;
use crate::StateRemove;
use crate::Unpaid;
use crate::screens;
use crate::state_onboarding::Progress;
use crate::state_onboarding::Step;
use crate::state_payment::StatePayment;

/// How long to wait before opening another stream after losing one.
///
/// Long enough that a server refusing them is asked twenty times a minute
/// rather than sixty, and short enough that backgrounding the app and coming
/// straight back does not leave the screen still on a connection it has lost.
const WATCH_AGAIN_AFTER: Duration = Duration::from_secs(3);

/// Everything the app is, at one moment.
///
/// Nothing here is derived: a field that could be recomputed is a field that
/// can disagree with itself.
///
/// **The fields are private and stay private.** State changes only through
/// [`handle_events`](StateApp::handle_events); a `pub` field would be a way to
/// mutate state without an event.
#[derive(Debug)]
pub struct StateApp {
    /// Which tab.
    screen: Screen,

    /// Everything that has happened, newest first.
    entries: Vec<Entry>,

    connections: Vec<Connection>,

    streams: Vec<Stream>,

    /// The last reading the runtime sent. The machine never reads a clock
    /// itself: this moves only when an
    /// [`EventRuntime::Tick`](crate::EventRuntime) arrives.
    device_now: Timestamp,

    /// Whether figures are covered. Starts revealed: the phone draws nothing
    /// that uncovers them.
    masked: bool,

    /// The day tapped in the recurring calendar, as a day index.
    picked_day: Option<u64>,

    /// How many months the calendar has been swiped from the one the app is
    /// in. Months rather than a date, so that the clock moving over midnight
    /// carries the calendar with it.
    month_step: i32,

    /// The Ledger screen's own state. A sub-machine because the filter is the
    /// one piece of state a single screen owns; a machine per screen would be
    /// several copies of the ledger that can disagree.
    ledger: StateLedger,

    /// Set once a load fails, cleared when one succeeds. The reason rather
    /// than the sentence: what a failure reads as is decided on the way out,
    /// with every other string the app prints.
    ///
    /// The two failures the machine acts on never land here. A session the
    /// server does not know puts first run back, and one that has not paid is
    /// [`unpaid`](Self::unpaid) — neither is news to print over the screen
    /// somebody is already on.
    load_failed: Option<LoadFailure>,

    /// What the server said about a subscription it will not serve. `Some` is
    /// the paywall, in front of every other screen including first run.
    unpaid: Option<Unpaid>,

    /// Whether a snapshot has ever arrived. Distinguishes "still loading" from
    /// "loaded, and there is nothing".
    loaded: bool,

    /// Whether a stream carrying the sync's progress is open, or on its way
    /// to being. Cleared by the runtime saying the connection went.
    watching: bool,

    /// When one was last asked for, so a connection that will not stay up is
    /// retried rather than reopened as fast as it fails.
    watch_asked: Option<Timestamp>,

    /// First run, until it has been walked. `None` means the app is the app.
    ///
    /// Starts *present*, cleared by a stored session: absent until the store
    /// says otherwise would draw an empty Ledger on every launch, for the frame
    /// between launching and the runtime answering.
    onboarding: Option<StateOnboarding>,

    /// How far paying from the paywall has got. Means nothing while
    /// [`unpaid`](Self::unpaid) is `None`, and is put back to the start
    /// whenever that is cleared.
    payment: StatePayment,

    /// A bank being attached from the Connections page, from the tap until the
    /// sign-in reports. True is the button waiting instead of taking another
    /// tap. Not used during first run, which has a screen of its own to wait
    /// on.
    connecting: bool,

    /// The bank being repaired, from the Sign in tap until the sign-in
    /// reports. `Some` is that bank's button waiting instead of taking
    /// another tap.
    reconnecting: Option<String>,

    /// Removing one bank, from the trash on its block until it is cancelled
    /// or its rows are gone. `None` means nobody is removing anything.
    removing: Option<StateRemove>,

    /// Deleting the account, from the button on the Account tab until it is
    /// cancelled or the rows are gone. `None` means nobody is leaving.
    deleting: Option<StateDelete>,

    /// Who the device is signed in as, as of the last snapshot. `None` until
    /// one has arrived: the account chip has nothing to print before that and
    /// a placeholder would be a guess.
    user: Option<User>,

    /// The name being typed on the Account screen, while it differs from the
    /// kept one. `None` is a field nobody has touched, which draws the kept
    /// name — so the field has one value and the machine has one fact about
    /// it rather than two that can disagree.
    renaming: Option<String>,
}

/// Who the device is signed in as.
///
/// The wire's own shape kept out of the machine, so a field added to the
/// snapshot is a decision taken here rather than one that arrives.
#[derive(Debug, Clone)]
pub struct User {
    pub email: String,

    /// Never empty. An account is seeded with one when it is made, so the chip
    /// always has something to print and no view has to decide what to draw
    /// when it does not.
    pub name: String,
}

impl Default for StateApp {
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            entries: Vec::new(),
            connections: Vec::new(),
            streams: Vec::new(),
            device_now: Timestamp::EPOCH,
            masked: false,
            picked_day: None,
            month_step: 0,
            ledger: StateLedger::default(),
            load_failed: None,
            unpaid: None,
            loaded: false,
            watching: false,
            watch_asked: None,
            onboarding: Some(StateOnboarding::new()),
            payment: StatePayment::default(),
            connecting: false,
            reconnecting: None,
            removing: None,
            deleting: None,
            user: None,
            renaming: None,
        }
    }
}

impl StateApp {
    pub fn new() -> Self {
        Self::default()
    }

    /// The only entry point. Everything under it is one function per event
    /// group, mirroring the enums.
    ///
    /// No clock is handed in: time arrives as an
    /// [`EventRuntime::Tick`](crate::EventRuntime), which is what makes a log
    /// of the events a recording of the session.
    ///
    /// Actions are pushed into a buffer the caller owns, so a batch of events
    /// costs one allocation and a replay can reuse the same one.
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }

        // Only once every event in the batch has been handled: whether a
        // stream should be open is a fact about where the flow *is*, not about
        // which event moved it there.
        self.settle_watch(actions);
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromUi { event } => self.handle_event_ui(event, actions),
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromRuntime { event } => self.handle_event_runtime(event, actions),
        }
    }

    fn handle_event_runtime(&mut self, event: &EventRuntime, actions: &mut Vec<Action>) {
        match event {
            EventRuntime::Tick { now } => {
                let now = Timestamp::from_secs(*now);

                // Time only ever moves forward. A runtime sending a stale
                // reading would make "in 3 days" wrong in a way nothing else
                // here would catch.
                debug_assert!(
                    now >= self.device_now,
                    "time went backwards: {now} after {}",
                    self.device_now,
                );

                self.device_now = now;

                // Asking the server whether it has heard of a payment is a
                // rule about time, so it is asked here; the one tick that
                // stops the asking changes the screen.
                let stopped_asking = self.payment.tick(now, actions);

                // Drawing on every tick would rebuild the Ledger sixty times a
                // minute to arrive at the same list.
                if self.draws_on_a_tick() || stopped_asking {
                    self.render(actions);
                }
            }
        }
    }

    fn handle_event_ui(&mut self, event: &EventUi, actions: &mut Vec<Action>) {
        match event {
            EventUi::Started => {
                // Who is signed in decides whether the app opens on the Ledger
                // or on the first screen of first run; the snapshot follows.
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSession,
                });
            }

            EventUi::ScreenOpened { screen } => {
                self.screen = *screen;

                // No fetch: one snapshot feeds every screen, so switching tabs
                // is free.
            }

            EventUi::RefreshRequested => {
                // The same ask as any other: what it reaches is the server's
                // freshness window, not Plaid.
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSnapshot,
                });

                // And Check again, on a paywall that stopped asking whether a
                // payment had been heard of: another minute of asking.
                let now = self.now();

                self.payment.check_again(now);
            }

            // Only from the paywall: a session being served has nothing to buy.
            EventUi::SubscribeRequested => {
                if self.unpaid.is_some() {
                    self.payment.subscribe(actions);
                }
            }

            EventUi::MaskToggled => self.masked = !self.masked,

            EventUi::FilterGroupToggled { group } => self.ledger.toggle_group(*group),

            EventUi::CategoryToggled { id } => {
                self.ledger.toggle_category(screens::category_from_id(id))
            }

            EventUi::CategoriesCleared => self.ledger.clear_categories(),

            EventUi::RangeSelected { kind } => self.ledger.set_range(*kind),

            EventUi::CalendarDaySelected { id } => {
                // A card that could only be opened would need a close button of
                // its own, so tapping the day already picked clears it.
                let day = id.parse::<u64>().ok();

                self.picked_day = if day == self.picked_day { None } else { day };
            }

            EventUi::CalendarMonthStepped { step } => {
                self.month_step = self.month_step.saturating_add(*step);

                // The card belongs to the month it was opened in, and a swipe
                // takes that month off the screen.
                self.picked_day = None;
            }

            EventUi::TxSelected { .. } => {
                // No detail screen yet. The event exists so the view can
                // already dispatch it and the log records the taps.
            }

            // Each first-run arm is guarded rather than asserted: an event for
            // a flow that has finished is a stale tap, not a broken invariant.
            EventUi::EmailChosen => {
                if let Some(flow) = &mut self.onboarding {
                    flow.choose_email();
                }
            }

            EventUi::EmailTyped { text } => {
                if let Some(flow) = &mut self.onboarding {
                    flow.type_email(text);
                }
            }

            // The same two events on both screens that ask, because it is the
            // same question: first run's is the first answer and the Account
            // screen's is a later one. Which machine holds the draft is the
            // only difference, and first run owns it while it is up.
            EventUi::NameTyped { text } => match &mut self.onboarding {
                Some(flow) => flow.type_name(text),
                None => self.renaming = Some(text.to_string()),
            },

            // The flow moves on the tap and the write follows it. Nothing
            // waits: the name is the caller's own value, the server has no
            // opinion to answer with, and a screen held open until a write
            // lands is a screen that stops when the write is slow.
            EventUi::NameSubmitted => {
                let name = match self.onboarding.is_some() {
                    true => self
                        .onboarding
                        .as_mut()
                        .and_then(StateOnboarding::submit_name),

                    // The later answer, from the settings window. Same write,
                    // and the draft goes with it.
                    false => self.submit_rename(),
                };

                if let Some(name) = name {
                    self.user = self.user.take().map(|user| User {
                        name: name.clone(),
                        ..user
                    });

                    actions.push(Action::ToSource {
                        action: ActionSource::SetName { name },
                    });
                }
            }

            EventUi::EmailSubmitted => {
                let email = self
                    .onboarding
                    .as_mut()
                    .and_then(StateOnboarding::submit_email);

                if let Some(email) = email {
                    actions.push(Action::ToSource {
                        action: ActionSource::SendCode { email },
                    });
                }
            }

            EventUi::CodeTyped { digits } => {
                if let Some(flow) = &mut self.onboarding {
                    flow.type_code(digits);
                }
            }

            EventUi::CodeSubmitted => {
                let filled = self
                    .onboarding
                    .as_mut()
                    .and_then(StateOnboarding::submit_code);

                if let Some((email, code)) = filled {
                    actions.push(Action::ToSource {
                        action: ActionSource::VerifyCode { email, code },
                    });
                }
            }

            EventUi::ResendRequested => {
                let now = self.now();

                let email = self.onboarding.as_mut().and_then(|flow| flow.resend(now));

                if let Some(email) = email {
                    actions.push(Action::ToSource {
                        action: ActionSource::SendCode { email },
                    });
                }
            }

            EventUi::BackPressed => {
                if let Some(flow) = &mut self.onboarding {
                    flow.back();
                }
            }

            EventUi::BankConnectRequested => {
                if let Some(flow) = &mut self.onboarding {
                    flow.connect_bank();

                    actions.push(Action::ToSource {
                        action: ActionSource::StartBankLink,
                    });
                } else if !self.connecting {
                    // Add connection, on the Connections page. A second tap
                    // while one sign-in is under way would open a second
                    // sheet, as a repair's would.
                    self.connecting = true;

                    actions.push(Action::ToSource {
                        action: ActionSource::StartBankLink,
                    });
                }
            }

            EventUi::BankReconnectRequested { item_id } => {
                // First run has no Sign in to tap, and a second tap while one
                // repair is under way would open a second sheet.
                if self.onboarding.is_none() && self.reconnecting.is_none() {
                    self.reconnecting = Some(item_id.clone());

                    actions.push(Action::ToSource {
                        action: ActionSource::StartBankRepair {
                            item_id: item_id.clone(),
                        },
                    });
                }
            }

            EventUi::BankRemoveRequested { item_id } => {
                // First run has no bank to remove yet. A second trash while
                // one card is open moves the question to that bank rather
                // than being ignored — the card stands under the button that
                // was pressed, so an unanswered one somewhere else is a
                // question nobody is looking at. Not while a removal is out:
                // that one is already happening.
                if self.onboarding.is_none()
                    && !matches!(self.removing, Some(StateRemove::Sending { .. }))
                {
                    self.removing = Some(StateRemove::Asking {
                        item_id: item_id.clone(),
                    });
                }
            }

            EventUi::BankRemoveConfirmed => {
                if let Some(StateRemove::Asking { item_id } | StateRemove::Refused { item_id }) =
                    &self.removing
                {
                    let item_id = item_id.clone();

                    self.removing = Some(StateRemove::Sending {
                        item_id: item_id.clone(),
                    });

                    actions.push(Action::ToSource {
                        action: ActionSource::RemoveBank { item_id },
                    });
                }
            }

            EventUi::BankRemoveCancelled => {
                // Never while the request is out: the bank may already be
                // gone at Plaid, and a popup taken down mid-removal is a page
                // that goes on listing it.
                if !matches!(self.removing, Some(StateRemove::Sending { .. })) {
                    self.removing = None;
                }
            }

            EventUi::OnboardingFinished => {
                // Nothing has to be lifted out of the flow first: what first
                // run produced is a credential, and that is the runtime's.
                if self.onboarding.take().is_some() {
                    actions.push(Action::ToSource {
                        action: ActionSource::SaveSession,
                    });

                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot,
                    });
                }
            }

            EventUi::SignOutRequested => {
                // Every field here belongs to the account being signed out of,
                // so the way back is a machine as new as a fresh install's
                // rather than a list of things to clear one at a time.
                //
                // The clock is the exception: it belongs to the device, and a
                // machine put back to the epoch would draw the screen under
                // this tap against 1970 until the next tick corrected it.
                let now = self.device_now;

                *self = Self::new();
                self.device_now = now;

                actions.push(Action::ToSource {
                    action: ActionSource::ForgetSession,
                });
            }

            EventUi::DeleteAccountRequested => {
                // Only from the Account tab, which first run does not have.
                if self.onboarding.is_none() && self.deleting.is_none() {
                    self.deleting = Some(StateDelete::Confirming);
                }
            }

            EventUi::DeleteConfirmed => {
                if self.deleting == Some(StateDelete::Confirming) {
                    self.deleting = Some(StateDelete::SendingCode);

                    actions.push(Action::ToSource {
                        action: ActionSource::SendDeleteCode,
                    });
                }
            }

            // From any step, the account still exists and nothing has to be
            // taken back: a code already sent simply goes unused. While the
            // deletion itself is out there is no backing out of what the
            // server may already be doing, so the tap waits.
            EventUi::DeleteCancelled => {
                if self.deleting != Some(StateDelete::Deleting) {
                    self.deleting = None;
                }
            }

            EventUi::DeleteCodeTyped { digits } => {
                if let Some(StateDelete::Code {
                    digits: typed,
                    wrong,
                }) = &mut self.deleting
                {
                    *typed = digits
                        .chars()
                        .filter(char::is_ascii_digit)
                        .take(crate::state_onboarding::CODE_LENGTH)
                        .collect();

                    *wrong = false;
                }
            }

            EventUi::DeleteCodeSubmitted => {
                if let Some(StateDelete::Code { digits, .. }) = &self.deleting
                    && digits.len() == crate::state_onboarding::CODE_LENGTH
                {
                    let code = digits.clone();

                    self.deleting = Some(StateDelete::Deleting);

                    actions.push(Action::ToSource {
                        action: ActionSource::DeleteAccount { code },
                    });
                }
            }
        }

        self.render(actions);
    }

    fn handle_event_source(&mut self, event: &EventSource, actions: &mut Vec<Action>) {
        match event {
            EventSource::SnapshotLoaded { json } => {
                match serde_json::from_str::<WireSnapshot>(json) {
                    Ok(snapshot) => {
                        self.entries = snapshot.entries.iter().map(|e| e.to_entry()).collect();
                        self.connections = snapshot
                            .connections
                            .iter()
                            .map(|c| c.to_connection())
                            .collect();
                        self.streams = snapshot.streams.iter().map(|s| s.to_stream()).collect();

                        self.loaded = true;
                        self.load_failed = None;

                        // Served at all is served, so whatever the last
                        // refusal said about a subscription is no longer true,
                        // and nothing is left to pay.
                        self.unpaid = None;
                        self.payment = StatePayment::default();

                        // Which screen follows the code is this snapshot's
                        // to say. Banks on it mean the address already has an
                        // account that walked the road, on some other device,
                        // and the flow stands aside and offers the app.
                        let has_banks = !self.connections.is_empty();

                        self.user = snapshot.user.as_ref().map(|user| User {
                            email: user.email.clone(),
                            name: user.name.clone(),
                        });

                        if let Some(flow) = &mut self.onboarding {
                            flow.snapshot(has_banks);
                        }

                        debug_assert!(
                            self.entries.windows(2).all(|w| w[0].ts() >= w[1].ts()),
                            "the ledger should arrive newest first",
                        );
                    }

                    Err(e) => {
                        // A snapshot we cannot read is a failed load, not a
                        // crash. Whatever was on screen stays there.
                        tracing::warn!(error = %e, "the snapshot could not be read");

                        self.load_failed = Some(LoadFailure::Unreadable);
                    }
                }

                self.render(actions);
            }

            EventSource::SessionLoaded { signed_in } => {
                // This only ever *ends* first run: a device with nobody signed
                // in stays on the Welcome screen it is already drawing.
                if *signed_in {
                    self.onboarding = None;

                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot,
                    });
                }

                self.render(actions);
            }

            EventSource::CodeSent => {
                let now = self.now();

                if let Some(flow) = &mut self.onboarding {
                    flow.code_sent(now);
                }

                self.render(actions);
            }

            EventSource::CodeAccepted => {
                if let Some(flow) = &mut self.onboarding {
                    flow.code_accepted();

                    // Whether payment is settled is decided here, not on the
                    // bank step: the ask this starts is refused for a session
                    // that has not paid, and that refusal is the paywall.
                    // Somebody who has paid walks straight on to the bank.
                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot,
                    });
                }

                self.render(actions);
            }

            EventSource::CodeRejected => {
                if let Some(flow) = &mut self.onboarding {
                    flow.code_rejected();
                }

                self.render(actions);
            }

            EventSource::DeleteCodeSent => {
                if self.deleting == Some(StateDelete::SendingCode) {
                    self.deleting = Some(StateDelete::Code {
                        digits: String::new(),
                        wrong: false,
                    });
                }

                self.render(actions);
            }

            EventSource::DeleteCodeRejected => {
                // Emptied rather than left standing: the server counts every
                // wrong guess, so digits worth resubmitting do not exist.
                if self.deleting == Some(StateDelete::Deleting) {
                    self.deleting = Some(StateDelete::Code {
                        digits: String::new(),
                        wrong: true,
                    });
                }

                self.render(actions);
            }

            EventSource::AccountDeleted => {
                // Everything the machine holds belonged to the account that
                // is gone, so the way out is the sign-out reset: a machine as
                // new as a fresh install's, keeping only the device's clock.
                let now = self.device_now;

                *self = Self::new();
                self.device_now = now;

                actions.push(Action::ToSource {
                    action: ActionSource::ForgetSession,
                });

                self.render(actions);
            }

            EventSource::BankLinkOpened => {
                // Only first run waits behind the window on a screen of its
                // own; a repair's button already reads busy.
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_window_opened();
                }

                self.render(actions);
            }

            EventSource::BankLinked => {
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_linked();
                }

                // A bank attached, or a repair that worked: the server has
                // the Item, and a fresh snapshot is what puts it on the page
                // and takes the amber off it.
                if self.link_reported() {
                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot,
                    });
                }

                self.render(actions);
            }

            EventSource::BankPending => {
                let now = self.now();

                if let Some(flow) = &mut self.onboarding {
                    flow.bank_pending(now);
                }

                // The server has not said yet. The next snapshot decides, so
                // one is asked for rather than leaving the button waiting on
                // news that arrives by no other route.
                if self.link_reported() {
                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot,
                    });
                }

                self.render(actions);
            }

            EventSource::BankAbandoned => {
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_abandoned();
                }

                // They closed the sheet without signing in. Nothing changed
                // and the button goes back to being tappable.
                self.link_reported();

                self.render(actions);
            }

            EventSource::BankBlocked => {
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_blocked();
                }

                self.link_reported();

                self.render(actions);
            }

            EventSource::BankRemoved => {
                self.removing = None;

                // The rows are gone on the server, and a fresh snapshot is
                // what takes the bank off the page.
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSnapshot,
                });

                self.render(actions);
            }

            EventSource::BankRemoveFailed => {
                if let Some(StateRemove::Sending { item_id }) = &self.removing {
                    self.removing = Some(StateRemove::Refused {
                        item_id: item_id.clone(),
                    });
                }

                self.render(actions);
            }

            EventSource::CheckoutOpened => {
                self.payment.opened();

                self.render(actions);
            }

            EventSource::PaymentTaken => {
                let now = self.now();

                self.payment.taken(now, actions);

                self.render(actions);
            }

            EventSource::CheckoutClosed => {
                self.payment.closed();

                self.render(actions);
            }

            EventSource::SyncWatchEnded => {
                // Nothing is drawn about it and nothing is decided here: if the
                // sync screen is still up, the rule at the end of this batch
                // opens another one when the backoff allows.
                self.watching = false;
            }

            EventSource::SyncStatusLoaded { json } => {
                let now = self.now();

                // A status we cannot read leaves the screen where it is and the
                // next tick asks again, rather than throwing somebody out of
                // first run over one malformed reply.
                match serde_json::from_str::<WireSyncStatus>(json) {
                    Ok(status) => {
                        if let Some(flow) = &mut self.onboarding {
                            // The name shows up one message into the sync rather
                            // than with the bank attaching, so the screen opens
                            // saying "Your bank" and corrects itself.
                            if let Some(institution) = &status.institution {
                                flow.bank_named(institution);
                            }

                            let progress = Progress {
                                stage: status.stage(),
                                accounts: status.accounts,
                                transactions: status.transactions,
                                streams: status.streams,
                            };

                            if status.finished {
                                flow.sync_finished(progress, now);
                            } else {
                                flow.sync_progressed(progress);
                            }
                        }
                    }

                    Err(e) => {
                        tracing::warn!(error = %e, "the sync status could not be read");

                        self.load_failed = Some(LoadFailure::Unreadable);
                    }
                }

                self.render(actions);
            }

            EventSource::LoadFailed { failure } => {
                // The snapshot first run was waiting on is not coming, so the
                // screen it would have chosen has to be chosen without it.
                // Not on a lost session, which puts the flow back to its
                // first screen anyway.
                if !matches!(failure, LoadFailure::SignedOut)
                    && let Some(flow) = &mut self.onboarding
                {
                    flow.snapshot_failed();
                }

                match failure {
                    // The session is gone, and everything held under it went with
                    // it. First run is where a fresh install starts, so it is
                    // where this lands — with a line on the Welcome screen saying
                    // why, because arriving there having done nothing is otherwise
                    // unexplained.
                    LoadFailure::SignedOut => {
                        let now = self.device_now;

                        *self = Self::new();
                        self.device_now = now;

                        if let Some(flow) = &mut self.onboarding {
                            flow.signed_out();
                        }

                        actions.push(Action::ToSource {
                            action: ActionSource::ForgetSession,
                        });

                        self.render(actions);
                    }

                    // Not news and not a failure: it is where the app is until
                    // somebody pays, so it takes the screen rather than printing
                    // a banner over one.
                    LoadFailure::Unpaid { reason } => {
                        self.unpaid = Some(reason.clone());

                        // Refused while a bank sign-in was being opened is why it
                        // never opened, and first run is under the paywall until
                        // it is paid for.
                        if let Some(flow) = &mut self.onboarding {
                            flow.link_unpaid();
                        }

                        self.render(actions);
                    }

                    // Nothing to buy: the server has a subscription for this
                    // person already. Whether the app opens is the snapshot's to
                    // say, so it is asked rather than a banner drawn — a card
                    // form over a subscription that exists is the second one this
                    // is here to stop.
                    LoadFailure::Subscribed => {
                        self.payment.already(self.device_now, actions);

                        self.render(actions);
                    }

                    LoadFailure::Offline
                    | LoadFailure::TooMany
                    | LoadFailure::Server { .. }
                    | LoadFailure::Unreadable => {
                        self.load_failed = Some(failure.clone());

                        // A subscription the server could not start was not
                        // started, so Subscribe is offered again under the banner.
                        self.payment.failed();

                        // A delete flow whose request failed is closed the same
                        // way: the account stands, and the banner says why.
                        self.deleting = None;

                        // Still render: what the screen says depends on whether
                        // anything was ever loaded.
                        self.render(actions);

                        actions.push(Action::ToUi {
                            action: ActionUi::ShowError {
                                message: screens::failure(failure),
                            },
                        });
                    }
                }
            }
        }
    }

    /// Save the name the Account screen is holding. `None` when there is
    /// nothing to save — a blank field, or one that says what is already
    /// kept, both of which the absent button should have prevented.
    ///
    /// The draft is dropped either way: what is saved is what the field then
    /// draws, so a draft that survived its own save would be a second copy of
    /// the same string waiting to disagree.
    fn submit_rename(&mut self) -> Option<String> {
        let draft = self.renaming.take()?;
        let draft = draft.trim();

        let kept = self.user.as_ref().map(|user| user.name.as_str());

        match draft.is_empty() || kept == Some(draft) {
            true => None,
            false => Some(draft.to_string()),
        }
    }

    /// Hand the view the whole screen. Called at the end of every event, which
    /// is what lets the view hold no state of its own.
    fn render(&self, actions: &mut Vec<Action>) {
        actions.push(Action::ToUi {
            action: ActionUi::Render {
                screen: screens::screen(self),

                // No dock while first run or the paywall is on screen: their
                // screens are not tabs, and a dock under them would offer
                // three empty ones.
                dock: (self.onboarding.is_none() && self.unpaid.is_none())
                    .then(|| screens::dock(self.screen)),

                // The account chip keeps the dock's company for the same
                // reason: it belongs to the app, not to the road in.
                who: (self.onboarding.is_none() && self.unpaid.is_none())
                    .then(|| screens::who(self.user.as_ref()))
                    .flatten(),

                // And the window the chip opens, which is the same corner of
                // the app under another press.
                settings: (self.onboarding.is_none() && self.unpaid.is_none())
                    .then(|| screens::settings(self))
                    .flatten(),
            },
        });
    }
}

/// Everything under here is read by the screen builders in [`crate::screens`],
/// which is the only other module allowed to see inside the machine.
impl StateApp {
    pub(crate) fn screen(&self) -> Screen {
        self.screen
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub(crate) fn streams(&self) -> &[Stream] {
        &self.streams
    }

    /// Whether what is on screen is a function of the clock. Only the code
    /// screen is: its countdown reads a new value every second.
    fn draws_on_a_tick(&self) -> bool {
        match &self.onboarding {
            Some(flow) => flow.step() == Step::Code,
            None => false,
        }
    }

    /// Have a stream open exactly while the sync screen is up.
    ///
    /// Decided from where the flow is rather than pushed by the events that
    /// get it there, so a step reached by any road is watched the same way —
    /// and so a connection that dies is reopened by the same rule that opened
    /// the first one.
    fn settle_watch(&mut self, actions: &mut Vec<Action>) {
        // Not while the paywall is up. The stream is one of the routes this
        // session is refused, so asking again every three seconds is a poll
        // against an answer that will not change until somebody pays.
        let wanted = match &self.onboarding {
            Some(flow) => flow.step() == Step::Sync && self.unpaid.is_none(),
            None => false,
        };

        if !wanted {
            if self.watching {
                self.watching = false;

                actions.push(Action::ToSource {
                    action: ActionSource::StopWatchingSync,
                });
            }

            return;
        }

        if self.watching {
            return;
        }

        // Backed off. Without it a stream the server keeps refusing is one
        // request per turn, which is a poll wearing a different name.
        if let Some(asked) = self.watch_asked
            && self.now().since(asked) < WATCH_AGAIN_AFTER
        {
            return;
        }

        self.watching = true;
        self.watch_asked = Some(self.now());

        actions.push(Action::ToSource {
            action: ActionSource::WatchSync,
        });
    }

    /// What time it is, as the device last said. A phone that had to be told
    /// the time would draw nothing until the first fetch answered, and would
    /// stop moving between fetches.
    pub(crate) fn now(&self) -> Timestamp {
        self.device_now
    }

    /// Whether an Add connection tap is still being answered.
    pub(crate) fn connecting(&self) -> bool {
        self.connecting
    }

    /// A link session outside first run has reported: whichever button was
    /// waiting on it stops waiting, and the answer says whether there is
    /// anything new to ask the server for.
    fn link_reported(&mut self) -> bool {
        let repaired = self.reconnecting.take().is_some();
        let attached = self.connecting;

        self.connecting = false;

        repaired || attached
    }

    /// The bank being repaired, if a Sign in tap is still being answered.
    pub(crate) fn reconnecting(&self) -> Option<&str> {
        self.reconnecting.as_deref()
    }

    /// Where removing a bank has got, while one is being removed.
    pub(crate) fn removing(&self) -> Option<&StateRemove> {
        self.removing.as_ref()
    }

    /// Where deleting the account has got, while somebody is leaving.
    pub(crate) fn deleting(&self) -> Option<&StateDelete> {
        self.deleting.as_ref()
    }

    pub(crate) fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// What is in the Account screen's name field: the draft if there is one,
    /// otherwise the kept name.
    pub(crate) fn renaming(&self) -> Option<&str> {
        self.renaming.as_deref()
    }

    pub(crate) fn masked(&self) -> bool {
        self.masked
    }

    pub(crate) fn picked_day(&self) -> Option<u64> {
        self.picked_day
    }

    pub(crate) fn month_step(&self) -> i32 {
        self.month_step
    }

    pub(crate) fn ledger(&self) -> &StateLedger {
        &self.ledger
    }

    pub(crate) fn load_failed(&self) -> Option<&LoadFailure> {
        self.load_failed.as_ref()
    }

    /// What the server said about a subscription it will not serve, while it
    /// is saying it.
    pub(crate) fn unpaid(&self) -> Option<&Unpaid> {
        self.unpaid.as_ref()
    }

    pub(crate) fn payment(&self) -> StatePayment {
        self.payment
    }

    pub(crate) fn loaded(&self) -> bool {
        self.loaded
    }

    /// The flow, while there is one. `None` once the app is the app.
    pub(crate) fn onboarding(&self) -> Option<&StateOnboarding> {
        self.onboarding.as_ref()
    }

    /// The entries this filter lets through, newest first.
    pub(crate) fn filtered(&self) -> Vec<&Entry> {
        let now = self.now();

        self.entries
            .iter()
            .filter(|e| self.ledger.matches(e, now))
            .collect()
    }

    /// How many entries fall in each category, ignoring the category filter
    /// itself — counted through it, every unselected category would read zero.
    /// The time range still applies, because that filter is independent.
    pub(crate) fn category_counts(&self) -> Vec<(Category, usize)> {
        let now = self.now();

        Category::ALL
            .iter()
            .map(|category| {
                let count = self
                    .entries
                    .iter()
                    .filter(|e| e.category == *category && self.ledger.matches_range(e, now))
                    .count();

                (*category, count)
            })
            .collect()
    }
}

/// Read by tests. The view learns everything from the actions it is handed.
impl StateApp {
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }
}
