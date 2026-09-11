# Compliance

Who may publish Meridian, on which platform, and what the law asks of it
wherever it runs. Checked 2026-09-11.

**The iOS and Android apps cannot be released from a personal account. The
website can.** That split is the stores' policy, not the law. The law is the
same on every platform, and is at the bottom.

Every claim marked **(?)** is unconfirmed. The list is at the bottom.

## App Store

App Review Guideline 5.1.1(ix), verbatim:

> Apps that provide services in highly regulated fields (such as banking and
> financial services, healthcare, gambling, legal cannabis use, air travel and
> crypto exchanges) or that require sensitive user information should be
> submitted by a legal entity that provides the services, and not by an
> individual developer.

Meridian shows bank balances and transactions, which is both "financial
services" and "sensitive user information". **An Individual submission is
likely to be rejected.** Enforcement is uneven and some apps like it are live,
but a pass is not safe either: a read-only finance viewer from an individual
was rejected on a routine bugfix update, and three appeals failed. An app with
paying users can be stranded that way.

The membership is team `U5PF7N9778`, type Individual. That is enough to build
and run on a device (#68) and not enough to release. To release:

- A legal entity. Apple does not accept DBAs, trade names or sole
  proprietorships under a business name
- A D-U-N-S number for it (free)
- An email address at `meridiancash.io`, and a real website there. A registrar
  placeholder or a thin page is refused
- Apple Developer Support converts the Individual membership to an
  Organization. Whether `U5PF7N9778` survives that is not known (?)

The entity's name becomes the seller shown on the App Store. In EU
storefronts Apple also shows the seller's address, phone and email publicly.

**Guideline 3.2.1(viii)** is the other one to know: apps "used for financial
trading, investing, or money management should be submitted by the financial
institution performing such services". **Meridian never moves money**; it only
reads. The usual reading is that this does not reach a budgeting app — Monarch
and Copilot are not banks.

## Google Play

Financial products and services must be published from an Organization
account, which needs a D-U-N-S number too. Google's list names banking, loans,
stock trading, investment funds and crypto. Whether it counts a budgeting app
is unconfirmed (?); plan as if it does. The entity Apple needs covers this
too.

## The web

No store and no reviewer, so no rule about who publishes. The website can run
under a person's own name.

- **Plaid** accepts a person's own name as the legal entity. Leaving the Trial
  plan means a compliance review that asks about "your business"
- **Stripe** accepts a sole proprietor, signed up with a Social Security number
- **Trading as "Meridian"** rather than under one's own name means an assumed
  name certificate from the county clerk, for a small fee (?)
- **No store commission.** Stripe's fee is the whole cut, which is what
  `BUSINESS.md` assumes

## Everywhere

Store policy decides who may publish; the law decides what the product owes
its users, and it does not care which platform they used.

- **A privacy policy and terms.** Written in `site/`, not yet hosted (#56).
  The operator is blank in `site/lib/legal.ts`, and a person's own name is a
  valid answer until there is an entity
- **New York's SHIELD Act** asks for reasonable safeguards over New York
  residents' private information, which can include bank account data (?)
- **The FTC Safeguards Rule** and **the CFPB's open-banking rule (1033)** may
  reach a product that aggregates bank data. 1033 is being reconsidered and
  its dates are in flux (?)
- **Liability.** Without an entity, a leak of other people's bank data is the
  owner's personal liability. That is the case for an entity that has nothing
  to do with any store

## The entity

Not decided. What each shape costs is in `BUSINESS.md`. The rules that decide
between them:

- **A New York LLC** publishes a notice in two newspapers for six weeks, within
  120 days, in the county named on its filing. Not publishing suspends its
  authority to do business in New York, including suing in its courts; it
  does not dissolve it
- **An LLC formed in another state** — Florida, Delaware, Wyoming — and run
  from New York must still register here ($250) and publish the same notice.
  Its profit is still taxed where the owner lives
- **A New York corporation** publishes nothing, but files its own tax return
  every year, where a single-member LLC is a line on a personal one

## Confirm these

- [ ] Apple: whether team `U5PF7N9778` keeps its ID when converted. If not,
      `MERIDIAN_TEAM_ID`, the provisioning scripts and the
      apple-app-site-association file all change
- [ ] Google Play: whether its Organization rule covers a budgeting app, or
      only banking, loans and trading
- [ ] New York: the assumed name certificate fee in New York County
- [ ] A lawyer or accountant: whether SHIELD, the FTC Safeguards Rule and
      1033 reach Meridian, and what each would ask of it

## Sources

- [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/)
- [Apple: enrollment requirements](https://developer.apple.com/support/enrollment/)
- [Forum: a finance viewer rejected on a bugfix update](https://developer.apple.com/forums/thread/740042)
- [Google Play: choosing a developer account type](https://support.google.com/googleplay/android-developer/answer/13634885?hl=en)
- [Plaid: Sandbox, Production and the Trial plan](https://support.plaid.com/hc/en-us/articles/16110110883479-How-are-Sandbox-Production-Trial-plan-and-Limited-Production-different)
- [New York LLC Law § 802](https://www.nysenate.gov/legislation/laws/LLC/802)
