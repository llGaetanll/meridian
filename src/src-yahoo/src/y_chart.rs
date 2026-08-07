use serde::Deserialize;

use crate::YQuoteBlock;

#[derive(Debug, Deserialize)]
pub struct YChart {
    /// Absent when Yahoo refused the request, which it does for intervals it
    /// will not serve as far back as they were asked for.
    #[serde(default)]
    pub result: Option<Vec<YQuoteBlock>>,

    #[serde(default)]
    pub error: Option<YChartError>,
}

#[derive(Debug, Deserialize)]
pub struct YChartError {
    pub code: String,
    pub description: String,
}

impl std::fmt::Display for YChartError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.description)
    }
}
