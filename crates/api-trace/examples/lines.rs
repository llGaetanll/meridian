//! Every line the app writes, printed once, in the order a session produces
//! them. This is what the list at the end of `docs/LOGGING.md` is taken from.
//!
//! Lines are logged under the module they came from, and this example is one
//! module. Where a line is a stand-in for one the app writes elsewhere, the
//! module it really comes from is named — with `target:` for a plain line, and
//! as the first argument for a `->`/`<-` pair, which is where a real caller
//! passes its `module_path!()`.
//!
//! `cargo run -p api-trace --example lines`

use api_trace::ApiCall;
use api_trace::Config;
use api_trace::Level;
use api_trace::Served;
use api_trace::Source;
use api_trace::scrub;

// The same sources the clients declare, so what prints here is what a real
// session prints — Plaid's pretty-printed response included, which is the
// thing `compact_json` exists to squeeze.
const PLAID: Source =
    Source::new("plaid").through(&[scrub::redact_credentials, scrub::compact_json]);
const YAHOO: Source = Source::new("yahoo");
const MERIDIAN: Source = Source::new("meridian");

fn main() {
    api_trace::init(Config {
        level: Level::DEBUG,
        buffer: 64,
        file: None,
        quiet: &[],
    });

    tracing::info!(target: "web_main", version = "0.1.0", "meridian starting");

    // One conversation with a client, from both ends. The server writes the
    // `client=` pair (`server_main::router_trace`) and the phone writes the
    // `source=meridian` pair (`phone_main::runtime_io`); they are two logs on
    // two machines, which is why each is worth having on its own.
    Served::received(
        "server_main::router_trace",
        "192.168.1.24:53112",
        "GET",
        "/snapshot",
    )
    .answered(200, Some(91233));

    ApiCall::get(
        "phone_main::runtime_io",
        MERIDIAN,
        "http://127.0.0.1:8082/snapshot",
    )
    .response(
        200,
        "{\"accounts\":[{\"id\":\"6Pd…\"}],\"transactions\":[]}",
    );

    // A client that hung up before its answer, which is what closing the app
    // mid-refresh looks like from the server.
    let _ = Served::received(
        "server_main::router_trace",
        "192.168.1.24:53112",
        "GET",
        "/snapshot",
    );

    // Plaid, which is POST and whose body *is* logged — field names and all,
    // with the credentials replaced by their length.
    tracing::info!(
        target: "src_plaid::plaid_client",
        env = "sandbox",
        base_url = "https://sandbox.plaid.com/",
        "plaid client",
    );

    ApiCall::post(
        "src_plaid::plaid_client",
        PLAID,
        "https://sandbox.plaid.com/transactions/sync",
        "{\"access_token\":\"access-sandbox-1\",\"client_id\":\"6xxxxxxxxxxxxxxxxxxxxxxx\",\"cursor\":\"CAESJTM3…\",\"secret\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}",
    )
    .response(
        200,
        "{\n    \"added\": [\n        {\n            \"account_id\": \"6Pd…\"\n        }\n    ]\n}",
    );

    tracing::info!(
        target: "src_plaid",
        added = 18,
        modified = 0,
        removed = 1,
        "transactions synced",
    );

    let span = tracing::info_span!("quote", symbol = "SPY", range = "1D");
    let _entered = span.enter();

    tracing::info!(target: "web_main::io::markets", "quote asked for");

    ApiCall::get(
        "src_yahoo::yahoo_client",
        YAHOO,
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

    ApiCall::get("src_yahoo::yahoo_client", YAHOO, "/v8/finance/chart/SPY")
        .failed("Network error: operation timed out");

    ApiCall::get("src_yahoo::yahoo_client", YAHOO, "/v8/finance/chart/SPY")
        .response(200, "<html>\n  <body>Too Many Requests</body>\n</html>")
        .parse_error("missing field `chart`");

    // Never given an outcome: what a cancelled request looks like.
    let _ = ApiCall::get("src_yahoo::yahoo_client", YAHOO, "/v8/finance/chart/QQQ");

    tracing::warn!(
        target: "trace",
        error = "logs/meridian.log: permission denied",
        "log file unavailable",
    );

    println!("\n--- buffered ---\n{}", api_trace::dump());
}
