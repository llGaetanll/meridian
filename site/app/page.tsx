import Image from "next/image";
import Link from "next/link";

import { Icon, type IconKind } from "@/components/icon";
import { Institution } from "@/components/institution";
import { site } from "@/lib/site";

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

/// The institutions the marquee names. Set as wordmarks in the site's own type
/// rather than as logo files: these are other companies' trademarks, and a page
/// that reproduces them is making a claim about them. The names say the same
/// thing and are ours to write.
///
/// Chosen to be recognisable across the kinds of account a connection reaches —
/// the national banks, the card issuers, the brokerages, the newer accounts —
/// so the row reads as "all of it" rather than as a list of four banks.
const INSTITUTIONS = [
  "Chase",
  "Bank of America",
  "Wells Fargo",
  "American Express",
  "Discover",
  "Citi",
  "Capital One",
  "Fidelity",
  "Charles Schwab",
  "Robinhood",
  "Vanguard",
  "U.S. Bank",
  "PNC",
  "Truist",
  "Ally",
  "SoFi",
  "Navy Federal",
  "E*TRADE",
];

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
        </div>

        {/* Pulled up so the top of the lid crosses the last line of the
            headline. The machine is the description — the page says what the
            app is by showing it, which is why there is no paragraph here. */}
        <div className="hero-shot">
          <Image
            src="/shots/laptop-markets.avif"
            width={2200}
            height={1350}
            alt="Meridian on a laptop, showing the markets page for SPY: the price chart, the position across three portfolios, and the trade composer."
            priority
          />
        </div>

        <div className="wrap">
          <div className="cta">
            <a className="btn primary lg" href={site.app}>
              Open the app
            </a>
            <a className="btn plain" href="#coverage">
              Learn more
            </a>
          </div>
        </div>
      </section>

      <section className="sec" id="coverage">
        <div className="wrap">
          <div className="eyebrow">Coverage</div>
          <h2>We support your bank.</h2>
          <p className="lede">
            Meridian connects through Plaid, the same service your other
            financial apps use. Every institution Plaid supports, Meridian
            supports — every national bank, every regional one, the credit
            unions, the card issuers and the brokerages. We keep no shorter list
            of our own.
          </p>
        </div>

        <Marquee />
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
            <h2>Ask in plain English, never do the math again.</h2>
            <p className="lede">
              Why was August so expensive? What are my fees actually costing me?
              What happens if I put another $500 a month in? Ask it the way you
              would ask a friend who happens to be good with money — and get an
              answer that is right, every time.
            </p>
            <p className="lede">
              Because the model never does the arithmetic. Your question becomes
              a query, the engine computes the answer from your own ledger, and
              the model just writes the sentence around it. No hallucinated
              figures, no plausible-looking nonsense — and your transactions
              never leave the machine.
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

/// The institutions, sliding right to left, forever.
///
/// The row is written out twice and the track is translated by exactly half its
/// width, so the second copy is where the first was when the animation restarts
/// and the loop has no seam. `aria-hidden` on the duplicate keeps a screen
/// reader from hearing the list twice; the marquee stops for anybody who has
/// asked their machine for less motion.
function Marquee() {
  return (
    <div className="marquee">
      <div className="marquee-track">
        <div className="marquee-row">
          {INSTITUTIONS.map((name) => (
            <Institution key={name} name={name} />
          ))}
        </div>
        <div className="marquee-row" aria-hidden="true">
          {INSTITUTIONS.map((name) => (
            <Institution key={name} name={name} />
          ))}
        </div>
      </div>
    </div>
  );
}
