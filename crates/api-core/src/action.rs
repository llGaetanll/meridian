//! What to do about it. Actions are the only IO the core asks for; a thin
//! runtime on each platform carries them out and feeds the result back in as
//! the next event.

use crate::event::TxSummary;

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Action {
    ToUi { action: UiAction },
    ToSource { action: SourceAction },
}

/// Something for the view to do. Note that these carry finished values, not
/// instructions to go and compute one.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum UiAction {
    /// Draw this list. The whole list, in the order it is drawn in.
    ShowTxs { txs: Vec<TxSummary> },
    /// Tell the user something went wrong, in these words.
    ShowError { message: String },
}

/// Something for a source to do.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum SourceAction {
    /// Go and get the transactions.
    LoadTxs,
}
