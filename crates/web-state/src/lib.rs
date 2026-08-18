use api_ledger::Tx;

/// Global state
pub struct State {
    /// A list of transactions
    #[allow(dead_code)]
    txs: Vec<Tx>,
}
