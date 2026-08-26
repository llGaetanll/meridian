//! What `/link/token/get` answers with.
//!
//! Deliberately shallow. The session record carries the accounts chosen, the
//! institution, and a good deal else that we would only be re-reading from
//! `/accounts/balance/get` a moment later — so this parses the one field that
//! exists nowhere else, and lets serde discard the rest.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkSessionResponse {
    /// One per attempt. A person who backs out of one bank and picks another
    /// produces two, which is why this is a list and why
    /// [`public_tokens`](LinkSessionResponse::public_tokens) returns every one
    /// rather than assuming the first.
    #[serde(default)]
    pub link_sessions: Vec<LinkSession>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkSession {
    #[serde(default)]
    pub link_session_id: Option<String>,

    /// Absent while the session is still open. That is the flag a poller
    /// watches: an unfinished session is not an error, it is someone still
    /// typing their password.
    #[serde(default)]
    pub finished_at: Option<String>,

    #[serde(default)]
    pub results: Option<LinkResults>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkResults {
    /// One per Item created. Empty on a session that ended without adding one,
    /// which is the ordinary outcome of someone closing the sheet.
    #[serde(default)]
    pub item_add_results: Vec<ItemAddResult>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemAddResult {
    /// The thirty-minute token, to be exchanged for a permanent one.
    pub public_token: String,

    /// Which bank, as Plaid identifies it. Not the name — that arrives with
    /// the balances, which is where `project::connection` already reads it
    /// from.
    #[serde(default)]
    pub institution_id: Option<String>,
}

impl LinkSessionResponse {
    /// Every public token this Link session produced.
    ///
    /// Empty is the normal answer while somebody is still signing in, so a
    /// caller polls on empty rather than treating it as a failure. It is also
    /// what a session that ended with nothing looks like — see
    /// [`finished`](LinkSessionResponse::finished) for telling those apart.
    pub fn public_tokens(&self) -> Vec<&str> {
        self.link_sessions
            .iter()
            .filter_map(|session| session.results.as_ref())
            .flat_map(|results| &results.item_add_results)
            .map(|result| result.public_token.as_str())
            .collect()
    }

    /// Whether every session on this token has ended, one way or another.
    ///
    /// A poller stops on this rather than on a token appearing: a session that
    /// finished with no Item is an answer, and waiting for a token that is
    /// never coming is how a spinner runs until it times out.
    pub fn finished(&self) -> bool {
        !self.link_sessions.is_empty()
            && self
                .link_sessions
                .iter()
                .all(|session| session.finished_at.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape Plaid sends, trimmed to what we read. Everything else in the
    /// real body is discarded by serde, which is the point of parsing
    /// shallowly.
    const FINISHED: &str = r#"{
        "link_token": "link-sandbox-1",
        "link_sessions": [
            {
                "link_session_id": "session-1",
                "finished_at": "2026-08-26T10:00:00Z",
                "results": {
                    "item_add_results": [
                        {
                            "public_token": "public-sandbox-1",
                            "institution_id": "ins_109508",
                            "accounts": [{ "id": "acc-1" }]
                        }
                    ]
                }
            }
        ]
    }"#;

    #[test]
    fn a_finished_session_yields_its_token() {
        let res: LinkSessionResponse = serde_json::from_str(FINISHED).unwrap();

        assert!(res.finished());
        assert_eq!(res.public_tokens(), vec!["public-sandbox-1"]);
    }

    /// Someone is still typing. Not an error, and not something to give up on.
    #[test]
    fn an_open_session_is_neither_finished_nor_empty_handed() {
        let res: LinkSessionResponse =
            serde_json::from_str(r#"{ "link_sessions": [{ "link_session_id": "session-1" }] }"#)
                .unwrap();

        assert!(!res.finished());
        assert!(res.public_tokens().is_empty());
    }

    /// Someone closed the sheet. Finished, and there is nothing coming — the
    /// case that separates "keep polling" from "say it did not finish".
    #[test]
    fn a_session_can_end_with_nothing() {
        let res: LinkSessionResponse = serde_json::from_str(
            r#"{ "link_sessions": [{ "finished_at": "2026-08-26T10:00:00Z", "results": { "item_add_results": [] } }] }"#,
        )
        .unwrap();

        assert!(res.finished());
        assert!(res.public_tokens().is_empty());
    }

    /// Nothing has happened yet at all: the token was made and the page has
    /// not been opened.
    #[test]
    fn no_sessions_yet_is_not_finished() {
        let res: LinkSessionResponse = serde_json::from_str(r#"{ "link_sessions": [] }"#).unwrap();

        assert!(!res.finished());
    }
}
