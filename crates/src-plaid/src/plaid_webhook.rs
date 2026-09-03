//! What Plaid POSTs at us, once it has been believed.
//!
//! Parsed shallowly, like every other response here: Plaid sends a great many
//! webhook types and will send more, and a strict parse would turn "a type we
//! do not act on" into "a body we could not read".

use serde::Deserialize;

/// One webhook, as Plaid sends it.
///
/// Every field but the two that name it is optional, because which fields are
/// present is decided by *which* webhook this is. Read
/// [`kind`](PlaidWebhook::kind) rather than the fields: it is the one place
/// that knows which combination means what.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaidWebhook {
    /// The family: `LINK`, `TRANSACTIONS`, `ITEM`, and a dozen more.
    #[serde(default)]
    pub webhook_type: String,

    /// What happened within that family.
    #[serde(default)]
    pub webhook_code: String,

    /// Which bank, on everything that is about one.
    #[serde(default)]
    pub item_id: Option<String>,

    /// Which Link session, on `LINK` webhooks. It is what says whose bank the
    /// session was about to become — see `link_sessions` in `src-postgres`.
    #[serde(default)]
    pub link_token: Option<String>,

    /// What a finished Link session produced. Empty is the ordinary outcome of
    /// somebody closing the sheet, and a list because one session can add more
    /// than one bank.
    #[serde(default)]
    pub public_tokens: Vec<String>,

    /// The error a `ITEM: ERROR` webhook is about. Its `error_code` is the
    /// only part worth matching on.
    #[serde(default)]
    pub error: Option<PlaidWebhookError>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaidWebhookError {
    #[serde(default)]
    pub error_code: Option<String>,
}

/// What a webhook is asking us to do about it.
///
/// The mapping from Plaid's two strings onto something with meaning, kept here
/// because it is knowledge about Plaid rather than about this app.
/// [`Other`](PlaidWebhookKind::Other) is not a failure: Plaid sends types
/// nobody planned for and retries anything that is not answered 2xx, so the
/// only safe thing to do with an unfamiliar one is take it and do nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaidWebhookKind {
    /// A Link session ended, and this is what it came to. The push half of
    /// [`link_session`](crate::link_session), which is the pull half.
    SessionFinished {
        link_token: String,
        public_tokens: Vec<String>,
    },

    /// This bank has transactions we have not read yet.
    SyncUpdatesAvailable { item_id: String },

    /// This bank has stopped answering until its owner signs in again. No
    /// amount of syncing fixes it, which is why it is worth writing down.
    LoginRequired { item_id: String },

    /// They signed in again, and it works. The counterpart of the above, and
    /// the only thing that clears it without a sync being tried.
    LoginRepaired { item_id: String },

    /// Something we do not act on. Named so a log line can say which.
    Other,
}

/// Plaid's `error_code` for a connection that needs its owner.
pub const ITEM_LOGIN_REQUIRED: &str = "ITEM_LOGIN_REQUIRED";

impl PlaidWebhook {
    /// What this webhook means, or [`Other`](PlaidWebhookKind::Other).
    ///
    /// A webhook whose type is one we act on but which is missing the id it
    /// would act *on* is `Other` as well. There is nothing to do with a
    /// `SYNC_UPDATES_AVAILABLE` that does not say which bank, and inventing
    /// one is how a sync happens against somebody else's.
    pub fn kind(&self) -> PlaidWebhookKind {
        match (self.webhook_type.as_str(), self.webhook_code.as_str()) {
            ("LINK", "SESSION_FINISHED") => match &self.link_token {
                Some(link_token) => PlaidWebhookKind::SessionFinished {
                    link_token: link_token.clone(),
                    public_tokens: self.public_tokens.clone(),
                },
                None => PlaidWebhookKind::Other,
            },

            ("TRANSACTIONS", "SYNC_UPDATES_AVAILABLE") => {
                self.about_an_item(|item_id| PlaidWebhookKind::SyncUpdatesAvailable { item_id })
            }

            // Two spellings for one thing. `ERROR` carrying
            // `ITEM_LOGIN_REQUIRED` is what Plaid documents; the bare code is
            // what several of its own examples show, and accepting both costs
            // one arm.
            ("ITEM", "ERROR") if self.error_code() == Some(ITEM_LOGIN_REQUIRED) => {
                self.about_an_item(|item_id| PlaidWebhookKind::LoginRequired { item_id })
            }

            ("ITEM", ITEM_LOGIN_REQUIRED) => {
                self.about_an_item(|item_id| PlaidWebhookKind::LoginRequired { item_id })
            }

            ("ITEM", "LOGIN_REPAIRED") => {
                self.about_an_item(|item_id| PlaidWebhookKind::LoginRepaired { item_id })
            }

            _ => PlaidWebhookKind::Other,
        }
    }

    fn about_an_item(&self, kind: impl FnOnce(String) -> PlaidWebhookKind) -> PlaidWebhookKind {
        match &self.item_id {
            Some(item_id) => kind(item_id.clone()),
            None => PlaidWebhookKind::Other,
        }
    }

    fn error_code(&self) -> Option<&str> {
        self.error.as_ref()?.error_code.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> PlaidWebhook {
        serde_json::from_str(body).expect("the fixture is valid JSON")
    }

    #[test]
    fn a_finished_session_carries_its_token() {
        let webhook = parse(
            r#"{
                "webhook_type": "LINK",
                "webhook_code": "SESSION_FINISHED",
                "status": "SUCCESS",
                "link_session_id": "session-1",
                "link_token": "link-sandbox-1",
                "public_tokens": ["public-sandbox-1"],
                "environment": "sandbox"
            }"#,
        );

        assert_eq!(
            webhook.kind(),
            PlaidWebhookKind::SessionFinished {
                link_token: "link-sandbox-1".to_string(),
                public_tokens: vec!["public-sandbox-1".to_string()],
            },
        );
    }

    /// Somebody closed the sheet. Finished, with nothing — an outcome.
    #[test]
    fn a_session_can_finish_with_no_tokens() {
        let webhook = parse(
            r#"{
                "webhook_type": "LINK",
                "webhook_code": "SESSION_FINISHED",
                "status": "EXITED",
                "link_token": "link-sandbox-1"
            }"#,
        );

        assert_eq!(
            webhook.kind(),
            PlaidWebhookKind::SessionFinished {
                link_token: "link-sandbox-1".to_string(),
                public_tokens: vec![],
            },
        );
    }

    #[test]
    fn a_sync_webhook_names_its_bank() {
        let webhook = parse(
            r#"{
                "webhook_type": "TRANSACTIONS",
                "webhook_code": "SYNC_UPDATES_AVAILABLE",
                "item_id": "item-1",
                "initial_update_complete": true,
                "historical_update_complete": false
            }"#,
        );

        assert_eq!(
            webhook.kind(),
            PlaidWebhookKind::SyncUpdatesAvailable {
                item_id: "item-1".to_string(),
            },
        );
    }

    #[test]
    fn both_spellings_of_a_dead_login_are_read() {
        let nested = parse(
            r#"{
                "webhook_type": "ITEM",
                "webhook_code": "ERROR",
                "item_id": "item-1",
                "error": { "error_type": "ITEM_ERROR", "error_code": "ITEM_LOGIN_REQUIRED" }
            }"#,
        );

        let bare = parse(
            r#"{ "webhook_type": "ITEM", "webhook_code": "ITEM_LOGIN_REQUIRED", "item_id": "item-1" }"#,
        );

        let expected = PlaidWebhookKind::LoginRequired {
            item_id: "item-1".to_string(),
        };

        assert_eq!(nested.kind(), expected);
        assert_eq!(bare.kind(), expected);
    }

    /// An `ITEM: ERROR` about something else is not a dead login, and treating
    /// it as one would stop a bank syncing over a rate limit.
    #[test]
    fn another_item_error_is_not_a_dead_login() {
        let webhook = parse(
            r#"{
                "webhook_type": "ITEM",
                "webhook_code": "ERROR",
                "item_id": "item-1",
                "error": { "error_code": "INTERNAL_SERVER_ERROR" }
            }"#,
        );

        assert_eq!(webhook.kind(), PlaidWebhookKind::Other);
    }

    /// Plaid sends types nobody planned for, and retries anything not
    /// answered 2xx. Reading one has to succeed.
    #[test]
    fn an_unknown_type_reads_as_something_to_ignore() {
        let webhook = parse(
            r#"{ "webhook_type": "ASSETS", "webhook_code": "PRODUCT_READY", "asset_report_id": "a-1" }"#,
        );

        assert_eq!(webhook.kind(), PlaidWebhookKind::Other);
    }

    /// Nothing to act on. A sync against a bank we picked ourselves would be
    /// a sync against somebody else's.
    #[test]
    fn a_webhook_with_no_item_is_ignored_rather_than_guessed_at() {
        let webhook = parse(
            r#"{ "webhook_type": "TRANSACTIONS", "webhook_code": "SYNC_UPDATES_AVAILABLE" }"#,
        );

        assert_eq!(webhook.kind(), PlaidWebhookKind::Other);
    }
}
