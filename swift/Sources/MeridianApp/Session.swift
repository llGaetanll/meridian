/// Where the session token lives on the device.
///
/// The token is what every request to `server-main` carries — `Authorization:
/// Bearer …`, read by `session_token::from_header` — and it is good for ninety
/// days, so it is the one secret the app holds. That puts it in the keychain
/// rather than in `UserDefaults`, which is a plist that any file read or
/// unencrypted backup lifts whole.
///
/// It never reaches the state machine. The machine is told whether a session
/// exists and nothing more (`EventSource::SessionLoaded`), because a recorded
/// event log is something we want to be able to read off somebody's phone, and
/// a live bearer token in one would be a way into their bank. See
/// `docs/ARCHITECTURE.md`.
///
/// No decisions here, and no state: three functions over a store, the same
/// shape `RuntimeIo` has over the network.
///
/// ## Why there are two stores
///
/// **The simulator cannot use the keychain, and this is not a bug we can fix
/// in code.** An app reaching the keychain needs a `keychain-access-groups`
/// entitlement; an ad-hoc signature — which is all `scripts/ios-run.sh` has,
/// deliberately, so that running the app needs no Apple account — may not
/// claim one, and `taskgated` kills the app on launch for trying. A device
/// provisioning profile does not help either: it is issued against device
/// UDIDs and the simulator is not one of them. Without the entitlement every
/// `SecItem` call answers -34018, `errSecMissingEntitlement`.
///
/// So the simulator keeps the token in `UserDefaults` instead, chosen at
/// compile time. Compile time rather than a fallback when the keychain
/// refuses, because a fallback is a silent downgrade: a device build whose
/// profile was wrong would quietly write the token to a plist and nothing
/// would say so. This way the device binary does not contain that path at all.
///
/// It costs nothing real. A simulator's container is an ordinary directory on
/// the Mac, readable by anything the user runs, so its keychain would not have
/// been a boundary either — and the only tokens that reach it are sandbox
/// ones from a server on loopback.

import Foundation
import Security

#if targetEnvironment(simulator)
typealias Session = SessionDefaults
#else
typealias Session = SessionKeychain
#endif

/// The real one. Used by every build that runs on a phone.
enum SessionKeychain {
    /// The token this device holds, if it has one.
    ///
    /// `nil` covers both a device that never signed in and a keychain that
    /// refused to answer. They are the same thing to the caller: with no token
    /// in hand there is nothing to send, and first run is where that goes.
    static func token() -> String? {
        var query = base
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        guard status == errSecSuccess else {
            // Not found is the ordinary answer on a fresh install, so it is
            // not worth a line. Anything else is the keychain actually failing.
            if status != errSecItemNotFound {
                NSLog("MERIDIAN keychain: could not read the session (%d)", status)
            }

            return nil
        }

        guard let data = item as? Data, let token = String(data: data, encoding: .utf8) else {
            return nil
        }

        return token
    }

    /// Write it, replacing whatever was there.
    ///
    /// Delete-then-add rather than `SecItemUpdate`: the two paths differ only
    /// in whether an item happened to exist, which is not a difference worth
    /// branching on. Deleting something that is not there is not an error.
    static func save(_ token: String) {
        guard let data = token.data(using: .utf8) else { return }

        SecItemDelete(base as CFDictionary)

        var item = base
        item[kSecValueData as String] = data

        // Available once the phone has been unlocked after a reboot, and never
        // in a backup or on another device. A session belongs to the phone it
        // was signed in on: restoring one onto a new handset would hand it a
        // signed-in session that its owner never authorised there.
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly

        let status = SecItemAdd(item as CFDictionary, nil)

        if status != errSecSuccess {
            NSLog("MERIDIAN keychain: could not save the session (%d)", status)
        }
    }

    /// Forget it, which is how a device signs out or is sent back through
    /// first run.
    static func forget() {
        SecItemDelete(base as CFDictionary)
    }

    /// What identifies our one item. A generic password, under the bundle id,
    /// with a fixed account name — there is only ever one session per device.
    private static var base: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Bundle.main.bundleIdentifier ?? "com.meridian.phone",
            kSecAttrAccount as String: "session",
        ]
    }
}

/// The simulator's. Same three functions, no protection — see the note above
/// for why that is the only thing available here, and why it is acceptable.
///
/// `xcrun simctl uninstall booted com.meridian.phone` is how first run is
/// walked again, since this survives a reinstall of the app over itself.
enum SessionDefaults {
    static func token() -> String? {
        UserDefaults.standard.string(forKey: key)
    }

    static func save(_ token: String) {
        UserDefaults.standard.set(token, forKey: key)
    }

    static func forget() {
        UserDefaults.standard.removeObject(forKey: key)
    }

    private static let key = "MeridianSession"
}
