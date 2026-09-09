#!/usr/bin/env bash
#
# Downloads the models and the environment map the scenes need.
#
# These are third-party assets — a few megabytes each — so they are fetched
# rather than committed. Run this once after cloning, then `./render.sh`.
#
set -euo pipefail

cd "$(dirname "$0")"
mkdir -p models hdri

get() {
  local path="$1" url="$2"
  if [ -f "$path" ]; then
    echo "have $path"
    return
  fi
  echo "get  $path"
  curl -fsSL --max-time 180 "$url" -o "$path"
}

get models/iphone.glb \
  "https://www.get3dmodels.com/download/iphone_17_pro_by_get3dmodels.glb"

get models/macbook.glb \
  "https://raw.githubusercontent.com/arnobt78/MacBook-Showcase-Landing-Page--React-Frontend/HEAD/public/models/macbook-16.glb"

get hdri/studio_small_08_2k.hdr \
  "https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/2k/studio_small_08_2k.hdr"

echo "done"
