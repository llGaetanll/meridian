# The site, as it is deployed: `site/` exported by Next to static files, and
# those files handed out by Caddy.
#
# Built from the root of the repository, because the site imports the design
# language from `sketches/shared`:
#
#     docker build -f deploy/site/Dockerfile -t meridian-site:$(git rev-parse --short HEAD) .
#
# The context is filtered by `Dockerfile.dockerignore` beside this file rather
# than by the root `.dockerignore`, which is the server's.

FROM node:26-alpine AS builder

RUN apk add --no-cache brotli

# Next reports each build to Vercel unless told not to.
ENV NEXT_TELEMETRY_DISABLED=1

WORKDIR /app/site
COPY site/package.json site/package-lock.json ./
RUN npm ci

COPY sketches/shared /app/sketches/shared
COPY site ./
RUN npm run build

# Compressed once here rather than on every request by a one-CPU box. Caddy
# hands out the `.br` beside a file to any browser that asks for brotli.
RUN find out -type f \( -name '*.html' -o -name '*.js' -o -name '*.css' -o -name '*.txt' -o -name '*.svg' -o -name '*.xml' \) \
    -exec brotli --best --keep {} +

FROM caddy:2-alpine

COPY deploy/site/Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /app/site/out /srv

RUN caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

EXPOSE 8080
