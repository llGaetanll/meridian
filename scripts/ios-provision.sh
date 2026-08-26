#!/bin/sh
# Gets a fresh provisioning profile for the app. On a free Apple account that
# is a weekly errand, because the profile it issues lasts seven days.
#
# A profile says which phones a build may run on and which entitlements it may
# claim, and only Apple can issue one. Apple issues it to a *build* rather than
# to a request — there is no `xcrun` that just asks — so this writes a throwaway
# Xcode project carrying the app's bundle id, builds it against the phone, and
# keeps the profile that falls out. The stub compiles one file that does
# nothing and its product is thrown away; the real app is still built by
# ios-device.sh with swiftc and no project at all.
#
# Run it when ios-device.sh says the profile has expired, and when a new phone
# needs to be added — a profile names the devices it was cut for, so a phone
# that was not plugged in when it was issued is a phone it will not run on.
#
# Needs an Apple ID signed into Xcode and an Apple Development certificate in
# the keychain. Both are once per machine, and docs/GETTING_STARTED.md has
# them; this script is the part that repeats.

set -e

cd "$(dirname "$0")/.."

BUNDLE_ID=com.meridian.phone
DEPLOYMENT=17.0
STUB=target/ios-provision

# The certificate to issue the profile against. A free Apple ID names it
# "Apple Development", the same as a paid account does, so the search is by
# name either way — the same one ios-device.sh makes.
IDENTITY=$(security find-identity -v -p codesigning \
    | grep -m1 "Apple Development" \
    | sed -e 's/.*"\(.*\)".*/\1/')

if [ -z "$IDENTITY" ]; then
    # A certificate that is present but not *valid* is nearly always a missing
    # intermediate rather than a missing certificate, and the two need
    # completely different fixes. Worth telling them apart here, because
    # `find-identity -v` reports both as nothing at all.
    if security find-certificate -c "Apple Development" >/dev/null 2>&1; then
        echo "an Apple Development certificate is in the keychain, but nothing can" >&2
        echo "be signed with it — usually Apple's WWDR intermediate is missing or" >&2
        echo "expired, so the certificate cannot be chained to a root." >&2
        echo >&2
        echo "https://www.apple.com/certificateauthority/ has the current one." >&2
        echo "Download the WWDR certificate matching your own certificate's issuer" >&2
        echo "and \`security import\` it into the login keychain." >&2
    else
        echo "no Apple Development certificate in the keychain." >&2
        echo "docs/GETTING_STARTED.md explains how to get one." >&2
    fi

    exit 1
fi

# The team the profile belongs to, read off the certificate rather than written
# down here: it is a different ten characters for every person who checks this
# out, and the certificate is already the thing that decides.
TEAM=$(security find-certificate -c "$IDENTITY" -p \
    | openssl x509 -noout -subject -nameopt multiline \
    | awk '/organizationalUnitName/ { print $3 }')

if [ -z "$TEAM" ]; then
    echo "could not read a team id off '$IDENTITY'." >&2
    exit 1
fi

# The stub. Static, because everything that differs between people — the team
# and the bundle id — is passed to xcodebuild as a build setting below rather
# than written into the file.
rm -rf $STUB
mkdir -p $STUB/Stub.xcodeproj

cat > $STUB/main.swift <<'SWIFT'
import UIKit

class AppDelegate: UIResponder, UIApplicationDelegate {}

UIApplicationMain(
    CommandLine.argc,
    CommandLine.unsafeArgv,
    nil,
    NSStringFromClass(AppDelegate.self)
)
SWIFT

cat > $STUB/Stub.xcodeproj/project.pbxproj <<'PBXPROJ'
// !$*UTF8*$!
{
	archiveVersion = 1;
	classes = {};
	objectVersion = 56;
	objects = {
		1A0000000000000000000001 = {isa = PBXBuildFile; fileRef = 1A0000000000000000000002; };
		1A0000000000000000000002 = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = main.swift; sourceTree = "<group>"; };
		1A0000000000000000000003 = {isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = Stub.app; sourceTree = BUILT_PRODUCTS_DIR; };
		1A0000000000000000000010 = {isa = PBXSourcesBuildPhase; buildActionMask = 2147483647; files = ( 1A0000000000000000000001, ); runOnlyForDeploymentPostprocessing = 0; };
		1A0000000000000000000020 = {isa = PBXGroup; children = ( 1A0000000000000000000002, 1A0000000000000000000021, ); sourceTree = "<group>"; };
		1A0000000000000000000021 = {isa = PBXGroup; children = ( 1A0000000000000000000003, ); name = Products; sourceTree = "<group>"; };
		1A0000000000000000000030 = {isa = PBXNativeTarget; buildConfigurationList = 1A0000000000000000000041; buildPhases = ( 1A0000000000000000000010, ); buildRules = (); dependencies = (); name = Stub; productName = Stub; productReference = 1A0000000000000000000003; productType = "com.apple.product-type.application"; };
		1A0000000000000000000031 = {isa = PBXProject; attributes = { BuildIndependentTargetsInParallel = 1; }; buildConfigurationList = 1A0000000000000000000040; compatibilityVersion = "Xcode 14.0"; developmentRegion = en; hasScannedForEncodings = 0; knownRegions = ( en, ); mainGroup = 1A0000000000000000000020; productRefGroup = 1A0000000000000000000021; projectDirPath = ""; projectRoot = ""; targets = ( 1A0000000000000000000030, ); };
		1A0000000000000000000040 = {isa = XCConfigurationList; buildConfigurations = ( 1A0000000000000000000050, ); defaultConfigurationIsVisible = 0; defaultConfigurationName = Debug; };
		1A0000000000000000000041 = {isa = XCConfigurationList; buildConfigurations = ( 1A0000000000000000000051, ); defaultConfigurationIsVisible = 0; defaultConfigurationName = Debug; };
		1A0000000000000000000050 = {isa = XCBuildConfiguration; buildSettings = { SDKROOT = iphoneos; SWIFT_VERSION = 5.0; ONLY_ACTIVE_ARCH = YES; }; name = Debug; };
		1A0000000000000000000051 = {isa = XCBuildConfiguration; buildSettings = { CODE_SIGN_STYLE = Automatic; GENERATE_INFOPLIST_FILE = YES; PRODUCT_NAME = Stub; SWIFT_VERSION = 5.0; TARGETED_DEVICE_FAMILY = 1; }; name = Debug; };
	};
	rootObject = 1A0000000000000000000031;
}
PBXPROJ

# Which phone to cut the profile for. Asked of the project rather than of
# devicectl, because xcodebuild wants the identifier in its own spelling — the
# hardware UDID, which is not the one devicectl prints.
#
# `platform:iOS,` with the comma is what excludes the simulators, which spell
# it `platform:iOS Simulator`; the placeholder is the "Any iOS Device" entry,
# and a profile cannot be cut for a device that is not real.
DEVICE=$(xcodebuild -project $STUB/Stub.xcodeproj -scheme Stub -showdestinations 2>/dev/null \
    | grep 'platform:iOS,' \
    | grep -v placeholder \
    | sed -e 's/.*id:\([^,]*\).*/\1/' \
    | head -1)

if [ -z "$DEVICE" ]; then
    echo "no phone found. Plug it in, unlock it, and trust this Mac." >&2
    echo "a profile names the phones it was cut for, so one has to be here." >&2
    exit 1
fi

# `-allowProvisioningUpdates` is the whole point: it is the flag that lets
# xcodebuild register the bundle id and the phone with Apple and download what
# comes back. Without it the build fails rather than asking for anything.
xcodebuild \
    -project $STUB/Stub.xcodeproj \
    -scheme Stub \
    -destination "id=$DEVICE" \
    -allowProvisioningUpdates \
    DEVELOPMENT_TEAM="$TEAM" \
    PRODUCT_BUNDLE_IDENTIFIER="$BUNDLE_ID" \
    IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT" \
    build >/dev/null

PROFILE=$(scripts/ios-profile.sh "$BUNDLE_ID" || true)

if [ -z "$PROFILE" ]; then
    echo "the build succeeded but no profile for $BUNDLE_ID appeared." >&2
    echo "re-run without the \`>/dev/null\` on xcodebuild to see what it said." >&2
    exit 1
fi

EXPIRY=$(security cms -D -i "$PROFILE" | plutil -extract ExpirationDate raw -)

echo "profile for $BUNDLE_ID, team $TEAM, good until $EXPIRY"
echo "./scripts/ios-device.sh will pick it up."
