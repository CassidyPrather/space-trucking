"""Turn one mesh file into one .glb, headless.

Bevy loads glTF and Synty ships FBX, and Blender is the converter that is
already on most art machines. This script is the whole of the Blender
involvement: no scene, no user preferences, no add-ons beyond the bundled
importers.

Run as::

    blender --background --factory-startup --python-exit-code 1 \
        --python fbx_to_gltf.py -- <source> <destination.glb> [texture]

`cargo xtask art resolve` writes this file out beside the cache and runs
exactly that command, so the copy in `xtask/blender/` is the readable
original rather than a thing anybody has to keep on a path.

It also measures what it wrote and prints one `aabb` line — see
`report_bounds`. That is the fact the manifest's `fill` promise is
checked against, and it is here rather than in the resolver because
this is the only program in the pipeline that can see a mesh.

The third argument is the atlas the manifest DECLARED, and `paint_with`
is what happens to it. This used not to exist, and the reason it does now
is the first real Synty mesh the pipeline ever converted: it came out
colourless. Synty assign their materials in Unity, through the
`.unitypackage`'s `.mat` files, so the FBX commonly carries a bare
material that names no image at all — and a file that names nothing
resolves nothing, however carefully the resolver staged the atlas beside
it. So the staged copies stay, because an FBX that DOES name its own
texture still resolves against them and still wins, and the declaration
fills the silence underneath: every material with no image of its own
gets the declared atlas as its Base Color, and a mesh with no material at
all gets one made for it.

One thing it still deliberately does not do: correct scale. A Synty FBX
arrives at whatever unit its exporter chose, and guessing here would put
the correction somewhere nobody can see it, while `art/manifest.toml`'s
per-asset `scale` puts it on a line with a comment.
"""

import os
import sys

import bpy
import mathutils


def import_any(path):
    """Import `path` into the empty scene, whatever kind of file it is.

    The FBX importer moved. Blender through 4.x has it as the Python
    add-on operator `import_scene.fbx`; the rewritten importer is
    `wm.fbx_import`. Both are tried because a script that only knows one
    of them breaks on somebody's machine and not on ours.
    """
    extension = os.path.splitext(path)[1].lower()
    attempts = {
        ".fbx": ["wm.fbx_import", "import_scene.fbx"],
        ".obj": ["wm.obj_import", "import_scene.obj"],
        ".dae": ["wm.collada_import"],
        ".gltf": ["import_scene.gltf"],
        ".glb": ["import_scene.gltf"],
        ".blend": [],
    }.get(extension)
    if attempts is None:
        raise SystemExit(f"cannot import {extension or 'a file with no extension'}: {path}")

    if extension == ".fbx":
        try:
            bpy.ops.preferences.addon_enable(module="io_scene_fbx")
        except Exception:  # noqa: BLE001 - a bundled add-on that is already on
            pass

    complaints = []
    for name in attempts:
        group, operator = name.split(".")
        candidate = getattr(getattr(bpy.ops, group), operator, None)
        if candidate is None:
            complaints.append(f"{name} does not exist in this Blender")
            continue
        try:
            candidate(filepath=path)
            return
        except Exception as err:  # noqa: BLE001 - report every route that failed
            complaints.append(f"{name} said: {err}")
    raise SystemExit(f"could not import {path}\n  " + "\n  ".join(complaints))


def paint_with(path):
    """Give every material that names no image of its own this atlas.

    The declaration filling a silence, never overruling a statement. A
    material that already names an image is left exactly as the importer
    built it — that FBX knew where its texture was, and the manifest's
    `texture` line is a fallback for the ones that do not, not a
    correction to the ones that do.

    Small and defensive on purpose: it runs on somebody else's Blender,
    against a file nothing in this repository has seen, and the worst
    outcome available is a traceback where a grey crate would have done.
    """
    if not os.path.isfile(path):
        # The resolver stages this file beside the mesh before running
        # this script, so its absence here is a fault in the resolver and
        # not something to work around. Say which file, the way every
        # other refusal in this pipeline does.
        raise SystemExit(
            f"the declared texture is not where the resolver staged it: {path}\n"
            "  `cargo xtask art resolve` copies it beside the mesh before running this,\n"
            "  so a missing one is a fault in the resolver rather than in the pack."
        )
    image = bpy.data.images.load(path, check_existing=True)
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        materials = getattr(obj.data, "materials", None)
        if materials is None:
            continue
        if not len(materials):
            # A mesh with no material at all is the emptiest silence
            # there is, and it exports as an untextured default.
            materials.append(bpy.data.materials.new(name="declared_atlas"))
        for material in materials:
            if material is not None:
                paint_material(material, image)


def paint_material(material, image):
    """Plug `image` into this material's Base Color, if nothing is there."""
    if not material.use_nodes:
        # Setting this builds the default Principled tree, which is what
        # the branch below then wires the image into.
        material.use_nodes = True
    tree = material.node_tree
    if tree is None:
        return
    if any(node.type == "TEX_IMAGE" and node.image is not None for node in tree.nodes):
        return  # it named its own; the declaration has nothing to say here
    shader = next((node for node in tree.nodes if node.type == "BSDF_PRINCIPLED"), None)
    if shader is None:
        shader = tree.nodes.new("ShaderNodeBsdfPrincipled")
        output = next(
            (node for node in tree.nodes if node.type == "OUTPUT_MATERIAL"), None
        ) or tree.nodes.new("ShaderNodeOutputMaterial")
        tree.links.new(shader.outputs["BSDF"], output.inputs["Surface"])
    base_color = shader.inputs.get("Base Color")
    if base_color is None:
        return
    texture = tree.nodes.new("ShaderNodeTexImage")
    texture.image = image
    tree.links.new(texture.outputs["Color"], base_color)


def export_glb(path):
    """Write the scene as one binary glTF.

    The keyword arguments have come and gone across Blender releases, so
    the full set is tried and then the minimum, rather than pinning a
    version this repository has no way to check.
    """
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    try:
        bpy.ops.export_scene.gltf(
            filepath=path,
            export_format="GLB",
            export_apply=True,
            export_yup=True,
        )
    except TypeError:
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB")


def report_bounds():
    """Print the tight box round everything in the scene, in the GLB's axes.

    One line, `aabb minx miny minz maxx maxy maxz`. This is the FACT half
    of the manifest's `fill` declaration: a description claims a box and a
    purchased mesh occupies some part of it, and until something measured
    the mesh, `fill` was the only statement in the system about which
    part. `cargo xtask art resolve` reads this line, writes it into the
    index, and refuses a `fill` that disagrees with it.

    The corners of every object's own bounding box, carried through that
    object's world matrix, which is tight for an axis-aligned body and a
    hair loose for a rotated one — the same bound the game's own
    `pieces::drawn_box` takes, for the same reason.

    **In the exported file's axes and not Blender's.** `export_yup` turns
    Blender's Z-up into glTF's Y-up on the way out, so a box measured in
    the scene and a box measured in the file disagree about two of three
    axes — and a number that names the wrong axis is worse than no number,
    because it looks like a measurement.
    """
    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        for corner in obj.bound_box:
            point = obj.matrix_world @ mathutils.Vector(corner)
            # Z-up to Y-up: (x, y, z) becomes (x, z, -y).
            for axis, value in enumerate((point.x, point.z, -point.y)):
                lo[axis] = min(lo[axis], value)
                hi[axis] = max(hi[axis], value)
    if lo[0] > hi[0]:
        # Nothing with a mesh in it. The export already refused an empty
        # scene, so this is a scene of lights and empties, and the honest
        # answer is to say nothing rather than to print an infinity.
        return
    print("aabb " + " ".join(f"{value:.6f}" for value in lo + hi))


def main():
    if "--" not in sys.argv:
        raise SystemExit(
            "expected: blender --background --python fbx_to_gltf.py -- "
            "<source> <destination> [texture]"
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
    import_any(source)
    if not bpy.context.scene.objects:
        raise SystemExit(f"{source} imported without producing a single object")
    if texture is not None:
        paint_with(texture)
    export_glb(destination)
    report_bounds()
    print(f"fbx_to_gltf: wrote {destination}")


main()
