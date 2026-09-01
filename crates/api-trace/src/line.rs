//! Turning an event into exactly one line of text.

use std::fmt::Debug;
use std::fmt::Write as _;

use tracing::field::Field;
use tracing::field::Visit;

use crate::style;
use crate::style::Paint;

/// How much of a value we are willing to put on a line before cutting it. Long
/// values are usually response bodies, and the head of one is enough to tell
/// what came back.
const MAX_VALUE: usize = 400;

/// Collects an event's fields into one line, message first.
///
/// The message is what a human reads; everything else is `key=value` after it,
/// in the order the event declared. Values that would break the one-line rule
/// are escaped, and long ones are cut with a `…`.
///
/// Fields are kept apart until [`Line::render`] so that a name can be dimmed
/// and its value left bright, which is the whole point of colouring a log.
///
/// Two field names are not fields. `message` is the message, and `module` is
/// the module the line came from, printed in the target column — see
/// [`Line::module`].
#[derive(Clone, Default)]
pub struct Line {
    message: String,
    module: Option<String>,
    fields: Vec<(String, String)>,
}

impl Line {
    pub fn is_empty(&self) -> bool {
        self.message.is_empty() && self.fields.is_empty()
    }

    /// Where the line says it came from, if it said.
    ///
    /// `tracing` fixes an event's target at the callsite, baked into a static
    /// that has to be a constant, so a helper that logs on somebody else's
    /// behalf can only ever report *itself*. Every `->`/`<-` pair is logged
    /// inside [`crate::ApiCall`] and [`crate::Served`], which would make the
    /// whole of the network half of this log read `api_trace::call`.
    ///
    /// So the caller passes its own `module_path!()` in as a field and the
    /// renderer prints that in the target column instead. Underneath, the
    /// event keeps the target `tracing` gave it — `api_trace::call`, which is
    /// where it really happened, and what a level filter still sees. The
    /// reader is shown the other one, because the module a reader wants is the
    /// one to go and look in.
    pub fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    /// The message, then every `key=value`, separated by single spaces.
    pub fn render(&self, paint: Paint) -> String {
        let mut out = String::new();

        out.push_str(&self.message);

        for (name, value) in &self.fields {
            if !out.is_empty() {
                out.push(' ');
            }

            paint.start(&mut out, style::DIM);
            out.push_str(name);
            out.push('=');
            paint.end(&mut out);

            out.push_str(value);
        }

        out
    }

    fn push(&mut self, name: &str, value: &str) {
        if name == "message" {
            self.message = escape(value);
            return;
        }

        if name == "module" {
            self.module = Some(escape(value));
            return;
        }

        self.fields.push((name.to_string(), quote(value)));
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
    fn a_module_is_taken_off_the_fields() {
        let mut line = Line::default();
        line.push("module", "src_plaid::plaid_client");
        line.push("id", "1");

        assert_eq!(line.module(), Some("src_plaid::plaid_client"));
        assert_eq!(line.render(Paint::Off), "id=1");
    }

    #[test]
    fn renders_message_then_fields() {
        let mut line = Line::default();
        line.push("message", "quote asked for");
        line.push("symbol", "SPY");

        assert_eq!(line.render(Paint::Off), "quote asked for symbol=SPY");
    }

    #[test]
    fn colours_the_key_and_leaves_the_value_alone() {
        let mut line = Line::default();
        line.push("symbol", "SPY");

        let rendered = line.render(Paint::Ansi);

        assert!(rendered.ends_with("SPY"));
        assert!(rendered.contains("symbol="));
        assert_ne!(rendered, line.render(Paint::Off));
    }

    #[test]
    fn cuts_long_values() {
        let long = "x".repeat(MAX_VALUE * 2);
        let cut = escape(&long);

        assert_eq!(cut.chars().count(), MAX_VALUE + 1);
        assert!(cut.ends_with('…'));
    }
}
