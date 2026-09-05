//! Rules a body passes through on its way to the log.
//!
//! A body is logged as it crossed the network, which is the only version that
//! can answer "what did we actually send". But *as it crossed* and *as it is
//! worth reading* are not the same string: Plaid pretty-prints its responses,
//! and 400 characters of four-space indentation says almost nothing where 400
//! characters of compact JSON says most of what came back. The same seam is
//! where a credential has to stop, and a rule that drops a field is the same
//! shape as one that drops a space.
//!
//! So: a body goes through an ordered list of rules, each one a function of
//! the body and what is known about the call. Rules compose because they are
//! ordered and each returns a body, and a source picks the ones that apply to
//! it — see [`crate::Source`].
//!
//! A rule may not change what a body *means*. Reformatting and redaction both
//! keep that promise: the first says the same thing in less space, the second
//! leaves the field where it was and says plainly that its value was removed.
//! A rule that quietly rewrote a value would leave a log that is worse than no
//! log, because it would still be believed.
//!
//! # None of this reaches the code that reads the body
//!
//! Rules run on the way to the log and nowhere else. A caller hands a body in
//! by reference and never gets one back — [`crate::ApiCall::post`] returns the
//! call, and `response` returns a handle carrying an id and a source — so the
//! string that gets parsed is the one that came off the wire, untouched. That
//! is not a convention to be careful about: [`Scrub`] cannot be run from
//! outside this crate, so there is no way to route a real body through a rule
//! by accident. Reformatting would only cost us a confusing diff if it leaked;
//! [`redact_credentials`] would cost us a request sent to Plaid with
//! `[redacted 4]` where the secret belonged.

use std::borrow::Cow;

/// Which half of the pair a rule is looking at.
///
/// The two are not symmetrical — we choose what a request says and only
/// receive what a response says — so a rule that belongs on one is usually
/// wrong on the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Going out. `->` on the line.
    Request,
    /// Coming back. `<-` on the line.
    Response,
}

/// What a rule is told about the body it is handed.
///
/// Deliberately the same few facts that are already on the line, so a rule can
/// never key off something the reader of the log cannot see.
pub struct Context<'a> {
    pub source: &'static str,
    pub direction: Direction,
    /// The endpoint, for a rule that applies to one and not the rest.
    pub url: &'a str,
}

/// One rewrite of a body.
///
/// [`Cow`] in and out so that a rule with nothing to do costs nothing: the
/// common case is a body that is already compact and holds no secret, and it
/// should reach the log without being copied.
pub type Rule = for<'a> fn(&Context<'_>, Cow<'a, str>) -> Cow<'a, str>;

/// An ordered list of [`Rule`]s. The body comes out of one and goes into the
/// next, so order is the composition.
#[derive(Clone, Copy)]
pub struct Scrub(&'static [Rule]);

impl Scrub {
    /// Log the body exactly as it crossed the wire.
    pub(crate) const NONE: Self = Self(&[]);

    pub(crate) const fn new(rules: &'static [Rule]) -> Self {
        Self(rules)
    }

    /// Crate-private, and the reason the invariant above is structural rather
    /// than a rule to remember: the only caller is the logging path.
    pub(crate) fn apply<'a>(self, ctx: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
        self.0
            .iter()
            .copied()
            .fold(body, |body, rule| rule(ctx, body))
    }
}

/// Squeeze the whitespace out of a JSON body, leaving strings untouched.
///
/// Not a parse and re-serialise: a body worth logging is often one that does
/// not parse — truncated, or an error in a shape we did not expect — and that
/// is exactly when we need to read it. This walks the text instead, so it
/// works on anything JSON-shaped and cannot fail.
pub fn compact_json<'a>(_ctx: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
    if !looks_like_json(&body) {
        return body;
    }

    let mut out = String::with_capacity(body.len());
    let mut in_string = false;
    let mut escaped = false;

    for c in body.chars() {
        if in_string {
            out.push(c);

            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }

            continue;
        }

        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            // Only outside a string. A space in a merchant name is part of the
            // name, and a `\n` inside a JSON string is two characters that
            // this loop never sees as one.
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }

    // Nothing was dropped, so the original is the same string and does not
    // need to be replaced by a copy of itself.
    if out.len() == body.len() {
        return body;
    }

    Cow::Owned(out)
}

/// Whether this is worth treating as JSON at all.
///
/// A 500 from a proxy is an HTML page, and a rate limiter sometimes answers in
/// plain prose. Squeezing the spaces out of either makes a bad body harder to
/// read at the moment it matters most.
fn looks_like_json(body: &str) -> bool {
    matches!(body.trim_start().as_bytes().first(), Some(b'{' | b'['))
}

/// Field names whose value is a credential and never reaches a line.
///
/// Plaid's vocabulary, because Plaid is the only source that puts a credential
/// in a body at all — a price feed authenticates with nothing, and Resend with
/// a header. A source added later brings its names here rather than its own
/// rule: one list is the only version of this anybody has to keep true.
const CREDENTIALS: &[&str] = &[
    // On every request. `secret` is account-wide: it reads every Item on the
    // account, not one user's.
    "client_id",
    "secret",
    // A bank, permanently. Never expires and is never handed to a client.
    "access_token",
    // Thirty minutes, and exchangeable for the one above.
    "public_token",
    // Opens a Link session belonging to one of our users. `hosted_link_url` is
    // the page that carries it, so the URL is the token.
    "link_token",
    "hosted_link_url",
];

/// Replace the value of every credential-shaped field with its length.
///
/// The field names stay, and that is the point: logging a request body is for
/// answering "what did we actually send", and a field we did not intend to
/// send is visible from its name alone. What the value *was* has never been
/// part of that question.
///
/// Not a parse and re-serialise, for [`compact_json`]'s reason and one of its
/// own: a body that does not parse is exactly when a rule that gives up would
/// hand the log a secret. This walks the text, and anything it cannot read to
/// the end of — a response cut mid-flight — it redacts to the end of the body.
pub fn redact_credentials<'a>(_ctx: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
    let bytes = body.as_bytes();
    let mut out: Option<String> = None;
    let mut kept = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }

        // Consuming whole strings is what keeps a merchant called `secret`
        // from being read as a field name.
        let Some(close) = string_end(bytes, i) else {
            break;
        };

        let name = &bytes[i + 1..close];
        let colon = skip_whitespace(bytes, close + 1);

        // A `:` after it is what makes a string a key. Nothing else in JSON is
        // followed by one, so this needs no notion of where we are.
        if bytes.get(colon) != Some(&b':') || !CREDENTIALS.iter().any(|c| c.as_bytes() == name) {
            i = close + 1;
            continue;
        }

        let start = skip_whitespace(bytes, colon + 1);
        let end = value_end(bytes, start);

        let out = out.get_or_insert_with(|| String::with_capacity(body.len()));
        out.push_str(&body[kept..start]);
        out.push_str(&redaction(&body[start..end]));

        kept = end;
        i = end;
    }

    match out {
        None => body,
        Some(mut out) => {
            out.push_str(&body[kept..]);
            Cow::Owned(out)
        }
    }
}

/// What a credential leaves behind.
///
/// The length stays because it is not a secret and it is the one thing that
/// tells "we sent the wrong secret" apart from "we sent an empty one" — a
/// misconfiguration no other line in the log would show. Quoted, so the body
/// is still JSON and still says plainly that something was taken out.
fn redaction(value: &str) -> String {
    // The quotes belong to the JSON, not to the credential. Stripped in two
    // steps so a value cut mid-string still counts what it actually holds.
    let value = value.strip_prefix('"').unwrap_or(value);
    let value = value.strip_suffix('"').unwrap_or(value);

    format!("\"[redacted {}]\"", value.chars().count())
}

/// The index of the closing quote of the string opening at `open`.
///
/// `None` when the body ends inside it.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;

    while i < bytes.len() {
        match bytes[i] {
            // A `\"` is two characters and neither of them ends the string.
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }

    None
}

fn skip_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b) if b.is_ascii_whitespace()) {
        i += 1;
    }

    i
}

/// One past the end of the JSON value starting at `start`.
///
/// The end of the body when the value has no end, which is what makes a
/// truncated response fail closed: everything from the credential onwards goes
/// rather than everything up to where the reading stopped.
fn value_end(bytes: &[u8], start: usize) -> usize {
    match bytes.get(start) {
        None => bytes.len(),
        Some(b'"') => match string_end(bytes, start) {
            Some(close) => close + 1,
            None => bytes.len(),
        },
        Some(b'{' | b'[') => nested_end(bytes, start),
        // `null`, a number, `true`. Not a credential in any body we send or
        // receive, and redacted all the same: a rule that decided what to hide
        // by reading the value would leak the day it read one wrong.
        Some(_) => {
            let mut i = start;

            while !matches!(bytes.get(i), None | Some(b',' | b'}' | b']')) {
                i += 1;
            }

            // Whitespace before the comma is the body's formatting, not part
            // of the value, and putting it inside the marker reads as if it
            // were.
            while i > start && matches!(bytes.get(i - 1), Some(b) if b.is_ascii_whitespace()) {
                i -= 1;
            }

            i
        }
    }
}

/// One past the `}` or `]` closing the object or array opening at `start`.
fn nested_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0;
    let mut i = start;

    while i < bytes.len() {
        match bytes[i] {
            // Skipped whole, so a brace inside a string does not count.
            b'"' => match string_end(bytes, i) {
                Some(close) => i = close,
                None => return bytes.len(),
            },
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth <= 1 => return i + 1,
            b'}' | b']' => depth -= 1,
            _ => {}
        }

        i += 1;
    }

    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context<'static> {
        Context {
            source: "test",
            direction: Direction::Response,
            url: "https://example.test/thing",
        }
    }

    fn compact(body: &str) -> String {
        compact_json(&ctx(), Cow::Borrowed(body)).into_owned()
    }

    #[test]
    fn squeezes_between_tokens() {
        let pretty = "{\n    \"added\": [\n        1,\n        2\n    ]\n}";

        assert_eq!(compact(pretty), "{\"added\":[1,2]}");
    }

    #[test]
    fn leaves_spaces_inside_strings_alone() {
        assert_eq!(
            compact("{ \"name\": \"Plaid  Checking\" }"),
            "{\"name\":\"Plaid  Checking\"}"
        );
    }

    #[test]
    fn is_not_fooled_by_an_escaped_quote() {
        assert_eq!(
            compact(r#"{ "note": "say \" then stop", "n": 1 }"#),
            r#"{"note":"say \" then stop","n":1}"#
        );
    }

    #[test]
    fn keeps_an_escaped_backslash_from_eating_the_quote() {
        assert_eq!(
            compact(r#"{ "path": "C:\\", "n": 1 }"#),
            r#"{"path":"C:\\","n":1}"#
        );
    }

    #[test]
    fn leaves_a_body_that_is_not_json_where_it_is() {
        let prose = "too many requests, try again later";

        assert_eq!(compact(prose), prose);
    }

    #[test]
    fn borrows_a_body_it_has_nothing_to_do_to() {
        let already = r#"{"a":1}"#;

        let out = compact_json(&ctx(), Cow::Borrowed(already));

        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn runs_rules_in_order() {
        fn first<'a>(_: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
            Cow::Owned(format!("{body}a"))
        }

        fn second<'a>(_: &Context<'_>, body: Cow<'a, str>) -> Cow<'a, str> {
            Cow::Owned(format!("{body}b"))
        }

        let scrub = Scrub::new(&[first, second]);

        assert_eq!(scrub.apply(&ctx(), Cow::Borrowed("x")), "xab");
    }

    fn redact(body: &str) -> String {
        redact_credentials(&ctx(), Cow::Borrowed(body)).into_owned()
    }

    #[test]
    fn takes_the_secret_out_of_a_plaid_request() {
        let sent = r#"{"access_token":"access-sandbox-1","client_id":"6xxxxxxxxx","cursor":"CAESJTM3","secret":"xxxx"}"#;

        assert_eq!(
            redact(sent),
            r#"{"access_token":"[redacted 16]","client_id":"[redacted 10]","cursor":"CAESJTM3","secret":"[redacted 4]"}"#
        );
    }

    #[test]
    fn takes_the_access_token_out_of_an_exchange_response() {
        assert_eq!(
            redact(r#"{"access_token":"access-sandbox-1","item_id":"8Pd5Zx"}"#),
            r#"{"access_token":"[redacted 16]","item_id":"8Pd5Zx"}"#
        );
    }

    #[test]
    fn reaches_a_token_nested_in_a_link_session() {
        let answered = r#"{"link_sessions":[{"results":{"item_add_results":[{"public_token":"public-sandbox-1"}]}}]}"#;

        assert_eq!(
            redact(answered),
            r#"{"link_sessions":[{"results":{"item_add_results":[{"public_token":"[redacted 16]"}]}}]}"#
        );
    }

    #[test]
    fn reads_a_key_through_the_whitespace_plaid_sends() {
        assert_eq!(
            redact("{\n    \"secret\" : \"xxxx\"\n}"),
            "{\n    \"secret\" : \"[redacted 4]\"\n}"
        );
    }

    #[test]
    fn a_value_that_reads_like_a_key_is_still_a_value() {
        assert_eq!(
            redact(r#"{"name":"secret","secret":"xxxx"}"#),
            r#"{"name":"secret","secret":"[redacted 4]"}"#
        );
    }

    #[test]
    fn a_secret_the_body_stops_in_the_middle_of_goes_whole() {
        // A response cut mid-flight. There is no closing quote to stop at, so
        // the rule takes everything rather than what it managed to read.
        assert_eq!(redact(r#"{"secret":"xxxx"#), r#"{"secret":"[redacted 4]""#);
    }

    #[test]
    fn a_credential_that_is_not_a_string_goes_too() {
        assert_eq!(
            redact(r#"{"access_token":null,"item_id":"8Pd5Zx"}"#),
            r#"{"access_token":"[redacted 4]","item_id":"8Pd5Zx"}"#
        );
    }

    #[test]
    fn an_escaped_quote_does_not_end_the_secret_early() {
        assert_eq!(
            redact(r#"{"secret":"a\"b","n":1}"#),
            r#"{"secret":"[redacted 4]","n":1}"#
        );
    }

    #[test]
    fn leaves_a_body_with_nothing_to_hide_where_it_is() {
        let clean = r#"{"added":[],"cursor":"CAESJTM3"}"#;

        let out = redact_credentials(&ctx(), Cow::Borrowed(clean));

        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn plaids_rules_redact_before_they_reformat() {
        let scrub = Scrub::new(&[redact_credentials, compact_json]);
        let pretty = "{\n    \"secret\": \"xxxx\"\n}";

        assert_eq!(
            scrub.apply(&ctx(), Cow::Borrowed(pretty)),
            r#"{"secret":"[redacted 4]"}"#
        );
    }

    #[test]
    fn no_rules_is_the_body_untouched() {
        let out = Scrub::NONE.apply(&ctx(), Cow::Borrowed("{ \"a\": 1 }"));

        assert!(matches!(out, Cow::Borrowed(_)));
    }
}
