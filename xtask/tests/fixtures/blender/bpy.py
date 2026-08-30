"""A Blender that is not there, big enough to drive `fbx_to_gltf.py`.

There is no Blender in continuous integration, in the container this was
written in, or on any machine that has only ever built the whitebox — and
the three defects the converter script has shipped were all defects in
its CONTROL FLOW rather than in anything Blender did. The first crate
came out grey because no atlas was ever handed over. The second came out
grey because one was, and a skip rule read a broken texture reference as
a material that knew its own texture. The third came out grey because
the rewritten rule read Blender 5.0's placeholder for the same reference
— `has_data` set, nought by nought — as pixels. None needed a mesh, a
renderer or a `.glb` to catch. All three needed somebody to run the
script against what Blender actually answers, which is why the `Image`
below models the answer and not the assumption.

So this is the smallest `bpy` the script can be run against: enough scene
graph to import into, enough node graph to paint, and a trace on standard
output of every decision that matters — what was loaded, what image was
bound to what node, what nodes were built, what was linked, what was
exported. `xtask/tests/converter_script.rs` drives it and reads the
trace.

**What it is not.** It is not Blender and cannot become Blender. It
proves what the script DECIDES, never what Blender does with the
decision: that a broken reference is repainted rather than skipped, that
a working one is left alone, that the refusal fires when nothing was
painted, and that a run with no texture is unchanged. Whether a repainted
material exports as a textured glTF is a question only the owner's
machine can answer, and `docs/ART_PIPELINE.md` says which command asks
it.

`FAKE_BLENDER_SCENE` picks what the importer finds; `FAKE_BLENDER_LOADED`
is a path on disk for the one scene that needs an image reference which
actually resolves.
"""

import os
import sys

SCENE = os.environ.get("FAKE_BLENDER_SCENE", "bare_material")
LOADED = os.environ.get("FAKE_BLENDER_LOADED", "")


def say(text):
    """One trace line. Standard error, so it cannot be mistaken for the
    script's own `aabb` line, which is parsed."""
    print(f"fake: {text}", file=sys.stderr)


class Image:
    def __init__(
        self, name, filepath, has_data=False, packed=None, source="FILE", size=None
    ):
        self.name = name
        self.filepath = filepath
        self.has_data = has_data
        self.packed_file = packed
        self.source = source
        self.library = None
        self._size = size

    @property
    def size(self):
        """What Blender answers by opening the file: the image's true
        dimensions, or nought by nought for a path that resolves nowhere
        — whatever `has_data` said. A scene may pin it, which is how the
        Blender 5.0 placeholder is modelled: a flag that says yes over a
        size that says nothing."""
        if self._size is None:
            self._size = (2048, 2048) if os.path.isfile(self.filepath) else (0, 0)
        say(f"probed {self.name} size = {self._size[0]}x{self._size[1]}")
        return self._size


class Socket:
    def __init__(self, node, name):
        self.node = node
        self.name = name


class Sockets:
    def __init__(self, node, names):
        self.by_name = {name: Socket(node, name) for name in names}

    def get(self, name):
        return self.by_name.get(name)

    def __getitem__(self, name):
        return self.by_name[name]

    def __iter__(self):
        return iter(self.by_name.values())


# What each node type offers, which is only ever the sockets the script
# reaches for by name.
PORTS = {
    "TEX_IMAGE": ((), ("Color", "Alpha")),
    "BSDF_PRINCIPLED": (("Base Color", "Metallic", "Roughness"), ("BSDF",)),
    "OUTPUT_MATERIAL": (("Surface",), ()),
}

IDNAMES = {
    "ShaderNodeTexImage": "TEX_IMAGE",
    "ShaderNodeBsdfPrincipled": "BSDF_PRINCIPLED",
    "ShaderNodeOutputMaterial": "OUTPUT_MATERIAL",
}


class Node:
    def __init__(self, tree, type, name, image=None):
        self.tree = tree
        self.type = type
        self.name = name
        inputs, outputs = PORTS[type]
        self.inputs = Sockets(self, inputs)
        self.outputs = Sockets(self, outputs)
        self._image = image

    @property
    def image(self):
        return self._image

    @image.setter
    def image(self, value):
        # The line the whole harness exists for: which node got which
        # image, and therefore whether a broken reference was rebound in
        # place or a second node was built beside it.
        say(f"{self.tree.material}/{self.name} image = {value.name if value else 'none'}")
        self._image = value


class Nodes(list):
    def __init__(self, tree):
        super().__init__()
        self.tree = tree

    def new(self, idname):
        say(f"made {idname} in {self.tree.material}")
        node = Node(self.tree, IDNAMES[idname], idname)
        self.append(node)
        return node


class Links(list):
    def __init__(self, tree):
        super().__init__()
        self.tree = tree

    def new(self, source, sink):
        say(
            f"linked {source.node.name}.{source.name}"
            f" -> {sink.node.name}.{sink.name}"
        )
        self.append((source, sink))
        return (source, sink)


class NodeTree:
    def __init__(self, material):
        self.material = material
        self.nodes = Nodes(self)
        self.links = Links(self)


class Material:
    """A material, and whether asking it for nodes gets any.

    `withholds_nodes` is the one scene that is otherwise unreachable: a
    material that says it uses nodes and hands back no tree. The script
    has always returned quietly from that, which is precisely the silence
    the refusal was added to break, so something has to be able to
    produce it.
    """

    def __init__(self, name, use_nodes=False, withholds_nodes=False):
        self.name = name
        self.node_tree = None
        self.withholds_nodes = withholds_nodes
        self._sheds = False
        self._use_nodes = False
        if use_nodes:
            self.use_nodes = True

    def shed_use_nodes(self):
        """Become a Blender 6 material: the node tree stays, and asking
        after `use_nodes` — reading it or setting it — answers the way a
        removed attribute answers. 5.0's deprecation warning names 6.0
        as when this happens, so it is a scene worth being able to
        stage before it happens to somebody's art machine."""
        self._sheds = True

    @property
    def use_nodes(self):
        if self._sheds:
            raise AttributeError("'Material' object has no attribute 'use_nodes'")
        return self._use_nodes

    @use_nodes.setter
    def use_nodes(self, value):
        if self._sheds:
            raise AttributeError("'Material' object has no attribute 'use_nodes'")
        self._use_nodes = value
        if not value or self.node_tree is not None or self.withholds_nodes:
            return
        # Blender builds the default Principled tree on the way in.
        tree = NodeTree(self.name)
        shader = Node(tree, "BSDF_PRINCIPLED", "Principled BSDF")
        output = Node(tree, "OUTPUT_MATERIAL", "Material Output")
        tree.nodes.extend([shader, output])
        tree.links.append((shader.outputs["BSDF"], output.inputs["Surface"]))
        self.node_tree = tree

    def wire_image(self, image, name="imported_diffuse"):
        """What an FBX importer leaves behind: an image node, a UV-fed
        Base Color link, and — when the file it named is not here — an
        image datablock with no pixels in it."""
        self.use_nodes = True
        tree = self.node_tree
        node = Node(tree, "TEX_IMAGE", name, image=image)
        tree.nodes.append(node)
        shader = next(one for one in tree.nodes if one.type == "BSDF_PRINCIPLED")
        tree.links.append((node.outputs["Color"], shader.inputs["Base Color"]))
        return node


class MeshData:
    def __init__(self, materials):
        self.materials = materials


class Object:
    def __init__(self, name, data, type="MESH"):
        self.name = name
        self.type = type
        self.data = data
        self.matrix_world = Matrix()
        # A half-unit cube, so `report_bounds` has something to measure.
        self.bound_box = [
            (x, y, z)
            for x in (-0.5, 0.5)
            for y in (-0.5, 0.5)
            for z in (-0.5, 0.5)
        ]


class Matrix:
    def __matmul__(self, other):
        return other


class Scene:
    def __init__(self):
        self.objects = []


class Context:
    def __init__(self):
        self.scene = Scene()


class Data:
    def __init__(self):
        self.images = ImageStore()
        self.materials = MaterialStore()


class ImageStore:
    def __init__(self):
        self.loaded = {}

    def load(self, filepath, check_existing=False):
        if check_existing and filepath in self.loaded:
            return self.loaded[filepath]
        image = Image(os.path.basename(filepath), filepath)
        self.loaded[filepath] = image
        say(f"loaded {image.name}")
        return image


class MaterialStore:
    def new(self, name):
        say(f"made material {name}")
        return Material(name)


def build_scene():
    """What the importer finds, per `FAKE_BLENDER_SCENE`."""
    if SCENE == "bare_material":
        # The original Synty case: a material assigned in Unity, so the
        # FBX carries one that names nothing at all.
        return [Object("SM_Prop_Crate_01", MeshData([Material("M_Crate")]))]
    if SCENE == "broken_reference":
        # The case that shipped the second grey crate: the FBX names a
        # texture by the path of the tree it was exported from, and that
        # path is not on this machine.
        material = Material("M_Crate")
        material.wire_image(
            Image(
                "PolygonSciFiSpace_Texture_01_A.psd",
                "C:/Synty/PolygonSciFiSpace/Textures/"
                "PolygonSciFiSpace_Texture_01_A.psd",
            )
        )
        return [Object("SM_Prop_Crate_01", MeshData([material]))]
    if SCENE == "placeholder_claiming_data":
        # The case that shipped the third grey crate: Blender 5.0's
        # importer answers the same unresolvable reference with a
        # placeholder that SAYS its pixels are in memory, and is nought
        # by nought. The path is the one SM_Prop_Crate_01.fbx really
        # carries — Synty's own Dropbox, and a City atlas at that.
        material = Material("M_Crate")
        material.wire_image(
            Image(
                "PolygonSciFiCity_Texture_01_A.psd",
                "U:/Dropbox/SyntyStudios/PolygonSciFiSpace/Working/_Textures/"
                "PolygonSciFiCity_Texture_01_A.psd",
                has_data=True,
                size=(0, 0),
            )
        )
        return [Object("SM_Prop_Crate_01", MeshData([material]))]
    if SCENE == "image_node_without_image":
        material = Material("M_Crate")
        material.wire_image(None)
        return [Object("SM_Prop_Crate_01", MeshData([material]))]
    if SCENE == "always_nodes":
        # Blender 6.0, as 5.0's deprecation warning describes it: the
        # node tree is simply there and `use_nodes` is not. The importer
        # wired an image node that holds nothing — the file was missing —
        # so the atlas must still be rebound, by a script that never
        # reads the attribute bare.
        material = Material("M_Crate")
        material.wire_image(None)
        material.shed_use_nodes()
        return [Object("SM_Prop_Crate_01", MeshData([material]))]
    if SCENE == "loaded_reference":
        # An FBX that knew where its texture was, and the texture is
        # there. This one the declaration may not touch.
        material = Material("M_Crate")
        material.wire_image(Image(os.path.basename(LOADED), LOADED))
        return [Object("SM_Prop_Crate_01", MeshData([material]))]
    if SCENE == "withheld_node_tree":
        return [
            Object(
                "SM_Prop_Crate_01",
                MeshData([Material("M_Crate", withholds_nodes=True)]),
            )
        ]
    raise SystemExit(f"fake blender: no scene called {SCENE}")


class Operators:
    """`bpy.ops.<group>`, which in Blender exists for any group name and
    raises only when the operator inside it does not."""

    def __init__(self, **operators):
        for name, call in operators.items():
            setattr(self, name, call)


class Ops:
    def __init__(self):
        self.wm = Operators(
            read_factory_settings=read_factory_settings,
            fbx_import=fbx_import,
        )
        self.preferences = Operators(addon_enable=addon_enable)
        self.export_scene = Operators(gltf=export_gltf)

    def __getattr__(self, group):
        # Every other group exists and is empty, which is what Blender
        # does — `bpy.ops.import_scene` is a module whether or not the
        # add-on providing `fbx` was ever installed.
        return Operators()


def read_factory_settings(use_empty=False):
    context.scene.objects.clear()


def fbx_import(filepath=None):
    say("imported via wm.fbx_import")
    context.scene.objects.extend(build_scene())


def addon_enable(module=None):
    # Blender 5.0 removed `io_scene_fbx`; the script must survive that.
    raise RuntimeError(f"add-on not found: {module}")


def export_gltf(filepath=None, **rest):
    say(f"exported {filepath}")
    with open(filepath, "wb") as file:
        file.write(b"glTF")


class Path:
    @staticmethod
    def abspath(filepath, library=None):
        return (
            os.path.join(os.getcwd(), filepath[2:])
            if filepath.startswith("//")
            else filepath
        )


context = Context()
data = Data()
ops = Ops()
path = Path()
