//! Stripe.js: the card form's frames, and the one call that pays with them.
//!
//! The card is typed into Stripe's frames and goes from them to Stripe, so it
//! never passes through this app or the server. What comes back is Stripe's
//! word for how a payment went, turned here into ours.
//!
//! No key is built in. The server hands out the publishable key beside each
//! subscription's secret, and [`open`] is given it then.

use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlElement;

use crate::io::Payment;

#[wasm_bindgen(module = "/src/io/stripe.js")]
extern "C" {
    #[wasm_bindgen(js_name = open)]
    fn open_stripe(key: &str);

    #[wasm_bindgen(catch, js_name = mount)]
    async fn mount_frames(
        number: &HtmlElement,
        expiry: &HtmlElement,
        cvc: &HtmlElement,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = unmount)]
    fn unmount_frames();

    #[wasm_bindgen(js_name = lock)]
    fn lock_frames(locked: bool);

    #[wasm_bindgen(catch, js_name = confirm)]
    async fn confirm_payment(secret: &str) -> Result<JsValue, JsValue>;
}

/// Take the publishable key a subscription came back with, and start loading
/// Stripe.js, so the frames are ready by the time the form is drawn.
pub fn open(key: &str) {
    open_stripe(key);
}

/// Put Stripe's frames in a card's three wells, once [`open`] has been given a
/// key.
///
/// A form that will not mount is logged and left empty; paying from it then
/// fails, which is the machine's to word.
pub async fn mount(number: &HtmlElement, expiry: &HtmlElement, cvc: &HtmlElement) {
    if let Err(e) = mount_frames(number, expiry, cvc).await {
        tracing::error!(error = ?e, "the card form would not open");
    }
}

/// Take the frames down, as the form leaves the page.
pub fn unmount() {
    unmount_frames();
}

/// Stop the frames taking typing while a payment is out, or let them again.
pub fn lock(locked: bool) {
    lock_frames(locked);
}

/// Pay the invoice `secret` is for, with the card in the frames.
///
/// Where the bank asks for 3-D Secure, Stripe puts its own window over the
/// page and this answers once it has closed.
pub async fn confirm(secret: &str) -> Payment {
    let said = match confirm_payment(secret).await {
        Ok(said) => said.as_string().unwrap_or_default(),
        Err(e) => {
            tracing::error!(error = ?e, "the card form threw");

            return Payment::Failed;
        }
    };

    let Ok(said) = serde_json::from_str::<Said>(&said) else {
        tracing::error!(said, "stripe answered in a shape this build cannot read");

        return Payment::Failed;
    };

    tracing::info!(
        status = ?said.status,
        kind = ?said.kind,
        code = ?said.code,
        decline_code = ?said.decline_code,
        "stripe answered",
    );

    said.payment()
}

/// What `stripe.js` hands back: a payment's status, or the error Stripe
/// raised instead. Codes only — nothing here is the card.
#[derive(Debug, Deserialize)]
struct Said {
    status: Option<String>,

    #[serde(rename = "type")]
    kind: Option<String>,

    code: Option<String>,
    decline_code: Option<String>,
}

impl Said {
    /// Stripe's words for how it went, as ours.
    ///
    /// Classification and not a sentence: what a refused card reads as is the
    /// machine's.
    fn payment(&self) -> Payment {
        // Settled, whatever else was said. A second confirm of a payment that
        // already went through answers with an error carrying this.
        if let Some("succeeded" | "processing") = self.status.as_deref() {
            return Payment::Taken;
        }

        match (self.kind.as_deref(), self.code.as_deref()) {
            (_, Some("payment_intent_authentication_failure")) => Payment::Unauthenticated,

            // Checked before anything was sent: a field incomplete, or a
            // number that cannot be one.
            (Some("validation_error"), _) => Payment::Incorrect,

            (Some("card_error"), Some(code)) if details_wrong(code) => Payment::Incorrect,
            (Some("card_error"), Some("processing_error")) => Payment::Failed,
            (Some("card_error"), _) => Payment::Declined,

            // Answered without an error, and not paid: the card went and was
            // not taken.
            (None, None) if self.status.as_deref() == Some("requires_payment_method") => {
                Payment::Declined
            }

            _ => Payment::Failed,
        }
    }
}

/// The card errors that are a detail typed wrong rather than a bank saying no.
fn details_wrong(code: &str) -> bool {
    matches!(
        code,
        "incorrect_number"
            | "invalid_number"
            | "incomplete_number"
            | "incorrect_cvc"
            | "invalid_cvc"
            | "incomplete_cvc"
            | "invalid_expiry_month"
            | "invalid_expiry_year"
            | "invalid_expiry_year_past"
            | "incomplete_expiry"
            | "incorrect_zip"
    )
}
