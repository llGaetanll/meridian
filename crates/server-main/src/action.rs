//! What to do about it.

use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionRouter;
use crate::ActionSource;
use crate::ActionStore;

#[derive(Debug)]
pub enum Action {
    ToRouter { action: ActionRouter },
    ToEmail { action: ActionEmail },
    ToSource { action: ActionSource },
    ToStore { action: ActionStore },
    ToProcess { action: ActionProcess },
}
