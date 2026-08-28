use serde_json::Value;

use crate::PlaidRequest;

/// `/sandbox/public_token/create` — a Link session's outcome, without the Link
/// session.
///
/// Sandbox only, and it exists because the alternative during development is a
/// person signing into a fake bank in a browser several times a day. It hands
/// back the same `public_token` a finished Hosted Link session would, so
/// everything downstream — the exchange, the Item, the token, the sync — is
/// the ordinary code path and not a special one.
///
/// **It spends nothing.** The ten-Item cap is Production's, and this endpoint
/// does not exist there at all, so there is no version of this call that can
/// cost a slot.
pub struct SandboxPublicTokenRequest {
    /// Which fake bank. `ins_109508` is First Platypus Bank, which is what the
    /// fixtures were captured from.
    pub institution_id: String,

    /// What the Item is created with. The same single product the real link
    /// asks for — balances and recurring both come off the Item that
    /// `transactions` creates.
    pub initial_products: Vec<String>,
}

impl SandboxPublicTokenRequest {
    pub fn new(institution_id: impl Into<String>) -> Self {
        Self {
            institution_id: institution_id.into(),
            initial_products: vec![String::from(crate::TRANSACTIONS)],
        }
    }
}

impl PlaidRequest for SandboxPublicTokenRequest {
    fn path(&self) -> String {
        String::from("sandbox/public_token/create")
    }

    fn body(&self) -> Value {
        serde_json::json!({
            "institution_id": self.institution_id,
            "initial_products": self.initial_products,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_names_an_institution_and_the_one_product() {
        let body = SandboxPublicTokenRequest::new("ins_109508").body();

        assert_eq!(body["institution_id"], "ins_109508");
        assert_eq!(body["initial_products"][0], crate::TRANSACTIONS);
    }
}
