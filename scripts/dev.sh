#!/bin/sh
# The stack in one tab, and the commands that drive it from every other one.
#
# Run this with no arguments in a tmux window of its own and leave it there:
#
#     ./scripts/dev.sh
#
# It brings the database and the server up *detached*, then follows their logs.
# Detached is the whole point. A plain `docker compose up` owns its containers
# and ties them to the terminal it was typed in, so anyone wanting to restart
# the server has to interrupt that terminal or start a shell of their own — and
# a shell of their own is how a second copy of the logs ends up somewhere
# nobody is looking. Here nothing owns the containers but the daemon, and this
# window is only a reader.
#
# So from anywhere else — another pane, an agent, a script — the stack is
# driven with the subcommands below, and the output still lands in this window:
#
#     ./scripts/dev.sh restart server    restart, same image
#     ./scripts/dev.sh build server      rebuild after a code change, and swap
#     ./scripts/dev.sh up                after a down
#     ./scripts/dev.sh down              stop and remove everything
#     ./scripts/dev.sh ps                what is actually running
#
# Every one of them is one-shot: it returns as soon as the daemon has done as
# asked. None of them prints logs, and none of them wants a terminal kept open
# for it.

set -e

cd "$(dirname "$0")/.."

# docker-compose.yml puts the server behind a profile so that a bare `up` gets
# the database alone. Setting it here rather than passing `--profile` at each
# call site means `logs` and `restart` cannot end up disagreeing about whether
# the server is part of the stack.
COMPOSE_PROFILES=server
export COMPOSE_PROFILES

# The commit every log line the server writes is stamped with. The image build
# cannot work it out for itself — `.dockerignore` keeps `.git` out of the
# context on purpose — so it has to be handed in, and here is the only place
# every local build passes through. Left to compose's default it is `unknown`,
# which is the one answer the stamp exists to avoid.
#
# `--dirty` because a local build is usually not the commit exactly: a bare sha
# on a log line from a tree with uncommitted changes points at source that is
# not what ran.
if [ -z "${MERIDIAN_SHA:-}" ]; then
    if MERIDIAN_SHA=$(git rev-parse --short HEAD 2>/dev/null); then
        git diff --quiet HEAD 2>/dev/null || MERIDIAN_SHA="$MERIDIAN_SHA-dirty"
    else
        MERIDIAN_SHA=unknown
    fi
fi
export MERIDIAN_SHA

if ! docker info >/dev/null 2>&1; then
    echo "docker is not running — start Docker Desktop and try again." >&2
    exit 1
fi

# The database serves TLS and will not start without a certificate, so a fresh
# clone would fail here with a Postgres error rather than a useful one. Making
# them is instant and idempotent, so it costs a stat on every other run.
if [ ! -f certs/server.key ]; then
    ./scripts/db-certs.sh
fi

cmd=logs
if [ $# -gt 0 ]; then
    cmd=$1
    shift
fi

case "$cmd" in
    up)      exec docker compose up -d "$@" ;;
    down)    exec docker compose down "$@" ;;
    stop)    exec docker compose stop "$@" ;;
    restart) exec docker compose restart "$@" ;;
    ps)      exec docker compose ps "$@" ;;

    # `up -d --build` rather than `build`: an image nobody is running is not a
    # change anyone can see, and building without swapping the container is the
    # mistake that has you rereading the old build's logs and disbelieving them.
    build)   exec docker compose up -d --build "$@" ;;

    logs)    ;;

    *)
        echo "usage: $0 [logs|up|down|stop|restart|build|ps] [service...]" >&2
        exit 1
        ;;
esac

docker compose up -d

# Deliberately no service argument. `docker compose logs -f` given one attaches
# to that container and dies with it; given none it follows the whole project
# and keeps following — across a restart, across a rebuild that replaces the
# container, and across a `down` and `up` that replace both. That is what lets
# this window survive everything the subcommands above do to it.
#
# The cost is that each reattach reprints the tail, so `--tail` is small: it is
# the price of every restart, not just of starting up.
while :; do
    docker compose logs -f --tail=20 || true

    # Only reached if the follower gives up, which in practice means the daemon
    # went away — Docker Desktop restarting, or a machine waking up. A dead log
    # window is the thing this script exists to prevent, so it waits and tries
    # again rather than leaving a shell prompt behind.
    echo "-- lost the docker daemon; retrying"
    sleep 2
done
