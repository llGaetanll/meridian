/// Which logins the app has, and whether they are working.
///
/// One block per login: the accounts under a bank are the ones it brings in,
/// held and owed together. A mortgage is a minus sign on a row of the bank
/// that reports it, not a section of its own — the page answers which logins
/// we have, and splitting a login in two answers that twice.

import SwiftUI

struct ConnectionsScreen: View {
    let view: ViewConnections

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The frame's title already says the page's name, so what stands
            // at the top here is the count and the way to add another.
            HStack(spacing: 12) {
                Text(view.count)
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.88)
                    .textCase(.uppercase)
                    .foregroundStyle(Theme.textFaint)

                Spacer(minLength: 8)

                Button { runtime.ui(.bankConnectRequested) } label: {
                    Text(view.addLabel)
                }
                .buttonStyle(.quiet)
                .disabled(view.adding)
                .opacity(view.adding ? 0.55 : 1)
            }
            .padding(.horizontal, pageInset)
            .padding(.top, 14)
            .padding(.bottom, 20)

            if let empty = view.empty {
                Text(empty)
                    .font(.system(size: 13.5))
                    .foregroundStyle(Theme.textFaint)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.horizontal, pageInset)
                    .padding(.vertical, 40)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(view.connections.enumerated()), id: \.element.id) {
                        index, connection in

                        ConnectionBlock(connection: connection)
                            .padding(.top, index == 0 ? 0 : 30)
                    }
                }
                .padding(.bottom, 30)
            }
        }
    }
}
