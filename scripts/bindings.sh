#!/bin/sh
# Regenerates the Swift and Kotlin bindings from the Rust core.
#
# Run this after any change to client-state's public types. The generated files are
# committed, so `git status` after running is the check that they were current:
# a dirty tree means someone changed the core and did not regenerate.
#
# See docs/FFI.md for why this is a script you have to run rather than
# something the build does for you.

set -e

cd "$(dirname "$0")/.."

cargo build -p client-state

# The name of the built library is the platform's, not ours: a cdylib is
# `.dylib` on macOS and `.so` everywhere else this is run. uniffi-bindgen reads
# the built library rather than the source, so it has to be given the real one.
case "$(uname -s)" in
    Darwin) LIB=target/debug/libclient_state.dylib ;;
    *) LIB=target/debug/libclient_state.so ;;
esac

cargo run -q -p client-state --bin uniffi-bindgen -- generate \
    --library "$LIB" \
    --language swift \
    --out-dir swift/Sources/Meridian

cargo run -q -p client-state --bin uniffi-bindgen -- generate \
    --library "$LIB" \
    --language kotlin \
    --out-dir kotlin/src/main/kotlin

echo "bindings regenerated"
