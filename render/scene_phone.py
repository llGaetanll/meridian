"""
Render a phone showing a screen.

    blender --background --python scene_phone.py -- screen.png out.png [options]

Options:
    --model PATH   the phone model (default models/iphone.glb)
    --samples N    render samples (default 256)
    --res F        resolution multiplier (default 1)
    --tilt F       degrees the phone leans back (default 8)
    --turn F       degrees the phone is turned to the left (default 16)
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

parser = argparse.ArgumentParser(description="Render a phone product shot")
parser.add_argument("screen", help="UI image to put on the display")
parser.add_argument("output", help="Where the PNG goes")
parser.add_argument("--model", default=os.path.join(HERE, "models/iphone.glb"))
parser.add_argument("--samples", type=int, default=256)
parser.add_argument("--res", type=float, default=1.0)
parser.add_argument("--tilt", type=float, default=14.0)
parser.add_argument("--turn", type=float, default=-18.0)
parser.add_argument("--rotate", type=float, default=0.0)
parser.add_argument("--flip-u", dest="flip_u", action="store_true")
parser.add_argument("--flip-v", dest="flip_v", action="store_true")
parser.add_argument("--bg", type=str, default=None)
parser.add_argument("--env", type=float, default=0.55, help="environment light strength")

args = parser.parse_args(argv)

SCREEN = os.path.abspath(args.screen)
OUTPUT = os.path.abspath(args.output)

# A current-generation 6.3" phone is 149.6mm tall.
PHONE_H = 0.1496

lib.clear_scene()

phone = lib.import_model(os.path.abspath(args.model))
# The model stands upright already, with its long run along Z.
lib.unit_scale(phone, PHONE_H, axis=2)

# The model ships in a teal finish with a bezel that reads light grey. Black
# titanium, and a bezel dark enough that the display's edge is the edge.
lib.retint(["Material.002", "Material.004"], (0.021, 0.022, 0.024), metallic=1.0, roughness=0.33)
lib.retint(["Rim_Buttons", "Screw"], (0.030, 0.031, 0.034), metallic=1.0, roughness=0.30)
lib.retint(["Screen_Rim"], (0.004, 0.004, 0.005), metallic=0.0, roughness=0.45)
# The camera lenses ship with a violet tint baked into their texture. A real
# lens coating flares blue-green at an angle and reads near-black head on; the
# purple dot is the single most obviously wrong thing in an otherwise good
# model, so it goes.
lib.retint(
    ["Camera_Pixel_Glass_002", "Camera_Pixel__002", "Glass_Camera_Logo"],
    (0.010, 0.011, 0.014),
    metallic=0.0,
    roughness=0.12,
)
# The cover glass sits over the display and reflects the whole room. Left as it
# ships, a white studio turns a black UI grey — a real screen shows the room
# only where the room is bright, and the app's own greys everywhere else.
lib.dim_glass(["Screen_Glass"], transmission=0.02)

screen = lib.find_screen(phone, "Screen_BG")
lib.apply_screen(
    screen,
    SCREEN,
    brightness=3.0,
    rotate=args.rotate,
    flip_u=args.flip_u,
    flip_v=args.flip_v,
)

# Leaned back and turned through one empty, so the model's own hierarchy is
# left alone.
bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0, 0, 0))
rig = bpy.context.active_object
rig.name = "PhoneRig"
for obj in phone:
    if obj.parent is None:
        obj.parent = rig
        obj.matrix_parent_inverse = rig.matrix_world.inverted()

rig.rotation_euler = (math.radians(-args.tilt), 0, math.radians(-args.turn))
bpy.context.view_layer.update()
lib.drop_to_ground([rig] + phone)

# The room the body reflects. Area lights alone give aluminium four rectangular
# highlights and nothing else, which is what makes a render look like a render.
lib.setup_hdri(os.path.join(HERE, "hdri/studio_small_08_2k.hdr"), strength=args.env)

if args.bg:
    lib.create_ground(size=4.0)
else:
    lib.create_shadow_catcher(size=4.0)

# A key over the top of the environment, and no tinted rim. A violet rim draws a
# bright line down every edge of the body and puts a glow in the camera glass,
# and neither is on a phone you are actually holding: the borders of a black
# phone in a dark room are close to invisible.
lib.setup_lighting(rig, key=30.0, fill=8.0, rim=8.0, size=0.9,
                   rim_color=(1.0, 1.0, 1.0))

RES_X = int(1500 * args.res) // 2 * 2
RES_Y = int(2000 * args.res) // 2 * 2

_, hi = lib.bounds(phone)
aim = lib.aim_at((0, 0, hi.z / 2))

# Far back on a long lens. Close and wide is what makes a phone look like a
# fish-eye of a phone, with the near edge twice the size of the far one.
lib.setup_camera(aim, location=(0.13, -0.81, hi.z * 0.62), lens=180)

lib.setup_gpu_and_cycles(args.samples, RES_X, RES_Y, film_transparent=args.bg is None)

bpy.context.scene.render.filepath = OUTPUT
print(f"Rendering phone -> {OUTPUT}  {RES_X}x{RES_Y}  {args.samples} samples")
bpy.ops.render.render(write_still=True)
print("Done!")
