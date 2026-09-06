//! The HTTP surface: what each route takes, and what it answers with.
//!
//! Every handler has the same shape and nothing else — dispatch one event,
//! answer from what came back. **The state is a channel**: a handler cannot
//! reach the store, the machine or the runtime, so a route cannot grow a rule
//! that no `eval!` test can reach.

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
use crate::WEBHOOK_PATH;
use crate::WebOrigins;
use crate::router_trace;
use crate::runtime_router;
use crate::runtime_router::RequestTx;
use crate::session_token;
use server_wire::WireSnapshot;
use server_wire::WireSyncStatus;

/// Every route, over one channel.
pub(crate) fn router(requests: RequestTx, origins: &WebOrigins) -> Router {
    Router::new()
        .route("/snapshot", get(snapshot))
        .route("/auth/request", post(auth_request))
        .route("/auth/verify", post(auth_verify))
        .route("/link/start", post(link_start))
        .route("/link/finish", post(link_finish))
        .route("/sync/status", get(sync_status))
        .route(WEBHOOK_PATH, post(plaid_webhook))
        // Inside the CORS layer, so a browser's preflight `OPTIONS` is answered
        // above this and never logged.
        .layer(axum::middleware::from_fn(router_trace::trace))
        .layer(cors(origins))
        .with_state(requests)
}

/// What a browser may do here, and from where. The webhook route is untouched:
/// Plaid sends no `Origin`, and a request without one is passed through.
///
/// `allow_headers` is easy to leave out and makes the web app work at all: a
/// preflight that does not name the header back is a request the browser never
/// sends, arriving as nothing at all on this side. Listed rather than `Any`,
/// whose wildcard does not cover `Authorization`.
fn cors(origins: &WebOrigins) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(origins.values())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        // Without it every call the web app makes is two round trips.
        .max_age(Duration::from_secs(3600))
}

/// `GET /snapshot`
///
/// Answers with the ledger of whoever the bearer token signs in, and 401s
/// without one.
///
/// **It takes nothing.** A parameter that forces a trip to Plaid hands a
/// gesture anybody can repeat the power to spend this account's Plaid budget;
/// `should_fetch` is where that decision belongs.
async fn snapshot(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WireSnapshot>, StatusCode> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // The machine answers only once every fetch this request set going has
    // landed, so a snapshot cannot serve rows its own sync would replace.
    let answer = runtime_router::dispatch(
        &requests,
        Client::anonymous(ip),
        EventRouter::SnapshotRequested { token_sha256 },
    )
    .await;

    too_many(&answer)?;

    match answer {
        Some(ActionRouter::Serve { snapshot }) => Ok(Json(snapshot)),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// What `POST /auth/request` takes.
#[derive(Debug, Deserialize)]
struct AuthRequestBody {
    email: String,
}

/// `POST /auth/request` — send a sign-in code to this address.
///
/// Always answers 204, whether or not the address is one we know. The
/// alternative tells anybody who asks which addresses have accounts here.
async fn auth_request(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthRequestBody>,
) -> StatusCode {
    let email = normalize(&body.email);

    // `Unavailable` is the machine refusing to mint a code it has no entropy
    // for; 204 there leaves somebody waiting for an email that is not coming.
    let answer = runtime_router::dispatch(
        &requests,
        Client::anonymous(ip),
        EventRouter::CodeRequested { email },
    )
    .await;

    match answer {
        Some(ActionRouter::Accepted) => StatusCode::NO_CONTENT,

        // The one place the 204 rule bends, and it leaks nothing: 429 is a fact
        // about the caller, not about the address.
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
/// somebody working through guesses.
async fn auth_verify(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthVerifyBody>,
) -> Result<Json<AuthVerified>, StatusCode> {
    let answer = runtime_router::dispatch(
        &requests,
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

        // A store that would not answer, told apart from a code that was no
        // good because only one of the two is the caller's to fix.
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
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkStarted>, StatusCode> {
    // Hashed here: the plaintext is a live credential and the event goes in the
    // log.
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Anonymous, because who is asking is not known yet — the machine resolves
    // the session and buckets the request once it has.
    let answer = runtime_router::dispatch(
        &requests,
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
/// Three answers rather than a count. Plaid is explicit that the redirect
/// closing the sheet is not a successful connection, so "Plaid has not said
/// yet" is a real state and reporting it as zero is sometimes a false one.
/// Tagged rather than a count and a flag, because `pending` with three banks
/// attached is a state somebody would have to check for.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum LinkFinished {
    /// Banks were attached, and this is how many — one call collects every
    /// session the caller has open. It does not name them: nothing knows what a
    /// bank is called until the balances call inside its first sync answers.
    Linked { count: u32 },

    /// The session ended without a bank. The ordinary outcome of somebody
    /// closing the sheet, not an error.
    Abandoned,

    /// Plaid has not said what the session came to, and the bank may still be
    /// landing.
    Pending,
}

/// `POST /link/finish` — what did the Link session come to?
///
/// Two jobs, depending on how results are collected. With
/// `PLAID_COLLECT=poll` it is the *trigger*: nothing else ever collects. With
/// `PLAID_COLLECT=webhook` it is a *question*, and one that can be asked a
/// fraction of a second before the answer exists — which is why the machine
/// waits and why `Pending` exists.
async fn link_finish(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkFinished>, StatusCode> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &requests,
        Client::anonymous(ip),
        EventRouter::LinkClosed { token_sha256 },
    )
    .await;

    too_many(&answer)?;

    if let Some(ActionRouter::Unauthorized) = answer {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(Json(match answer {
        Some(ActionRouter::Linked { count }) => LinkFinished::Linked { count },
        Some(ActionRouter::LinkPending) => LinkFinished::Pending,
        Some(ActionRouter::LinkAbandoned) => LinkFinished::Abandoned,

        // The machine giving up on a poll that never answered, which from here
        // is the same sentence as a session that came to nothing.
        _ => LinkFinished::Abandoned,
    }))
}

/// `POST /plaid/webhook` — Plaid telling us something happened.
///
/// **The one route here with no session behind it**, so the first thing that
/// happens is proving the POST is Plaid's. The proof is over the *raw bytes*,
/// which is why this takes [`Bytes`] and not `Json`: a body an extractor had
/// already parsed hashes to something else, and every genuine webhook is
/// refused.
///
/// A believed webhook is answered 2xx even when nothing is done about it, since
/// Plaid retries anything else. One status for every way of being wrong; the
/// reason is logged and not returned.
async fn plaid_webhook(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(jwt) = headers
        .get(src_plaid::VERIFICATION_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::UNAUTHORIZED;
    };

    match runtime_router::dispatch(
        &requests,
        // Plaid, from addresses we do not choose. The one route with no
        // bucket — `EventRouter::route` says why.
        Client::anonymous(ip),
        EventRouter::WebhookReceived {
            jwt: jwt.to_string(),
            body: body.to_vec(),
        },
    )
    .await
    {
        // `Linked` is the same acceptance one turn further on: the webhook was
        // still taken.
        Some(
            ActionRouter::Accepted | ActionRouter::Linked { .. } | ActionRouter::LinkAbandoned,
        ) => StatusCode::NO_CONTENT,

        Some(ActionRouter::Unauthorized) => StatusCode::UNAUTHORIZED,

        // Not answered 2xx on purpose: Plaid sending it again is what we want.
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// `GET /sync/status` — how far the caller's first sync has got.
///
/// Its own route rather than a field on the snapshot, because `/snapshot`
/// blocks until the fetch it triggered has landed and could only ever report a
/// sync that was already over. This answers immediately, so it is pollable
/// while one runs, and a user with nothing syncing gets `finished` and zeroes
/// rather than a 404.
async fn sync_status(
    State(requests): State<RequestTx>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WireSyncStatus>, StatusCode> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &requests,
        Client::anonymous(ip),
        EventRouter::SyncStatusRequested { token_sha256 },
    )
    .await;

    too_many(&answer)?;

    match answer {
        Some(ActionRouter::SyncStatus { status }) => Ok(Json(status)),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED),
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// The hash of the bearer token this request carries, if it carries one.
///
/// Whether a token that is here names anybody is the machine's to decide; a
/// request with none at all has no session to resolve and is refused by the
/// caller of this.
fn bearer(headers: &axum::http::HeaderMap) -> Option<Vec<u8>> {
    let header = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    session_token::from_header(header).map(session_token::hash)
}

/// The address as the store and the machine both spell it.
fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Stop here if the machine refused this request for being one too many.
///
/// It carries no `Retry-After` — see
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
