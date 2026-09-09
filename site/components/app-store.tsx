/// The App Store badge.
///
/// Drawn rather than served as Apple's own artwork file: the badge has to sit
/// on a near-black page in the site's own greys, and the supplied asset is a
/// fixed bitmap in Apple's black. This is the same lockup — the apple, the
/// small line, the wordmark — built out of type and one path so it takes the
/// page's colours and stays sharp at any size.
///
/// It does not link anywhere yet. The app is not on the store, and a badge that
/// opens a dead page is worse than one that plainly says it is coming.
export function AppStore({ live = false }: { live?: boolean }) {
  const inner = (
    <>
      <svg viewBox="0 0 16 20" aria-hidden="true">
        <path
          fill="currentColor"
          d="M13.2 10.6c0-2.2 1.8-3.3 1.9-3.4-1-1.5-2.6-1.7-3.2-1.7-1.4-.1-2.7.8-3.3.8-.7 0-1.7-.8-2.8-.8-1.5 0-2.8.9-3.6 2.2-1.5 2.6-.4 6.5 1.1 8.6.7 1 1.6 2.2 2.7 2.2 1.1 0 1.5-.7 2.8-.7 1.3 0 1.6.7 2.8.7 1.2 0 1.9-1 2.6-2.1.8-1.2 1.2-2.4 1.2-2.5-.1 0-2.2-.9-2.2-3.3zM10.9 3.9c.6-.7 1-1.7.9-2.7-.9 0-2 .6-2.6 1.3-.6.6-1.1 1.7-.9 2.6 1 .1 2-.5 2.6-1.2z"
        />
      </svg>
      <span>
        <small>Download on the</small>
        App Store
      </span>
    </>
  );

  if (!live) {
    return (
      <span className="store dead" aria-label="Coming soon to the App Store">
        {inner}
      </span>
    );
  }

  return (
    <a className="store" href="https://apps.apple.com/">
      {inner}
    </a>
  );
}
