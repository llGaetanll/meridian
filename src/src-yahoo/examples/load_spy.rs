use src_yahoo::{YInterval, YRange, YRequestChart, YResponseChart, YahooClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    api_trace::init(api_trace::Config::default());

    let client = YahooClient::new()?;

    let request = YRequestChart::range("SPY", YRange::DayOne, YInterval::MinuteFive);
    let response: YResponseChart = client.send_request(&request).await?;

    println!("{:#?}", response);

    Ok(())
}
