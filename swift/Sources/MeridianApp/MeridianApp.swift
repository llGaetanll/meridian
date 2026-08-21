/// The app's entry point.
///
/// Everything it does is start the runtime and hand it to the view. There is
/// no state here, and no decision — the first event goes in and the loop takes
/// over from there.

import SwiftUI

@main
struct MeridianApp: App {
    @StateObject private var runtime = RuntimeApp()

    var body: some Scene {
        WindowGroup {
            AppView()
                .environmentObject(runtime)
                .task { runtime.start() }
        }
    }
}
