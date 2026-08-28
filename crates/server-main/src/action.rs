//! What to do about it.

use crate::ActionClient;
use crate::ActionProcess;
use crate::ActionSource;
use crate::ActionStore;

#[derive(Debug)]
pub enum Action {
    ToClient { action: ActionClient },
    ToSource { action: ActionSource },
    ToStore { action: ActionStore },
    ToProcess { action: ActionProcess },
}
