#!/bin/sh
# The images a machine keeps, trimmed to the last few commits.
#
#     ./scripts/prune-images.sh       keep the last 5
#     ./scripts/prune-images.sh 10    keep the last 10
#
# Old images are worth their disk: there is no source on the box to rebuild one
# from, so `deploy.sh rollback` reaches back exactly as far as the images do.
# They are not worth unbounded disk — the droplet is 25 GB and holds Postgres
# too, and a ship lands three more images on it. Five commits is a fortnight of
# rollback at the rate this moves.
#
# `deploy.sh ship` pipes this to the box after every load, so the box trims
# itself and nobody has to remember. `deploy.sh prune` is the same thing on its
# own. Nothing here knows it is on the box: run it on a laptop and it trims the
# copies the builds leave there.
#
# The tags are read from `meridian-server`, because one commit is one ship of
# all three and that is the repository every ship has ever written to. The
# other two are removed under the same tag if they are there — a commit shipped
# before the web app and the site had images of their own has only the server's.
#
# The commit in `.sha` is kept whatever its age. A box sitting on an old image
# after a `rollback` would otherwise have the ground taken out from under it by
# the next ship of something else.

set -e

KEEP=${1:-5}

tags=$(docker images meridian-server --format '{{.Tag}}' | grep -v '^<none>$' || true)

if [ -z "$tags" ]; then
    echo "no meridian images to prune"
    exit 0
fi

# `docker images` is newest first, so the head of that list is what to keep.
running=$(cat .sha 2>/dev/null || true)
keep_list=$(printf '%s\n%s\n' "$running" "$(echo "$tags" | head -n "$KEEP")" | grep -v '^$' | sort -u)

removed=0

for tag in $tags; do
    if echo "$keep_list" | grep -qxF "$tag"; then
        continue
    fi

    for repo in meridian-server meridian-web meridian-site meridian-books; do
        docker image rm "$repo:$tag" >/dev/null 2>&1 || true
    done

    echo "  removed $tag"
    removed=$((removed + 1))
done

echo "kept $(($(echo "$tags" | wc -l) - removed)) commits, removed $removed"
