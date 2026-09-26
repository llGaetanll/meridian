//! A Sandbox brokerage's holdings and trades, printed.
//!
//! Creates its own Item — `transactions` plus `investments` — so it needs no
//! setup beyond credentials, which come from `.env` at the workspace root.
//!
//! ```sh
//! cargo run -p src-plaid --example investments
//! cargo run -p src-plaid --example investments -- ins_109509
//! cargo run -p src-plaid --example investments -- ins_109508 <access-token>
//! ```
//!
//! With an access token it skips the creation and asks that Item instead.
//! Sandbox only, and it checks: the Item this creates is throwaway, and ten
//! of those against Production would end the account.

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use api_common::Timestamp;
use src_plaid::PRODUCT_NOT_READY;
use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidEnv;
use src_plaid::PlaidError;

const DAY: u64 = 86_400;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    api_trace::init(api_trace::Config::default());

    let credentials = PlaidCredentials::from_env()?;
    let client = PlaidClient::new(credentials)?;

    if client.env() != PlaidEnv::Sandbox {
        return Err("this example creates a throwaway Item, which Production cannot afford".into());
    }

    let institution_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("ins_109508"));

    let access_token = match std::env::args().nth(2) {
        Some(token) => token,
        None => {
            let created = src_plaid::create_sandbox_brokerage(&client, &institution_id).await?;

            src_plaid::exchange_public_token(&client, &created.public_token)
                .await?
                .access_token
        }
    };

    let holdings = with_patience(|| src_plaid::fetch_holdings(&client, &access_token)).await?;

    println!("\n{} accounts:", holdings.accounts.len());
    for account in &holdings.accounts {
        println!(
            "  {} ({} / {})",
            account.name,
            account.kind,
            account.subtype.as_deref().unwrap_or("-"),
        );
    }

    let name_of = |id: Option<&str>, securities: &[src_plaid::PlaidSecurity]| {
        id.and_then(|id| securities.iter().find(|s| s.security_id == id))
            .map(|s| s.label().to_string())
            .unwrap_or_else(|| String::from("-"))
    };

    println!("\n{} holdings:", holdings.holdings.len());
    for holding in &holdings.holdings {
        println!(
            "  {:<8} {:>12.4} × {:>10.2} = {:>12.2} {} (cost {})",
            name_of(Some(&holding.security_id), &holdings.securities),
            holding.quantity,
            holding.institution_price,
            holding.institution_value,
            holding.iso_currency_code.as_deref().unwrap_or("?"),
            holding
                .cost_basis
                .map(|c| format!("{c:.2}"))
                .unwrap_or_else(|| String::from("-")),
        );
    }

    let now = Timestamp::from_secs(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    );
    let window = now.saturating_sub_secs(730 * DAY)..now;

    let trades =
        with_patience(|| src_plaid::fetch_investments(&client, &access_token, window.clone(), 0))
            .await?;

    println!(
        "\n{} of {} investment transactions:",
        trades.investment_transactions.len(),
        trades.total_investment_transactions,
    );
    for tx in &trades.investment_transactions {
        println!(
            "  {:<10} {:<12} {:<8} {:>10.3} × {:>9.2} = {:>10.2} {} fees {}",
            api_common::format::fmt_date(tx.date),
            format!("{}/{}", tx.kind, tx.subtype.as_deref().unwrap_or("-")),
            name_of(tx.security_id.as_deref(), &trades.securities),
            tx.quantity,
            tx.price,
            tx.amount,
            tx.iso_currency_code.as_deref().unwrap_or("?"),
            tx.fees
                .map(|f| format!("{f:.2}"))
                .unwrap_or_else(|| String::from("-")),
        );
    }

    Ok(())
}

/// A fresh Item answers `PRODUCT_NOT_READY` while Plaid generates its data,
/// which in Sandbox is seconds. Asking again is the documented cure, so ask
/// again, for a while.
async fn with_patience<T, F, Fut>(mut call: F) -> Result<T, PlaidError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, PlaidError>>,
{
    const TRIES: u32 = 10;

    for waited in 1..TRIES {
        match call().await {
            Err(PlaidError::Plaid { error_code, .. }) if error_code == PRODUCT_NOT_READY => {
                eprintln!("not ready, asking again ({waited}/{TRIES})");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
            done => return done,
        }
    }

    call().await
}
