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

**A reference is not knowledge.** The second colourless crate taught the
rest of it. A Synty FBX often does name a texture — by the path of the
tree it was exported from, sometimes a `.psd` nobody shipped — and
Blender answers a reference it cannot resolve with a placeholder image
datablock: a name, a filepath pointing nowhere, no pixels. A skip rule
that asks "is there an image here?" reads that placeholder as a material
that knows its own texture, leaves it alone, and exports grey. So the
question `usable_image` asks is whether an image could be LOADED, and a
material whose every image reference is broken counts as silence — the
atlas is rebound onto the importer's own node, keeping the wiring the
importer built.

**And a flag is not pixels.** The third colourless crate was the second
one a level further in. Blender 5.0's importer answers the same
unresolvable reference with a placeholder that reports `has_data` — and
a size of nought by nought — so a rule that asked whether the pixels
were in memory took the placeholder's word for it, left the material
alone, and the refusal below, asking the same question, agreed.
`usable_image` now asks for the image's size, which Blender can only
answer by opening the file, and nought by nought is silence.

**And silence is refused rather than shipped.** When a texture is handed
over, `refuse_unless_painted` checks before exporting that at least one
material in the scene bears an image that can be loaded, and exits
nonzero naming the atlas if none does. That is part of the contract, not
a nicety: every colourless crate was a conversion that succeeded, printed
its measurement, wrote a plausible file, and was only found out by
somebody looking at a grey box in the cabin. A conversion handed a
texture and painting nothing is now a refusal — one that is exactly as
good as the question `usable_image` asks, which is where the third crate
got past it.

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
    `wm.fbx_import`, and by 5.0 the `io_scene_fbx` add-on that provided
    the old one is gone entirely. Both are tried, newest first, because a
    script that only knows one of them breaks on somebody's machine and
    not on ours — and nothing here may assume which one answered.
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


def mesh_objects():
    """Every mesh in the scene: what gets painted, checked and measured.

    One definition, used by all three, so the set of materials the atlas
    is offered to and the set the refusal below counts can never drift
    apart.
    """
    return [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]


def usable_image(image):
    """Whether this image datablock is one that anything could draw.

    The whole of the second colourless crate is in this function. An
    image node with a datablock hanging off it is not a material that
    knows where its texture is: Synty FBX files name their textures by
    the paths of the tree they were exported from — commonly a `.psd`
    that was never part of the pack — and Blender answers a reference it
    cannot resolve with a placeholder. The placeholder is a real
    datablock with a real name, so `node.image is not None` is true of
    it, which is exactly what the old skip rule asked and exactly why a
    crate with a broken reference was left grey.

    And `has_data` is true of it too, on Blender 5.0, which is the whole
    of the third colourless crate. The rule that replaced the first one
    asked whether the pixels were already decoded in memory, and the new
    importer's placeholder says they are — with a size of nought by
    nought. A flag is not pixels. So the question is now the one only
    the file can answer: how big is the image? Reading `size` makes
    Blender open the file, a reference that resolves nowhere comes back
    0×0, and 0×0 is silence whatever the flag beside it says. A packed
    or generated image is still a yes, since both carry their pixels
    with them by construction.

    Read defensively, because this runs on a Blender version nothing in
    this repository has seen and a missing attribute must mean "no"
    rather than a traceback.
    """
    if image is None:
        return False
    if getattr(image, "packed_file", None) is not None:
        return True
    if getattr(image, "source", "") == "GENERATED":
        return True
    try:
        width, height = int(image.size[0]), int(image.size[1])
    except Exception:  # noqa: BLE001 - a size nobody can read is no pixels
        return False
    return width > 0 and height > 0


def paint_with(path):
    """Give every material with no usable image of its own this atlas.

    The declaration filling a silence, never overruling a statement — and
    a broken reference is silence. A material that names an image which
    LOADS is left exactly as the importer built it, because that FBX
    knew where its texture was and the manifest's `texture` line is a
    fallback for the ones that do not, not a correction to the ones that
    do. A material whose every image reference is a placeholder knows
    nothing, whatever it appears to name.

    Small and defensive on purpose: it runs on somebody else's Blender,
    against a file nothing in this repository has seen, and the worst
    outcome available is a traceback where a grey crate would have done.

    Hands back the atlas datablock, which is what `refuse_unless_painted`
    then looks for.
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
    for obj in mesh_objects():
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
    return image


def paint_material(material, image):
    """Plug `image` into this material, unless it already has one that loads.

    Three cases, and the middle one is the defect this was rewritten for.

    A material with a usable image is a statement and is left alone. A
    material with image nodes and not one usable image among them was
    wired up by the importer against files that are not on this machine:
    the nodes, their links and the UV coordinates feeding them are all
    exactly right and only the pixels are missing, so the atlas is
    rebound onto those nodes rather than a second node being built beside
    them — two nodes claiming one Base Color is a worse answer than a
    normal map wearing a colour atlas. A material with no image nodes at
    all is the original bare-Synty case and gets one made for it.

    The boundary is deliberate: one loadable image anywhere in a material
    is enough to leave the whole of it alone, even if some other slot is
    broken. Overruling half of what a file says is how a fallback turns
    into a correction, and this is a fallback.
    """
    # Read through getattr: Blender 5.0 deprecates `Material.use_nodes`
    # and expects 6.0 to remove it, and the removal will mean what a
    # missing attribute means here — every material simply has its tree.
    if not getattr(material, "use_nodes", True):
        # Setting this builds the default Principled tree, which is what
        # the branch below then wires the image into.
        material.use_nodes = True
    tree = material.node_tree
    if tree is None:
        return
    textures = [node for node in tree.nodes if node.type == "TEX_IMAGE"]
    if any(usable_image(node.image) for node in textures):
        return  # it named its own, and the name resolved
    if textures:
        for node in textures:
            node.image = image
        return
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


def refuse_unless_painted(source, texture, atlas):
    """Refuse to export a scene that was handed an atlas and used it nowhere.

    Part of the contract and not a nicety. Every colourless crate this
    pipeline has shipped was a conversion that SUCCEEDED: it exited
    zero, printed its measurement, wrote a file of an entirely plausible
    size, and was found out later by somebody looking at a grey box in
    the cabin. Nothing between here and that moment can tell the
    difference, because a `.glb` with no image in it is a perfectly
    valid `.glb`.

    So the one moment that can tell is this one, and it says so out loud.
    A texture was declared, staged, and handed over; if not one material
    in the scene about to be exported carries an image, the painting did
    not happen and the file is not written.

    It is only as good as the question it asks. The third grey crate
    walked straight through it, because `usable_image` took a
    placeholder's `has_data` at its word and this asked `usable_image`;
    the fix for that lives in that function and not in a second check
    here, so that the two can never disagree about what counts.

    An image counts if it IS the atlas — `paint_with` loaded that one
    itself, from a path it had already checked was a file, so it is
    loadable by construction and no spelling of a filepath can argue
    otherwise — or if it is any other image that loads, which is the
    material that named its own texture and was rightly left alone.
    """
    painted = 0
    materials = []
    for obj in mesh_objects():
        for material in getattr(obj.data, "materials", None) or []:
            if material is None or any(material is one for one in materials):
                continue
            materials.append(material)
            tree = (
                getattr(material, "node_tree", None)
                # getattr for the reason paint_material reads it so: a
                # Blender that removed the flag is one where every
                # material has its tree.
                if getattr(material, "use_nodes", True)
                else None
            )
            if tree is not None and any(
                node.type == "TEX_IMAGE"
                and (node.image is atlas or usable_image(node.image))
                for node in tree.nodes
            ):
                painted += 1
    if painted:
        return
    raise SystemExit(
        f"the declared atlas reached no material: {texture}\n"
        f"  {source} exported {len(materials)} material(s), and not one of them bears\n"
        "  an image that can be loaded — so this .glb would be grey wherever the\n"
        "  atlas should be. A conversion handed a texture and painting nothing is\n"
        "  the defect the texture argument exists to fix, and it is silent, so it\n"
        "  is refused here rather than discovered in the cabin."
    )


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
    for obj in mesh_objects():
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
        refuse_unless_painted(source, texture, paint_with(texture))
    export_glb(destination)
    report_bounds()
    print(f"fbx_to_gltf: wrote {destination}")


main()
