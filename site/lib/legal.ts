/// The handful of facts the two legal pages need and the code cannot know.
///
/// Every value in square brackets is a blank, not a placeholder that happens to
/// read oddly: shipping one is shipping a policy that names the wrong company
/// or the wrong court, which is worse than a page that admits it is unfinished.
/// `site/README.md` lists them.
export const legal = {
  /// The company that runs Meridian, and the address on its New York filing.
  operator: "Meridian Studios LLC",
  address: "418 Broadway, Ste N, Albany, NY 12207",
  based: "New York, United States",
  jurisdiction: "the State of New York, United States",
  courts: "the state and federal courts located in New York",
  effective: "September 11, 2026",
  updated: "September 14, 2026",

  /// Who the ledger is built from. Named here because both documents refer to
  /// them and a list that disagrees with itself is the one thing a privacy
  /// policy cannot afford.
  processors: [
    {
      name: "Plaid Inc.",
      does: "connects your bank and brokerage accounts and returns balances, transactions and holdings",
      link: "https://plaid.com/legal/#end-user-privacy-policy",
    },
    {
      name: "Resend",
      does: "delivers the sign-in codes and account email we send you",
      link: "https://resend.com/legal/privacy-policy",
    },
    {
      name: "Stripe",
      does: "takes payment for a subscription on the web; card details go from your browser to Stripe and never reach our servers",
      link: "https://stripe.com/privacy",
    },
    {
      name: "DigitalOcean",
      does: "hosts the server and the database the service runs on",
      link: "https://www.digitalocean.com/legal/privacy-policy",
    },
  ],
} as const;
