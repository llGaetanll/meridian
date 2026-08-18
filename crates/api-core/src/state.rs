//! The state machine. Synchronous, reads no clock, opens no socket, awaits
//! nothing — which is what makes a log of events a recording of the session.

use crate::action::Action;
use crate::action::SourceAction;
use crate::action::UiAction;
use crate::event::Event;
use crate::event::Screen;
use crate::event::SourceEvent;
use crate::event::TxSummary;
use crate::event::UiEvent;

#[derive(Debug, Default)]
pub struct State {
    txs: Vec<TxSummary>,
    selected: Option<usize>,
}

impl State {
    /// The only entry point. Everything under it is one function per event
    /// group, mirroring the enums.
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
        }
    }

    fn handle_event_ui(&mut self, event: &UiEvent, actions: &mut Vec<Action>) {
        match event {
            UiEvent::ScreenOpened { screen } => match screen {
                // Nothing is cached yet, so opening the screen is a request.
                Screen::Transactions => actions.push(Action::ToSource {
                    action: SourceAction::LoadTxs,
                }),
            },
            UiEvent::TxSelected { index } => {
                let index = *index as usize;

                // A stale list and a fresh tap can disagree. The view is not
                // the authority on what exists.
                if index < self.txs.len() {
                    self.selected = Some(index);
                }
            }
        }
    }

    fn handle_event_source(&mut self, event: &SourceEvent, actions: &mut Vec<Action>) {
        match event {
            SourceEvent::TxsLoaded { txs } => {
                self.txs = txs.clone();

                // A new list invalidates a selection made against the old one.
                self.selected = None;

                actions.push(Action::ToUi {
                    action: UiAction::ShowTxs { txs: txs.clone() },
                });
            }
            SourceEvent::LoadFailed { message } => actions.push(Action::ToUi {
                action: UiAction::ShowError {
                    message: message.clone(),
                },
            }),
        }
    }

    /// Which row is selected, if any. Read by tests; the view learns about
    /// selection through actions.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(kind: &str) -> TxSummary {
        TxSummary {
            kind: kind.to_string(),
            date: "Jun 12, 2026".to_string(),
            from: Some("6,124.00 USD".to_string()),
            to: "10.00 SPY".to_string(),
        }
    }

    /// A test is a slice of events in, a `Vec<Action>` and a `State` to assert
    /// on out. No runtime, no await, no mocks.
    fn run(events: &[Event]) -> (State, Vec<Action>) {
        let mut state = State::default();
        let mut actions = Vec::new();

        state.handle_events(events, &mut actions);

        (state, actions)
    }

    #[test]
    fn opening_the_screen_asks_for_transactions() {
        let (_, actions) = run(&[Event::FromUi {
            event: UiEvent::ScreenOpened {
                screen: Screen::Transactions,
            },
        }]);

        assert_eq!(
            actions,
            vec![Action::ToSource {
                action: SourceAction::LoadTxs
            }]
        );
    }

    #[test]
    fn loaded_transactions_are_handed_to_the_view() {
        let (state, actions) = run(&[Event::FromSource {
            event: SourceEvent::TxsLoaded {
                txs: vec![tx("Trade")],
            },
        }]);

        assert_eq!(state.txs.len(), 1);
        assert_eq!(
            actions,
            vec![Action::ToUi {
                action: UiAction::ShowTxs {
                    txs: vec![tx("Trade")]
                }
            }]
        );
    }

    #[test]
    fn a_selection_out_of_range_is_ignored() {
        let (state, _) = run(&[
            Event::FromSource {
                event: SourceEvent::TxsLoaded {
                    txs: vec![tx("Trade")],
                },
            },
            Event::FromUi {
                event: UiEvent::TxSelected { index: 7 },
            },
        ]);

        assert_eq!(state.selected(), None);
    }

    #[test]
    fn reloading_clears_a_stale_selection() {
        let (state, _) = run(&[
            Event::FromSource {
                event: SourceEvent::TxsLoaded {
                    txs: vec![tx("Trade"), tx("Deposit")],
                },
            },
            Event::FromUi {
                event: UiEvent::TxSelected { index: 1 },
            },
            Event::FromSource {
                event: SourceEvent::TxsLoaded {
                    txs: vec![tx("Trade")],
                },
            },
        ]);

        assert_eq!(state.selected(), None);
    }

    #[test]
    fn the_same_events_give_the_same_actions() {
        let events = [
            Event::FromUi {
                event: UiEvent::ScreenOpened {
                    screen: Screen::Transactions,
                },
            },
            Event::FromSource {
                event: SourceEvent::TxsLoaded {
                    txs: vec![tx("Trade")],
                },
            },
        ];

        let (_, first) = run(&events);
        let (_, second) = run(&events);

        assert_eq!(first, second);
    }
}
