/// The pieces more than one screen draws.
///
/// Every one of these takes a finished value from the core and prints it. If
/// you find yourself wanting a `String(format:)` or an `if amount < 0` in this
/// file, the field is missing from the view type — add it in `client-state`
/// instead. That rule is what keeps the two front-ends the same app.
///
/// Every row carries its own inset rather than sitting in a padded column, so
/// that the rules between them can run the full width of the screen. A rule
/// that stops short of the edge reads as a third kind of line.

import SwiftUI

/// The inset every row is set to.
let pageInset: CGFloat = 20

/// One transaction. The figure sits on the merchant's baseline, at the size of
/// the day's net above it, and on the same right edge.
struct TxRow: View {
    let row: ViewTxRow

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 14) {
            VStack(alignment: .leading, spacing: 0) {
                Text(row.title)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                HStack(spacing: 7) {
                    Text(row.category)
                        .foregroundStyle(Theme.textMuted)

                    if row.pending {
                        Text("·").foregroundStyle(Theme.slate700)
                        Text("Pending").foregroundStyle(Theme.textFaint)
                    }
                }
                .font(.system(size: 12.5, weight: .medium))
                .padding(.top, 5)
            }

            Spacer(minLength: 8)

            Text(row.amount)
                .font(.system(size: 13.5, weight: .semibold, design: .monospaced))
                .foregroundStyle(row.isInflow ? Theme.up : Theme.text)
        }
        .padding(.horizontal, pageInset)
        .padding(.vertical, 14)
        // Pending — the whole row recedes, so it reads as one provisional
        // thing rather than a row with some grey parts.
        .opacity(row.pending ? 0.45 : 1)
        .contentShape(Rectangle())
        .onTapGesture { runtime.ui(.txSelected(id: row.id)) }
    }
}

/// One recurring stream: what it is called, when it comes back, what it costs.
struct StreamRow: View {
    let row: ViewStreamRow

    var body: some View {
        HStack(alignment: .top, spacing: 13) {
            VStack(alignment: .leading, spacing: 4) {
                Text(row.title)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                Text(row.sub)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)
            }

            Spacer(minLength: 8)

            Text(row.amount)
                .font(.system(size: 15, weight: .semibold, design: .monospaced))
        }
        .padding(.horizontal, pageInset)
        .padding(.vertical, 12)
    }
}

/// One account on the Connections screen.
struct AccountRow: View {
    let row: ViewAccountRow

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(row.name)
                    .font(.system(size: 14.5, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                Text(row.subtitle)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)

                if let utilisation = row.utilisation {
                    Text(utilisation)
                        .font(.system(size: 12.5))
                        .foregroundStyle(Theme.textFaint)
                }
            }

            Spacer(minLength: 8)

            // Owed is not red here: the minus sign says it.
            Text(row.amount)
                .font(.system(size: 14.5, weight: .semibold, design: .monospaced))
        }
        .padding(.horizontal, pageInset)
        .padding(.vertical, 15)
    }
}

/// One connection: the bank, then every account it brings in.
///
/// The mark is the bank glyph rather than the bank's initials: two letters in
/// a square read as a monogram nobody chose, and four of them down a page read
/// as four. No total under the rows either — a login that reports a current
/// account and a mortgage has no figure worth adding them into.
struct ConnectionBlock: View {
    let connection: ViewConnection

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 11) {
                Icon(name: "bank", size: 21)
                    .foregroundStyle(Theme.textFaint)
                    .frame(width: 24, height: 24)

                Text(connection.name)
                    .font(.system(size: 13.5, weight: .semibold))
                    .foregroundStyle(Theme.textMuted)
                    .lineLimit(1)

                Spacer(minLength: 8)

                // The sign-in and the sync time take the same place on the
                // line, because they answer the same question.
                if connection.needsReconnect {
                    ReconnectButton(connection: connection)
                } else {
                    Text(connection.synced)
                        .font(.system(size: 12.5))
                        .foregroundStyle(Theme.textFaint)
                }
            }
            .padding(.horizontal, pageInset)
            .padding(.bottom, 2)

            ForEach(connection.accounts, id: \.id) { account in
                AccountRow(row: account)
            }
        }
    }
}

/// The sign-in a stale connection needs: Plaid Link in update mode against
/// this bank, which repairs it rather than attaching another.
///
/// While the machine says the repair is under way the button waits, so a
/// second tap cannot open a second sheet.
struct ReconnectButton: View {
    let connection: ViewConnection

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Button {
            runtime.ui(.bankReconnectRequested(itemId: connection.itemId))
        } label: {
            HStack(spacing: 7) {
                Icon(name: connection.reconnecting ? "spin" : "ext", size: 14, weight: 1.9)
                Text(connection.reconnectLabel)
            }
        }
        .buttonStyle(.quiet)
        .foregroundStyle(Theme.amber300)
        .disabled(connection.reconnecting)
        .opacity(connection.reconnecting ? 0.55 : 1)
    }
}
