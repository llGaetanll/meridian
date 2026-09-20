//! Meridian's own server. Every call the app makes of it.
//!
//! Nothing here is reactive and nothing here decides: async functions over
//! plain data, like the price source beside it. Bodies the machine reads —
//! the snapshot, a sync status — go back as strings rather than parsed, so
//! every platform gets the machine's one parse and there is one place that
//! decides what a missing field means.
//!
//! Nothing here holds the token either. It is a parameter; the runtime holds
//! one, and turns an outcome into an event.

use std::borrow::Cow;

use api_trace::ApiCall;
use api_trace::Source;
use api_trace::scrub::Context;
use api_trace::scrub::Direction;
use futures::StreamExt;
use serde::Deserialize;

use crate::io::Failure;
use crate::io::LinkOutcome;
use crate::io::Sse;

/// What this client is called in the logs, and what its bodies pass through
/// on the way there.
const SOURCE: Source = Source::new("meridian").through(&[redact_session]);

/// Where the server is when the build does not name one. `MERIDIAN_SERVER` at
/// build time points it elsewhere, as the same variable does on the phone.
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8082/";

/// As much of a refused stream's body as is worth reading. `WireUnpaid` is
/// two fields, and anything past this is not that.
const REFUSAL_CAP: usize = 4096;

#[derive(Clone)]
pub struct Meridian {
    client: reqwest::Client,
    base_url: String,
}

impl Meridian {
    pub fn new() -> Self {
        let base_url = option_env!("MERIDIAN_SERVER").unwrap_or(DEFAULT_BASE_URL);

        // A base without the trailing slash swallows its last segment when a
        // path is joined onto it, which is a 404 nobody can see the cause of.
        let base_url = match base_url.ends_with('/') {
            true => base_url.to_string(),
            false => format!("{base_url}/"),
        };

        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    /// The snapshot, as JSON.
    ///
    /// It asks for nothing but the ledger. When to go back to a bank is the
    /// server's decision, and a gesture anybody can repeat is not.
    pub async fn snapshot(&self, token: &str) -> Result<String, Failure> {
        let url = self.url("snapshot")?;
        let call = ApiCall::get(module_path!(), SOURCE, &url);

        let (status, body) = self
            .send(self.client.get(url).bearer_auth(token), call)
            .await?;

        match Failure::refusal(status, &body) {
            Some(refusal) => Err(refusal),
            None => Ok(body),
        }
    }

    /// Ask for a sign-in code.
    ///
    /// The server answers 204 whether or not the address is one it knows, so
    /// there is nothing to read out of a success.
    pub async fn request_code(&self, email: &str) -> Result<(), Failure> {
        let body = serde_json::json!({ "email": email });

        let (status, body) = self.post_json("auth/request", &body).await?;

        match Failure::refusal(status, &body) {
            Some(refusal) => Err(refusal),
            None => Ok(()),
        }
    }

    /// Trade a code for a session, and answer with the token.
    ///
    /// `None` is the code being refused — wrong, expired and too many guesses
    /// answer alike, and this passes that through. It is not a failure: a
    /// mistyped digit is the ordinary way this ends.
    pub async fn verify_code(&self, email: &str, code: &str) -> Result<Option<String>, Failure> {
        let body = serde_json::json!({ "email": email, "code": code });

        let (status, body) = self.post_json("auth/verify", &body).await?;

        if status == 401 {
            return Ok(None);
        }

        if let Some(refusal) = Failure::refusal(status, &body) {
            return Err(refusal);
        }

        #[derive(Deserialize)]
        struct Verified {
            token: String,
        }

        let verified: Verified = serde_json::from_str(&body).map_err(|_| Failure::Unreadable)?;

        Ok(Some(verified.token))
    }

    /// Open a Link session, and answer with the address of Plaid's page for
    /// it.
    pub async fn start_link(&self, token: &str) -> Result<String, Failure> {
        #[derive(Deserialize)]
        struct Started {
            url: String,
        }

        let started: Started = self.post_authed("link/start", token).await?;

        Ok(started.url)
    }

    /// Open an update-mode Link session against one bank, and answer with the
    /// address of Plaid's page for it. The sign-in there repairs the bank
    /// rather than attaching a new one.
    pub async fn repair_link(&self, token: &str, item_id: &str) -> Result<String, Failure> {
        #[derive(Deserialize)]
        struct Started {
            url: String,
        }

        let body = serde_json::json!({ "item_id": item_id });

        let started: Started = self.post_authed_body("link/repair", token, &body).await?;

        Ok(started.url)
    }

    /// Ask what the Link session came to.
    pub async fn finish_link(&self, token: &str) -> Result<LinkOutcome, Failure> {
        #[derive(Deserialize)]
        struct Finished {
            outcome: String,
        }

        let finished: Finished = self.post_authed("link/finish", token).await?;

        Ok(match finished.outcome.as_str() {
            "linked" => LinkOutcome::Linked,
            "pending" => LinkOutcome::Pending,

            // Including a word this build has never heard of. A server saying
            // something newer than the app is not a reason to leave somebody
            // waiting, and every other outcome ends the same way.
            _ => LinkOutcome::Abandoned,
        })
    }

    /// Open a Checkout page at Stripe, and answer with where it is.
    ///
    /// Nobody is entitled by this: the money is taken on the page, and
    /// nothing has been charged when it answers.
    pub async fn checkout(&self, token: &str) -> Result<String, Failure> {
        #[derive(Deserialize)]
        struct Opened {
            url: String,
        }

        let opened: Opened = self.post_authed("billing/checkout", token).await?;

        Ok(opened.url)
    }

    /// Ask for the code that confirms deleting the account. The server sends
    /// it to the address the account is written to, so nothing but the
    /// session goes with the request.
    pub async fn request_delete_code(&self, token: &str) -> Result<(), Failure> {
        let url = self.url("account/delete/request")?;
        let call = ApiCall::post(module_path!(), SOURCE, &url, "");

        let (status, body) = self
            .send(self.client.post(url).bearer_auth(token), call)
            .await?;

        match Failure::refusal(status, &body) {
            Some(refusal) => Err(refusal),
            None => Ok(()),
        }
    }

    /// Trade the emailed code for the deletion. There is no undo: the server
    /// answers once the rows are gone.
    ///
    /// `false` is the code being refused — wrong, expired and too many
    /// guesses answer alike. It is not a failure: a mistyped digit is the
    /// ordinary way this ends.
    pub async fn delete_account(&self, token: &str, code: &str) -> Result<bool, Failure> {
        let url = self.url("account/delete")?;
        let body = serde_json::json!({ "code": code }).to_string();
        let call = ApiCall::post(module_path!(), SOURCE, &url, &body);

        let request = self
            .client
            .post(url)
            .bearer_auth(token)
            .header("Content-Type", "application/json")
            .body(body);

        let (status, body) = self.send(request, call).await?;

        if status == 403 {
            return Ok(false);
        }

        match Failure::refusal(status, &body) {
            Some(refusal) => Err(refusal),
            None => Ok(true),
        }
    }

    /// How the sync is going, handed to `status` as JSON each time it moves,
    /// for as long as the connection lasts.
    ///
    /// A response that never ends: the server holds it open and writes a
    /// message whenever the sync moves. `Ok` is the connection going, which is
    /// a fact and not a failure. `Err` is the server refusing to open it.
    ///
    /// Dropping the future closes the connection, which is what frees the slot
    /// the server is holding for it.
    pub async fn sync_statuses(
        &self,
        token: &str,
        mut status: impl FnMut(String),
    ) -> Result<(), Failure> {
        let url = self.url("sync/status")?;
        let call = ApiCall::get(module_path!(), SOURCE, &url);

        // No timeout of its own. What keeps the connection up through minutes
        // of quiet is the server's keep-alive every fifteen seconds.
        let sent = self
            .client
            .get(url)
            .bearer_auth(token)
            .header("Accept", "text/event-stream")
            .send()
            .await;

        let response = match sent {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);

                return Err(Failure::Unreachable);
            }
        };

        let code = response.status().as_u16();
        let mut body = response.bytes_stream();

        // Refused before any of the stream went out, which is why the server
        // answers the open as an ordinary request. A 402 met here is somebody
        // who reached the bank step without paying.
        if !(200..300).contains(&code) {
            let mut refused = Vec::new();

            while let Some(Ok(chunk)) = body.next().await {
                refused.extend_from_slice(&chunk);

                if refused.len() >= REFUSAL_CAP {
                    break;
                }
            }

            let refused = String::from_utf8_lossy(&refused);

            let _ = call.response(code, &refused);

            return Err(Failure::refusal(code, &refused).unwrap_or(Failure::Unreachable));
        }

        let _ = call.response(code, "[event stream]");

        let mut sse = Sse::default();
        let mut statuses = Vec::new();

        while let Some(chunk) = body.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => {
                    // The connection went after it had opened. The ending is
                    // the whole of what happened, and the caller reports it.
                    tracing::info!(error = %e, "the sync stream ended");

                    return Ok(());
                }
            };

            sse.feed(&chunk, &mut statuses);

            for json in statuses.drain(..) {
                status(json);
            }
        }

        Ok(())
    }

    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, String), Failure> {
        let url = self.url(path)?;
        let body = body.to_string();

        let call = ApiCall::post(module_path!(), SOURCE, &url, &body);

        let request = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body);

        self.send(request, call).await
    }

    /// A POST that carries nothing but the session, and whose answer is JSON.
    async fn post_authed<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        token: &str,
    ) -> Result<T, Failure> {
        let url = self.url(path)?;
        let call = ApiCall::post(module_path!(), SOURCE, &url, "");

        let (status, body) = self
            .send(self.client.post(url).bearer_auth(token), call)
            .await?;

        if let Some(refusal) = Failure::refusal(status, &body) {
            return Err(refusal);
        }

        serde_json::from_str(&body).map_err(|_| Failure::Unreadable)
    }

    /// A POST that carries the session and a JSON body, and whose answer is
    /// JSON.
    async fn post_authed_body<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<T, Failure> {
        let url = self.url(path)?;
        let body = body.to_string();
        let call = ApiCall::post(module_path!(), SOURCE, &url, &body);

        let request = self
            .client
            .post(url)
            .bearer_auth(token)
            .header("Content-Type", "application/json")
            .body(body);

        let (status, body) = self.send(request, call).await?;

        if let Some(refusal) = Failure::refusal(status, &body) {
            return Err(refusal);
        }

        serde_json::from_str(&body).map_err(|_| Failure::Unreadable)
    }

    /// Send it, and hand back the status and the body.
    ///
    /// The status is returned rather than failed on, because what a given
    /// status means differs by route — a 401 from `/auth/verify` is a mistyped
    /// digit, and a 401 from `/snapshot` is a session that has run out.
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        call: ApiCall,
    ) -> Result<(u16, String), Failure> {
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                call.failed(&e);

                return Err(Failure::Unreachable);
            }
        };

        let status = response.status().as_u16();

        let body = match response.text().await {
            Ok(body) => body,
            Err(e) => {
                call.failed(&e);

                return Err(Failure::Unreachable);
            }
        };

        let _ = call.response(status, &body);

        Ok((status, body))
    }

    /// A path on the server. A base that will not parse is a server nobody
    /// can reach.
    fn url(&self, path: &str) -> Result<reqwest::Url, Failure> {
        reqwest::Url::parse(&self.base_url)
            .and_then(|base| base.join(path))
            .map_err(|_| Failure::Unreachable)
    }
}

/// Two answers here are a credential: the session `/auth/verify` hands back,
/// good for ninety days, and the page `/link/start` hands back, whose address
/// is a Link token. The code that buys the first goes the same way, and so
/// does the one `/account/delete` carries — six digits that destroy an
/// account. Each is logged as its length. Checkout's page is not one: its
/// address is made to be opened in somebody's browser.
fn redact_session<'a>(ctx: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
    let secret = match ctx.direction {
        Direction::Request => {
            ctx.url.ends_with("/auth/verify") || ctx.url.ends_with("/account/delete")
        }
        Direction::Response => {
            ctx.url.ends_with("/auth/verify") || ctx.url.ends_with("/link/start")
        }
    };

    match secret {
        true => Cow::Owned(format!("[redacted {}]", body.len())),
        false => body,
    }
}
