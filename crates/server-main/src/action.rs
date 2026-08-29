//! What to do about it.

use crate::ActionClient;
use crate::ActionEmail;
use crate::ActionProcess;
use crate::ActionSource;
use crate::ActionStore;

#[derive(Debug)]
pub enum Action {
    ToClient { action: ActionClient },
    ToEmail { action: ActionEmail },
    ToSource { action: ActionSource },
    ToStore { action: ActionStore },
    ToProcess { action: ActionProcess },
}
