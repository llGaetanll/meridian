use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YSearchQuote {
    pub exchange: String,
    pub shortname: Option<String>,
    pub quote_type: String,
    pub symbol: String,
    pub index: String,
    pub score: f64,
    pub type_disp: Option<String>,
    pub longname: Option<String>,
    pub exch_disp: Option<String>,
    pub sector: Option<String>,
    pub sector_disp: Option<String>,
    pub industry: Option<String>,
    pub industry_disp: Option<String>,
    pub disp_sec_ind_flag: Option<bool>,
    pub is_yahoo_finance: bool,
}
