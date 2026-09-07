import Link from "next/link";

import { Icon } from "@/components/icon";

/// Next draws a black-on-white 404 of its own if this file is not here, which
/// on a dark site reads as the page having broken twice.
export default function NotFound() {
  return (
    <main className="close">
      <div className="wrap">
        <div className="eyebrow">404</div>
        <h2>There is nothing at this address.</h2>
        <p>The page was moved, or the link was wrong. The rest of the site is where it was.</p>
        <div className="cta">
          <Link className="btn primary lg" href="/">
            Back to the start
            <Icon kind="arrowRight" />
          </Link>
        </div>
      </div>
    </main>
  );
}
