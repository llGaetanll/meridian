/// The month, with a mark on every day a stream lands.
///
/// It runs sideways, because months do, and turning it is a scroll rather than
/// a redraw: all three months are drawn, laid a page apart, and the swipe moves
/// the lot. Only a column of the months either side shows at the edge, but it
/// is that month's own column and the rest of it is right behind the glass.
///
/// The grid is the core's, rows and all, so this places cells and decides
/// nothing about which day is which.

import SwiftUI

struct CalendarStrip: View {
    let calendar: ViewCalendar

    /// The month gives up a little of the measure so the months either side
    /// have somewhere to be: 310 across seven still leaves half a day's width
    /// around each 32pt disc.
    private static let month: CGFloat = 310

    /// The months are pages, not columns of one grid, so they sit further from
    /// each other than the days inside them do.
    private static let gap: CGFloat = 14

    /// How far one month is from the next, which is how far the strip travels
    /// to turn a page.
    private static var pitch: CGFloat { month + gap }

    /// The months named above the grid turn with it, but they are pulled in to
    /// the edges so they can be read rather than laid out a page apart — so
    /// they have a shorter way to go, and travel at a fraction of the rate.
    fileprivate static let namePitch: CGFloat = 165
    fileprivate static var parallax: CGFloat { namePitch / pitch }

    /// How far the finger has to travel before a swipe turns the month. Short
    /// enough to feel like a flick, long enough that a vertical scroll begun
    /// over the calendar is not one.
    private static let commit: CGFloat = 56

    /// How long a page takes to come in, and how long the grid takes to grow
    /// or shrink a row under it.
    private static let turn: Double = 0.28

    @EnvironmentObject private var runtime: RuntimeApp

    /// Where the finger has dragged the strip to. Presentation, so it is the
    /// view's — which month is on is the machine's.
    @State private var drag: CGFloat = 0

    /// What the grid is currently as tall as. Held rather than left to the
    /// content so that a month with a row more or less can be moved to rather
    /// than jumped to; `nil` until the first month has been measured.
    @State private var height: CGFloat?

    /// A page is already on its way over. A second swipe on top of it would
    /// start from a position the first one is still animating away from.
    @State private var turning = false

    var body: some View {
        VStack(spacing: 0) {
            Names(calendar: calendar, drag: drag)
                .padding(.bottom, 24)

            Grid(month: calendar.current, weekdays: calendar.weekdays)
                .frame(width: Self.month)
                .background {
                    GeometryReader { geometry in
                        Color.clear.preference(key: GridHeight.self, value: geometry.size.height)
                    }
                }
                // The months either side hang off the middle one rather than
                // standing beside it in a row. Three grids in a row are wider
                // than the screen — which is the point, they are meant to be
                // cut off at the glass — but a page wider than the screen is
                // centred in what it overflows, and that moves every margin on
                // it. An overlay cannot widen what it is drawn over.
                .overlay(alignment: .topLeading) {
                    Grid(month: calendar.prev, weekdays: calendar.weekdays)
                        .frame(width: Self.month)
                        .offset(x: -Self.pitch)
                }
                .overlay(alignment: .topTrailing) {
                    Grid(month: calendar.next, weekdays: calendar.weekdays)
                        .frame(width: Self.month)
                        .offset(x: Self.pitch)
                }
                .offset(x: drag)
                .frame(maxWidth: .infinity)
                // The height is the middle month's, and it is the one thing
                // about a page turn that is allowed to take its time.
                .frame(height: height, alignment: .top)
                .clipped()
                .mask(Self.fade)
                .gesture(swipe)
        }
        .onPreferenceChange(GridHeight.self) { measured in
            guard measured > 0, measured != height else { return }

            // The first month is however tall it is. Every one after that is a
            // month arriving, and the rule under the calendar moves to its new
            // place rather than appearing there.
            if height == nil {
                height = measured
            } else {
                withAnimation(.easeOut(duration: Self.turn)) { height = measured }
            }
        }
    }

    /// The fade has to be narrower than the peek it is fading, or the months
    /// either side are gone rather than dim.
    private static var fade: LinearGradient {
        LinearGradient(
            stops: [
                .init(color: .clear, location: 0),
                .init(color: .black, location: 0.07),
                .init(color: .black, location: 0.93),
                .init(color: .clear, location: 1),
            ],
            startPoint: .leading,
            endPoint: .trailing
        )
    }

    /// Left and right turns the month, because that is the direction the
    /// calendar is laid out in and the month at the edge is what it is turning
    /// towards.
    private var swipe: some Gesture {
        DragGesture(minimumDistance: 14)
            .onChanged { gesture in
                if !turning {
                    drag = gesture.translation.width
                }
            }
            .onEnded { gesture in
                guard !turning else { return }

                let travelled = gesture.translation.width

                guard abs(travelled) > Self.commit else {
                    withAnimation(.easeOut(duration: 0.2)) { drag = 0 }
                    return
                }

                let forward = travelled < 0
                turning = true

                // Scroll the month over first, and only then tell the machine.
                // The other way round the new month is drawn where the old one
                // stood and the days change under the finger.
                withAnimation(.easeOut(duration: Self.turn)) {
                    drag = forward ? -Self.pitch : Self.pitch
                }

                DispatchQueue.main.asyncAfter(deadline: .now() + Self.turn) {
                    // The month that was next is now current, and current is
                    // drawn where next has just finished — the same pixels. So
                    // both halves go in one breath and neither animates:
                    // anything animating across this swap interpolates the old
                    // month's rows into the new one's in place, which is a grid
                    // of numbers sliding past each other and the month name
                    // walking backwards before it arrives.
                    //
                    // The height is the exception, and it is animated where it
                    // is measured rather than here.
                    var instant = Transaction()
                    instant.disablesAnimations = true

                    withTransaction(instant) {
                        runtime.ui(.calendarMonthStepped(step: forward ? 1 : -1))
                        drag = 0
                    }

                    turning = false
                }
            }
    }
}

/// One month's grid.
private struct Grid: View {
    let month: ViewCalendarMonth
    let weekdays: [String]

    private var columns: [GridItem] {
        Array(repeating: GridItem(.flexible(), spacing: 0), count: 7)
    }

    var body: some View {
        VStack(spacing: 0) {
            LazyVGrid(columns: columns, spacing: 0) {
                // Tuesday and Thursday are both "T", so the headings are keyed
                // by where they sit rather than by what they say.
                ForEach(Array(weekdays.enumerated()), id: \.offset) { _, letter in
                    Text(letter)
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(Theme.textFaint.opacity(0.7))
                        .frame(maxWidth: .infinity)
                }
            }
            .padding(.bottom, 6)

            // Keyed by where a cell sits and not by which day it is. The
            // months overlap at their edges — the 2nd of August closes July —
            // so keying by the day lets one be matched across a change of month
            // and walked from where it was to where it now is, which is rows of
            // numbers crawling over each other.
            LazyVGrid(columns: columns, spacing: 8) {
                ForEach(Array(month.weeks.enumerated()), id: \.offset) { week, days in
                    ForEach(Array(days.enumerated()), id: \.offset) { column, day in
                        Day(day: day).id("\(week)-\(column)")
                    }
                }
            }
        }
    }
}

/// One day. Plain, marked, today, tapped, or outside the month.
private struct Day: View {
    let day: ViewCalendarDay

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(spacing: 3) {
            Text(day.label)
                .font(.system(size: 13, weight: emphasised ? .bold : .regular, design: .monospaced))
                .foregroundStyle(colour)
                .frame(width: 32, height: 32)
                .background {
                    if day.today {
                        Circle().fill(Theme.accentSoft.opacity(0.20))
                    } else if day.picked {
                        // Grey, not outlined: an outline reads as a different
                        // kind of control sitting in the grid.
                        Circle().fill(Theme.bgInput)
                    }
                }

            // Marks say something landed and no longer say what — the split
            // above the calendar says that, in words.
            HStack(spacing: 3) {
                ForEach(Array(day.marks.enumerated()), id: \.offset) { _, _ in
                    Circle()
                        .fill(Theme.accentSoft.opacity(0.55))
                        .frame(width: 5, height: 5)
                }
            }
            .frame(height: 5)
            .padding(.top, 1)
        }
        .frame(maxWidth: .infinity)
        .contentShape(Rectangle())
        .onTapGesture {
            if !day.outside {
                runtime.ui(.calendarDaySelected(id: day.id))
            }
        }
    }

    private var emphasised: Bool { day.today || day.picked }

    private var colour: Color {
        if day.today {
            Theme.accentBright
        } else if day.picked {
            Theme.text
        } else if day.outside {
            Theme.textFaint.opacity(0.45)
        } else {
            Theme.textMuted
        }
    }
}

/// The three months named, the middle one lit.
///
/// One drawing of each label rather than a big one in the middle and two small
/// ones at the edges: they travel with the grid, and a label that swaps size
/// and colour when it arrives has not travelled, it has been replaced. What
/// makes the middle one the middle one is how close to the middle it is.
private struct Names: View {
    let calendar: ViewCalendar
    let drag: CGFloat

    var body: some View {
        ZStack {
            Name(label: calendar.prev.label, x: -CalendarStrip.namePitch + shift)
            Name(label: calendar.current.label, x: shift)
            Name(label: calendar.next.label, x: CalendarStrip.namePitch + shift)
        }
        .frame(height: 26)
        .frame(maxWidth: .infinity)
        .clipped()
        .mask(
            LinearGradient(
                stops: [
                    .init(color: .clear, location: 0.02),
                    .init(color: .black, location: 0.32),
                    .init(color: .black, location: 0.68),
                    .init(color: .clear, location: 0.98),
                ],
                startPoint: .leading,
                endPoint: .trailing
            )
        )
    }

    private var shift: CGFloat { drag * CalendarStrip.parallax }

    private struct Name: View {
        let label: String
        let x: CGFloat

        var body: some View {
            // Nearest the middle is brightest and largest, and it falls off
            // with the distance — so a label brightens as it arrives rather
            // than at the moment it lands.
            let near = max(0, 1 - abs(x) / CalendarStrip.namePitch)

            Text(label)
                .font(.system(size: 15, weight: .semibold))
                .tracking(-0.15)
                .fixedSize()
                .foregroundStyle(Theme.text.opacity(0.3 + 0.7 * near))
                .scaleEffect(0.87 + 0.13 * near)
                .offset(x: x)
        }
    }
}

/// How tall the month in the middle is.
private struct GridHeight: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = max(value, nextValue())
    }
}

/// What landed on the day you tapped.
struct DayCard: View {
    let card: ViewDayCard

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text(card.heading)
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.88)
                    .textCase(.uppercase)

                Spacer(minLength: 12)

                Text(card.total)
                    .font(.system(size: 13, weight: .bold, design: .monospaced))
                    .foregroundStyle(Theme.textMuted)
            }
            .foregroundStyle(Theme.textFaint)
            .padding(.horizontal, pageInset)
            .padding(.bottom, 8)

            ForEach(card.rows, id: \.id) { row in
                RowRule()

                HStack(alignment: .firstTextBaseline) {
                    Text(row.name)
                        .font(.system(size: 13.5, weight: .medium))

                    Spacer(minLength: 12)

                    Text(row.amount)
                        .font(.system(size: 13.5, weight: .semibold, design: .monospaced))
                }
                .padding(.horizontal, pageInset)
                .padding(.vertical, 11)
            }
        }
        .padding(.top, 20)
    }
}
