# Stripe

Before the switch:
- [x] support address needs to receive mail. Namecheap Private Email;
      `support@` is the mailbox, `privacy@` an alias on it, both answering
- [x] #119 $5.99 includes sales tax
- [x] #99 The live-key guard is one check in one function
- [x] EIN
- [x] price, terms and refund policy on the site
- [x] test endpoint pinned. `we_1UFuJ54o4Z6OcQSwpoGMsDR8` replaces
      `we_1UE7gB4o4Z6OcQSwXeehNrSa`, which is disabled rather than deleted
- [x] test endpoint's signing secret on the box, with STRIPE_SECRET_KEY and
      STRIPE_PRICE_ID beside it

bugs:
- [x] #92 Nothing acts on the two invoice webhooks the endpoint subscribes to
- [x] #93 A card that needs 3D Secure leaves the subscription stuck
- [x] #94 Two subscribe requests that overlap make two customers at Stripe
- [x] #96 A missed webhook is permanent, because nothing asks Stripe
- [x] #97 A stale webhook overwrites newer entitlement in memory
- [x] #122 An unpaid subscription opened the app
- [x] #91 A restart locks out every paying customer
- [x] #95 Somebody who subscribes twice ends up with two subscriptions
- [x] #107 A second try at paying can lock out someone who paid

After the switch:
- [x] activate the account: business details, EIN, what we sell. The IRS holds
      the name as `MERIDIAN STUDIOS LLC GAETAN ALMELA SOLE MBR % GAETAN ALMELA
      SOLE MBR`, which is the legal name field; `Meridian Studios LLC` is the
      public one and `MERIDIAN STUDIOS` the statement descriptor
- [x] #118 Stripe has no bank account to pay us into — part of activation, not
      before it
- [x] live webhook endpoint, pinned to `2026-08-26.dahlia`
- [ ] reveal that endpoint's secret and put it on the box — in the same edit as
      the live keys, never before. `STRIPE_WEBHOOK_SECRET` holds one value, so
      the live secret on a box running test keys fails every test webhook
- [x] live price. `price_1UG1xN4huf4x3hvR38ha6sua`, $5.99/mo, tax inclusive,
      on product `prod_VGZ5SaLRe2M80c`. A price exists in one mode only, so the
      test one does not resolve under a live key
- [ ] switch live keys on the box. All four of STRIPE_SECRET_KEY,
      STRIPE_PRICE_ID, STRIPE_PUBLISHABLE_KEY and STRIPE_WEBHOOK_SECRET change
      in one edit — there is one slot per role, not one per mode, and a half
      switch is a server that talks to live Stripe about a test price

## After the first paying customer
Nothing is owed until somebody is charged, and registering early only buys a
quarterly filing chore.

- [ ] #120 We are not registered to collect New York sales tax
- [ ] #121 Stripe Tax costs either 0.5% or 50¢ a charge, and we do not know which

## After the switch
- [ ] #115 Somebody whose card stopped working has nothing to press
- [x] #76 Only one email address can sign up — very last, it is what rations
      Plaid's ten Item slots

# Plaid application
- [x] Create an LLC, otherwise *you* are liable
- [x] Connect citi account on trial
    - [ ] #53 An OAuth bank has nowhere to hand control back to — server side
          built but switched off
    - [x] #105 The iOS app does not build: BankLink calls a method Apple does
          not have
- [x] #60 client_name is a literal, and it is what the user reads on Plaid's
      consent screen
- [x] Hosted homepage, and the support address on it is live
- [ ] Security Questionaire
    - [ ] #33 The database volume is not encrypted at rest
    - [ ] #38 pg_dump on a timer, off the box
    - [ ] #16 Handling user financial data: what we're on the hook for
    - [ ] #64 There is no way to delete an account
    - [ ] #102 Nothing calls /account/delete, so there is no way to leave
issue knockouts:
- [x] #60 client_name is a literal, and it is what the user reads on Plaid's
      consent screen
- [x] #58 A bank that needs a new sign-in cannot be reconnected
- [x] #51 The freshness window is 30 seconds, which is a Plaid bill on
      Production
- [x] #49 A first sync that answers PRODUCT_NOT_READY is never asked again
- [x] #48 A link Plaid would not open writes a row for a session that never
      existed
