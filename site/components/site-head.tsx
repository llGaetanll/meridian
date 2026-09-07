import Link from "next/link";

import { Mark } from "@/components/mark";
import { site } from "@/lib/site";

/// The bar. Two links and the one action, which is the same action the bottom
/// of the home page ends on.
export function SiteHead() {
  return (
    <header className="site-head">
      <div className="wrap in">
        <Link href="/" className="brand" aria-label={`${site.name} home`}>
          <Mark className="brand-mark" />
          <span>{site.name}</span>
        </Link>

        <nav className="head-nav">
          <Link href="/#what">What it does</Link>
          <Link href="/#ledger">One ledger</Link>
          <Link href="/#data">Your data</Link>
        </nav>

        <div className="head-right">
          <a className="btn primary" href={site.app}>
            Open the app
          </a>
        </div>
      </div>
    </header>
  );
}
