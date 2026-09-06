/// Talking to the server. No decisions and no state — async functions over
/// plain data, exactly as `docs/ARCHITECTURE.md` asks.
///
/// Every call the app makes, and the list `docs/API_FLOW.md` is written from.
/// It hands the snapshot back as a string rather than parsing it: the machine
/// parses, so every platform gets the same parse and there is only one place
/// that decides what a missing field means.
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
    /// code. The plist names the hosted server, which is what ships. A build
    /// pointed at a laptop is the exception — the simulator reaches the Mac's
    /// own loopback, a phone has to name the Mac's address on the LAN, and
    /// that address changes with the network — so `scripts/ios-run.sh` and
    /// `scripts/ios-device.sh` write it into the plist at build time.
    ///
    /// The fallback is the hosted server, because a bundle built without the
    /// key is a bundle nobody pointed anywhere.
    static let server = Bundle.main.object(forInfoDictionaryKey: "MeridianServer") as? String
        ?? "https://api.meridiancash.io"

    /// The snapshot, as JSON.
    ///
    /// Throws rather than returning a `Result`: the caller turns either
    /// outcome into an event, and Swift's error handling is what a caller
    /// expects to write against.
    ///
    /// The token is required rather than optional. `/snapshot` is somebody's
    /// bank transactions and has no unauthenticated form, so a call without
    /// one is a bug here rather than a request the server might answer.
    ///
    /// It asks for nothing but the ledger. There is no way to demand a trip to
    /// Plaid, and there used to be one: pull-to-refresh sent `?refresh=true`,
    /// which skipped the server's freshness window. When to go back to a bank
    /// is the server's decision, and a gesture anybody can repeat is not.
    static func snapshot(token: String) async throws -> String {
        guard let url = URL(string: "\(server)/snapshot") else {
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

    /// Ask what the Link session came to.
    ///
    /// Three answers, not two. The redirect that closed the sheet says nothing
    /// on its own — Plaid is explicit that reaching it is not a successful
    /// connection — so the server is the only party that sees both endings,
    /// and it may not have seen the second one yet. `pending` is that, and it
    /// is a real answer rather than a polite no.
    ///
    /// None of them names the bank. Nothing knows what one is called until the
    /// balances call inside its first sync answers — `syncStatus` is where the
    /// name turns up.
    enum Outcome {
        /// At least one bank is attached. The count is the server's; the
        /// machine does not read it, because one bank and two lead to the same
        /// screen.
        case linked

        /// The session ended without one. Plaid said so.
        case abandoned

        /// Plaid has not said. The bank may still be landing.
        case pending
    }

    static func finishLink(token: String) async throws -> Outcome {
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

        switch body.outcome {
        case "linked": return .linked
        case "pending": return .pending

        // Including a word this build has never heard of. A server saying
        // something newer than the app is not a reason to leave somebody on a
        // spinner, and every other outcome ends the same way.
        default: return .abandoned
        }
    }

    private struct Started: Decodable {
        let url: String
    }

    private struct Finished: Decodable {
        let outcome: String
    }

    /// How the sync is going, as JSON, for as long as it goes on.
    ///
    /// A response that never ends: the server holds it open and writes a
    /// message whenever the sync moves. iOS has no `EventSource`, so this
    /// reads the body as it arrives and splits it into Server-Sent Events by
    /// hand, which is most of what the format is.
    ///
    /// Each status is handed back as a string rather than parsed, the same as
    /// the snapshot: the machine parses, so all three platforms get one parse
    /// and one place that decides what a missing field means.
    ///
    /// The sequence ending is a fact and not a failure — the connection went,
    /// which happens every time the app is backgrounded. `RuntimeApp` reports
    /// it, and the machine decides whether to open another.
    static func syncStatuses(token: String) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let reading = Task {
                do {
                    try await readStatuses(token: token, into: continuation)

                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }

            // Letting go of the sequence closes the connection, which is what
            // frees the slot the server is holding for it.
            continuation.onTermination = { _ in reading.cancel() }
        }
    }

    private static func readStatuses(
        token: String,
        into continuation: AsyncThrowingStream<String, Error>.Continuation
    ) async throws {
        guard let url = URL(string: "\(server)/sync/status") else {
            throw Failure.unreachable
        }

        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")

        // No timeout of its own. A first sync of a real bank is minutes of
        // quiet punctuated by three messages, and what keeps the connection up
        // through it is the server's keep-alive every fifteen seconds — well
        // inside the sixty this would otherwise give up after. Which is the
        // right way round: if the heartbeat stops, the connection *should*
        // time out, and the machine opens another.

        let bytes: URLSession.AsyncBytes
        let response: URLResponse

        do {
            (bytes, response) = try await URLSession.shared.bytes(for: request)
        } catch {
            throw Failure.unreachable
        }

        guard let response = response as? HTTPURLResponse else {
            throw Failure.unreachable
        }

        // Refused before any of the body went out, which is the whole reason
        // the server answers the open as an ordinary request.
        guard (200..<300).contains(response.statusCode) else {
            throw Failure.answered(status: response.statusCode)
        }

        // The kind of message the last `event:` line named. Kept across lines
        // because it arrives before the data it is about.
        var kind = ""

        for try await line in bytes.lines {
            // The keep-alive, so that a quiet connection is not reaped.
            if line.hasPrefix(":") {
                continue
            }

            if let named = field("event", of: line) {
                kind = named

                continue
            }

            guard let json = field("data", of: line) else {
                continue
            }

            // A message is one `data:` line here, which is what lets this
            // dispatch on arrival: `AsyncLineSequence` drops the blank line
            // that ends a message, so it is not something to wait for.
            //
            // Dispatched on the name so a kind added later is skipped rather
            // than parsed as a status it cannot be read as.
            if kind.isEmpty || kind == "sync" {
                continuation.yield(json)
            }

            kind = ""
        }
    }

    /// The value of one Server-Sent Events field, or `nil` if the line is not
    /// that field. The space after the colon is optional and is dropped.
    private static func field(_ name: String, of line: String) -> String? {
        guard line.hasPrefix("\(name):") else {
            return nil
        }

        let value = line.dropFirst(name.count + 1)

        return String(value.hasPrefix(" ") ? value.dropFirst() : value)
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
