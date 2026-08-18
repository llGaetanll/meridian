//! Reactive state for the markets page.
//!
//! It holds signals and resources and nothing else: the requests themselves
//! live in [`crate::io`], and the components it feeds take plain values.

use leptos::prelude::*;

use crate::io::AssetQuote;
use crate::io::ChartRange;
use crate::state::markets_cache;

/// What the markets page is looking at, and what has come back so far.
#[derive(Clone, Copy)]
pub struct MarketsState {
    symbol: RwSignal<String>,
    /// Index into [`ChartRange::ALL`], which is what the range picker reports.
    pub selected_range: RwSignal<usize>,
    /// The series behind the main chart, for the selected range.
    pub quote: LocalResource<Result<AssetQuote, String>>,
    /// The whole history behind the range selector.
    pub history: LocalResource<Result<AssetQuote, String>>,
}

impl MarketsState {
    pub fn new(symbol: &str) -> Self {
        let symbol = RwSignal::new(symbol.to_string());
        let selected_range = RwSignal::new(0);
        let markets = markets_cache();

        let quote = LocalResource::new(move || {
            let markets = markets.read_value().clone();
            let symbol = symbol.get();
            let range = ChartRange::ALL[selected_range.get()];

            async move { markets.quote(&symbol, range).await }
        });

        let history = LocalResource::new(move || {
            let markets = markets.read_value().clone();
            let symbol = symbol.get();

            async move { markets.quote(&symbol, ChartRange::Max).await }
        });

        Self {
            symbol,
            selected_range,
            quote,
            history,
        }
    }

    pub fn symbol(&self) -> Signal<String> {
        let symbol = self.symbol;
        Signal::derive(move || symbol.get())
    }

    /// Point the page at another asset. Both resources refetch on their own —
    /// though "refetch" is only a request for whatever is not cached yet.
    pub fn look_at(&self, symbol: &str) {
        self.symbol.set(symbol.to_string());
    }
}
