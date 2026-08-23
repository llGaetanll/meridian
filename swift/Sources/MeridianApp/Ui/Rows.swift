/// The pieces more than one screen draws.
///
/// Every one of these takes a finished value from the core and prints it. If
/// you find yourself wanting a `String(format:)` or an `if amount < 0` in this
/// file, the field is missing from the view type — add it in `phone-state`
/// instead. That rule is what keeps the two front-ends the same app.

import SwiftUI

/// One transaction. The same row in the ledger and on Home, at two sizes.
struct TxRow: View {
    let row: ViewTxRow

    /// Home previews the row smaller, and shows the category as plain text: at
    /// preview scale a column of pills reads as noise.
    var compact = false

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        HStack(spacing: compact ? 11 : 13) {
            Logo(url: row.logoUrl, size: compact ? 32 : 40)

            VStack(alignment: .leading, spacing: compact ? 1 : 3) {
                Text(row.title)
                    .font(.system(size: compact ? 14 : 15, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                HStack(spacing: 7) {
                    if row.pending {
                        Text("Pending")
                            .font(.system(size: 10, weight: .semibold))
                            .tracking(0.6)
                            .foregroundStyle(Theme.amber300)
                    }

                    if compact {
                        Text(row.category)
                            .font(.system(size: 11.5))
                            .foregroundStyle(Theme.textFaint)
                    } else {
                        CategoryTag(label: row.category, token: row.categoryToken)
                    }
                }
            }

            Spacer(minLength: 8)

            Text(row.amount)
                .font(.system(size: compact ? 14 : 15, weight: .semibold))
                .monospacedDigit()
                .foregroundStyle(row.isInflow ? Theme.up : Theme.text)
        }
        .padding(.vertical, 9)
        .padding(.horizontal, compact ? 0 : 4)
        .frame(minHeight: 44)
        // Pending — the whole row recedes, chip included, so it reads as one
        // provisional thing rather than a row with some grey parts.
        .opacity(row.pending ? 0.45 : 1)
        .contentShape(Rectangle())
        .onTapGesture { runtime.ui(.txSelected(id: row.id)) }
    }
}

/// One recurring stream.
struct StreamRow: View {
    let row: ViewStreamRow

    var compact = false

    var body: some View {
        HStack(spacing: 13) {
            Logo(url: row.logoUrl)

            VStack(alignment: .leading, spacing: 4) {
                Text(row.title)
                    .font(.system(size: compact ? 14 : 15, weight: .semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)

                Text(row.when)
                    .font(.system(size: 11.5, weight: row.soon ? .semibold : .regular))
                    // Due within the week reads as a heads-up, not an alarm.
                    .foregroundStyle(row.soon ? Theme.amber300 : Theme.textFaint)
            }

            Spacer(minLength: 8)

            VStack(alignment: .trailing, spacing: 2) {
                Text(row.amount)
                    .font(.system(size: compact ? 14 : 15, weight: .semibold))
                    .monospacedDigit()

                Text(row.frequency)
                    .font(.system(size: 11))
                    .foregroundStyle(Theme.textFaint)
            }
        }
        .padding(.vertical, compact ? 9 : 12)
        .padding(.horizontal, compact ? 0 : 4)
    }
}

/// One account on the balances screen.
struct AccountRow: View {
    let row: ViewAccountRow

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(row.name)
                    .font(.system(size: 14, weight: .medium))
                    .lineLimit(1)
                    .truncationMode(.tail)

                Text(row.subtitle)
                    .font(.system(size: 11.5))
                    .foregroundStyle(Theme.textFaint)

                if let utilisation = row.utilisation {
                    Text(utilisation)
                        .font(.system(size: 11))
                        .foregroundStyle(Theme.textFaint)
                        .padding(.top, 1)
                }
            }

            Spacer(minLength: 8)

            Text(row.amount)
                .font(.system(size: 14, weight: .semibold))
                .monospacedDigit()
                .foregroundStyle(row.owed ? Theme.down : Theme.text)
        }
        .padding(.vertical, 10)
        .padding(.horizontal, 2)
    }
}

/// One connection: its accounts, its total, and its state.
struct ConnectionBlock: View {
    let connection: ViewConnection

    /// Home previews connections that need attention, without their totals.
    var compact = false

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 11) {
                Text(connection.initials)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(Theme.slate300)
                    .frame(width: 30, height: 30)
                    .background(Theme.bgInput, in: RoundedRectangle(cornerRadius: Theme.radiusSm))

                Text(connection.name)
                    .font(.system(size: 15, weight: .semibold))
                    // A debt group reads differently from an asset group.
                    .foregroundStyle(connection.isDebts ? Theme.textMuted : Theme.text)
                    .lineLimit(1)

                Spacer(minLength: 8)

                if connection.needsReconnect {
                    ReconnectButton()
                } else if connection.isDebts {
                    Text("Owed")
                        .font(.system(size: 10, weight: .bold))
                        .tracking(0.6)
                        .foregroundStyle(Theme.down)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2)
                        .background(Theme.downTint, in: Capsule())
                } else {
                    Text(connection.synced)
                        .font(.system(size: 11))
                        .foregroundStyle(Theme.up)
                }
            }
            .padding(.bottom, 10)

            ForEach(connection.accounts, id: \.id) { account in
                Divider().overlay(Theme.divider)
                AccountRow(row: account)
            }

            if !compact {
                Divider().overlay(Theme.borderStrong).padding(.top, 2)

                HStack(alignment: .firstTextBaseline) {
                    Text(connection.totalLabel)
                        .font(.system(size: 12))
                        .foregroundStyle(Theme.textFaint)

                    Spacer()

                    Text(connection.total)
                        .font(.system(size: 15, weight: .bold))
                        .monospacedDigit()
                        .foregroundStyle(connection.isDebts ? Theme.down : Theme.text)
                }
                .padding(.top, 11)
                .padding(.horizontal, 2)
            }
        }
    }
}

/// The action a stale connection needs.
///
/// Inert, and deliberately so: clearing `ITEM_LOGIN_REQUIRED` takes Plaid Link
/// in update mode, which is a hosted page and a server round trip that do not
/// exist yet. A button that looked live and did nothing would be worse than
/// one that says what it is waiting for.
struct ReconnectButton: View {
    var body: some View {
        Button("Reconnect") {}
            .font(.system(size: 11.5, weight: .semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 11)
            .padding(.vertical, 5)
            .background(Theme.accent, in: Capsule())
            .disabled(true)
            .opacity(0.5)
    }
}
