//! Signing in's own state: what a code and a session are drawn from, and when
//! the expired sessions were last swept out.
//!
//! A sub-machine rather than fields on [`StateServer`](crate::StateServer):
//! nothing outside signing somebody in reads any of it.

use api_common::Timestamp;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::StoredCode;
use crate::email_address;
use crate::login_code;

/// How long a session lasts. Long, because the alternative is asking somebody
/// to check their email to look at their own balance.
pub const SESSION_GOOD_FOR_SECS: u64 = 90 * 24 * 60 * 60;

/// How often the expired sessions are swept out of the store. Nothing depends
/// on it being prompt — an expired session signs nobody in either way — so
/// this is only about the table not growing.
pub const SWEEP_SESSIONS_EVERY_SECS: u64 = 60 * 60;

/// A code drawn for one address, and when it lapses.
#[derive(Debug)]
pub(crate) struct Minted {
    pub(crate) code: String,
    pub(crate) expires_at: Timestamp,
}

/// What asking for a sign-in code came to.
#[derive(Debug)]
pub(crate) enum Asked {
    /// Drawn.
    Minted(Minted),

    /// Not an address, so nothing was drawn and nothing is sent.
    Implausible,

    /// Nothing to draw from.
    Unavailable,
}

/// What a code somebody typed came to.
#[derive(Debug)]
pub(crate) enum Checked {
    /// In, with the token to hand back and when it lapses.
    In {
        token: String,
        expires_at: Timestamp,
    },

    /// Refused, and the try counts against the address.
    Refused,

    /// Refused, and there is nothing to count: no code stood against this
    /// address, or there was nothing to mint a session from.
    Ignored,
}

/// The generator a code and a session are drawn from, and the sweep that
/// clears the sessions out again.
#[derive(Debug, Default)]
pub struct StateSignIn {
    /// Where sign-in codes and session tokens come from, seeded by
    /// [`EventProcess::Seeded`](crate::EventProcess).
    ///
    /// ChaCha20 rather than a fast PRNG: a generator whose state can be
    /// recovered from its outputs hands an attacker every code after them.
    rng: Option<ChaCha20Rng>,

    /// When the store's expired sessions were last swept. The first tick
    /// starts the clock rather than sweeping, so a restart brings none on.
    swept: Option<Timestamp>,
}

impl StateSignIn {
    /// Take the entropy the process drew.
    pub(crate) fn seed(&mut self, seed: &[u8; 32]) {
        self.rng = Some(ChaCha20Rng::from_seed(*seed));
    }

    /// Mint nothing more. The generator goes rather than a flag beside it: a
    /// credential drawn by a process that is about to exit is one nobody can
    /// use, and there is no way back from here.
    pub(crate) fn stopping(&mut self) {
        self.rng = None;
    }

    /// Draw a code for an address, if it is one.
    pub(crate) fn ask(&mut self, email: &str, now: Timestamp) -> Asked {
        // Answered as though it had been taken: the route says the same thing
        // whether or not an address has an account, and the shape of the
        // string must not say otherwise.
        if !email_address::plausible(email) {
            tracing::warn!("a sign-in was asked for with something that is not an address",);

            return Asked::Implausible;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("asked for a sign-in code by a machine that mints none");

            return Asked::Unavailable;
        };

        Asked::Minted(Minted {
            code: login_code::generate(rng),
            expires_at: Timestamp::from_secs(now.as_secs() + login_code::GOOD_FOR.as_secs()),
        })
    }

    /// Check a code against what the store held. A turn later than the
    /// submission, which is why the machine kept what they typed.
    pub(crate) fn check(
        &mut self,
        email: &str,
        code: &str,
        stored: Option<&StoredCode>,
        now: Timestamp,
    ) -> Checked {
        // No code standing against this address, and a rejection like any
        // other: telling them apart says which addresses have one in flight.
        let Some(stored) = stored else {
            return Checked::Ignored;
        };

        let verdict = login_code::verify(stored, code, now);

        if verdict != login_code::Verdict::Accepted {
            tracing::warn!(email, ?verdict, "a sign-in code was rejected");

            return Checked::Refused;
        }

        let Some(rng) = self.rng.as_mut() else {
            tracing::error!("a code was accepted by a machine that mints none");

            return Checked::Ignored;
        };

        Checked::In {
            token: login_code::generate_token(rng),
            expires_at: Timestamp::from_secs(now.as_secs() + SESSION_GOOD_FOR_SECS),
        }
    }

    /// Whether the store is due to be asked to drop its expired sessions.
    ///
    /// Nothing is owed a sweep at boot: expired rows are already inert, so the
    /// first tick starts the clock instead of sweeping.
    pub(crate) fn sweep_due(&mut self, now: Timestamp) -> bool {
        let Some(swept) = self.swept else {
            self.swept = Some(now);

            return false;
        };

        if now.since(swept).as_secs() < SWEEP_SESSIONS_EVERY_SECS {
            return false;
        }

        self.swept = Some(now);

        true
    }
}
