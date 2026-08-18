pub enum YRange {
    /// 1 day range
    DayOne,

    /// 5 day range
    DayFive,

    /// 1 month range
    MonthOne,

    /// 3 month range
    MonthThree,

    /// 6 month range
    MonthSix,

    /// 1 year range
    YearOne,

    /// 2 year range
    YearTwo,

    /// 5 year range
    YearFive,

    /// 10 year range
    YearTen,

    /// Year to date
    YearToDate,

    /// Maximum available history
    Max,
}

impl std::fmt::Display for YRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            YRange::DayOne => "1d",
            YRange::DayFive => "5d",
            YRange::MonthOne => "1mo",
            YRange::MonthThree => "3mo",
            YRange::MonthSix => "6mo",
            YRange::YearOne => "1y",
            YRange::YearTwo => "2y",
            YRange::YearFive => "5y",
            YRange::YearTen => "10y",
            YRange::YearToDate => "ytd",
            YRange::Max => "max",
        };
        write!(f, "{}", s)
    }
}
