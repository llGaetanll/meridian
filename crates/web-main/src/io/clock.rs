//! The browser's clock, read at the edge.

use std::time::Duration;

use js_sys::Promise;
use wasm_bindgen_futures::JsFuture;

/// The wall clock, in seconds.
///
/// Read here and sent to the machine, which has none of its own. The
/// browser's is the only clock the app has: the server sends the timestamps
/// of things and never a reading of its own.
pub fn now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).unwrap_or(0)
}

/// Wait this long.
///
/// A `setTimeout` behind a promise. With no window to ask, it returns at
/// once, which only ever happens outside a browser.
pub async fn sleep(duration: Duration) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let ms = i32::try_from(duration.as_millis()).unwrap_or(i32::MAX);

    let promise = Promise::new(&mut |resolve, _| {
        if window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
            .is_err()
        {
            // No timer to wait on. Resolved at once, so the wait is short
            // rather than endless.
            let _ = resolve.call0(&resolve);
        }
    });

    let _ = JsFuture::from(promise).await;
}
