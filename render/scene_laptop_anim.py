"""
Render a laptop opening, its screen coming on as the lid rises.

    blender --background --python scene_laptop_anim.py -- screen.png out.webm [options]

The last frame is the still `scene_laptop.py` renders: same lid angle, same
camera, same lighting, so the video can hand over to the PNG without a jump.

Options:
    --model PATH   the laptop model (default models/macbook.glb)
    --samples N    render samples (default 256)
    --res F        resolution multiplier (default 1)
    --frames N     length in frames at 60fps (default 120, two seconds)
    --still N      render frame N as a PNG instead of the whole video
    --turn F       degrees the machine is turned to the left (default 0, head on)
    --env F        environment light strength (default 0.52)
    --rotate F     degrees the screen image is turned on the display
    --flip-u       mirror the screen image left to right
    --flip-v       mirror the screen image top to bottom
"""

import argparse
import math
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib

HERE = os.path.dirname(os.path.abspath(__file__))

argv = sys.argv
argv = argv[argv.index("--") + 1:] if "--" in argv else []

parser = argparse.ArgumentParser(description="Render a laptop opening")
parser.add_argument("screen", help="UI image to put on the display")
parser.add_argument("output", help="Where the WebM goes")
parser.add_argument("--model", default=os.path.join(HERE, "models/macbook.glb"))
parser.add_argument("--samples", type=int, default=256)
parser.add_argument("--res", type=float, default=1.0)
parser.add_argument("--frames", type=int, default=120)
parser.add_argument("--still", type=int, default=0)
parser.add_argument("--turn", type=float, default=0.0)
parser.add_argument("--env", type=float, default=0.52)
parser.add_argument("--rotate", type=float, default=0.0)
parser.add_argument("--flip-u", dest="flip_u", action="store_true")
parser.add_argument("--flip-v", dest="flip_v", action="store_true")

args = parser.parse_args(argv)

SCREEN = os.path.abspath(args.screen)
OUTPUT = os.path.abspath(args.output)

# A 16" MacBook Pro is 355.7mm wide.
LAPTOP_W = 0.3557

# The hinge, in the model's own metres after scaling. The lid leans 20° back
# from vertical, and the plane through this point along that lean is what
# `split_lid` cuts on: everything past it is lid, and the base stops just short.
HINGE = Vector((0.0, 0.1300, 0.0090))
LID_DIR = Vector((0.0, math.sin(math.radians(20.0)), math.cos(math.radians(20.0))))

# Closed, the lid lies flat over the keyboard. Open it leans 20° back, so the
# travel is 90° to vertical plus the 20° beyond it.
CLOSED = math.radians(110.0)

lib.clear_scene()

laptop = lib.import_model(os.path.abspath(args.model))
lib.unit_scale(laptop, LAPTOP_W, axis=0)

# Space Black aluminium, as in the still. The palette materials are left alone:
# they carry the keycap legends, and retinting them paints over the keyboard.
BODY = ["lmWQsEjxpsebDlK", "CRQixVLpahJzhJc", "iyDJFXmHelnMTbD", "LpqXZqhaGCeSzdu"]
lib.retint(BODY, (0.026, 0.027, 0.030), metallic=1.0, roughness=0.38)

screen = lib.find_screen(laptop, "Object_123")
screen_mat = lib.apply_screen(
    screen,
    SCREEN,
    brightness=2.2,
    rotate=args.rotate,
    flip_u=args.flip_u,
    flip_v=args.flip_v,
)

hinge, lid = lib.split_lid(laptop, HINGE, LID_DIR)
parts = laptop + lid + [hinge]

bpy.ops.object.empty_add(type="PLAIN_AXES", location=(0, 0, 0))
rig = bpy.context.active_object
rig.name = "LaptopRig"
for obj in parts:
    if obj.parent is None:
        obj.parent = rig
        obj.matrix_parent_inverse = rig.matrix_world.inverted()

rig.rotation_euler = (0, 0, math.radians(-args.turn))
bpy.context.view_layer.update()

# Framing is measured with the lid open, so that the camera the still uses is
# the camera the last frame gets. Dropping to the ground has to happen here too:
# a closed lid has a different lowest point, and levelling on that would sink
# the machine by the thickness of the lid for the whole shot.
lib.drop_to_ground([rig] + parts)
lo, hi = lib.bounds(laptop + lid)

lib.setup_hdri(os.path.join(HERE, "hdri/studio_small_08_2k.hdr"), strength=args.env)
lib.create_shadow_catcher(size=6.0)
lib.setup_lighting(rig, key=21.0, fill=5.5, rim=24.0, size=1.2)

RES_X = int(2200 * args.res) // 2 * 2
RES_Y = int(1500 * args.res) // 2 * 2

aim = lib.aim_at((0, 0, hi.z * 0.55))
aim.parent = rig
aim.matrix_parent_inverse = rig.matrix_world.inverted()
lib.setup_camera(aim, location=(0, -2.55, hi.z * 1.15), lens=135)

lib.setup_gpu_and_cycles(args.samples, RES_X, RES_Y, film_transparent=True)

scene = bpy.context.scene
scene.frame_start = 1
scene.frame_end = args.frames
scene.render.fps = 60

# The lid runs for the first three quarters and the screen wakes over the last
# half, so the two overlap rather than happening in turn.
LID_END = max(2, int(args.frames * 0.72))
GLOW_START = int(args.frames * 0.45)
GLOW_END = max(GLOW_START + 1, int(args.frames * 0.88))

hinge.rotation_euler = (CLOSED, 0.0, 0.0)
hinge.keyframe_insert(data_path="rotation_euler", index=0, frame=1)
hinge.rotation_euler = (0.0, 0.0, 0.0)
hinge.keyframe_insert(data_path="rotation_euler", index=0, frame=LID_END)

strength = screen_mat.node_tree.nodes["Emission"].inputs["Strength"]
strength.default_value = 0.0
strength.keyframe_insert(data_path="default_value", frame=GLOW_START)
strength.default_value = 2.2
strength.keyframe_insert(data_path="default_value", frame=GLOW_END)


def curves(datablock):
    """
    The f-curves of an animated datablock.

    Blender 4.4 moved actions into layered strips; the flat `fcurves` list is
    still there on older files, so both shapes are read.
    """
    action = datablock.animation_data.action
    if hasattr(action, "fcurves") and len(action.fcurves):
        return list(action.fcurves)
    found = []
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                found.extend(bag.fcurves)
    return found


# Ease out, so the lid settles into the open position rather than stopping dead
# at it. The screen ramp is linear: a light coming up wants no acceleration.
for curve in curves(hinge):
    for key in curve.keyframe_points:
        key.interpolation = "QUAD"
        key.easing = "EASE_OUT"
for curve in curves(screen_mat.node_tree):
    for key in curve.keyframe_points:
        key.interpolation = "LINEAR"

if args.still:
    scene.frame_set(args.still)
    scene.render.filepath = OUTPUT
    print(f"Rendering frame {args.still} -> {OUTPUT}  {RES_X}x{RES_Y}  {args.samples} samples")
    bpy.ops.render.render(write_still=True)
else:
    scene.render.image_settings.media_type = "VIDEO"
    scene.render.image_settings.file_format = "FFMPEG"
    # VP9 in WebM is the one encoder here that keeps an alpha channel, which the
    # page needs for the same reason the still is a transparent PNG.
    scene.render.ffmpeg.format = "WEBM"
    scene.render.ffmpeg.codec = "WEBM"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.ffmpeg.constant_rate_factor = "HIGH"
    scene.render.ffmpeg.ffmpeg_preset = "GOOD"
    scene.render.filepath = OUTPUT
    print(f"Rendering {args.frames} frames -> {OUTPUT}  {RES_X}x{RES_Y}  {args.samples} samples")
    bpy.ops.render.render(animation=True)

print("Done!")
