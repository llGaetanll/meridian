use std::iter::Zip;
use std::ops::Range;
use std::vec::IntoIter;

use crate::Price;
use crate::Timestamp;

// Both inner vectors should have the same length.
#[derive(Debug, Clone, Default)]
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

    pub fn len(&self) -> usize {
        self.ts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ts.is_empty()
    }

    /// The timestamps, in the order the points are plotted.
    pub fn timestamps(&self) -> &[Timestamp] {
        &self.ts
    }

    /// Re-bar the series down to at most `max_points`, keeping the closing
    /// value of each bucket — what a coarser interval would have given.
    ///
    /// Use this for prices.
    pub fn resample_last(&self, max_points: usize) -> Self {
        self.resample(max_points, |_, px| *px.last().unwrap_or(&0.0))
    }

    /// The same buckets, adding each one up instead. Use this for volume and
    /// anything else that accumulates.
    ///
    /// Two series of equal length resampled to the same `max_points` land on
    /// identical buckets, so they stay index-aligned — which the charts rely
    /// on to draw a volume bar under its price point.
    pub fn resample_sum(&self, max_points: usize) -> Self {
        self.resample(max_points, |_, px| px.iter().sum())
    }

    fn resample<F>(&self, max_points: usize, fold: F) -> Self
    where
        F: Fn(&[Timestamp], &[Price]) -> Price,
    {
        let len = self.len();

        if max_points == 0 || len <= max_points {
            return self.clone();
        }

        let mut out = Self::default();
        let mut start = 0;

        for i in 0..len {
            // Points share a bucket while this index holds steady.
            let last_of_bucket = i + 1 == len || (i + 1) * max_points / len != i * max_points / len;

            if last_of_bucket {
                let (ts, px) = (&self.ts[start..=i], &self.px[start..=i]);

                // `start <= i` always holds, so the slice is never empty.
                let Some(&last) = ts.last() else {
                    unreachable!("start <= i, so the bucket holds a point")
                };

                out.ts.push(last);
                out.px.push(fold(ts, px));

                start = i + 1;
            }
        }

        out
    }

    pub fn iter(&self) -> impl Iterator<Item = (Timestamp, Price)> + '_ {
        self.ts.iter().copied().zip(self.px.iter().copied())
    }

    pub fn first(&self) -> Option<(&Timestamp, &Price)> {
        self.ts.first().zip(self.px.first())
    }

    pub fn last(&self) -> Option<(&Timestamp, &Price)> {
        self.ts.last().zip(self.px.last())
    }
}

impl FromIterator<(Timestamp, Price)> for ChartData {
    fn from_iter<I: IntoIterator<Item = (Timestamp, Price)>>(iter: I) -> Self {
        let (ts, px) = iter.into_iter().unzip();
        Self { ts, px }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn series(n: usize) -> ChartData {
        (0..n)
            .map(|i| (Timestamp::from_secs(i as u64), i as Price))
            .collect()
    }

    fn secs(values: &[u64]) -> Vec<Timestamp> {
        values.iter().copied().map(Timestamp::from_secs).collect()
    }

    #[test]
    fn leaves_short_series_alone() {
        assert_eq!(series(10).resample_last(50).len(), 10);
        assert_eq!(series(50).resample_last(50).len(), 50);
    }

    #[test]
    fn caps_at_the_target() {
        for len in [51, 100, 1879, 5000] {
            assert!(series(len).resample_last(900).len() <= 900);
        }
    }

    #[test]
    fn keeps_the_close_of_each_bucket() {
        let out = series(10).resample_last(5);

        assert_eq!(out.px, vec![1.0, 3.0, 5.0, 7.0, 9.0]);
        assert_eq!(out.ts, secs(&[1, 3, 5, 7, 9]));
    }

    #[test]
    fn adds_up_within_a_bucket() {
        let out = series(10).resample_sum(5);

        assert_eq!(out.px, vec![1.0, 5.0, 9.0, 13.0, 17.0]);
    }

    /// Prices and volume have to stay aligned: the charts pair them by index.
    #[test]
    fn buckets_the_same_way_for_both_folds() {
        let data = series(1879);

        let (last, sum) = (data.resample_last(900), data.resample_sum(900));

        assert_eq!(last.len(), sum.len());
        assert_eq!(last.ts, sum.ts);
    }

    #[test]
    fn always_ends_on_the_last_point() {
        let out = series(1879).resample_last(900);

        assert_eq!(out.ts.last(), Some(&Timestamp::from_secs(1878)));
        assert_eq!(out.px.last(), Some(&1878.0));
    }
}
