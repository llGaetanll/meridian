/// Where the money sits.
///
/// The headline is what you hold, not net worth. Plaid reports the mortgage
/// but never the house behind it, and the student loan but not the degree, so
/// a net figure is structurally too red — opening the app should not read as
/// "you are $77k in the hole". Debts get their own blocks below.

import SwiftUI

struct BalancesScreen: View {
    let view: ViewBalances

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(view.headlineLabel)
                .font(.system(size: 11))
                .tracking(0.88)
                .textCase(.uppercase)
                .foregroundStyle(Theme.textFaint)

            Masked(
                value: view.headline,
                masked: view.masked,
                tappable: true,
                hero: true,
                size: 38
            )
            .padding(.top, 4)

            ForEach(Array(view.connections.enumerated()), id: \.element.id) {
                index, connection in
                ConnectionBlock(connection: connection)
                    .padding(.top, index == 0 ? 8 : 22)
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 16)
    }
}
