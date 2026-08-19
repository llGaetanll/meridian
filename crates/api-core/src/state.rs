//! The state machine. Synchronous, reads no clock, opens no socket, awaits
//! nothing — which is what makes a log of events a recording of the session.

use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use api_wire::Snapshot;

use crate::action::Action;
use crate::action::SourceAction;
use crate::action::UiAction;
use crate::event::Event;
use crate::event::FilterGroup;
use crate::event::RangeSelection;
use crate::event::RangeUnit;
use crate::event::SourceEvent;
use crate::event::UiEvent;
use crate::fmt;
use crate::screens;
use crate::view::Screen;

/// Everything the app is, at one moment.
///
/// Deliberately small. Almost every field is either data that arrived from
/// outside or a choice the user made — nothing here is derived, because
/// anything derived is computed on the way out instead. A field that could be
/// recomputed is a field that can disagree with itself.
#[derive(Debug)]
pub struct State {
    /// Which tab.
    pub(crate) screen: Screen,

    /// Everything that has happened, newest first.
    pub(crate) entries: Vec<Entry>,

    pub(crate) connections: Vec<Connection>,

    pub(crate) streams: Vec<Stream>,

    /// The server's clock, as of the last snapshot. The machine never reads a
    /// clock itself, so this is the only "now" it has — see `docs/DECISIONS.md`.
    pub(crate) now: u64,

    /// Whether figures are covered. Starts covered, so the ledger can be
    /// opened in public without a decision.
    pub(crate) masked: bool,

    pub(crate) filter: Filter,

    /// Set once a load fails, cleared when one succeeds. Drives the empty
    /// state's wording — "nothing yet" and "we could not reach the server"
    /// are different screens.
    pub(crate) load_failed: Option<String>,

    /// Whether a snapshot has ever arrived. Distinguishes "still loading" from
    /// "loaded, and there is nothing".
    pub(crate) loaded: bool,
}

impl Default for State {
    /// Figures start **covered**.
    ///
    /// This is the one field whose default is not `Default::default()`, and it
    /// is deliberate: the app should be openable on a train without first
    /// deciding to hide anything. Deriving `Default` here would silently start
    /// the app revealed, which is why it is written out.
    fn default() -> Self {
        Self {
            screen: Screen::default(),
            entries: Vec::new(),
            connections: Vec::new(),
            streams: Vec::new(),
            now: 0,
            masked: true,
            filter: Filter::default(),
            load_failed: None,
            loaded: false,
        }
    }
}

/// What the ledger is filtered by.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub open: bool,
    pub categories_open: bool,
    pub range_open: bool,

    /// Empty means everything, rather than nothing. That is the only reading
    /// that makes an unused filter invisible.
    pub categories: Vec<Category>,

    pub range: RangeSelection,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            open: false,
            // Both groups start open, because a panel that opens showing two
            // collapsed headings has told the user nothing.
            categories_open: true,
            range_open: true,
            categories: Vec::new(),
            range: RangeSelection::All,
        }
    }
}

impl Filter {
    /// Whether this filter is actually narrowing anything.
    pub fn is_active(&self) -> bool {
        !self.categories.is_empty() || self.range != RangeSelection::All
    }

    /// The window this range covers, as timestamps. `None` at either end means
    /// unbounded.
    pub fn window(&self, now: u64) -> (Option<u64>, Option<u64>) {
        match self.range {
            RangeSelection::All => (None, None),

            // Exact: "this month" is from the first of the month.
            RangeSelection::This { unit } => (Some(fmt::start_of(unit, now)), None),

            // Approximate, and meant to be: "last 3 months" is about three
            // months back, not a calendar boundary.
            RangeSelection::Last { count, unit } => {
                let span = unit.seconds().saturating_mul(count.max(1) as u64);
                (Some(now.saturating_sub(span)), None)
            }

            RangeSelection::Between { start, end } => {
                // The end is a day, and a day includes its own transactions,
                // so the window runs to the end of it rather than to midnight
                // at its start.
                (Some(start), Some(end.saturating_add(86_400)))
            }
        }
    }

    /// Whether an entry survives this filter.
    pub fn matches(&self, entry: &Entry, now: u64) -> bool {
        if !self.categories.is_empty() && !self.categories.contains(&entry.category) {
            return false;
        }

        let (start, end) = self.window(now);
        let ts = entry.ts();

        if let Some(start) = start
            && ts < start
        {
            return false;
        }

        if let Some(end) = end
            && ts >= end
        {
            return false;
        }

        true
    }
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
            UiEvent::Started { now } => {
                // The clock arrives with the event. Until a snapshot lands
                // this is the only "now" the app has, and the loading screen
                // needs one to be drawn at all.
                self.now = *now;

                actions.push(Action::ToSource {
                    action: SourceAction::LoadSnapshot { force: false },
                });
            }

            UiEvent::ScreenOpened { screen } => {
                self.screen = *screen;

                // No fetch: one snapshot feeds every screen, so switching tabs
                // is free. That is the whole reason the server answers with
                // one document rather than four endpoints.
            }

            UiEvent::RefreshRequested => {
                actions.push(Action::ToSource {
                    action: SourceAction::LoadSnapshot { force: true },
                });
            }

            UiEvent::MaskToggled => self.masked = !self.masked,

            UiEvent::FilterToggled => self.filter.open = !self.filter.open,

            UiEvent::FilterGroupToggled { group } => match group {
                FilterGroup::Categories => {
                    self.filter.categories_open = !self.filter.categories_open
                }
                FilterGroup::Range => self.filter.range_open = !self.filter.range_open,
            },

            UiEvent::CategoryToggled { id } => {
                let category = screens::category_from_id(id);

                // An id the view made up is ignored rather than filed under
                // Unknown: a typo should narrow nothing, not silently filter
                // to a category nothing is in.
                if category == Category::Unknown {
                    return;
                }

                match self.filter.categories.iter().position(|c| *c == category) {
                    Some(at) => {
                        self.filter.categories.remove(at);
                    }
                    None => self.filter.categories.push(category),
                }
            }

            UiEvent::CategoriesCleared => self.filter.categories.clear(),

            UiEvent::RangeSelected { kind } => self.filter.range = *kind,

            UiEvent::TxSelected { .. } => {
                // There is no detail screen yet. The event exists so the view
                // can already dispatch it, and so the log records the taps
                // that will one day open one.
            }
        }

        self.render(actions);
    }

    fn handle_event_source(&mut self, event: &SourceEvent, actions: &mut Vec<Action>) {
        match event {
            SourceEvent::SnapshotLoaded { json, now } => {
                match serde_json::from_str::<Snapshot>(json) {
                    Ok(snapshot) => {
                        self.entries = snapshot.entries.iter().map(|e| e.to_entry()).collect();
                        self.connections = snapshot
                            .connections
                            .iter()
                            .map(|c| c.to_connection())
                            .collect();
                        self.streams = snapshot.streams.iter().map(|s| s.to_stream()).collect();

                        // The server's clock, not the phone's. A device with a
                        // wrong date would otherwise render a confidently
                        // wrong "in 3 days".
                        self.now = if snapshot.now > 0 { snapshot.now } else { *now };

                        self.loaded = true;
                        self.load_failed = None;
                    }

                    Err(e) => {
                        // A snapshot we cannot read is a failed load, not a
                        // crash. Whatever was on screen stays there.
                        self.load_failed = Some(format!("could not read the snapshot: {e}"));
                    }
                }

                self.render(actions);
            }

            SourceEvent::LoadFailed { message } => {
                self.load_failed = Some(message.clone());

                // Still render: the screen has to say something, and what it
                // says depends on whether anything was ever loaded.
                self.render(actions);

                actions.push(Action::ToUi {
                    action: UiAction::ShowError {
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
            action: UiAction::Render {
                screen: screens::screen(self),
                dock: screens::dock(self.screen),
            },
        });
    }

    /// How much is loaded. Read by tests; the view learns everything it needs
    /// from the actions it is handed.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// The entries this filter lets through, newest first.
    pub(crate) fn filtered(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.filter.matches(e, self.now))
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
        let range_only = Filter {
            categories: Vec::new(),
            ..self.filter.clone()
        };

        Category::ALL
            .iter()
            .map(|category| {
                let count = self
                    .entries
                    .iter()
                    .filter(|e| e.category == *category && range_only.matches(e, self.now))
                    .count();

                (*category, count)
            })
            .collect()
    }

    /// Which range unit is currently chosen, for drawing the sentence's
    /// tokens. The tokens keep their last value even when another form is
    /// live, so switching back does not reset the sentence.
    pub(crate) fn range_units(&self) -> (RangeUnit, u32, RangeUnit) {
        match self.filter.range {
            RangeSelection::This { unit } => (unit, 3, RangeUnit::Month),
            RangeSelection::Last { count, unit } => (RangeUnit::Month, count, unit),
            _ => (RangeUnit::Month, 3, RangeUnit::Month),
        }
    }
}
