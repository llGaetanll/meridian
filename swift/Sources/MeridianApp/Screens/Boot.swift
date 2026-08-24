/// What the app shows before it has anything to show.
///
/// The mark arrives first, in the middle of the screen, and the name comes in
/// beside it — the pair staying centred as it widens, so the mark drifts left
/// rather than the name pushing the two off balance.
///
/// It holds for a moment even when the snapshot is already back. A screen that
/// flashes for 90ms reads as a glitch rather than as the app opening, and the
/// first load off a warm server is exactly that fast.

import SwiftUI

struct BootScreen: View {
    /// Whether the name has joined the mark.
    @State private var named = false

    var body: some View {
        HStack(spacing: 14) {
            BrandMark(size: 54)

            if named {
                Text("Meridian")
                    .font(.system(size: 38, weight: .semibold))
                    .tracking(-0.76)
                    .fixedSize()
                    .transition(.opacity)
            }
        }
        .foregroundStyle(Theme.accentBright)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.bg)
        .task {
            try? await Task.sleep(for: .milliseconds(Self.markAlone))
            withAnimation(.easeOut(duration: 0.42)) { named = true }
        }
    }

    /// How long the mark is on its own before the name arrives.
    static let markAlone = 380

    /// How long the whole thing is up for, at the least.
    static let hold = 1250
}
