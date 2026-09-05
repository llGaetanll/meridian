//! The HTTP surface: what each route takes, and what it answers with.
//!
//! Every handler here has the same three steps and nothing else — read the
//! facts the machine does not keep, dispatch one event, answer from what came
//! back. Nothing decides anything, and nothing reaches past
//! [`RuntimeServer`]'s own methods, so a route cannot grow a rule that no
//! `eval!` test can reach.
//!
//! Split out of `runtime_server.rs` because carrying an action out and
//! answering a request are two jobs. The runtime performs actions; this turns
//! requests into events and events' answers into JSON.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::Extension;
use axum::extract::State;
use axum::http::Method;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use tower_http::cors::CorsLayer;

use crate::ActionRouter;
use crate::Client;
use crate::ClientIp;
use crate::EventRouter;
use crate::RuntimeServer;
use crate::WEBHOOK_PATH;
use crate::WebOrigins;
use crate::router_trace;
use crate::session_token;
use server_wire::WireSnapshot;
use server_wire::WireSyncStatus;

/// Every route, over one runtime.
pub(crate) fn router(runtime: Arc<RuntimeServer>, origins: &WebOrigins) -> Router {
    Router::new()
        .route("/snapshot", get(snapshot))
        .route("/auth/request", post(auth_request))
        .route("/auth/verify", post(auth_verify))
        .route("/link/start", post(link_start))
        .route("/link/finish", post(link_finish))
        .route("/sync/status", get(sync_status))
        .route(WEBHOOK_PATH, post(plaid_webhook))
        // Inside the CORS layer, so the preflight `OPTIONS` a browser sends
        // ahead of an authenticated request is answered above this and never
        // logged. It is transport, and a line for each would double the
        // traffic in the log without saying anything about it.
        .layer(axum::middleware::from_fn(router_trace::trace))
        .layer(cors(origins))
        .with_state(runtime)
}

/// What a browser may do here, and from where.
///
/// Only a browser is affected by any of this. The webhook route is under the
/// layer with everything else and is untouched by it: Plaid sends no `Origin`,
/// and a request without one is passed through whatever this says.
///
/// `allow_headers` is the line that makes the web app work at all, and it is
/// easy to leave out. Every route here takes either an `Authorization` header
/// or a JSON `Content-Type`, and neither is on the browsers' safelist — so
/// every one of them is preflighted, and a preflight that does not name the
/// header back is a request the browser never sends. That failure arrives as
/// nothing at all on this side, which is why it is worth writing down.
///
/// The headers are listed rather than `Any`, because the wildcard in
/// `Access-Control-Allow-Headers` deliberately does not cover `Authorization`
/// — the one header this server most needs to be sent.
///
/// Nothing allows credentials, and nothing should: the session is a bearer
/// token in a header, so there is no cookie for a browser to attach on its own
/// initiative and no request that carries authority the caller did not put in
/// it deliberately.
fn cors(origins: &WebOrigins) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(origins.values())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        // How long a browser may reuse one preflight. Without it every call
        // the web app makes is two round trips to the box rather than one.
        .max_age(Duration::from_secs(3600))
}

/// `GET /snapshot`
///
/// Answers with the ledger of whoever the bearer token signs in, and 401s
/// without one. There is no unauthenticated form: this is somebody's bank
/// transactions.
///
/// **It takes nothing.** A caller cannot ask for a trip to Plaid, and there
/// used to be a `?refresh=true` that let one: pull-to-refresh sent it, and a
/// gesture anybody can repeat became a way to spend this account's Plaid
/// budget. When to go back to a bank is the server's decision and only the
/// server's, and `should_fetch` is where it is made.
async fn snapshot(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WireSnapshot>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    // Read before deciding. These are the facts the machine needs and does not
    // keep — where each bank's last sync reached, and whether it still works.
    let items = runtime.facts_for(user_id).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "could not read this user's banks");
        Vec::new()
    });

    let answer = runtime
        .request(
            Client::new(ip, Some(user_id)),
            EventRouter::SnapshotRequested { items },
        )
        .await;

    // Read before the store is. A snapshot is the expensive answer this server
    // gives — a whole ledger out of Postgres and projected — so a refused
    // request must not pay for it on the way to a 429.
    too_many(&answer)?;

    // Matched rather than assumed, like every other route here. This one used
    // to serve whatever the machine said, so a machine that had answered
    // `Unavailable` still had a ledger read and returned over the top of it.
    let Some(ActionRouter::Serve) = answer else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    Ok(Json(runtime.project_snapshot(user_id).await))
}

/// What `POST /auth/request` takes.
#[derive(Debug, Deserialize)]
struct AuthRequestBody {
    email: String,
}

/// `POST /auth/request` — send a sign-in code to this address.
///
/// Always answers 204, whether or not the address is one we know. The
/// alternative tells anybody who asks which addresses have accounts here,
/// which for a banking app is worth more than the convenience of a clearer
/// error.
async fn auth_request(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthRequestBody>,
) -> StatusCode {
    let email = normalize(&body.email);

    // Read off what the machine decided rather than assumed. `Unavailable` is
    // the machine refusing to mint a code it has no entropy for, and answering
    // 204 to that would leave somebody waiting for an email that is not coming.
    let answer = runtime
        .request(Client::anonymous(ip), EventRouter::CodeRequested { email })
        .await;

    match answer {
        Some(ActionRouter::Accepted) => StatusCode::NO_CONTENT,

        // The one place the 204 rule bends, and it does not leak what the rule
        // protects: 429 is a fact about the caller, not about whether the
        // address they typed has an account here.
        Some(ActionRouter::TooMany) => StatusCode::TOO_MANY_REQUESTS,

        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// What `POST /auth/verify` takes.
#[derive(Debug, Deserialize)]
struct AuthVerifyBody {
    email: String,
    code: String,
}

/// What it answers with: the token every later request carries.
#[derive(Debug, serde::Serialize)]
struct AuthVerified {
    token: String,
}

/// `POST /auth/verify` — trade a code for a session.
///
/// One failure shape for every way of being wrong — wrong code, expired code,
/// too many guesses, no code at all — because the difference is only useful to
/// somebody working through guesses. The app already knows what to say from
/// the screen it is on.
async fn auth_verify(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthVerifyBody>,
) -> Result<Json<AuthVerified>, StatusCode> {
    // Nothing is read here. What the store holds against this address is the
    // machine's to ask for, and it arrives on its own event a turn later.
    let answer = runtime
        .request(
            Client::anonymous(ip),
            EventRouter::CodeSubmitted {
                email: normalize(&body.email),
                code: body.code.trim().to_string(),
            },
        )
        .await;

    too_many(&answer)?;

    match answer {
        Some(ActionRouter::SignedIn { token }) => Ok(Json(AuthVerified { token })),

        // A store that would not answer. Told apart from a code that was no
        // good, because they are not the same thing and only one of them is
        // the caller's to fix.
        Some(ActionRouter::Unavailable) => Err(StatusCode::INTERNAL_SERVER_ERROR),

        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// What `POST /link/start` answers with: the page to open.
#[derive(Debug, serde::Serialize)]
struct LinkStarted {
    url: String,
}

/// `POST /link/start` — open a Link session for the signed-in user.
///
/// Answers with Plaid's own hosted page. Nothing is created at Plaid's end by
/// this, so it is safe to call again if somebody backs out.
async fn link_start(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkStarted>, StatusCode> {
    // Hashed here and never carried further. The plaintext is a live
    // credential and an event is written to the log; the digest is what the
    // store holds anyway, so nothing downstream is the poorer for it.
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Anonymous, because who is asking is not known yet — the machine resolves
    // the session and buckets the request once it has. That is also when the
    // rate limit is asked: a caller's own window cannot be charged before
    // there is a caller.
    let answer = runtime
        .request(
            Client::anonymous(ip),
            EventRouter::LinkRequested { token_sha256 },
        )
        .await;

    too_many(&answer)?;

    match answer {
        Some(ActionRouter::LinkOpened { url }) => Ok(Json(LinkStarted { url })),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED),
        _ => Err(StatusCode::BAD_GATEWAY),
    }
}

/// What `POST /link/finish` answers with.
///
/// Three answers rather than a count, because with webhooks there are three
/// things that can be true. The redirect that closes the sheet says nothing —
/// Plaid is explicit that reaching it is not a successful connection — so
/// "Plaid has not said yet" is a real state and used to be reported as zero,
/// which is a different sentence and sometimes a false one.
///
/// Tagged rather than a count and a flag: `pending` with three banks attached
/// is not a thing that can happen, and a shape that can express it is a shape
/// somebody has to check.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum LinkFinished {
    /// Banks were attached, and this is how many.
    ///
    /// A count, because one call collects every session the caller has open
    /// and so can attach more than one. It does not name them: nothing knows
    /// what a bank is called until the balances call inside its first sync
    /// answers, and `/sync/status` carries the name once it does.
    Linked { count: u32 },

    /// The session ended without a bank. The ordinary outcome of somebody
    /// closing the sheet, not an error.
    Abandoned,

    /// Plaid has not said what the session came to. Neither of the above is
    /// known yet, and the bank may still be landing.
    Pending,
}

/// `POST /link/finish` — what did the Link session come to?
///
/// Called when the web sheet closes, and it does two different jobs depending
/// on how results are collected. With `PLAID_COLLECT=poll` it is a *trigger*:
/// nothing else ever collects, so this is what goes and asks Plaid. With
/// `PLAID_COLLECT=webhook` collection happens unprompted, and this degrades to
/// a *question* — one that can be asked a fraction of a second before the
/// answer exists, which is why the machine waits and why `Pending` is one of
/// the three things it can answer.
///
/// The name is left alone. It is still what finishes a link in the mode a
/// laptop runs, and renaming it would make it wrong there instead.
async fn link_finish(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkFinished>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    // The sessions this user has open, read before deciding — the same shape
    // as everywhere else: the machine keeps nothing, so the facts arrive with
    // the request. They are what a webhook's `link_token` is matched against,
    // and what a poll goes and asks about.
    let pending = runtime.pending_links(user_id).await.map_err(|e| {
        tracing::error!(error = %e, "could not read this user's link sessions");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let answer = runtime
        .request(
            Client::new(ip, Some(user_id)),
            EventRouter::LinkClosed { pending },
        )
        .await;

    too_many(&answer)?;

    Ok(Json(match answer {
        Some(ActionRouter::Linked { count }) => LinkFinished::Linked { count },
        Some(ActionRouter::LinkPending) => LinkFinished::Pending,
        Some(ActionRouter::LinkAbandoned) => LinkFinished::Abandoned,

        // The machine giving up on a poll that never answered. From here that
        // is the same sentence as a session that came to nothing, and the
        // phone has the same road out of both — but it is the machine's
        // answer being rendered, not a count being reinterpreted.
        _ => LinkFinished::Abandoned,
    }))
}

/// `POST /plaid/webhook` — Plaid telling us something happened.
///
/// **The one route here with no session behind it.** Anybody can POST to it,
/// so the first thing that happens is proving the POST is Plaid's — and the
/// proof is over the *raw bytes*, which is why this takes [`Bytes`] and not
/// `Json`. Plaid's hash is of what it put on the wire, pretty-printed with two
/// spaces, so a body an extractor had already parsed would hash to something
/// else and every genuine webhook would be refused.
///
/// One status for every way of being wrong, like `/auth/verify`: the reason is
/// logged and not returned, because the difference is only useful to whoever
/// is guessing.
///
/// A believed webhook is answered 2xx even when nothing is done about it.
/// Plaid retries anything else, so an error on a type we do not act on is that
/// webhook arriving again in a minute, and again after that.
async fn plaid_webhook(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    let webhook = match runtime.believe_webhook(&headers, &body).await {
        Ok(webhook) => webhook,

        Err(rejection) => {
            tracing::warn!(%rejection, "a webhook was not believed");

            return StatusCode::UNAUTHORIZED;
        }
    };

    // Read before deciding, the same as every other route here: a webhook
    // names a bank rather than a person, so what the store holds for it is
    // looked up and handed in on the event.
    let webhook = runtime.webhook_facts(webhook).await;

    match runtime
        .request(
            // Plaid, from addresses we do not choose. The one route with no
            // bucket — `EventRouter::route` says why — so there is nothing to
            // name it by here either.
            Client::anonymous(ip),
            EventRouter::WebhookReceived { webhook },
        )
        .await
    {
        // `Linked` is the same acceptance one turn further on: a session
        // whose token was exchanged answers with the count, and the webhook
        // was still taken.
        Some(ActionRouter::Accepted | ActionRouter::Linked { .. }) => StatusCode::NO_CONTENT,

        // The machine said nothing, or said it could not. Not answered 2xx on
        // purpose: Plaid sending it again is exactly what we want.
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// `GET /sync/status` — how far the caller's first sync has got.
///
/// Its own route rather than a field on the snapshot, because `/snapshot`
/// blocks until the fetch it triggered has landed: it could only ever report
/// a sync that was already over. This answers immediately, whatever is
/// happening, which is what makes it pollable while one runs.
///
/// A user with nothing syncing gets `finished` and zeroes rather than a 404 —
/// "no sync is running" is an answer, not a missing resource.
async fn sync_status(
    State(runtime): State<Arc<RuntimeServer>>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WireSyncStatus>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    // Which banks are theirs, read before deciding — the same shape as every
    // other route here. The machine holds progress by item and has no idea
    // whose item is whose.
    let items = runtime.facts_for(user_id).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "could not read this user's banks");
        Vec::new()
    });

    let answer = runtime
        .request(
            Client::new(ip, Some(user_id)),
            EventRouter::SyncStatusRequested { items },
        )
        .await;

    too_many(&answer)?;

    let Some(ActionRouter::SyncStatus { status }) = answer else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    Ok(Json(status))
}

/// The hash of the bearer token this request carries, if it carries one.
///
/// A request with no token at all is refused here rather than sent on: there
/// is no session to resolve and so nothing for the machine to decide. Whether
/// a token that *is* here names anybody is the machine's, and arrives as a
/// fact from the store.
fn bearer(headers: &axum::http::HeaderMap) -> Option<Vec<u8>> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    session_token::from_header(header).map(session_token::hash)
}

/// The address as the store and the machine both spell it.
///
/// Here rather than in the machine only because the store is read on this side
/// of the door: `/auth/verify` looks a code up by address *before* the event
/// exists, so a machine that normalized would be matching against a row found
/// under a different key. It moves in the moment that read becomes an action.
fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Stop here if the machine refused this request for being one too many.
///
/// A helper rather than an arm in each handler's match, because the answer is
/// the same everywhere and a route that forgot it would serve a request the
/// machine had already declined. It carries no `Retry-After`: see
/// [`ActionRouter::TooMany`](crate::ActionRouter::TooMany).
fn too_many(answer: &Option<ActionRouter>) -> Result<(), StatusCode> {
    match answer {
        Some(ActionRouter::TooMany) => Err(StatusCode::TOO_MANY_REQUESTS),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three words the phone switches on. They are a wire contract with
    /// `RuntimeIo.finishLink`, so a rename here is a rename there — and a
    /// silent one, because JSON has no type to disagree with.
    #[test]
    fn the_three_outcomes_are_named_on_the_wire() {
        let json = |answer: LinkFinished| serde_json::to_string(&answer).expect("a plain struct");

        assert_eq!(
            json(LinkFinished::Linked { count: 1 }),
            r#"{"outcome":"linked","count":1}"#,
        );
        assert_eq!(json(LinkFinished::Abandoned), r#"{"outcome":"abandoned"}"#);
        assert_eq!(json(LinkFinished::Pending), r#"{"outcome":"pending"}"#);
    }

    /// The preflight a browser sends ahead of an authenticated call, answered
    /// by the layer rather than by any route — which is what makes it testable
    /// with no runtime behind it.
    async fn preflight(origins: &str, from: &str) -> axum::http::HeaderMap {
        use tower::ServiceExt;

        let origins = WebOrigins::parse(origins).expect("a literal origin");

        let app = Router::new()
            .route("/snapshot", get(|| async { "" }))
            .layer(cors(&origins));

        let request = axum::http::Request::builder()
            .method(Method::OPTIONS)
            .uri("/snapshot")
            .header("origin", from)
            .header("access-control-request-method", "GET")
            .header("access-control-request-headers", "authorization")
            .body(axum::body::Body::empty())
            .expect("a literal request");

        app.oneshot(request)
            .await
            .expect("the layer answers every preflight")
            .headers()
            .clone()
    }

    /// The bug this replaced: `allow_headers` was never set, so the answer
    /// named no headers and every browser dropped the request it was asking
    /// about — with nothing arriving here to say so.
    #[tokio::test]
    async fn a_preflight_allows_the_headers_an_authenticated_call_carries() {
        let headers = preflight("https://app.meridiancash.io", "https://app.meridiancash.io").await;

        assert_eq!(
            headers
                .get("access-control-allow-origin")
                .and_then(|value| value.to_str().ok()),
            Some("https://app.meridiancash.io"),
        );

        let allowed = headers
            .get("access-control-allow-headers")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_lowercase();

        assert!(
            allowed.contains("authorization"),
            "every route here is behind a bearer token: {allowed}",
        );
        assert!(
            allowed.contains("content-type"),
            "a JSON body preflights too, and /auth/request has no token: {allowed}",
        );
    }

    /// The other half, and the reason the variable exists: an origin nobody
    /// named is answered without permission rather than with it.
    #[tokio::test]
    async fn an_origin_we_do_not_serve_is_not_allowed() {
        let headers = preflight("https://app.meridiancash.io", "https://evil.example").await;

        assert!(
            headers.get("access-control-allow-origin").is_none(),
            "only the origins we serve are named back",
        );
    }
}
