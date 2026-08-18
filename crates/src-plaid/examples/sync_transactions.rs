//! One sync against the Sandbox, printed.
//!
//! Credentials come from `.env` at the workspace root — copy `.env.example`
//! and fill it in.
//!
//! ```sh
//! cargo run -p src-plaid --example sync_transactions
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
    // Loads `.env` from here upward, so the workspace root's is found from
    // any directory. Absent is fine: a real environment already set wins, and
    // `dotenvy` never overwrites a variable that exists.
    let _ = dotenvy::dotenv();

    api_trace::init(api_trace::Config::default());

    // An argument beats the file, so a second Item can be synced without
    // editing anything.
    let access_token = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("PLAID_ACCESS_TOKEN").ok())
        .ok_or("no access token: set PLAID_ACCESS_TOKEN in .env, or pass one as an argument")?;

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
