# CLAUDE.md

- Be concise in your responses.
- If a question can be answered by looking at the code, look at the code instead
  of asking the user.
- Never start the application yourself, the user is already running it. If the
  build is broken you can kill the user's instance and start it yourself, but
  otherwise you shouldn't need to.

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
