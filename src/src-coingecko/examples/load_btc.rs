use chrono::TimeZone;
use chrono::Utc;
use src_coingecko::CoinGeckoClient;
use src_coingecko::GcRequestCoinData;
use src_coingecko::GcResponseCoinData;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = CoinGeckoClient::new()?;

    let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2025, 1, 10, 0, 0, 0).unwrap();

    let request = GcRequestCoinData::vs_usd("bitcoin", start, end);
    let response: GcResponseCoinData = client.send_request(&request).await?;

    println!("{:#?}", response);

    Ok(())
}
