/// Which banks are attached, and the way out.
///
/// Deliberately small, and `sketches/mobile/account.html` is why: everything a
/// setting could configure — the range, the categories, what is covered —
/// already lives on the screen it affects, so what is left for this tab is the
/// connections and the session.
///
/// Two of the sketch's things are not drawn yet. There is no address at the
/// top, because nothing the server sends says who the device is signed in as;
/// and there is no bank screen behind a chevron, so the rows do not offer one.
/// Delete account is the sketch's third, and it is a server route that does
/// not exist — App Store review 5.1.1(v) wants it before this ships.

import SwiftUI

struct AccountScreen: View {
    let view: ViewAccount

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Sec(title: view.banksLabel, first: true) {
                if let message = view.banksEmpty {
                    Text(message)
                        .font(.system(size: 13))
                        .foregroundStyle(Theme.textFaint)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, pageInset)
                } else {
                    ForEach(Array(view.banks.enumerated()), id: \.element.id) { index, bank in
                        if index > 0 {
                            RowRule()
                        }

                        BankRow(bank: bank)
                    }
                }
            }

            Sec(title: view.sessionLabel) {
                // The one button on the tab, and it is quiet: there is no
                // thing-you-are-here-to-do on this screen, so nothing on it is
                // violet.
                Button(view.signOutLabel) { runtime.ui(.signOutRequested) }
                    .buttonStyle(.block)
                    .padding(.horizontal, pageInset)
            }

            // Nobody reads this until something is wrong, and then it is the
            // first thing we ask them for.
            Text(view.build)
                .font(.system(size: 11.5))
                .foregroundStyle(Theme.textFaint)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.top, 26)
        }
    }
}

/// One bank: its mark, its name, and whether it is still working.
private struct BankRow: View {
    let bank: ViewAccountBank

    var body: some View {
        HStack(spacing: 11) {
            Text(bank.initials)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(Theme.textFaint)
                .frame(width: 26, height: 26)
                .overlay(
                    RoundedRectangle(cornerRadius: Theme.radiusSm)
                        .stroke(Theme.borderStrong, lineWidth: 1)
                )

            VStack(alignment: .leading, spacing: 2) {
                Text(bank.name)
                    .font(.system(size: 14.5, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                // A bank that has stopped syncing is the one thing in the app
                // that stops the numbers being true, so the row says so in
                // colour rather than leaving it to be read.
                Text(bank.detail)
                    .font(.system(size: 12.5))
                    .foregroundStyle(bank.needsReconnect ? Theme.amber300 : Theme.textFaint)
            }

            Spacer(minLength: 8)
        }
        .padding(.horizontal, pageInset)
        .padding(.vertical, 13)
    }
}
