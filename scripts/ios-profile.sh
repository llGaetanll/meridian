#!/bin/sh
# Prints the path of the newest provisioning profile for a bundle id, and
# nothing at all when there is none.
#
#     ./scripts/ios-profile.sh com.meridian.phone
#
# Its own script because two things need the answer — ios-device.sh, which
# embeds the profile in the bundle it signs, and ios-provision.sh, which says
# what it just got — and where profiles live has already moved once. One place
# decides, so the two cannot come to different conclusions about it.

set -e

BUNDLE_ID=$1

if [ -z "$BUNDLE_ID" ]; then
    echo "usage: $0 <bundle-id>" >&2
    exit 2
fi

# Two directories because Xcode 16 moved them. It reads both, and a Mac that
# has had more than one Xcode on it has profiles in both, so both are searched
# rather than picking one and being wrong on half the machines.
PROFILES=$HOME/Library/MobileDevice/Provisioning\ Profiles
PROFILES_XCODE=$HOME/Library/Developer/Xcode/UserData/Provisioning\ Profiles

# Profiles are stored by UUID rather than by what they are for, so finding the
# right one means decoding each and looking at the bundle id it names.
#
# `security cms -D` is what turns the signed profile back into a plist. The
# newest match wins, because a re-issued profile lands beside the one it
# replaces rather than on top of it — free accounts re-issue weekly, so there
# are usually several.
PROFILE=

for candidate in "$PROFILES"/*.mobileprovision "$PROFILES_XCODE"/*.mobileprovision; do
    [ -e "$candidate" ] || continue

    id=$(security cms -D -i "$candidate" 2>/dev/null \
        | plutil -extract Entitlements.application-identifier raw - 2>/dev/null) || continue

    case "$id" in
        *".$BUNDLE_ID")
            if [ -z "$PROFILE" ] || [ "$candidate" -nt "$PROFILE" ]; then
                PROFILE=$candidate
            fi
            ;;
    esac
done

[ -n "$PROFILE" ] || exit 1

printf '%s\n' "$PROFILE"
