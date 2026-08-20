use crate::GcRequest;

pub struct GcRequestCoinList;

// API details.
//
//  There is no pagination required for this endpoint.
//  Access to inactive coins via the Public API (Demo plan) is restricted. To access them, please
//  subscribe to one of our paid plans to obtain a Pro-API key.
//  Cache/Update Frequency:
//      Every 30 minutes for Public API.
//      Every 5 minutes for Pro API (Analyst, Lite, Pro, Enterprise).

impl GcRequest for GcRequestCoinList {
    fn url(&self) -> reqwest::Url {
        "https://api.coingecko.com/api/v3/coins/list"
            .parse()
            .unwrap() // won't fail
    }
}
