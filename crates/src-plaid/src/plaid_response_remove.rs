use serde::Deserialize;

/// What `/item/remove` answers with.
///
/// Plaid returns only its own request id — there is nothing to say about an
/// Item that no longer exists. Kept as a type rather than discarded so the
/// call has the same shape as every other one in this crate.
#[derive(Debug, Deserialize)]
pub struct RemoveResponse {
    pub request_id: String,
}
