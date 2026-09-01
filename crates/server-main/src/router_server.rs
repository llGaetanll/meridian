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

use axum::Json;
use axum::Router;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

use crate::ActionRouter;
use crate::EventRouter;
use crate::RuntimeServer;
use crate::router_trace;
use server_wire::WireSnapshot;
use server_wire::WireSyncStatus;

/// Every route, over one runtime.
pub(crate) fn router(runtime: Arc<RuntimeServer>) -> Router {
    Router::new()
        .route("/snapshot", get(snapshot))
        .route("/auth/request", post(auth_request))
        .route("/auth/verify", post(auth_verify))
        .route("/link/start", post(link_start))
        .route("/link/finish", post(link_finish))
        .route("/sync/status", get(sync_status))
        .route("/health", get(health))
        // Inside the CORS layer, so the preflight `OPTIONS` a browser sends
        // ahead of an authenticated request is answered above this and never
        // logged. It is transport, and a line for each would double the
        // traffic in the log without saying anything about it.
        .layer(axum::middleware::from_fn(router_trace::trace))
        // The phone app is served by trunk on another port, so every request
        // it makes is cross-origin. This is a development server on localhost;
        // a deployed one would name its origin instead.
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .with_state(runtime)
}

/// Everything a request can ask for.
#[derive(Debug, Default, Deserialize)]
struct SnapshotQuery {
    /// Skip the freshness window and go straight to Plaid. Pull-to-refresh.
    #[serde(default)]
    refresh: bool,
}

/// `GET /snapshot`
///
/// Answers with the ledger of whoever the bearer token signs in, and 401s
/// without one. There is no unauthenticated form: this is somebody's bank
/// transactions.
async fn snapshot(
    State(runtime): State<Arc<RuntimeServer>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<SnapshotQuery>,
) -> Result<Json<WireSnapshot>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    // Read before deciding. These are the facts the machine needs and does not
    // keep — where each bank's last sync reached, and whether it still works.
    let items = runtime.facts_for(user_id).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "could not read this user's banks");
        Vec::new()
    });

    runtime
        .request(EventRouter::SnapshotRequested {
            force: query.refresh,
            items,
        })
        .await;

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
    Json(body): Json<AuthRequestBody>,
) -> StatusCode {
    let email = body.email.trim().to_lowercase();

    // Not an address, so nothing to send to. Still 204: see above.
    if !email.contains('@') || email.len() < 3 {
        tracing::warn!("a sign-in was asked for with something that is not an address");

        return StatusCode::NO_CONTENT;
    }

    // Read off what the machine decided rather than assumed. `Unavailable` is
    // the machine refusing to mint a code it has no entropy for, and answering
    // 204 to that would leave somebody waiting for an email that is not coming.
    match runtime.request(EventRouter::CodeRequested { email }).await {
        Some(ActionRouter::Accepted) => StatusCode::NO_CONTENT,
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
    Json(body): Json<AuthVerifyBody>,
) -> Result<Json<AuthVerified>, StatusCode> {
    let email = body.email.trim().to_lowercase();

    // What the store holds against this address, read before deciding — the
    // same shape as the snapshot path, and for the same reason: the machine
    // keeps nothing, so the facts arrive with the request.
    let stored = match runtime.stored_code(&email).await {
        Ok(stored) => stored,

        Err(e) => {
            tracing::error!(error = %e, "could not read the sign-in code");

            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // The outcome is read off what the machine decided, not worked out again
    // here. There is exactly one answer that signs somebody in.
    let answer = runtime
        .request(EventRouter::CodeSubmitted {
            email,
            code: body.code.trim().to_string(),
            stored,
        })
        .await;

    let Some(ActionRouter::SignedIn { token }) = answer else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    Ok(Json(AuthVerified { token }))
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
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkStarted>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    let answer = runtime
        .request(EventRouter::LinkRequested { user_id })
        .await;

    let Some(ActionRouter::LinkOpened { url }) = answer else {
        return Err(StatusCode::BAD_GATEWAY);
    };

    Ok(Json(LinkStarted { url }))
}

/// What `POST /link/finish` answers with.
#[derive(Debug, serde::Serialize)]
struct LinkFinished {
    /// Whether a bank is now attached. False is the ordinary outcome of
    /// somebody closing the sheet, not an error.
    linked: bool,
}

/// `POST /link/finish` — collect whatever the Link session came to.
///
/// Called when the web sheet closes. The redirect that closed it carries no
/// token (`docs/PLAID.md`), so this is what goes and asks Plaid; with
/// `PLAID_COLLECT=webhook` the same collection would arrive on its own and
/// this becomes a read of what already happened.
async fn link_finish(
    State(runtime): State<Arc<RuntimeServer>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkFinished>, StatusCode> {
    let user_id = runtime.authenticate(&headers).await?;

    // The sessions this user has open, read before deciding — the same shape
    // as everywhere else: the machine keeps nothing, so the facts arrive with
    // the request.
    let pending = runtime.pending_links(user_id).await.map_err(|e| {
        tracing::error!(error = %e, "could not read this user's link sessions");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let answer = runtime.request(EventRouter::LinkClosed { pending }).await;

    Ok(Json(LinkFinished {
        linked: matches!(answer, Some(ActionRouter::Linked)),
    }))
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
        .request(EventRouter::SyncStatusRequested { items })
        .await;

    let Some(ActionRouter::SyncStatus { status }) = answer else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    Ok(Json(status))
}

/// `GET /health`
/// Whether the server is up, and whether what it depends on is.
///
/// It reports the database by asking it something rather than by checking that
/// a pool exists: a pool holds connections the far end may have closed since,
/// so the only honest answer comes from a query. This is what `docker compose
/// up` is checked with, so an optimistic answer here would defeat the point of
/// having it.
async fn health(State(runtime): State<Arc<RuntimeServer>>) -> (StatusCode, String) {
    match runtime.ping_store().await {
        Ok(()) => (StatusCode::OK, String::from("ok store=postgres")),
        Err(e) => {
            tracing::error!(error = %e, "the store is not answering");

            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("store unreachable: {e}"),
            )
        }
    }
}
