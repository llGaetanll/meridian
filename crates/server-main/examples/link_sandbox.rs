//! Link a Sandbox bank and write the Item to the store.
//!
//! ```sh
//! cargo run -p server-main --example link_sandbox -- alex@example.com
//! cargo run -p server-main --example link_sandbox -- alex@example.com ins_109509
//! ```
//!
//! It makes the two calls the real flow ends with — exchange a
//! `public_token`, store what comes back — and skips the browser sign-in in
//! the middle. It cannot spend an Item slot: `/sandbox/public_token/create`
//! does not exist in Production.

use src_plaid::PlaidClient;
use src_plaid::PlaidCredentials;
use src_plaid::PlaidEnv;

/// First Platypus Bank, which the fixtures were captured from.
const DEFAULT_INSTITUTION: &str = "ins_109508";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    api_trace::init(api_trace::Config {
        quiet: &["sqlx::postgres::notice"],
        ..api_trace::Config::default()
    });

    // Not defaulted: a guess puts a bank on the wrong account.
    let email = std::env::args()
        .nth(1)
        .ok_or("usage: link_sandbox <email> [institution_id]")?;

    let institution_id = std::env::args()
        .nth(2)
        .unwrap_or_else(|| DEFAULT_INSTITUTION.to_string());

    let credentials = PlaidCredentials::from_env()?;

    // The endpoint below exists only in Sandbox, and a clear refusal beats
    // Plaid's 400 explaining that it is not a route.
    if credentials.env != PlaidEnv::Sandbox {
        return Err("PLAID_ENV must be sandbox: this creates an Item without a person".into());
    }

    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;

    let db = src_postgres::connect(&url).await?;
    src_postgres::migrate(&db).await?;

    // Made if absent, so this works before anybody has asked for a code.
    let user_id = src_postgres::upsert_user(&db, &email).await?;

    // The server has to read the row this writes, so the key must be its own.
    let db_key = src_postgres::DbKey::from_env()?;

    let client = PlaidClient::new(credentials)?;

    let created = src_plaid::create_sandbox_public_token(&client, &institution_id).await?;
    let exchanged = src_plaid::exchange_public_token(&client, &created.public_token).await?;

    // The name is absent rather than guessed: it arrives with the balances on
    // the first sync.
    src_postgres::save_item(
        &db,
        &db_key,
        &src_postgres::StoredItem {
            item_id: exchanged.item_id.clone(),
            user_id,
            access_token: exchanged.access_token,
            institution_id: Some(institution_id.clone()),
            institution_name: None,
            cursor: None,
            error_code: None,
            synced_at: api_common::Timestamp::EPOCH,
        },
    )
    .await?;

    // The access token is not printed: it is a credential, and this output is
    // the easy thing to paste by accident.
    println!(
        "\nlinked {institution_id} as item {} for {email}",
        exchanged.item_id
    );
    println!("start the server and it will sync on the first request\n");

    Ok(())
}
