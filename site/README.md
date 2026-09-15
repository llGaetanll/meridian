# The site

`meridiancash.io` — the page somebody lands on before they have an account, and
the two legal documents. Not the web app, which is `crates/web-main` and will
live on its own subdomain.

Next.js and Tailwind, exported as static files. There is no server, no database
and no form: three pages and a build.

```sh
npm install
npm run dev      # http://localhost:3000
npm run build    # writes out/
```

`out/` is what gets deployed. Any web server can hand it out.

## The design language is not copied here

`app/globals.css` imports `sketches/shared/theme.css` and
`sketches/shared/components.css` directly, and the body carries the `ui` class
those files are scoped to. So the button on this site *is* the app's button, and
changing the design language changes both. Nothing in `css/` picks a colour, a
radius or a type weight of its own — it lays out the page and stops there.

Tailwind is available for spacing and layout on this site only. A
`bg-slate-900` next to a `var(--bg-panel)` is how a second design language
starts, so colour never comes from it.

## Before this goes live

`lib/legal.ts` holds the facts the code cannot know, and they are all answered:
Meridian Studios LLC, based in New York, under New York law, dated 11 September
2026. `updated` moves whenever either document changes in a way a reader would
want to know about.

The pages name the LLC and no person. The only street address is the one on
the LLC's New York filing, which is its registered agent's in Albany.

The documents also assume things that are true today and have to stay true:
connections are read-only, nothing is sold, there is no analytics on this site,
and the assistant never sends the raw ledger to a model. If one of those
changes, the policy changes in the same pass.

**Neither document has been reviewed by a lawyer.** They are written to be
accurate about what the software actually does; that is a different thing from
being sufficient.
