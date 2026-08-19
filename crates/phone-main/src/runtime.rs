//! The loop: `event -> state machine -> action -> IO -> event`.
//!
//! This is the "thin runtime" `docs/ARCHITECTURE.md` describes, and it is the
//! only place in the front end that is allowed to do anything. It holds no
//! decisions — it dispatches events, and it carries out the actions it gets
//! back.
//!
//! There is exactly one signal in the whole app, holding the last screen the
//! core rendered. That is the state minimisation the docs ask for, taken as
//! far as it goes: the components below read that signal and nothing else,
//! which is what makes them a straight transcription target for SwiftUI and
//! Compose. Neither of those has signals, and neither will need them.

use api_core::Action;
use api_core::DockView;
use api_core::Event;
use api_core::ScreenView;
use api_core::SourceAction;
use api_core::SourceEvent;
use api_core::State;
use api_core::UiAction;
use api_core::UiEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::io;

/// Everything on screen, and the one way to change it.
#[derive(Clone, Copy)]
pub struct Runtime {
    /// The machine. `StoredValue` rather than a signal: nothing reads it
    /// reactively, because reading state is what the view is not supposed to
    /// do. It is written to and asked for actions.
    state: StoredValue<State, LocalStorage>,

    /// The last screen the core rendered. The only reactive value in the app.
    screen: RwSignal<Option<ScreenView>>,

    dock: RwSignal<Option<DockView>>,

    /// The last error, for the banner. Cleared by the next successful load.
    error: RwSignal<Option<String>>,

    /// Whether a fetch is in flight, for the refresh spinner. Deliberately
    /// *not* in the core: it is a fact about the runtime rather than a
    /// decision, and the core would have to be told when a request started to
    /// know it.
    loading: RwSignal<bool>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            state: StoredValue::new_local(State::default()),
            screen: RwSignal::new(None),
            dock: RwSignal::new(None),
            error: RwSignal::new(None),
            loading: RwSignal::new(false),
        }
    }

    pub fn screen(&self) -> RwSignal<Option<ScreenView>> {
        self.screen
    }

    pub fn dock(&self) -> RwSignal<Option<DockView>> {
        self.dock
    }

    pub fn error(&self) -> RwSignal<Option<String>> {
        self.error
    }

    pub fn loading(&self) -> RwSignal<bool> {
        self.loading
    }

    /// Send an event in, and carry out whatever comes back.
    ///
    /// Every interaction in the app goes through here. A component's click
    /// handler dispatches; it never writes a signal itself.
    pub fn dispatch(&self, event: Event) {
        let mut actions = Vec::new();

        self.state
            .update_value(|state| state.handle_events(&[event], &mut actions));

        for action in actions {
            self.run(action);
        }
    }

    fn run(&self, action: Action) {
        match action {
            Action::ToUi { action } => match action {
                UiAction::Render { screen, dock } => {
                    self.screen.set(Some(screen));
                    self.dock.set(Some(dock));
                }

                UiAction::ShowError { message } => {
                    tracing::warn!(target: "app", message, "load failed");
                    self.error.set(Some(message));
                }
            },

            Action::ToSource { action } => match action {
                SourceAction::LoadSnapshot { force } => self.load(force),
            },
        }
    }

    /// The one piece of IO, and the only `async` in the front end.
    ///
    /// Whatever comes back re-enters the machine as an event, which is what
    /// closes the loop: nothing here inspects the answer or decides anything
    /// about it.
    fn load(&self, force: bool) {
        // Two refreshes at once would race to write the same state, and the
        // slower one would win. One at a time.
        if self.loading.get_untracked() {
            return;
        }

        self.loading.set(true);

        let runtime = *self;

        spawn_local(async move {
            tracing::info!(target: "app", force, "snapshot asked for");

            let event = match io::fetch_snapshot(force).await {
                Ok(json) => {
                    // `now` is in the snapshot; this is the fallback for a
                    // server that did not send one.
                    Event::FromSource {
                        event: SourceEvent::SnapshotLoaded { json, now: 0 },
                    }
                }

                Err(message) => Event::FromSource {
                    event: SourceEvent::LoadFailed { message },
                },
            };

            runtime.loading.set(false);

            // A successful load clears whatever the last failure said.
            if matches!(
                event,
                Event::FromSource {
                    event: SourceEvent::SnapshotLoaded { .. }
                }
            ) {
                runtime.error.set(None);
            }

            runtime.dispatch(event);
        });
    }

    /// Shorthand for the common case.
    pub fn ui(&self, event: UiEvent) {
        self.dispatch(Event::FromUi { event });
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub fn provide_runtime() -> Runtime {
    let runtime = Runtime::new();
    provide_context(runtime);
    runtime
}

pub fn use_runtime() -> Runtime {
    expect_context()
}
