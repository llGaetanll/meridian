/// Everything about the site that is a name or an address, in one place.
///
/// The app lives on its own subdomain and does not exist yet. It is written
/// here as a link anyway, so that turning it on is a DNS record and a deploy
/// rather than an edit to every page that offers it.
export const site = {
  name: "Meridian",
  domain: "meridiancash.io",
  url: "https://meridiancash.io",
  app: "https://app.meridiancash.io",
  repo: "https://github.com/llGaetanll/meridian",
  tagline: "Everything you own, everything you owe, everything you spend.",
  support: "support@meridiancash.io",
  privacy: "privacy@meridiancash.io",
} as const;
