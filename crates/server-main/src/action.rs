//! What to do about it.

use crate::ActionClient;
use crate::ActionProcess;
use crate::ActionSource;

#[derive(Debug)]
pub enum Action {
    ToClient { action: ActionClient },
    ToSource { action: ActionSource },
    ToProcess { action: ActionProcess },
}
