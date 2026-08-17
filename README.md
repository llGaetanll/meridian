# About

Meridian is one place to see everything you own, everything you owe, and
everything you spend, with the tools to explore what any of it means. It runs
on the web, on the desktop (Tauri), and on the phone (iOS/Android).

It is built for people earning good money for the first time who have never had
to manage it — a default 401k, an uninvested HSA, RSUs nobody explained, cash
with nowhere to go. The budgeting apps answer "am I spending too much".
Meridian answers "where do I actually stand, and what would happen if": what
you hold and what it costs you, what real return looks like across every
account, and how a portfolio you build plays out over the next few years.

It shows you your money. It does not tell you what to do with it.

[docs/ABOUT.md](docs/ABOUT.md) has the full picture — who it's for, where the
edge is, how the assistant works, and why everything is a trade.
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) covers how it's put together.

# Getting Started

You will need the tailwindcss cli. If you want to install it without going
through npm, you can do so as follows:
```
curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64
chmod +x tailwindcss-linux-x64
sudo mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss
```

You will also need `trunk`, and the right wasm target
```
cargo binstall trunk
```

```
rustup target add wasm32-unknown-unknown
```

Then, from inside `web-main`, run `trunk serve --open`

# Road to 1.0

Release Aug 31.

## Transactions
- [x] Display tx list
- [ ] Support adding new tx

## Chart
- [ ] Add time windows (24H, 7D, 1M, 3M, 1Y, Max)
- [ ] Add timewindow selection (left click and hold to select a time window,
      like on CoinGecko).
- [ ] Can graph any derived data
    - Portfolio performance, BTC price, sharpe ratio overtime, etc...

## Misc
- [ ] Search bar, can pull from all data sources
    - Tickers on Yahoo Finance
    - Coins on CoinGecko

## Data Sources
- [x] Parse Yahoo Finance API (`src-yahoo`)
- [x] Parse Coin Gecko API (`src-coin-gecko`)

## Misc
- [ ] Import/Export ledgers

## Portfolios
- [ ] Support creation of portfolios

# Reach/Less important

- [ ] General theme support

## Chart
- [ ] Can graph any derived data
    - Portfolio performance, BTC price, sharpe ratio overtime, etc...
