//! Stripe, which is how a subscription is paid for on the web and the desktop.
//!
//! The same shape as the other `src-*` crates: plain async functions over
//! plain data, one request each, no decisions. Whether somebody may have a
//! subscription, what a status means for their entitlement, and what to do
//! about a failed payment all belong to whoever calls this.
//!
//! The phone is not here. iOS sells through StoreKit, and what reaches the
//! server from a phone is Apple's word for a purchase rather than a card.

mod stripe_client;
mod stripe_env;
mod stripe_error;
mod stripe_rejection;
mod stripe_request_checkout;
mod stripe_request_customer;
mod stripe_request_subscription;
mod stripe_response_checkout;
mod stripe_response_customer;
mod stripe_response_subscription;
mod stripe_status;
mod stripe_webhook;
mod stripe_webhook_event;

pub use stripe_client::API_VERSION;
pub use stripe_client::StripeClient;
pub use stripe_client::StripeCredentials;
pub use stripe_env::StripeEnv;
pub use stripe_error::StripeError;
pub use stripe_request_checkout::create_checkout_session;
pub use stripe_request_customer::create_customer;
pub use stripe_request_subscription::cancel_subscription;
pub use stripe_request_subscription::create_subscription;
pub use stripe_request_subscription::get_subscription;
pub use stripe_response_checkout::StripeCheckoutSession;
pub use stripe_response_customer::StripeCustomer;
pub use stripe_response_subscription::StripeConfirmationSecret;
pub use stripe_response_subscription::StripeExpandable;
pub use stripe_response_subscription::StripeInvoice;
pub use stripe_response_subscription::StripeSubscription;
pub use stripe_response_subscription::StripeSubscriptionItem;
pub use stripe_response_subscription::StripeSubscriptionItems;
pub use stripe_status::StripeStatus;
pub use stripe_webhook::StripeSignature;
pub use stripe_webhook::StripeSignatureError;
pub use stripe_webhook::TOLERANCE_SECS;
pub use stripe_webhook::verify;
pub use stripe_webhook_event::StripeWebhookData;
pub use stripe_webhook_event::StripeWebhookEvent;
pub use stripe_webhook_event::StripeWebhookObject;
