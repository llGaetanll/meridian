//! The phone app's decisions, and nothing else.
//!
//! This is a *program*'s state machine rather than a bag of shared types —
//! `api-common` and `api-ledger` are where the types live. What is here is the
//! app: what happens when a tab is tapped, what each screen shows, and what
//! IO to ask for. `server-main` owns the equivalent for the server.
//!
//! The phone apps reach this over FFI, so its public surface is deliberately
//! narrow: one object, one method that matters. Everything above it renders;
//! everything below it does IO.

mod action;
mod action_source;
mod action_ui;
mod event;
mod event_runtime;
mod event_source;
mod event_ui;
mod fmt;
mod range;
mod screens;
mod screens_onboarding;
mod state_app;
mod state_ledger;
mod state_onboarding;
mod view;
mod view_balances;
mod view_calendar;
mod view_dock;
mod view_filter;
mod view_ledger;
mod view_onboarding;
mod view_recurring;
mod view_tx_row;

pub use action::Action;
pub use action_source::ActionSource;
pub use action_ui::ActionUi;
pub use event::Event;
pub use event_runtime::EventRuntime;
pub use event_source::EventSource;
pub use event_ui::EventUi;
pub use event_ui::FilterGroup;
pub use range::RangeSelection;
pub use range::RangeUnit;
pub use state_app::StateApp;
pub use state_ledger::StateLedger;
pub use state_onboarding::StateOnboarding;
pub use view::Screen;
pub use view::ViewScreen;
pub use view_balances::ViewAccountRow;
pub use view_balances::ViewBalances;
pub use view_balances::ViewConnection;
pub use view_calendar::ViewCalendar;
pub use view_calendar::ViewCalendarDay;
pub use view_calendar::ViewCalendarMonth;
pub use view_dock::ViewDock;
pub use view_dock::ViewDockItem;
pub use view_filter::ViewCategoryChip;
pub use view_filter::ViewFilter;
pub use view_filter::ViewRange;
pub use view_filter::ViewRangeKind;
pub use view_ledger::ViewDaySection;
pub use view_ledger::ViewLedger;
pub use view_ledger::ViewMonthSection;
pub use view_onboarding::ViewCodeCell;
pub use view_onboarding::ViewField;
pub use view_onboarding::ViewIconRow;
pub use view_onboarding::ViewOnboarding;
pub use view_onboarding::ViewResend;
pub use view_onboarding::ViewRowTone;
pub use view_onboarding::ViewRun;
pub use view_onboarding::ViewTally;
pub use view_recurring::ViewBreakdownSlice;
pub use view_recurring::ViewDayCard;
pub use view_recurring::ViewDayRow;
pub use view_recurring::ViewRecurring;
pub use view_recurring::ViewStreamGroup;
pub use view_recurring::ViewStreamRow;
pub use view_tx_row::ViewTxRow;

use std::sync::Mutex;

uniffi::setup_scaffolding!();

/// The state machine, as the platforms see it.
///
/// Swift and Kotlin get a handle to one of these and send it events. The
/// `Mutex` is what makes it `Send + Sync`, which UniFFI requires of an
/// exported object; the machine itself has no opinion about threads.
#[derive(uniffi::Object)]
pub struct Core {
    state: Mutex<StateApp>,
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(StateApp::new()),
        }
    }

    /// Hand in what happened, get back what to do about it.
    ///
    /// No clock: the machine has none of its own and is not given one here
    /// either. The runtime reads the platform's clock and sends it in as an
    /// [`EventRuntime::Tick`] like anything else it observed.
    ///
    /// The owning counterpart to [`StateApp::handle_events`]: a `&mut Vec` does
    /// not cross FFI, so the buffer is allocated here instead of reused. The
    /// borrowing version stays for replay and tests, where the allocation is
    /// worth avoiding.
    pub fn handle_events(&self, events: Vec<Event>) -> Vec<Action> {
        let mut actions = Vec::new();

        // A poisoned mutex means another thread panicked while holding it.
        // Carrying on beats taking the app down — and a panic across FFI is
        // undefined behaviour rather than an exception, so propagating is not
        // an option either. It is logged loudly so the panic that caused it
        // does not become a mystery here.
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::error!(
                    "core state lock is poisoned — a thread panicked while holding it; \
                     carrying on with the held state",
                );

                poisoned.into_inner()
            }
        };

        state.handle_events(&events, &mut actions);

        actions
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}
