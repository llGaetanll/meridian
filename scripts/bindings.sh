#!/bin/sh
# Regenerates the Swift and Kotlin bindings from the Rust core.
#
# Run this after any change to phone-state's public types. The generated files are
# committed, so `git status` after running is the check that they were current:
# a dirty tree means someone changed the core and did not regenerate.
#
# See docs/FFI.md for why this is a script you have to run rather than
# something the build does for you.

set -e

cd "$(dirname "$0")/.."

cargo build -p phone-state

cargo run -q -p phone-state --bin uniffi-bindgen -- generate \
    --library target/debug/libphone_state.so \
    --language swift \
    --out-dir swift/Sources/Meridian

cargo run -q -p phone-state --bin uniffi-bindgen -- generate \
    --library target/debug/libphone_state.so \
    --language kotlin \
    --out-dir kotlin/src/main/kotlin

echo "bindings regenerated"
