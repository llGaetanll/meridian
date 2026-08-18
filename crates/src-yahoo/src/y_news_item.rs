use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;

use crate::y_thumbnail::YThumbnail;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YNewsItem {
    pub uuid: String,

    pub title: String,

    pub publisher: String,

    pub link: String,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub provider_publish_time: DateTime<Utc>,

    #[serde(rename = "type")]
    pub news_type: String,

    pub thumbnail: Option<YThumbnail>,

    pub related_tickers: Vec<String>,
}
