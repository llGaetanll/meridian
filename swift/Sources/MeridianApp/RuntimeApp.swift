/// The IO half of the app: everything effectful, and nothing that decides.
///
/// The pairing `docs/ARCHITECTURE.md` describes — a state machine that decides
/// (`phone_state::StateApp`, reached here as `Core`) and a runtime that acts.
/// The function structure mirrors the machine's: `performActions` fans out to
/// `performAction`, which fans out to one function per action group.
///
/// It holds the last screen the machine rendered and nothing else that the
/// machine could have decided. SwiftUI's `@Published` is plumbing that redraws
/// the view, not a place where behaviour is decided.
///
/// `@MainActor` because every field here drives the view. The `Core` behind it
/// is `Send + Sync` and has no opinion about threads, so the choice costs
/// nothing and means no state is touched from two places at once.

import Foundation

@MainActor
final class RuntimeApp: ObservableObject {
    /// The machine. Not published: reading state is what the view is not
    /// supposed to do. It is sent events and asked for actions.
    private let core = Core()

    /// The last screen the machine rendered, and the dock that came with it.
    @Published private(set) var screen: ViewScreen?
    @Published private(set) var dock: ViewDock?

    /// The last failure, for the banner. Cleared by the next successful load.
    @Published private(set) var error: String?

    /// The stream the sync's progress arrives on, held so it can be let go.
    ///
    /// The runtime's own bookkeeping, like `loading` below. Whether there
    /// should be one is the machine's — it says so with `watchSync` and
    /// `stopWatchingSync`; this is only the connection those two are about.
    private var watch: Task<Void, Never>?

    /// Whether a fetch is in flight, for the refresh spinner. Deliberately not
    /// in the machine: it is a fact about the runtime rather than a decision,
    /// and the machine would have to be told when a request started in order
    /// to know it.
    @Published private(set) var loading = false

    /// The session token, once there is one.
    ///
    /// Not published and never sent to the machine, which is told only that a
    /// session exists (`EventSource::SessionLoaded`). A bearer token is good
    /// for ninety days, and an event log with one in it is a way into
    /// somebody's bank — see `Session` and `docs/ARCHITECTURE.md`.
    ///
    /// It lives here from the moment `/auth/verify` answers, which is before
    /// the machine says to save it: `/link/start` needs it during first run,
    /// and only `OnboardingFinished` commits it to the keychain.
    private var token: String?

    /// The clock, once a second, for as long as the app is up.
    ///
    /// Held so it lives as long as the runtime does. It is the machine's only
    /// source of time: nothing else hands one in.
    private var clock: Timer?

    /// Start the app: tell the machine what time it is, then send the first
    /// event.
    func start() {
        tick()

        dispatch(.fromUi(event: .started))

        clock = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
    }

    /// The clock, read at the edge and sent in as an event like any other.
    ///
    /// A tick a second whatever is on screen: the machine decides which of
    /// them are worth drawing for, and today only the code screen's countdown
    /// is. Deciding that here would be behaviour in the runtime, which is the
    /// thing `docs/ARCHITECTURE.md` asks us not to have.
    private func tick() {
        dispatch(.fromRuntime(event: .tick(now: Self.unixNow())))
    }

    /// Shorthand for the common case. A view's tap handler calls this; it
    /// never writes a published value itself.
    func ui(_ event: EventUi) {
        // TEMPORARY tracing. `core` is the machine plus lifting the screen back
        // into Swift; `settled` is measured on the next turn of the main queue,
        // which is after SwiftUI has done the work the event caused. The gap
        // between them is the drawing.
        let t0 = Date()
        let before = screenName

        dispatch(.fromUi(event: event))

        let core = Date().timeIntervalSince(t0) * 1000
        let after = screenName

        DispatchQueue.main.async {
            let settled = Date().timeIntervalSince(t0) * 1000
            NSLog(
                "MERIDIAN tap event=%@ %@->%@ core=%.0fms settled=%.0fms",
                Self.name(event), before, after, core, settled
            )
        }
    }

    private var screenName: String {
        switch screen {
        case .ledger: "ledger"
        case .recurring: "recurring"
        case .balances: "balances"
        case .account: "account"
        case .onboarding: "first-run"
        case .paywall: "paywall"
        case nil: "none"
        }
    }

    private static func name(_ event: EventUi) -> String {
        switch event {
        case .started: "started"
        case .screenOpened(let screen): "screenOpened(\(screen))"
        case .refreshRequested: "refresh"
        case .maskToggled: "mask"
        case .filterGroupToggled: "filterGroup"
        case .categoryToggled(let id): "category(\(id))"
        case .categoriesCleared: "categoriesCleared"
        case .rangeSelected: "range"
        case .calendarDaySelected(let id): "calendarDay(\(id))"
        case .calendarMonthStepped(let step): "calendarMonth(\(step))"
        case .txSelected(let id): "tx(\(id))"
        case .emailChosen: "emailChosen"
        case .emailTyped: "emailTyped"
        case .emailSubmitted: "emailSubmitted"
        case .codeTyped: "codeTyped"
        case .codeSubmitted: "codeSubmitted"
        case .resendRequested: "resend"
        case .backPressed: "back"
        case .bankConnectRequested: "bankConnect"
        case .onboardingFinished: "onboardingFinished"
        case .signOutRequested: "signOut"
        }
    }

    /// Send an event in, and carry out whatever comes back. One turn of the
    /// loop.
    private func dispatch(_ event: Event) {
        let actions = core.handleEvents(events: [event])

        performActions(actions)
    }

    private func performActions(_ actions: [Action]) {
        for action in actions {
            performAction(action)
        }
    }

    private func performAction(_ action: Action) {
        switch action {
        case .toUi(let action): performActionUi(action)
        case .toSource(let action): performActionSource(action)
        }
    }

    private func performActionUi(_ action: ActionUi) {
        switch action {
        case .render(let screen, let dock):
            self.screen = screen
            self.dock = dock

        case .showError(let message):
            error = message
        }
    }

    private func performActionSource(_ action: ActionSource) {
        switch action {
        case .loadSnapshot: loadSnapshot()
        case .loadSession: loadSession()
        case .saveSession: saveSession()
        case .forgetSession: forgetSession()
        case .sendCode(let email): sendCode(to: email)
        case .verifyCode(let email, let code): verifyCode(email: email, code: code)

        // Still standing in for a server call — see `FirstRun`, which is what
        // is left of the stub. Nothing above this line can tell the
        // difference, and nothing below it decides anything.
        case .startBankLink: startBankLink()
        case .watchSync: watchSync()
        case .stopWatchingSync: stopWatchingSync()
        }
    }

    /// Whether the device has a session, read out of the keychain.
    ///
    /// Read straight away and answered on the next turn, so that the actions
    /// from the batch that asked are all carried out before the answer starts
    /// another. The app draws its opening screen behind this, and the keychain
    /// is fast enough that nobody sees the one it guessed.
    ///
    /// The token is kept here and the machine is told only that there is one.
    private func loadSession() {
        token = Session.token()

        let signedIn = token != nil

        Task { [weak self] in
            self?.dispatch(.fromSource(event: .sessionLoaded(signedIn: signedIn)))
        }
    }

    /// First run is through, so the token stops being something held in memory
    /// for the length of a flow and becomes what this device is signed in as.
    private func saveSession() {
        guard let token else {
            // Reachable only if the flow finished without a verified code,
            // which nothing in the machine allows. Worth a line rather than a
            // silent no-op, because the next launch would open on first run
            // again and there would be nothing saying why.
            NSLog("MERIDIAN first run finished with no token to save")

            return
        }

        Session.save(token)
    }

    /// Throw the session away.
    ///
    /// Both copies: the keychain's, so the next launch opens on first run, and
    /// the one held here, so nothing in flight can carry on using a token the
    /// user has just signed out of. The machine has already put itself back to
    /// where a fresh install starts.
    private func forgetSession() {
        token = nil

        Session.forget()

        // And the banner with it. It is the runtime's own bookkeeping, like
        // `loading`, and a "the server answered 401" left over the first
        // screen after signing back in would be reporting a session that no
        // longer exists.
        error = nil
    }

    /// Ask for a code. The server answers alike whether or not it knows the
    /// address, so there is nothing to read out of a success.
    private func sendCode(to email: String) {
        Task { [weak self] in
            do {
                try await RuntimeIo.requestCode(email: email)

                self?.dispatch(.fromSource(event: .codeSent))
            } catch {
                self?.dispatch(.fromSource(event: .loadFailed(failure: RuntimeIo.classify(error))))
            }
        }
    }

    /// Trade the code for a session.
    ///
    /// The token is kept and not announced: what goes to the machine is that
    /// the code was accepted. Whether the code was merely wrong or too late is
    /// the machine's to work out — the server refuses all three failures
    /// alike, on purpose, and the machine knows when it asked for the code.
    private func verifyCode(email: String, code: String) {
        Task { [weak self] in
            do {
                let verified = try await RuntimeIo.verifyCode(email: email, code: code)

                guard let self else { return }

                guard let verified else {
                    dispatch(.fromSource(event: .codeRejected))

                    return
                }

                token = verified

                dispatch(.fromSource(event: .codeAccepted))
            } catch {
                self?.dispatch(.fromSource(event: .loadFailed(failure: RuntimeIo.classify(error))))
            }
        }
    }

    /// The bank connection, end to end.
    ///
    /// Three steps and one event out of them: ask the server to open a Link
    /// session, put Plaid's page in front of the user, then tell the server
    /// the sheet has closed so it can collect whatever the session came to.
    ///
    /// One action carries all three because they are one thing to the machine
    /// — the tap was "connect a bank" and the answer is whether one is
    /// connected. Splitting them into three actions would put the order in the
    /// machine, which has no opinion about it and no way to be wrong about it.
    ///
    /// Nothing here decides. Dismissing the sheet and finishing with no bank
    /// both arrive as the same event, because both are the same fact: nothing
    /// was attached. A server that has not heard from Plaid yet is a third
    /// fact and arrives as a third event — it is not "no bank", and the flow
    /// has somewhere else to go with it.
    private func startBankLink() {
        guard let token else {
            NSLog("MERIDIAN asked to connect a bank with no session")

            dispatch(.fromSource(event: .bankAbandoned))

            return
        }

        Task { [weak self] in
            do {
                let page = try await RuntimeIo.startLink(token: token)

                // False is the sheet being dismissed, which is ordinary — and
                // there is nothing to collect, because the server only ever
                // had a session nobody finished.
                guard await BankLink.open(page) else {
                    self?.dispatch(.fromSource(event: .bankAbandoned))

                    return
                }

                switch try await RuntimeIo.finishLink(token: token) {
                case .linked: self?.dispatch(.fromSource(event: .bankLinked))
                case .abandoned: self?.dispatch(.fromSource(event: .bankAbandoned))
                case .pending: self?.dispatch(.fromSource(event: .bankPending))
                }
            } catch {
                // A failure to reach the server on either call leaves no bank
                // attached, which is the same screen as backing out — with the
                // reason in the banner behind it.
                self?.dispatch(.fromSource(event: .loadFailed(failure: RuntimeIo.classify(error))))
                self?.dispatch(.fromSource(event: .bankAbandoned))
            }
        }
    }

    /// Open the stream the sync's progress arrives on, and report what
    /// arrives on it.
    ///
    /// The loop here is the *reading* of one connection and not a decision to
    /// keep asking: the machine said to have a stream, and this is what having
    /// one looks like. When to open another after losing this one is the
    /// machine's, which is what keeps every attempt in the event log.
    ///
    /// Every way it can end — refused, dropped, backgrounded — is the same
    /// event, because none of them is this side's to interpret.
    private func watchSync() {
        guard let token else {
            NSLog("MERIDIAN asked to watch a sync with no session")

            dispatch(.fromSource(event: .syncWatchEnded))

            return
        }

        // The machine decides when there is one, so a second is a bug rather
        // than a race to live with.
        watch?.cancel()

        watch = Task { [weak self] in
            do {
                for try await json in RuntimeIo.syncStatuses(token: token) {
                    self?.dispatch(.fromSource(event: .syncStatusLoaded(json: json)))
                }
            } catch let refusal as RuntimeIo.Failure {
                // The server would not open the stream. Reported, because a
                // refusal swallowed here is what left the machine reopening a
                // stream it will never be given every three seconds.
                self?.dispatch(.fromSource(event: .loadFailed(failure: refusal.failure)))
            } catch {
                // The connection went after it had opened — backgrounded, out
                // of signal. Only the ending below is reported, because that
                // is the whole of what happened.
                NSLog("MERIDIAN sync stream ended: %@", error.localizedDescription)
            }

            // Not when the machine is the one that closed it: it knows, and a
            // stream it let go of is not a stream that went away.
            if !Task.isCancelled {
                self?.dispatch(.fromSource(event: .syncWatchEnded))
            }
        }
    }

    /// Let the stream go. Cancelling drops the response, which is what closes
    /// the connection and frees the slot the server is holding for it.
    private func stopWatchingSync() {
        watch?.cancel()
        watch = nil
    }

    /// The one piece of IO in the app.
    ///
    /// Whatever comes back re-enters the machine as an event, which is what
    /// closes the loop: nothing here inspects the answer or decides anything
    /// about it.
    private func loadSnapshot() {
        // Two refreshes at once would race to write the same state, and the
        // slower one would win. One at a time.
        if loading { return }

        loading = true

        Task { [weak self] in
            guard let self else { return }

            let event: EventSource

            do {
                // `/snapshot` has no unauthenticated form, so with nothing to
                // send there is no request to make. Reported rather than acted
                // on here: nothing to send is the same fact as a token the
                // server will not honour, and the machine puts first run back
                // for both.
                guard let token else {
                    throw RuntimeIo.Failure.noSession
                }

                event = .snapshotLoaded(
                    json: try await RuntimeIo.snapshot(token: token)
                )
            } catch {
                event = .loadFailed(failure: RuntimeIo.classify(error))
            }

            loading = false

            // A successful load clears whatever the last failure said.
            if case .snapshotLoaded = event {
                self.error = nil
            }

            dispatch(.fromSource(event: event))
        }
    }

    /// The wall clock, in seconds.
    ///
    /// Read at the edge and sent to the machine, which has none of its own.
    /// The device's is the only clock the app has: the server sends the
    /// timestamps of things — when a sync landed, when a stream is next due —
    /// and never a reading of its own.
    private static func unixNow() -> UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970))
    }
}
