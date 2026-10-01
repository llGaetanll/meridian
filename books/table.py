"""The month table the sheet shows: one column per day, money in against
money out, and a Month column summing each row."""

import calendar
from dataclasses import dataclass, field
from datetime import date


@dataclass
class DayFlows:
    """One day's movements. Money in dollars, counts as of end of day."""

    gross: float = 0.0
    fees: float = 0.0
    refunds: float = 0.0
    subscribers: int = 0
    items: int = 0


@dataclass
class FixedLine:
    """A cost that does not move with subscribers, in dollars per month."""

    label: str
    monthly: float


@dataclass
class MonthTable:
    """The rows to write, which of them carry money, and the totals the
    Summary tab wants."""

    tab: str
    rows: list = field(default_factory=list)
    money_rows: list = field(default_factory=list)
    bold_rows: list = field(default_factory=list)
    total_in: float = 0.0
    total_out: float = 0.0
    subscribers: int = 0
    items: int = 0

    @property
    def net(self) -> float:
        return self.total_in - self.total_out


def build(
    year: int,
    month: int,
    upto: date,
    flows: dict,
    fixed: list,
    plaid_item_monthly: float,
) -> MonthTable:
    """One month as rows. Days after `upto` stay blank rather than showing a
    projection; fixed costs are amortized over the month's days."""

    day_count = calendar.monthrange(year, month)[1]
    days = [date(year, month, d) for d in range(1, day_count + 1)]
    filled = [d for d in days if d <= upto]

    def flow(d: date) -> DayFlows:
        return flows.get(d, DayFlows())

    gross = {d: flow(d).gross for d in filled}
    fees = {d: flow(d).fees for d in filled}
    refunds = {d: flow(d).refunds for d in filled}
    plaid = {d: flow(d).items * plaid_item_monthly / day_count for d in filled}

    fixed_days = {f.label: {d: f.monthly / day_count for d in filled} for f in fixed}
    out = {
        d: fees[d] + refunds[d] + plaid[d] + sum(f.monthly for f in fixed) / day_count
        for d in filled
    }
    net = {d: gross[d] - out[d] for d in filled}

    table = MonthTable(tab=f"{year:04d}-{month:02d}")

    def push(row: list, bold: bool = False, money: bool = False) -> None:
        table.rows.append(row)
        if bold:
            table.bold_rows.append(len(table.rows))
        if money:
            table.money_rows.append(len(table.rows))

    def money_row(label: str, per_day: dict, bold: bool = False) -> None:
        row = [label]
        row += [round(per_day[d], 2) if d in per_day else "" for d in days]
        row.append(round(sum(per_day.values()), 2))
        push(row, bold=bold, money=True)

    def count_row(label: str, value) -> None:
        row = [label]
        row += [value(d) if d in filled else "" for d in days]
        row.append(value(filled[-1]) if filled else "")
        push(row)

    label = calendar.month_abbr[month]
    push([""] + [f"{label} {d.day}" for d in days] + ["Month"], bold=True)

    push(["Money in"], bold=True)
    money_row("Subscriptions", gross)
    money_row("Total in", gross, bold=True)
    push([])

    push(["Money out"], bold=True)
    money_row("Stripe fees", fees)
    money_row("Refunds", refunds)
    money_row("Plaid", plaid)
    for f in fixed:
        money_row(f.label, fixed_days[f.label])
    money_row("Total out", out, bold=True)
    push([])

    money_row("Net", net, bold=True)
    push([])

    count_row("Subscribers", lambda d: flow(d).subscribers)
    count_row("Bank items", lambda d: flow(d).items)

    table.total_in = round(sum(gross.values()), 2)
    table.total_out = round(sum(out.values()), 2)
    if filled:
        table.subscribers = flow(filled[-1]).subscribers
        table.items = flow(filled[-1]).items
    return table
