/// What comes back: the net of it, split by category, then the month it lands
/// on and every stream behind it.

import SwiftUI

struct RecurringScreen: View {
    let view: ViewRecurring

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Hero(
                label: view.headlineLabel,
                value: view.headline,
                flowIn: view.flowIn,
                flowOut: view.flowOut
            )
            .bandAnchor()

            Sec(title: view.breakdownLabel, value: view.breakdownTotal, first: true) {
                Split(slices: view.breakdown)
            }

            // No heading: the calendar is the split above it drawn a second
            // way, and a title over it would be naming the same thing twice.
            Sec {
                CalendarStrip(calendar: view.calendar)

                if let card = view.dayCard {
                    DayCard(card: card)
                }
            }

            Sec(title: view.streamsLabel, value: view.streamsCount) {
                ForEach(Array(view.groups.enumerated()), id: \.element.self) { index, group in
                    StreamGroupHead(group: group, first: index == 0)

                    ForEach(group.streams, id: \.id) { row in
                        StreamRow(row: row)
                    }
                }
            }
        }
    }
}

/// The streams, grouped the way the split groups them.
private struct StreamGroupHead: View {
    let group: ViewStreamGroup
    let first: Bool

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 9) {
            Text(group.name)
                .font(.system(size: 13, weight: .semibold))
                .tracking(-0.08)

            Spacer(minLength: 0)

            Text(group.countLabel)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(Theme.textFaint)
        }
        .padding(.horizontal, pageInset)
        .padding(.top, first ? 0 : 20)
        .padding(.bottom, 6)
    }
}

/// The headline, once it has scrolled away.
struct RecurringBand: View {
    let view: ViewRecurring

    var body: some View {
        Band {
            HStack(spacing: 12) {
                Text(view.headline)
                    .font(.system(size: 15, weight: .bold, design: .monospaced))

                Spacer(minLength: 8)

                // A diff, not a sentence: there is no room for the words that
                // carry the direction on the hero, so the figures carry it.
                HStack(spacing: 10) {
                    Text(view.flowInSigned).foregroundStyle(Theme.up)
                    Text(view.flowOutSigned).foregroundStyle(Theme.down)
                }
                .font(.system(size: 13, weight: .semibold, design: .monospaced))
            }
        }
    }
}
