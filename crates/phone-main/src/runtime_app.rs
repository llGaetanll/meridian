//! The IO half of the app: everything effectful, and nothing that decides. The
//! pairing `docs/ARCHITECTURE.md` describes: a state machine that decides
//! ([`phone_state::StateApp`]) and a runtime that acts. There is exactly one
//! signal in the whole app, holding the last screen the machine rendered, and
//! the loop is driven by user interaction rather than by a `loop`, because a
//! browser app is woken by events rather than polling for them.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use phone_state::Action;
use phone_state::ActionSource;
use phone_state::ActionUi;
use phone_state::Event;
use phone_state::EventRuntime;
use phone_state::EventSource;
use phone_state::EventUi;
use phone_state::StateApp;
use phone_state::ViewDock;
use phone_state::ViewScreen;

use crate::runtime_io;

/// Everything on screen, and the one way to change it.
#[derive(Clone, Copy)]
pub struct RuntimeApp {
    /// The machine. `StoredValue` rather than a signal: reading state
    /// reactively is what the view is not supposed to do.
    state: StoredValue<StateApp, LocalStorage>,

    /// The last screen the machine rendered. The only reactive value in the app.
    screen: RwSignal<Option<ViewScreen>>,

    dock: RwSignal<Option<ViewDock>>,

    /// The last error, for the banner.
    error: RwSignal<Option<String>>,

    /// Whether a fetch is in flight, for the refresh spinner. Deliberately
    /// *not* in the machine: a fact about the runtime rather than a decision.
    loading: RwSignal<bool>,
}

impl RuntimeApp {
    /// Start the app: provide the runtime and send the first event. Returns
    /// rather than looping, because Leptos owns the loop from here.
    pub fn run() -> Self {
        let runtime = Self::new();

        provide_context(runtime);

        // The clock first: nothing is drawn against a machine never told the time.
        runtime.tick();

        runtime.dispatch(Event::FromUi {
            event: EventUi::Started,
        });

        set_interval(move || runtime.tick(), Duration::from_secs(1));

        runtime
    }

    /// The clock, read at the edge and sent in as an event like any other.
    /// Which ticks are worth drawing for is the machine's decision.
    fn tick(&self) {
        self.dispatch(Event::FromRuntime {
            event: EventRuntime::Tick {
                now: (js_sys::Date::now() / 1000.0) as u64,
            },
        });
    }

    fn new() -> Self {
        Self {
            state: StoredValue::new_local(StateApp::new()),
            screen: RwSignal::new(None),
            dock: RwSignal::new(None),
            error: RwSignal::new(None),
            loading: RwSignal::new(false),
        }
    }

    pub fn screen(&self) -> RwSignal<Option<ViewScreen>> {
        self.screen
    }

    pub fn dock(&self) -> RwSignal<Option<ViewDock>> {
        self.dock
    }

    pub fn error(&self) -> RwSignal<Option<String>> {
        self.error
    }

    pub fn loading(&self) -> RwSignal<bool> {
        self.loading
    }

    /// Send an event in, and carry out whatever comes back — one turn of the
    /// loop. A component's click handler dispatches; it never writes a signal
    /// itself.
    pub fn dispatch(&self, event: Event) {
        let events = [event];
        let mut actions = Vec::new();

        self.state
            .update_value(|state| state.handle_events(&events, &mut actions));

        // A tick that decided nothing would bury every other turn in the log.
        let quiet_tick = actions.is_empty() && matches!(events, [Event::FromRuntime { .. }]);

        if !quiet_tick {
            tracing::debug!(?events, ?actions, "turn");
        }

        self.perform_actions(&actions);
    }

    /// Shorthand for the common case.
    pub fn ui(&self, event: EventUi) {
        self.dispatch(Event::FromUi { event });
    }

    fn perform_actions(&self, actions: &[Action]) {
        for action in actions {
            self.perform_action(action);
        }
    }

    fn perform_action(&self, action: &Action) {
        match action {
            Action::ToUi { action } => self.perform_action_ui(action),
            Action::ToSource { action } => self.perform_action_source(action),
        }
    }

    fn perform_action_ui(&self, action: &ActionUi) {
        match action {
            ActionUi::Render { screen, dock } => {
                self.screen.set(Some(screen.clone()));
                self.dock.set(dock.clone());
            }

            ActionUi::ShowError { message } => {
                tracing::warn!(message, "load failed");
                self.error.set(Some(message.clone()));
            }
        }
    }

    fn perform_action_source(&self, action: &ActionSource) {
        match action {
            ActionSource::LoadSnapshot => self.load_snapshot(),

            ActionSource::LoadSession => self.load_session(),

            // First run is drawn on the phone and not here, and `load_session`
            // keeps it out of reach — so one arriving is worth a line.
            ActionSource::SaveSession
            | ActionSource::ForgetSession
            | ActionSource::SendCode { .. }
            | ActionSource::VerifyCode { .. }
            | ActionSource::StartBankLink
            | ActionSource::WatchSync
            | ActionSource::StopWatchingSync => {
                tracing::warn!(?action, "first run is not drawn on the web");
            }
        }
    }

    /// Who the browser is signed in as: always signed in, because the phone
    /// owns first run and the web has no onboarding screens to send anyone to.
    /// Answered on the next turn, so the actions from the batch that asked are
    /// carried out before the answer starts another.
    fn load_session(&self) {
        let runtime = *self;

        spawn_local(async move {
            runtime.dispatch(Event::FromSource {
                event: EventSource::SessionLoaded { signed_in: true },
            });
        });
    }

    /// The one piece of IO that reaches the network. Whatever comes back
    /// re-enters the machine as an event: nothing here decides anything.
    fn load_snapshot(&self) {
        // Two refreshes at once race to write the same state, slower one wins.
        if self.loading.get_untracked() {
            return;
        }

        self.loading.set(true);

        let runtime = *self;

        spawn_local(async move {
            let event = match runtime_io::fetch_snapshot().await {
                Ok(json) => EventSource::SnapshotLoaded { json },
                Err(message) => EventSource::LoadFailed { message },
            };

            runtime.loading.set(false);

            if matches!(event, EventSource::SnapshotLoaded { .. }) {
                runtime.error.set(None);
            }

            runtime.dispatch(Event::FromSource { event });
        });
    }
}

pub fn use_runtime() -> RuntimeApp {
    expect_context()
}
