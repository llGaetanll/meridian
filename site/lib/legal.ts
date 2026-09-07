/// The handful of facts the two legal pages need and the code cannot know.
///
/// Every value in square brackets is a blank, not a placeholder that happens to
/// read oddly: shipping one is shipping a policy that names the wrong company
/// or the wrong court, which is worse than a page that admits it is unfinished.
/// `site/README.md` lists them.
export const legal = {
  entity: "[the legal entity that operates Meridian]",
  jurisdiction: "[state], [country]",
  courts: "the courts of [state], [country]",
  address: "[postal address]",
  effective: "[effective date]",
  updated: "[last updated]",

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
      name: "DigitalOcean",
      does: "hosts the server and the database the service runs on",
      link: "https://www.digitalocean.com/legal/privacy-policy",
    },
  ],
} as const;
