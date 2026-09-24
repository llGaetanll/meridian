# The server, as it is deployed.
#
# Two stages, because a Rust toolchain is a gigabyte and a static-ish binary is
# a few megabytes. The runtime image carries the binary and the two libraries
# it actually opens at run time, and nothing else — no cargo, no source, no
# `git`.
#
# `cargo-chef` is what makes the dependency compile cacheable. Without it, any
# change to any source file invalidates the layer that built everything, and
# every image rebuild recompiles the whole dependency tree from scratch. With
# it, dependencies are their own layer keyed only on Cargo.toml and Cargo.lock,
# so an ordinary code change rebuilds `server-main` alone.

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# Reads the manifests and writes a recipe: the dependency graph with none of
# our own code in it. This stage is cheap and is thrown away.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# The expensive layer, and the one that survives a code change. Scoped to
# `server-main` so the phone's and the web's dependency trees — uniffi, leptos,
# wasm — are never built for a server image that cannot use them.
RUN cargo chef cook --release -p server-main --recipe-path recipe.json

COPY . .

# The commit this build came from, since `.git` is not in the context. Without
# it every line the container logs says `unknown`, and a log from a deployed
# build that cannot be matched to its source is most of the value gone. The
# default keeps a bare `docker compose build` working; a deploy passes the real
# one — see docker-compose.yml.
ARG MERIDIAN_SHA=unknown
ENV MERIDIAN_SHA=$MERIDIAN_SHA

RUN cargo build --release -p server-main

FROM debian:bookworm-slim AS runtime
WORKDIR /app

# `ca-certificates` because every call to Plaid is TLS and a container with no
# root store fails at the handshake with an error that sounds like a network
# problem. `libssl3` because reqwest links the system OpenSSL.
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Not root. The server opens a socket and talks to Postgres; nothing it does
# needs to be able to write to the image.
RUN useradd --system --create-home --shell /usr/sbin/nologin meridian

# The log file api-trace appends to. Its parent has to exist and be writable by
# the user the server runs as, or every boot opens with a warning about the log
# — which is a poor first line for a file whose whole job is being trusted.
#
# Everything is on stdout as well, and in a container that is the copy anyone
# actually reads: `docker compose logs`, or whatever collects it in a
# deployment. The file is the same lines again, and dies with the container.
RUN mkdir -p /app/logs && chown -R meridian:meridian /app

USER meridian

COPY --from=builder /app/target/release/server-main /usr/local/bin/server-main

# The symbols the server fetches on its own initiative, baked in so a
# deployment warms its market cache without being asked. The variable can be
# pointed elsewhere to override, or set empty to turn the behaviour off.
COPY data/universe.txt /etc/meridian/universe.txt
ENV MARKET_UNIVERSE=/etc/meridian/universe.txt

# Matches `PORT` in crates/server-main/src/runtime_server.rs. Documentation
# rather than a rule — compose is what actually publishes it.
EXPOSE 8082

CMD ["server-main"]
