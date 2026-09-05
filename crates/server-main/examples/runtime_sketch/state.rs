use std::collections::HashMap;

pub type RequestId = u64;

pub type CallId = u64;

pub const CALL_GOOD_FOR_SECS: u64 = 3;

#[expect(
    clippy::enum_variant_names,
    reason = "an event names where it came from"
)]
#[derive(Debug)]
pub enum Event {
    FromRouter {
        request_id: RequestId,
        event: EventRouter,
    },
    FromSource {
        event: EventSource,
    },
    FromProcess {
        event: EventProcess,
    },
}

#[derive(Debug)]
pub enum EventRouter {
    ThingRequested { name: String },
    Pinged,
}

#[derive(Debug)]
pub enum EventSource {
    ThingLoaded { call_id: CallId, value: String },
    ThingFailed { call_id: CallId },
}

#[derive(Debug)]
pub enum EventProcess {
    Tick { now: u64 },
}

#[derive(Debug)]
pub enum Action {
    ToRouter {
        request_id: RequestId,
        action: ActionRouter,
    },
    ToSource {
        action: ActionSource,
    },
}

#[derive(Debug, Clone)]
pub enum ActionRouter {
    Serve { body: String },
    Accepted,
    Unavailable,
}

#[derive(Debug)]
pub enum ActionSource {
    FetchThing { call_id: CallId, name: String },
}

#[derive(Debug)]
struct Waiting {
    request_id: RequestId,
    since: u64,
}

#[derive(Debug, Default)]
pub struct StateServer {
    now: u64,
    next_call: CallId,
    waiting: HashMap<CallId, Waiting>,
}

impl StateServer {
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromRouter { request_id, event } => {
                self.handle_event_router(*request_id, event, actions)
            }
            Event::FromSource { event } => self.handle_event_source(event, actions),
            Event::FromProcess { event } => self.handle_event_process(event, actions),
        }
    }

    fn handle_event_router(
        &mut self,
        request_id: RequestId,
        event: &EventRouter,
        actions: &mut Vec<Action>,
    ) {
        match event {
            EventRouter::ThingRequested { name } => {
                let call_id = self.calling(request_id);

                actions.push(Action::ToSource {
                    action: ActionSource::FetchThing {
                        call_id,
                        name: name.clone(),
                    },
                });
            }

            EventRouter::Pinged => actions.push(Action::ToRouter {
                request_id,
                action: ActionRouter::Accepted,
            }),
        }
    }

    fn handle_event_source(&mut self, event: &EventSource, actions: &mut Vec<Action>) {
        match event {
            EventSource::ThingLoaded { call_id, value } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                actions.push(Action::ToRouter {
                    request_id,
                    action: ActionRouter::Serve {
                        body: value.clone(),
                    },
                });
            }

            EventSource::ThingFailed { call_id } => {
                let Some(request_id) = self.answered(*call_id) else {
                    return;
                };

                actions.push(Action::ToRouter {
                    request_id,
                    action: ActionRouter::Unavailable,
                });
            }
        }
    }

    fn handle_event_process(&mut self, event: &EventProcess, actions: &mut Vec<Action>) {
        match event {
            EventProcess::Tick { now } => {
                self.now = *now;

                self.give_up(actions);
            }
        }
    }

    fn give_up(&mut self, actions: &mut Vec<Action>) {
        let now = self.now;

        let expired: Vec<CallId> = self
            .waiting
            .iter()
            .filter(|(_, waiting)| now.saturating_sub(waiting.since) >= CALL_GOOD_FOR_SECS)
            .map(|(call_id, _)| *call_id)
            .collect();

        for call_id in expired {
            let Some(waiting) = self.waiting.remove(&call_id) else {
                unreachable!("the key was just read out of the same map")
            };

            actions.push(Action::ToRouter {
                request_id: waiting.request_id,
                action: ActionRouter::Unavailable,
            });
        }
    }

    fn calling(&mut self, request_id: RequestId) -> CallId {
        self.next_call += 1;

        self.waiting.insert(
            self.next_call,
            Waiting {
                request_id,
                since: self.now,
            },
        );

        self.next_call
    }

    fn answered(&mut self, call_id: CallId) -> Option<RequestId> {
        self.waiting.remove(&call_id).map(|w| w.request_id)
    }
}
