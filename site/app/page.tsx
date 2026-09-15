import Image from "next/image";
import Link from "next/link";

import { AppStore } from "@/components/app-store";
import { Icon, type IconKind } from "@/components/icon";
import { Institution } from "@/components/institution";

/// What the markets side of the app actually gives you. Written as capability
/// rather than as feature names: "a charting engine" is a phrase, and "chart
/// any metric you can define over your own ledger" is a thing you can picture
/// doing.
const TOOLS: { icon: IconKind; title: string; body: string }[] = [
  {
    icon: "markets",
    title: "Charts that go deep",
    body: "Price and volume history on every asset you hold, drawn from real market data. Zoom from a day to a decade, overlay what you want to compare, and read it on the phone as well as the desktop.",
  },
  {
    icon: "portfolio",
    title: "Build a portfolio, then test it",
    body: "Assemble a portfolio out of anything the market offers, set a contribution, choose your assumptions, and watch how it would have played out. Run it against the one you actually hold.",
  },
  {
    icon: "ledger",
    title: "Chart anything you can define",
    body: "Define a metric over your own ledger and plot it — fees as a share of returns, exposure to one employer, cash drag over the year. For the questions a finance app cannot anticipate.",
  },
  {
    icon: "search",
    title: "Know your real return",
    body: "Time-weighted and contribution-adjusted, across every account at once, including the retirement accounts no brokerage dashboard can see. The number your statements never quite give you.",
  },
  {
    icon: "settings",
    title: "Costs, in the open",
    body: "What your funds charge, where your positions overlap, how much of this year's gains would be taxed as short-term. The facts that quietly decide what you keep.",
  },
  {
    icon: "connections",
    title: "Export it, or call the API",
    body: "Your ledger leaves in a format another program can read, and the same data is available over an API. Nothing here is designed to keep you from leaving.",
  },
];

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
    title: "No ads, ever.",
    body: "Meridian is paid for by the people who use it. There is no second business here that your ledger would be the raw material for, and nothing about you is for sale.",
  },
  {
    title: "We don't store your credentials.",
    body: "You sign in to your bank on your bank's own page, inside Plaid. Meridian is handed a token that reads and nothing more — it never sees what you typed, and it cannot move money.",
  },
  {
    title: "One tap, and it's gone.",
    body: "Disconnecting an institution deletes the accounts, transactions and holdings that came through it. Closing your account deletes the rest. No email, no retention period, no talking you out of it.",
  },
  {
    title: "Encrypted at the source.",
    body: "Your data is encrypted in transit and at rest, and the connection to your bank is encrypted end to end by Plaid before it ever reaches us.",
  },
];

export default function Home() {
  return (
    <main>
      <section className="hero">
        <div className="hero-wash" />
        <div className="wrap in">
          <div className="eyebrow">The all in one you&rsquo;re glad to see</div>
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
            {/* Where the button will go. The app does not exist yet, and a
                live-looking action that opens nothing is the one thing a
                landing page must not do. */}
            <span className="soon">Coming soon</span>
            <AppStore />
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

      <section className="sec" id="recurring">
        <div className="wrap split">
          <div>
            <div className="eyebrow">The full picture</div>
            <h2>Getting the full picture has never been so easy.</h2>
            <p className="lede">
              Every subscription, every bill, every standing order — found for
              you, totalled, and laid out on the month they land on. No
              spreadsheet, no combing through a year of statements, no
              discovering in March that you have been paying for something since
              June.
            </p>
            <p className="lede">
              It is the question almost nobody can answer about their own money:
              what leaves the account every month without being decided again.
              Meridian answers it on the first screen.
            </p>
          </div>

          <Device
            src="/shots/phone-recurring.avif"
            width={1500}
            height={2000}
            alt="Meridian on a phone, showing recurring expenses: the month's total, a breakdown by category, and a calendar of the days charges land on."
            className="phone-shot"
          />
        </div>
      </section>

      <section className="sec" id="ledger">
        <div className="wrap">
          <div className="eyebrow">One ledger</div>
          <h2>One ledger to rule them all.</h2>
          <p className="lede">
            Two banks, a credit card, the brokerage, the old 401k, the app your
            rent goes through. Six logins, six apps, six different ideas of what
            a month looks like — and no single place that adds them up. Working
            out where you actually stand means checking all of them and doing
            the arithmetic yourself.
          </p>
          <p className="lede">
            Connect them once and Meridian keeps them together from then on.
            Your spending sits next to your holdings, your debts next to what
            you own, and a question that crosses between them is one question
            rather than six.
          </p>
        </div>
      </section>

      <section className="sec" id="tools">
        <div className="wrap">
          <div className="eyebrow">Pro tooling</div>
          <h2>Pro tooling, for everyone.</h2>
          <p className="lede">
            The market data and charting a terminal gives a professional, in an
            app you already have open. Real prices, real history, and the tools
            to do something with them.
          </p>

          <div className="cards">
            {TOOLS.map((tool) => (
              <div className="card" key={tool.title}>
                <Icon kind={tool.icon} />
                <h3>{tool.title}</h3>
                <p>{tool.body}</p>
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
              Meridian comes with a built-in assistant to help you answer a
              question, build a projection, or find ways to save. It pulls
              directly from the data in the app, so what it tells you is what
              your accounts actually say today.
            </p>
            <p className="lede">
              And the model never does the arithmetic. Your question becomes a
              query, the engine computes the answer from your own ledger, and
              the model writes the sentence around it — so the figures are real
              rather than plausible.
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
            We reach your bank through Plaid, the connection trusted by major
            institutions and thousands of the apps you already use. Connecting
            an account is the largest thing Meridian asks of anybody, so here is
            exactly what it means.
          </p>

          <div className="points">
            {POINTS.map((point) => (
              <div className="point" key={point.title}>
                <h3>{point.title}</h3>
                <p>{point.body}</p>
              </div>
            ))}
          </div>

          <div className="points-cta">
            <Link className="btn plain" href="/privacy">
              <Icon kind="lock" />
              Read the privacy policy
            </Link>
          </div>
        </div>
      </section>

      <section className="sec" id="price">
        <div className="wrap split">
          <div>
            <div className="eyebrow">Price</div>
            <h2>One price, and nothing else to choose.</h2>
            <p className="lede">
              Everybody pays the same, and it buys the whole app — every account
              you connect, every tool on the markets side, and the assistant.
              There is nothing above this and nothing below it.
            </p>
            <p className="lede">
              No free tier paid for by what it learns about you, and no upgrade
              you find out about the week you need it. Cancel whenever you like
              and it runs to the end of the period you have paid for.
            </p>
          </div>

          <Price />
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
            <span className="soon">Coming soon</span>
            <AppStore />
            <Link className="btn plain" href="/terms">
              Read the terms
            </Link>
          </div>
        </div>
      </section>
    </main>
  );
}

/// The price: the figure in the app's money face, and the facts under it in the
/// app's rows.
///
/// A panel rather than one of a row of cards. There is one price, so there is
/// nothing to compare it against and no grid to put it in — the shape a pricing
/// page usually takes is an argument between three columns, and this page has no
/// argument to stage.
function Price() {
  return (
    <div className="price">
      <div className="price-fig">
        <span className="money">$5.99</span>
        <span className="per">a month</span>
      </div>

      <div className="rows">
        <div className="row flat">
          <span className="rt">Included</span>
          <span className="rv hard">Every account, every tool</span>
        </div>
        <div className="row flat">
          <span className="rt">Billed</span>
          <span className="rv hard">Monthly, by card</span>
        </div>
        <div className="row flat">
          <span className="rt">Tax</span>
          <span className="rv hard">Included, not added at the end</span>
        </div>
        <div className="row flat">
          <span className="rt">Cancel</span>
          <span className="rv hard">Whenever you like</span>
        </div>
      </div>

      <div className="price-note">
        Payments are taken by Stripe. Your card goes to them and never reaches
        Meridian.
      </div>
    </div>
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

/// A product shot: the app on a device, rendered in Blender out of a picture of
/// the screen taken from the sketches. `render/render.sh` makes them.
///
/// The renders have a transparent background and are lit from one side, so this
/// gives them no frame and no ground — the page is the room they are standing
/// in, and a card drawn round one would be a second device around the first.
function Device({
  src,
  width,
  height,
  alt,
  className,
}: {
  src: string;
  width: number;
  height: number;
  alt: string;
  className?: string;
}) {
  return (
    <Image
      className={`device${className ? ` ${className}` : ""}`}
      src={src}
      width={width}
      height={height}
      alt={alt}
    />
  );
}
