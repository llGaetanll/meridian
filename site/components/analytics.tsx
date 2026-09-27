"use client";

import { useEffect } from "react";

import posthog from "posthog-js";

/// The PostHog project key. Public by design — it can write events and read
/// nothing — and baked in at build: `deploy/site/Dockerfile` carries it, `npm
/// run dev` does not, so a development build loads no analytics at all.
const KEY = process.env.NEXT_PUBLIC_POSTHOG_KEY;

/// Loads analytics when the build carries a key.
///
/// `/relay` is this same origin: the root Caddyfile forwards it to PostHog, so
/// no request from this page reaches a third-party host. `docs/ANALYTICS.md`
/// says what is measured and under which rules.
export function Analytics() {
  useEffect(() => {
    if (!KEY) return;

    posthog.init(KEY, {
      api_host: "/relay",
      ui_host: "https://us.posthog.com",
      defaults: "2025-05-24",
    });
  }, []);

  return null;
}
