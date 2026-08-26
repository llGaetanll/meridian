/// The ledger: everything that has happened, newest first, and the filter that
/// narrows it.

import SwiftUI

struct LedgerScreen: View {
    let view: ViewLedger

    /// What the band scrolls back to.
    static let filterAnchor = "filter"

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Eagerly, and outside the lazy stack below: the band that stands
            // in for this once it has gone by is driven by where it is, and a
            // lazy stack would throw it away as it left the screen.
            FilterPanel(filter: view.filter)
                .id(Self.filterAnchor)
                .bandAnchor()

            list
        }
    }

    // Not pinned. The stylesheet makes a day's heading sticky, but the band
    // floats over the top of this list — a heading pinned up there would sit
    // behind it, and the alternative is the band pushing the list down, which
    // is the jump it exists to avoid. Each day is already fenced by a rule and
    // carries its own net.
    private var list: some View {
        LazyVStack(alignment: .leading, spacing: 0) {
            if let empty = view.empty {
                Text(empty)
                    .font(.system(size: 13.5))
                    .foregroundStyle(Theme.textFaint)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 40)
            }

            ForEach(Array(view.months.enumerated()), id: \.element.key) { index, month in
                MonthMarker(month: month, first: index == 0)

                ForEach(Array(month.days.enumerated()), id: \.element.heading) { position, day in
                    // The rule between two days sits in the list rather than
                    // in the heading below it, so that a heading pinned to the
                    // top does not carry a second line up against whatever is
                    // already drawing one there — the band, in particular.
                    // The first day of a month goes without: the month rule is
                    // right above it.
                    if position > 0 {
                        RowRule()
                    }

                    DayHeading(day: day)

                    ForEach(day.rows, id: \.id) { row in
                        TxRow(row: row)
                    }
                }
            }
        }
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

/// The coarse ruler over the day groups: a section rule with the month under
/// it.
private struct MonthMarker: View {
    let month: ViewMonthSection
    let first: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionRule()

            Text(month.label)
                .font(.system(size: 15, weight: .bold))
                .tracking(-0.22)
                .padding(.horizontal, pageInset)
                .padding(.top, 16)
        }
        .padding(.top, first ? 8 : 24)
    }
}

/// A day's heading. It carries the day's net, so the two halves of the line
/// say the same thing about the same day.
///
/// The rule that separates it from the day above is not here but in the list,
/// so that a band floating over the top of the list never has two lines under
/// it. See where it is drawn.
private struct DayHeading: View {
    let day: ViewDaySection

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(day.heading)
                .font(.system(size: 12, weight: .semibold))

            Spacer(minLength: 0)

            Text(day.total)
                .font(.system(size: 13.5, weight: .medium, design: .monospaced))
        }
        .foregroundStyle(Theme.textFaint)
        .padding(.horizontal, pageInset)
        .padding(.top, 18)
        .padding(.bottom, 10)
        .background(Theme.bg)
    }
}

/// The filter, drawn always: the two groups are the filter, and a button to
/// reveal them was a door in front of a door. Once it has scrolled by, what it
/// is set to comes back as `LedgerBand`.
private struct FilterPanel: View {
    let filter: ViewFilter

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Label(text: "Categories")

            ChipRows(spacing: 8) {
                ForEach(filter.categories, id: \.id) { chip in
                    CategoryChip(chip: chip)
                }
            }

            Label(text: "Time range")

            RangeControl(range: filter.range)
        }
        .padding(.horizontal, pageInset)
        .padding(.top, 14)
        .padding(.bottom, 4)
    }

    private struct Label: View {
        let text: String

        var body: some View {
            Text(text)
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.99)
                .textCase(.uppercase)
                .foregroundStyle(Theme.textFaint)
        }
    }
}

/// What the filter is set to, once the filter itself has gone by.
///
/// No ground of its own: a rule under it is what separates it, the same as
/// every other boundary in the app. The chevron on the left says it opens, and
/// what is in it is the filter's own two halves in the filter's own order —
/// drawn as the app's chips at a smaller size rather than as pills invented
/// for the band.
struct LedgerBand: View {
    let filter: ViewFilter
    let scroller: ScrollViewProxy

    var body: some View {
        Band {
            HStack(spacing: 10) {
                Icon(name: "chevron", size: 14)
                    .foregroundStyle(Theme.textFaint)

                HStack(spacing: 6) {
                    ForEach(filter.categoriesSummary, id: \.id) { chip in
                        Chip(label: chip.label, selected: true, small: true)
                    }

                    if let overflow = filter.categoriesOverflow {
                        Text(overflow)
                            .font(.system(size: 11.5))
                            .foregroundStyle(Theme.textFaint)
                    }
                }
                .lineLimit(1)

                Spacer(minLength: 0)

                Chip(label: filter.rangeSummary, small: true)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            withAnimation(.easeOut(duration: 0.25)) {
                scroller.scrollTo(LedgerScreen.filterAnchor, anchor: .top)
            }
        }
    }
}
