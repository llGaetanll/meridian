mod runtime_router;
mod state;

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinSet;

use runtime_router::ActionRx;
use runtime_router::ActionTx;
use runtime_router::EventRx;
use runtime_router::EventTx;
use runtime_router::RuntimeRouter;
use state::Action;
use state::ActionRouter;
use state::ActionSource;
use state::CallId;
use state::Event;
use state::EventProcess;
use state::EventSource;
use state::RequestId;
use state::StateServer;

const TICK_EVERY: Duration = Duration::from_secs(1);

struct RuntimeServer {
    action_tx: ActionTx,
}

impl RuntimeServer {
    async fn run() -> std::io::Result<()> {
        let mut state = StateServer::default();

        let (action_tx, action_rx): (ActionTx, ActionRx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx): (EventTx, EventRx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            if let Err(e) = RuntimeRouter::run(action_rx, event_tx).await {
                tracing::error!(error = %e, "the router stopped");
            }
        });

        let runtime = Self { action_tx };

        let mut clock = tokio::time::interval(TICK_EVERY);

        let mut calls: JoinSet<Vec<Event>> = JoinSet::new();

        let mut events: Vec<Event> = Vec::new();
        let mut actions: Vec<Action> = Vec::new();

        loop {
            tokio::select! {
                _ = clock.tick() => events.push(Event::FromProcess {
                    event: EventProcess::Tick { now: unix_now() },
                }),

                Some((request_id, event)) = event_rx.recv() => events.push(Event::FromRouter {
                    request_id,
                    event,
                }),

                Some(landed) = calls.join_next() => match landed {
                    Ok(landed) => events.extend(landed),

                    Err(e) => tracing::error!(error = %e, "a call did not finish"),
                },
            }

            state.handle_events(&events, &mut actions);
            runtime.perform_actions(&actions, &mut calls);

            tracing::debug!(?events, ?actions, "turn");

            events.clear();
            actions.clear();
        }
    }

    fn perform_actions(&self, actions: &[Action], calls: &mut JoinSet<Vec<Event>>) {
        for action in actions {
            self.perform_action(action, calls);
        }
    }

    fn perform_action(&self, action: &Action, calls: &mut JoinSet<Vec<Event>>) {
        match action {
            Action::ToRouter { request_id, action } => {
                self.perform_action_router(*request_id, action)
            }

            Action::ToSource { action } => self.perform_action_source(action, calls),
        }
    }

    fn perform_action_router(&self, request_id: RequestId, action: &ActionRouter) {
        if self.action_tx.send((request_id, action.clone())).is_err() {
            tracing::error!(request_id, "the router is gone; its answer was dropped");
        }
    }

    fn perform_action_source(&self, action: &ActionSource, calls: &mut JoinSet<Vec<Event>>) {
        match action {
            ActionSource::FetchThing { call_id, name } => {
                let (call_id, name) = (*call_id, name.clone());

                calls.spawn(async move { fetch_thing(call_id, &name).await });
            }
        }
    }
}

async fn fetch_thing(call_id: CallId, name: &str) -> Vec<Event> {
    let event = match name {
        "slow" => {
            tokio::time::sleep(Duration::from_secs(10)).await;

            EventSource::ThingFailed { call_id }
        }

        name => {
            tokio::time::sleep(Duration::from_millis(200)).await;

            EventSource::ThingLoaded {
                call_id,
                value: format!("hello, {name}\n"),
            }
        }
    };

    vec![Event::FromSource { event }]
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    api_trace::init(api_trace::Config::default());

    RuntimeServer::run().await
}
