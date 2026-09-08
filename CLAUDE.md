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
- **Never create a branch or a worktree unless the user explicitly asks for
  one.** Work happens on `master`, and that is where a commit goes.

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

`docs/STYLE.md` is the house style. **Read it before writing code.** Don't make
style decisions yourself. If you're unsure about a style rule, just ask the
user.

## Design

`sketches/design-language.html` is every control the app draws, in one place,
with a note on why each one is the way it is. Open it before designing anything
for the phone, the web or the desktop app, and build screens out of what is
already there. The styles behind it are `sketches/shared/components.css`; the
colours are `sketches/shared/theme.css`, which is where a colour is decided for
every platform.

If a screen needs a control the sheet does not have, add it to the sheet in the
same pass — a component that exists only inside one screen is how three
platforms end up with three design languages. Be sure to alert the user when
this happens.

## The database

`docs/DATABASE.md` covers Postgres, the containers, and migrations. Read it
before touching the schema or writing a query.

`scripts/db-reset.sh` destroys the database. It is for a human who has decided;
never run it yourself.

## Logging

How logging works is in `docs/LOGGING.md`.

## Issue tracking

This project lives on GitHub at `llGaetanll/meridian`. Issues and features are
tracked in the GitHub issues there.

**An issue describes a problem.** It is written in three parts, in this order:

1. **One sentence saying what is wrong**, in the plainest words you can find.
   A reader who knows nothing about this project should understand it.
2. **A paragraph explaining it properly** — what happens, when, and what it
   affects.
3. **What is needed to resolve it**, as a checklist.

When an issue is opened that third part is often not known yet, and that is
fine — leave it out. **Never decide how to fix an issue without asking the
user.** A plan nobody agreed to is context that misleads the next agent, and
writing the problem down is the whole job.

Be very concise and use simple language. When work lands, check the item off.

**An issue is one message.** When something changes — work lands, a claim turns
out to be wrong — edit the body. Never add a comment saying what the body now
gets wrong, because that leaves a reader two accounts to reconcile and the
stale one is on top.

## Plaid

`docs/PLAID.md` explains how the bank connection works. The rule that matters:
**only ever use the Plaid Sandbox unless the user explicitly says otherwise.**
The account is capped at 10 Production Items and removing an Item does not free
the slot, so a careless connection is permanent.
