/// Where the money sits.
///
/// The headline is what you hold, not net worth. Plaid reports the mortgage
/// but never the house behind it, and the student loan but not the degree, so
/// a net figure is structurally too red — opening the app should not read as
/// "you are $77k in the hole". Debts get their own section below.

import SwiftUI

struct BalancesScreen: View {
    let view: ViewBalances

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Hero(label: view.headlineLabel, value: view.headline)
                .bandAnchor()

            // What you hold is the same section as the headline above it, so
            // there is no rule between them — but it needs floor under it, or
            // the last block's total ends flush against the rule that starts
            // the next section.
            Blocks(connections: held)
                .padding(.bottom, 30)

            if !owed.isEmpty {
                Sec(title: view.owedLabel, value: view.owedTotal, valueColour: Theme.down) {
                    Blocks(connections: owed)
                }
            }
        }
    }

    private var held: [ViewConnection] { view.connections.filter { !$0.isDebts } }
    private var owed: [ViewConnection] { view.connections.filter(\.isDebts) }

    private struct Blocks: View {
        let connections: [ViewConnection]

        var body: some View {
            VStack(spacing: 0) {
                ForEach(Array(connections.enumerated()), id: \.element.id) { index, connection in
                    ConnectionBlock(connection: connection)
                        .padding(.top, index == 0 ? 0 : 30)
                }
            }
        }
    }
}

/// The headline, once it has scrolled away.
struct BalancesBand: View {
    let view: ViewBalances

    var body: some View {
        Band {
            HStack(spacing: 12) {
                Text(view.headlineLabel)
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.88)
                    .textCase(.uppercase)
                    .foregroundStyle(Theme.textFaint)

                Spacer(minLength: 8)

                Text(view.headline)
                    .font(.system(size: 15, weight: .bold, design: .monospaced))
            }
        }
    }
}
