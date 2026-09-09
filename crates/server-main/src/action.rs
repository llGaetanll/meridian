//! What to do about it.

use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
use crate::ActionSourceStripe;
use crate::ActionStore;
use crate::RequestId;

#[derive(Debug)]
pub enum Action {
    ToRouter {
        request_id: RequestId,
        action: ActionRouter,
    },
    ToEmail {
        action: ActionEmail,
    },
    ToSourcePlaid {
        action: ActionSourcePlaid,
    },
    ToSourceStripe {
        action: ActionSourceStripe,
    },
    ToSocket {
        action: ActionSocket,
    },
    ToStore {
        action: ActionStore,
    },
    ToProcess {
        action: ActionProcess,
    },
}
