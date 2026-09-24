//! One security, as Plaid describes it.
//!
//! Both investments endpoints answer with a `securities` list beside the rows
//! that use it. A holding or a trade carries a `security_id` and nothing else
//! about the instrument, so anything that names one joins against this list.

use api_common::Timestamp;
use serde::Deserialize;

use crate::optional_date;

/// One security, as Plaid reports it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaidSecurity {
    pub security_id: String,

    /// `AAPL`. Cash rows and some bonds have none.
    #[serde(default)]
    pub ticker_symbol: Option<String>,

    #[serde(default)]
    pub name: Option<String>,

    /// `equity`, `etf`, `mutual fund`, `fixed income`, `cash`,
    /// `cryptocurrency`, `derivative`, `loan`, `other`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,

    /// A currency held as a position — the row a brokerage's uninvested cash
    /// sits on.
    #[serde(default)]
    pub is_cash_equivalent: Option<bool>,

    /// The close as of `close_price_as_of`, a day old at best. Live value is
    /// quantity times our own market price, never this.
    #[serde(default)]
    pub close_price: Option<f64>,

    #[serde(default, deserialize_with = "optional_date")]
    pub close_price_as_of: Option<Timestamp>,

    #[serde(default)]
    pub iso_currency_code: Option<String>,

    /// ISIN, where the institution reports one. The stable name across
    /// institutions, where the ticker is not.
    #[serde(default)]
    pub isin: Option<String>,
}

impl PlaidSecurity {
    /// What to show for this security: the ticker when it has one, the name
    /// when it does not.
    pub fn label(&self) -> &str {
        self.ticker_symbol
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or(&self.security_id)
    }
}
