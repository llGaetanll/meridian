//! The app's state machine. Synchronous, reads no clock, opens no socket,
//! awaits nothing — which is what makes a log of events a recording of the
//! session.

use api_common::Timestamp;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::Entry;
use api_ledger::Stream;
use server_wire::WireSnapshot;

use crate::Action;
use crate::ActionSource;
use crate::ActionUi;
use crate::Event;
use crate::EventSource;
use crate::EventUi;
use crate::FilterGroup;
use crate::RangeSelection;
use crate::RangeUnit;
use crate::Screen;
use crate::fmt;
use crate::screens;

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

    /// When the current batch of events is being handled, as the runtime
    /// reported it. The machine never reads a clock itself.
    device_now: Timestamp,

    /// The server's clock as of the last snapshot, and the device's reading at
    /// the moment it arrived.
    ///
    /// Kept as a pair so that time still passes between snapshots: `now()`
    /// returns the server's reading plus however long the device says has
    /// elapsed since. Trusting the device outright would render a wrong date
    /// on a phone with a wrong clock; trusting the server outright would
    /// freeze "in 3 days" until the next fetch.
    server_clock: Option<(Timestamp, Timestamp)>,

    /// Whether figures are covered. Starts covered, so the ledger can be
    /// opened in public without a decision.
    masked: bool,

    filter: Filter,

    /// Set once a load fails, cleared when one succeeds. Drives the empty
    /// state's wording — "nothing yet" and "we could not reach the server" are
    /// different screens.
    load_failed: Option<String>,

    /// Whether a snapshot has ever arrived. Distinguishes "still loading" from
    /// "loaded, and there is nothing".
    loaded: bool,
}

/// What the ledger is filtered by.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Filter {
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

    /// The window this range covers. `None` at either end means unbounded.
    pub fn window(&self, now: Timestamp) -> (Option<Timestamp>, Option<Timestamp>) {
        match self.range {
            RangeSelection::All => (None, None),

            // Exact: "this month" is from the first of the month.
            RangeSelection::This { unit } => (Some(fmt::start_of(unit, now)), None),

            // Approximate, and meant to be: "last 3 months" is about three
            // months back, not a calendar boundary.
            RangeSelection::Last { count, unit } => {
                let span = unit.seconds().saturating_mul(count.max(1) as u64);
                (Some(now.saturating_sub_secs(span)), None)
            }

            RangeSelection::Between { .. } => {
                let Some((start, end)) = self.range.window_bounds() else {
                    return (None, None);
                };

                // The end is a day, and a day includes its own transactions,
                // so the window runs to the end of it rather than to midnight
                // at its start.
                (Some(start), Some(end.saturating_add_secs(86_400)))
            }
        }
    }

    /// Whether an entry survives this filter.
    pub fn matches(&self, entry: &Entry, now: Timestamp) -> bool {
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

impl Default for StateApp {
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
            device_now: Timestamp::EPOCH,
            server_clock: None,
            masked: true,
            filter: Filter::default(),
            load_failed: None,
            loaded: false,
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
    /// `now` is handed in once per batch rather than stapled to every event:
    /// a batch happens at one instant, and an event that had to invent a clock
    /// reading would be an event the runtime could get wrong. The machine
    /// never reads a clock itself, which is what makes a replay of an event log
    /// land where it did the first time.
    ///
    /// Actions are pushed into a buffer the caller owns, so a batch of events
    /// costs one allocation and a replay can reuse the same one. The FFI
    /// boundary in `lib.rs` wraps this with an owning version, since a `&mut
    /// Vec` does not cross.
    pub fn handle_events(&mut self, now: Timestamp, events: &[Event], actions: &mut Vec<Action>) {
        // Time only ever moves forward. A runtime handing in a stale reading
        // would make "in 3 days" wrong in a way nothing else would catch.
        debug_assert!(
            now >= self.device_now,
            "time went backwards: {now} after {}",
            self.device_now,
        );

        self.device_now = now;

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

    fn handle_event_ui(&mut self, event: &EventUi, actions: &mut Vec<Action>) {
        match event {
            EventUi::Started => {
                actions.push(Action::ToSource {
                    action: ActionSource::LoadSnapshot { force: false },
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

            EventUi::FilterToggled => self.filter.open = !self.filter.open,

            EventUi::FilterGroupToggled { group } => match group {
                FilterGroup::Categories => {
                    self.filter.categories_open = !self.filter.categories_open
                }
                FilterGroup::Range => self.filter.range_open = !self.filter.range_open,
            },

            EventUi::CategoryToggled { id } => {
                let category = screens::category_from_id(id);

                // An id the view made up is ignored rather than filed under
                // Unknown: a typo should narrow nothing, not silently filter to
                // a category nothing is in.
                if category == Category::Unknown {
                    return;
                }

                match self.filter.categories.iter().position(|c| *c == category) {
                    Some(at) => {
                        self.filter.categories.remove(at);
                    }
                    None => self.filter.categories.push(category),
                }

                debug_assert!(
                    self.filter.categories.len() <= Category::ALL.len(),
                    "a category was selected twice",
                );
            }

            EventUi::CategoriesCleared => self.filter.categories.clear(),

            EventUi::RangeSelected { kind } => self.filter.range = *kind,

            EventUi::TxSelected { .. } => {
                // There is no detail screen yet. The event exists so the view
                // can already dispatch it, and so the log records the taps that
                // will one day open one.
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

                        // The server's clock, paired with the device reading
                        // it arrived at, so time keeps moving between fetches.
                        if snapshot.now > Timestamp::EPOCH {
                            self.server_clock = Some((snapshot.now, self.device_now));
                        }

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
                dock: screens::dock(self.screen),
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

    /// What the app treats as now.
    ///
    /// The server's clock advanced by however long the device says has passed
    /// since the snapshot landed — see [`StateApp::server_clock`]. Falls back to
    /// the device's own reading before the first snapshot.
    ///
    /// Public because `eval!` reads it: a test sets the time it wants before
    /// driving the machine, rather than the macro carrying a clock of its own.
    pub fn now(&self) -> Timestamp {
        match self.server_clock {
            Some((server, device_at)) => server + self.device_now.since(device_at),
            None => self.device_now,
        }
    }

    pub(crate) fn masked(&self) -> bool {
        self.masked
    }

    pub(crate) fn filter(&self) -> &Filter {
        &self.filter
    }

    pub(crate) fn load_failed(&self) -> Option<&str> {
        self.load_failed.as_deref()
    }

    pub(crate) fn loaded(&self) -> bool {
        self.loaded
    }

    /// The entries this filter lets through, newest first.
    pub(crate) fn filtered(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| self.filter.matches(e, self.now()))
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
                    .filter(|e| e.category == *category && range_only.matches(e, self.now()))
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

/// Read by tests. The view learns everything it needs from the actions it is
/// handed, so nothing in the app calls these.
impl StateApp {
    /// Move the clock, for tests that are *about* time.
    ///
    /// In a running app time only ever arrives through `handle_events`, from
    /// the runtime that read it.
    pub fn set_now(&mut self, now: Timestamp) {
        self.device_now = now;
    }

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
