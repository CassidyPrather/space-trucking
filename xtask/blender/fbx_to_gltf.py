"""Turn one mesh file into one .glb, headless.

Bevy loads glTF and Synty ships FBX, and Blender is the converter that is
already on most art machines. This script is the whole of the Blender
involvement: no scene, no user preferences, no add-ons beyond the bundled
importers.

Run as::

    blender --background --factory-startup --python-exit-code 1 \
        --python fbx_to_gltf.py -- <source> <destination.glb>

`cargo xtask art resolve` writes this file out beside the cache and runs
exactly that command, so the copy in `xtask/blender/` is the readable
original rather than a thing anybody has to keep on a path.

Two things it deliberately does not do. It does not correct scale: a
Synty FBX arrives at whatever unit its exporter chose, and guessing here
would put the correction somewhere nobody can see it, while
`art/manifest.toml`'s per-asset `scale` puts it on a line with a comment.
And it does not chase textures: the resolver stages the atlas beside the
mesh before calling this, because that is the only arrangement in which a
relative texture path inside an FBX resolves anywhere but its original
tree.
"""

import os
import sys

import bpy


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


def main():
    if "--" not in sys.argv:
        raise SystemExit(
            "expected: blender --background --python fbx_to_gltf.py -- <source> <destination>"
        )
    arguments = sys.argv[sys.argv.index("--") + 1 :]
    if len(arguments) != 2:
        raise SystemExit(f"expected a source and a destination, got {arguments}")
    source, destination = arguments
    if not os.path.isfile(source):
        raise SystemExit(f"no such source file: {source}")

    bpy.ops.wm.read_factory_settings(use_empty=True)
    import_any(source)
    if not bpy.context.scene.objects:
        raise SystemExit(f"{source} imported without producing a single object")
    export_glb(destination)
    print(f"fbx_to_gltf: wrote {destination}")


main()
