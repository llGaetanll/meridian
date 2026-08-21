/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on
/// and hands over everything that screen shows, so this is a `switch` over the
/// last thing it rendered — the same shape as
/// `crates/phone-main/src/app.rs`.
///
/// This is a first pass on purpose. It draws every field the core sends so the
/// loop can be watched working end to end, but the layout is not the sketch
/// yet: `crates/phone-main` is the design to transcribe, and that is the next
/// piece of work rather than this one.
///
/// The rule this file follows, and the one that matters: no `if`, no
/// `switch`, and no string formatting over a value the core sent. Every figure
/// below arrives ready to print. If something here wants to compute, the field
/// is missing from the view type and belongs in `phone-state`.

import SwiftUI

struct AppView: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            VStack(spacing: 0) {
                Header()

                // A banner rather than a screen: whatever was loaded before is
                // still worth looking at, so the failure sits above it instead
                // of replacing it.
                if let message = runtime.error {
                    Banner(message: message)
                }

                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        ScreenView(screen: runtime.screen)
                    }
                    .padding(.horizontal, 18)
                    .padding(.vertical, 12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .refreshable {
                    runtime.ui(.refreshRequested)
                }

                Dock()
            }
        }
        .preferredColorScheme(.dark)
        .foregroundStyle(Theme.text)
    }
}

/// Whichever screen the core rendered.
///
/// Exhaustive with no `default:`. Add a `ViewScreen` variant in Rust,
/// regenerate, and this stops compiling until the new screen is drawn — the
/// guarantee `docs/FFI.md` is built around.
private struct ScreenView: View {
    let screen: ViewScreen?

    var body: some View {
        switch screen {
        case .none:
            Text("Starting…").foregroundStyle(Theme.textFaint)

        case .home(let view):
            HomeScreen(view: view)

        case .ledger(let view):
            LedgerScreen(view: view)

        case .recurring(let view):
            RecurringScreen(view: view)

        case .balances(let view):
            BalancesScreen(view: view)
        }
    }
}

private struct HomeScreen: View {
    let view: ViewHome

    var body: some View {
        Block(title: "Balances") {
            Headline(value: view.balances.headline, detail: view.balances.detail)
        }

        Block(title: "Recurring") {
            Headline(value: view.recurring.headline, detail: view.recurring.detail)
            Breakdown(slices: view.recurring.breakdown)

            ForEach(view.recurring.upcoming, id: \.id) { row in
                StreamRow(row: row)
            }
        }

        Block(title: "Ledger") {
            ForEach(view.ledger, id: \.id) { row in
                TxRow(row: row)
            }
        }

        // Only drawn when something actually needs attention — a healthy
        // connection is not news.
        if !view.attention.isEmpty {
            Block(title: "Connections") {
                ForEach(view.attention, id: \.id) { connection in
                    ConnectionBlock(connection: connection)
                }
            }
        }
    }
}

private struct LedgerScreen: View {
    let view: ViewLedger

    var body: some View {
        PageTitle(title: "Ledger", subtitle: view.subtitle)

        FilterButton(filter: view.filter)

        if let empty = view.empty {
            Text(empty).foregroundStyle(Theme.textFaint)
        }

        ForEach(view.months, id: \.self) { month in
            HStack {
                Text(month.name).font(.headline)
                Spacer()
                Text(month.year).foregroundStyle(Theme.textFaint)
            }
            .padding(.top, 8)

            ForEach(month.days, id: \.self) { day in
                Text(day.heading)
                    .font(.caption)
                    .foregroundStyle(Theme.textFaint)

                ForEach(day.rows, id: \.id) { row in
                    TxRow(row: row)
                }
            }
        }
    }
}

private struct RecurringScreen: View {
    let view: ViewRecurring

    var body: some View {
        PageTitle(title: "Recurring", subtitle: view.subtitle)

        Headline(value: view.headline, detail: view.detail)
        Breakdown(slices: view.breakdown)

        ForEach(view.groups, id: \.self) { group in
            HStack {
                Text(group.name).foregroundStyle(Theme.category(group.token))
                Spacer()
                Text(group.total).monospacedDigit()
            }
            .padding(.top, 8)

            ForEach(group.streams, id: \.id) { row in
                StreamRow(row: row)
            }
        }
    }
}

private struct BalancesScreen: View {
    let view: ViewBalances

    var body: some View {
        PageTitle(title: "Balances", subtitle: view.subtitle)

        Headline(value: view.headline, detail: view.headlineLabel)

        ForEach(view.connections, id: \.id) { connection in
            ConnectionBlock(connection: connection)
        }
    }
}

private struct Header: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        HStack {
            Text("Meridian")
                .font(.title2.weight(.semibold))
                .foregroundStyle(Theme.accentSoft)

            Spacer()

            Button {
                runtime.ui(.refreshRequested)
            } label: {
                Image(systemName: "arrow.clockwise")
                    .foregroundStyle(runtime.loading ? Theme.accentSoft : Theme.textFaint)
            }
            .accessibilityLabel("Refresh")

            Button {
                runtime.ui(.maskToggled)
            } label: {
                Image(systemName: "eye").foregroundStyle(Theme.textFaint)
            }
            .accessibilityLabel("Reveal figures")
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
    }
}

/// The dock. Which tab is lit is the core's answer, not a comparison here.
private struct Dock: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        HStack {
            ForEach(runtime.dock?.items ?? [], id: \.self) { item in
                Button {
                    runtime.ui(.screenOpened(screen: item.screen))
                } label: {
                    VStack(spacing: 3) {
                        Image(systemName: Theme.icon(item.icon))
                        Text(item.label).font(.caption2)
                    }
                    .foregroundStyle(item.selected ? Theme.accentSoft : Theme.textFaint)
                    .frame(maxWidth: .infinity)
                }
            }
        }
        .padding(.vertical, 10)
        .background(Theme.bgPanel)
        .overlay(Rectangle().frame(height: 1).foregroundStyle(Theme.border), alignment: .top)
    }
}

private struct Banner: View {
    @EnvironmentObject private var runtime: RuntimeApp

    let message: String

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Text(message).font(.footnote)
            Spacer()
            Button("Retry") { runtime.ui(.refreshRequested) }
                .font(.footnote.weight(.semibold))
        }
        .foregroundStyle(Theme.down)
        .padding(10)
        .background(Theme.down.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .padding(.horizontal, 18)
    }
}

private struct PageTitle: View {
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.largeTitle.weight(.bold))
            Text(subtitle).font(.caption).foregroundStyle(Theme.textFaint)
        }
    }
}

private struct Block<Content: View>: View {
    let title: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.textMuted)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(Theme.bgPanel)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(
            RoundedRectangle(cornerRadius: 16).stroke(Theme.border, lineWidth: 1)
        )
    }
}

/// A figure and the line under it.
///
/// The core has already replaced a covered figure with dots, so there is
/// nothing to hide here — the real value is not in the view at all while it is
/// masked, which is stricter than hiding it with an opacity.
private struct Headline: View {
    @EnvironmentObject private var runtime: RuntimeApp

    let value: String
    let detail: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value)
                .font(.system(size: 30, weight: .semibold))
                .monospacedDigit()
                .onTapGesture { runtime.ui(.maskToggled) }

            Text(detail).font(.caption).foregroundStyle(Theme.textFaint)
        }
    }
}

private struct TxRow: View {
    @EnvironmentObject private var runtime: RuntimeApp

    let row: ViewTxRow

    var body: some View {
        HStack(spacing: 10) {
            Logo(url: row.logoUrl)

            VStack(alignment: .leading, spacing: 2) {
                Text(row.title).lineLimit(1)
                Text(row.category)
                    .font(.caption2)
                    .foregroundStyle(Theme.category(row.categoryToken))
            }

            Spacer()

            Text(row.amount)
                .monospacedDigit()
                .foregroundStyle(row.isInflow ? Theme.up : Theme.text)
        }
        .opacity(row.pending ? 0.6 : 1)
        .contentShape(Rectangle())
        .onTapGesture { runtime.ui(.txSelected(id: row.id)) }
    }
}

private struct StreamRow: View {
    let row: ViewStreamRow

    var body: some View {
        HStack(spacing: 10) {
            Logo(url: row.logoUrl)

            VStack(alignment: .leading, spacing: 2) {
                Text(row.title).lineLimit(1)
                Text(row.when)
                    .font(.caption2)
                    .foregroundStyle(row.soon ? Theme.accentSoft : Theme.textFaint)
            }

            Spacer()

            VStack(alignment: .trailing, spacing: 2) {
                Text(row.amount).monospacedDigit()
                Text(row.frequency).font(.caption2).foregroundStyle(Theme.textFaint)
            }
        }
    }
}

private struct ConnectionBlock: View {
    let connection: ViewConnection

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(connection.initials)
                    .font(.caption.weight(.bold))
                    .frame(width: 28, height: 28)
                    .background(Theme.bgRaised)
                    .clipShape(RoundedRectangle(cornerRadius: 8))

                Text(connection.name)
                Spacer()
                Text(connection.synced).font(.caption2).foregroundStyle(Theme.textFaint)
            }

            ForEach(connection.accounts, id: \.id) { account in
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(account.name).font(.footnote)
                        Text(account.subtitle).font(.caption2).foregroundStyle(Theme.textFaint)
                    }

                    Spacer()

                    Text(account.amount)
                        .monospacedDigit()
                        .foregroundStyle(account.owed ? Theme.down : Theme.text)
                }
            }

            HStack {
                Text(connection.totalLabel).font(.caption).foregroundStyle(Theme.textFaint)
                Spacer()
                Text(connection.total)
                    .monospacedDigit()
                    .foregroundStyle(connection.isDebts ? Theme.down : Theme.text)
            }
        }
        .padding(12)
        .background(Theme.bgPanel)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.border, lineWidth: 1))
    }
}

/// The breakdown bar. `share` is geometry rather than a formatted string —
/// the one exception `docs/FFI.md` carves out, because each platform draws
/// with its own API.
private struct Breakdown: View {
    let slices: [ViewBreakdownSlice]

    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 2) {
                ForEach(slices, id: \.self) { slice in
                    Theme.category(slice.token)
                        .frame(width: max(0, geometry.size.width * slice.share))
                }
            }
        }
        .frame(height: 8)
        .clipShape(Capsule())
    }
}

/// The logo slot, reserved whether or not there is a logo, so every row's text
/// starts on the same line.
private struct Logo: View {
    let url: String?

    var body: some View {
        AsyncImage(url: url.flatMap(URL.init(string:))) { image in
            image.resizable().scaledToFit()
        } placeholder: {
            Color.clear
        }
        .frame(width: 28, height: 28)
        .background(Theme.bgRaised)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

/// The control that opens the filter panel.
///
/// The panel itself is not drawn yet — it is the biggest piece of the sketch
/// and belongs with the real transcription. The button is here because the
/// core already tracks whether anything is filtering, and a dot that lights up
/// proves the state is crossing.
private struct FilterButton: View {
    @EnvironmentObject private var runtime: RuntimeApp

    let filter: ViewFilter

    var body: some View {
        Button {
            runtime.ui(.filterToggled)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "line.3.horizontal.decrease")
                Text("Filter")
                if filter.active {
                    Circle().frame(width: 6, height: 6).foregroundStyle(Theme.accent)
                }
            }
            .font(.footnote)
            .foregroundStyle(filter.open ? Theme.text : Theme.textMuted)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(Theme.bgPanel)
            .clipShape(Capsule())
        }
    }
}
