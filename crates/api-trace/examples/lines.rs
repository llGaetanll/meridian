//! Every line the app writes, printed once, in the order a session produces
//! them. This is what the list at the end of `docs/LOGGING.md` is taken from.
//!
//! `cargo run -p api-trace --example lines`

use api_trace::ApiCall;
use api_trace::Config;
use api_trace::Level;

fn main() {
    api_trace::init(Config {
        level: Level::DEBUG,
        buffer: 64,
        file: None,
    });

    tracing::info!(target: "app", version = "0.1.0", "meridian starting");

    // Plaid, which is POST and whose body is never logged: it holds the secret.
    tracing::info!(
        target: "app",
        env = "sandbox",
        base_url = "https://sandbox.plaid.com/",
        "plaid client",
    );

    ApiCall::post("plaid", "https://sandbox.plaid.com/transactions/sync")
        .response(200, "{\"added\":[{\"account_id\":\"6Pd…");

    tracing::info!(
        target: "app",
        added = 18,
        modified = 0,
        removed = 1,
        "transactions synced",
    );

    let span = tracing::info_span!("quote", symbol = "SPY", range = "1D");
    let _entered = span.enter();

    tracing::info!(target: "app", "quote asked for");

    ApiCall::get(
        "yahoo",
        "/v8/finance/chart/SPY?period1=1785542400&period2=1785628800&interval=1m",
    )
    .response(
        200,
        "{\"chart\":{\"result\":[{\"meta\":{\"currency\":\"USD\",\"symbol\":\"SPY\"…",
    );

    tracing::info!(
        target: "cache",
        symbol = "SPY",
        interval = "Min1",
        window = "1785542400..1785628800",
        bars = 930,
        cached = false,
        "served",
    );

    tracing::info!(
        target: "cache",
        symbol = "BTC-USD",
        asked = "Min1",
        served = "Hour1",
        "source substituted an interval",
    );

    ApiCall::get("yahoo", "/v8/finance/chart/SPY").failed("Network error: operation timed out");

    ApiCall::get("yahoo", "/v8/finance/chart/SPY")
        .response(200, "<html>\n  <body>Too Many Requests</body>\n</html>")
        .parse_error("missing field `chart`");

    // Never given an outcome: what a cancelled request looks like.
    let _ = ApiCall::get("yahoo", "/v8/finance/chart/QQQ");

    tracing::warn!(
        target: "trace",
        error = "logs/meridian.log: permission denied",
        "log file unavailable",
    );

    println!("\n--- buffered ---\n{}", api_trace::dump());
}
