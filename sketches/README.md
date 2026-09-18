# Sketches

Screens drawn in HTML and CSS before they are built in Rust, so that a screen
is argued about once rather than three times. Open any of them in a browser;
`index.html` links them all.

Nothing here runs. Every file is a picture.

## First run, the flow

Two boards, one per platform, and they are the thing to read before touching
onboarding:

- `screens/first-run-flow.html` — desktop
- `screens/first-run-phone-flow.html` — phone

Each draws **every screen of the road in**, in four parts — Account, Payment,
Bank, Finishing — with an arrow for every way the machine gets from one to the
next.

**They show the flow we want, not the flow that is built.** Today paying
happens when Connect a bank is pressed; on these boards it is settled before
the bank is mentioned, and there are three doors on the front rather than one.
Anything you implement from here is a change, not a transcription.

### Finding a screen

The top of each board lists every box on it, grouped by part, each one a link
to that box. Every box also carries an id, so from a terminal:

```
grep -n 'id="code-refused"' sketches/screens/first-run-flow.html
```

and in a browser, `first-run-flow.html#code-refused` jumps to it. The contents
list at the top of the board is the same set as links, and the ids are, in the
order they are drawn:

| Part | Ids |
| --- | --- |
| Account | `welcome` `welcome-out` `apple` `google` `email` `email-bad` `code` `code-filled` `code-refused` `code-resend` |
| Payment | `paywall` `pay-chrome` `pay-declined` `pay-ended` `pay-unknown` `stripe` `confirming` `conf-again` |
| Bank | `bank` `plaid` `failed` `failed-block` `failed-linked` |
| Finishing | `sync` `ready` `app` |

The phone board uses the same ids for the screens it has. It has `storekit`
where the desktop has `stripe`, and it has no `pay-chrome`, `confirming`,
`conf-again` or `failed-block`: Apple sells the subscription, so there is no
Stripe page, no webhook to wait on and no browser to block a window.

### What the marks mean

- **A solid violet arrow** is a tap, or an answer from the server, moving the
  flow on.
- **A dashed grey line** ties a screen to the other states it can be in. The
  state is the same screen, not a place of its own: a code refused, a card
  declined.
- **A dashed edge on a box** means it is not our screen. Apple's, Google's and
  Plaid's are named rather than drawn, because we cannot know what they will
  show. Stripe's is drawn, because we can.
- **A dotted band** is one part of the road.
- **A grey name** above a box is a state; a violet one is a screen on the road.

### Where the words come from

Not from the view. Every sentence on every one of these screens is produced in
one place and handed to all three platforms finished:

- `crates/client-state/src/screens_onboarding.rs` — first run
- `crates/client-state/src/screens_paywall.rs` — the paywall

Those build `ViewOnboarding` and `ViewPaywall`, and the view draws the arm it
is handed. So a screen changes there, and `docs/FFI.md` says why. A platform
that formats its own date or words its own refusal is the bug.

`crates/client-state/src/state_onboarding.rs` is the machine behind the arrows:
one method per transition the board draws.

### Editing a board

Both are generated: the nodes are placed by hand in the HTML, and the arrows
are measured from where they land by `screens/first-run-flow.js`, which both
boards share. To move a screen, change the one `left`/`top` on its node — no
path needs redrawing. To add an arrow, add an entry to the `drawFlow` call at
the foot of the page.

## The rest

- `design-language.html` — every control the app draws, once, with a note on
  why each is the way it is. **Read it before designing anything.** The styles
  behind it are `shared/components.css`; the colours are `shared/theme.css`.
- `screens/` — the web and desktop screens: the shell, the ledger, recurring,
  balances, the account page.
- `mobile/` — the phone's own screens.
- `desktop/` — the markets side.
- `brand/` — the mark, and the script that draws it.
