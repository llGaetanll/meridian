use std::iter::Zip;
use std::ops::Range;
use std::vec::IntoIter;

use crate::Price;
use crate::Timestamp;

// Both inner vectors should have the same length.
#[derive(Clone)]
pub struct ChartData {
    /// Timestamps
    ts: Vec<Timestamp>,

    /// Prices
    px: Vec<Price>,
}

impl ChartData {
    /// If no data is present, this ranges from Timestamp::MAX..Timestamp::MIN
    pub fn range_time(&self) -> Range<Timestamp> {
        let mut min = Timestamp::MAX;
        let mut max = Timestamp::MIN;

        for &t in &self.ts {
            min = min.min(t);
            max = max.max(t);
        }

        min..max
    }

    /// If no data is present, this ranges from Price::MAX..Price::MIN
    pub fn range_price(&self) -> Range<Price> {
        let mut min = Price::MAX;
        let mut max = Price::MIN;

        for &p in &self.px {
            min = min.min(p);
            max = max.max(p);
        }

        min..max
    }

    pub fn first(&self) -> Option<(&Timestamp, &Price)> {
        self.ts.first().zip(self.px.first())
    }

    pub fn last(&self) -> Option<(&Timestamp, &Price)> {
        self.ts.last().zip(self.px.last())
    }
}

impl IntoIterator for ChartData {
    type Item = (Timestamp, Price);
    type IntoIter = Zip<IntoIter<Timestamp>, IntoIter<Price>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ts.into_iter().zip(self.px)
    }
}

impl<'a> IntoIterator for &'a ChartData {
    type Item = (&'a Timestamp, &'a Price);
    type IntoIter = Zip<std::slice::Iter<'a, Timestamp>, std::slice::Iter<'a, Price>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ts.iter().zip(self.px.iter())
    }
}
