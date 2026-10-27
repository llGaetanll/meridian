# Ports

This machine runs more than one project, and more than one of them has a
Postgres and a web app. Left alone they all reach for 5432, 8080 and 3000, and
whichever starts second does not start at all — a container that exits with a
bind error reads as a broken database and is not one. So every host port is
allocated, and the number says what it belongs to.

This file covers Meridian's block. The machine-wide allocation, which names the
other projects, is kept outside this repository: each project is independent of
the rest and none of them publishes the others' layout.

## The scheme

A published port is four digits, `7PSE`:

| Digit | Means | Values |
| --- | --- | --- |
| `7` | one of ours | above 1024, far below the ephemeral range, and nothing common lives at 71xx or 72xx |
| `P` | the project | `1` meridian; the machine's other projects hold the rest |
| `S` | the service's role | `1` database, `2` backend, `3` web app, `4` site, `5`–`9` spare |
| `E` | the environment | `0` a laptop, `1` the box |

The role digit means the same thing in every project, which is the point:
`7120` and `7220` are each that project's backend, so a port can be read rather
than remembered.

## What is allocated

| Port | Project | Service | Where |
| --- | --- | --- | --- |
| 7110 | meridian | `db` | laptop |
| 7111 | meridian | `db` | box |
| 7120 | meridian | `server` | laptop |
| 7121 | meridian | `server` | box |
| 7130 | meridian | `web` | laptop |
| 7140 | meridian | `site` | — reserved, published nowhere |
| 7150 | meridian | `books` | — reserved, it listens on nothing |

The `72xx` block and above are taken by the machine's other projects. Read the
machine-wide list before claiming a number outside `71xx`.

80, 443 and 443/udp are Caddy's, on the box. They are the public web's numbers
and are not ours to allocate.

## Inside a container, nothing changed

Only the host side of a publish follows this. Inside its own network a service
keeps the number its ecosystem expects, because there is nothing to collide
with there and a surprising number is its own cost:

| Service | Listens on |
| --- | --- |
| `db` | 5432 |
| `server` | 7120 |
| `web`, `site` | 3000 |

The `server` is the exception, and deliberately. It is not always a
container — `cargo run` on the host is the usual way to write server code — and
`stripe listen`, `scripts/ios-run.sh` and the web app all name one address that
has to be right either way. One number, inside and out, is what makes that
true.

## On the box

Caddy reaches every service by name over the compose network, so almost
nothing needs publishing there at all. What is published is published on
loopback and stated in `docker-compose.prod.yml` with `!override`, which
replaces the dev file's list rather than appending to it — a plain merge
appends, which is how a laptop's `0.0.0.0` used to reach the box (#145).
Stating the prod number next to the prod service means no variable has to be
remembered in a `.env` for the box to be safe.

## Adding a service

Take the next free role digit in your project's block, use `0` for the laptop,
and add the row above. If it needs reaching from a shell on the box, give it
the `1` as well and an `!override` in the prod file. If it does not, publish
nothing: Caddy has its name.
