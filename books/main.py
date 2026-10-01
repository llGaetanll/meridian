"""A daily bookkeeping pass: pull the month's numbers, rewrite the sheet.

Runs forever and writes once a day at BOOKS_RUN_AT, or a single pass with
`main.py once`. Every pass rewrites the current month and the one before it,
so the run just after midnight on the 1st finalizes the month that ended.
"""

import calendar
import logging
import os
import sys
import time as systime
import tomllib
from datetime import date, datetime, time, timedelta
from pathlib import Path
from zoneinfo import ZoneInfo

import sheet
import sources
import table

log = logging.getLogger("books")


def config() -> dict:
    return {
        "google_key": os.environ.get("BOOKS_GOOGLE_KEY", "certs/books-google-key.json"),
        "stripe_key": os.environ.get("BOOKS_STRIPE_KEY", ""),
        "database_url": os.environ.get("DATABASE_URL", ""),
        "title": os.environ.get("BOOKS_SHEET_TITLE", "Meridian Books"),
        "tz": ZoneInfo(os.environ.get("BOOKS_TZ", "America/New_York")),
        "run_at": os.environ.get("BOOKS_RUN_AT", "06:30"),
    }


def load_costs() -> tuple:
    path = Path(__file__).parent / "costs.toml"
    with open(path, "rb") as f:
        data = tomllib.load(f)
    fixed = [table.FixedLine(f["label"], f["monthly"]) for f in data["fixed"]]
    return fixed, data["plaid"]["bank_item_monthly"]


def month_flows(cfg: dict, first: date, last: date) -> dict:
    money = sources.stripe_days(cfg["stripe_key"], cfg["tz"], first, last)
    counts = sources.db_days(cfg["database_url"], cfg["tz"], first, last)

    flows = {}
    day = first
    while day <= last:
        gross, fees, refunds = money.get(day, (0.0, 0.0, 0.0))
        subscribers, items = counts.get(day, (0, 0))
        flows[day] = table.DayFlows(gross, fees, refunds, subscribers, items)
        day += timedelta(days=1)
    return flows


def run_pass(cfg: dict) -> None:
    fixed, plaid_rate = load_costs()
    today = datetime.now(cfg["tz"]).date()

    this_month = (today.year, today.month)
    last_month = (today.year - 1, 12) if today.month == 1 else (today.year, today.month - 1)

    sh = sheet.open_books(cfg["google_key"], cfg["title"])

    written = {}
    for year, month in (last_month, this_month):
        first = date(year, month, 1)
        last = min(date(year, month, calendar.monthrange(year, month)[1]), today)
        flows = month_flows(cfg, first, last)
        t = table.build(year, month, today, flows, fixed, plaid_rate)
        sheet.write_month(sh, t)
        written[t.tab] = t
        log.info(
            "%s: in %.2f, out %.2f, net %.2f", t.tab, t.total_in, t.total_out, t.net
        )

    sheet.write_summary(sh, written)
    log.info("wrote %s", sh.url)


def sleep_until_next(cfg: dict) -> None:
    hour, minute = (int(p) for p in cfg["run_at"].split(":"))
    now = datetime.now(cfg["tz"])
    at = datetime.combine(now.date(), time(hour, minute), tzinfo=cfg["tz"])
    if at <= now:
        at += timedelta(days=1)
    seconds = (at - now).total_seconds()
    log.info("next pass at %s", at.isoformat())
    systime.sleep(seconds)


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    cfg = config()

    if "once" in sys.argv[1:]:
        run_pass(cfg)
        return

    while True:
        try:
            run_pass(cfg)
        except Exception:
            log.exception("pass failed; trying again at the next run time")
        sleep_until_next(cfg)


if __name__ == "__main__":
    main()
