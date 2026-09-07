import type { Metadata } from "next";
import Link from "next/link";

import { LegalDoc, type LegalSection } from "@/components/legal-doc";
import { legal } from "@/lib/legal";
import { site } from "@/lib/site";

export const metadata: Metadata = {
  title: "Privacy Policy",
  description:
    "What Meridian collects, why it holds it, who it goes to, how long it stays, and how to get rid of it.",
};

const SECTIONS: LegalSection[] = [
  {
    id: "who-we-are",
    title: "Who this covers",
    body: (
      <>
        <p>
          This policy covers the Meridian applications, the website at{" "}
          {site.domain}, and the API behind them. {legal.entity} is the
          controller of the personal data described here.
        </p>
        <p>
          Connecting a bank is the largest thing this app asks of anybody, so
          this document is written to be read rather than to be survived. If
          something in it is unclear, write to{" "}
          <a href={`mailto:${site.privacy}`}>{site.privacy}</a> and we will
          answer and fix the wording.
        </p>
      </>
    ),
  },
  {
    id: "what-we-collect",
    title: "What we collect",
    body: (
      <>
        <p>
          <strong>What you give us.</strong> Your email address, and — if you
          subscribe — the billing details our payment processor needs. We never
          receive or store your card number.
        </p>
        <p>
          <strong>What your institutions give us, through Plaid.</strong> For
          each account you connect: the institution, the account name, type and
          last four digits, balances, transactions with their amounts, dates,
          merchants and categories, and investment holdings and their cost basis.
          This is your ledger, and it is the reason the app exists.
        </p>
        <p>
          <strong>What we hold to keep the account working.</strong> A hashed
          sign-in code while one is outstanding, a hashed session token for each
          device you are signed in on, and the sealed token that reads a bank
          connection.
        </p>
        <p>
          <strong>What the server writes down.</strong> Ordinary request logs:
          the time, the path, the response, and the IP address the request came
          from. They exist to find faults and to stop abuse.
        </p>
        <p>
          <strong>What we do not collect.</strong> Your bank username and
          password never reach us — you type those on your institution&rsquo;s
          own page inside Plaid. We do not ask for your government ID, your
          social security number, your phone number or your date of birth. This
          website sets no cookies and runs no analytics.
        </p>
      </>
    ),
  },
  {
    id: "why",
    title: "Why we hold it",
    body: (
      <>
        <ul>
          <li>
            <strong>To run the app.</strong> Your ledger is what every screen and
            every calculation is made of.
          </li>
          <li>
            <strong>To sign you in.</strong> The code we email you, and the
            session that follows it.
          </li>
          <li>
            <strong>To bill you.</strong> If you are on a paid plan.
          </li>
          <li>
            <strong>To keep it working and safe.</strong> Diagnosing faults,
            rate-limiting, and investigating abuse.
          </li>
          <li>
            <strong>To answer you.</strong> Support email, and the account
            notices we have to send.
          </li>
          <li>
            <strong>To meet legal obligations.</strong> Tax and accounting
            records, and responses to lawful requests.
          </li>
        </ul>
        <p>
          Where the GDPR applies, our bases are performing our contract with you
          (running the app, billing), our legitimate interests (security, fault
          diagnosis, preventing abuse), your consent where we ask for it, and
          legal obligation. You can withdraw consent at any time.
        </p>
        <p>
          <strong>We do not sell your personal information</strong>, we do not
          share it for cross-context behavioural advertising, and we do not use
          your ledger to build products for anybody but you.
        </p>
      </>
    ),
  },
  {
    id: "assistant",
    title: "The assistant, and models",
    body: (
      <>
        <p>
          Meridian&rsquo;s assistant works under one rule:{" "}
          <strong>the model does not do the math and does not see your raw
          ledger.</strong> Your question is turned into a query, our engine
          computes the answer from your data, and the model explains the result.
        </p>
        <p>
          What leaves our servers for a model provider is therefore your question
          and a structured, minimal result — not a dump of your transactions. We
          do not permit model providers to train on it.
        </p>
      </>
    ),
  },
  {
    id: "who-gets-it",
    title: "Who else sees it",
    body: (
      <>
        <p>
          Only the providers the Service is built out of, each doing one job
          under contract with us:
        </p>
        <ul>
          {legal.processors.map((processor) => (
            <li key={processor.name}>
              <strong>{processor.name}</strong> — {processor.does}.{" "}
              <a href={processor.link}>Their privacy policy</a>.
            </li>
          ))}
          <li>
            <strong>Our payment processor</strong> — takes payment and holds the
            card details we never see, if you are on a paid plan.
          </li>
          <li>
            <strong>A model provider</strong> — receives a question and a
            structured result when you use the assistant, under the rule in
            section 4.
          </li>
        </ul>
        <p>
          Beyond those: when the law compels us, and then only what is compelled
          and, unless we are forbidden from saying so, after telling you; and if
          the business is ever sold or merged, to the acquirer, who is bound by
          this policy until it tells you otherwise.
        </p>
        <p>That is the whole list. There are no advertisers and no data brokers on it.</p>
      </>
    ),
  },
  {
    id: "how-long",
    title: "How long we keep it",
    body: (
      <>
        <ul>
          <li>
            <strong>Your ledger</strong> — for as long as your account is open.
          </li>
          <li>
            <strong>A disconnected institution</strong> — removing a bank deletes
            the accounts, transactions and holdings that came through it, and
            revokes the token that read it.
          </li>
          <li>
            <strong>A closed account</strong> — deleted from the live database
            within 30 days, and gone from encrypted backups within 90 as those
            rotate.
          </li>
          <li>
            <strong>Sign-in codes</strong> — minutes. They expire and are
            deleted.
          </li>
          <li>
            <strong>Sessions</strong> — until you sign out, which deletes the
            row.
          </li>
          <li>
            <strong>Request logs</strong> — up to 90 days.
          </li>
          <li>
            <strong>Invoices and tax records</strong> — as long as tax law
            requires, which is usually several years, and separately from your
            ledger.
          </li>
        </ul>
      </>
    ),
  },
  {
    id: "security",
    title: "How it is protected",
    body: (
      <>
        <ul>
          <li>
            Everything travels over TLS. The server is reachable only through a
            single proxy that terminates it.
          </li>
          <li>
            The token that reads a bank connection is sealed with AES-256-GCM
            before it is written down, so a stolen backup is not a set of live
            bank connections.
          </li>
          <li>
            Sign-in codes and session tokens are stored as SHA-256 hashes, never
            as themselves, so a stolen database is not a set of live logins.
          </li>
          <li>
            Data is scoped to its owner at the query, and access to production is
            limited to the people who run the Service.
          </li>
        </ul>
        <p>
          No system is perfectly secure. If a breach affects your data we will
          tell you and the relevant regulator without undue delay, and say what
          happened rather than that &ldquo;an incident occurred&rdquo;. If you
          have found a vulnerability, write to{" "}
          <a href={`mailto:${site.privacy}`}>{site.privacy}</a>.
        </p>
      </>
    ),
  },
  {
    id: "your-rights",
    title: "What you can ask for",
    body: (
      <>
        <ul>
          <li>
            <strong>A copy.</strong> Export your ledger from the app, in a
            format something else can read.
          </li>
          <li>
            <strong>A correction.</strong> Data that came from an institution is
            corrected at the institution and re-synced; anything else, tell us.
          </li>
          <li>
            <strong>Deletion.</strong> Close your account, or ask us to close it.
          </li>
          <li>
            <strong>Restriction or objection</strong> to a particular processing,
            and <strong>portability</strong> of what you gave us.
          </li>
          <li>
            <strong>To know</strong> what we hold, where it came from, and who it
            went to.
          </li>
        </ul>
        <p>
          Write to <a href={`mailto:${site.privacy}`}>{site.privacy}</a>. We
          answer within 30 days, we do not charge for it, and we will not treat
          you differently for asking. If you are in the EEA or the UK you may
          also complain to your data protection authority; if you are in
          California, the rights above are the ones the CCPA gives you, and we do
          not sell or share your information.
        </p>
      </>
    ),
  },
  {
    id: "where",
    title: "Where it is processed",
    body: (
      <p>
        The Service runs on servers in the United States, and our providers
        process data there. If you use it from outside the United States your
        data is transferred there; where the GDPR applies, we rely on the
        European Commission&rsquo;s standard contractual clauses with providers
        that need them.
      </p>
    ),
  },
  {
    id: "children",
    title: "Children",
    body: (
      <p>
        The Service is not for anybody under 18, and we do not knowingly collect
        data from a child. If you believe a child has an account, write to{" "}
        <a href={`mailto:${site.privacy}`}>{site.privacy}</a> and we will delete
        it.
      </p>
    ),
  },
  {
    id: "changes",
    title: "Changes to this policy",
    body: (
      <p>
        We may update this policy. If a change materially affects what we do with
        your data, we will email you at least 30 days before it takes effect. The
        date at the top always says when it last changed, and continuing to use
        the Service after that date is acceptance of it.
      </p>
    ),
  },
  {
    id: "contact",
    title: "Contact",
    body: (
      <>
        <p>
          Privacy: <a href={`mailto:${site.privacy}`}>{site.privacy}</a>.
          Everything else: <a href={`mailto:${site.support}`}>{site.support}</a>.
          By post: {legal.address}.
        </p>
        <p>
          The <Link href="/terms">Terms of Service</Link> cover the rest of the
          agreement.
        </p>
      </>
    ),
  },
];

export default function Privacy() {
  return (
    <LegalDoc
      title="Privacy Policy"
      summary={
        <>
          The short version: we hold your email, and the ledger your institutions
          send us through Plaid, so the app can show it to you. Your bank
          credentials never reach us and our access is read-only. We do not sell
          your data, we run no ads, and there is no analytics on this site.
          Remove a bank and its history goes with it; close your account and the
          rest follows.
        </>
      }
      sections={SECTIONS}
    />
  );
}
