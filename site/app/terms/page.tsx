import type { Metadata } from "next";
import Link from "next/link";

import { LegalDoc, type LegalSection } from "@/components/legal-doc";
import { legal } from "@/lib/legal";
import { site } from "@/lib/site";

export const metadata: Metadata = {
  title: "Terms of Service",
  description:
    "The agreement between you and Meridian: what the service is, what it is not, and what each of us is responsible for.",
};

const SECTIONS: LegalSection[] = [
  {
    id: "the-agreement",
    title: "The agreement",
    body: (
      <>
        <p>
          These Terms of Service are the agreement between you and{" "}
          {legal.operator}, a limited liability company based in {legal.based}{" "}
          (&ldquo;Meridian&rdquo;, &ldquo;we&rdquo;, &ldquo;us&rdquo;), covering
          your use of the Meridian applications, the website at {site.domain},
          and the API behind them (together, the &ldquo;Service&rdquo;).
        </p>
        <p>
          By creating an account or using the Service you accept these terms. If
          you do not accept them, do not use the Service. Our{" "}
          <Link href="/privacy">Privacy Policy</Link> explains what we do with
          your information and forms part of this agreement.
        </p>
      </>
    ),
  },
  {
    id: "your-account",
    title: "Your account",
    body: (
      <>
        <p>
          You must be at least 18 years old and able to enter a contract where
          you live. An account belongs to one person; it is not for a business,
          a household or a group, and it is not to be shared.
        </p>
        <ul>
          <li>
            Give us an email address you control. Signing in sends a code to
            that address, so whoever reads it can reach your account.
          </li>
          <li>
            Keep your devices and your email secure, and tell us at{" "}
            <a href={`mailto:${site.support}`}>{site.support}</a> if you believe
            somebody else has reached your account.
          </li>
          <li>
            You are responsible for what happens under your account, except
            where it is our fault.
          </li>
        </ul>
      </>
    ),
  },
  {
    id: "what-meridian-is-not",
    title: "What Meridian is not",
    body: (
      <>
        <p>
          <strong>
            Meridian is not a bank, a broker-dealer, an investment adviser, a
            tax adviser or an accountant, and nothing in the Service is financial,
            investment, legal or tax advice.
          </strong>{" "}
          We do not recommend securities, we do not predict markets, and we do
          not tell you what to do with your money.
        </p>
        <p>
          The Service describes money you already have and computes scenarios you
          build yourself. A figure it shows you — a balance, a return, a
          projection, a category — is information, not a recommendation and not a
          promise. Projections are arithmetic on assumptions you chose; they are
          not forecasts and the future will not match them.
        </p>
        <p>
          Decisions about your money are yours. Where a number matters — a tax
          filing, a trade, a loan — check it against your institution&rsquo;s own
          statements and, where it is warranted, with a professional.
        </p>
      </>
    ),
  },
  {
    id: "connecting-accounts",
    title: "Connecting your accounts",
    body: (
      <>
        <p>
          You connect banks, brokerages and other institutions through Plaid Inc.
          You sign in on your institution&rsquo;s own page inside Plaid&rsquo;s
          flow, and Plaid returns to us a token that reads that connection.
        </p>
        <ul>
          <li>
            <strong>Access is read-only.</strong> The Service reads balances,
            transactions, holdings and account details. It cannot move money,
            place trades or change anything at your institution.
          </li>
          <li>
            You may only connect accounts you own or are authorised to access,
            and you authorise us and Plaid to retrieve that data on your behalf
            for as long as the connection exists.
          </li>
          <li>
            Plaid&rsquo;s handling of your data is governed by{" "}
            <a href="https://plaid.com/legal/#end-user-privacy-policy">
              Plaid&rsquo;s end user privacy policy
            </a>
            , not by these terms.
          </li>
          <li>
            Institutions change formats, go down, and are sometimes wrong. We
            show what we are given; we do not guarantee that it is complete,
            current or accurate.
          </li>
        </ul>
        <p>
          You can disconnect an institution at any time. Doing so removes the
          accounts, transactions and holdings that came through it.
        </p>
      </>
    ),
  },
  {
    id: "subscription",
    title: "Subscription and payment",
    body: (
      <>
        <p>
          Paid plans are billed in advance for the period you choose, and renew
          automatically at the end of each period until you cancel. Prices are
          shown before you subscribe and include any tax we are required to
          collect, so the figure you see is the figure you are charged.
        </p>
        <ul>
          <li>
            <strong>Cancel any time, and cancelling closes the account.</strong>{" "}
            Cancelling stops the next renewal and deletes your account: we
            disconnect your institutions and remove your data. There is no
            paused state, and no undo. If you come back, you sign up again and
            reconnect your accounts.
          </li>
          <li>
            You can cancel through the app at any point, including while a
            payment has failed or after a subscription has lapsed. We never
            require a subscription in order to let you leave.
          </li>
          <li>
            Payments are handled by our payment processor. We do not receive or
            store your card number.
          </li>
          <li>
            We may change prices for future periods, and will tell you by email
            at least 30 days before a change takes effect. Continuing after that
            date is acceptance of the new price.
          </li>
          <li>
            Fees already paid are non-refundable except where the law requires a
            refund, or where we have failed to provide the Service. If you think
            you were billed in error, write to us and we will look at it.
          </li>
          <li>
            If a payment fails we may suspend the paid parts of the Service until
            it is settled.
          </li>
        </ul>
        <p>
          Some parts of the Service may be offered free, in trial, or in beta. We
          may change or end those at any time.
        </p>
      </>
    ),
  },
  {
    id: "acceptable-use",
    title: "Acceptable use",
    body: (
      <>
        <p>Use the Service for your own finances. Do not:</p>
        <ul>
          <li>
            access an account, an institution or data that is not yours to
            access;
          </li>
          <li>
            resell the Service, or use it to provide a service to somebody else;
          </li>
          <li>
            interfere with it — probing, overloading, circumventing rate limits
            or authentication, or scraping outside the API and its limits;
          </li>
          <li>
            use it to break the law, or in a way that would put us in breach of
            our own agreements with Plaid or our other providers.
          </li>
        </ul>
        <p>
          We may set reasonable limits — on requests, on connected institutions,
          on stored history — and will say what they are.
        </p>
      </>
    ),
  },
  {
    id: "your-data",
    title: "Your data stays yours",
    body: (
      <>
        <p>
          Your financial data is yours. You give us the permission we need to run
          the Service for you: to store it, process it, compute over it, and show
          it back to you. That permission exists so the Service can work, it
          lasts as long as your account does, and it is not a licence to do
          anything else with it.
        </p>
        <p>
          We do not sell your data and we do not use it for advertising. What we
          collect and who we share it with is set out in the{" "}
          <Link href="/privacy">Privacy Policy</Link>. You can export your ledger
          at any time.
        </p>
      </>
    ),
  },
  {
    id: "availability",
    title: "Availability and changes",
    body: (
      <>
        <p>
          We will try to keep the Service running and accurate, but we do not
          promise a level of uptime. It depends on institutions, on Plaid, and on
          infrastructure we do not own, and any of those can fail.
        </p>
        <p>
          We may add, change or remove features. If a change materially reduces
          what you are paying for, we will tell you by email before it takes
          effect, and you may cancel and receive a pro-rated refund of the unused
          part of your current period.
        </p>
      </>
    ),
  },
  {
    id: "termination",
    title: "Ending the agreement",
    body: (
      <>
        <p>
          You can close your account at any time, from the app or by writing to
          us. We may suspend or close an account that breaks these terms, that
          creates a legal or security risk, or that we are required to close —
          and other than in an emergency we will tell you why first and give you
          a chance to put it right.
        </p>
        <p>
          When an account is closed we delete its data on the schedule in the{" "}
          <Link href="/privacy">Privacy Policy</Link>, and we remove the
          connections to your institutions. Export anything you want to keep
          before you close it.
        </p>
      </>
    ),
  },
  {
    id: "disclaimer",
    title: "Disclaimer",
    body: (
      <p>
        To the extent the law allows, the Service is provided &ldquo;as is&rdquo;
        and &ldquo;as available&rdquo;, without warranties of any kind, express
        or implied, including merchantability, fitness for a particular purpose,
        accuracy and non-infringement. We do not warrant that the Service will be
        uninterrupted, error-free, or that the data it shows is complete or
        correct. Some jurisdictions do not allow these exclusions, in which case
        they do not apply to you.
      </p>
    ),
  },
  {
    id: "liability",
    title: "Limitation of liability",
    body: (
      <>
        <p>
          To the extent the law allows, neither party is liable for indirect,
          incidental, special or consequential damages, or for lost profits,
          lost opportunities or lost data, arising out of the Service.
        </p>
        <p>
          Our total liability arising out of or relating to the Service is
          limited to the greater of the amounts you paid us in the twelve months
          before the event giving rise to the claim, and one hundred US dollars.
        </p>
        <p>
          Nothing here limits liability that cannot be limited by law, including
          for fraud, or for death or personal injury caused by negligence.
        </p>
      </>
    ),
  },
  {
    id: "indemnity",
    title: "Indemnity",
    body: (
      <p>
        You will defend and indemnify us against third-party claims arising from
        your use of the Service in breach of these terms or of the law, including
        connecting an account you were not authorised to connect. We will tell
        you promptly about any such claim and let you control the defence, and we
        will not settle it without your consent.
      </p>
    ),
  },
  {
    id: "changes",
    title: "Changes to these terms",
    body: (
      <p>
        We may update these terms. If a change is material we will email you and
        post the new version here at least 30 days before it takes effect, and
        the date at the top will say when it changed. Using the Service after
        that date is acceptance; if you do not accept, close your account before
        it and we will refund the unused part of your current period.
      </p>
    ),
  },
  {
    id: "law",
    title: "Governing law and disputes",
    body: (
      <>
        <p>
          These terms are governed by the laws of {legal.jurisdiction}, without
          regard to its conflict-of-laws rules, and the parties submit to{" "}
          {legal.courts}. If you are a consumer somewhere whose law gives you
          rights that cannot be taken away by contract, this does not take them
          away.
        </p>
        <p>
          Before filing anything, write to us at{" "}
          <a href={`mailto:${site.support}`}>{site.support}</a>. Most disputes
          end there, and we would rather fix the problem than argue about it.
        </p>
      </>
    ),
  },
  {
    id: "the-rest",
    title: "The rest",
    body: (
      <ul>
        <li>
          These terms and the Privacy Policy are the whole agreement between us
          about the Service.
        </li>
        <li>
          If a provision is unenforceable, the rest stays in force and that
          provision is narrowed to what is enforceable.
        </li>
        <li>
          Not enforcing a right once does not waive it.
        </li>
        <li>
          You may not assign this agreement; we may assign it to a successor in a
          merger, acquisition or sale of assets, on notice to you.
        </li>
        <li>
          Notices to you go to the email address on your account. Notices to us
          go to <a href={`mailto:${site.support}`}>{site.support}</a>, or by
          post to {legal.operator}, {legal.address}.
        </li>
      </ul>
    ),
  },
];

export default function Terms() {
  return (
    <LegalDoc
      title="Terms of Service"
      summary={
        <>
          The short version: Meridian shows you your own money and computes what
          you ask it to. It is not a bank, a broker or an adviser, and nothing in
          it is advice. Your bank connections are read-only, your data is yours,
          and you can leave and take it with you. This summary is not the
          agreement — the sections below are.
        </>
      }
      sections={SECTIONS}
    />
  );
}
