/// What is left of the first-run stub: the bank link, and the sync behind it.
///
/// **Everything here is standing in for a call that is not wired yet.** The
/// account half is real — `/auth/request` and `/auth/verify` are live and
/// `RuntimeApp` calls them — so what remains is Plaid: `/link/start` exists on
/// the server and nothing opens it, and the sync has no progress to report at
/// all (issue #25).
///
/// It stays one file for the same reason it always was. The machine decides
/// the whole flow, and each function here answers with the very event the real
/// call will, so wiring one is a matter of replacing a body and changing
/// nothing above it. `docs/PLAID.md` has the four calls the real link makes,
/// and the rule that goes with them: Sandbox unless the user says otherwise.
///
/// What it does *not* stand in for is any decision. Which screen follows an
/// abandoned link, what the tally says — all of that is in `phone-state` and
/// is real already.
///
/// `@MainActor` because `RuntimeApp` is, and because the attempt count below
/// would otherwise be shared mutable state.

import Foundation

@MainActor
enum FirstRun {
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
