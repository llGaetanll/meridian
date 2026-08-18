//! Turning an event into exactly one line of text.

use std::fmt::Debug;
use std::fmt::Write as _;

use tracing::field::Field;
use tracing::field::Visit;

/// How much of a value we are willing to put on a line before cutting it. Long
/// values are usually response bodies, and the head of one is enough to tell
/// what came back.
const MAX_VALUE: usize = 400;

/// Collects an event's fields into one line, message first.
///
/// The message is what a human reads; everything else is `key=value` after it,
/// in the order the event declared. Values that would break the one-line rule
/// are escaped, and long ones are cut with a `…`.
#[derive(Default)]
pub struct Line {
    message: String,
    fields: String,
}

impl Line {
    pub fn finish(self) -> String {
        let mut out = self.message;

        if !self.fields.is_empty() {
            if !out.is_empty() {
                out.push(' ');
            }

            out.push_str(&self.fields);
        }

        out
    }

    fn push(&mut self, name: &str, value: &str) {
        if name == "message" {
            self.message = escape(value);
            return;
        }

        if !self.fields.is_empty() {
            self.fields.push(' ');
        }

        let _ = write!(self.fields, "{}={}", name, quote(value));
    }
}

impl Visit for Line {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field.name(), value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.push(field.name(), &format!("{value:?}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        // Errors are worth their causes: the outer message alone is often just
        // "Network error" with the reason one level down.
        let mut text = value.to_string();
        let mut source = value.source();

        while let Some(cause) = source {
            let _ = write!(text, ": {cause}");
            source = cause.source();
        }

        self.push(field.name(), &text);
    }
}

/// Newlines and tabs would split the line in two, so they are spelled out.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for c in value.chars().take(MAX_VALUE) {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }

    if value.chars().nth(MAX_VALUE).is_some() {
        out.push('…');
    }

    out
}

/// Values only get quotes when they need them, so the common case — a symbol,
/// a status, a count — stays easy to read and easy to grep.
fn quote(value: &str) -> String {
    let escaped = escape(value);

    let needs_quotes = escaped.is_empty()
        || escaped
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '=');

    if needs_quotes {
        format!("\"{}\"", escaped.replace('"', "\\\""))
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_needs_it() {
        assert_eq!(quote("SPY"), "SPY");
        assert_eq!(quote("1d"), "1d");
        assert_eq!(quote("two words"), "\"two words\"");
        assert_eq!(quote(""), "\"\"");
    }

    #[test]
    fn keeps_multiline_values_on_one_line() {
        assert_eq!(escape("a\nb"), "a\\nb");
    }

    #[test]
    fn cuts_long_values() {
        let long = "x".repeat(MAX_VALUE * 2);
        let cut = escape(&long);

        assert_eq!(cut.chars().count(), MAX_VALUE + 1);
        assert!(cut.ends_with('…'));
    }
}
