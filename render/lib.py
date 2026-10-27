"""
Shared library for the device renders.

The devices are real models imported from glTF, not geometry built here. What
this module does is put a screen image onto the one object in an imported model
that is the screen, light it, and render it.
"""

import math

import bpy
from mathutils import Vector


def clear_scene():
    """Remove every object from the scene."""
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for block in (bpy.data.objects, bpy.data.meshes, bpy.data.materials, bpy.data.images):
        for item in list(block):
            block.remove(item, do_unlink=True)


# ---------------------------------------------------------------------------
# Importing
# ---------------------------------------------------------------------------

def import_model(path):
    """Import a glTF file and return the objects it brought in."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [obj for obj in bpy.data.objects if obj not in before]


def find_screen(objects, name):
    """
    Find the object in an imported model that is the screen.

    Matched by name rather than by guessing at the largest flat face: a model
    has a dozen flat faces and only one of them is the display, and the name is
    the thing the artist actually told us.
    """
    for obj in objects:
        if obj.type == "MESH" and name.lower() in obj.name.lower():
            return obj
    for obj in objects:
        if obj.type == "MESH":
            for mat in obj.data.materials:
                if mat is not None and name.lower() in mat.name.lower():
                    return obj
    raise SystemExit(f"no screen matching {name!r} in {[o.name for o in objects]}")


def split_lid(objects, hinge, direction, name="LidHinge"):
    """
    Cut a laptop model's lid away from its base and hang it off an empty.

    The MacBook arrives as a single machine in the open position: no armature,
    no hinge, and three of its meshes — the shell, the two palette bodies —
    carry base and lid geometry in one mesh each. So the split is geometric. A
    vertex belongs to the lid when it lies past `hinge` along `direction`, which
    for this model separates cleanly: the base tops out just under the plane and
    the lid starts just over it.

    Returns the empty, placed at the hinge with the lid parented to it, so that
    rotating it about X opens and closes the machine.
    """
    bpy.ops.object.select_all(action="DESELECT")
    bpy.ops.object.mode_set(mode="OBJECT")

    lid = []
    for obj in list(objects):
        if obj.type != "MESH":
            continue
        world = obj.matrix_world
        over = [v.index for v in obj.data.vertices
                if (world @ v.co - hinge).dot(direction) > 0.0]
        if not over:
            continue

        if len(over) == len(obj.data.vertices):
            lid.append(obj)
            continue

        # A mesh straddling the hinge is separated in edit mode. `separate`
        # leaves the new object selected and the original active, which is the
        # only way to tell which of the two came out.
        for v in obj.data.vertices:
            v.select = v.index in set(over)
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.separate(type="SELECTED")
        bpy.ops.object.mode_set(mode="OBJECT")
        for part in bpy.context.selected_objects:
            if part is not obj:
                lid.append(part)
        bpy.ops.object.select_all(action="DESELECT")

    bpy.ops.object.empty_add(type="PLAIN_AXES", location=hinge)
    empty = bpy.context.active_object
    empty.name = name
    for obj in lid:
        obj.parent = empty
        obj.matrix_parent_inverse = empty.matrix_world.inverted()
    bpy.context.view_layer.update()
    return empty, lid


def unit_scale(objects, target_height, axis=2):
    """
    Scale a model so its longest run along `axis` measures `target_height`.

    A downloaded model arrives in whatever units its author used. Everything
    downstream — the camera distance, the light sizes, the depth of field — is
    stated in metres, so the model is put into metres once, here.
    """
    lo = min((obj.matrix_world @ Vector(c)).to_tuple()[axis]
             for obj in objects if obj.type == "MESH"
             for c in obj.bound_box)
    hi = max((obj.matrix_world @ Vector(c)).to_tuple()[axis]
             for obj in objects if obj.type == "MESH"
             for c in obj.bound_box)
    factor = target_height / (hi - lo)

    bpy.ops.object.select_all(action="DESELECT")
    roots = [obj for obj in objects if obj.parent is None]
    for obj in roots:
        obj.select_set(True)
        obj.scale = tuple(s * factor for s in obj.scale)
    bpy.context.view_layer.update()
    return factor


def bounds(objects):
    """The world-space bounding box of a set of objects, as (min, max)."""
    corners = [obj.matrix_world @ Vector(c)
               for obj in objects if obj.type == "MESH"
               for c in obj.bound_box]
    lo = Vector((min(c.x for c in corners), min(c.y for c in corners), min(c.z for c in corners)))
    hi = Vector((max(c.x for c in corners), max(c.y for c in corners), max(c.z for c in corners)))
    return lo, hi


def drop_to_ground(objects):
    """Move a model so its lowest point sits at z = 0."""
    lo, _ = bounds(objects)
    for obj in objects:
        if obj.parent is None:
            obj.location.z -= lo.z
    bpy.context.view_layer.update()


# ---------------------------------------------------------------------------
# The screen
# ---------------------------------------------------------------------------

def apply_screen(obj, image_path, brightness=1.0, rotate=0, flip_u=False, flip_v=False, gloss=0.006):
    """
    Put a UI image on the screen object of an imported model.

    The image goes on the model's own screen geometry rather than on a plane
    placed over it. That geometry already has the display's corner radius and
    sits at the depth the glass puts it, which is what stops the picture
    appearing to float clear of the device at the corners.

    Emission rather than a lit surface: a screen is a light source, and a
    diffuse one goes grey as soon as the key light moves.
    """
    mat = bpy.data.materials.new(name="ScreenUI")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    coord = nodes.new("ShaderNodeTexCoord")

    # The model's own UV layout is whatever its author chose, and it is rarely a
    # clean 0..1 rectangle in the orientation the screenshot is in. Mapping the
    # image by generated coordinates instead puts it on the object's bounding
    # box, which for a flat screen is exactly the display rectangle.
    # Generated coordinates run 0..1, so a rotation about the origin swings the
    # image off the screen. It is moved to put its middle at the origin, turned
    # there, and moved back.
    centre = nodes.new("ShaderNodeMapping")
    centre.inputs["Location"].default_value = (-0.5, -0.5, 0.0)
    links.new(coord.outputs["Generated"], centre.inputs["Vector"])

    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Location"].default_value = (0.5, 0.5, 0.0)
    mapping.inputs["Rotation"].default_value = (0.0, 0.0, math.radians(rotate))
    mapping.inputs["Scale"].default_value = (
        -1.0 if flip_u else 1.0,
        -1.0 if flip_v else 1.0,
        1.0,
    )
    links.new(centre.outputs["Vector"], mapping.inputs["Vector"])

    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(image_path)
    tex.image.colorspace_settings.name = "sRGB"
    tex.interpolation = "Cubic"
    tex.extension = "EXTEND"
    links.new(mapping.outputs["Vector"], tex.inputs["Vector"])

    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Strength"].default_value = brightness
    links.new(tex.outputs["Color"], emission.inputs["Color"])

    # A little of the room on the glass, so the display reads as something
    # behind glass rather than as a sticker. Kept very small: the app's UI is
    # nearly black, and a studio environment reflected at even a few percent
    # lifts the whole screen to grey.
    glossy = nodes.new("ShaderNodeBsdfGlossy")
    glossy.inputs["Roughness"].default_value = 0.08

    mix = nodes.new("ShaderNodeMixShader")
    mix.inputs["Fac"].default_value = gloss
    links.new(emission.outputs["Emission"], mix.inputs[1])
    links.new(glossy.outputs["BSDF"], mix.inputs[2])

    out = nodes.new("ShaderNodeOutputMaterial")
    links.new(mix.outputs["Shader"], out.inputs["Surface"])

    obj.data.materials.clear()
    obj.data.materials.append(mat)
    return mat


def retint(names, color, metallic=None, roughness=None):
    """
    Override the base colour of some of a model's materials.

    A downloaded phone comes in whatever finish its author picked — this one is
    teal — and a product shot wants the finish chosen rather than inherited. The
    texture, if there is one, is disconnected: a base colour set behind a linked
    texture does nothing, which is a quiet way to spend an hour.
    """
    for name in names:
        mat = bpy.data.materials.get(name)
        if mat is None or not mat.use_nodes:
            continue
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf is None:
            continue
        base = bsdf.inputs["Base Color"]
        for link in list(base.links):
            mat.node_tree.links.remove(link)
        base.default_value = (*color, 1.0)
        if metallic is not None:
            bsdf.inputs["Metallic"].default_value = metallic
        if roughness is not None:
            bsdf.inputs["Roughness"].default_value = roughness


def dim_glass(names, transmission=0.03):
    """
    Turn a model's cover glass down to a faint reflection.

    A screen protector modelled as clear glass is right in a bright room and
    wrong in this one: the display underneath is nearly black, and a full
    specular reflection of a studio softbox turns the whole UI grey. Keeping a
    few percent leaves the highlight that says there is glass there.
    """
    for name in names:
        mat = bpy.data.materials.get(name)
        if mat is None or not mat.use_nodes:
            continue
        nodes = mat.node_tree.nodes
        links = mat.node_tree.links
        nodes.clear()

        glossy = nodes.new("ShaderNodeBsdfGlossy")
        glossy.inputs["Roughness"].default_value = 0.05

        clear = nodes.new("ShaderNodeBsdfTransparent")

        mix = nodes.new("ShaderNodeMixShader")
        mix.inputs["Fac"].default_value = transmission
        links.new(clear.outputs["BSDF"], mix.inputs[1])
        links.new(glossy.outputs["BSDF"], mix.inputs[2])

        out = nodes.new("ShaderNodeOutputMaterial")
        links.new(mix.outputs["Shader"], out.inputs["Surface"])


# ---------------------------------------------------------------------------
# The room
# ---------------------------------------------------------------------------

def create_ground(size=6.0, color=(0.012, 0.013, 0.016), roughness=0.45):
    """
    The surface the device stands on.

    Dark and a little glossy, so the device gets a reflection under it without
    the floor becoming a mirror the eye reads before the product.
    """
    bpy.ops.mesh.primitive_plane_add(size=size, location=(0, 0, 0))
    ground = bpy.context.active_object
    ground.name = "Ground"

    mat = bpy.data.materials.new("Ground")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Specular IOR Level"].default_value = 0.3
    ground.data.materials.append(mat)
    return ground


def create_shadow_catcher(size=6.0):
    """
    A ground plane that catches shadow and nothing else.

    A transparent render cannot have a floor — a lit plane is opaque wherever
    the camera sees it, and the shot arrives as a rectangle around the device.
    A shadow catcher is the way to have both: the plane itself stays invisible
    and only the shadow falling on it is written into the alpha, so the device
    sits on a surface on whatever the page's background turns out to be.
    """
    bpy.ops.mesh.primitive_plane_add(size=size, location=(0, 0, 0))
    plane = bpy.context.active_object
    plane.name = "ShadowCatcher"
    plane.is_shadow_catcher = True
    return plane


def setup_lighting(target, key=90.0, fill=28.0, rim=55.0, size=1.6,
                   rim_color=(0.72, 0.66, 1.0)):
    """
    Three area lights tracking a target: key, fill and rim.

    The energies are arguments because the scenes want different rooms — a phone
    on a stand and a laptop in a dark room are not lit the same way.

    `rim_color` is one too, because a tinted rim is a choice with a cost: it
    draws a coloured line down every edge it catches, including the ones a
    product shot wants invisible, and it puts a glow in anything glassy.
    """
    def light(name, location, energy, color, radius):
        bpy.ops.object.light_add(type="AREA", location=location)
        obj = bpy.context.active_object
        obj.name = name
        obj.data.energy = energy
        obj.data.size = radius
        obj.data.color = color
        track = obj.constraints.new(type="TRACK_TO")
        track.target = target
        track.track_axis = "TRACK_NEGATIVE_Z"
        track.up_axis = "UP_Y"
        return obj

    light("KeyLight", (1.5, -1.9, 2.2), key, (1.0, 0.99, 0.97), size * 1.6)
    light("FillLight", (-1.9, -1.4, 0.9), fill, (0.94, 0.96, 1.0), size * 2.2)
    # Behind and above, to separate the body from the background.
    if rim > 0:
        light("RimLight", (-0.8, 2.4, 1.9), rim, rim_color, size)


def setup_hdri(path, strength=1.0, rotation=0.0, visible=False):
    """
    Light the scene from an environment map.

    What makes metal read as metal is a reflection with shape in it. Area lights
    give a body four rectangular highlights and nothing else; an environment
    gives it a room to reflect, which is the difference between a render of a
    phone and a photograph of one.

    `visible` is off by default, so the environment lights the device without
    appearing behind it — the shot stays transparent.
    """
    world = bpy.data.worlds.get("World") or bpy.data.worlds.new("World")
    bpy.context.scene.world = world
    world.use_nodes = True
    nodes = world.node_tree.nodes
    links = world.node_tree.links
    nodes.clear()

    coord = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Rotation"].default_value = (0.0, 0.0, math.radians(rotation))
    links.new(coord.outputs["Generated"], mapping.inputs["Vector"])

    env = nodes.new("ShaderNodeTexEnvironment")
    env.image = bpy.data.images.load(path)
    links.new(mapping.outputs["Vector"], env.inputs["Vector"])

    bg = nodes.new("ShaderNodeBackground")
    bg.inputs["Strength"].default_value = strength
    links.new(env.outputs["Color"], bg.inputs["Color"])

    out = nodes.new("ShaderNodeOutputWorld")
    links.new(bg.outputs["Background"], out.inputs["Surface"])

    bpy.context.scene.render.film_transparent = not visible


def setup_world(strength=0.02, color=(0.10, 0.11, 0.14)):
    """A dim ambient, so the shadow side is not pure black."""
    world = bpy.data.worlds.get("World") or bpy.data.worlds.new("World")
    bpy.context.scene.world = world
    world.use_nodes = True
    nodes = world.node_tree.nodes
    nodes.clear()
    bg = nodes.new("ShaderNodeBackground")
    bg.inputs["Color"].default_value = (*color, 1.0)
    bg.inputs["Strength"].default_value = strength
    out = nodes.new("ShaderNodeOutputWorld")
    world.node_tree.links.new(bg.outputs["Background"], out.inputs["Surface"])


# ---------------------------------------------------------------------------
# Camera
# ---------------------------------------------------------------------------

def setup_camera(target, location, lens=85, fstop=None):
    """Place a camera tracking a target, with depth of field when asked for."""
    bpy.ops.object.camera_add(location=location)
    camera = bpy.context.active_object
    camera.name = "Camera"
    track = camera.constraints.new(type="TRACK_TO")
    track.target = target
    track.track_axis = "TRACK_NEGATIVE_Z"
    track.up_axis = "UP_Y"
    camera.data.lens = lens

    if fstop is not None:
        camera.data.dof.use_dof = True
        camera.data.dof.focus_object = target
        camera.data.dof.aperture_fstop = fstop

    bpy.context.scene.camera = camera
    return camera


def aim_at(location):
    """An empty for a camera to track, so the aim is not tied to a model."""
    bpy.ops.object.empty_add(type="PLAIN_AXES", location=location)
    empty = bpy.context.active_object
    empty.name = "CameraTarget"
    return empty


# ---------------------------------------------------------------------------
# Render setup
# ---------------------------------------------------------------------------

def setup_gpu_and_cycles(samples, res_x, res_y, film_transparent=True):
    """Configure Cycles with GPU detection, falling back to CPU."""
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = samples
    scene.cycles.use_denoising = True
    scene.cycles.denoiser = "OPTIX"
    scene.cycles.denoising_use_gpu = True
    if hasattr(scene.render, "use_persistent_data"):
        scene.render.use_persistent_data = True

    prefs = bpy.context.preferences.addons["cycles"].preferences
    for device_type in ("OPTIX", "CUDA", "HIP", "METAL"):
        try:
            prefs.compute_device_type = device_type
            prefs.get_devices()
            found = False
            for device in prefs.devices:
                if device.type == "CPU":
                    device.use = False
                else:
                    device.use = True
                    found = True
            if found:
                scene.cycles.device = "GPU"
                print(f"Using GPU compute: {device_type} (CPU disabled)")
                break
        except Exception:
            continue
    else:
        print("No GPU found, using CPU")
        scene.cycles.device = "CPU"

    scene.render.resolution_x = res_x
    scene.render.resolution_y = res_y
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = film_transparent

    # Filmic would wash the UI out: the screen is an emitter carrying the app's
    # own greys, and the point of the shot is that they arrive as themselves.
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"

    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
