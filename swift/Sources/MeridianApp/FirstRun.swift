/// First run, answered on the device.
///
/// **Everything here is standing in for a server that does not have these
/// endpoints yet.** `server-main` serves `/snapshot` and `/health` and nothing
/// else, so there is no account to make, no code to send and no `link_token` to
/// exchange — and the screens that ask for all three are drawn and wired.
///
/// It is one file on purpose. The machine already decides the whole flow, and
/// each function here answers with the very event the real call will, so
/// growing the server is a matter of replacing the bodies below and changing
/// nothing above them. `docs/PLAID.md` has the four calls the real bank link
/// makes, and the rule that goes with them: Sandbox unless the user says
/// otherwise.
///
/// What it does *not* stand in for is any decision. Which screen follows a
/// rejected code, whether a countdown has run out, what the tally says — all of
/// that is in `phone-state` and is real already.
///
/// `@MainActor` because `RuntimeApp` is, and because the attempt count below
/// would otherwise be shared mutable state.

import Foundation

@MainActor
enum FirstRun {
    /// The address this device last completed first run as, if it has.
    ///
    /// `UserDefaults` rather than the keychain because there is no secret in it
    /// yet. When this becomes a session token it belongs in the keychain, and
    /// that is a change to these two lines.
    static var session: String? {
        UserDefaults.standard.string(forKey: sessionKey)
    }

    static func remember(_ email: String) {
        UserDefaults.standard.set(email, forKey: sessionKey)
    }

    /// Forget it, which is how first run is walked a second time.
    ///
    /// Nothing calls this yet — the app has no sign-out, because the Account
    /// screen it would live on is drawn in `sketches/mobile/account.html` and
    /// not built. Deleting the app from the simulator does the same thing.
    static func forget() {
        UserDefaults.standard.removeObject(forKey: sessionKey)
    }

    private static let sessionKey = "MeridianFirstRunEmail"

    /// What the stub makes of a code.
    ///
    /// Two reserved values, so that both failures are reachable by hand and
    /// neither screen is drawn once and never seen again: `000000` is wrong and
    /// `111111` is expired. Every other six digits are right, because there is
    /// no code to be right about until a code is really sent.
    static func verdict(on code: String) -> EventSource {
        switch code {
        case wrongCode: .codeRejected(expired: false)
        case expiredCode: .codeRejected(expired: true)
        default: .codeAccepted
        }
    }

    static let wrongCode = "000000"
    static let expiredCode = "111111"

    /// What Plaid's page came back with.
    ///
    /// The first attempt of a launch ends with nothing, and every one after it
    /// works. That is not a coin toss dressed up: closing the sheet to go and
    /// find a password is the ordinary way this ends, and a stub that always
    /// succeeded would leave the screen that says so drawn and never seen. Set
    /// `abandonsFirstAttempt` to `false` to go straight through.
    static func link() -> EventSource {
        attempts += 1

        if abandonsFirstAttempt && attempts == 1 {
            return .bankAbandoned
        }

        return .bankLinked(institution: institution)
    }

    static let abandonsFirstAttempt = true

    /// The Sandbox institution `crates/server-main/fixtures` was captured from.
    static let institution = "First Platypus Bank"

    private static var attempts = 0

    /// The first sync, page by page.
    ///
    /// The counts are the fixture's — 8 accounts, 48 transactions, 8 recurring
    /// streams — so what the last screen of the flow says matches what the
    /// Ledger behind it is about to show.
    static let sync: [EventSource] = [
        progress(.accounts, 0, 0, 0),
        progress(.accounts, 8, 0, 0),
        progress(.transactions, 8, 0, 0),
        progress(.transactions, 8, 19, 0),
        progress(.transactions, 8, 48, 0),
        progress(.streams, 8, 48, 0),
        progress(.streams, 8, 48, 8),
        .syncFinished,
    ]

    private static func progress(
        _ stage: SyncStage,
        _ accounts: UInt32,
        _ transactions: UInt32,
        _ streams: UInt32
    ) -> EventSource {
        .syncProgressed(
            stage: stage,
            accounts: accounts,
            transactions: transactions,
            streams: streams
        )
    }
}
