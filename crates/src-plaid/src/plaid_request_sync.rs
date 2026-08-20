use serde_json::Value;

use crate::PlaidRequest;

/// `/transactions/sync` — everything that has changed since a cursor.
///
/// The first call passes no cursor and gets the history from the start. Every
/// call after passes the cursor the last one returned, and gets only what has
/// happened since, which is why the cursor is saved beside the access token
/// rather than being derived from anything.
pub struct SyncRequest {
    pub access_token: String,

    /// Where the last sync got to. `None` on the first call only.
    pub cursor: Option<String>,

    /// How many to return per page, capped by Plaid at 500.
    pub count: u32,
}

impl SyncRequest {
    /// The first sync for an Item: no cursor, so the whole history.
    pub fn initial(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            cursor: None,
            count: 500,
        }
    }

    /// The next page, or the next sync — the same call either way.
    pub fn after(access_token: impl Into<String>, cursor: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            cursor: Some(cursor.into()),
            count: 500,
        }
    }
}

impl PlaidRequest for SyncRequest {
    fn path(&self) -> String {
        String::from("transactions/sync")
    }

    fn body(&self) -> Value {
        let mut body = serde_json::json!({
            "access_token": self.access_token,
            "count": self.count,
        });

        // Absent rather than null: Plaid reads a null cursor as "from the
        // beginning", which is the same thing, but only absent is documented.
        if let Some(cursor) = &self.cursor {
            body["cursor"] = Value::String(cursor.clone());
        }

        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sync_sends_no_cursor() {
        let body = SyncRequest::initial("access-sandbox-1").body();

        assert_eq!(body["access_token"], "access-sandbox-1");
        assert!(body.get("cursor").is_none());
    }

    #[test]
    fn later_syncs_send_the_cursor() {
        let body = SyncRequest::after("access-sandbox-1", "cursor-abc").body();

        assert_eq!(body["cursor"], "cursor-abc");
    }
}
