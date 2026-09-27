#!/bin/sh
# Known vulnerabilities in the dependencies, against the RustSec advisory
# database.
#
#     ./scripts/audit.sh
#
# `cargo audit` reads `Cargo.lock`, which a workspace has exactly one of, so
# one run covers every crate here — there is no `-p` to get wrong. The
# advisory database is fetched fresh on each run, so the answer is as current
# as the moment it was typed.
#
# This is a check, not a fix. A finding is read, weighed against the patch
# windows in `docs/SECURITY.md`, and resolved by a person bumping the
# dependency — never by a tool rewriting `Cargo.lock` on its own.

set -e

cd "$(dirname "$0")/.."

# Not installed by rustup, so `rust-toolchain.toml` cannot supply it and the
# first run on a new machine lands here.
if ! command -v cargo-audit >/dev/null 2>&1; then
    echo "cargo-audit is not installed. Install it with:" >&2
    echo "  cargo install cargo-audit" >&2
    exit 1
fi

cargo audit
