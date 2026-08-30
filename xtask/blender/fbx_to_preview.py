"""Turn one mesh file into one picture of it, and count what is in it.

`cargo xtask art describe` shows that picture to a vision model and files
what the model says in `art/dex/<pack>.toml`. This script is the whole of
the looking: no scene, no user preferences, no add-ons beyond the bundled
importers and whichever render engine answers first.

Run as::

    blender --background --factory-startup --python-exit-code 1 \
        --python fbx_to_preview.py -- <source> <destination.png> [texture]

The third argument is the atlas to paint with, and it means exactly what
it means to `fbx_to_gltf.py` — which is not a coincidence, because this
script imports that one and calls its `paint_with`. **The two must agree
about what a texture is.** A preview painted by a rule that had drifted
from the converter's would be a picture of a grey crate filed beside a
conversion that came out in colour, and the catalogue would then carry a
sentence describing the wrong object. So `usable_image`, `paint_with` and
`import_any` are borrowed rather than copied, and `art/cache/blender/`
holds both files for that reason.

One thing it does NOT borrow is `refuse_unless_painted`. A conversion that
painted nothing is a grey mesh shipped into a game and is refused; a
preview that painted nothing is a picture of an unpainted mesh, which is
worth rendering and worth saying so about — the `image` lines below are
empty in that case, and the catalogue records that the scene bore no
textures rather than pretending it did.

## What it prints

One fact per line, read by `xtask/src/preview.rs`::

    tris 412
    meshes 1
    materials 1
    image PolygonSciFiSpace_Texture_01_A.png
    aabb <min x> <min y> <min z> <max x> <max y> <max z>

The `aabb` line is the converter's own, measured by the converter's own
function, in the exported file's axes rather than Blender's.

## The picture

Four views in one image: the mesh as it is, and three copies of it turned
a quarter, a half and three quarters about its up axis, laid out two by
two in the camera's own plane. A model shown one three-quarter view of a
crate can say nothing about the back of it, and four views cost one
render — the copies share their mesh data, so the scene is four objects
and one mesh.

The camera is orthographic and the copies are offset in the image plane,
so nothing can occlude anything else however deep the mesh is. The view
transform is set to Standard where that exists: Synty's palettes are flat
colour on a shared atlas, and a filmic curve over them is a description of
the wrong colours.
"""

import math
import os
import sys

import bpy
import mathutils

# The converter, beside this file in `art/cache/blender/`, imported for
# its answers about importing and painting. `cargo xtask art describe`
# writes both files out before running this one.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fbx_to_gltf  # noqa: E402 - the path above is what makes it importable


#: How wide and tall the rendered contact sheet is, in pixels. Four views
#: at 512, which is about what a hosted vision model reads before it
#: downsamples anything, and small enough that the base64 of it is not the
#: bulk of a request.
SIZE = 1024


def span():
    """The box round every mesh in the scene, in Blender's own axes.

    Not `fbx_to_gltf.bounds`, and the difference is deliberate: that one
    answers in the exported file's axes, because it is the number the
    manifest's `fill` promise is checked against. This one places a
    camera, and a camera lives in the scene.
    """
    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    for obj in fbx_to_gltf.mesh_objects():
        for corner in obj.bound_box:
            point = obj.matrix_world @ mathutils.Vector(corner)
            for axis, value in enumerate((point.x, point.y, point.z)):
                lo[axis] = min(lo[axis], value)
                hi[axis] = max(hi[axis], value)
    if lo[0] > hi[0]:
        return None
    return mathutils.Vector(lo), mathutils.Vector(hi)


def turntable(centre, right, up, spacing):
    """Three more copies of the scene, a quarter turn apart.

    The originals stay exactly where they are — moving them would mean
    reaching through whatever parenting an FBX arrived with — so the
    copies are offset around them and the camera is aimed at the middle of
    the four cells rather than at the mesh.

    Each copy shares its original's mesh data, so this costs four objects
    and no geometry.
    """
    cells = [
        (0.25 * math.tau, right * spacing),
        (0.5 * math.tau, -up * spacing),
        (0.75 * math.tau, right * spacing - up * spacing),
    ]
    originals = fbx_to_gltf.mesh_objects()
    for angle, offset in cells:
        about = (
            mathutils.Matrix.Translation(centre)
            @ mathutils.Matrix.Rotation(angle, 4, "Z")
            @ mathutils.Matrix.Translation(-centre)
        )
        placed = mathutils.Matrix.Translation(offset) @ about
        for obj in originals:
            copy = obj.copy()
            copy.parent = None
            copy.matrix_world = placed @ obj.matrix_world
            bpy.context.scene.collection.objects.link(copy)


def light(scene):
    """A key, a fill, and a background that is not black.

    An empty factory scene has no light and no world in it, so a render of
    one is a black square — which a vision model will describe, at length,
    as a black square.
    """
    world = bpy.data.worlds.new("preview")
    scene.world = world
    world.use_nodes = True
    background = world.node_tree.nodes.get("Background")
    if background is not None:
        background.inputs[0].default_value = (0.42, 0.44, 0.48, 1.0)
        background.inputs[1].default_value = 1.0
    for name, energy, angles in (
        ("key", 4.0, (math.radians(52), 0.0, math.radians(38))),
        ("fill", 1.5, (math.radians(65), 0.0, math.radians(-125))),
    ):
        lamp = bpy.data.lights.new(name, type="SUN")
        lamp.energy = energy
        obj = bpy.data.objects.new(name, lamp)
        obj.rotation_euler = angles
        scene.collection.objects.link(obj)


def frame(scene, centre, radius, spacing, right, up, towards):
    """An orthographic camera looking at the middle of the four cells."""
    middle = centre + (right * spacing - up * spacing) * 0.5
    camera = bpy.data.cameras.new("preview")
    camera.type = "ORTHO"
    # Two cells across, and a tenth of a cell of air round the outside.
    camera.ortho_scale = 2.2 * spacing
    obj = bpy.data.objects.new("preview", camera)
    obj.location = middle - towards * (radius * 6.0 + 1.0)
    obj.rotation_euler = towards.to_track_quat("-Z", "Y").to_euler()
    scene.collection.objects.link(obj)
    scene.camera = obj


#: The engines to try, in the order they are tried. Cycles is first
#: because it is the one that renders in `--background` on a machine with
#: no display and no GPU, which is every machine this might run on that
#: is not the owner's. The others are faster where they work.
ENGINES = ("CYCLES", "BLENDER_EEVEE_NEXT", "BLENDER_EEVEE", "BLENDER_WORKBENCH")


def render(destination):
    """Render the scene to `destination`, trying engines until one works.

    Nothing here can know what Blender build it is inside — which engines
    it has, whether it can make a GL context, whether the machine has a
    GPU at all — so the choice is made by trying, and a refusal from one
    engine is a reason to try the next rather than the end of the run.
    """
    scene = bpy.context.scene
    scene.render.filepath = destination
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.resolution_x = SIZE
    scene.render.resolution_y = SIZE
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = False
    try:
        # Synty paint flat colour onto one atlas. A filmic or AgX curve
        # over that is a picture of colours the pack does not have.
        scene.view_settings.view_transform = "Standard"
    except Exception:  # noqa: BLE001 - a Blender without it renders anyway
        pass

    complaints = []
    for engine in ENGINES:
        try:
            scene.render.engine = engine
        except Exception as err:  # noqa: BLE001 - this build has not got it
            complaints.append(f"{engine}: {err}")
            continue
        if engine == "CYCLES":
            settle_cycles(scene)
        try:
            bpy.ops.render.render(write_still=True)
        except Exception as err:  # noqa: BLE001 - try the next engine
            complaints.append(f"{engine}: {err}")
            continue
        if os.path.isfile(destination) and os.path.getsize(destination) > 0:
            return engine
        complaints.append(f"{engine}: exited cleanly and wrote no file")
    raise SystemExit(
        "could not render a preview with any engine this Blender has\n  "
        + "\n  ".join(complaints)
    )


def settle_cycles(scene):
    """Few samples, few bounces, denoised if this build can.

    A Synty prop is flat colour under two suns; the picture is being read
    by a model that is about to describe its shape, not printed. Every
    line here is guarded because `scene.cycles` is an add-on's property
    group and the names in it have moved between releases.
    """
    try:
        bpy.ops.preferences.addon_enable(module="cycles")
    except Exception:  # noqa: BLE001 - a bundled add-on that is already on
        pass
    for name, value in (
        ("samples", 16),
        ("preview_samples", 16),
        ("max_bounces", 3),
        ("use_denoising", True),
        ("caustics_reflective", False),
        ("caustics_refractive", False),
    ):
        try:
            setattr(scene.cycles, name, value)
        except Exception:  # noqa: BLE001 - a setting this build has not got
            pass


def image_file(image):
    """What to call an image in the catalogue: its file, not its datablock.

    Blender names a second datablock for the same file
    `PolygonApocalypse_Texture_01_A.png.001`, and it does that routinely
    here — the importer loads the atlas the FBX names and `paint_with`
    loads the one the resolver staged. A catalogue that recorded the
    datablock name would be recording a fact about one Blender session,
    and `.001` is not a file anybody can go and look at.
    """
    path = getattr(image, "filepath", "") or ""
    leaf = os.path.basename(path.replace("\\", "/").rstrip("/"))
    return leaf or image.name


def count():
    """Triangles, meshes, materials and the images actually bound.

    The triangles are counted off the mesh as it stands, which is what a
    person comparing forty crates wants and is not the same as what the
    exporter will emit: a modifier or a subdivision would change it. Synty
    props carry neither, and a count that needed an evaluated depsgraph to
    be exact would be a count that fails differently on every Blender.
    """
    triangles = 0
    meshes = 0
    materials = []
    images = []
    for obj in fbx_to_gltf.mesh_objects():
        meshes += 1
        mesh = obj.data
        for polygon in getattr(mesh, "polygons", []):
            triangles += max(len(polygon.vertices) - 2, 0)
        for material in getattr(mesh, "materials", None) or []:
            if material is None or material.name in materials:
                continue
            materials.append(material.name)
            tree = getattr(material, "node_tree", None)
            if tree is None:
                continue
            for node in tree.nodes:
                if node.type != "TEX_IMAGE":
                    continue
                image = node.image
                if not fbx_to_gltf.usable_image(image):
                    continue
                name = image_file(image)
                if name not in images:
                    images.append(name)
    return triangles, meshes, materials, images


def main():
    if "--" not in sys.argv:
        raise SystemExit(
            "expected: blender --background --python fbx_to_preview.py -- "
            "<source> <destination.png> [texture]"
        )
    arguments = sys.argv[sys.argv.index("--") + 1 :]
    if len(arguments) not in (2, 3):
        raise SystemExit(
            f"expected a source, a destination and an optional texture, got {arguments}"
        )
    source, destination = arguments[0], arguments[1]
    texture = arguments[2] if len(arguments) == 3 else None
    if not os.path.isfile(source):
        raise SystemExit(f"no such source file: {source}")

    bpy.ops.wm.read_factory_settings(use_empty=True)
    fbx_to_gltf.import_any(source)
    if not fbx_to_gltf.mesh_objects():
        raise SystemExit(f"{source} imported without producing a single mesh")
    if texture is not None:
        # Painted, and not refused when the painting reaches nothing: an
        # unpainted preview is a fact about the mesh worth recording.
        fbx_to_gltf.paint_with(texture)

    triangles, meshes, materials, images = count()
    print(f"tris {triangles}")
    print(f"meshes {meshes}")
    print(f"materials {len(materials)}")
    for image in images:
        print(f"image {image}")
    # **Measured before the turntable below copies anything.** The box
    # this prints is the box round the mesh, and three of the four things
    # about to be standing in the scene are copies of it a metre apart —
    # so a measurement taken afterwards is a measurement of the grid. It
    # was: the first real crate through this went into the catalogue at
    # 1.84 units across, being 0.73.
    fbx_to_gltf.report_bounds()

    measured = span()
    if measured is not None:
        lo, hi = measured
        centre = (lo + hi) * 0.5
        radius = max((hi - lo).length * 0.5, 0.001)
        towards = mathutils.Vector((0.72, 0.58, -0.38)).normalized()
        right = towards.cross(mathutils.Vector((0.0, 0.0, 1.0))).normalized()
        up = right.cross(towards).normalized()
        # A cell wide enough that no turn of the mesh can reach the next
        # one: the diagonal is the widest any rotation of a box can be.
        spacing = (hi - lo).length * 1.06
        turntable(centre, right, up, spacing)
        light(bpy.context.scene)
        frame(bpy.context.scene, centre, radius, spacing, right, up, towards)

    engine = render(destination)
    print(f"fbx_to_preview: wrote {destination} with {engine}")


if __name__ == "__main__":
    main()
