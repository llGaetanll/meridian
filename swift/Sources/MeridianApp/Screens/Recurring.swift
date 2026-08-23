/// What comes back: the monthly commitment, split by category, then every
/// stream grouped the same way.

import SwiftUI

struct RecurringScreen: View {
    let view: ViewRecurring

    var body: some View {
        PhoneShell(title: "Recurring", subtitle: view.subtitle) {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: 3) {
                    Masked(
                        value: view.headline,
                        masked: view.masked,
                        tappable: true,
                        hero: true,
                        size: 34
                    )

                    Text("/mo")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(Theme.textFaint)
                }

                Text(view.detail)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)
                    .padding(.top, 5)

                Breakdown(slices: view.breakdown, legend: true)
                    .padding(.top, 14)
                    .padding(.bottom, 6)

                ForEach(view.groups, id: \.self) { group in
                    HStack(alignment: .firstTextBaseline, spacing: 9) {
                        Text(group.name)
                            .font(.system(size: 12, weight: .bold))
                            .tracking(0.48)
                            .textCase(.uppercase)
                            .foregroundStyle(Theme.textFaint)

                        Spacer(minLength: 0)

                        Text(group.total)
                            .font(.system(size: 12, weight: .semibold))
                            .monospacedDigit()
                            .foregroundStyle(Theme.textMuted)
                    }
                    .padding(.top, 22)
                    .padding(.bottom, 8)
                    .padding(.horizontal, 4)

                    ForEach(Array(group.streams.enumerated()), id: \.element.id) { index, row in
                        Rule(first: index == 0)
                        StreamRow(row: row)
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 14)
        }
    }
}
