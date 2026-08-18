use src_yahoo::{YRequestSearch, YResponseSearch, YahooClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = YahooClient::new()?;

    let request = YRequestSearch::new("MSFT");
    let response: YResponseSearch = client.send_request(&request).await?;

    println!("{:#?}", response);

    Ok(())
}
