# CLAUDE.md

- Be concise in your responses.
- If a question can be answered by looking at the code, look at the code instead
  of asking the user.
- **Never ask the user anything through a form.** No option pickers, no
  multiple choice, no `AskUserQuestion`. When you need an answer or have a list
  of questions, write them in plaintext in your reply and let the user answer in
  their own words.
- Never start the application yourself, the user is already running it. If the
  build is broken you can kill the user's instance and start it yourself, but
  otherwise you shouldn't need to.

## Running the stack

The database and the server run under docker compose, and their logs belong in
one tmux window that the user keeps open, running `./scripts/dev.sh` with no
arguments. That window is the user's; do not start it, and do not read logs by
starting a second one.

**Never run `docker compose up`, and never start a service in a shell of your
own.** Both tie the containers to a terminal nobody is watching, which is the
problem this script exists to solve. Drive the stack with the one-shot
subcommands, from wherever you are — each returns as soon as the daemon has
done as asked, and the output appears in the user's window:

```
./scripts/dev.sh restart server    restart, same image
./scripts/dev.sh build server      rebuild after a code change, and swap
./scripts/dev.sh up                after a down
./scripts/dev.sh down              stop and remove everything
./scripts/dev.sh ps                what is actually running
```

A change to Rust needs `build`, not `restart` — `restart` runs the image that
is already there, so the code you just wrote is not in it.

To read logs, ask for the slice you want and let it return: `docker compose
logs --since 2m server`. Never `-f`, which does not return.

## Style

`docs/STYLE.md` holds the house rules a formatter cannot enforce: no banner
comments, no panics in production code, how state machines are named and laid
out, and how JSON is formatted. **Read it before writing code.** If you find
yourself making a style decision it does not cover, add it there.

**Commit messages are one title and at most two sentences.** STYLE.md has it.

Two that catch people most often:

- **No `assert!`/`unwrap()`/`expect()` in production paths.** `debug_assert!`
  is encouraged in state machines instead.
- **No banner comments** (`// ─── section ───`). Split the file instead.

## Design

`sketches/design-language.html` is every control the app draws, in one place,
with a note on why each one is the way it is. Open it before designing anything
for the phone, the web or the desktop app, and build screens out of what is
already there. The styles behind it are `sketches/shared/components.css`; the
colours are `sketches/shared/theme.css`, which is where a colour is decided for
every platform.

If a screen needs a control the sheet does not have, add it to the sheet in the
same pass — a component that exists only inside one screen is how three
platforms end up with three design languages.

## The database

`docs/DATABASE.md` covers Postgres, the containers, and the one rule that
matters: **a migration that has been applied anywhere is immutable** — fix a
mistake with the next migration, never by editing the last one. All SQL lives
in `src-postgres` and nowhere else.

`scripts/db-reset.sh` destroys the database. It is for a human who has decided;
never run it yourself.

## Logging

How logging works is in `docs/LOGGING.md`. It ends with a list of every log
line the app writes, one example each. Whenever you add, change or remove a log
line, add, change or remove its example in that list.

## Issue tracking

This project lives on GitHub at `llGaetanll/meridian`. Issues and features are
tracked in the GitHub issues there. When you finish work on a feature, check it
off in the corresponding issue. If we are working on a bigger issue, we may need
to track our progress or the features we've implemented, or what we plan on
doing in the future. The user will usually tell you this but it's your
responsibility to jot that down in the issue.

You need to be very concise when writing anything in the GitHub issues, use
simple language whenever possible, no need to make it complicated.

## Plaid

`docs/PLAID.md` explains how the bank connection works. The rule that matters:
**only ever use the Plaid Sandbox unless the user explicitly says otherwise.**
The account is capped at 10 Production Items and removing an Item does not free
the slot, so a careless connection is permanent.
