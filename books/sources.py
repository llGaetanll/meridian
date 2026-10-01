"""Where the numbers come from: Stripe for the money, Postgres for the
counts. A source being unreachable is a logged gap, never a crash — the
fixed costs still get written."""

import logging
from datetime import date, datetime, time, timedelta

import psycopg
import requests

log = logging.getLogger("books")

STRIPE_URL = "https://api.stripe.com/v1/balance_transactions"


def stripe_days(key: str, tz, first: date, last: date) -> dict:
    """Per-day (gross, fees, refunds) in dollars, from balance transactions.

    Balance transactions rather than charges, because the fee on each is the
    fee Stripe actually took rather than an estimate of it.
    """
    if not key:
        log.warning("BOOKS_STRIPE_KEY is empty; money in and fees read zero")
        return {}

    start = datetime.combine(first, time.min, tzinfo=tz)
    end = datetime.combine(last + timedelta(days=1), time.min, tzinfo=tz)
    params = {
        "created[gte]": int(start.timestamp()),
        "created[lt]": int(end.timestamp()),
        "limit": "100",
    }

    txns = []
    try:
        while True:
            resp = requests.get(STRIPE_URL, params=params, auth=(key, ""), timeout=30)
            resp.raise_for_status()
            page = resp.json()
            txns += page["data"]
            if not page.get("has_more"):
                break
            params["starting_after"] = page["data"][-1]["id"]
    except requests.RequestException as e:
        log.error("stripe unreachable, money in and fees read zero: %s", e)
        return {}

    out: dict = {}
    for t in txns:
        day = datetime.fromtimestamp(t["created"], tz).date()
        gross, fees, refunds = out.get(day, (0.0, 0.0, 0.0))
        kind = t["type"]
        if kind in ("charge", "payment"):
            gross += t["amount"] / 100
            fees += t["fee"] / 100
        elif kind in ("refund", "payment_refund"):
            refunds += -t["amount"] / 100
            fees += t["fee"] / 100
        elif kind == "stripe_fee":
            fees += -t["amount"] / 100
        else:
            continue
        out[day] = (gross, fees, refunds)
    return out


def db_days(url: str, tz, first: date, last: date) -> dict:
    """Per-day (subscribers, items), reconstructed from created_at.

    An Item removed at the user's request is deleted outright, so history
    undercounts by exactly the Items Plaid has also stopped billing for.
    """
    if not url:
        log.warning("DATABASE_URL is empty; subscriber and Item counts read zero")
        return {}

    try:
        with psycopg.connect(url, connect_timeout=10) as conn:
            items = [r[0] for r in conn.execute("SELECT created_at FROM items")]
            subs = list(
                conn.execute("SELECT created_at, current_period_end FROM subscriptions")
            )
    except psycopg.Error as e:
        log.error("database unreachable, counts read zero: %s", e)
        return {}

    out = {}
    day = first
    while day <= last:
        day_start = datetime.combine(day, time.min, tzinfo=tz)
        day_end = day_start + timedelta(days=1)
        n_items = sum(1 for created in items if created < day_end)
        n_subs = sum(
            1
            for created, period_end in subs
            if created < day_end and period_end is not None and period_end > day_start
        )
        out[day] = (n_subs, n_items)
        day += timedelta(days=1)
    return out
