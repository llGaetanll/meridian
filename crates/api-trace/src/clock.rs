//! Wall clock readings, the same on native and in the browser.
//!
//! `chrono` reads the JS clock on wasm, so one implementation covers both and
//! timestamps in a log file mean the same thing wherever it was produced.

use chrono::DateTime;
use chrono::Utc;

/// A timestamp for the head of a line. UTC, so logs from two machines sort.
pub fn now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

pub fn instant() -> DateTime<Utc> {
    Utc::now()
}

/// Milliseconds since `start`, floored at zero: a clock that steps backwards
/// should not turn into a negative duration halfway down a log.
pub fn millis_since(start: DateTime<Utc>) -> i64 {
    (Utc::now() - start).num_milliseconds().max(0)
}
