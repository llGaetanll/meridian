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

# TODO

## UI
- [ ] Transactions list
- [ ] Search bar (search for tickers on yahoo finance)
- [ ] Price chart
    - [ ] Pull in data from yahoo finance

### Price chart
- Can show any time series data (portfolio performance, BTC price, derived
statistics, etc...)

## Misc
- [ ] Import/Export ledgers
