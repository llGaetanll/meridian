//! The batch being handled, as a sub-machine sees it.

use api_common::Timestamp;

use crate::Action;
use crate::ActionRouter;
use crate::CallId;
use crate::Calls;
use crate::Owed;
use crate::Pending;
use crate::RequestId;

/// When it is, what is out, and what has been decided so far.
///
/// The three things every handler needs and none of them owns. Handed to a
/// sub-machine rather than held by one: the clock and the calls belong to
/// [`StateServer`](crate::StateServer), which is what keeps one counter and
/// one clock however many machines are deciding.
#[derive(Debug)]
pub struct Turn<'a> {
    now: Timestamp,
    calls: &'a mut Calls,
    actions: &'a mut Vec<Action>,
}

impl<'a> Turn<'a> {
    pub fn new(now: Timestamp, calls: &'a mut Calls, actions: &'a mut Vec<Action>) -> Self {
        Self {
            now,
            calls,
            actions,
        }
    }

    /// When this batch is being handled.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Something for the runtime to do.
    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    /// What a route is owed, sent the moment nothing it started is still out.
    pub fn answer(&mut self, request_id: RequestId, action: ActionRouter) {
        self.calls
            .answer(request_id, action, self.now, self.actions);
    }

    /// A fresh [`CallId`], noted as owed to the route that is waiting.
    pub fn calling(&mut self, request_id: RequestId) -> CallId {
        self.calls.calling(request_id, self.now)
    }

    /// A fresh [`CallId`] for a call nobody is waiting on.
    pub fn call(&mut self) -> CallId {
        self.calls.call()
    }

    /// Note what a request will do with the answer to the read it just asked
    /// for.
    pub fn waits_on(&mut self, call_id: CallId, pending: Pending) {
        self.calls.waits_on(call_id, pending);
    }

    /// What is being held for this route, for an answer that adds to it.
    pub fn owed(&self, request_id: RequestId) -> Option<&Owed> {
        self.calls.owed(request_id)
    }
}
