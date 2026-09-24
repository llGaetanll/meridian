//! What happened to the server.

use crate::CallId;
use crate::Client;
use crate::EventProcess;
use crate::EventRouter;
use crate::EventSocket;
use crate::EventSourcePlaid;
use crate::EventSourceStripe;
use crate::EventSourceYahoo;
use crate::EventStore;
use crate::EventTerminal;
use crate::RequestId;

/// Grouped by origin, the same way the app's events are.
#[derive(Debug)]
pub enum Event {
    /// A route was asked for something.
    ///
    /// The id and the client ride here rather than on each
    /// [`EventRouter`](crate::EventRouter) variant, because both are true of
    /// *every* request.
    FromRouter {
        request_id: RequestId,
        client: Client,
        event: EventRouter,
    },

    /// Plaid answered, or failed to.
    ///
    /// The call rides here rather than on each
    /// [`EventSourcePlaid`](crate::EventSourcePlaid) variant, because every
    /// answer is the answer to one.
    FromSourcePlaid {
        call_id: CallId,
        event: EventSourcePlaid,
    },

    /// Stripe answered, or failed to.
    ///
    /// The call rides here for the reason it does on
    /// [`FromSourcePlaid`](Event::FromSourcePlaid).
    FromSourceStripe {
        call_id: CallId,
        event: EventSourceStripe,
    },

    /// Yahoo answered, or failed to.
    ///
    /// The call rides here for the reason it does on
    /// [`FromSourcePlaid`](Event::FromSourcePlaid).
    FromSourceYahoo {
        call_id: CallId,
        event: EventSourceYahoo,
    },

    /// A stream the server writes to changed, without being asked anything.
    FromSocket { event: EventSocket },

    /// The store answered, or failed to.
    FromStore { call_id: CallId, event: EventStore },

    /// The process did something to itself.
    FromProcess { event: EventProcess },

    /// The process was signalled.
    FromTerminal { event: EventTerminal },
}
