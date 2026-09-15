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

        if let refusal = refusal(status: status, body: data) {
            throw refusal
        }

        guard let json = String(data: data, encoding: .utf8) else {
            throw Failure.unreadable
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

        if let refusal = refusal(status: status, body: data) {
            throw refusal
        }

        guard
            let body = try? JSONDecoder().decode(Started.self, from: data),
            let page = URL(string: body.url)
        else {
            throw Failure.unreadable
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

        if let refusal = refusal(status: status, body: data) {
            throw refusal
        }

        guard let body = try? JSONDecoder().decode(Finished.self, from: data) else {
            throw Failure.unreadable
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
        // the server answers the open as an ordinary request. Its body is read
        // like any other refusal's: a 402 met here is somebody who reached the
        // bank step without paying, and which paywall they see is in it.
        if !(200..<300).contains(response.statusCode) {
            throw refusal(status: response.statusCode, body: await collect(bytes)) ?? .unreachable
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

    /// A refusal's body, out of a response that was opened as a stream.
    ///
    /// Capped, because this is reached only when the status already says the
    /// stream is not coming: a server answering a refusal with a megabyte is
    /// not something to read to the end.
    private static func collect(_ bytes: URLSession.AsyncBytes) async -> Data {
        var body = Data()

        do {
            for try await byte in bytes {
                body.append(byte)

                // `WireUnpaid` is two fields. Anything past this is not that.
                if body.count >= 4096 {
                    break
                }
            }
        } catch {
            // A body that stopped arriving is still a refusal. What did arrive
            // is what gets read, and an unreadable one is `never`.
        }

        return body
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

        let (data, status) = try await send(post(url, ["email": email]))

        if let refusal = refusal(status: status, body: data) {
            throw refusal
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

        if let refusal = refusal(status: status, body: data) {
            throw refusal
        }

        guard
            let body = try? JSONDecoder().decode(Verified.self, from: data)
        else {
            throw Failure.unreadable
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

    /// What a call came to, when it did not come to an answer.
    ///
    /// No sentences. What the user reads is the machine's — every platform
    /// gets one wording rather than three, and a sentence written here is one
    /// no `eval!` test can read.
    enum Failure: Error {
        /// Nothing came back at all.
        case unreachable

        /// The server answered, and this is what it said.
        case answered(status: Int)

        /// **402**, and the body that came with it. The one refusal that
        /// carries anything: a status code cannot tell somebody who has never
        /// subscribed from somebody whose card stopped working, and those are
        /// two screens.
        case unpaid(reason: Unpaid)

        /// An answer arrived and could not be read.
        case unreadable

        /// Asked for something that needs a session, with no token in hand.
        /// The app having got ahead of itself, or a session that has already
        /// been thrown away.
        case noSession

        /// The same fact, in the machine's vocabulary.
        ///
        /// Classification and not a verdict: which screen a 401 leads to is
        /// decided in `client-state`, and this only says which of these it was.
        var failure: LoadFailure {
            switch self {
            case .unreachable: return .offline
            case .unreadable: return .unreadable
            case .unpaid(let reason): return .unpaid(reason: reason)

            // Nothing to send means nothing the server would honour, which is
            // the same place a 401 leads.
            case .noSession: return .signedOut

            case .answered(let status):
                switch status {
                case 401: return .signedOut
                case 409: return .subscribed
                case 429: return .tooMany
                default: return .server(status: UInt16(clamping: status))
                }
            }
        }
    }

    /// What the server refused this with, or `nil` if it did not.
    ///
    /// Every route's `guard` in one place, so a refusal is read the same way
    /// whichever route met it — and so a 402's body is never the one route
    /// that forgot to look.
    private static func refusal(status: Int, body: Data) -> Failure? {
        if (200..<300).contains(status) {
            return nil
        }

        guard status == 402 else {
            return .answered(status: status)
        }

        return .unpaid(reason: unpaid(body))
    }

    /// Why a 402 was answered.
    ///
    /// A body we cannot read reads as `never`, which is the only one of the
    /// five that claims nothing about what came before — so an old app meeting
    /// a word it has never heard of says less rather than something untrue.
    private static func unpaid(_ body: Data) -> Unpaid {
        guard let said = try? JSONDecoder().decode(UnpaidBody.self, from: body) else {
            return .never
        }

        switch said.reason {
        case "declined": return .declined(ranOut: said.ran_out)
        case "ended": return .ended(ranOut: said.ran_out)
        case "unfinished": return .unfinished
        case "unconfirmed": return .unconfirmed
        default: return .never
        }
    }

    /// The JSON a 402 carries. `server_wire::WireUnpaid`, and the field names
    /// are that struct's — a rename there is a rename here, silently, because
    /// JSON has no type to disagree with.
    private struct UnpaidBody: Decodable {
        let reason: String
        let ran_out: UInt64?
    }

    /// Whatever was thrown, as the machine's vocabulary.
    ///
    /// Anything that is not one of ours came from URLSession or from a decoder
    /// and means the same thing as `unreachable`: nothing usable came back.
    static func classify(_ error: Error) -> LoadFailure {
        (error as? Failure)?.failure ?? .offline
    }
}
