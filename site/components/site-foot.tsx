import Link from "next/link";

import { Mark } from "@/components/mark";
import { legal } from "@/lib/legal";
import { site } from "@/lib/site";

/// The footer, and the only place the disclaimer is stated on every page.
/// It belongs here rather than in the terms alone: somebody who reads one page
/// of this site should not be able to miss it.
export function SiteFoot() {
  return (
    <footer className="site-foot">
      <div className="wrap">
        <div className="foot-cols">
          <div>
            <Link href="/" className="brand" aria-label={`${site.name} home`}>
              <Mark className="brand-mark" />
              <span>{site.name}</span>
            </Link>
            <p className="foot-blurb">{site.tagline}</p>
          </div>

          <div>
            <h3>Product</h3>
            <div className="foot-links">
              <Link href="/#coverage">Coverage</Link>
              <Link href="/#tools">Tooling</Link>
              <Link href="/#assistant">Assistant</Link>
              <Link href="/#data">Your data</Link>
              <Link href="/#price">Price</Link>
            </div>
          </div>

          <div>
            <h3>Legal</h3>
            <div className="foot-links">
              <Link href="/terms">Terms of Service</Link>
              <Link href="/privacy">Privacy Policy</Link>
            </div>
          </div>

          <div>
            <h3>Contact</h3>
            <div className="foot-links">
              <a href={`mailto:${site.support}`}>{site.support}</a>
              <a href={site.repo}>Source</a>
            </div>
          </div>
        </div>

        <div className="foot-note">
          <span>
            &copy; {new Date().getFullYear()} {legal.operator}
          </span>
          <span>
            Meridian shows you your money. It is not a bank, a broker or an
            adviser, and nothing in it is financial advice.
          </span>
        </div>
      </div>
    </footer>
  );
}
