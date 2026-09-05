use std::collections::HashMap;

use axum::Json;
use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::state::ActionRouter;
use crate::state::EventRouter;
use crate::state::RequestId;

const PORT: u16 = 8099;

const REQUESTS_WAITING: usize = 64;

pub type ActionRx = UnboundedReceiver<(RequestId, ActionRouter)>;
pub type ActionTx = UnboundedSender<(RequestId, ActionRouter)>;

pub type EventRx = UnboundedReceiver<(RequestId, EventRouter)>;
pub type EventTx = UnboundedSender<(RequestId, EventRouter)>;

type RequestRx = mpsc::Receiver<Request>;
type RequestTx = mpsc::Sender<Request>;

struct Request {
    event: EventRouter,
    response_tx: oneshot::Sender<Response>,
}

#[derive(Default)]
pub struct RuntimeRouter {
    next_request: RequestId,
    parked: HashMap<RequestId, oneshot::Sender<Response>>,
}

impl RuntimeRouter {
    pub async fn run(mut action_rx: ActionRx, event_tx: EventTx) -> std::io::Result<()> {
        let (request_tx, mut request_rx): (RequestTx, RequestRx) = mpsc::channel(REQUESTS_WAITING);

        let app = Router::new()
            .route("/ping", get(ping))
            .route("/thing", post(thing_posted))
            .route("/thing/{name}", get(thing))
            .with_state(request_tx);

        let listener = TcpListener::bind(("127.0.0.1", PORT)).await?;

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!(error = %e, "the http server stopped");
            }
        });

        tracing::info!(port = PORT, "sketch listening");

        let mut router = Self::default();

        loop {
            tokio::select! {
                Some(request) = request_rx.recv() => {
                    if event_tx.send(router.park(request)).is_err() {
                        return Ok(());
                    }
                }

                Some((request_id, action)) = action_rx.recv() => {
                    router.answer(request_id, action);
                }

                else => return Ok(()),
            }
        }
    }

    fn park(&mut self, request: Request) -> (RequestId, EventRouter) {
        self.next_request += 1;

        self.parked.insert(self.next_request, request.response_tx);

        (self.next_request, request.event)
    }

    fn answer(&mut self, request_id: RequestId, action: ActionRouter) {
        let Some(response_tx) = self.parked.remove(&request_id) else {
            tracing::debug!(request_id, "answered a route that is no longer waiting");

            return;
        };

        if response_tx.send(render(action)).is_err() {
            tracing::debug!(request_id, "a route hung up before its answer arrived");
        }
    }
}

fn render(action: ActionRouter) -> Response {
    match action {
        ActionRouter::Serve { body } => (StatusCode::OK, body).into_response(),
        ActionRouter::Accepted => StatusCode::NO_CONTENT.into_response(),
        ActionRouter::Unavailable => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn ping(State(requests): State<RequestTx>) -> Response {
    dispatch(&requests, EventRouter::Pinged).await
}

async fn thing(State(requests): State<RequestTx>, Path(name): Path<String>) -> Response {
    dispatch(&requests, EventRouter::ThingRequested { name }).await
}

#[derive(Debug, Deserialize)]
struct ThingBody {
    name: String,
}

async fn thing_posted(State(requests): State<RequestTx>, Json(body): Json<ThingBody>) -> Response {
    dispatch(&requests, EventRouter::ThingRequested { name: body.name }).await
}

async fn dispatch(requests: &RequestTx, event: EventRouter) -> Response {
    let (response_tx, response_rx) = oneshot::channel();

    if requests.send(Request { event, response_tx }).await.is_err() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    match response_rx.await {
        Ok(response) => response,

        Err(_) => {
            tracing::error!("a route asked something nothing answered");

            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
