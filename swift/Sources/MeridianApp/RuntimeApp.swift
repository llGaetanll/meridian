/// The IO half of the app: everything effectful, and nothing that decides.
///
/// The pairing `docs/ARCHITECTURE.md` describes — a state machine that decides
/// (`phone_state::StateApp`, reached here as `Core`) and a runtime that acts.
/// The function structure mirrors the machine's: `performActions` fans out to
/// `performAction`, which fans out to one function per action group.
///
/// It holds the last screen the machine rendered and nothing else that the
/// machine could have decided. SwiftUI's `@Published` is playing the part
/// Leptos signals play in `crates/phone-main/src/runtime_app.rs`: plumbing
/// that redraws the view, not a place where behaviour is decided.
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

    /// Whether a fetch is in flight, for the refresh spinner. Deliberately not
    /// in the machine: it is a fact about the runtime rather than a decision,
    /// and the machine would have to be told when a request started in order
    /// to know it.
    @Published private(set) var loading = false

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
        case .onboarding: "first-run"
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
        case .resendRequested: "resend"
        case .backPressed: "back"
        case .bankConnectRequested: "bankConnect"
        case .onboardingFinished: "onboardingFinished"
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
        case .loadSnapshot(let force): loadSnapshot(force: force)

        // First run. Every one of these is answered by `FirstRun`, which is
        // standing in for a server that does not have these endpoints yet —
        // see the note on that type. Nothing above this line can tell the
        // difference, and nothing below it decides anything.
        case .loadSession: loadSession()
        case .saveSession(let email): FirstRun.remember(email)
        case .sendCode(let email): sendCode(to: email)
        case .verifyCode(_, let code): verifyCode(code)
        case .startBankLink: startBankLink()
        case .watchSync: watchSync()
        }
    }

    /// Who the device says is signed in.
    ///
    /// Read straight away and answered on the next turn, so that the actions
    /// from the batch that asked are all carried out before the answer starts
    /// another. The app draws its opening screen behind this, and the store is
    /// fast enough that nobody sees the one it guessed.
    private func loadSession() {
        let email = FirstRun.session

        Task { [weak self] in
            self?.dispatch(.fromSource(event: .sessionLoaded(email: email)))
        }
    }

    private func sendCode(to email: String) {
        NSLog("MERIDIAN first run: a code for %@ would be sent here", email)

        Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(500))

            self?.dispatch(.fromSource(event: .codeSent))
        }
    }

    private func verifyCode(_ code: String) {
        Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(400))

            self?.dispatch(.fromSource(event: FirstRun.verdict(on: code)))
        }
    }

    /// Ask the server for a `link_token` and open the `hosted_link_url` that
    /// comes back, which is Plaid's own page in a web session the app does not
    /// draw.
    private func startBankLink() {
        Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(900))

            self?.dispatch(.fromSource(event: FirstRun.link()))
        }
    }

    /// Follow the first sync. The work is the server's; this reports what
    /// lands.
    private func watchSync() {
        Task { [weak self] in
            for step in FirstRun.sync {
                try? await Task.sleep(for: .milliseconds(650))

                self?.dispatch(.fromSource(event: step))
            }
        }
    }

    /// The one piece of IO in the app.
    ///
    /// Whatever comes back re-enters the machine as an event, which is what
    /// closes the loop: nothing here inspects the answer or decides anything
    /// about it.
    private func loadSnapshot(force: Bool) {
        // Two refreshes at once would race to write the same state, and the
        // slower one would win. One at a time.
        if loading { return }

        loading = true

        Task { [weak self] in
            guard let self else { return }

            let event: EventSource

            do {
                event = .snapshotLoaded(json: try await RuntimeIo.snapshot(force: force))
            } catch {
                event = .loadFailed(
                    message: error.localizedDescription
                )
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
    /// The server's clock rides in with the snapshot and is the one used for
    /// anything the user sees — a phone with a wrong date should not render a
    /// confidently wrong "in 3 days" — but the app has to draw something
    /// before the first snapshot lands.
    private static func unixNow() -> UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970))
    }
}
