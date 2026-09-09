#!/usr/bin/env bash
#
# The product shots, from the sketches to the finished PNGs.
#
# Two steps. `shoot.mjs` takes a picture of each screen out of `sketches/`, and
# the Blender scenes put those pictures on a real device model and render it.
#
#     ./render.sh            full quality
#     ./render.sh --draft    a quarter of the samples, for checking framing
#
set -euo pipefail

cd "$(dirname "$0")"

SAMPLES=512
RES=1

if [ "${1:-}" = "--draft" ]; then
  SAMPLES=128
  RES=0.5
fi

SCREENS=screens
OUT=out

mkdir -p "$SCREENS" "$OUT"

echo "==> screens"
node shoot.mjs "$SCREENS"

echo "==> devices"

# The screen images arrive turned and mirrored relative to the model's own
# display geometry, which is a fact about each model rather than a bug: the
# flags are where that is written down.
blender --background --python scene_phone.py -- \
  "$SCREENS/phone-recurring.png" "$OUT/phone-recurring.png" \
  --samples "$SAMPLES" --res "$RES" --rotate 180 --flip-u \
  2>&1 | grep -E "Rendering|Saved" || true

blender --background --python scene_phone.py -- \
  "$SCREENS/phone-balances.png" "$OUT/phone-balances.png" \
  --samples "$SAMPLES" --res "$RES" --rotate 180 --flip-u --turn 6 --tilt 5 \
  2>&1 | grep -E "Rendering|Saved" || true

blender --background --python scene_laptop.py -- \
  "$SCREENS/desktop-markets.png" "$OUT/laptop-markets.png" \
  --samples "$SAMPLES" --res "$RES" \
  2>&1 | grep -E "Rendering|Saved" || true

echo "==> done"
ls -la "$OUT"
