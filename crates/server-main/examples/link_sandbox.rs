//! Link a Sandbox bank and write the Item to the store.
//!
//! ```sh
//! cargo run -p server-main --example link_sandbox -- alex@example.com
//! cargo run -p server-main --example link_sandbox -- alex@example.com ins_109509
//! ```
//!
//! **A shortcut, not a fixture.** It performs the same two calls the real flow
//! ends with — exchange a `public_token`, store what comes back — and leaves
//! an ordinary Item that syncs like any other. What it skips is the part in
//! the middle, where a person signs into their bank on Plaid's own page.
//!
//! Hosted Link is built now (`/link/start` and `/link/finish`), so this is no
//! longer the only way a bank gets onto the books — it is the way that needs
//! no browser, which is what makes it useful from a terminal and in a script.
//!
//! It costs nothing. `/sandbox/public_token/create` does not exist in
//! Production, so there is no version of this that can spend an Item slot.
//!
//! Running it twice against the same institution is safe: Plaid answers with a
//! fresh Item each time, and `save_item` upserts, so the worst case is a
//! second bank in the list rather than a corrupted first one.

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

    // Whose bank it is. Required and not defaulted: an Item belongs to
    // somebody, and guessing who would put a bank on the wrong account.
    let email = std::env::args()
        .nth(1)
        .ok_or("usage: link_sandbox <email> [institution_id]")?;

    let institution_id = std::env::args()
        .nth(2)
        .unwrap_or_else(|| DEFAULT_INSTITUTION.to_string());

    let credentials = PlaidCredentials::from_env()?;

    // The same guard `runtime_plaid::fetch_all` carries, for the same reason.
    // This one is not really about caution — the endpoint below only exists in
    // Sandbox — but a clear refusal beats Plaid's 400 explaining that
    // `/sandbox/public_token/create` is not a route.
    if credentials.env != PlaidEnv::Sandbox {
        return Err("PLAID_ENV must be sandbox: this creates an Item without a person".into());
    }

    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;

    let db = src_postgres::connect(&url).await?;
    src_postgres::migrate(&db).await?;

    // Made if they do not exist, the same as signing in for the first time
    // does. So this works before anybody has ever asked for a code.
    let user_id = src_postgres::upsert_user(&db, &email).await?;

    // The same key the server uses, from the same place. This writes a row the
    // server has to be able to read, so a key of its own would be an Item that
    // exists and cannot be used.
    let db_key = src_postgres::DbKey::from_env()?;

    let client = PlaidClient::new(credentials)?;

    let created = src_plaid::create_sandbox_public_token(&client, &institution_id).await?;
    let exchanged = src_plaid::exchange_public_token(&client, &created.public_token).await?;

    // The name is deliberately absent rather than guessed. It arrives with the
    // balances on the first sync, which `SaveFetched` then writes — and a
    // guess here would be a row claiming to know something it does not.
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

    // The token is not printed. It is a credential, and this is the one place
    // it would be easy to paste somewhere by accident.
    println!(
        "\nlinked {institution_id} as item {} for {email}",
        exchanged.item_id
    );
    println!("start the server and it will sync on the first request\n");

    Ok(())
}
