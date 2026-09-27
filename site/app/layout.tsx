import type { Metadata } from "next";
import { Inter } from "next/font/google";

import { Analytics } from "@/components/analytics";
import { SiteFoot } from "@/components/site-foot";
import { SiteHead } from "@/components/site-head";
import { site } from "@/lib/site";

import "./globals.css";

/// Self-hosted rather than fetched from Google at page load: a font request to
/// a third party on a page about somebody's bank is a request that did not need
/// to happen.
const inter = Inter({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-inter",
});

export const metadata: Metadata = {
  metadataBase: new URL(site.url),
  title: {
    default: `${site.name} — ${site.tagline}`,
    template: `%s — ${site.name}`,
  },
  description:
    "Meridian puts everything you own, everything you owe and everything you spend in one ledger, with the tools to explore what any of it means.",
  openGraph: {
    type: "website",
    url: site.url,
    siteName: site.name,
    title: `${site.name} — ${site.tagline}`,
    description:
      "One ledger for your accounts, your holdings and your spending. It shows you your money; it does not tell you what to do with it.",
  },
  robots: { index: true, follow: true },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={inter.variable}>
      {/* `ui` is the scope the design language is written under. Everything the
          site draws is one of its components, so the scope goes on the body and
          is never opened a second time. */}
      <body className="ui">
        <SiteHead />
        {children}
        <SiteFoot />
        <Analytics />
      </body>
    </html>
  );
}
