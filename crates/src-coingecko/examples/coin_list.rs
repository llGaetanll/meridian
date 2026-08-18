use src_coingecko::CoinGeckoClient;
use src_coingecko::GcRequestCoinList;
use src_coingecko::GcResponseCoinList;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = CoinGeckoClient::new()?;

    let request = GcRequestCoinList;
    let response: GcResponseCoinList = client.send_request(&request).await?;

    println!("{:#?}", response);

    Ok(())
}
