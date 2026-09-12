/// The app's entry point.
///
/// Everything it does is start the runtime and hand it to the view. There is
/// no state here, and no decision — the first event goes in and the loop takes
/// over from there.

import SwiftUI

/// The path an OAuth bank hands the sign-in back on.
///
/// The whole of what makes a link ours: iOS delivers every https link for
/// `api.meridiancash.io` this app claims, and the association file names this
/// one path. Checking it again here is what keeps a link the file may some day
/// also claim from being handed to a bank session that is not waiting for it.
private let handbackPath = "/link/oauth"

/// Record that an OAuth bank handed the sign-in back.
///
/// The link is dropped either way, because there is nothing to do with it:
/// `ASWebAuthenticationSession` offers no way to give a running session a URL,
/// and the Link session is the only thing that could finish the sign-in. All
/// this leaves is the log line, which says whether the session outlived the
/// trip to the bank's app — the one fact a device test of an OAuth bank needs
/// and the only one this can observe.
///
/// A link that is not the handback is not this flow and is dropped silently.
@MainActor
private func handback(_ url: URL) {
    guard url.path == handbackPath else { return }

    NSLog(
        "MERIDIAN handback, Link session %@",
        BankLink.waiting == nil ? "gone" : "open"
    )
}

@main
struct MeridianApp: App {
    @StateObject private var runtime = RuntimeApp()

    var body: some Scene {
        WindowGroup {
            AppView()
                .environmentObject(runtime)
                .task {
                    runtime.start()
                    HitProbe.run()
                }
                .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
                    guard let url = activity.webpageURL else { return }

                    handback(url)
                }
        }
    }
}

/// TEMPORARY: asks UIKit what is under each dock button, which is what a tap
/// would find. No input, no mouse — `hitTest` is the same lookup the touch
/// system does, run against the live hierarchy.
import UIKit

enum HitProbe {
    static func run() {
        // After the first layout pass, or there is nothing to hit yet.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) {
            guard
                let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                let window = scene.windows.first
            else {
                NSLog("MERIDIAN probe: no window")
                return
            }

            NSLog("MERIDIAN probe window bounds %@", NSCoderString(window.bounds))

            // The four dock buttons, and one transaction row as a control —
            // that one is known to work.
            let points: [(String, CGPoint)] = [
                ("home", CGPoint(x: 57, y: 787)),
                ("ledger", CGPoint(x: 129, y: 787)),
                ("recurring", CGPoint(x: 201, y: 787)),
                ("balances", CGPoint(x: 273, y: 787)),
                ("row-control", CGPoint(x: 280, y: 726)),
            ]

            for (name, p) in points {
                guard let hit = window.hitTest(p, with: nil) else {
                    NSLog("MERIDIAN probe %@ at %.0f,%.0f -> NOTHING", name, p.x, p.y)
                    continue
                }

                // The chain matters more than the leaf: it says which layer
                // took the touch.
                var chain: [String] = []
                var v: UIView? = hit
                while let cur = v, chain.count < 6 {
                    chain.append(String(describing: type(of: cur)))
                    v = cur.superview
                }

                NSLog(
                    "MERIDIAN probe %@ at %.0f,%.0f -> %@ | interaction=%@",
                    name, p.x, p.y, chain.joined(separator: " < "),
                    hit.isUserInteractionEnabled ? "on" : "OFF"
                )
            }
        }
    }
}

private func NSCoderString(_ r: CGRect) -> String {
    "\(Int(r.width))x\(Int(r.height))"
}

