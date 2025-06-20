use crate::YRequest;

pub struct YRequestSearch<'a> {
    query: &'a str,
}

impl<'a> YRequestSearch<'a> {
    /// Create a new ticker search request
    pub fn new(query: &'a str) -> Self {
        YRequestSearch { query }
    }
}

impl<'a> YRequest for YRequestSearch<'a> {
    fn url(&self) -> reqwest::Url {
        reqwest::Url::parse_with_params(
            "https://query2.finance.yahoo.com/v1/finance/search",
            &[("q", self.query)],
        )
        .unwrap() // won't fail
    }
}
