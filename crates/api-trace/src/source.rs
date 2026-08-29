//! One external API, and how its traffic is logged.
//!
//! The name alone was enough while every body was logged verbatim. It stopped
//! being enough the moment two sources wanted different treatment: Plaid's
//! responses are pretty-printed and its requests carry an account-wide secret,
//! and neither is true of a price feed. Rules that belong to a source travel
//! with it rather than being repeated at each call, so adding one — the next
//! is redaction — is a line in one place and every call it makes is covered.

use crate::scrub::Rule;
use crate::scrub::Scrub;

/// An API we talk to, as the log knows it.
#[derive(Clone, Copy)]
pub struct Source {
    name: &'static str,
    scrub: Scrub,
}

impl Source {
    /// A source whose bodies are logged exactly as they crossed the wire.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            scrub: Scrub::NONE,
        }
    }

    /// The rules its bodies pass through, in order.
    pub const fn through(self, rules: &'static [Rule]) -> Self {
        Self {
            name: self.name,
            scrub: Scrub::new(rules),
        }
    }

    /// What `source=` on the line says.
    pub const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn scrub(self) -> Scrub {
        self.scrub
    }
}
