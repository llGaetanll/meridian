# The device renders

Product shots of the app on a phone and a laptop, rendered in Blender. The
stills come out as PNGs with a transparent background; the one animation comes
out as a WebM with an alpha channel. Nothing here runs in a browser.

```sh
./fetch-assets.sh      # once, after cloning — pulls the models and the HDRI
./render.sh            # full quality
./render.sh --draft    # a quarter of the samples, for checking framing
```

Needs `blender`, `node`, `avifenc` and Chrome on the machine. Output lands in
`out/`.

The models and the environment map are third-party downloads of a few megabytes
each, so they are fetched rather than committed. `fetch-assets.sh` knows where
they come from and skips anything already there.

## How it works

Two steps.

1. **`shoot.mjs`** drives headless Chrome over the DevTools protocol and takes a
   picture of each screen out of `sketches/`. The screen is torn out of its
   sketch and stood alone on the page first — a clip out of the page as it
   stands comes back with the app's own scroller empty.

2. **The Blender scenes** put that picture on a device and render it.
   `scene_phone.py` and `scene_laptop.py` are the two stills; `lib.py` holds the
   importing, the screen material, the lighting and the Cycles setup.

The screens come from `sketches/`, not from the running app. A screenshot of the
app is right on the day it is taken; the sketches are the design language
itself, so a render made from them stays right for as long as that does.

## The models

The devices are **real models**, not geometry built here. A phone modelled by
hand has a screen that does not sit in its bezel and corners that pop out of the
body, which is exactly what it looks like.

| File | What | Source | Licence |
| --- | --- | --- | --- |
| `models/iphone.glb` | iPhone 17 Pro | [getglb.com](https://www.getglb.com/robot/iphone-17-pro/) | free download |
| `models/macbook.glb` | MacBook Pro 16" | [github.com/arnobt78/MacBook-Showcase-Landing-Page](https://github.com/arnobt78/MacBook-Showcase-Landing-Page--React-Frontend) | see repo |
| `hdri/studio_small_08_2k.hdr` | Studio environment | [Poly Haven](https://polyhaven.com/a/studio_small_08) | CC0 |

**Check the licences before this ships anywhere public.** They are fine for
working with; a marketing page is a different question, and the MacBook in
particular came out of a third-party repository rather than from its author.

The UI is a flat image projected onto the model's own screen geometry. That
geometry already has the display's corner radius and sits at the depth the glass
puts it, which is what keeps the picture inside the device.

## Working on a scene

Every scene takes `--samples` and `--res`, so a framing pass is a second and a
final render is a few. Both take `--rotate`, `--flip-u` and `--flip-v`: a
model's own screen geometry is mapped however its author left it, and these are
where the correction for that is written down.

**Look at the render before using it.** The failure modes here are visual and
silent — an image mapped upside down, a body that reads as plastic, a screen
washed grey by a reflection — and none of them make Blender complain.

## The laptop opening

`scene_laptop_anim.py` is the laptop's still as two seconds of video: the lid
rises from closed and the screen comes on behind it.

```sh
blender --background --python scene_laptop_anim.py -- \
  screens/desktop-markets.png out/laptop-open.webm --samples 256
```

The last frame is `scene_laptop.py`'s still — same lid angle, same camera, same
lighting — so a page can play the video and leave the PNG standing at the end of
it. `--still N` renders one frame as a PNG instead, which is how to check the
motion without waiting for an encode.

The model has no hinge: it is a MacBook frozen in the open position, and three
of its meshes carry base and lid geometry in one mesh each. `lib.split_lid` cuts
the lid off along a plane at the hinge line and hangs it on an empty, which is
the thing that then rotates. The plane works because this model leaves a gap
there — the base stops just under it and the lid starts just over it — so a
different laptop would need the plane found again.
