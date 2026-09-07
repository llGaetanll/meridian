import type { ReactNode } from "react";

import { legal } from "@/lib/legal";

export type LegalSection = {
  /// The anchor. Stable, because a section people link to that moves when a
  /// paragraph is inserted above it is a link that quietly starts lying.
  id: string;
  title: string;
  body: ReactNode;
};

/// The shape both legal pages take: a heading with the two dates, a plain-words
/// summary, a table of contents, then the sections numbered in order.
///
/// The summary is not the agreement and says so. It is here because a document
/// nobody reads protects nobody, and the paragraph most people will read is the
/// first one.
export function LegalDoc({
  title,
  summary,
  sections,
}: {
  title: string;
  summary: ReactNode;
  sections: LegalSection[];
}) {
  return (
    <main className="legal">
      <div className="wrap narrow">
        <div className="legal-head">
          <div className="eyebrow">Legal</div>
          <h1>{title}</h1>
          <div className="dates">
            <span>Effective {legal.effective}</span>
            <span>Last updated {legal.updated}</span>
          </div>
          <div className="summary">{summary}</div>
        </div>

        <nav className="toc" aria-label="Contents">
          {sections.map((section, i) => (
            <a key={section.id} href={`#${section.id}`}>
              <span className="n">{i + 1}</span>
              {section.title}
            </a>
          ))}
        </nav>

        {sections.map((section, i) => (
          <section key={section.id} id={section.id}>
            <h2>
              <span className="n">{i + 1}</span>
              {section.title}
            </h2>
            {section.body}
          </section>
        ))}
      </div>
    </main>
  );
}
