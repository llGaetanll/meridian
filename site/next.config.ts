import path from "node:path";

import type { NextConfig } from "next";

/// A static site: three pages, no server, no database, nothing to run. It is a
/// directory of files that any web server can hand out, which is what keeps the
/// apex independent of the box that runs the API.
const config: NextConfig = {
  // The repo has a lockfile above it, and Next picks the wrong root without
  // being told. Wrong root, wrong traced files.
  outputFileTracingRoot: path.join(import.meta.dirname, ".."),

  output: "export",
  trailingSlash: true,
  images: { unoptimized: true },
};

export default config;
