//! The HTTP surface: what each route takes, and what it answers with.
//!
//! Every handler has the same shape and nothing else — dispatch one event,
//! answer from what came back. **The state is a channel**: a handler cannot
//! reach the store, the machine or the runtime, so a route cannot grow a rule
//! that no `eval!` test can reach.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
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
use axum::response::Sse;
use axum::response::sse::Event as SseEvent;
use axum::response::sse::KeepAlive;
use axum::response::sse::KeepAliveStream;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_stream::Stream;
use tower_http::cors::CorsLayer;

use crate::ActionRouter;
use crate::Client;
use crate::ClientIp;
use crate::EventRouter;
use crate::Refusal;
use crate::WEBHOOK_PATH;
use crate::WebOrigins;
use crate::router_trace;
use crate::runtime_router;
use crate::runtime_router::Closing;
use crate::runtime_router::ToLoop;
use crate::runtime_router::Watched;
use crate::session_token;
use server_wire::WireSnapshot;
use server_wire::WireSyncStatus;
use server_wire::WireUnpaid;

/// The route Stripe is told to call.
///
/// Beside the route rather than in `public_url`, unlike Plaid's: nothing here
/// advertises this address. It is typed into Stripe's dashboard once, and
/// #88 is where that is written down.
pub const STRIPE_WEBHOOK_PATH: &str = "/stripe/webhook";

/// The header Stripe signs a webhook with.
const STRIPE_SIGNATURE_HEADER: &str = "Stripe-Signature";

/// The route Apple is told to call.
///
/// Beside the others for the same reason Stripe's is: nothing advertises it,
/// and it is typed into App Store Connect once. Apple keeps a separate URL per
/// environment, so the sandbox and the production build of this server each
/// name their own.
pub const APPLE_WEBHOOK_PATH: &str = "/apple/webhook";

/// Every route, over one channel.
pub(crate) fn router(to_loop: ToLoop, origins: &WebOrigins) -> Router {
    Router::new()
        .route("/snapshot", get(snapshot))
        .route("/auth/request", post(auth_request))
        .route("/auth/verify", post(auth_verify))
        .route("/link/start", post(link_start))
        .route("/link/finish", post(link_finish))
        .route("/sync/status", get(sync_status))
        .route("/billing/subscribe", post(billing_subscribe))
        .route("/billing/status", get(billing_status))
        .route("/account/delete", post(account_delete))
        .route("/billing/apple", post(billing_apple))
        .route(STRIPE_WEBHOOK_PATH, post(stripe_webhook))
        .route(APPLE_WEBHOOK_PATH, post(apple_webhook))
        .route(WEBHOOK_PATH, post(plaid_webhook))
        // Inside the CORS layer, so a browser's preflight `OPTIONS` is answered
        // above this and never logged.
        .layer(axum::middleware::from_fn(router_trace::trace))
        .layer(cors(origins))
        .with_state(to_loop)
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
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WireSnapshot>, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // The machine answers only once every fetch this request set going has
    // landed, so a snapshot cannot serve rows its own sync would replace.
    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::SnapshotRequested { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::Serve { snapshot }) => Ok(Json(snapshot)),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE.into()),
    }
}

/// `POST /billing/subscribe`
///
/// Starts a subscription for whoever the bearer token signs in, and answers
/// with the secret the card form confirms against.
///
/// **It takes nothing.** The price is the server's, not the caller's: a body
/// naming an amount is a body somebody can edit.
async fn billing_subscribe(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SubscribeAnswer>, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::SubscribeRequested { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::Subscribing { client_secret }) => {
            Ok(Json(SubscribeAnswer { client_secret }))
        }
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE.into()),
    }
}

/// What `POST /billing/subscribe` answers with.
#[derive(Debug, serde::Serialize)]
struct SubscribeAnswer {
    /// What the card form confirms against. Not a credential of ours: it is
    /// scoped to one payment and is meant to reach a browser.
    client_secret: String,
}

/// `GET /billing/status`
///
/// What this user's subscription is. One of the two routes a session without
/// one may reach — the other is the one above.
async fn billing_status(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<BillingAnswer>, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::BillingStatusRequested { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::BillingStatus {
            entitled,
            unpaid,
            good_until,
            cancel_at_period_end,
        }) => Ok(Json(BillingAnswer {
            entitled,
            unpaid,
            good_until,
            cancel_at_period_end,
        })),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE.into()),
    }
}

/// What `GET /billing/status` answers with.
#[derive(Debug, serde::Serialize)]
struct BillingAnswer {
    entitled: bool,

    /// Why not, present exactly when `entitled` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    unpaid: Option<WireUnpaid>,

    good_until: Option<u64>,
    cancel_at_period_end: bool,
}

/// `POST /account/delete`
///
/// Cancel the subscription, stop the banks at Plaid, and delete everything.
/// One action, because a paused account is one we keep paying Plaid for.
///
/// **It takes nothing, and there is no undo.** The confirmation belongs in the
/// client, where somebody can be told what they are about to lose; a body
/// asking "are you sure" is a thing that can be sent by anything holding the
/// token.
///
/// Answered only once the rows are actually gone, so a client that hears this
/// can forget its session without wondering.
async fn account_delete(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::DeleteRequested { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::Deleted) => Ok(StatusCode::NO_CONTENT),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE.into()),
    }
}

/// `POST /stripe/webhook`
///
/// What Stripe says about a subscription. No session and no rate limit: the
/// signature is the whole of what protects it, and it is checked in the
/// machine rather than here.
///
/// The body is taken as raw [`Bytes`] because that is what the signature is
/// over. Anything that parsed it first would hand the machine a body whose
/// signature no longer matches.
async fn stripe_webhook(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    let Some(signature) = headers
        .get(STRIPE_SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::UNAUTHORIZED;
    };

    match runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::StripeWebhookReceived {
            signature: signature.to_string(),
            body: body.to_vec(),
        },
    )
    .await
    {
        Some(ActionRouter::Accepted) => StatusCode::NO_CONTENT,
        Some(ActionRouter::Unauthorized) => StatusCode::UNAUTHORIZED,

        // Anything else is a webhook we could not act on. Told to Stripe as a
        // failure so it retries: dropping one silently is a subscription
        // nobody is entitled by.
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// `POST /billing/apple`
///
/// A purchase the App Store has already taken, handed over by the app.
///
/// Authenticated, unlike the webhook below, and that is the whole reason it
/// exists: Apple's transaction says which Apple ID paid and never which of our
/// accounts did, so the session is the only thing that ties the two together.
/// Open to a session that has not paid, because paying is what it is doing.
async fn billing_apple(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AppleTransactionBody>,
) -> Result<StatusCode, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::AppleTransactionReceived {
            token_sha256,
            signed: body.signed_transaction,
        },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::Accepted) => Ok(StatusCode::NO_CONTENT),
        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),

        // The Apple ID that paid already pays for another account. Its own
        // code because it is the one refusal here somebody can act on.
        Some(ActionRouter::Claimed) => Err(StatusCode::CONFLICT.into()),

        _ => Err(StatusCode::SERVICE_UNAVAILABLE.into()),
    }
}

/// What `POST /billing/apple` takes.
#[derive(Debug, Deserialize)]
struct AppleTransactionBody {
    /// What StoreKit calls `jwsRepresentation`: the transaction, signed by
    /// Apple. Nothing in it is believed until the machine has checked the
    /// chain that signed it.
    #[serde(rename = "signedTransaction")]
    signed_transaction: String,
}

/// `POST /apple/webhook`
///
/// What Apple says happened to a subscription while the app was shut. No
/// session and no rate limit, the same as Stripe's: the signature on the
/// payload is the whole of what protects it, and it is checked in the machine
/// rather than here.
///
/// Unlike the two webhooks beside it the body is parsed before anything is
/// believed, and that is safe here: Apple signs the payload itself rather than
/// the envelope that carried it, so re-reading the JSON changes nothing that
/// is checked.
async fn apple_webhook(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AppleNotificationBody>,
) -> StatusCode {
    match runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::AppleNotificationReceived {
            signed: body.signed_payload,
        },
    )
    .await
    {
        Some(ActionRouter::Accepted) => StatusCode::NO_CONTENT,
        Some(ActionRouter::Unauthorized) => StatusCode::UNAUTHORIZED,

        // Anything else is a notification we could not act on. Told to Apple
        // as a failure so it sends it again: dropping one silently is a
        // renewal nobody is entitled by, or a refund nobody loses access for.
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// What `POST /apple/webhook` takes.
#[derive(Debug, Deserialize)]
struct AppleNotificationBody {
    /// Apple's `signedPayload`, which is the whole notification as a JWS.
    #[serde(rename = "signedPayload")]
    signed_payload: String,
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
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthRequestBody>,
) -> StatusCode {
    let email = normalize(&body.email);

    // `Unavailable` is the machine refusing to mint a code it has no entropy
    // for; 204 there leaves somebody waiting for an email that is not coming.
    let answer = runtime_router::dispatch(
        &to_loop,
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
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    Json(body): Json<AuthVerifyBody>,
) -> Result<Json<AuthVerified>, Refusal> {
    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::CodeSubmitted {
            email: normalize(&body.email),
            code: body.code.trim().to_string(),
        },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::SignedIn { token }) => Ok(Json(AuthVerified { token })),

        // A store that would not answer, told apart from a code that was no
        // good because only one of the two is the caller's to fix.
        Some(ActionRouter::Unavailable) => Err(StatusCode::INTERNAL_SERVER_ERROR.into()),

        _ => Err(StatusCode::UNAUTHORIZED.into()),
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
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkStarted>, Refusal> {
    // Hashed here: the plaintext is a live credential and the event goes in the
    // log.
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Anonymous, because who is asking is not known yet — the machine resolves
    // the session and buckets the request once it has.
    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::LinkRequested { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::LinkOpened { url }) => Ok(Json(LinkStarted { url })),

        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),

        // The session this caller holds could not be read. Told apart from
        // Plaid refusing, the same way the other routes tell a store that
        // would not answer apart from an answer: a gateway error points at
        // somebody else's service for something that went wrong here.
        Some(ActionRouter::Unavailable) => Err(StatusCode::INTERNAL_SERVER_ERROR.into()),

        _ => Err(StatusCode::BAD_GATEWAY.into()),
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
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LinkFinished>, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let answer = runtime_router::dispatch(
        &to_loop,
        Client::anonymous(ip),
        EventRouter::LinkClosed { token_sha256 },
    )
    .await;

    refused(&answer)?;

    match answer {
        Some(ActionRouter::Linked { count }) => Ok(Json(LinkFinished::Linked { count })),
        Some(ActionRouter::LinkPending) => Ok(Json(LinkFinished::Pending)),
        Some(ActionRouter::LinkAbandoned) => Ok(Json(LinkFinished::Abandoned)),

        Some(ActionRouter::Unauthorized) => Err(StatusCode::UNAUTHORIZED.into()),

        // The store would not say what this caller has open, or the poll it
        // asked for never came back. Told apart from a session that came to
        // nothing: "abandoned" is a sentence about somebody's bank, and saying
        // it because Postgres was down is saying something we do not know.
        Some(ActionRouter::Unavailable) => Err(StatusCode::INTERNAL_SERVER_ERROR.into()),

        // Nothing at all, which cannot happen: every router event answers. It
        // reads as the ordinary ending rather than a 500, because a phone with
        // no bank attached has the same road out of both.
        _ => Ok(Json(LinkFinished::Abandoned)),
    }
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
    State(to_loop): State<ToLoop>,
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
        &to_loop,
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

/// `GET /sync/status` — how the caller's sync is going, for as long as it goes.
///
/// A response that does not end: the machine writes to it whenever the sync it
/// is watching moves, and the phone reads the body as it arrives. Its own route
/// rather than a field on the snapshot, because `/snapshot` blocks until the
/// fetch it triggered has landed and could only ever report a sync that was
/// already over.
///
/// The first message rides on the answer that opens the stream, so a client
/// that connects late — or reconnects, which is every time iOS backgrounds the
/// app — is told where things stand rather than waiting for the next change. A
/// caller with nothing running is told that, and left connected.
async fn sync_status(
    State(to_loop): State<ToLoop>,
    Extension(ip): Extension<ClientIp>,
    headers: axum::http::HeaderMap,
) -> Result<Sse<KeepAliveStream<Statuses>>, Refusal> {
    let token_sha256 = bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Refused before a byte of the response has gone out. A stream that opens
    // and then dies silently reads to a client as a sync with nothing to say,
    // which is a far worse answer than 401.
    let watched = runtime_router::watch(&to_loop, Client::anonymous(ip), token_sha256).await;

    let statuses = match watched {
        Watched::Open {
            status,
            updates,
            closing,
        } => Statuses {
            first: Some(status),
            updates,
            closing,
        },

        Watched::Refused(Some(ActionRouter::Unauthorized)) => {
            return Err(StatusCode::UNAUTHORIZED.into());
        }

        Watched::Refused(Some(ActionRouter::PaymentRequired { unpaid })) => {
            return Err(Refusal::unpaid(unpaid));
        }

        Watched::Refused(Some(ActionRouter::TooMany)) => {
            return Err(StatusCode::TOO_MANY_REQUESTS.into());
        }

        Watched::Refused(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR.into()),
    };

    // Comments down an idle connection, so nothing between here and the phone
    // reaps a sync that is simply taking a while.
    Ok(Sse::new(statuses).keep_alive(KeepAlive::default()))
}

/// The body of a watch: where things stood when it opened, then every message
/// after it.
struct Statuses {
    /// What the stream was accepted with, taken on the first poll.
    first: Option<WireSyncStatus>,

    updates: UnboundedReceiver<WireSyncStatus>,

    /// Dropped with the response, which is what tells the loop this connection
    /// has gone. Held rather than read.
    #[allow(dead_code)]
    closing: Closing,
}

impl Stream for Statuses {
    type Item = Result<SseEvent, axum::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Every field is `Unpin`, so there is nothing here to project.
        let this = self.get_mut();

        if let Some(status) = this.first.take() {
            return Poll::Ready(Some(named(status)));
        }

        this.updates.poll_recv(cx).map(|status| status.map(named))
    }
}

/// One status as a message on the wire.
///
/// Named, though it is the only kind this stream carries: a client that
/// dispatches on the name reads a second kind added later as one it does not
/// handle, rather than as a sync status it cannot parse.
fn named(status: WireSyncStatus) -> Result<SseEvent, axum::Error> {
    SseEvent::default().event("sync").json_data(status)
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

/// Stop here if the machine refused this request outright — for being one too
/// many, or for coming from somebody who has not paid.
///
/// Both here rather than in each route's `match`, so a route added later
/// cannot answer a refusal with a 503 by forgetting to name it. `TooMany`
/// carries no `Retry-After` — see
/// [`ActionRouter::TooMany`](crate::ActionRouter::TooMany).
fn refused(answer: &Option<ActionRouter>) -> Result<(), Refusal> {
    match answer {
        Some(ActionRouter::TooMany) => Err(StatusCode::TOO_MANY_REQUESTS.into()),

        // Signed in, and has not paid. Told apart from a 401 on purpose: the
        // session is good, and what the client needs is a paywall rather than
        // a sign-in screen. It is the one refusal with a body, because which
        // paywall is a fact only this side holds.
        Some(ActionRouter::PaymentRequired { unpaid }) => Err(Refusal::unpaid(unpaid.clone())),

        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;

    use super::*;

    /// The codes a refusal turns into, which are a wire contract with every
    /// client: 402 is a paywall and 401 is a sign-in screen, and a client that
    /// conflated them would sign somebody out for not having paid.
    #[test]
    fn a_refusal_is_told_apart_from_a_failure() {
        let code = |answer| match refused(&Some(answer)) {
            Ok(()) => None,
            Err(refusal) => Some(refusal.into_response().status()),
        };

        assert_eq!(
            code(ActionRouter::PaymentRequired {
                unpaid: WireUnpaid {
                    reason: "never".to_string(),
                    ran_out: None,
                },
            }),
            Some(StatusCode::PAYMENT_REQUIRED),
        );

        assert_eq!(
            code(ActionRouter::TooMany),
            Some(StatusCode::TOO_MANY_REQUESTS)
        );

        // Everything else is the route's own business, and passes through.
        assert_eq!(code(ActionRouter::Unauthorized), None);
        assert_eq!(code(ActionRouter::Accepted), None);
        assert!(refused(&None).is_ok());
    }

    /// The 402 is the one refusal with a body, and the body is why. A client
    /// reading the code alone cannot tell somebody who has never subscribed
    /// from somebody whose card stopped working, and those are two screens.
    #[tokio::test]
    async fn a_payment_refusal_says_why() {
        let refusal = Refusal::unpaid(WireUnpaid {
            reason: "declined".to_string(),
            ran_out: Some(1_786_492_800),
        });

        let response = refusal.into_response();

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body built from a plain struct");

        assert_eq!(
            String::from_utf8_lossy(&body),
            r#"{"reason":"declined","ran_out":1786492800}"#,
        );
    }

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
