/// A screen with nothing on it.
///
/// The frame around this is real — the header, the dock, the pull to refresh —
/// and lives in `AppView`, which keeps one of it for every screen. This is
/// only the body, which is what a screen is: every tab shows this one while
/// the screens are rebuilt one at a time.

import SwiftUI

struct EmptyScreen: View {
    var body: some View {
        Text("Nothing here yet")
            .font(.system(size: 13.5))
            .foregroundStyle(Theme.textFaint)
            .frame(maxWidth: .infinity)
            .padding(.top, 120)
    }
}
