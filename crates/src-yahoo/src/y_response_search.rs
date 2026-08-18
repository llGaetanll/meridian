use serde::Deserialize;

use crate::YNewsItem;
use crate::YSearchQuote;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YResponseSearch {
    pub explains: Vec<serde_json::Value>,
    pub count: u32,
    pub quotes: Vec<YSearchQuote>,
    pub news: Vec<YNewsItem>,
    pub nav: Vec<serde_json::Value>,
    pub lists: Vec<serde_json::Value>,
    pub research_reports: Vec<serde_json::Value>,
    pub screener_field_results: Vec<serde_json::Value>,
    pub total_time: u32,
    pub time_taken_for_quotes: u32,
    pub time_taken_for_news: u32,
    pub time_taken_for_algowatchlist: u32,
    pub time_taken_for_predefined_screener: u32,
    pub time_taken_for_crunchbase: u32,
    pub time_taken_for_nav: u32,
    pub time_taken_for_research_reports: u32,
    pub time_taken_for_screener_field: u32,
    pub time_taken_for_cultural_assets: u32,
    pub time_taken_for_search_lists: u32,
}
