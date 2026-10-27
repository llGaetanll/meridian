# About

Meridian is one place to see everything you own, everything you owe, and
everything you spend, with the tools to explore what any of it means.

It shows you your money. It does not tell you what to do with it — the user
asks the questions, and the app answers them with a number.

It runs on the web and on the phone (iOS/Android), against a central server,
and the same crates back both. A desktop app was drawn and never built —
`sketches/desktop/` is as far as it got.

## Who it's for

People in their twenties and thirties earning good money for the first time,
who have never had to manage it. Concretely: a 401k left on whatever the
default was, a brokerage with a few positions bought on a hunch, an HSA sitting
in cash, RSUs on a vest schedule nobody explained, maybe some crypto, and a
checking account that just grows because there is nowhere obvious to put it.

They are not confused about whether they overspend — mostly they don't. What
they lack is a clear picture: what they actually hold, what it has actually
done, and what it looks like if they change something. Nobody gives them that
picture, so they guess.

That makes Meridian a different product from the budgeting apps. Those are
built around "am I spending too much". Meridian is built around "where do I
actually stand, and what would happen if".

The audience is also, largely, technical. They notice when an app is fast. They
expect keyboard shortcuts, exports, and an API, and they will not forgive a
spinner. Meridian is built to their standard, not to a bank website's.

## The edge

Not the data model, and not the bank connection — anyone can buy the same
connection. The edge is what the app can *show* once the whole picture is in
one place:

- **Surface what is there.** Where the cash is sitting and what it earns, what
  the funds charge, where positions overlap, how much of the year's gains are
  short-term. Facts about the account, stated plainly, that the user would
  otherwise have to go looking for across five websites.
- **Make equity comp legible.** Vest schedules, how much of the total sits in
  one employer, what a sale would realize. Central to this audience and
  essentially unserved; most people do it in a spreadsheet a coworker sent them.
- **Report real return.** Time-weighted and contribution-adjusted, across every
  account at once, the 401k included.
- **Run the scenario the user builds.** Assemble a portfolio, set a
  contribution, pick assumptions, and see how it plays out over the next few
  years. The user chooses what to try; the app does the arithmetic and shows
  the result.
- **Chart anything derived.** Any metric over the ledger, defined by the user,
  plotted. Along with export and an API, this is the part built for people who
  will want to ask their own questions.

Every one of these ends in a number on screen, not a suggestion. Powerful in
what it computes, simple in what it asks. Both halves matter: miss on the first
and it is another budgeting app, miss on the second and it is a spreadsheet
with extra steps.

## AI

An assistant is core to the app, not a bolt-on. It is a way to ask the app
questions in plain language instead of building the query by hand: why last
month looked different, what the fees add up to, how a scenario turns out. It
answers what was asked and stops there.

The rule it works under is that **the model does not do the math and does not
see the raw ledger**. It turns a question into a query, the engine computes the
answer, and the model explains the result. Every number is therefore
deterministic, reproducible and testable, and the data leaving the machine is
structured and minimal rather than a dump of someone's transactions. That fits
the [event and action loop](ARCHITECTURE.md) directly — the assistant is one
more event source emitting query actions, and it replays like anything else.

This is also where the product stance and the legal one line up. Describing the
user's own money, and computing the scenario they set up, is fine.
Recommending a security or predicting a market is not — and we were not going
to do it anyway. See [issue #16](https://github.com/llGaetanll/meridian/issues/16)
for the handling rules that come with holding this data at all.

## Sources

The ledger is populated by connecting to the user's banks and brokerages —
Plaid is the likely provider. Beyond that, the app can hook into services like
Amazon for detail an institution does not carry, and the phone can photograph a
receipt for the app to match against a transaction already in the ledger.
