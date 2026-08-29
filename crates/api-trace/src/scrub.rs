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
//! says plainly that something was removed. A rule that quietly rewrote a
//! value would leave a log that is worse than no log, because it would still
//! be believed.
//!
//! # None of this reaches the code that reads the body
//!
//! Rules run on the way to the log and nowhere else. A caller hands a body in
//! by reference and never gets one back — [`crate::ApiCall::post`] returns the
//! call, and `response` returns a handle carrying an id and a source — so the
//! string that gets parsed is the one that came off the wire, untouched. That
//! is not a convention to be careful about: [`Scrub`] cannot be run from
//! outside this crate, so there is no way to route a real body through a rule
//! by accident. It matters more with every rule we add. Reformatting only
//! costs us a confusing diff if it leaks; redaction, which is next, would cost
//! us a request sent with the word `REDACTED` where a secret belonged.

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

    #[test]
    fn no_rules_is_the_body_untouched() {
        let out = Scrub::NONE.apply(&ctx(), Cow::Borrowed("{ \"a\": 1 }"));

        assert!(matches!(out, Cow::Borrowed(_)));
    }
}
