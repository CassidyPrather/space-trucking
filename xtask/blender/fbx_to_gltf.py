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

**And a colour is not a light.** The fourth defect was a lamp rather
than a crate, and it was not grey: it glowed all over. A Synty FBX can
name TWO textures it cannot find — the colour atlas and, on a fitting
the pack lights, the emissive atlas beside it, wired by the importer into
the material's emission. The rebind above rebound the colour atlas onto
every placeholder node alike, so the lamp's emission was fed its own
albedo and the whole body burned at the brightness of its paint. An
image node feeding emission is never a place for the colour atlas: where
the manifest declares an `emissive`, `light_with` rebinds THAT onto the
importer's node; where it declares none, `unlight_strays` unwires the
reference and says so, and the mesh is converted unlit rather than lit
with the wrong picture.

One thing it still deliberately does not do: correct scale. A Synty FBX
arrives at whatever unit its exporter chose, and guessing here would put
the correction somewhere nobody can see it, while `art/manifest.toml`'s
per-asset `scale` puts it on a line with a comment.
"""

import os
import sys

import bpy
import mathutils


def openable(path):
    """The same file, spelled so that this Blender can open it.

    Windows stops at 260 characters, and a path into the art cache is a
    cache root, plus a pack, plus the name of the archive the file came
    out of, plus the tree inside that archive — which goes past 260 on
    ordinary packs with ordinary names. `POLYGON - Alpine Mountain`'s ice
    axe does it at 268.

    The half that makes it a trap: Rust's standard library reaches those
    files without being asked, so the resolver finds the mesh, hashes it,
    writes it into a jobs file and hands it over — and then Blender's own
    Python answers `no such source file` about a file that is right
    there. It was a quarter of a real sweep, and every one of them looked
    like a broken pack rather than a spelling.

    The `\\\\?\\` prefix is what turns a Windows path into one the long-path
    APIs take. It is applied only when it is needed and only when it
    resolves, so nothing else in the pipeline has to know about it.
    """
    if os.name != "nt" or not path or path.startswith("\\\\?\\") or len(path) < 240:
        return path
    extended = "\\\\?\\" + os.path.abspath(path)
    return extended if os.path.exists(extended) else path


def import_any(path):
    """Import `path` into the empty scene, whatever kind of file it is.

    The FBX importer moved. Blender through 4.x has it as the Python
    add-on operator `import_scene.fbx`; the rewritten importer is
    `wm.fbx_import`, and by 5.0 the `io_scene_fbx` add-on that provided
    the old one is gone entirely. Both are tried, newest first, because a
    script that only knows one of them breaks on somebody's machine and
    not on ours — and nothing here may assume which one answered.
    """
    path = openable(path)
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
    path = openable(path)
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


def principled(tree):
    """This tree's Principled BSDF, or None for a material built some other way."""
    return next((node for node in tree.nodes if node.type == "BSDF_PRINCIPLED"), None)


def emission_socket(shader):
    """The shader's emission colour input, under whichever name this Blender gives it."""
    return shader.inputs.get("Emission Color") or shader.inputs.get("Emission")


def feeders(tree, socket):
    """The image nodes wired into `socket`, each with the link that wires it.

    What the importer leaves behind when an FBX names an emissive: an
    Image Texture node linked into emission, holding a placeholder if the
    file was not there. Empty for a socket nobody wired, and for `None`.
    """
    if socket is None:
        return []
    return [
        (link.from_node, link)
        for link in tree.links
        if link.to_socket == socket and link.from_node.type == "TEX_IMAGE"
    ]


def light_with(path):
    """Wire this emissive atlas into every material in the scene.

    The counterpart of `paint_with`, and deliberately NOT the same rule.
    An atlas fills a silence and never overrules a statement, because a
    Synty FBX often does know where its colour texture is. **No Synty FBX
    has ever known where its emissive is** — emission is assigned in
    Unity, in the `.mat`, which the FBX does not carry — so there is no
    statement here to defer to and the declaration is the whole of it.
    Every material gets it, on the emission slot alone; nothing about
    Base Color is touched. Where the importer already wired an image node
    into emission — an FBX that named an emissive by a path that is not
    on this machine — the atlas is rebound onto that node, for the reason
    `paint_material` rebinds the colour one: the wiring is what the file
    asked for and only the pixels are missing, and a second node beside
    it would be two claims on one socket.

    The image is the pack's own emissive atlas, laid out on the same
    swatch grid as the colour one: black everywhere the mesh is not lit,
    and the lamp's colour where it is. So this lights exactly the faces
    Synty meant to light, and a mesh that is mostly not a lamp stays
    mostly dark.

    Strength is left at one. glTF carries `emissiveTexture` times
    `emissiveFactor` and Bevy reads both into `StandardMaterial`, so what
    reaches the screen is the atlas at its own brightness — the pack's
    judgement about how hard its own lamps burn, which is the same
    deference the colour atlas gets.
    """
    path = openable(path)
    if not os.path.isfile(path):
        raise SystemExit(
            f"the declared emissive is not where the resolver staged it: {path}\n"
            "  `cargo xtask art resolve` copies it beside the mesh before running this,\n"
            "  so a missing one is a fault in the resolver rather than in the pack."
        )
    image = bpy.data.images.load(path, check_existing=True)
    # Emission is light, not albedo: reading it through the colour
    # pipeline would gamma-correct a quantity that is already linear.
    try:
        image.colorspace_settings.name = "Non-Color"
    except Exception:  # noqa: BLE001 - an old Blender may spell it differently
        pass
    for obj in mesh_objects():
        materials = getattr(obj.data, "materials", None)
        if materials is None:
            continue
        for material in materials:
            if material is not None:
                light_material(material, image)
    return image


def light_material(material, image):
    """Plug `image` into this material's emission, building what it needs.

    Same defensive shape as `paint_material`: it runs on somebody else's
    Blender against a file nothing here has seen, and the worst outcome
    available should be a mesh that does not glow rather than a
    traceback. A material with no Principled shader gets no emission —
    there is nowhere to put one that the glTF exporter would read.
    """
    if not getattr(material, "use_nodes", True):
        material.use_nodes = True
    tree = material.node_tree
    if tree is None:
        return
    shader = principled(tree)
    if shader is None:
        return
    colour = emission_socket(shader)
    strength = shader.inputs.get("Emission Strength")
    if colour is None:
        return
    wired = feeders(tree, colour)
    if wired:
        # The importer's own node, rebound — unless it holds an image that
        # actually loads, which is an FBX that knew where its emissive
        # was and is left alone like a colour reference that resolves.
        for node, _ in wired:
            if not usable_image(node.image):
                node.image = image
    else:
        node = tree.nodes.new("ShaderNodeTexImage")
        node.image = image
        tree.links.new(node.outputs["Color"], colour)
    if strength is not None and not strength.is_linked:
        # Blender 4.x defaults this to 0.0 on an untouched Principled
        # node, and a texture into a colour multiplied by nothing is a
        # conversion that reports success and exports a dark mesh —
        # which is the exact failure mode every colourless crate in this
        # pipeline's history has taken.
        strength.default_value = 1.0


def unlight_strays(source):
    """Unwire every emission image reference that resolves nowhere.

    Run only when the manifest declared no `emissive`, which is the case
    `light_with` does not reach. An FBX that names an emissive atlas by
    the path of the tree it was exported from arrives with an image node
    holding a placeholder wired into emission and an Emission Strength
    the importer set to one — and left like that, what the exporter
    writes is a material lit by nothing in particular, or by whatever
    somebody rebound onto the placeholder. The floor lamp this was
    written for burned at the brightness of its own paint, all over.

    A reference that resolves nowhere is silence (`usable_image`), and
    silence on the emission slot means unlit: the link comes off, the
    strength goes to nought, and a sentence on standard error names the
    file the FBX asked for, because the pack almost certainly ships it
    and one `emissive` line in the manifest is the whole of the cure. An
    emission reference that LOADS is a statement and is left alone.
    """
    strays = []
    for obj in mesh_objects():
        for material in getattr(obj.data, "materials", None) or []:
            if material is None:
                continue
            tree = (
                getattr(material, "node_tree", None)
                if getattr(material, "use_nodes", True)
                else None
            )
            if tree is None:
                continue
            shader = principled(tree)
            if shader is None:
                continue
            colour = emission_socket(shader)
            strength = shader.inputs.get("Emission Strength")
            unwired = 0
            for node, link in feeders(tree, colour):
                if usable_image(node.image):
                    continue
                named = getattr(node.image, "filepath", "") or getattr(node.image, "name", "")
                strays.append(named or "an image node holding nothing")
                tree.links.remove(link)
                unwired += 1
            if unwired and strength is not None and not strength.is_linked:
                # Blender 4.x defaults the colour to white, so a strength
                # the importer set to one would light the whole mesh
                # white the moment the texture came off it.
                strength.default_value = 0.0
    if strays:
        names = ", ".join(sorted(set(strays)))
        print(
            f"fbx_to_gltf: {source} names an emissive it cannot find, and no `emissive`\n"
            f"  line declares one, so it is converted unlit: {names}\n"
            "  The pack's own is usually under Textures/Emissive/, on the same swatch\n"
            "  grid as the colour atlas; an `emissive` line in art/manifest.toml lights it.",
            file=sys.stderr,
        )


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

    And a colour is not a light. An image node the importer wired into
    EMISSION is a broken emissive reference, not a broken colour one,
    and the fourth defect was the colour atlas rebound onto it: a lamp
    lit by its own paint, all over. Those nodes are `light_with`'s and
    `unlight_strays`'s, and this never touches them.
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
    shader = principled(tree)
    lit = [node for node, _ in feeders(tree, emission_socket(shader))] if shader else []
    # Equality and not identity: Blender hands back a fresh wrapper for
    # the same node on every access, so `is` never matches and the
    # emission node is swept into the colour rebinding after all.
    colour_nodes = [node for node in textures if not any(node == one for one in lit)]
    if colour_nodes:
        for node in colour_nodes:
            node.image = image
        return
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


def bounds():
    """The tight box round everything in the scene, in the GLB's axes.

    Hands back `(lo, hi)`, or `None` for a scene with no mesh in it. The
    corners of every object's own bounding box, carried through that
    object's world matrix, which is tight for an axis-aligned body and a
    hair loose for a rotated one — the same bound the game's own
    `pieces::drawn_box` takes, for the same reason.

    **In the exported file's axes and not Blender's.** `export_yup` turns
    Blender's Z-up into glTF's Y-up on the way out, so a box measured in
    the scene and a box measured in the file disagree about two of three
    axes — and a number that names the wrong axis is worse than no number,
    because it looks like a measurement.

    Split from `report_bounds` for `fbx_to_preview.py`, which measures the
    same box for the catalogue and must measure it the same way: two
    readings of one mesh that disagree about an axis is a `fill` refusal
    on one side of the pipeline and a size in the catalogue on the other.
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
        return None
    return lo, hi


def report_bounds():
    """Print the tight box round everything in the scene, in the GLB's axes.

    One line, `aabb minx miny minz maxx maxy maxz`. This is the FACT half
    of the manifest's `fill` declaration: a description claims a box and a
    purchased mesh occupies some part of it, and until something measured
    the mesh, `fill` was the only statement in the system about which
    part. `cargo xtask art resolve` reads this line, writes it into the
    index, and refuses a `fill` that disagrees with it.
    """
    measured = bounds()
    if measured is None:
        # Nothing with a mesh in it. The export already refused an empty
        # scene, so this is a scene of lights and empties, and the honest
        # answer is to say nothing rather than to print an infinity.
        return
    lo, hi = measured
    print("aabb " + " ".join(f"{value:.6f}" for value in lo + hi))


def main():
    if "--" not in sys.argv:
        raise SystemExit(
            "expected: blender --background --python fbx_to_gltf.py -- "
            "<source> <destination> [texture [emissive]]"
        )
    arguments = sys.argv[sys.argv.index("--") + 1 :]
    if len(arguments) not in (2, 3, 4):
        raise SystemExit(
            "expected a source, a destination, an optional texture and an optional "
            f"emissive, got {arguments}"
        )
    source, destination = arguments[0], arguments[1]
    texture = arguments[2] if len(arguments) >= 3 else None
    # Positional and behind the atlas, which is why the resolver refuses
    # a manifest that declares one without the other: a lone emissive
    # would arrive here as `texture` and be painted on as Base Color.
    emissive = arguments[3] if len(arguments) == 4 else None
    if not os.path.isfile(source):
        raise SystemExit(f"no such source file: {source}")

    bpy.ops.wm.read_factory_settings(use_empty=True)
    import_any(source)
    if not bpy.context.scene.objects:
        raise SystemExit(f"{source} imported without producing a single object")
    if texture is not None:
        refuse_unless_painted(source, texture, paint_with(texture))
    if emissive is not None:
        light_with(emissive)
    else:
        unlight_strays(source)
    export_glb(destination)
    report_bounds()
    print(f"fbx_to_gltf: wrote {destination}")


# Guarded, so that `fbx_to_preview.py` beside this file can import it and
# borrow `import_any`, `usable_image`, `paint_with` and `bounds` instead
# of keeping a second copy of them. Blender runs a `--python` script as
# `__main__`, so the conversion below still happens exactly as it did.
if __name__ == "__main__":
    main()
