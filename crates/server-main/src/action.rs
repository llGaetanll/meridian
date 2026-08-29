//! What to do about it.

use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSource;
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
    ToSource {
        action: ActionSource,
    },
    ToStore {
        action: ActionStore,
    },
    ToProcess {
        action: ActionProcess,
    },
}
