//! The IO half of the app: everything effectful, and nothing that decides.
//!
//! The pairing `docs/ARCHITECTURE.md` describes — a state machine that decides
//! ([`phone_state::StateApp`]) and a runtime that acts. The function structure
//! mirrors the machine's: `perform_actions` fans out to `perform_action`,
//! which fans out to one function per action group.
//!
//! There is exactly one signal in the whole app, holding the last screen the
//! machine rendered. That is the state minimisation the docs ask for, taken as
//! far as it goes: the components read that signal and nothing else, which is
//! what makes them a straight transcription target for SwiftUI and Compose.
//! Neither of those has signals, and neither will need them.
//!
//! The loop is driven by user interaction rather than by a `loop`, because a
//! browser app is woken by events rather than polling for them. `dispatch` is
//! one turn of the loop the server's runtime writes out in full.

use api_common::Timestamp;
use leptos::prelude::*;
use leptos::task::spawn_local;
use phone_state::Action;
use phone_state::ActionSource;
use phone_state::ActionUi;
use phone_state::Event;
use phone_state::EventSource;
use phone_state::EventUi;
use phone_state::StateApp;
use phone_state::ViewDock;
use phone_state::ViewScreen;

use crate::runtime_io;

/// Everything on screen, and the one way to change it.
#[derive(Clone, Copy)]
pub struct RuntimeApp {
    /// The machine. `StoredValue` rather than a signal: nothing reads it
    /// reactively, because reading state is what the view is not supposed to
    /// do. It is written to and asked for actions.
    state: StoredValue<StateApp, LocalStorage>,

    /// The last screen the machine rendered. The only reactive value in the app.
    screen: RwSignal<Option<ViewScreen>>,

    dock: RwSignal<Option<ViewDock>>,

    /// The last error, for the banner. Cleared by the next successful load.
    error: RwSignal<Option<String>>,

    /// Whether a fetch is in flight, for the refresh spinner. Deliberately
    /// *not* in the machine: it is a fact about the runtime rather than a
    /// decision, and the machine would have to be told when a request started
    /// in order to know it.
    loading: RwSignal<bool>,
}

impl RuntimeApp {
    /// Start the app: provide the runtime and send the first event.
    ///
    /// The counterpart to the server's `run`. It returns rather than looping,
    /// because Leptos owns the loop from here.
    pub fn run() -> Self {
        let runtime = Self::new();

        provide_context(runtime);

        runtime.dispatch(Event::FromUi {
            event: EventUi::Started,
        });

        runtime
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

    /// Send an event in, and carry out whatever comes back. One turn of the
    /// loop.
    ///
    /// Every interaction in the app goes through here. A component's click
    /// handler dispatches; it never writes a signal itself.
    pub fn dispatch(&self, event: Event) {
        let events = [event];
        let mut actions = Vec::new();

        // The clock is read here, at the edge, and handed in. Nothing below
        // this line reads one — which is what makes a replay of the event log
        // land where it did the first time.
        let now = unix_now();

        self.state
            .update_value(|state| state.handle_events(now, &events, &mut actions));

        tracing::debug!(?events, ?actions, "handled");

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
                self.dock.set(Some(dock.clone()));
            }

            ActionUi::ShowError { message } => {
                tracing::warn!(message, "load failed");
                self.error.set(Some(message.clone()));
            }
        }
    }

    fn perform_action_source(&self, action: &ActionSource) {
        match action {
            ActionSource::LoadSnapshot { force } => self.load_snapshot(*force),
        }
    }

    /// The one piece of IO, and the only `async` in the front end.
    ///
    /// Whatever comes back re-enters the machine as an event, which is what
    /// closes the loop: nothing here inspects the answer or decides anything
    /// about it.
    fn load_snapshot(&self, force: bool) {
        // Two refreshes at once would race to write the same state, and the
        // slower one would win. One at a time.
        if self.loading.get_untracked() {
            return;
        }

        self.loading.set(true);

        let runtime = *self;

        spawn_local(async move {
            tracing::info!(force, "snapshot asked for");

            let event = match runtime_io::fetch_snapshot(force).await {
                Ok(json) => EventSource::SnapshotLoaded { json },
                Err(message) => EventSource::LoadFailed { message },
            };

            runtime.loading.set(false);

            // A successful load clears whatever the last failure said.
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

/// The wall clock, in seconds.
///
/// Read at the edge and handed to the machine, which has none of its own. The
/// server's clock rides in with the snapshot and is the one used for anything
/// the user sees — a phone with a wrong date should not render a confidently
/// wrong "in 3 days" — but the app has to draw something before the first
/// snapshot lands.
fn unix_now() -> Timestamp {
    Timestamp::from_secs((js_sys::Date::now() / 1000.0) as u64)
}
