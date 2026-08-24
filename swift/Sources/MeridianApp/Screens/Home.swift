/// Home: the highlight of each tab and nothing more.
///
/// Every block is a door to the page that owns the detail. No "see all" link —
/// the dock is right there, and the block's title already says where this
/// lives.

import SwiftUI

struct HomeScreen: View {
    let view: ViewHome

    var body: some View {
        VStack(spacing: 12) {
            Block(title: "Balances", screen: .balances) {
                Masked(
                    value: view.balances.headline,
                    masked: view.balances.masked,
                    tappable: true,
                    size: 27
                )

                Text(view.balances.detail)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)
                    .padding(.top, 3)
            }

            Block(title: "Recurring", screen: .recurring) {
                HStack(alignment: .firstTextBaseline, spacing: 5) {
                    Masked(
                        value: view.recurring.headline,
                        masked: view.recurring.masked,
                        tappable: true,
                        size: 27
                    )

                    Text("/mo")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(Theme.textFaint)
                }

                Breakdown(slices: view.recurring.breakdown, legend: true, compact: true)
                    .padding(.top, 12)
                    .padding(.bottom, 9)

                Text(view.recurring.detail)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)

                ForEach(Array(view.recurring.upcoming.enumerated()), id: \.element.id) {
                    index, row in
                    Rule(first: index == 0)
                    StreamRow(row: row, compact: true)
                }
            }

            Block(title: "Ledger", screen: .ledger) {
                ForEach(Array(view.ledger.enumerated()), id: \.element.id) { index, row in
                    Rule(first: index == 0)
                    TxRow(row: row, compact: true)
                }
            }

            // Only drawn when something actually needs attention — a
            // healthy connection is not news.
            if !view.attention.isEmpty {
                Block(title: "Connections") {
                    ForEach(view.attention, id: \.id) { connection in
                        ConnectionBlock(connection: connection, compact: true)
                    }
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 6)
    }
}

/// One block, named for the tab it previews and tapping through to it.
private struct Block<Content: View>: View {
    let title: String

    /// `nil` for a block that is not a preview of a screen — Connections is
    /// the only one, because there is no connections tab.
    var screen: Screen?

    @ViewBuilder let content: Content

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.88)
                .textCase(.uppercase)
                .foregroundStyle(Theme.textFaint)
                .padding(.bottom, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
                .onTapGesture {
                    if let screen {
                        runtime.ui(.screenOpened(screen: screen))
                    }
                }

            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: Theme.radiusLg))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusLg).stroke(Theme.border, lineWidth: 1)
        )
    }
}

/// The hairline between rows inside a block. The first row does not get one —
/// it would read as a line under the block's title.
struct Rule: View {
    var first = false

    var body: some View {
        if !first {
            Rectangle()
                .fill(Theme.divider)
                .frame(height: 1)
        }
    }
}
