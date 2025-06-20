use serde::Deserialize;

use crate::YChart;

#[derive(Debug, Deserialize)]
pub struct YResponseChart {
    pub chart: YChart,
}
