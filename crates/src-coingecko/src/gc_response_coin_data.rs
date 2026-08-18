use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Deserializer;

#[derive(Debug, Deserialize)]
pub struct GcResponseCoinData {
    #[serde(deserialize_with = "deserialize_gc_tuples")]
    pub prices: Vec<(DateTime<Utc>, f64)>,

    #[serde(deserialize_with = "deserialize_gc_tuples")]
    pub market_caps: Vec<(DateTime<Utc>, f64)>,

    #[serde(deserialize_with = "deserialize_gc_tuples")]
    pub total_volumes: Vec<(DateTime<Utc>, f64)>,
}

fn deserialize_gc_tuples<'de, D>(deserializer: D) -> Result<Vec<(DateTime<Utc>, f64)>, D::Error>
where
    D: Deserializer<'de>,
{
    let tuples: Vec<(u64, f64)> = Vec::deserialize(deserializer)?;
    tuples
        .into_iter()
        .map(|(timestamp, value)| {
            DateTime::from_timestamp_millis(timestamp as i64)
                .map(|dt| (dt, value))
                .ok_or_else(|| serde::de::Error::custom("Invalid timestamp"))
        })
        .collect()
}
