//! Starts the server. Everything it does is in `lib.rs`.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    // Before anything else: a log that starts after the first request is a log
    // missing the request that broke.
    api_trace::init(api_trace::Config {
        // sqlx narrates its own housekeeping at INFO on every boot.
        quiet: &["sqlx::postgres::notice"],
        ..api_trace::Config::default()
    });

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "meridian server starting",
    );

    server_main::RuntimeServer::run().await
}
