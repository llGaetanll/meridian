//! The IO half of the app: everything effectful, and nothing that decides.
//!
//! The pairing `docs/ARCHITECTURE.md` describes — `client_state::StateApp`
//! decides, and this acts. The browser is a second runtime for the machine
//! the phone runs, and this mirrors `swift/Sources/MeridianApp/RuntimeApp.swift`
//! action for action: `perform_actions` fans out to `perform_action`, which
//! fans out to one function per action group.
//!
//! It holds the last screen the machine rendered and nothing else the machine
//! could have decided. The signals are plumbing that redraws the view, not a
//! place where behaviour is decided.

use std::time::Duration;

use client_state::Action;
use client_state::ActionSource;
use client_state::ActionUi;
use client_state::Event;
use client_state::EventRuntime;
use client_state::EventSource;
use client_state::EventUi;
use client_state::StateApp;
use client_state::ViewDock;
use client_state::ViewScreen;
use futures::future::AbortHandle;
use futures::future::Abortable;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::io::Failure;
use crate::io::LinkOutcome;
use crate::io::Meridian;
use crate::io::Popup;
use crate::io::clock;
use crate::io::session;

/// Provided once in `App` with `provide_context`; a page takes it with
/// `expect_context::<RuntimeApp>()`.
#[derive(Clone, Copy)]
pub struct RuntimeApp {
    /// The machine. Not readable from outside: reading state is what the view
    /// is not supposed to do. It is sent events and asked for actions.
    state: StoredValue<StateApp, LocalStorage>,

    /// The last screen the machine rendered, and the dock that came with it.
    screen: RwSignal<Option<ViewScreen>>,
    dock: RwSignal<Option<ViewDock>>,

    /// The last failure, for the banner. Cleared by the next successful load.
    error: RwSignal<Option<String>>,

    /// Whether a snapshot is in flight. A fact about the runtime rather than
    /// a decision, and the machine would have to be told when a request
    /// started in order to know it.
    loading: StoredValue<bool>,

    /// The session token, once there is one.
    ///
    /// Never sent to the machine, which is told only that a session exists.
    /// A bearer token is good for ninety days, and an event log with one in
    /// it is a way into somebody's bank.
    ///
    /// Held from the moment `/auth/verify` answers, which is before the
    /// machine says to save it: `/link/start` needs it during first run, and
    /// only `SaveSession` commits it to the browser's storage.
    token: StoredValue<Option<String>>,

    /// The stream the sync's progress arrives on, held so it can be let go.
    /// Whether there should be one is the machine's; this is only the
    /// connection `WatchSync` and `StopWatchingSync` are about.
    watch: StoredValue<Option<AbortHandle>>,

    server: StoredValue<Meridian, LocalStorage>,
}

impl RuntimeApp {
    pub fn new() -> Self {
        Self {
            state: StoredValue::new_local(StateApp::new()),
            screen: RwSignal::new(None),
            dock: RwSignal::new(None),
            error: RwSignal::new(None),
            loading: StoredValue::new(false),
            token: StoredValue::new(None),
            watch: StoredValue::new(None),
            server: StoredValue::new_local(Meridian::new()),
        }
    }

    /// Start the app: tell the machine what time it is, send the first event,
    /// and keep the clock going for as long as the app is up.
    pub fn start(&self) {
        self.tick();

        self.dispatch(Event::FromUi {
            event: EventUi::Started,
        });

        let runtime = *self;

        match set_interval_with_handle(move || runtime.tick(), Duration::from_secs(1)) {
            Ok(clock) => on_cleanup(move || clock.clear()),
            Err(_) => tracing::error!("the browser would not keep a clock; nothing will move"),
        }
    }

    /// Hand a UI event to the machine, and carry out what comes back.
    ///
    /// A click handler calls this and nothing else. It runs to the end inside
    /// the click, which is what lets a bank's window open: a browser allows
    /// one only from inside the click that asked for it.
    pub fn ui(&self, event: EventUi) {
        self.dispatch(Event::FromUi { event });
    }

    /// The last screen the machine rendered. `None` until it has rendered one.
    pub fn screen(&self) -> ReadSignal<Option<ViewScreen>> {
        self.screen.read_only()
    }

    /// The dock that came with it. `None` on a screen that is not a tab.
    pub fn dock(&self) -> ReadSignal<Option<ViewDock>> {
        self.dock.read_only()
    }

    /// What the banner says, if anything.
    pub fn error(&self) -> ReadSignal<Option<String>> {
        self.error.read_only()
    }

    /// The clock, read at the edge and sent in as an event like any other.
    ///
    /// A tick a second whatever is on screen: the machine decides which of
    /// them are worth drawing for.
    fn tick(&self) {
        self.dispatch(Event::FromRuntime {
            event: EventRuntime::Tick { now: clock::now() },
        });
    }

    fn source(&self, event: EventSource) {
        self.dispatch(Event::FromSource { event });
    }

    /// Send an event in, and carry out whatever comes back. One turn of the
    /// loop.
    fn dispatch(&self, event: Event) {
        tracing::debug!(?event, "event");

        let mut actions = Vec::new();

        let handled = self
            .state
            .try_update_value(|state| state.handle_events(&[event], &mut actions));

        if handled.is_none() {
            tracing::warn!("an event arrived after the app was taken down");

            return;
        }

        self.perform_actions(actions);
    }

    fn perform_actions(&self, actions: Vec<Action>) {
        for action in actions {
            self.perform_action(action);
        }
    }

    fn perform_action(&self, action: Action) {
        match action {
            Action::ToUi { action } => self.perform_action_ui(action),
            Action::ToSource { action } => self.perform_action_source(action),
        }
    }

    fn perform_action_ui(&self, action: ActionUi) {
        match action {
            ActionUi::Render { screen, dock } => {
                self.screen.set(Some(screen));
                self.dock.set(dock);
            }

            ActionUi::ShowError { message } => {
                tracing::debug!(message, "error shown");

                self.error.set(Some(message));
            }
        }
    }

    fn perform_action_source(&self, action: ActionSource) {
        tracing::debug!(?action, "action");

        match action {
            ActionSource::LoadSnapshot => self.load_snapshot(),
            ActionSource::LoadSession => self.load_session(),
            ActionSource::SaveSession => self.save_session(),
            ActionSource::ForgetSession => self.forget_session(),
            ActionSource::SendCode { email } => self.send_code(email),
            ActionSource::VerifyCode { email, code } => self.verify_code(email, code),
            ActionSource::StartBankLink => self.start_bank_link(),
            ActionSource::WatchSync => self.watch_sync(),
            ActionSource::StopWatchingSync => self.stop_watching_sync(),
        }
    }

    /// Whether the browser has a session, read out of its storage.
    ///
    /// Read straight away and answered on the next turn, so that the actions
    /// from the batch that asked are all carried out before the answer starts
    /// another. The token is kept here and the machine is told only that
    /// there is one.
    fn load_session(&self) {
        let token = session::load();
        let signed_in = token.is_some();

        self.token.set_value(token);

        let runtime = *self;

        spawn_local(async move {
            runtime.source(EventSource::SessionLoaded { signed_in });
        });
    }

    /// First run is through, so the token stops being something held in memory
    /// for the length of a flow and becomes what this browser is signed in as.
    fn save_session(&self) {
        match self.token.get_value() {
            Some(token) => session::save(&token),

            // Reachable only if the flow finished without a verified code,
            // which nothing in the machine allows. Worth a line, because the
            // next visit would open on first run with nothing saying why.
            None => tracing::error!("first run finished with no token to save"),
        }
    }

    /// Throw the session away: the stored copy, so the next visit opens on
    /// first run, and the one held here, so nothing in flight carries on
    /// using a token the user has just signed out of.
    fn forget_session(&self) {
        self.token.set_value(None);

        session::forget();

        // And the banner with it, which would otherwise be reporting on a
        // session that no longer exists.
        self.error.set(None);
    }

    /// Ask for a code. The server answers alike whether or not it knows the
    /// address, so there is nothing to read out of a success.
    fn send_code(&self, email: String) {
        let runtime = *self;
        let server = self.server.get_value();

        spawn_local(async move {
            let event = match server.request_code(&email).await {
                Ok(()) => EventSource::CodeSent,
                Err(failure) => EventSource::LoadFailed {
                    failure: failure.failure(),
                },
            };

            runtime.source(event);
        });
    }

    /// Trade the code for a session.
    ///
    /// The token is kept and not announced: what goes to the machine is that
    /// the code was accepted. Whether the code was merely wrong or too late is
    /// the machine's to work out.
    fn verify_code(&self, email: String, code: String) {
        let runtime = *self;
        let server = self.server.get_value();

        spawn_local(async move {
            let event = match server.verify_code(&email, &code).await {
                Ok(Some(token)) => {
                    runtime.token.set_value(Some(token));

                    EventSource::CodeAccepted
                }
                Ok(None) => EventSource::CodeRejected,
                Err(failure) => EventSource::LoadFailed {
                    failure: failure.failure(),
                },
            };

            runtime.source(event);
        });
    }

    /// The bank connection, end to end: open a Link session, put Plaid's page
    /// in front of the user, then ask the server what the session came to.
    ///
    /// One action carries all three because they are one thing to the machine
    /// — the tap was "connect a bank" and the answer is whether one is
    /// connected.
    ///
    /// The window is opened before anything is awaited, because this is still
    /// inside the click that asked for it. However it closes — by hand, or
    /// sent back to this app when Plaid is done — the server is asked, since
    /// a window closing is not an answer.
    fn start_bank_link(&self) {
        let Some(token) = self.token.get_value() else {
            tracing::error!("asked to connect a bank with no session");

            self.source(EventSource::BankAbandoned);

            return;
        };

        let popup = Popup::open();

        let runtime = *self;
        let server = self.server.get_value();

        spawn_local(async move {
            let Some(popup) = popup else {
                tracing::warn!("the browser would not open a window for the bank");

                runtime.source(EventSource::BankAbandoned);

                return;
            };

            let page = match server.start_link(&token).await {
                Ok(page) => page,
                Err(failure) => {
                    popup.close();

                    runtime.abandoned(failure);

                    return;
                }
            };

            popup.show(&page);
            popup.gone().await;

            match server.finish_link(&token).await {
                Ok(LinkOutcome::Linked) => runtime.source(EventSource::BankLinked),
                Ok(LinkOutcome::Abandoned) => runtime.source(EventSource::BankAbandoned),
                Ok(LinkOutcome::Pending) => runtime.source(EventSource::BankPending),
                Err(failure) => runtime.abandoned(failure),
            }
        });
    }

    /// A failure to reach the server on either Link call leaves no bank
    /// attached, which is the same screen as backing out — with the reason in
    /// the banner behind it.
    fn abandoned(&self, failure: Failure) {
        self.source(EventSource::LoadFailed {
            failure: failure.failure(),
        });
        self.source(EventSource::BankAbandoned);
    }

    /// Open the stream the sync's progress arrives on, and report what
    /// arrives on it.
    ///
    /// The reading of one connection and not a decision to keep asking: when
    /// to open another after losing this one is the machine's. Every way it
    /// can end is the same event, except the machine closing it — it knows,
    /// and a stream it let go of is not a stream that went away.
    fn watch_sync(&self) {
        let Some(token) = self.token.get_value() else {
            tracing::error!("asked to watch a sync with no session");

            self.source(EventSource::SyncWatchEnded);

            return;
        };

        // The machine decides when there is one, so a second is a bug rather
        // than a race to live with.
        self.stop_watching_sync();

        let (watch, registration) = AbortHandle::new_pair();

        self.watch.set_value(Some(watch.clone()));

        let runtime = *self;
        let server = self.server.get_value();

        spawn_local(async move {
            let statuses = server.sync_statuses(&token, |json| {
                // Let go of part way through a chunk: the rest of it is for a
                // stream the machine no longer has.
                if !watch.is_aborted() {
                    runtime.source(EventSource::SyncStatusLoaded { json });
                }
            });

            match Abortable::new(statuses, registration).await {
                Err(_) => return,

                // The server would not open it. Reported, so that the machine
                // is not left reopening a stream it will never be given.
                Ok(Err(failure)) => runtime.source(EventSource::LoadFailed {
                    failure: failure.failure(),
                }),

                Ok(Ok(())) => {}
            }

            runtime.source(EventSource::SyncWatchEnded);
        });
    }

    /// Let the stream go. Dropping the request is what closes the connection
    /// and frees the slot the server is holding for it.
    fn stop_watching_sync(&self) {
        if let Some(watch) = self.watch.try_update_value(Option::take).flatten() {
            watch.abort();
        }
    }

    /// The snapshot. Whatever comes back re-enters the machine as an event;
    /// nothing here inspects the answer.
    fn load_snapshot(&self) {
        // Two at once would race to write the same state, and the slower one
        // would win. One at a time.
        if self.loading.get_value() {
            return;
        }

        self.loading.set_value(true);

        let runtime = *self;
        let server = self.server.get_value();
        let token = self.token.get_value();

        spawn_local(async move {
            // `/snapshot` has no unauthenticated form, so with nothing to send
            // there is no request to make. Nothing to send is the same fact
            // as a token the server will not honour.
            let loaded = match token {
                Some(token) => server.snapshot(&token).await,
                None => Err(Failure::NoSession),
            };

            runtime.loading.set_value(false);

            let event = match loaded {
                Ok(json) => {
                    // A successful load clears whatever the last failure said.
                    runtime.error.set(None);

                    EventSource::SnapshotLoaded { json }
                }
                Err(failure) => EventSource::LoadFailed {
                    failure: failure.failure(),
                },
            };

            runtime.source(event);
        });
    }
}
