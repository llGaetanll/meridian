//! Which App Store a payload came from.

/// Sandbox or production, as a payload names its own.
///
/// Deliberately not `Default`, and deliberately without an `Other`. Every
/// other vocabulary Apple owns gets a catch-all because the set is theirs to
/// extend; this one is two words that have never changed, and a third would
/// mean a store nobody has a rule for. Guessing which of the two it is like is
/// how a test purchase becomes a subscription somebody never paid for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleEnvironment {
    /// A sandbox tester's purchase. No money moved.
    Sandbox,

    /// A real purchase in the App Store.
    Production,
}

impl AppleEnvironment {
    /// The environment one of Apple's names means, exactly.
    ///
    /// `None` for anything else, which is a payload this build will not act
    /// on rather than one it treats as production.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Sandbox" => Some(AppleEnvironment::Sandbox),
            "Production" => Some(AppleEnvironment::Production),
            _ => None,
        }
    }

    /// Apple's name for it, for the log line that says which store a purchase
    /// came from.
    pub fn as_str(&self) -> &'static str {
        match self {
            AppleEnvironment::Sandbox => "Sandbox",
            AppleEnvironment::Production => "Production",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_environments_round_trip_through_their_name() {
        for environment in [AppleEnvironment::Sandbox, AppleEnvironment::Production] {
            assert_eq!(
                AppleEnvironment::from_name(environment.as_str()),
                Some(environment),
            );
        }
    }

    /// None of these is production, and the spelling is Apple's: capitalised,
    /// and not what a developer would type.
    #[test]
    fn anything_else_is_no_environment_at_all() {
        for value in ["", "sandbox", "production", "PRODUCTION", "Prod", "Xcode"] {
            assert_eq!(
                AppleEnvironment::from_name(value),
                None,
                "`{value}` must not resolve to an environment",
            );
        }
    }
}
