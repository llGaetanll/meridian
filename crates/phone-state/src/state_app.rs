//! The app's state machine. Synchronous, reads no clock, opens no socket,
//! awaits nothing — which is what makes a log of events a recording of the
//! session.

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
use crate::Screen;
use crate::StateLedger;
use crate::StateOnboarding;
use crate::screens;
use crate::state_onboarding::Progress;
use crate::state_onboarding::Step;

/// Everything the app is, at one moment.
///
/// Deliberately small. Every field is either data that arrived from outside or
/// a choice the user made — nothing here is derived, because anything derived
/// is computed on the way out instead. A field that could be recomputed is a
/// field that can disagree with itself.
///
/// **The fields are private and stay private.** State changes only through
/// [`handle_events`](StateApp::handle_events); a `pub` field would be a way to
/// mutate state without an event, which is the one thing this design exists to
/// prevent.
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

    /// Whether figures are covered.
    ///
    /// Starts revealed. It used to start covered, so that the ledger could be
    /// opened in public without a decision — but the phone has no way to
    /// uncover them any more, and a screen of grey bars with nothing to tap is
    /// worse than one that shows the figures. The web still has its eye.
    masked: bool,

    /// The day tapped in the recurring calendar, as a day index. A choice
    /// rather than data, and the only one the Recurring screen owns — not
    /// enough to be worth a machine of its own.
    picked_day: Option<u64>,

    /// How many months the calendar has been swiped from the one the app is
    /// in. Months rather than a date, so that the clock moving over midnight
    /// carries the calendar with it.
    month_step: i32,

    /// The Ledger screen's own state.
    ///
    /// A sub-machine, because the filter is the one piece of state that belongs
    /// to a single screen. Everything else here — the entries, the connections,
    /// the streams, the mask — is read by more than one, and Home reads all of
    /// them.
    ///
    /// That is why there is no `StateBalances` or `StateRecurring`: neither
    /// screen owns any state, both being projections of data the app holds. A
    /// machine per screen would mean several copies of the ledger that can
    /// disagree, which is the failure `docs/ABOUT.md`'s one-ledger stance
    /// exists to prevent. `docs/STYLE.md` states the rule: split when a screen
    /// owns state nothing else reads.
    ledger: StateLedger,

    /// Set once a load fails, cleared when one succeeds. Drives the empty
    /// state's wording — "nothing yet" and "we could not reach the server" are
    /// different screens.
    load_failed: Option<String>,

    /// Whether a snapshot has ever arrived. Distinguishes "still loading" from
    /// "loaded, and there is nothing".
    loaded: bool,

    /// First run, until it has been walked. `None` means the app is the app.
    ///
    /// It starts *present*: a device the app has never run on is the
    /// assumption, and a stored session is what clears it. The other way round
    /// — absent until the store says there is no session — would draw an empty
    /// Ledger for the frame between launching and the runtime answering, on
    /// every launch, for the whole life of the app.
    ///
    /// A sub-machine for the same reason [`StateLedger`] is one: the address
    /// being typed and the digits so far are read by nothing outside the flow,
    /// and once the flow is through there is nothing left to read.
    onboarding: Option<StateOnboarding>,
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
            loaded: false,
            onboarding: Some(StateOnboarding::new()),
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
    /// No clock is handed in. Time arrives as an
    /// [`EventRuntime::Tick`](crate::EventRuntime) like anything else the
    /// machine cannot go and get for itself, which is what makes a log of the
    /// events a recording of the session rather than a list of things that
    /// happened at whatever time the replay runs at. A test that is *about*
    /// time sends a tick.
    ///
    /// Actions are pushed into a buffer the caller owns, so a batch of events
    /// costs one allocation and a replay can reuse the same one. The FFI
    /// boundary in `lib.rs` wraps this with an owning version, since a `&mut
    /// Vec` does not cross.
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }
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

                // A sync runs on the server and finishes whether or not
                // anybody asks, so while that screen is up the clock is what
                // makes the app go and look. Deciding it here rather than
                // looping in the runtime is what puts every poll in the log:
                // a replay makes the same number of them in the same places.
                if self.polls_on_a_tick() {
                    actions.push(Action::ToSource {
                        action: ActionSource::PollSync,
                    });
                }

                // The countdown on the code screen is the one thing in the app
                // that changes while nobody is touching it, so it is the one
                // screen a tick redraws. Drawing on every tick would rebuild
                // the Ledger sixty times a minute to arrive at the same list.
                if self.draws_on_a_tick() {
                    self.render(actions);
                }
            }
        }
    }

    fn handle_event_ui(&mut self, event: &EventUi, actions: &mut Vec<Action>) {
        match event {
            EventUi::Started => {
                // Who is signed in is asked for first, because the answer is
                // what decides whether the app opens on the Ledger or on the
                // first screen of first run. The snapshot follows from it.
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSession,
                });
            }

            EventUi::ScreenOpened { screen } => {
                self.screen = *screen;

                // No fetch: one snapshot feeds every screen, so switching tabs
                // is free. That is the whole reason the server answers with one
                // document rather than four endpoints.
            }

            EventUi::RefreshRequested => {
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSnapshot { force: true },
                });
            }

            EventUi::MaskToggled => self.masked = !self.masked,

            // Everything the Ledger owns is routed to its own machine, which
            // keeps the app's arms free of what a category chip is.
            EventUi::FilterGroupToggled { group } => self.ledger.toggle_group(*group),

            EventUi::CategoryToggled { id } => {
                self.ledger.toggle_category(screens::category_from_id(id))
            }

            EventUi::CategoriesCleared => self.ledger.clear_categories(),

            EventUi::RangeSelected { kind } => self.ledger.set_range(*kind),

            EventUi::CalendarDaySelected { id } => {
                // Tapping the day already picked clears it. A card that could
                // only be opened would need a close button of its own.
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
                // There is no detail screen yet. The event exists so the view
                // can already dispatch it, and so the log records the taps that
                // will one day open one.
            }

            // Everything first run owns is routed to its own machine. Each arm
            // is guarded rather than asserted: an event for a flow that has
            // finished is a stale tap, not a broken invariant, and dropping it
            // is the right answer.
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
                }
            }

            EventUi::OnboardingFinished => {
                // A road with an end: past this the machine holds no memory
                // of the flow at all. Nothing has to be lifted out first —
                // what first run produced is a credential, and that has been
                // the runtime's since `VerifyCode` answered.
                if self.onboarding.take().is_some() {
                    actions.push(Action::ToSource {
                        action: ActionSource::SaveSession,
                    });

                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot { force: false },
                    });
                }
            }

            EventUi::SignOutRequested => {
                // Every field here belongs to the account being signed out of
                // — the entries, the banks, the filter, which tab was open —
                // so the way back is a machine as new as a fresh install's
                // rather than a list of things to clear one at a time, which
                // is a list somebody adds a field to and forgets.
                //
                // The clock is the exception, and it is carried over rather
                // than reset: it belongs to the device and not to the account.
                // A machine put back to the epoch would draw the screen under
                // this tap against 1970 until the next tick corrected it.
                let now = self.device_now;

                *self = Self::new();
                self.device_now = now;

                actions.push(Action::ToSource {
                    action: ActionSource::ForgetSession,
                });
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

                        debug_assert!(
                            self.entries.windows(2).all(|w| w[0].ts() >= w[1].ts()),
                            "the ledger should arrive newest first",
                        );
                    }

                    Err(e) => {
                        // A snapshot we cannot read is a failed load, not a
                        // crash. Whatever was on screen stays there.
                        self.load_failed = Some(format!("could not read the snapshot: {e}"));
                    }
                }

                self.render(actions);
            }

            EventSource::SessionLoaded { signed_in } => {
                // This only ever *ends* first run — see the field's own note on
                // why it starts present. A device with nobody signed in stays
                // on the Welcome screen it is already drawing.
                if *signed_in {
                    self.onboarding = None;

                    actions.push(Action::ToSource {
                        action: ActionSource::LoadSnapshot { force: false },
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
                }

                self.render(actions);
            }

            EventSource::CodeRejected => {
                if let Some(flow) = &mut self.onboarding {
                    flow.code_rejected();
                }

                self.render(actions);
            }

            EventSource::BankLinked => {
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_linked();

                    // The bank being attached is what starts the history
                    // arriving, so the first ask follows the news rather than
                    // the runtime deciding a sync must be next. Every ask
                    // after this one is a tick's.
                    actions.push(Action::ToSource {
                        action: ActionSource::PollSync,
                    });
                }

                self.render(actions);
            }

            EventSource::BankPending => {
                let now = self.now();

                if let Some(flow) = &mut self.onboarding {
                    flow.bank_pending(now);

                    // The same first ask as a bank that did land. There may be
                    // no sync to report yet — that is what pending means — and
                    // the flow is what knows an empty status is the gap rather
                    // than the end.
                    actions.push(Action::ToSource {
                        action: ActionSource::PollSync,
                    });
                }

                self.render(actions);
            }

            EventSource::BankAbandoned => {
                if let Some(flow) = &mut self.onboarding {
                    flow.bank_abandoned();
                }

                self.render(actions);
            }

            EventSource::SyncStatusLoaded { json } => {
                let now = self.now();

                // A status we cannot read leaves the screen where it is. The
                // next tick asks again, which is a better answer than throwing
                // somebody out of first run over one malformed reply.
                match serde_json::from_str::<WireSyncStatus>(json) {
                    Ok(status) => {
                        if let Some(flow) = &mut self.onboarding {
                            // The name whenever it is there. It shows up one
                            // poll into the sync rather than with the bank
                            // being attached, so the screen opens saying
                            // "Your bank" and corrects itself.
                            if let Some(institution) = &status.institution {
                                flow.bank_named(institution);
                            }

                            // The same counts either way: whether this is
                            // the last reply decides the step and nothing
                            // else. The totals are on the finished one too,
                            // and Ready is the screen that shows them.
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
                        self.load_failed = Some(format!("could not read the sync status: {e}"));
                    }
                }

                self.render(actions);
            }

            EventSource::LoadFailed { message } => {
                self.load_failed = Some(message.clone());

                // Still render: the screen has to say something, and what it
                // says depends on whether anything was ever loaded.
                self.render(actions);

                actions.push(Action::ToUi {
                    action: ActionUi::ShowError {
                        message: message.clone(),
                    },
                });
            }
        }
    }

    /// Hand the view the whole screen.
    ///
    /// Called at the end of every event, which is what lets the view hold no
    /// state of its own: whatever it last received is what it draws.
    fn render(&self, actions: &mut Vec<Action>) {
        actions.push(Action::ToUi {
            action: ActionUi::Render {
                screen: screens::screen(self),

                // No dock while first run is on screen: its screens are not
                // tabs, and a dock under them would offer three empty ones.
                dock: self
                    .onboarding
                    .is_none()
                    .then(|| screens::dock(self.screen)),
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

    /// Whether what is on screen is a function of the clock.
    ///
    /// The code screen is, and nothing else is: its countdown reads a new
    /// value every second and then turns into a button, and the tick that
    /// turns it has to draw as much as the ones before it.
    fn draws_on_a_tick(&self) -> bool {
        match &self.onboarding {
            Some(flow) => flow.step() == Step::Code,
            None => false,
        }
    }

    /// Whether there is a sync worth asking about.
    ///
    /// Only the screen that draws one. Leaving it is what ends the polling —
    /// there is nothing to cancel, because nothing was ever started.
    fn polls_on_a_tick(&self) -> bool {
        match &self.onboarding {
            Some(flow) => flow.step() == Step::Sync,
            None => false,
        }
    }

    /// What time it is, as the device last said.
    ///
    /// The device's clock is the only one the app has and it is taken at its
    /// word. The snapshot carries the timestamps *of* things — when a sync
    /// landed, when a stream is next due — and no clock of its own: a phone
    /// that had to be told the time would draw nothing until the first fetch
    /// answered, and would stop moving between fetches.
    pub(crate) fn now(&self) -> Timestamp {
        self.device_now
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

    pub(crate) fn load_failed(&self) -> Option<&str> {
        self.load_failed.as_deref()
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
    /// itself.
    ///
    /// Ignoring it is the point: the counts on the chips have to say what
    /// selecting *that* chip would give, and a count computed through the
    /// current category selection would read zero for everything unselected.
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

/// Read by tests. The view learns everything it needs from the actions it is
/// handed, so nothing in the app calls these.
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
