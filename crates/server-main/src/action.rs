//! What to do about it.

use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSocket;
use crate::ActionSourcePlaid;
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
