//! The client apps' decisions, and nothing else.
//!
//! What happens when a tab is tapped, what each screen shows, and what IO to
//! ask for. The phone apps reach it over FFI: one object, one method. The web
//! app links it directly.

mod action;
mod action_source;
mod action_ui;
mod event;
mod event_runtime;
mod event_source;
mod event_ui;
mod fmt;
mod load_failure;
mod range;
mod screens;
mod screens_onboarding;
mod screens_paywall;
mod state_app;
mod state_delete;
mod state_ledger;
mod state_onboarding;
mod state_payment;
mod state_remove;
mod unpaid;
mod view;
mod view_account;
mod view_calendar;
mod view_connections;
mod view_dock;
mod view_filter;
mod view_ledger;
mod view_onboarding;
mod view_paywall;
mod view_paywall_step;
mod view_recurring;
mod view_settings;
mod view_tx_row;
mod view_who;

pub use action::Action;
pub use action_source::ActionSource;
pub use action_ui::ActionUi;
pub use event::Event;
pub use event_runtime::EventRuntime;
pub use event_source::EventSource;
pub use event_ui::EventUi;
pub use event_ui::FilterGroup;
pub use load_failure::LoadFailure;
pub use range::RangeSelection;
pub use range::RangeUnit;
pub use state_app::StateApp;
pub use state_app::User;
pub use state_delete::StateDelete;
pub use state_ledger::StateLedger;
pub use state_onboarding::AWAIT_GOOD_FOR_SECS;
pub use state_onboarding::StateOnboarding;
pub use state_remove::StateRemove;
pub use unpaid::Unpaid;
pub use view::Screen;
pub use view::ViewScreen;
pub use view_account::ViewAccount;
pub use view_account::ViewAccountBank;
pub use view_account::ViewAccountDelete;
pub use view_account::ViewAccountName;
pub use view_calendar::ViewCalendar;
pub use view_calendar::ViewCalendarDay;
pub use view_calendar::ViewCalendarMonth;
pub use view_connections::ViewAccountRow;
pub use view_connections::ViewConnection;
pub use view_connections::ViewConnectionRemove;
pub use view_connections::ViewConnections;
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
pub use view_paywall::ViewPaywall;
pub use view_paywall_step::ViewPaywallStep;
pub use view_recurring::ViewBreakdownSlice;
pub use view_recurring::ViewDayCard;
pub use view_recurring::ViewDayRow;
pub use view_recurring::ViewRecurring;
pub use view_recurring::ViewStreamGroup;
pub use view_recurring::ViewStreamRow;
pub use view_settings::ViewSettings;
pub use view_tx_row::ViewTxRow;
pub use view_who::ViewWho;

use std::sync::Mutex;

uniffi::setup_scaffolding!();

/// The state machine, as the platforms see it.
///
/// The `Mutex` is what makes it `Send + Sync`, which UniFFI requires of an
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
    /// The owning counterpart to [`StateApp::handle_events`]: a `&mut Vec` does
    /// not cross FFI, so the buffer is allocated here instead of reused.
    pub fn handle_events(&self, events: Vec<Event>) -> Vec<Action> {
        let mut actions = Vec::new();

        // Carrying on with the held state beats taking the app down, and a
        // panic across FFI is undefined behaviour rather than an exception, so
        // propagating is not an option either.
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
