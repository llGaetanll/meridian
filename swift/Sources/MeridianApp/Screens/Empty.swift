/// A screen with nothing on it.
///
/// The frame is real — the header, the dock, the pull to refresh — and the
/// body is a placeholder. Every tab lands here while the screens are rebuilt
/// one at a time.

import SwiftUI

struct EmptyScreen: View {
    /// The big title. `nil` on Home, which shows the mark instead.
    var title: String?

    var body: some View {
        PhoneShell(title: title) {
            Text("Nothing here yet")
                .font(.system(size: 13.5))
                .foregroundStyle(Theme.textFaint)
                .frame(maxWidth: .infinity)
                .padding(.top, 120)
        }
    }
}
