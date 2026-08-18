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

# Getting started

The web app needs the tailwindcss CLI, `trunk`, and the wasm target; the
phone apps need a Kotlin and Android toolchain on top of that.
[docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) has the commands.

# MVP Goals
*Due August 23 2026*. One week as of writing.

- Something genuinely useful to me.
- Doesn't have to be feature-packed
- Connects to my bank using Plaid. I don't want to input transactions myself
- Super simple homepage. How much did I spend last week? Basic categorization of transactions.
- No accounts, no servers, everything local to the app.
