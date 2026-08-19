//! The decisions, and nothing else.
//!
//! This crate is the one the phone apps reach over FFI, so its public surface
//! is deliberately narrow: one object, one method that matters. Everything
//! above it renders; everything below it does IO.

mod action;
mod event;
mod fmt;
mod screens;
mod state;
mod view;

pub use action::*;
pub use event::*;
pub use state::Filter;
pub use state::State;
pub use view::*;

use std::sync::Mutex;

uniffi::setup_scaffolding!();

/// The state machine, as the platforms see it.
///
/// Swift and Kotlin get a handle to one of these and send it events. The
/// `Mutex` is what makes it `Send + Sync`, which UniFFI requires of an
/// exported object; the machine itself has no opinion about threads.
#[derive(uniffi::Object)]
pub struct Core {
    state: Mutex<State>,
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
        }
    }

    /// Hand in what happened, get back what to do about it.
    ///
    /// The owning counterpart to [`State::handle_events`]: a `&mut Vec` does
    /// not cross FFI, so the buffer is allocated here instead of reused. The
    /// borrowing version stays for replay and tests, where the allocation is
    /// worth avoiding.
    pub fn handle_events(&self, events: Vec<Event>) -> Vec<Action> {
        let mut actions = Vec::new();

        self.state
            .lock()
            .expect("core state mutex poisoned")
            .handle_events(&events, &mut actions);

        actions
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}
