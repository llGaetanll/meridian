#!/bin/sh
# Builds the iOS app and runs it on a physical phone. The sibling of
# ios-run.sh, which does the same for the simulator, and it differs in the four
# ways a real device differs:
#
#   - the phone's architecture rather than the Mac's, against the iphoneos SDK
#   - a real signature, from a development certificate and a provisioning
#     profile, where the simulator is content with an ad-hoc one
#   - the profile embedded in the bundle, which is how iOS knows the phone is
#     one this build is allowed to run on
#   - devicectl to install and launch, where the simulator has simctl
#
# The app reads its data from server-main. A phone cannot reach the Mac's
# loopback the way the simulator can, so the Mac's address on the LAN is
# written into the bundle's Info.plist here and the server has to be listening
# on the network rather than on loopback — it binds 0.0.0.0 for exactly this:
#
#     cargo run -p server-main
#
# Both machines have to be on the same network, and that address changes when
# the network does, which is why it is a build-time lookup rather than a
# constant anywhere in the tree.
#
# Needs: a paired phone with Developer Mode on, a development certificate and a
# provisioning profile for the bundle id. docs/GETTING_STARTED.md has all
# three.

set -e

cd "$(dirname "$0")/.."

# Which phone. Any identifier `xcrun devicectl list devices` prints works — a
# name, a UDID, an ECID. With one phone paired the default finds it.
#
# Matched by shape rather than by column, because three of the columns are free
# text that can hold spaces — the name, the state ("connected" or "available
# (paired)"), and the model, which prints as "iPhone 14 Pro (iPhone15,2)" on
# some versions and "iPhone15,2" on others. Counting fields from either end
# picks a different one depending on the phone. The identifier is the only
# column with a fixed shape, so that is what is looked for.
DEVICE=${1:-$(xcrun devicectl list devices --hide-headers 2>/dev/null \
    | grep -oE '[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}' \
    | head -1)}

BUNDLE_ID=com.meridian.phone
DEPLOYMENT=17.0
BINDINGS=swift/Sources/Meridian
APP=target/ios-device/Meridian.app

# A separate directory from the simulator's, because the two bundles hold
# binaries for different architectures under the same name and whichever ran
# last would otherwise be the one on disk.
TARGET=arm64-apple-ios$DEPLOYMENT
RUST_TARGET=aarch64-apple-ios

if [ -z "$DEVICE" ]; then
    echo "no phone found. Plug it in, unlock it, and trust this Mac." >&2
    echo "\`xcrun devicectl list devices\` is what this reads." >&2
    exit 1
fi

# Where the Mac is on the LAN. Wi-Fi first and Ethernet second, which is the
# order a laptop is usually on. MERIDIAN_SERVER overrides the whole address for
# the case where the server is somewhere else entirely.
if [ -z "$MERIDIAN_SERVER" ]; then
    HOST=$(ipconfig getifaddr en0 || ipconfig getifaddr en1 || true)

    if [ -z "$HOST" ]; then
        echo "no LAN address on en0 or en1 — is the Mac on a network?" >&2
        echo "set MERIDIAN_SERVER=http://host:8082 to name it yourself." >&2
        exit 1
    fi

    MERIDIAN_SERVER=http://$HOST:8082
fi

# The signing identity. A free Apple ID issues one called "Apple Development",
# the same name a paid account gets, so the search is by name either way.
IDENTITY=$(security find-identity -v -p codesigning \
    | grep -m1 "Apple Development" \
    | awk '{ print $2 }')

if [ -z "$IDENTITY" ]; then
    echo "no Apple Development certificate in the keychain." >&2
    echo "docs/GETTING_STARTED.md explains how to get one — it is the one" >&2
    echo "step that needs Xcode itself, and it is once per machine." >&2
    exit 1
fi

# The provisioning profile: which phones this build may run on, and which
# entitlements it may claim. Where it lives is ios-profile.sh's business,
# because ios-provision.sh needs the same answer.
PROFILE=$(scripts/ios-profile.sh "$BUNDLE_ID" || true)

if [ -z "$PROFILE" ]; then
    echo "no provisioning profile for $BUNDLE_ID." >&2
    echo "run ./scripts/ios-provision.sh to get one." >&2
    exit 1
fi

# A profile carries its own expiry, and a free account's lasts seven days. The
# failure it causes on the phone is "Unable to Install", which names neither
# the profile nor the date, so it is worth saying here instead.
EXPIRY=$(security cms -D -i "$PROFILE" | plutil -extract ExpirationDate raw -)

if [ "$(date -u +%Y-%m-%dT%H:%M:%SZ)" ">" "$EXPIRY" ]; then
    echo "the provisioning profile expired on $EXPIRY." >&2
    echo "re-run ./scripts/ios-provision.sh to get a fresh one." >&2
    exit 1
fi

SDK=$(xcrun --sdk iphoneos --show-sdk-path)

cargo build -p phone-state --target $RUST_TARGET

rm -rf $APP
mkdir -p $APP

cp swift/Support/Info.plist $APP/Info.plist

# The app icon. `swift scripts/gen-icon.swift` redraws the PNG the catalogue
# holds; actool turns the catalogue into the Assets.car iOS reads.
xcrun actool swift/Support/Assets.xcassets \
    --compile $APP \
    --platform iphoneos \
    --minimum-deployment-target $DEPLOYMENT \
    --app-icon AppIcon \
    --output-partial-info-plist $APP/icon.plist \
    --output-format human-readable-text >/dev/null

# actool writes the keys that say which sizes ended up in there; they belong
# beside the rest rather than in a file of their own.
python3 - "$APP" <<'PLIST'
import plistlib
import pathlib
import sys

app = pathlib.Path(sys.argv[1])
partial = app / "icon.plist"

with open(app / "Info.plist", "rb") as f:
    info = plistlib.load(f)

with open(partial, "rb") as f:
    info.update(plistlib.load(f))

with open(app / "Info.plist", "wb") as f:
    plistlib.dump(info, f)

partial.unlink()
PLIST

# The one thing about the bundle that is not the same on every machine. It goes
# into the copy under target/ rather than the source, so a build never leaves
# this Mac's address in the tree.
plutil -replace MeridianServer -string "$MERIDIAN_SERVER" $APP/Info.plist

# The same compile as the simulator's, differing only in the two flags that say
# which machine it is for. See ios-run.sh for why the core is named as an
# archive and why the sysroot warning is silenced.
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
    $(find swift/Sources/MeridianApp -name '*.swift')

# compile_commands.json is deliberately not rewritten here. The editor wants
# one answer about which SDK these files are compiled against, and the
# simulator's is the one that is right nearly all the time; a device build
# every so often should not flip it back and forth.

# The profile goes in the bundle under a fixed name, which is where iOS looks
# for it. Without it the phone refuses the app without saying why.
cp "$PROFILE" $APP/embedded.mobileprovision

# The entitlements have to be handed to codesign separately, and they have to
# agree with the profile's — a signature claiming anything the profile does not
# grant is rejected at install. Taking them straight out of the profile is what
# guarantees they agree.
ENTITLEMENTS=target/ios-device/entitlements.plist

security cms -D -i "$PROFILE" \
    | plutil -extract Entitlements xml1 -o "$ENTITLEMENTS" -

codesign --force --sign "$IDENTITY" --entitlements "$ENTITLEMENTS" --timestamp=none $APP

# `--terminate-existing` so a re-run replaces the running app rather than
# failing because it is already up, which is the normal case here.
xcrun devicectl device install app --device "$DEVICE" $APP
xcrun devicectl device process launch --device "$DEVICE" --terminate-existing $BUNDLE_ID >/dev/null

echo "running on $DEVICE, pointed at $MERIDIAN_SERVER"
