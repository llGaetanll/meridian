"""
Render a laptop showing a screen.

    blender --background --python scene_laptop.py -- screen.png out.png [options]

Options:
    --model PATH   the laptop model (default models/macbook.glb)
    --samples N    render samples (default 256)
    --res F        resolution multiplier (default 1)
    --turn F       degrees the machine is turned to the left (default 0, head on)
    --env F        environment light strength (default 0.52)
    --rotate F     degrees the screen image is turned on the display
    --flip-u       mirror the screen image left to right
    --flip-v       mirror the screen image top to bottom
    --bg hex       background colour; omitted renders transparent
"""

import argparse
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib

HERE = os.path.dirname(os.path.abspath(__file__))

argv = sys.argv
argv = argv[argv.index("--") + 1:] if "--" in argv else []

parser = argparse.ArgumentParser(description="Render a laptop product shot")
parser.add_argument("screen", help="UI image to put on the display")
parser.add_argument("output", help="Where the PNG goes")
parser.add_argument("--model", default=os.path.join(HERE, "models/macbook.glb"))
parser.add_argument("--samples", type=int, default=256)
parser.add_argument("--res", type=float, default=1.0)
parser.add_argument("--turn", type=float, default=0.0)
parser.add_argument("--env", type=float, default=0.52)
parser.add_argument("--rotate", type=float, default=0.0)
parser.add_argument("--flip-u", dest="flip_u", action="store_true")
parser.add_argument("--flip-v", dest="flip_v", action="store_true")
parser.add_argument("--bg", type=str, default=None)

args = parser.parse_args(argv)

SCREEN = os.path.abspath(args.screen)
OUTPUT = os.path.abspath(args.output)

# A 16" MacBook Pro is 355.7mm wide.
LAPTOP_W = 0.3557

lib.clear_scene()

laptop = lib.import_model(os.path.abspath(args.model))
lib.unit_scale(laptop, LAPTOP_W, axis=0)

# Space Black aluminium. The model ships silver, and silver at `metallic = 0`
# is the reason an untouched import reads as light grey plastic rather than as a
# machined body.
# The palette materials are left alone: they carry the keycap legends, and
# retinting them paints over the keyboard.
BODY = ["lmWQsEjxpsebDlK", "CRQixVLpahJzhJc", "iyDJFXmHelnMTbD", "LpqXZqhaGCeSzdu"]
lib.retint(BODY, (0.026, 0.027, 0.030), metallic=1.0, roughness=0.38)

# The display panel. Named `Object_123` in this model, which is a name the
# exporter chose rather than one anybody would guess: it is the one object whose
# removal takes the wallpaper with it.
screen = lib.find_screen(laptop, "Object_123")
lib.apply_screen(
    screen,
    SCREEN,
    brightness=2.2,
    rotate=args.rotate,
    flip_u=args.flip_u,
    flip_v=args.flip_v,
)

bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0, 0, 0))
rig = bpy.context.active_object
rig.name = "LaptopRig"
for obj in laptop:
    if obj.parent is None:
        obj.parent = rig
        obj.matrix_parent_inverse = rig.matrix_world.inverted()

rig.rotation_euler = (0, 0, math.radians(-args.turn))
bpy.context.view_layer.update()
lib.drop_to_ground([rig] + laptop)

# The room the aluminium reflects, turned well down: this shot is asked for dim,
# and the display should be the brightest thing in the frame.
lib.setup_hdri(os.path.join(HERE, "hdri/studio_small_08_2k.hdr"), strength=args.env)

if args.bg:
    lib.create_ground(size=6.0)
else:
    lib.create_shadow_catcher(size=6.0)

lib.setup_lighting(rig, key=21.0, fill=5.5, rim=24.0, size=1.2)

RES_X = int(2200 * args.res) // 2 * 2
RES_Y = int(1500 * args.res) // 2 * 2

lo, hi = lib.bounds(laptop)
aim = lib.aim_at((0, 0, hi.z * 0.55))

# Head on, and close to the height of the display rather than looking down on
# it: from above, the desk fills the frame and the screen is a sliver at the
# back.
lib.setup_camera(aim, location=(0, -2.55, hi.z * 1.15), lens=135)

lib.setup_gpu_and_cycles(args.samples, RES_X, RES_Y, film_transparent=args.bg is None)

bpy.context.scene.render.filepath = OUTPUT
print(f"Rendering laptop -> {OUTPUT}  {RES_X}x{RES_Y}  {args.samples} samples")
bpy.ops.render.render(write_still=True)
print("Done!")
