pub enum YInterval {
    /// 1 minute intervals - only available for last 7 days
    MinuteOne,

    /// 2 minute intervals - only available for last 60 days  
    MinuteTwo,

    /// 5 minute intervals - only available for last 60 days
    MinuteFive,

    /// 15 minute intervals - only available for last 60 days
    MinuteFifteen,

    /// 30 minute intervals - only available for last 60 days
    MinuteThirty,

    /// 60 minute intervals - only available for last 60 days
    MinuteSixty,

    /// 90 minute intervals - only available for last 60 days
    MinuteNinety,

    /// 1 hour intervals - only available for last 60 days
    HourOne,

    /// 1 day intervals - no limit
    DayOne,

    /// 5 day intervals - no limit
    DayFive,

    /// 1 week intervals - no limit
    WeekOne,

    /// 1 month intervals - no limit
    MonthOne,

    /// 3 month intervals - no limit
    MonthThree,
}

impl std::fmt::Display for YInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            YInterval::MinuteOne => "1m",
            YInterval::MinuteTwo => "2m",
            YInterval::MinuteFive => "5m",
            YInterval::MinuteFifteen => "15m",
            YInterval::MinuteThirty => "30m",
            YInterval::MinuteSixty => "60m",
            YInterval::MinuteNinety => "90m",
            YInterval::HourOne => "1h",
            YInterval::DayOne => "1d",
            YInterval::DayFive => "5d",
            YInterval::WeekOne => "1wk",
            YInterval::MonthOne => "1mo",
            YInterval::MonthThree => "3mo",
        };
        write!(f, "{}", s)
    }
}

impl YInterval {
    /// Check if this interval is intraday (has time limitations)
    pub fn is_intraday(&self) -> bool {
        matches!(
            self,
            YInterval::MinuteOne
                | YInterval::MinuteTwo
                | YInterval::MinuteFive
                | YInterval::MinuteFifteen
                | YInterval::MinuteThirty
                | YInterval::MinuteSixty
                | YInterval::MinuteNinety
                | YInterval::HourOne
        )
    }

    /// Get the maximum historical period available for this interval
    pub fn max_period_days(&self) -> Option<u32> {
        match self {
            YInterval::MinuteOne => Some(7),
            _ if self.is_intraday() => Some(60),
            _ => None, // No limit
        }
    }
}
