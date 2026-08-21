#!/bin/sh
# Builds the iOS app and runs it in the simulator. No Xcode project and no
# Xcode: swiftc compiles, simctl installs and launches, and the Simulator is
# left where you put it.
#
# The Simulator window is an ordinary macOS window, so park it on whatever
# screen you like. Re-running this installs into the device that is already
# booted rather than starting a new one, so the window neither moves nor
# reopens — the app just becomes the new build.
#
# The app reads its data from server-main, which the simulator reaches over the
# Mac's own loopback:
#
#     cargo run -p server-main
#
# Needs: Xcode's command line tools, the Rust iOS targets, and a simulator
# runtime. docs/GETTING_STARTED.md has all three.

set -e

cd "$(dirname "$0")/.."

# Which simulator to use. Any name from `xcrun simctl list devices` works:
#
#     ./scripts/ios-run.sh "iPhone 16e"
DEVICE=${1:-iPhone 17 Pro}

BUNDLE_ID=com.meridian.phone
DEPLOYMENT=17.0
BINDINGS=swift/Sources/Meridian
APP=target/ios/Meridian.app

# The simulator runs the Mac's own architecture, so the triple follows the host
# rather than the phone. Rust spells the Apple Silicon simulator with a `-sim`
# suffix and the Intel one without, because on Intel the simulator target and
# the device target were never the same thing to begin with.
case "$(uname -m)" in
    arm64)
        TARGET=arm64-apple-ios$DEPLOYMENT-simulator
        RUST_TARGET=aarch64-apple-ios-sim
        ;;
    *)
        TARGET=x86_64-apple-ios$DEPLOYMENT-simulator
        RUST_TARGET=x86_64-apple-ios
        ;;
esac

SDK=$(xcrun --sdk iphonesimulator --show-sdk-path)

cargo build -p phone-state --target $RUST_TARGET

rm -rf $APP
mkdir -p $APP

cp swift/Support/Info.plist $APP/Info.plist

# One module out of the generated bindings and the app, which is what lets the
# app say `Core()` without an import. `-Xcc -fmodule-map-file` is how the C
# half of the bindings is found.
#
# The core is named as an archive rather than with `-lphone_state`, because the
# cargo build leaves a `.dylib` beside the `.a` and the linker prefers it — and
# an app that loads a library out of `target/` by absolute path works only on
# the machine that built it.
#
# `-Wno-incompatible-sysroot` silences the linker misreading the simulator SDK
# as the Mac's. The binary it produces says otherwise: `otool -l` reports
# platform 7, which is the simulator.
swiftc \
    -module-name Meridian \
    -target "$TARGET" \
    -sdk "$SDK" \
    -I $BINDINGS \
    -Xcc -fmodule-map-file=$BINDINGS/phone_stateFFI.modulemap \
    -Xclang-linker -Wno-incompatible-sysroot \
    target/$RUST_TARGET/debug/libphone_state.a \
    -o $APP/Meridian \
    $BINDINGS/phone_state.swift \
    swift/Sources/MeridianApp/*.swift

# What the editor needs to understand the same files, written from the same
# flags that just built them so the two cannot drift. sourcekit-lsp reads it
# from the repository root, which is where every other tool looks too.
python3 - "$TARGET" "$SDK" <<'PY'
import json
import pathlib
import sys

target, sdk = sys.argv[1], sys.argv[2]
bindings = pathlib.Path("swift/Sources/Meridian")

files = sorted(
    str(path)
    for directory in (bindings, pathlib.Path("swift/Sources/MeridianApp"))
    for path in directory.glob("*.swift")
)

arguments = [
    "swiftc",
    "-module-name", "Meridian",
    "-target", target,
    "-sdk", sdk,
    "-I", str(bindings),
    "-Xcc", f"-fmodule-map-file={bindings}/phone_stateFFI.modulemap",
    *files,
]

entries = [
    {"directory": str(pathlib.Path.cwd()), "arguments": arguments, "file": file}
    for file in files
]

with open("compile_commands.json", "w") as out:
    json.dump(entries, out, indent=4)
    out.write("\n")
PY

# Ad-hoc, because the simulator only asks that a signature exists. A real phone
# wants a development certificate, which is the one thing here that would need
# an Apple account.
codesign --force --sign - --timestamp=none $APP >/dev/null

# `boot` fails if it is already booted, which is the normal case on a re-run.
xcrun simctl boot "$DEVICE" 2>/dev/null || true

# Brings the window back if it was closed, and does nothing if it is open. It
# does not move a window that is already on another screen.
open -a Simulator

xcrun simctl install booted $APP
xcrun simctl launch booted $BUNDLE_ID >/dev/null

echo "running on $DEVICE"
