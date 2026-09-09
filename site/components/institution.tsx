import type { ReactNode } from "react";

/// The institution marks the coverage marquee draws.
///
/// Every one is a single path on the same 24x24 grid the app's own icons use,
/// and every one is drawn in `currentColor` with the brand colour stripped —
/// the row is one grey, so it reads as a texture of marks rather than as a
/// ransom note of six different brand palettes.
///
/// Only marks that survive being small are here, and only where open artwork
/// exists. Two things are left out on purpose:
///
/// A logo that is really lettering — Wells Fargo, American Express, Discover —
/// is a smudge at 23px and says the name a second time next to the name. Those
/// institutions get the wordmark alone.
///
/// The large US retail banks — Citi, Capital One, Fidelity, Schwab, Vanguard —
/// publish no open artwork at all. A logo traced by hand is a trademark
/// reproduced badly, which is worse than simply setting the name.
const marks: Record<string, ReactNode> = {
  Chase: (
    <path d="M0 15.415c0 .468.38.85.848.85h5.937V.575L0 7.72v7.695m15.416 8.582c.467 0 .846-.38.846-.849v-5.937H.573l7.146 6.785h7.697M24 8.587a.844.844 0 0 0-.847-.846h-5.938V23.43l6.782-7.148L24 8.586M8.585.003a.847.847 0 0 0-.847.847v5.94h15.688L16.282.003H8.585Z" />
  ),
  "Bank of America": (
    <path d="M15.194 7.57c.487-.163 1.047-.307 1.534-.451-1.408-.596-3.176-1.227-4.764-1.625-.253.073-1.01.271-1.534.434.541.162 2.328.577 4.764 1.642zm-8.896 6.785c.577.343 1.19.812 1.786 1.209 3.952-3.068 7.85-5.432 12.127-6.767-.596-.307-1.119-.578-1.787-.902-2.562.65-6.947 2.4-12.126 6.46zm-.758-6.46c-2.112.974-4.331 2.31-5.54 3.085.433.199.866.361 1.461.65 2.671-1.805 4.764-2.905 5.594-3.266-.595-.217-1.154-.361-1.515-.47zm8.066.234c-.686-.379-3.068-1.263-4.71-1.642-.487.18-1.173.451-1.642.65.595.162 2.815.758 4.71 1.714.487-.235 1.173-.523 1.642-.722zm-3.374 1.552c-.56-.27-1.173-.523-1.643-.74-1.425.704-3.284 1.769-5.63 3.447.505.27 1.047.595 1.624.92 1.805-1.335 3.627-2.598 5.649-3.627zm1.732 8.825c3.79-3.249 9.113-6.407 12.036-7.544a48.018 48.018 0 00-1.949-1.155c-3.771 1.246-8.174 4.007-12.108 7.129.667.505 1.371 1.028 2.02 1.57zm2.851-.235h-.108l-.18-.27h-.109v.27h-.072v-.596h.27c.055 0 .109 0 .145.036.054.019.072.073.072.127 0 .108-.09.162-.198.162zm-.289-.343c.09 0 .199.018.199-.09 0-.072-.072-.09-.144-.09h-.163v.18zm-.523.036c0-.289.235-.523.541-.523.307 0 .542.234.542.523a.543.543 0 01-.542.542.532.532 0 01-.54-.542m.107 0c0 .235.199.433.451.433a.424.424 0 100-.848c-.27 0-.45.199-.45.415" />
  ),
  Robinhood: (
    <path d="M2.84 24h.53c.096 0 .192-.048.224-.128C7.591 13.696 11.94 8.656 14.67 5.638c.112-.128.064-.225-.096-.225h-4.88a.55.55 0 0 0-.45.225L5.746 9.972c-.514.642-.642 1.236-.642 2.086v4.43c-1.14 3.194-1.862 5.361-2.392 7.32-.032.125.016.192.129.192M20.447.646c-.754-.802-4.157-.834-5.73-.224a3 3 0 0 0-.786.465 41 41 0 0 0-3.323 3.178c-.112.113-.064.225.097.225h5.409c.497 0 .786.289.786.786v6.1c0 .16.128.208.225.064l3.258-4.254c.53-.69.69-.898.835-1.861.192-1.413.08-3.58-.77-4.479m-6.982 16.18 2.231-3.676a.7.7 0 0 0 .064-.29V6.73c0-.16-.112-.225-.224-.097-3.355 3.74-5.971 7.672-8.395 12.407-.06.12.016.225.16.177l5.009-1.54c.565-.174.882-.402 1.155-.852" />
  ),
};

/// One institution in the marquee: its mark where we have one, its name where
/// we do not. Both are the same grey and sit on the same baseline, so a row
/// mixing the two reads as one row rather than as logos with gaps in it.
export function Institution({ name }: { name: string }) {
  const mark = marks[name];

  if (!mark) {
    return <span className="inst inst-word">{name}</span>;
  }

  return (
    <span className="inst" title={name}>
      <svg viewBox="0 0 24 24" fill="currentColor" role="img" aria-label={name}>
        {mark}
      </svg>
      <span className="inst-name">{name}</span>
    </span>
  );
}
