import Link from "next/link";

import { Icon, type IconKind } from "@/components/icon";
import { site } from "@/lib/site";

/// The line in the panel at the top: four years of a net worth that goes up
/// through a drawdown it recovers from. A path rather than an image, so it is
/// sharp at any size and costs nothing to load.
const LINE =
  "M 0 104.2 L 15.7 100 L 31.4 84.5 L 47.1 89.4 L 62.7 85.7 L 78.4 79.9 L 94.1 78.5 L 109.8 82.4 L 125.5 96 L 141.2 87.6 L 156.9 84.4 L 172.5 92 L 188.2 80.5 L 203.9 89.3 L 219.6 71.1 L 235.3 66 L 251 69.8 L 266.7 44.9 L 282.4 47.6 L 298 33.8 L 313.7 33.1 L 329.4 36.4 L 345.1 44 L 360.8 43.3 L 376.5 49.8 L 392.2 60.4 L 407.8 71.1 L 423.5 86.1 L 439.2 98.1 L 454.9 97 L 470.6 109.6 L 486.3 119.6 L 502 124 L 517.6 120 L 533.3 102.7 L 549 96.2 L 564.7 98.3 L 580.4 91.6 L 596.1 82 L 611.8 58.5 L 627.5 52.8 L 643.1 48.7 L 658.8 41.9 L 674.5 41.2 L 690.2 31 L 705.9 31.4 L 721.6 35 L 737.3 32.5 L 752.9 23.3 L 768.6 26.4 L 784.3 11.9 L 800 10";

const CARDS: { icon: IconKind; title: string; body: string }[] = [
  {
    icon: "search",
    title: "Surface what is there",
    body: "Where the cash is sitting and what it earns, what the funds charge, where positions overlap, how much of the year's gains are short-term. Facts you would otherwise go looking for across five websites.",
  },
  {
    icon: "portfolio",
    title: "Make equity comp legible",
    body: "Vest schedules, how much of the total sits in one employer, what a sale would realize. The part most people do in a spreadsheet a coworker sent them.",
  },
  {
    icon: "markets",
    title: "Report real return",
    body: "Time-weighted and contribution-adjusted, across every account at once. The 401k included, which is the one nobody can see.",
  },
  {
    icon: "settings",
    title: "Run the scenario you build",
    body: "Assemble a portfolio, set a contribution, pick the assumptions, and see how it plays out. You choose what to try; the app does the arithmetic.",
  },
  {
    icon: "ledger",
    title: "Chart anything derived",
    body: "Any metric over the ledger, defined by you, plotted. For the questions a product cannot anticipate.",
  },
  {
    icon: "connections",
    title: "Export it, or call the API",
    body: "Your ledger leaves in a format something else can read. Nothing here is trying to keep you in.",
  },
];

const TRADES = [
  { what: "Paycheck", trade: "nothing → 4,206.55 USD", amt: "+$4,206.55", dir: "in" },
  { what: "Lunch", trade: "14.20 USD → 1 burrito", amt: "−$14.20", dir: "out" },
  { what: "Bought VTI", trade: "1,180.44 USD → 4.021 VTI", amt: "−$1,180.44", dir: "out" },
  { what: "Bought bitcoin", trade: "3,204.00 USD → 0.0312 BTC", amt: "−$3,204.00", dir: "out" },
] as const;

const POINTS = [
  {
    title: "Connections are read-only",
    body: "Banks and brokerages connect through Plaid. Meridian reads balances, transactions and holdings. It cannot move money, and it is not asking for the ability to.",
  },
  {
    title: "Your bank credentials never reach us",
    body: "You sign into your bank on your bank's own page, inside Plaid. Meridian is handed a token for reading, and never sees what you typed.",
  },
  {
    title: "Remove a bank and its history goes with it",
    body: "Disconnecting a bank deletes the accounts, transactions and holdings that came through it. Closing your account deletes the rest.",
  },
  {
    title: "Nothing is sold, and there are no ads",
    body: "Meridian is paid for by the people who use it. There is no second business here that your ledger is the raw material for.",
  },
];

export default function Home() {
  return (
    <main>
      <section className="hero">
        <div className="hero-wash" />
        <div className="wrap in">
          <div className="eyebrow">Money, in one place</div>
          <h1>
            Everything you own, everything you owe,{" "}
            <span className="quiet">everything you spend.</span>
          </h1>
          <p>
            Meridian keeps one ledger for your accounts, your holdings and your
            spending, with the tools to explore what any of it means. It shows
            you your money. It does not tell you what to do with it.
          </p>

          <div className="cta">
            <a className="btn primary lg" href={site.app}>
              Open the app
              <Icon kind="arrowRight" />
            </a>
            <a className="btn plain" href="#what">
              See what it does
            </a>
          </div>

          <Shot />
        </div>
      </section>

      <section className="sec" id="what">
        <div className="wrap">
          <div className="eyebrow">What it does</div>
          <h2>Facts about your money, stated plainly.</h2>
          <p className="lede">
            The budgeting apps answer whether you are spending too much. Meridian
            answers where you actually stand, and what would happen if you
            changed something. Every one of these ends in a number on screen,
            not a suggestion.
          </p>

          <div className="cards">
            {CARDS.map((card) => (
              <div className="card" key={card.title}>
                <Icon kind={card.icon} />
                <h3>{card.title}</h3>
                <p>{card.body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="sec" id="ledger">
        <div className="wrap split">
          <div>
            <div className="eyebrow">One ledger</div>
            <h2>A paycheck and a share of VTI are the same kind of event.</h2>
            <p className="lede">
              Spending apps and portfolio apps keep two models and bolt a net
              worth number on top. Meridian keeps one. Everything in it is a
              trade — some amount of one thing for some amount of another, at a
              time — and money is not special, just a unit.
            </p>
            <p className="lede">
              So a question that crosses between spending and investing is just
              a query over one list. That is an implementation choice rather
              than a feature, and it is what makes everything above cheap to
              build instead of impossible.
            </p>
          </div>

          <div className="trades">
            {TRADES.map((trade) => (
              <div className="trade" key={trade.what}>
                <div>
                  <span className="what">{trade.what}</span>
                  <span className="sub money">{trade.trade}</span>
                </div>
                <span className={`amt money ${trade.dir}`}>{trade.amt}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="sec" id="assistant">
        <div className="wrap split">
          <div>
            <div className="eyebrow">The assistant</div>
            <h2>Ask a question. The model never does the math.</h2>
            <p className="lede">
              Ask why last month looked different, what the fees add up to, how
              a scenario turns out. The question becomes a query, the engine
              computes the answer, and the model explains the result — so every
              number is deterministic, reproducible and testable.
            </p>
            <p className="lede">
              It also means what leaves the machine is a structured query and a
              structured answer, never a dump of your transactions.
            </p>
          </div>

          <Ask />
        </div>
      </section>

      <section className="sec" id="data">
        <div className="wrap">
          <div className="eyebrow">Your data</div>
          <h2>Read-only, and yours to remove.</h2>
          <p className="lede">
            Connecting a bank is the largest thing this app asks of anybody, so
            here is exactly what it means.
          </p>

          <div className="points">
            {POINTS.map((point) => (
              <div className="point" key={point.title}>
                <h3>{point.title}</h3>
                <p>{point.body}</p>
              </div>
            ))}
          </div>

          <Link className="btn" href="/privacy">
            <Icon kind="lock" />
            Read the privacy policy
          </Link>
        </div>
      </section>

      <section className="close">
        <div className="wrap">
          <div className="eyebrow">Meridian</div>
          <h2>See where you actually stand.</h2>
          <p>
            One ledger, every account, and the tools to ask it anything. No ads,
            no data sold, and no advice you did not ask for.
          </p>
          <div className="cta">
            <a className="btn primary lg" href={site.app}>
              Open the app
              <Icon kind="arrowRight" />
            </a>
            <Link className="btn plain" href="/terms">
              Read the terms
            </Link>
          </div>
        </div>
      </section>
    </main>
  );
}

/// One question and its answer, in the app's field and the app's rows. The
/// numbers add up, because a screenshot of an assistant whose arithmetic is
/// wrong is an argument against the thing it is illustrating.
function Ask() {
  return (
    <div className="ask" aria-hidden="true">
      <span className="field on">
        <span className="in">
          <Icon kind="search" />
          Why was August higher than July?
          <i className="caret" />
        </span>
      </span>

      <div className="ask-a">
        <p>
          August was <b className="money">$1,312.40</b> higher, and three things
          account for almost all of it. Two of them happen once a year.
        </p>
        <div className="rows">
          <div className="row flat">
            <span className="rt">Insurance — annual premium</span>
            <span className="rv hard money">$742.00</span>
          </div>
          <div className="row flat">
            <span className="rt">Travel — one trip, four charges</span>
            <span className="rv hard money">$389.55</span>
          </div>
          <div className="row flat">
            <span className="rt">Groceries — one extra week</span>
            <span className="rv hard money">$118.40</span>
          </div>
        </div>
        <div className="ask-note">
          The engine computed those three figures from your ledger. The model was
          given them, and wrote the sentence.
        </div>
      </div>
    </div>
  );
}

/// The app's home screen, built from the app's own components: the figure, what
/// came in and went out, the line, and the last few rows of the ledger.
function Shot() {
  return (
    <div className="shot" aria-hidden="true">
      <div className="shot-head">
        <div>
          <div className="k">Net worth</div>
          <div className="v">$412,908</div>
          <div className="diff">
            <span className="in">
              <b>+$14,204</b> in
            </span>
            <span className="out">
              <b>−$6,118</b> out
            </span>
            <span>this month</span>
          </div>
        </div>
        <div className="btn-group">
          <button className="btn sm" type="button">1M</button>
          <button className="btn sm" type="button">6M</button>
          <button className="btn sm on" type="button">4Y</button>
        </div>
      </div>

      <svg className="shot-chart" viewBox="0 0 800 132" preserveAspectRatio="none">
        <defs>
          <linearGradient id="shot-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--violet-400)" stopOpacity="0.26" />
            <stop offset="100%" stopColor="var(--violet-400)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <path d={`${LINE} L 800 132 L 0 132 Z`} fill="url(#shot-fill)" />
        <path
          d={LINE}
          fill="none"
          stroke="var(--violet-400)"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>

      <div className="shot-rows">
        <ShotRow
          mark="AC"
          name="Acme, Inc."
          cat="Payroll"
          acct="Checking · 4491"
          amt="+$4,206.55"
          dir="in"
        />
        <ShotRow
          mark="VG"
          name="Vanguard"
          cat="Bought 4.021 VTI"
          acct="Brokerage · 8820"
          amt="−$1,180.44"
          dir="out"
        />
        <ShotRow
          mark="WF"
          name="Whole Foods Market"
          cat="Groceries"
          acct="Credit · 1174"
          amt="−$84.12"
          dir="out"
        />
      </div>
    </div>
  );
}

function ShotRow({
  mark,
  name,
  cat,
  acct,
  amt,
  dir,
}: {
  mark: string;
  name: string;
  cat: string;
  acct: string;
  amt: string;
  dir: "in" | "out";
}) {
  return (
    <div className="shot-row">
      <span className="mark">{mark}</span>
      <span>
        <span className="nm">{name}</span>
        <span className="ct">{cat}</span>
      </span>
      <span className="ct acct">{acct}</span>
      <span className={`amt money ${dir}`}>{amt}</span>
    </div>
  );
}
