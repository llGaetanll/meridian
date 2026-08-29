//! Resend, which is how an email leaves this app.
//!
//! The same shape as the other `src-*` crates: plain async functions over
//! plain data, no decisions. Whether to send at all, and what the message
//! says, belong to whoever calls this.

mod resend_client;
mod resend_error;
mod resend_request;

pub use resend_client::ResendClient;
pub use resend_client::ResendCredentials;
pub use resend_error::ResendError;
pub use resend_request::Email;

/// Send one message. Answers with Resend's id for it, which is the only thing
/// that ties a line in our log to a row in their dashboard.
pub async fn send(client: &ResendClient, email: &Email) -> Result<String, ResendError> {
    let sent = client.send(email).await?;

    tracing::info!(id = sent.id, to = email.to, "email sent");

    Ok(sent.id)
}
