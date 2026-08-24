/// The ledger: everything that has happened, newest first, and the filter that
/// narrows it.

import SwiftUI

/// The filter, which sits above the list rather than scrolling with it as
/// part of it. `AppView` hands this to the shell's panel slot.
///
/// Always drawn: the two groups are the filter, and a button to reveal them
/// was a door in front of a door.
struct LedgerPanel: View {
    let filter: ViewFilter

    var body: some View {
        FilterPanel(filter: filter)
    }
}

struct LedgerScreen: View {
    let view: ViewLedger

    var body: some View {
        // Lazy with pinned headers, so a day's heading sticks to the
        // top while its rows scroll under it — `position: sticky` in
        // the stylesheet.
        LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
            if let empty = view.empty {
                Text(empty)
                    .font(.system(size: 13.5))
                    .foregroundStyle(Theme.textFaint)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 40)
            }

            ForEach(Array(view.months.enumerated()), id: \.element.key) { index, month in
                HStack(spacing: 12) {
                    Text(month.name)
                        .font(.system(size: 15, weight: .bold))
                        .tracking(-0.15)

                    Rectangle()
                        .fill(Theme.borderStrong)
                        .frame(height: 1)

                    Text(month.year)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(Theme.textFaint)
                }
                .padding(.top, index == 0 ? 12 : 26)
                .padding(.bottom, 4)
                .padding(.horizontal, 4)

                ForEach(month.days, id: \.heading) { day in
                    Section {
                        ForEach(Array(day.rows.enumerated()), id: \.element.id) {
                            index, row in
                            Rule(first: index == 0)
                            TxRow(row: row)
                        }
                    } header: {
                        Text(day.heading)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(Theme.textFaint)
                            .padding(.top, 16)
                            .padding(.bottom, 8)
                            .padding(.horizontal, 4)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Theme.bg)
                    }
                }
            }
        }
        .padding(.horizontal, 20)
    }
}

private extension ViewMonthSection {
    /// What identifies a month: the month, not the transactions inside it.
    ///
    /// `phone-main` keys its `<For>` on `format!("{}{}", month.name, month.year)`
    /// and this has to agree. Keying on the whole value instead makes a month's
    /// identity change whenever any row inside it does, so SwiftUI throws the
    /// section away and builds a new one — losing scroll position in a list
    /// that is pinned and lazy.
    var key: String { name + year }
}

/// Drawn inside the group it clears, and only when something is selected.
private struct ClearCategories: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Button("Clear categories") { runtime.ui(.categoriesCleared) }
            .font(.system(size: 12))
            .foregroundStyle(Theme.textFaint)
            .padding(.top, 9)
    }
}

private struct FilterPanel: View {
    let filter: ViewFilter

    var body: some View {
        VStack(spacing: 10) {
            FilterSection(
                icon: "tag",
                title: "Categories",
                open: filter.categoriesOpen,
                group: .categories,
                summary: {
                    HStack(spacing: 5) {
                        ForEach(filter.categoriesSummary, id: \.id) { chip in
                            Text(chip.label)
                                .font(.system(size: 10.5, weight: .semibold))
                                .padding(.horizontal, 8)
                                .padding(.vertical, 2)
                                .background(Theme.categoryTint(chip.token), in: Capsule())
                                .foregroundStyle(Theme.category(chip.token))
                        }

                        if let overflow = filter.categoriesOverflow {
                            Text(overflow)
                                .font(.system(size: 11.5))
                                .foregroundStyle(Theme.textFaint)
                        }
                    }
                    .lineLimit(1)
                },
                content: {
                    Flow(spacing: 7) {
                        ForEach(filter.categories, id: \.id) { chip in
                            CategoryChip(chip: chip)
                        }
                    }

                    if !filter.categoriesSummary.isEmpty {
                        ClearCategories()
                    }
                }
            )

            FilterSection(
                icon: "clock",
                title: "Time range",
                open: filter.rangeOpen,
                group: .range,
                summary: {
                    Text(filter.rangeSummary)
                        .font(.system(size: 12))
                        .foregroundStyle(Theme.textMuted)
                        .lineLimit(1)
                },
                content: {
                    RangeRows(range: filter.range)
                }
            )
        }
        .padding(.horizontal, 20)
        .padding(.top, 10)
        .padding(.bottom, 4)
    }
}

/// One half of the panel. Shut, the heading is the whole control and shows
/// what is selected — set smaller, so a closed filter reads as a summary and
/// not as a second copy.
private struct FilterSection<Summary: View, Content: View>: View {
    let icon: String
    let title: String
    let open: Bool
    let group: FilterGroup

    @ViewBuilder let summary: Summary
    @ViewBuilder let content: Content

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 9) {
                Icon(name: icon, size: 16)
                    .foregroundStyle(Theme.textFaint)

                Text(title)
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.88)
                    .textCase(.uppercase)
                    .foregroundStyle(Theme.textFaint)

                Spacer(minLength: 0)

                if !open {
                    summary
                }

                Icon(name: "chevron", size: 15)
                    .foregroundStyle(Theme.textFaint)
                    .rotationEffect(.degrees(open ? 0 : -90))
                    .padding(.leading, 7)
            }
            .padding(.bottom, open ? 11 : 0)
            .contentShape(Rectangle())
            .onTapGesture { runtime.ui(.filterGroupToggled(group: group)) }

            if open {
                content
            }
        }
        .padding(12)
        .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: Theme.radiusMd))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusMd).stroke(Theme.border, lineWidth: 1)
        )
    }
}

/// The range, as the sentence the sketch draws. Each row is one of the four
/// forms; tapping one makes it live.
private struct RangeRows: View {
    let range: ViewRange

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(spacing: 7) {
            RangeRow(on: range.kind == .this) {
                runtime.ui(.rangeSelected(kind: .this(unit: .month)))
            } content: {
                Text("This")
                Token(range.thisUnit, on: range.kind == .this)
            }

            RangeRow(on: range.kind == .last) {
                // The tokens keep their last value even when another form is
                // live, so switching back does not reset the sentence.
                let count = UInt32(range.lastCount) ?? 3

                runtime.ui(.rangeSelected(kind: .last(count: count, unit: .month)))
            } content: {
                Text("Last")
                Token(range.lastCount, on: range.kind == .last, fixed: true)
                Token(range.lastUnit, on: range.kind == .last)
            }

            // Drawn because the sketch has it, and inert because picking a day
            // needs a date picker that is a screen of its own.
            RangeRow(on: range.kind == .between, disabled: true) {} content: {
                Text("Between")
                Token(range.betweenStart, on: false)
                Text("and")
                Token(range.betweenEnd, on: false)
            }

            RangeRow(on: range.kind == .all) {
                runtime.ui(.rangeSelected(kind: .all))
            } content: {
                Text("All time").fontWeight(.semibold).padding(.vertical, 3)
            }
        }
    }
}

private struct RangeRow<Content: View>: View {
    let on: Bool

    var disabled = false

    let select: () -> Void

    @ViewBuilder let content: Content

    var body: some View {
        HStack(spacing: 7) {
            content
            Spacer(minLength: 0)
        }
        .font(.system(size: 13, weight: on ? .medium : .regular))
        .foregroundStyle(on ? Theme.accentBright : Theme.textFaint)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            on ? Theme.accentTint : Theme.bgInput,
            in: RoundedRectangle(cornerRadius: Theme.radiusSm)
        )
        .opacity(disabled ? 0.45 : 1)
        .contentShape(Rectangle())
        .onTapGesture {
            if !disabled {
                select()
            }
        }
    }
}

/// One editable word in the sentence.
private struct Token: View {
    let text: String
    let on: Bool

    /// The count sits in a fixed slot so the sentence does not jump when it
    /// goes from 3 to 12.
    var fixed = false

    init(_ text: String, on: Bool, fixed: Bool = false) {
        self.text = text
        self.on = on
        self.fixed = fixed
    }

    var body: some View {
        HStack(spacing: 5) {
            Text(text)
            Icon(name: "caret", size: 10).opacity(0.75)
        }
        .font(.system(size: 12.5, weight: .semibold))
        .foregroundStyle(on ? Theme.accentBright : Theme.textMuted)
        .padding(.horizontal, 9)
        .padding(.vertical, 3)
        .frame(minWidth: fixed ? 34 : 0)
        .background(
            on ? Theme.accentSoft.opacity(0.22) : Color.white.opacity(0.06),
            in: RoundedRectangle(cornerRadius: Theme.radiusSm)
        )
    }
}
