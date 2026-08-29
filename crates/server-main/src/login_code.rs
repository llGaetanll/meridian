//! The six digits, and what to make of one somebody typed.
//!
//! The decision — accepted, wrong, expired, out of attempts — is a pure
//! function of what is stored and what arrived, so it is here and tested
//! directly rather than through a database. It is deliberately *not* in
//! `StateServer`: the machine holds what survives between requests, and a
//! pending code lives in Postgres because it has to survive a restart and be
//! visible to a second server process.

use api_common::Timestamp;
use rand::Rng;
use sha2::Digest;
use sha2::Sha256;

/// How long a code is good for. Long enough to switch to a mail app and back,
/// short enough that a code read over somebody's shoulder is stale by the time
/// it is useful.
pub const GOOD_FOR: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Wrong guesses before the code is dead.
///
/// Six digits is a million possibilities, so five guesses is not the thing
/// standing between an attacker and an account — the expiry is. This is here
/// so that a *stolen* code cannot be brute-forced in the ten minutes it lives.
pub const MAX_ATTEMPTS: i32 = 5;

/// What the code screen should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Sign them in, and delete the code — it is single use.
    Accepted,

    /// Wrong, and there are guesses left.
    Wrong,

    /// Right ten minutes ago. Asks for a new code rather than another guess,
    /// which is a different screen and a different sentence.
    Expired,

    /// Out of guesses. The code is dead whether or not the next one is right.
    Exhausted,
}

/// A code as it is stored: the hash, when it dies, and how many guesses it has
/// taken.
#[derive(Debug, Clone)]
pub struct StoredCode {
    pub code_sha256: Vec<u8>,
    pub expires_at: Timestamp,
    pub attempts: i32,
}

/// Six digits, drawn from the generator handed in.
///
/// The generator is the machine's, seeded from the edge, which is what makes a
/// replay of the event log produce the same code. Its unpredictability comes
/// from the seed being 32 bytes of OS entropy and the generator being
/// ChaCha20 — not from being unreproducible.
///
/// Formatted to a fixed width, so `000123` is a code and not the number 123.
pub fn generate(rng: &mut impl Rng) -> String {
    format!("{:06}", rng.random_range(0..1_000_000))
}

/// A session token: 32 random bytes as hex, from the same generator.
///
/// Longer than a code and not guessable by brute force, because unlike a code
/// it has no expiry measured in minutes and no attempt cap behind it — it is
/// the credential itself.
pub fn generate_token(rng: &mut impl Rng) -> String {
    let bytes: [u8; 32] = rng.random();

    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The hash a code is stored as.
///
/// Plain SHA-256 rather than a password hash, and on purpose. The input is six
/// digits from a CSPRNG with a ten-minute life and five guesses — the whole
/// keyspace is a million, so a slow hash buys nothing an attacker with the
/// database could not walk through anyway. What this defends against is a
/// leaked database being a set of live logins, and any preimage-resistant hash
/// does that.
pub fn hash(code: &str) -> Vec<u8> {
    Sha256::digest(code.as_bytes()).to_vec()
}

/// Whether this is the code we sent.
///
/// Pure, and the reason this module exists. `now` is an argument because
/// nothing that decides may read a clock — the same rule the state machines
/// follow, and what makes "was it expired?" a test rather than a stopwatch.
pub fn verify(stored: &StoredCode, submitted: &str, now: Timestamp) -> Verdict {
    // Checked before the digits: a dead code is dead whatever was typed, and
    // answering "wrong" to a right-but-exhausted code would say that the guess
    // was the problem.
    if stored.attempts >= MAX_ATTEMPTS {
        return Verdict::Exhausted;
    }

    if now >= stored.expires_at {
        return Verdict::Expired;
    }

    if constant_time_eq(&hash(submitted), &stored.code_sha256) {
        Verdict::Accepted
    } else {
        Verdict::Wrong
    }
}

/// Compare without leaking where the two differ.
///
/// A `==` on the hashes returns as soon as a byte differs, and the time that
/// takes is a measurement of how much of the code was right. Both inputs here
/// are SHA-256 outputs of the same length, so this only ever runs the full 32
/// bytes — the length check is for the caller who one day passes something
/// else.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

    fn stored(code: &str, attempts: i32, expires_in: u64) -> StoredCode {
        StoredCode {
            code_sha256: hash(code),
            expires_at: Timestamp::from_secs(NOW.as_secs() + expires_in),
            attempts,
        }
    }

    #[test]
    fn the_right_code_is_accepted() {
        assert_eq!(
            verify(&stored("123456", 0, 60), "123456", NOW),
            Verdict::Accepted
        );
    }

    #[test]
    fn the_wrong_code_is_not() {
        assert_eq!(
            verify(&stored("123456", 0, 60), "654321", NOW),
            Verdict::Wrong
        );
    }

    /// Expiry is checked against the clock handed in, which is what makes this
    /// a test rather than a wait.
    #[test]
    fn a_code_dies_at_its_expiry() {
        let code = stored("123456", 0, 60);

        assert_eq!(verify(&code, "123456", NOW), Verdict::Accepted);

        let later = Timestamp::from_secs(NOW.as_secs() + 60);

        assert_eq!(verify(&code, "123456", later), Verdict::Expired);
    }

    /// Out of guesses outranks a right answer, or the cap would be advice.
    #[test]
    fn an_exhausted_code_rejects_the_right_answer() {
        assert_eq!(
            verify(&stored("123456", MAX_ATTEMPTS, 60), "123456", NOW),
            Verdict::Exhausted,
        );
    }

    /// Six digits, always, including the ones with leading zeroes. A code
    /// formatted as a bare number would be five characters one time in ten and
    /// would not match a field that expects six.
    #[test]
    fn a_generated_code_is_always_six_digits() {
        use rand::SeedableRng;

        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(1);

        for _ in 0..1_000 {
            let code = generate(&mut rng);

            assert_eq!(code.len(), 6, "{code}");
            assert!(code.chars().all(|c| c.is_ascii_digit()), "{code}");
        }
    }

    /// The whole point of the generator living in the machine: the same seed
    /// gives the same codes, so an event log replays to the values it produced
    /// the first time.
    #[test]
    fn the_same_seed_gives_the_same_codes() {
        use rand::SeedableRng;

        let codes = |seed| {
            let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
            (0..5).map(|_| generate(&mut rng)).collect::<Vec<_>>()
        };

        assert_eq!(codes(7), codes(7));
        assert_ne!(codes(7), codes(8));
    }

    #[test]
    fn the_hash_is_not_the_code() {
        assert_ne!(hash("123456"), b"123456".to_vec());
        assert_eq!(hash("123456"), hash("123456"));
        assert_ne!(hash("123456"), hash("123457"));
    }
}
