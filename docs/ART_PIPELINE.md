# The art pipeline: references in, meshes out

The game draws a whitebox. Every crate, lamp, hoop and console in it is
geometry this repository cuts in code, and that is why the repository can
be public. The plan is two graphical implementations of every object —
the whitebox that exists, and a purchased asset — and this file is about
how the second one reaches a build.

## The licence decides the architecture

Synty's terms let their meshes ship inside a **built game** and forbid
redistributing them as **source**. A public repository is source. So the
payload cannot be here: not in git, not in LFS on a public remote, not in
a release branch, not ever.

What can be here is a *reference*. Three places, and the tool that turns
the first into the third:

| | what it is | in git? |
| --- | --- | --- |
| `art/manifest.toml` | ids, packs, paths, digests, per-asset overrides | yes, and heavily commented |
| `$SYNTY_STORE` | the packs as downloaded, unzipped, on your disk | no — it is your disk |
| `art/cache/` | rebuilt and converted, addressed by content | no — gitignored |

`cargo xtask art resolve` reads the first, looks in the second, and fills
the third.

## What to install

**Nothing, for the half that finds and hashes your art.** `cargo xtask
art check` works on a bare machine.

**Blender, for the half that converts it.** Bevy loads glTF and Synty
ships FBX, so something has to translate, and `blender --background
--python` is the reliable headless converter. Get it from
<https://www.blender.org/download/>. If it is installed somewhere the
search will not find it, set `$BLENDER` to the executable.

If you would rather not install 300 MB to convert a crate, `$ART_CONVERTER`
takes any program run as `<program> <source> <destination.glb>`, which is
what FBX2glTF and its forks already are.

`tar` is used to open a `.unitypackage` and is already on Linux and
macOS. Windows 10 build 1803 and later ship bsdtar as `tar.exe`.

## Where to put packs

Pick a directory and export it:

```bash
export SYNTY_STORE="$HOME/art/synty"
```

Under it, one directory per pack, unzipped. The directory names are
**yours**: `art/manifest.toml` records the name you chose on each pack's
`dir` line, so nothing has to predict what a download unpacks to, and
rearranging your downloads folder is an edit to that line.

```
$SYNTY_STORE/
  polygon-scifi-space/      <- dir = "polygon-scifi-space"
  polygon-scifi-horror/     <- dir = "polygon-scifi-horror"
```

**Prefer each pack's "Source Files" download.** It carries the FBX and
the textures directly, so nothing has to be unpacked and nothing has to be
reconstructed. Fall back to the Unity download only for a pack that ships
no source files; see "The two claims, checked" below for what that costs.

## The five commands

`cargo xtask` is an alias defined in `.cargo/config.toml`; without it the
long spelling is `cargo run -p xtask -- art check`.

```bash
cargo xtask art check          # find every asset, hash it, report; converts nothing
cargo xtask art resolve        # check, then convert what is not cached, then write the index
cargo xtask art hash [id ...]  # print the `sha256` lines to paste into the manifest
cargo xtask art find <text>    # search the packs by file name, print the manifest lines
cargo xtask art unpack <pack>  # rebuild the trees inside one pack's .unitypackage files
```

Everything they write goes under `art/cache/`, which is gitignored and
disposable:

```
art/cache/
  unpacked/<pack>/<archive>/   trees rebuilt out of a .unitypackage
  stage/<digest>/              a mesh and its textures, as the converter saw them
  glb/<digest>.glb             the converted asset
  index.toml                   what resolved, and the overrides for each
  blender/fbx_to_gltf.py       the converter script, written out from the binary
```

Deleting the whole directory costs one `resolve`. The staging directories
are kept rather than cleaned on purpose: when a conversion comes out
wrong, that is the directory to open.

## Adding the first asset

The packs are large and their file names are not guessable, so the flow
starts with a search rather than with a store page.

```bash
$ cargo xtask art find crate
  pack = "scifi_space"
  source = "SourceFiles/FBX/SM_Prop_Crate_01.fbx"
  ...
```

Paste the two lines it prints into a new table in `art/manifest.toml`,
give the table the stable id that code will use, and add the atlas the
mesh is painted from:

```toml
[asset.crate_small]
pack = "scifi_space"
source = "SourceFiles/FBX/SM_Prop_Crate_01.fbx"
texture = "SourceFiles/Textures/PolygonSciFi_Texture_01_A.png"
```

Then record the digest and resolve:

```bash
cargo xtask art hash crate_small   # paste the `sha256 = "..."` line it prints
cargo xtask art resolve
```

## What a successful run looks like

```
$ cargo xtask art resolve
art: 1 asset over 2 packs, from /home/you/art/synty
  crate_small              source files   9f2c1d4ab077  /home/you/art/synty/polygon-scifi-space/SourceFiles/FBX/SM_Prop_Crate_01.fbx
art: converting 1 of 1 with blender /usr/bin/blender
  converted crate_small              -> glb/9f2c1d4ab077...glb
art: wrote /home/you/space-trucking/art/cache/index.toml
```

Exit status 0, and `art/cache/index.toml` holds one `[asset.<id>]` table
per line of the manifest with the converted file and the four overrides.
A second `resolve` prints the same first two lines, converts nothing, and
needs no converter at all: the cache is addressed by the digest of the
source, so "already converted" is a question about a path rather than
about a timestamp.

## What a failure looks like

This is the message the tool exists for, because on a fresh machine it is
the first thing it will ever do:

```
crate_small is not on this machine.

  pack      POLYGON Sci-Fi Space
  download  POLYGON Sci-Fi Space, the Source Files download
  declared  art/manifest.toml:47
  wanted    /home/you/art/synty/polygon-scifi-space/SourceFiles/FBX/SM_Prop_Crate_01.fbx
  found     nothing: /home/you/art/synty/polygon-scifi-space does not exist

  fix       Download "POLYGON Sci-Fi Space, the Source Files download" from your Synty
            account's downloads, unzip it, and put the result at
            /home/you/art/synty/polygon-scifi-space
            The directory is $SYNTY_STORE (/home/you/art/synty) plus
            `dir = "polygon-scifi-space"` on art/manifest.toml:12.
```

Every missing asset is reported in one run, not one per run. A pack that
is present and does not carry the path reads differently from one that
was never downloaded — the first is a search and the second is a
download, and they have nothing in common. A still-zipped archive sitting
where the unzipped pack should be is named and the fix is "unzip this".

Two more refusals worth knowing:

- **A digest that no longer matches** stops the run and prints both, with
  the command that rewrites the line. A pack updated in the store is
  otherwise a mesh that silently became a different mesh, with override
  numbers beside it measured against the old one.
- **No converter** names Blender, its download page, both override
  variables, and the fact that `check` works without any of them.

## Proving the converter, with no Synty art at all

There is a fixture pack in the repository — a cube this project wrote,
laid out exactly like a bought asset — so a Blender install can be proved
before any pack is downloaded:

```bash
SYNTY_STORE=xtask/tests/fixtures/store \
ART_MANIFEST=xtask/tests/fixtures/manifest.toml \
ART_CACHE=/tmp/art-cache \
  cargo xtask art resolve
```

Success is exit 0, one `converted unit_cube` line, and a non-empty
`.glb` under `/tmp/art-cache/glb/`. If that works, the pipeline works and
what is left is finding the right paths inside your packs.

## The overrides, and why they are here before anything reads them

Every asset table carries four per-axis numbers, and nothing consumes any
of them yet:

```toml
scale = [1.0, 1.0, 1.0]
offset = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
fill = [1.0, 1.0, 1.0]
```

They are here on the first day deliberately. Geometry in this game is a
*pure description* — `pieces::parts`, `room::seam_parts`, `room::charts`,
`poi::character_of` — that something else stamps into the world, and
[the gauntlet](GAUNTLET.md) measures the description. That is what makes
swapping in a bought mesh a swap rather than a rewrite: the description
does not change, only what gets stamped.

But a description claims a **box**, and an imported mesh occupies some
unknown fraction of it. `poi::Shape::fill` is the existing precedent, and
it cost something to learn: five stations each wrote a hoop "set into the
deck", drew a wafer floating in the middle of its frame, and could not
write any number that cured it, because the containment law was reading
the frame and not the body. The fix was a field saying what fraction of
its frame a body actually fills. The whole cost of that was **the field
not existing when the descriptions were authored**.

So `fill` here means what `Shape::fill` means, on the same axes, and it
is present before anything reads it for exactly that reason. `scale` is
the other one worth explaining: an FBX arrives at whatever unit its
exporter chose, and the Blender script deliberately does not guess — a
guess in the converter is a correction nobody can see, and a number in the
manifest is a correction on a line with a comment beside it.

## What continuous integration does, and does not

CI **never** builds the art version and never will: the payload is not in
the repository, so there is nothing for CI to resolve. It keeps building
and testing the whitebox, which is where all sixteen gauntlet families
and the determinism guards live.

What CI does run is `xtask`'s own guards, which are about the resolver's
rules and need no art: the manifest dialect, the missing-asset message,
Source-Files-before-archive, the reconstruction of a tree out of a
synthetic `.unitypackage`, content addressing, the digest check, and that
a partial resolve indexes nothing.

The cabin declares a cargo feature, `art`, defaulting off. Nothing reads
it yet. It is declared now because the seam is the expensive half: the
slice that consumes `art/cache/index.toml` has to re-enable `bevy_gltf`,
`bevy_scene` and the image decoders that were cut out of the cabin's
dependency list, which is a cold build going back from 9m59s towards the
13m41s it was before the trim. A guard holds `default = []` so that cost
is paid deliberately.

## The two claims, checked

**"A `.unitypackage` is a gzipped tar of GUID-named directories, each
containing `asset`, `asset.meta` and `pathname`."** This holds, with
three qualifications the code carries:

- The tar is *usually* gzipped and not always, so nothing here passes
  `-z`; both GNU tar and the bsdtar Windows ships sniff it themselves.
- An entry with a `pathname` and no `asset` is a **folder**, not a
  corrupt entry, and there are many of them.
- `pathname` has a trailing newline and sometimes a second line that is
  not a path. Only the first line is the answer.

And one thing the claim does not say, which matters more than any of
them: a `pathname` is a string chosen by whoever built the archive, and
this tool writes files at whatever it says. `../../.ssh/authorized_keys`
is a well-formed string. Anything with a `..`, a leading `/` or a drive
letter in it is refused rather than skipped.

**"Synty's Source Files download usually contains the raw FBX and
textures directly, which skips the unitypackage entirely."** True, and
the right default — but "usually" is load-bearing and the *reason* to
prefer it is not the one it sounds like. It is not that the unitypackage
is harder to read; reconstruction is about eighty lines. It is that a
`.unitypackage` is a Unity project fragment — prefabs and materials as
well as meshes — and rebuilding the tree recovers the meshes while
dropping the assembly. A Synty prop is often a prefab combining several
meshes against a shared material, so the archive route is not a richer
answer than the FBX. It is the same answer through more machinery.

The layout inside a Source Files download is not fixed either. That is
why `art/manifest.toml` stores a **path you pasted** rather than a layout
this tool assumes, and why `cargo xtask art find` exists.

One consequence of preferring source files is real and worth stating: a
Synty pack paints itself from one shared atlas, and an FBX names its
texture by a path relative to the tree it was exported in. Copy the mesh
out of that tree and the reference dangles. Hence `texture` in the
manifest, and hence the resolver staging the atlas beside the mesh before
the converter sees either.

## Where it lives, and what that costs

The resolver is `xtask/`, a workspace member with **no dependencies at
all** — the TOML subset, the SHA-256, the tar reconstruction and the
converter search are each a few dozen lines rather than a dependency
graph.

A member rather than a loose script, because its rules get named guards
like every other rule in this repository and `cargo test --workspace` is
what runs them. The cost is that `cargo clippy --workspace --all-targets`
and `cargo test --workspace` compile one more crate, which for a
dependency-free package is a second or two, and that CI's cache carries
nothing new.

The alternative was a script. A shell script cannot be tested the way
this repository tests things, and a Python one would put a second
language and a second dependency story in a tree that has neither.
