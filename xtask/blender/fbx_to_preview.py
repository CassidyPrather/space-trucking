"""Turn one mesh file into one picture of it, and count what is in it.

`cargo xtask art describe` shows that picture to a vision model and files
what the model says in `art/dex/<pack>.toml`. This script is the whole of
the looking: no scene, no user preferences, no add-ons beyond the bundled
importers and whichever render engine answers first.

Run as::

    blender --background --factory-startup --python-exit-code 1 \
        --python fbx_to_preview.py -- <jobs file>

**One launch, many meshes**, and that is the whole reason the jobs arrive
in a file. Blender takes about two and a quarter seconds to start and
import its own Python before it has looked at anything, which was half
the cost of describing a prop; over a library of fifty thousand it is
thirty hours of starting up. So the file holds one job per line::

    <source>|<destination.png>|<texture>

`|` is the separator because Windows forbids it in a path and a shell
stand-in can split on it in one line. The texture — the atlas to paint
with — may be empty, and it means exactly what it means to
`fbx_to_gltf.py`. That is not a coincidence: this script imports that one
and calls its `paint_with`. **The two must agree about what a texture
is.** A preview painted by a rule that had drifted from the converter's
would be a picture of a grey crate filed beside a conversion that came
out in colour, and the catalogue would carry a sentence describing the
wrong object. So `usable_image`, `paint_with` and `import_any` are
borrowed rather than copied, and `art/cache/blender/` holds both files
for that reason.

**One bad mesh does not take the rest of the launch with it.** Each job
is run inside a try, and a job that raises prints `trouble <n> <why>` and
the next one starts — because a chunk is up to thirty-two meshes and one
unreadable FBX among them is not a reason to lose the other thirty-one.

One thing it does NOT borrow is `refuse_unless_painted`. A conversion that
painted nothing is a grey mesh shipped into a game and is refused; a
preview that painted nothing is a picture of an unpainted mesh, which is
worth rendering and worth saying so about — the `image` lines below are
empty in that case, and the catalogue records that the scene bore no
textures rather than pretending it did.

## What it prints

One block per job, headed by the job's line number, read by
`xtask/src/preview.rs`::

    look 1
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

`$ART_PREVIEW_SIZE` is how many pixels square the whole sheet is, so a
quarter of it across is one view. These are flat-shaded low-poly props
against a plain ground, and the thing that loses a description is a bad
angle rather than a missing pixel — see `SIZE` for what was measured.
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


#: How wide and tall the rendered contact sheet is, in pixels — four
#: views, so a quarter of this across is one view.
#:
#: It was 1024, which is 512 to a view, and that was overkill twice over:
#: a Synty prop is flat colour on a shared atlas, and Cycles time, PNG
#: bytes and the tokens an image costs all scale with the pixel count.
#:
#: 256 was chosen by rendering the same sixteen barrels at both sizes and
#: reading what came back. It halves the time per mesh (2.07 s to 1.12 s),
#: takes a preview from 245 KB to 70 KB and a call from 680 prompt tokens
#: to 450 — and the descriptions do not get worse. On the one mesh where
#: the two runs disagreed, a crate of drums, the 256 answer was the more
#: accurate of the two: it saw the olive drum that the 512 answer missed.
#: What neither size fixes is counting — "three hoops" and "four hoops"
#: came from the same barrel — because that is the model and not the
#: pixels.
#:
#: `$ART_PREVIEW_SIZE` overrides it for a pack of something more detailed.
SIZE = int(os.environ.get("ART_PREVIEW_SIZE") or 256)


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

#: The engine that worked, once one has. A launch renders up to
#: thirty-two meshes and the answer cannot change between them, so the
#: search below runs once and every job after the first goes straight to
#: the engine that answered.
WORKING = []


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
    for engine in WORKING or ENGINES:
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
        written = fbx_to_gltf.openable(destination)
        if os.path.isfile(written) and os.path.getsize(written) > 0:
            WORKING[:] = [engine]
            return engine
        complaints.append(f"{engine}: exited cleanly and wrote no file")
    raise SystemExit(
        "could not render a preview with any engine this Blender has: "
        + "; ".join(complaints)
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


def wants():
    """Every image this mesh asked for and did not get, by file name.

    **Asked before anything is painted, because painting destroys the
    evidence.** `fbx_to_gltf.paint_with` rebinds a broken image reference
    to the declared atlas — deliberately, that is the fix for a Synty FBX
    that names nothing — and once it has, nothing can say what the mesh
    had originally asked for.

    What it asked for is worth knowing, because a pack's shared atlas is
    not the only texture in a pack. Ivy, decals, screens and holograms
    carry their own, and a mesh painted with the pack atlas instead has
    its UVs land on whatever happens to lie at those coordinates — which
    is how a sheet of ivy came to be catalogued as `a jagged, faceted
    shard of near-black material`. The resolver takes the files named
    here out of the pack and looks again, and the second look resolves
    the mesh's own reference and leaves the material alone.

    Only the file name: an FBX names its textures by the path of the tree
    it was exported from, on a machine nobody here has ever seen.
    """
    asked = []
    for material in bpy.data.materials:
        tree = getattr(material, "node_tree", None)
        if tree is None:
            continue
        for node in tree.nodes:
            if node.type != "TEX_IMAGE" or node.image is None:
                continue
            if fbx_to_gltf.usable_image(node.image):
                continue
            named = getattr(node.image, "filepath", "") or node.image.name
            leaf = os.path.basename(named.replace("\\", "/").rstrip("/"))
            if leaf and leaf not in asked:
                asked.append(leaf)
    return asked


def wire_alpha():
    """Let a cutout texture cut out.

    Synty draw ivy, cobwebs, fences and holograms as flat cards whose
    shape lives in the atlas's alpha channel. A material that reads only
    the colour renders the whole card, so a sheet of ivy comes out a
    black rectangle — which is how one was catalogued as "a jagged,
    faceted shard of near-black material", accurately describing the
    picture and uselessly describing the asset.

    Only where there is an alpha channel to read: `depth` is 32 for an
    eight-bit RGBA image and 24 for RGB, and an atlas with no alpha is
    left exactly as it was. And only the preview does this — what a built
    game does about transparency is the game's business, and the
    converter is deliberately not told about it.
    """
    for material in bpy.data.materials:
        tree = getattr(material, "node_tree", None)
        if tree is None:
            continue
        shader = next((node for node in tree.nodes if node.type == "BSDF_PRINCIPLED"), None)
        if shader is None:
            continue
        alpha = shader.inputs.get("Alpha")
        if alpha is None or alpha.is_linked:
            continue
        painted = next(
            (
                node
                for node in tree.nodes
                if node.type == "TEX_IMAGE" and fbx_to_gltf.usable_image(node.image)
            ),
            None,
        )
        if painted is None or getattr(painted.image, "depth", 0) not in (32, 64):
            continue
        channel = painted.outputs.get("Alpha")
        if channel is None:
            continue
        tree.links.new(channel, alpha)


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


def look_at(number, source, destination, texture):
    """One job: import, paint, count, measure, render, and say so.

    The block is headed by the job's own line number, because that is what
    tells a reader on the other side which mesh these facts are about
    when the job before it failed.
    """
    # Spelled so this Blender can open it: a cache path goes past what
    # Windows will answer about, and the resolver's own reads do not.
    # See `fbx_to_gltf.openable`.
    source = fbx_to_gltf.openable(source)
    if not os.path.isfile(source):
        raise SystemExit(f"no such source file: {source}")

    # A fresh, empty file per job. This is also what keeps a launch of
    # thirty-two meshes from being thirty-two meshes' worth of memory:
    # reading a file frees every datablock the last one made.
    bpy.ops.wm.read_factory_settings(use_empty=True)
    fbx_to_gltf.import_any(source)
    if not fbx_to_gltf.mesh_objects():
        raise SystemExit(f"{source} imported without producing a single mesh")
    # Before the atlas goes on, because painting is what makes a broken
    # reference unreadable. See `wants`.
    asked = wants()
    if texture:
        # Painted, and not refused when the painting reaches nothing: an
        # unpainted preview is a fact about the mesh worth recording.
        fbx_to_gltf.paint_with(texture)
    # After painting, so that the atlas a material was just given is the
    # one whose alpha is read.
    wire_alpha()

    triangles, meshes, materials, images = count()
    print(f"look {number}")
    for name in asked:
        print(f"wants {name}")
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
    # **Everything said about this job, out before the renderer speaks.**
    # Blender writes its progress from C, straight to the handle, while
    # Python's own prints sit in a buffer — so without this the last fact
    # and the first `Fra:1 ... | Saved: '...'` land on one line, and the
    # reader on the other side takes a render log for a texture's name.
    # It did: an `image` value came back carrying a Windows path, the
    # backslash in it is not a character this dialect's strings may hold,
    # and the catalogue that run wrote was refused as unreadable.
    sys.stdout.flush()

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

    render(destination)


def jobs_in(path):
    """The jobs file: one `<source>|<destination>|<texture>` per line.

    Numbered from one, blank lines kept in the count, so that the number
    in a block is the line the job is on and stays that whatever a reader
    on the other side skipped.
    """
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle.read().splitlines(), start=1):
            if not line.strip():
                continue
            fields = line.split("|")
            if len(fields) != 3:
                print(f"trouble {number} not a `source|destination|texture` line")
                continue
            yield number, fields[0], fields[1], fields[2]


def flat(text):
    """One line, because a block is read line by line and a traceback's
    worth of newlines in the middle of one would be read as facts."""
    return " ".join(str(text).split())


def main():
    if "--" not in sys.argv:
        raise SystemExit(
            "expected: blender --background --python fbx_to_preview.py -- <jobs file>"
        )
    arguments = sys.argv[sys.argv.index("--") + 1 :]
    if len(arguments) != 1:
        raise SystemExit(f"expected one jobs file, got {arguments}")

    for number, source, destination, texture in jobs_in(arguments[0]):
        try:
            look_at(number, source, destination, texture)
        # BaseException and not Exception: `SystemExit` is how everything
        # in these two scripts refuses, and one refusal is one mesh out of
        # a chunk of thirty-two rather than the end of the launch.
        except BaseException as err:  # noqa: BLE001 - one bad mesh, not the batch
            print(f"trouble {number} {flat(err)}")


if __name__ == "__main__":
    main()
