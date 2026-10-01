"""The spreadsheet itself: one document, a Summary tab in front, a tab per
month behind it.

The document is owned by a person and shared with the service account,
because Google gives a service account no Drive storage of its own — one it
created would have nowhere to live. Creating it is a one-time step in
docs/BOOKS.md.
"""

import logging

import gspread
from gspread.utils import rowcol_to_a1

log = logging.getLogger("books")

SUMMARY_HEADER = ["Month", "In", "Out", "Net", "Subscribers", "Bank items"]


def open_books(key_file: str, title: str):
    gc = gspread.service_account(filename=key_file)
    try:
        return gc.open(title)
    except gspread.SpreadsheetNotFound:
        email = gc.auth.service_account_email
        log.error(
            "no spreadsheet named %r is shared with %s — create one in Google "
            "Sheets and share it with that address as an editor (docs/BOOKS.md)",
            title,
            email,
        )
        raise


def _column(rows: list) -> str:
    return rowcol_to_a1(1, len(rows[0])).rstrip("0123456789")


def write_month(sh, table) -> None:
    try:
        ws = sh.worksheet(table.tab)
    except gspread.WorksheetNotFound:
        ws = sh.add_worksheet(table.tab, rows=len(table.rows) + 10, cols=40)

    ws.clear()
    ws.update(table.rows, "A1")

    col = _column(table.rows)
    formats = [
        {
            "range": f"B{r}:{col}{r}",
            "format": {"numberFormat": {"type": "CURRENCY", "pattern": '"$"#,##0.00'}},
        }
        for r in table.money_rows
    ]
    formats += [
        {"range": f"A{r}:{col}{r}", "format": {"textFormat": {"bold": True}}}
        for r in table.bold_rows
    ]
    ws.batch_format(formats)
    ws.freeze(rows=1, cols=1)
    ws.columns_auto_resize(0, 1)


def write_summary(sh, months: dict) -> None:
    """Upsert one row per month: `months` maps a tab name to its totals."""
    try:
        ws = sh.worksheet("Summary")
    except gspread.WorksheetNotFound:
        try:
            ws = sh.worksheet("Sheet1")
            ws.update_title("Summary")
        except gspread.WorksheetNotFound:
            ws = sh.add_worksheet("Summary", rows=100, cols=10, index=0)

    existing = {row[0]: row for row in ws.get_all_values()[1:] if row and row[0]}
    for tab, t in months.items():
        existing[tab] = [
            tab,
            t.total_in,
            t.total_out,
            round(t.net, 2),
            t.subscribers,
            t.items,
        ]

    rows = [SUMMARY_HEADER] + [existing[k] for k in sorted(existing)]
    ws.clear()
    ws.update(rows, "A1")
    ws.batch_format(
        [
            {"range": "A1:F1", "format": {"textFormat": {"bold": True}}},
            {
                "range": f"B2:D{len(rows)}",
                "format": {
                    "numberFormat": {"type": "CURRENCY", "pattern": '"$"#,##0.00'}
                },
            },
        ]
    )
    ws.freeze(rows=1)
