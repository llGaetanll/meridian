# The web app, as it is deployed: `crates/web-main` compiled to wasm by trunk,
# and what trunk writes handed out by Caddy.
#
# Built from the root of the repository, because the app is one member of the
# workspace and its stylesheet imports the design language from
# `sketches/shared`:
#
#     docker build -f deploy/web/Dockerfile -t meridian-web:$(git rev-parse --short HEAD) .
#
# The context is filtered by `Dockerfile.dockerignore` beside this file rather
# than by the root `.dockerignore`, which is the server's and leaves `sketches/`
# out.
#
# Nothing here copies the whole context. This directory is in it, and a
# Caddyfile reworded would otherwise recompile the app to the same wasm.

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

RUN apt-get update \
    && apt-get install --no-install-recommends --yes brotli \
    && rm -rf /var/lib/apt/lists/*

# The tools trunk runs, each pinned and in a layer of its own. trunk takes a
# tool from PATH when it is the version it wants, and `--offline` below makes
# it fail rather than fetch one, so GitHub is asked only when a version here
# changes.
#
# wasm-bindgen has to be the version in Cargo.lock, and tailwind the one in
# `crates/web-main/Trunk.toml`. wasm-opt is not pinned there, so trunk takes
# whichever it finds.
ARG TRUNK=0.21.14
RUN curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v$TRUNK/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin trunk

ARG WASM_BINDGEN=0.2.100
RUN curl -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$WASM_BINDGEN/wasm-bindgen-$WASM_BINDGEN-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz -C /usr/local/bin --strip-components=1 "wasm-bindgen-$WASM_BINDGEN-x86_64-unknown-linux-musl/wasm-bindgen"

ARG BINARYEN=version_123
RUN curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN/binaryen-$BINARYEN-x86_64-linux.tar.gz" \
    | tar -xz -C /opt
ENV PATH="/opt/binaryen-$BINARYEN/bin:$PATH"

ARG TAILWIND=4.3.3
RUN curl -fsSL -o /usr/local/bin/tailwindcss "https://github.com/tailwindlabs/tailwindcss/releases/download/v$TAILWIND/tailwindcss-linux-x64" \
    && chmod +x /usr/local/bin/tailwindcss

COPY rust-toolchain.toml .
RUN rustup toolchain install && rustup target add wasm32-unknown-unknown

# One rustc at a time. Docker Desktop's machine on the laptop has 1.5 GB, and
# two release compiles side by side in the layer below are killed for memory
# with nothing in the log but `SIGKILL`. A bigger builder can pass more.
ARG CARGO_BUILD_JOBS=1

# The expensive layer, and the one that survives a code change: the app's
# dependencies, compiled for the browser.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target wasm32-unknown-unknown -p web-main --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY sketches/shared sketches/shared

# The server the app calls, read by `option_env!` in `src/io/meridian.rs` when
# it compiles. There is no reading it at run time: the build is the setting.
ARG MERIDIAN_SERVER=https://api.meridiancash.io

WORKDIR /app/crates/web-main
RUN trunk build --release --offline

# Compressed once here rather than on every request by a one-CPU box. Caddy
# hands out the `.br` beside a file to any browser that asks for brotli.
RUN find dist -type f \( -name '*.html' -o -name '*.js' -o -name '*.css' -o -name '*.wasm' \) \
    -exec brotli --best --keep {} +

FROM caddy:2-alpine

COPY deploy/web/Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/crates/web-main/dist /srv

RUN caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

EXPOSE 8080
