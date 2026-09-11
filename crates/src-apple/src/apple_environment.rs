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

    /// The environment this build sells in, named by `APPLE_ENV`.
    ///
    /// Ours to spell rather than Apple's, and lower case like `PLAID_ENV`
    /// beside it. `None` is a build that has not said which store it means,
    /// and one that has not said refuses every purchase — there is no
    /// default, because the two failures look identical from the outside and
    /// only one of them is fixed by checking the spelling of a variable
    /// nobody suspects.
    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "sandbox" => Some(AppleEnvironment::Sandbox),
            "production" => Some(AppleEnvironment::Production),
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

    /// The setting is ours and lower case; the payload's word is Apple's and
    /// is not. Reading either with the other's rule is how a build ends up
    /// meaning neither store.
    #[test]
    fn the_setting_and_the_payload_are_spelled_differently() {
        assert_eq!(
            AppleEnvironment::from_setting("sandbox"),
            Some(AppleEnvironment::Sandbox),
        );
        assert_eq!(
            AppleEnvironment::from_setting("production"),
            Some(AppleEnvironment::Production),
        );

        assert_eq!(AppleEnvironment::from_setting("Sandbox"), None);
        assert_eq!(AppleEnvironment::from_name("sandbox"), None);
    }

    /// Nothing said is not production, and it is not sandbox either.
    #[test]
    fn an_unset_setting_names_no_store() {
        for value in ["", "sandbx", "dev", "SANDBOX", "prod", "Xcode"] {
            assert_eq!(
                AppleEnvironment::from_setting(value),
                None,
                "`{value}` must not resolve to an environment",
            );
        }
    }
}
