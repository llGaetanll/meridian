//! Where an OAuth bank hands control back to the app.

use src_plaid::PlaidError;

/// The address the bank's own app opens to return the user to ours.
///
/// The one variable this server reads that is optional, and the reason is that
/// its absence is a real state rather than a missing answer: until the app can
/// claim a Universal Link, there is nowhere for a bank to hand back to, and
/// saying so is safer than naming an address that resolves to nobody. Unset,
/// the link token is the one a browser gets, and an OAuth bank stays inside
/// the web session.
///
/// Set, it goes on the link token as the top-level `redirect_uri` and carries
/// `is_mobile_app` with it — see [`LinkTokenRequest`](src_plaid::LinkTokenRequest)
/// for why those two are one decision.
#[derive(Debug, Clone)]
pub struct OauthRedirect(String);

impl OauthRedirect {
    /// The address named by `PLAID_REDIRECT_URI`, or `None` where it is unset.
    ///
    /// Empty counts as unset. A variable that is exported and blank is how a
    /// `.env` with the line still in it arrives, and reading that as an
    /// address gives Plaid an empty string.
    pub fn from_env() -> Result<Option<Self>, PlaidError> {
        match std::env::var("PLAID_REDIRECT_URI") {
            Ok(value) if value.trim().is_empty() => Ok(None),
            Ok(value) => Self::parse(&value).map(Some),
            Err(_) => Ok(None),
        }
    }

    /// The address a string names, checked on the way up.
    ///
    /// Separate from [`from_env`](OauthRedirect::from_env) so the parse is
    /// testable without the process environment, which every test in the
    /// binary shares.
    pub fn parse(value: &str) -> Result<Self, PlaidError> {
        let value = value.trim();

        let refuse = |why: &str| {
            Err(PlaidError::Config(format!(
                "PLAID_REDIRECT_URI {why}: `{value}`"
            )))
        };

        // The mistake this exists to catch, and it has to be named rather than
        // covered by the https check below: `meridian://linked` is already in
        // the codebase, already works for the redirect that closes the sheet,
        // and is the obvious thing to paste in here. iOS routes a custom
        // scheme to whoever registered it first, so a bank handing back to one
        // is a bank handing a signed-in session to whatever else claimed it.
        if !value.starts_with("https://") && value.contains("://") {
            return refuse(
                "must be an https Universal Link — a custom scheme cannot receive an OAuth handback",
            );
        }

        let Some(host) = value.strip_prefix("https://") else {
            return refuse("must start with https://");
        };

        if host.is_empty() || host.starts_with('/') {
            return refuse("names no host");
        }

        if value.contains(char::is_whitespace) {
            return refuse("has whitespace in it");
        }

        // Plaid matches a registered redirect exactly, so a query or a
        // fragment here is a link token refused at create time rather than a
        // handback that goes astray.
        if value.contains('?') || value.contains('#') {
            return refuse("must be a bare address, with no query or fragment");
        }

        Ok(Self(value.to_string()))
    }

    /// The address, as it goes on the link token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_universal_link_is_kept_as_written() {
        let redirect = OauthRedirect::parse("https://api.meridiancash.io/link/oauth").unwrap();

        assert_eq!(redirect.as_str(), "https://api.meridiancash.io/link/oauth");
    }

    /// The one that would be accepted by every other check here, and is the
    /// whole reason a bank cannot find its way back.
    #[test]
    fn the_scheme_that_closes_the_sheet_is_not_an_address_a_bank_can_use() {
        assert!(OauthRedirect::parse("meridian://linked").is_err());
    }

    /// Each of these is a link token Plaid refuses, or a handback that reaches
    /// nobody — both after the user has already signed in at their bank.
    #[test]
    fn anything_a_bank_cannot_hand_back_to_is_refused() {
        for value in [
            "api.meridiancash.io/link/oauth",
            "http://api.meridiancash.io/link/oauth",
            "https://",
            "https:///link/oauth",
            "https://api.meridiancash.io/link/oauth?state=1",
            "https://api.meridiancash.io/link/oauth#done",
            "https://api meridiancash.io/link/oauth",
        ] {
            assert!(
                OauthRedirect::parse(value).is_err(),
                "`{value}` must not be accepted as a handback address",
            );
        }
    }
}
