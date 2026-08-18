#!/bin/sh
# Regenerates the Swift and Kotlin bindings from the Rust core.
#
# Run this after any change to api-core's public types. The generated files are
# committed, so `git status` after running is the check that they were current:
# a dirty tree means someone changed the core and did not regenerate.
#
# See docs/FFI.md for why this is a script you have to run rather than
# something the build does for you.

set -e

cd "$(dirname "$0")/.."

cargo build -p api-core

cargo run -q -p api-core --bin uniffi-bindgen -- generate \
    --library target/debug/libapi_core.so \
    --language swift \
    --out-dir swift/Sources/Meridian

cargo run -q -p api-core --bin uniffi-bindgen -- generate \
    --library target/debug/libapi_core.so \
    --language kotlin \
    --out-dir kotlin/src/main/kotlin

echo "bindings regenerated"
