//! What the screen showing says, read out of the view.
//!
//! A screen is drawn from the arm it was handed, but it reads the view through
//! a signal, and a render can move the flow on in the moment before the
//! screen it left is taken down. So every read answers for every arm: empty
//! where that arm has no such part.

use client_state::ViewCodeCell;
use client_state::ViewField;
use client_state::ViewIconRow;
use client_state::ViewOnboarding;
use client_state::ViewResend;
use client_state::ViewRun;
use client_state::ViewTally;
use leptos::prelude::*;

/// One part of the view, as a signal that fires only when that part changes.
///
/// The code screen renders once a second for its countdown, and everything on
/// it but the countdown reads the same each time.
pub(super) fn part<T>(
    view: Signal<ViewOnboarding>,
    read: impl Fn(&ViewOnboarding) -> T + Send + Sync + 'static,
) -> Memo<T>
where
    T: PartialEq + Send + Sync + 'static,
{
    Memo::new(move |_| view.with(|view| read(view)))
}

pub(super) fn step(view: &ViewOnboarding) -> &str {
    match view {
        ViewOnboarding::Email { step, .. }
        | ViewOnboarding::Code { step, .. }
        | ViewOnboarding::Bank { step, .. }
        | ViewOnboarding::Waiting { step, .. }
        | ViewOnboarding::Sync { step, .. }
        | ViewOnboarding::Failed { step, .. } => step,
        ViewOnboarding::Welcome { .. } | ViewOnboarding::Ready { .. } => "",
    }
}

pub(super) fn title(view: &ViewOnboarding) -> &str {
    match view {
        ViewOnboarding::Welcome { title, .. }
        | ViewOnboarding::Email { title, .. }
        | ViewOnboarding::Code { title, .. }
        | ViewOnboarding::Bank { title, .. }
        | ViewOnboarding::Waiting { title, .. }
        | ViewOnboarding::Sync { title, .. }
        | ViewOnboarding::Ready { title, .. }
        | ViewOnboarding::Failed { title, .. } => title,
    }
}

pub(super) fn body(view: &ViewOnboarding) -> &[ViewRun] {
    match view {
        ViewOnboarding::Welcome { body, .. }
        | ViewOnboarding::Email { body, .. }
        | ViewOnboarding::Code { body, .. }
        | ViewOnboarding::Bank { body, .. }
        | ViewOnboarding::Waiting { body, .. }
        | ViewOnboarding::Sync { body, .. }
        | ViewOnboarding::Ready { body, .. }
        | ViewOnboarding::Failed { body, .. } => body,
    }
}

/// What went wrong, wherever the screen says it: under the body on Welcome,
/// under the field on Email, under the boxes on Code.
pub(super) fn fault(view: &ViewOnboarding) -> Option<&str> {
    match view {
        ViewOnboarding::Welcome { fault, .. } | ViewOnboarding::Code { fault, .. } => {
            fault.as_deref()
        }
        ViewOnboarding::Email { field, .. } => field.fault.as_deref(),
        ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => None,
    }
}

/// The violet button's label, where the screen has one.
pub(super) fn action(view: &ViewOnboarding) -> Option<&str> {
    match view {
        ViewOnboarding::Welcome { action, .. }
        | ViewOnboarding::Email { action, .. }
        | ViewOnboarding::Bank { action, .. }
        | ViewOnboarding::Waiting { action, .. }
        | ViewOnboarding::Failed { action, .. } => Some(action),
        ViewOnboarding::Code { action, .. } => action.as_deref(),
        ViewOnboarding::Ready { open, .. } => Some(open),
        ViewOnboarding::Sync { .. } => None,
    }
}

/// The quiet way out beside the violet button: Add another bank on Ready,
/// Open Meridian on Failed once a bank has connected.
pub(super) fn aside(view: &ViewOnboarding) -> Option<&str> {
    match view {
        ViewOnboarding::Ready { another, .. } => Some(another),
        ViewOnboarding::Failed { open, .. } => open.as_deref(),
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. } => None,
    }
}

pub(super) fn legal(view: &ViewOnboarding) -> &str {
    match view {
        ViewOnboarding::Welcome { legal, .. } | ViewOnboarding::Bank { legal, .. } => legal,
        ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => "",
    }
}

pub(super) fn field(view: &ViewOnboarding) -> Option<&ViewField> {
    match view {
        ViewOnboarding::Email { field, .. } => Some(field),
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => None,
    }
}

pub(super) fn cells(view: &ViewOnboarding) -> &[ViewCodeCell] {
    match view {
        ViewOnboarding::Code { cells, .. } => cells,
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => &[],
    }
}

pub(super) fn resend(view: &ViewOnboarding) -> Option<&ViewResend> {
    match view {
        ViewOnboarding::Code { resend, .. } => Some(resend),
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => None,
    }
}

/// What the button that opens Plaid says while it waits on Plaid's page, and
/// so whether it is waiting.
pub(super) fn opening(view: &ViewOnboarding) -> Option<&str> {
    match view {
        ViewOnboarding::Bank { opening, .. } | ViewOnboarding::Failed { opening, .. } => {
            opening.as_deref()
        }
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Ready { .. } => None,
    }
}

pub(super) fn progress(view: &ViewOnboarding) -> f64 {
    match view {
        ViewOnboarding::Sync { progress, .. } => *progress,
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Ready { .. }
        | ViewOnboarding::Failed { .. } => 0.0,
    }
}

pub(super) fn rows(view: &ViewOnboarding) -> &[ViewIconRow] {
    match view {
        ViewOnboarding::Bank { rows, .. }
        | ViewOnboarding::Sync { rows, .. }
        | ViewOnboarding::Failed { rows, .. } => rows,
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Ready { .. } => &[],
    }
}

pub(super) fn tally(view: &ViewOnboarding) -> &[ViewTally] {
    match view {
        ViewOnboarding::Ready { tally, .. } => tally,
        ViewOnboarding::Welcome { .. }
        | ViewOnboarding::Email { .. }
        | ViewOnboarding::Code { .. }
        | ViewOnboarding::Bank { .. }
        | ViewOnboarding::Waiting { .. }
        | ViewOnboarding::Sync { .. }
        | ViewOnboarding::Failed { .. } => &[],
    }
}
