//! Talking to the server. No decisions and no reactive state — plain async
//! functions over plain data, exactly as `docs/ARCHITECTURE.md` asks.
//!
//! There is one call: fetch the snapshot. Everything the app draws comes from
//! it, so there is no "which of my requests has landed" state anywhere in the
//! front end.

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

/// The snapshot, as JSON.
///
/// Handed to the machine as a string rather than parsed here, so that all three
/// platforms get the same parse. Doing it in the runtime would mean Swift and
/// Kotlin each writing their own, which is three chances to disagree about what
/// a missing field means.
pub async fn fetch_snapshot(force: bool) -> Result<String, String> {
    let url = if force {
        format!("{SERVER}/snapshot?refresh=true")
    } else {
        format!("{SERVER}/snapshot")
    };

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init(&url, &opts).map_err(describe)?;

    let window = web_sys::window().ok_or("no window")?;

    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| {
            // The overwhelmingly likely cause, and worth saying plainly rather
            // than surfacing a browser's opaque "TypeError: Failed to fetch".
            let _ = describe(e);
            format!(
                "could not reach the server at {SERVER} — is `cargo run -p server-main` running?"
            )
        })?;

    let response: Response = response.dyn_into().map_err(|_| "not a response")?;

    if !response.ok() {
        return Err(format!("the server answered {}", response.status()));
    }

    let text = JsFuture::from(response.text().map_err(describe)?)
        .await
        .map_err(describe)?;

    text.as_string().ok_or_else(|| "empty body".to_string())
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
