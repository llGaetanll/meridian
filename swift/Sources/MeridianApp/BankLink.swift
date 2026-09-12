/// Plaid's own page, in a web session the app does not draw.
///
/// The bank sign-in is the one screen of first run that is not ours, and
/// deliberately: `docs/PLAID.md` is explicit that the secret signing every
/// Plaid call grants access to every Item on the account and so cannot ship
/// inside an app anybody can unzip. The server holds it, creates the Link
/// session, and hands back a URL. This opens it.
///
/// `ASWebAuthenticationSession` rather than a `SFSafariViewController` or a
/// `WKWebView`, for one reason that matters: it is the only one that reports
/// *both* endings the machine has an event for. The callback fires when Plaid
/// redirects to `meridian://linked`, and a cancellation error comes back when
/// the sheet is dismissed — and closing the sheet to go and find a password is
/// the ordinary way this ends, not a rare one.
///
/// It also means the session is a real browser session: cookies, autofill and
/// a password manager all work, which on a bank sign-in is the difference
/// between finishing and giving up.
///
/// No decisions and no state, like the rest of the IO half: one function that
/// answers whether the page reached its redirect. What that means is the
/// machine's.

import AuthenticationServices
import Foundation
import UIKit

@MainActor
enum BankLink {
    /// The scheme Plaid redirects to when the flow ends.
    ///
    /// It carries no token — the redirect's only job is to close the sheet,
    /// and the `public_token` is collected afterwards by the server, over a
    /// separate channel (`link_opener.rs` sets this as
    /// `completion_redirect_uri`). So there is nothing to read out of the
    /// callback URL, only the fact that it arrived.
    ///
    /// It has to match `APP_REDIRECT` on the server, and it has to be
    /// registered in `Info.plist` under `CFBundleURLTypes` — without that, iOS
    /// hands the redirect to nobody and the sheet never closes.
    static let scheme = "meridian"

    /// Open the page and wait for it to end.
    ///
    /// `true` means Plaid redirected, so a bank *may* be attached — whether
    /// one actually is, is the server's answer to `/link/finish` and not
    /// something this can know. `false` is the sheet being dismissed.
    static func open(_ page: URL) async -> Bool {
        await withCheckedContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: page,
                callbackURLScheme: scheme
            ) { _, error in
                // The URL is discarded on purpose: it carries no token, and
                // reading anything out of it would be inventing meaning the
                // redirect does not have.
                continuation.resume(returning: error == nil)
            }

            session.presentationContextProvider = anchor

            // A fresh session each time. The alternative shares Safari's
            // cookies, which is convenient right up until somebody is signed
            // into their bank on a phone they have handed to someone else.
            //
            // Untested against an OAuth bank: whether an ephemeral session
            // survives the trip out to the bank's own app and back is not
            // documented either way. If the handback lands on a session that
            // has lost its cookies, this is the line to suspect.
            session.prefersEphemeralWebBrowserSession = true

            if !session.start() {
                continuation.resume(returning: false)

                return
            }

            // After `start`, because a session that refused to open is not
            // one that is waiting for anything.
            waiting = session
        }
    }

    /// Whether a Link session is open, for the app to read when an OAuth
    /// bank's handback arrives.
    ///
    /// An OAuth bank leaves for its own app part way through the sign-in and
    /// returns by opening the Universal Link, which arrives at the *app* while
    /// the session above is still suspended waiting for its callback. Nothing
    /// carries it across: `ASWebAuthenticationSession` has `start`, `cancel`
    /// and `canStart`, and no way at all to give a running session a URL. So a
    /// handback the app receives is a handback the session cannot be told
    /// about, and whether the session finishes on its own on the far side of
    /// the bank's app is untested.
    ///
    /// Weak, so a session that ends by any other route is not kept alive by
    /// being remembered here — which is what makes a `nil` here mean the
    /// session is gone rather than merely forgotten.
    private(set) static weak var waiting: ASWebAuthenticationSession?

    /// Which window the sheet hangs off. Held rather than made per call, since
    /// `ASWebAuthenticationSession` keeps only a weak reference to it and a
    /// temporary one is deallocated before the sheet can ask.
    private static let anchor = Anchor()

    private final class Anchor: NSObject, ASWebAuthenticationPresentationContextProviding {
        func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
            let scene = UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .first

            // An app with no window has nothing to present from, which cannot
            // happen while somebody is tapping a button in it. A bare
            // `ASPresentationAnchor()` is what the API asks for in the absence
            // of one, and it fails the presentation rather than the process.
            return scene?.keyWindow ?? ASPresentationAnchor()
        }
    }
}
