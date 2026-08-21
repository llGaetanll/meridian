/// Talking to the server. No decisions and no state — one async function over
/// plain data, exactly as `docs/ARCHITECTURE.md` asks.
///
/// The transcription of `crates/phone-main/src/runtime_io.rs`, down to handing
/// the snapshot back as a string rather than parsing it. The machine parses,
/// so all three platforms get the same parse and there is only one place that
/// decides what a missing field means.

import Foundation

enum RuntimeIo {
    /// Where `server-main` listens.
    ///
    /// The simulator shares the Mac's network, so loopback is the Mac's
    /// loopback and this is the same address the web front-end uses. A real
    /// phone will need somewhere else entirely, which is a problem for the day
    /// there is somewhere else to point at.
    static let server = "http://127.0.0.1:8082"

    /// The snapshot, as JSON.
    ///
    /// Throws rather than returning a `Result`: the caller turns either
    /// outcome into an event, and Swift's error handling is what a caller
    /// expects to write against.
    static func snapshot(force: Bool) async throws -> String {
        let path = force ? "\(server)/snapshot?refresh=true" : "\(server)/snapshot"

        guard let url = URL(string: path) else {
            throw Failure.unreachable
        }

        let data: Data
        let response: URLResponse

        do {
            (data, response) = try await URLSession.shared.data(from: url)
        } catch {
            // Every transport failure becomes the same sentence, as it does in
            // runtime_io.rs. URLSession's own wording — "Could not connect to
            // the server." — names neither the server nor the fix, and the one
            // the user needs is nearly always the same one.
            throw Failure.unreachable
        }

        if let response = response as? HTTPURLResponse, !(200..<300).contains(response.statusCode) {
            throw Failure.answered(status: response.statusCode)
        }

        guard let json = String(data: data, encoding: .utf8) else {
            throw Failure.emptyBody
        }

        return json
    }

    /// What went wrong, in the words the banner will print.
    ///
    /// The messages match `runtime_io.rs`'s, because the same failure should
    /// read the same way whichever front-end the user is looking at.
    enum Failure: LocalizedError {
        case unreachable
        case answered(status: Int)
        case emptyBody

        var errorDescription: String? {
            switch self {
            case .unreachable:
                return "could not reach the server at \(server) — is `cargo run -p server-main` running?"
            case .answered(let status):
                return "the server answered \(status)"
            case .emptyBody:
                return "empty body"
            }
        }
    }
}
