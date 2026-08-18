use api_ledger::Tx;

/// Global state
pub struct State {
    /// A list of transactions
    txs: Vec<Tx>,
}
