//! One sync against the Sandbox, printed.
//!
//! ```sh
//! export PLAID_CLIENT_ID=… PLAID_SECRET=…
//! cargo run -p src-plaid --example sync_transactions -- <access-token>
//! ```
//!
//! No `PLAID_ENV`, so this is Sandbox — see `docs/PLAID.md`. Getting an access
//! token for it takes `/sandbox/public_token/create` then
//! `/item/public_token/exchange`, which is the server's job and not written
//! yet; until it is, `curl` produces one.

use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    api_trace::init(api_trace::Config::default());

    let access_token = std::env::args()
        .nth(1)
        .ok_or("usage: sync_transactions <access-token>")?;

    let credentials = PlaidCredentials::from_env()?;
    let client = PlaidClient::new(credentials)?;

    let sync = src_plaid::sync_transactions(&client, &access_token, None).await?;

    println!("\ncursor: {}", sync.cursor);
    println!(
        "{} added, {} removed\n",
        sync.added.len(),
        sync.removed.len()
    );

    // Amounts are Plaid's own: positive means money left the account.
    for tx in &sync.added {
        let category = tx
            .category
            .as_ref()
            .map(|c| c.primary.as_str())
            .unwrap_or("-");

        println!(
            "{:>10.2} {:<24} {:<24} {}",
            tx.amount,
            tx.label(),
            category,
            if tx.pending { "pending" } else { "" },
        );
    }

    Ok(())
}
