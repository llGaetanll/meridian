//! A webhook signing key, or the fact that there was not one.

use api_common::Timestamp;
use src_plaid::PlaidJwk;

/// What is known about one `kid` Plaid named.
#[derive(Debug)]
pub struct CachedKey {
    /// `None` is a miss: Plaid did not answer for this id.
    pub key: Option<PlaidJwk>,

    /// When it was fetched. Only a miss expires; a key for a given `kid` is
    /// the same key forever, because rotation gives the new one a new id.
    pub at: Timestamp,
}
