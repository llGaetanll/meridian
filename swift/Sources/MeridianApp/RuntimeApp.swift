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

    /// Start the app: send the first event.
    func start() {
        dispatch(.fromUi(event: .started))
    }

    /// Shorthand for the common case. A view's tap handler calls this; it
    /// never writes a published value itself.
    func ui(_ event: EventUi) {
        dispatch(.fromUi(event: event))
    }

    /// Send an event in, and carry out whatever comes back. One turn of the
    /// loop.
    private func dispatch(_ event: Event) {
        // The clock is read here, at the edge, and handed in. Nothing below
        // this line reads one — which is what makes a replay of the event log
        // land where it did the first time.
        let actions = core.handleEvents(now: Self.unixNow(), events: [event])

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
    /// Read at the edge and handed to the machine, which has none of its own.
    /// The server's clock rides in with the snapshot and is the one used for
    /// anything the user sees — a phone with a wrong date should not render a
    /// confidently wrong "in 3 days" — but the app has to draw something
    /// before the first snapshot lands.
    private static func unixNow() -> UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970))
    }
}
