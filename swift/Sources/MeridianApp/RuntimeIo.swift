/// Talking to the server. No decisions and no state — async functions over
/// plain data, exactly as `docs/ARCHITECTURE.md` asks.
///
/// The transcription of `crates/phone-main/src/runtime_io.rs`, down to handing
/// the snapshot back as a string rather than parsing it. The machine parses,
/// so all three platforms get the same parse and there is only one place that
/// decides what a missing field means.
///
/// Nothing here reads the keychain, and nothing here decides what a failure
/// means. The token is a parameter because these are plain functions over
/// plain data; `RuntimeApp` is what holds one and what turns an outcome into
/// an event.

import Foundation

enum RuntimeIo {
    /// Where `server-main` listens.
    ///
    /// Read out of the bundle rather than written here, because the answer
    /// differs by where the app is running and neither answer belongs to the
    /// code. The simulator shares the Mac's network, so loopback is the Mac's
    /// loopback and the plist carries the same `127.0.0.1:8082` the web
    /// front-end uses. A real phone has to name the Mac's address on the LAN,
    /// which changes with the network it is on, so `scripts/ios-device.sh`
    /// writes that address into the plist at build time.
    ///
    /// The fallback is the simulator's, because a bundle built without the key
    /// is a bundle built by the simulator script.
    static let server = Bundle.main.object(forInfoDictionaryKey: "MeridianServer") as? String
        ?? "http://127.0.0.1:8082"

    /// The snapshot, as JSON.
    ///
    /// Throws rather than returning a `Result`: the caller turns either
    /// outcome into an event, and Swift's error handling is what a caller
    /// expects to write against.
    ///
    /// The token is required rather than optional. `/snapshot` is somebody's
    /// bank transactions and has no unauthenticated form, so a call without
    /// one is a bug here rather than a request the server might answer.
    static func snapshot(force: Bool, token: String) async throws -> String {
        let path = force ? "\(server)/snapshot?refresh=true" : "\(server)/snapshot"

        guard let url = URL(string: path) else {
            throw Failure.unreachable
        }

        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        let (data, status) = try await send(request)

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }

        guard let json = String(data: data, encoding: .utf8) else {
            throw Failure.emptyBody
        }

        return json
    }

    /// Open a Link session, and answer with the page that hosts it.
    ///
    /// Plaid's own page over the web rather than an SDK, which is what keeps
    /// the bank sign-in out of this app entirely — nothing platform-specific
    /// is integrated, and no token of either kind is ever held here
    /// (`docs/PLAID.md`). Safe to call again if somebody backs out: a session
    /// nobody finishes costs nothing and expires on its own.
    static func startLink(token: String) async throws -> URL {
        guard let url = URL(string: "\(server)/link/start") else {
            throw Failure.unreachable
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        let (data, status) = try await send(request)

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }

        guard
            let body = try? JSONDecoder().decode(Started.self, from: data),
            let page = URL(string: body.url)
        else {
            throw Failure.emptyBody
        }

        return page
    }

    /// Collect whatever the Link session came to, and answer with how many
    /// banks it attached.
    ///
    /// A count rather than a yes: one call collects every session the caller
    /// has open, so it can attach more than one. Zero is the ordinary outcome
    /// of somebody closing the sheet.
    ///
    /// It does not name them. Nothing knows what a bank is called until the
    /// balances call inside its first sync answers — `syncStatus` is where
    /// the name turns up.
    static func finishLink(token: String) async throws -> UInt32 {
        guard let url = URL(string: "\(server)/link/finish") else {
            throw Failure.unreachable
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        let (data, status) = try await send(request)

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }

        guard let body = try? JSONDecoder().decode(Finished.self, from: data) else {
            throw Failure.emptyBody
        }

        return body.linked
    }

    private struct Started: Decodable {
        let url: String
    }

    private struct Finished: Decodable {
        let linked: UInt32
    }

    /// How far the first sync has got, as JSON.
    ///
    /// Handed back as a string rather than parsed, the same as the snapshot:
    /// the machine parses, so all three platforms get one parse and one place
    /// that decides what a missing field means.
    static func syncStatus(token: String) async throws -> String {
        guard let url = URL(string: "\(server)/sync/status") else {
            throw Failure.unreachable
        }

        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        let (data, status) = try await send(request)

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }

        guard let json = String(data: data, encoding: .utf8) else {
            throw Failure.emptyBody
        }

        return json
    }

    /// Ask for a sign-in code.
    ///
    /// The server answers 204 whether or not the address is one it knows —
    /// deliberately, so that nobody can ask it which addresses have accounts.
    /// So there is nothing to read out of a success, and the only thing this
    /// can report is that the request did not get through.
    static func requestCode(email: String) async throws {
        guard let url = URL(string: "\(server)/auth/request") else {
            throw Failure.unreachable
        }

        let (_, status) = try await send(post(url, ["email": email]))

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }
    }

    /// Trade a code for a session, and answer with the token.
    ///
    /// `nil` is the code being refused — the server gives one shape for wrong,
    /// expired and too many guesses, and this passes that through without
    /// dressing it up. It is not an error: a mistyped digit is the ordinary
    /// way this ends, and the flow has a screen for it. A throw here means the
    /// request never got an answer at all, which is a different screen.
    static func verifyCode(email: String, code: String) async throws -> String? {
        guard let url = URL(string: "\(server)/auth/verify") else {
            throw Failure.unreachable
        }

        let (data, status) = try await send(post(url, ["email": email, "code": code]))

        if status == 401 {
            return nil
        }

        guard (200..<300).contains(status) else {
            throw Failure.answered(status: status)
        }

        guard
            let body = try? JSONDecoder().decode(Verified.self, from: data)
        else {
            throw Failure.emptyBody
        }

        return body.token
    }

    /// What `/auth/verify` answers with.
    private struct Verified: Decodable {
        let token: String
    }

    /// A JSON POST, built the one way every call here builds one.
    private static func post(_ url: URL, _ body: [String: String]) -> URLRequest {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        return request
    }

    /// Send it, and hand back the body and the status.
    ///
    /// The status is returned rather than thrown on, because what a given
    /// status means differs by route — a 401 from `/auth/verify` is a mistyped
    /// digit, and a 401 from `/snapshot` is a session that has run out. Each
    /// caller says which it is.
    private static func send(_ request: URLRequest) async throws -> (Data, Int) {
        let data: Data
        let response: URLResponse

        do {
            (data, response) = try await URLSession.shared.data(for: request)
        } catch {
            // Every transport failure becomes the same sentence, as it does in
            // runtime_io.rs. URLSession's own wording — "Could not connect to
            // the server." — names neither the server nor the fix, and the one
            // the user needs is nearly always the same one.
            throw Failure.unreachable
        }

        // A response that is not HTTP cannot have come from a URL we built, so
        // there is no status to read and nothing sensible to assume.
        guard let response = response as? HTTPURLResponse else {
            throw Failure.unreachable
        }

        return (data, response.statusCode)
    }

    /// What went wrong, in the words the banner will print.
    ///
    /// The messages match `runtime_io.rs`'s, because the same failure should
    /// read the same way whichever front-end the user is looking at.
    enum Failure: LocalizedError {
        case unreachable
        case answered(status: Int)
        case emptyBody

        /// Asked for something that needs a session, with no token in hand.
        /// Not a network failure at all — it is the app having got ahead of
        /// itself, and it reads that way rather than blaming the server.
        case noSession

        var errorDescription: String? {
            switch self {
            case .unreachable:
                return "could not reach the server at \(server) — is `cargo run -p server-main` running?"
            case .answered(let status):
                return "the server answered \(status)"
            case .emptyBody:
                return "empty body"
            case .noSession:
                return "not signed in"
            }
        }
    }
}
