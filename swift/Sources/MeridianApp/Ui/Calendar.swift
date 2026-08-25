/// The month, with a mark on every day a stream lands.
///
/// The grid is the core's — six rows of seven, days either side of the month
/// included — so this places cells and colours marks and decides nothing about
/// which day is which.

import SwiftUI

struct CalendarGrid: View {
    let calendar: ViewCalendar

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 0), count: 7)

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(calendar.month)
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.88)
                .textCase(.uppercase)
                .foregroundStyle(Theme.textFaint)

            LazyVGrid(columns: columns, spacing: 0) {
                // The headings repeat a letter or two — Tuesday and Thursday
                // are both "T" — so they are keyed by where they sit rather
                // than by what they say.
                ForEach(Array(calendar.weekdays.enumerated()), id: \.offset) { _, day in
                    Text(day)
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(Theme.textFaint.opacity(0.7))
                        .frame(maxWidth: .infinity)
                        .padding(.bottom, 6)
                }
            }

            LazyVGrid(columns: columns, spacing: 4) {
                ForEach(Array(calendar.weeks.enumerated()), id: \.offset) { week, days in
                    ForEach(Array(days.enumerated()), id: \.offset) { column, day in
                        Day(day: day)
                            .id("\(week)-\(column)")
                    }
                }
            }
        }
        .padding(.vertical, 12)
    }
}

private struct Day: View {
    let day: ViewCalendarDay

    var body: some View {
        VStack(spacing: 3) {
            Text(day.label)
                .font(.system(size: 12.5, weight: day.today ? .bold : .regular, design: .monospaced))
                .foregroundStyle(colour)
                .frame(width: 24, height: 24)
                .background {
                    if day.today {
                        Circle().fill(Theme.accentSoft.opacity(0.18))
                    }
                }

            // A row of marks rather than a count: which categories land that
            // day is the useful bit, and three dots is already a busy day.
            HStack(spacing: 2.5) {
                ForEach(Array(day.marks.enumerated()), id: \.offset) { _, token in
                    Circle()
                        .fill(Theme.category(token))
                        .frame(width: 4, height: 4)
                }
            }
            .frame(height: 4)
        }
        .frame(maxWidth: .infinity)
    }

    private var colour: Color {
        if day.today {
            Theme.accentBright
        } else if day.outside {
            Theme.textFaint.opacity(0.45)
        } else {
            Theme.textMuted
        }
    }
}
