use api_common::format::fmt_date;
use api_common::format::fmt_time_of_day;
use api_common::format::fmt_year;
use leptos::prelude::*;
use web_components::AssetHeader;
use web_components::AssetTradesTable;
use web_components::ChartFoot;
use web_components::HeldIn;
use web_components::KeyStats;
use web_components::PositionSummary;
use web_components::PriceChart;
use web_components::QuoteList;
use web_components::RangeFoot;
use web_components::RangeSelector;
use web_components::TradeComposer;
use web_components::VolumeChart;
use web_components::math;
use web_ui::SearchBar;
use web_ui::Tone;

use crate::fake;

/// 2026-08-02 — "today" for the stand-in data.
const TODAY: u64 = 1_785_628_800;

#[component]
pub fn MarketsPage() -> impl IntoView {
    let prices = fake::price_series();
    let volume = fake::volume_series();
    let history = fake::history_series();

    let open_px = prices.first().map(|(_, px)| *px).unwrap_or_default();
    let last_px = prices.last().map(|(_, px)| *px).unwrap_or_default();
    let tone = Tone::of(last_px - open_px);

    let session_labels = math::time_axis(&prices, 6)
        .into_iter()
        .map(fmt_time_of_day)
        .collect::<Vec<_>>();

    let history_labels = math::time_axis(&history, 7)
        .into_iter()
        .map(fmt_year)
        .collect::<Vec<_>>();

    // The session on show is the last sliver of the whole history.
    let window = (0.968, 1.0);

    let position = fake::position();
    let holdings = fake::holdings();
    let portfolio_count = holdings.len();

    view! {
        <div class="asset-layout">
            <div class="col">
                <SearchBar />

                <div>
                    <AssetHeader
                        symbol=fake::SYMBOL
                        price=last_px
                        source="Yahoo Finance"
                        details="ETF · USD · NYSE Arca"
                    />

                    <PriceChart data=prices tone=tone />
                    <VolumeChart data=volume />
                    <ChartFoot labels=session_labels />

                    <RangeSelector data=history window=window />
                    <RangeFoot labels=history_labels />
                </div>

                <AssetTradesTable asset=fake::SYMBOL trades=fake::trades() />
            </div>

            <aside class="rail">
                <TradeComposer
                    pay_asset="USD"
                    pay_amount=6_497.40
                    receive_asset=fake::SYMBOL
                    receive_amount=10.0
                    rate=last_px
                    portfolio="Core"
                    date_label=fmt_date(TODAY)
                />

                <PositionSummary position=position portfolio_count=portfolio_count />

                <HeldIn asset=fake::SYMBOL holdings=holdings />

                <KeyStats source="Yahoo Finance" stats=fake::key_stats() />

                <QuoteList title="Recently seen" sub="Clear" quotes=fake::recently_seen() />
            </aside>
        </div>
    }
}
