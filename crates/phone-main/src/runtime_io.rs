//! Talking to the server. No decisions and no reactive state — plain async
//! functions over plain data, exactly as `docs/ARCHITECTURE.md` asks.
//!
//! There is one call: fetch the snapshot. Everything the app draws comes from
//! it, so there is no "which of my requests has landed" state anywhere in the
//! front end.

use api_trace::ApiCall;
use api_trace::Source;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Request;
use web_sys::RequestInit;
use web_sys::Response;

/// Where `server-main` listens. A constant rather than configuration: this is a
/// sketch crate that runs beside the server on one machine, and the native
/// apps will point somewhere else entirely.
const SERVER: &str = "http://127.0.0.1:8082";

/// Our own server, as the log knows it. No rules: we write this JSON and
/// already write it compact, so there is nothing for `compact_json` to take
/// out that we put in.
const MERIDIAN: Source = Source::new("meridian");

/// The snapshot, as JSON.
///
/// Handed to the machine as a string rather than parsed here, so that all three
/// platforms get the same parse. Doing it in the runtime would mean Swift and
/// Kotlin each writing their own, which is three chances to disagree about what
/// a missing field means.
pub async fn fetch_snapshot() -> Result<String, String> {
    let url = format!("{SERVER}/snapshot");

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init(&url, &opts).map_err(describe)?;

    let window = web_sys::window().ok_or("no window")?;

    // Logged like every other call that crosses the network, and the other
    // half of what the server writes for the same request. Two logs, one
    // conversation: the phone's says what it asked for and what it was given
    // to parse, the server's says what it decided to answer.
    let call = ApiCall::get(module_path!(), MERIDIAN, &url);

    let response = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(response) => response,

        Err(e) => {
            call.failed(describe(e));

            // The overwhelmingly likely cause, and worth saying plainly rather
            // than surfacing a browser's opaque "TypeError: Failed to fetch".
            return Err(format!(
                "could not reach the server at {SERVER} — is `cargo run -p server-main` running?"
            ));
        }
    };

    let Ok(response) = response.dyn_into::<Response>() else {
        call.failed("not a response");

        return Err(String::from("not a response"));
    };

    let text = match response.text().map(JsFuture::from) {
        Ok(text) => text.await.map_err(describe),
        Err(e) => Err(describe(e)),
    };

    let text = match text {
        Ok(text) => text,

        Err(e) => {
            call.failed(&e);

            return Err(e);
        }
    };

    // Logged whatever we go on to do with it. An error page and a snapshot
    // arrive the same way, and which one it was is the question a log of this
    // has to answer.
    let body = text.as_string().unwrap_or_default();

    call.response(response.status(), &body);

    if !response.ok() {
        return Err(format!("the server answered {}", response.status()));
    }

    if body.is_empty() {
        return Err(String::from("empty body"));
    }

    Ok(body)
}

fn describe(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| "request failed".to_string())
}
