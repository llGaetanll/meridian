#!/bin/sh
# The box, driven from here.
#
# Everything runs over ssh, so nothing about a deploy needs a shell open on the
# server, and the same one-shot shape as scripts/dev.sh applies: every
# subcommand but `watch` returns as soon as the daemon has done as asked.
#
#     ./scripts/deploy.sh ship          build here, load it there, swap
#     ./scripts/deploy.sh rollback SHA  run an image the box already has
#     ./scripts/deploy.sh up            after a down
#     ./scripts/deploy.sh down          stop and remove everything
#     ./scripts/deploy.sh restart       restart, same image
#     ./scripts/deploy.sh ps            what is actually running
#     ./scripts/deploy.sh logs          the last few minutes, and return
#     ./scripts/deploy.sh watch         follow them, in a window of its own
#     ./scripts/deploy.sh health        ask the server, through the proxy
#     ./scripts/deploy.sh shell         an interactive shell, in the checkout
#
# `logs` and `watch` are the two halves of the same question and are separate on
# purpose. `logs` is asked something and answers it: it returns, so an agent or
# a script can call it, which is the rule CLAUDE.md sets for the dev stack too.
# `watch` never returns, because it is a window you leave open — the box's
# equivalent of `scripts/dev.sh` with no arguments.
#
# The image is built *here* and shipped over the same ssh connection as
# everything else. The box runs Postgres, Caddy and a ten-megabyte binary, and
# nothing else — no Rust toolchain, no source, no git. Compiling 427 crates is
# a fixed cost that does not belong in the monthly bill for a machine whose
# real job is to hold a socket open, and sizing a server for a compiler is
# paying for the build every month instead of once.
#
# There is no registry in the path either, for the reason there is no CI: a
# deploy that depends on nothing but ssh is a deploy that still works on the
# day GitHub is down. `docker save | ssh docker load` is that, and the image is
# small enough that the wire is not the slow part.

set -e

cd "$(dirname "$0")/.."

# Where to ssh, as `user@host`. Not committed — it names a machine, and the
# repo is public. `.env` is where every other piece of local configuration
# already lives and is already gitignored, so it lives there too rather than in
# a dotfile of its own that only this script knows about.
if [ -z "${MERIDIAN_HOST:-}" ] && [ -f .env ]; then
    MERIDIAN_HOST=$(sed -n 's/^MERIDIAN_HOST=//p' .env | tail -1)
fi

if [ -z "${MERIDIAN_HOST:-}" ]; then
    echo "MERIDIAN_HOST is not set — put it in .env as user@host, or export it." >&2
    echo "  MERIDIAN_HOST=meridian@api.meridiancash.io" >&2
    exit 1
fi

# Where the box keeps how it is run. `/srv` because this is a service the
# machine runs rather than a person's work: a home directory ties the deploy to
# whichever account happened to set it up. It holds the two compose files, the
# Caddyfile, the box's own `.env`, and nothing else — this is configuration,
# not a checkout.
MERIDIAN_PATH="${MERIDIAN_PATH:-/srv/meridian}"

# What the box is running, written by `ship` and read by everything else.
#
# It used to be `git rev-parse` run on the box, which worked only because there
# was a checkout there to ask. Now the commit is decided here, at build time,
# and the box is told — so this file is the box's answer to "which image is
# this", and `docker-compose.prod.yml` turns it into the tag it runs.
COMPOSE='COMPOSE_PROFILES=server MERIDIAN_SHA=$(cat .sha) docker compose -f docker-compose.yml -f docker-compose.prod.yml'

remote() {
    # shellcheck disable=SC2029  # expanded here on purpose: the command is ours
    ssh "$MERIDIAN_HOST" "cd '$MERIDIAN_PATH' && $*"
}

cmd=${1:-}
[ $# -gt 0 ] && shift

case "$cmd" in
    ship)
        # A dirty tree is refused because the stamp would lie. Building on the
        # box made this impossible — it could only ever build what had been
        # pushed — and building here gives that guarantee away, so it has to be
        # bought back explicitly. An image labelled with a commit that does not
        # contain it is worse than no label: it sends whoever is reading the
        # log to the wrong source.
        if ! git diff --quiet HEAD; then
            echo "the tree is dirty — commit or stash before shipping." >&2
            exit 1
        fi

        sha=$(git rev-parse --short HEAD)

        # `docker build` rather than `docker compose build`, so the tag is the
        # one named here rather than one compose derives from the directory
        # name. The box and this line have to agree on it exactly or `up` finds
        # no image.
        docker build --build-arg "MERIDIAN_SHA=$sha" -t "meridian-server:$sha" .

        # How the box is run, which is not in the image and changes far more
        # often than the code. Its `.env` is deliberately not in this list: it
        # holds that box's secrets, it is not ours to overwrite, and a deploy
        # that resets it is a deploy that logs somebody out.
        ssh "$MERIDIAN_HOST" "mkdir -p '$MERIDIAN_PATH'"
        scp docker-compose.yml docker-compose.prod.yml Caddyfile \
            "$MERIDIAN_HOST:$MERIDIAN_PATH/"

        # Gzipped because the layer that carries the binary does not compress
        # on the wire otherwise, and this runs over whatever connection the
        # laptop has rather than a datacentre one.
        docker save "meridian-server:$sha" | gzip | \
            ssh "$MERIDIAN_HOST" "gunzip | docker load"

        remote "echo '$sha' > .sha"
        remote "$COMPOSE up -d"
        ;;

    # Every image the box has ever been shipped is still on it under its own
    # tag, so going back is naming one. There is no source here to rebuild
    # from, which is exactly why the old images are worth their disk.
    #
    #     ./scripts/deploy.sh rollback 6f31686
    rollback)
        sha=${1:-}

        if [ -z "$sha" ]; then
            echo "usage: $0 rollback SHA" >&2
            remote "docker images meridian-server --format '  {{.Tag}}  {{.CreatedSince}}'" >&2 || true
            exit 1
        fi

        remote "docker image inspect 'meridian-server:$sha' >/dev/null"
        remote "echo '$sha' > .sha"
        remote "$COMPOSE up -d"
        ;;

    up)      remote "$COMPOSE up -d $*" ;;
    down)    remote "$COMPOSE down $*" ;;
    restart) remote "$COMPOSE restart $*" ;;
    ps)      remote "$COMPOSE ps $*" ;;

    logs)    remote "$COMPOSE logs --since ${MERIDIAN_SINCE:-5m} $*" ;;

    # The box's logs in a tmux window, the way `scripts/dev.sh` with no
    # arguments is the laptop's.
    #
    # The loop is not decoration. This runs over whatever connection the laptop
    # has, and ssh dies with the wifi, with a lid closing, with a sleep — none
    # of which is anything having gone wrong on the box. dev.sh reattaches for
    # the same reason; there it is Docker Desktop restarting, here it is a
    # train going into a tunnel. `ServerAliveInterval` is what makes a dropped
    # connection noticed in seconds rather than whenever TCP gives up.
    #
    # No service argument, for dev.sh's reason: `logs -f` given one attaches to
    # that container and dies with it, and this window has to survive the next
    # `ship` replacing the server underneath it.
    #
    # It does not bring anything up first, which is where it parts company with
    # dev.sh. On a laptop starting the stack is free; here it is a deploy, and
    # a window opened to read logs should not be the thing that decides the box
    # is running.
    watch)
        while :; do
            ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3 \
                "$MERIDIAN_HOST" \
                "cd '$MERIDIAN_PATH' && $COMPOSE logs -f --tail=20" || true

            echo "-- lost the connection to $MERIDIAN_HOST; retrying"
            sleep 2
        done
        ;;

    # Through the proxy rather than at the server, so it answers the question
    # anyone actually has — can the phone reach this — and exercises DNS, TLS
    # and Caddy on the way to the same `/health` the dev stack has.
    health)  curl -fsS "https://${MERIDIAN_DOMAIN:-api.meridiancash.io}/health" && echo ;;

    shell)   exec ssh -t "$MERIDIAN_HOST" "cd '$MERIDIAN_PATH' && exec \$SHELL -l" ;;

    *)
        echo "usage: $0 [ship|rollback|up|down|restart|ps|logs|watch|health|shell] [args...]" >&2
        exit 1
        ;;
esac
