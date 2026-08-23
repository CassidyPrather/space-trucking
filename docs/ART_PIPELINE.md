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
| `$SYNTY_STORE` | the packs exactly as downloaded, on your disk | no — it is your disk |
| `art/cache/` | the few files a manifest named, converted, addressed by content | no — gitignored |

`cargo xtask art resolve` reads the first, looks in the second, and fills
the third.

## What to install

**Almost nothing, for the half that finds and hashes your art.** `cargo
xtask art check` needs one thing, and on macOS and Windows you already
have it.

**Something that opens a zip, because a pack arrives as one.** `tar` is
enough on macOS and on Windows 10 build 1803 and later, whose `tar` is
bsdtar and reads zip as well as tar. Linux ships GNU tar, which reads tar
and not zip, so Linux also wants `unzip` — `apt install unzip`, `dnf
install unzip`. `tar` alone is enough for a `.unitypackage`, which is a
tar. If neither is there the tool says which one to install rather than
reporting your art missing.

**Blender, for the half that converts it.** Bevy loads glTF and Synty
ships FBX, so something has to translate, and `blender --background
--python` is the reliable headless converter. Get it from
<https://www.blender.org/download/>. If it is installed somewhere the
search will not find it, set `$BLENDER` to the executable.

If you would rather not install 300 MB to convert a crate, `$ART_CONVERTER`
takes any program run as `<program> <source> <destination.glb>`, which is
what FBX2glTF and its forks already are.

## Where to put packs

Pick a directory and export it:

```bash
export SYNTY_STORE="$HOME/art/synty"
```

**On Windows, a path in one of these variables has to be one Windows can
open.** `cargo` and the resolver it builds are native programs, and a
native program is handed the value verbatim: it does not know Git Bash's
`$HOME`, its `/c/...` or its `/tmp`. That is true of every variable here
— `$SYNTY_STORE`, `$ART_MANIFEST`, `$ART_CACHE`, `$BLENDER`,
`$ART_CONVERTER` — and forward slashes are fine in a Windows path, so
only the `/c/` prefix has to go:

```bash
export SYNTY_STORE="C:/Art/Synty"
```

A relative path sidesteps the question entirely, which is what the
examples further down use.

Under it, one directory per pack. **Do not unzip anything.** Put the
download in exactly as it arrived — the `.unitypackage`, the icon, the
zip of raw assets — and leave it there:

```
$SYNTY_STORE/
  POLYGON - Sci-Fi Space Pack/       <- dir = "POLYGON - Sci-Fi Space Pack"
    icon.png
    POLYGON Sci-Fi Space.unitypackage
    POLYGON Sci-Fi Space.zip         <- the raw assets, still zipped
  POLYGON - Sci-Fi Horror/           <- dir = "POLYGON - Sci-Fi Horror"
    ...
```

The directory names are **yours**: `art/manifest.toml` records the name
you chose on each pack's `dir` line, so nothing has to predict what a
download unpacks to, and rearranging your downloads folder is an edit to
that line. Spaces, capitals and punctuation in the name are ordinary —
the name the store gave the pack is a fine name for the directory, and
the two above are the real ones this repository's manifest carries. The
file names *inside* a pack directory are recorded nowhere: the archives
are found by their extension, so a download keeps whatever it arrived as.

The tool reads inside the archives and takes out only the files the
manifest names. A manifest naming fifty props out of a five-thousand-file
pack costs fifty files on disk, not an unzipped pack, and that is the
whole reason the store is left alone.

A tree you *have* unzipped still works, and answers first. So unzipping a
pack is never wrong, only unnecessary.

## What answers first

A pack can carry the same mesh in three places, and the resolver looks in
this order:

1. **A loose Source Files tree** in the pack directory, if you unzipped
   one. Nothing to open and nothing to reconstruct.
2. **The pack's raw archive.** The same Source Files download, still
   zipped. The file named is taken out into the cache and the archive is
   left alone.
3. **A `.unitypackage`**, rebuilt into the cache. This needs a `unity =
   "Assets/..."` line in the manifest, and it is for the packs that ship
   no source download at all.

The first two are the same download and rank together. The third ranks
last, and not because it is slower: a `.unitypackage` is a Unity project
fragment — prefabs and materials as well as meshes — and rebuilding the
tree recovers the meshes while dropping the assembly. A Synty prop is
often a prefab combining several meshes against a shared material, so the
archive route is not a richer answer than the FBX. It is the same answer
through more machinery.

**Prefer each pack's "Source Files" download** for that reason, zipped or
not.

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
  unpacked/<pack>/<archive file name>/
                               what came out of one of the pack's archives: the
                               files a manifest named, out of a zip, or a whole
                               tree rebuilt out of a .unitypackage
  stage/<digest>/              a mesh and its textures, as the converter saw them
  glb/<digest>.glb             the converted asset
  index.toml                   what resolved, and the overrides for each
  blender/fbx_to_gltf.py       the converter script, written out from the binary
  fixtures/                    the same layout again, written by the fixture run
                               in "Proving the pipeline" below
```

Deleting the whole directory costs one `resolve`. The staging directories
are kept rather than cleaned on purpose: when a conversion comes out
wrong, that is the directory to open.

## Adding the first asset

The packs are large and their file names are not guessable, so the flow
starts with a search rather than with a store page. `find` reads the
names inside the archives too, which is the only way to search a store
nothing has been unzipped in — and it reads them without extracting
anything, because a zip keeps a table of its members at the end of the
file.

```bash
$ cargo xtask art find crate
art: `crate` matches 452 files, in 37 pack directories under /home/you/art/synty

In the 2 packs art/manifest.toml declares:

pack = "scifi_space"                             38 matches
  source = "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Prop_Crate_01.fbx"
  source = "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Prop_Crate_02.fbx"
  ...

In 35 directories it does not:

POLYGON - Sci-Fi City                            41 matches
  source = "SourceFiles/FBX/SM_Crate_Stack.fbx"
  ... and 37 more here
```

The two forms there are the two places a file can be. A path found
**loose** is printed from the pack directory down, so it has no leading
folder. A path found **inside an archive** is printed as the archive
stores it, which means it carries the folder the zip wraps everything in
— and *that folder is the zip's own name for itself, not the directory
you filed the pack under*. `POLYGON Sci-Fi Space` above is what is
inside the zip; `POLYGON - Sci-Fi Space Pack` is what the directory is
called, and `dir` is the only one of the two the manifest records.

Paste either as it comes; a shorter tail works too, as long as it is
whole folders — `SourceFiles/FBX/SM_Prop_Crate_01.fbx` names the same
file whichever way it was found, which is why it is the spelling the
manifest below carries.

A library is a hundred packs and `crate` is a word four hundred files in
it are called, so the answer is grouped by the pack holding it and cut at
a hundred lines. **The packs `art/manifest.toml` declares are printed
first and cut last**, because that file is this project's own statement
of which packs it cares about. And **every directory that matched says
how many matches it has**, whether or not any of them fit — so a pack is
never simply absent from the answer, which is what cutting a flat list in
walk order did, and it made the pack being worked in read as though it
held no crates at all.

A pack's `.unitypackage` is read only when it is the only archive that
pack has, which is the order [the resolver already looks
in](#what-answers-first). Listing a zip is a seek to the table at its
end, however large the zip is. A `.unitypackage` is a gzipped tar and
keeps no table, so its names cost the whole file — about a second per
hundred and fifty megabytes, which over a hundred and fifteen packs is
two minutes rather than two seconds. The Source Files archive beside it
holds the same meshes. `find` says how many it left unread, and `cargo
xtask art unpack <pack>` rebuilds one into the cache, which `find`
searches.

Paste the two lines it prints into a new table in `art/manifest.toml`,
give the table the stable id that code will use, and add the atlas the
mesh is painted from:

```toml
[asset.crate_small]
pack = "scifi_space"
source = "SourceFiles/FBX/SM_Prop_Crate_01.fbx"
texture = "SourceFiles/Textures/PolygonSciFiSpace_Texture_01_A.png"
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
art: taking POLYGON - Sci-Fi Space Pack/SourceFiles/FBX/SM_Prop_Crate_01.fbx out of
     /home/you/art/synty/POLYGON - Sci-Fi Space Pack/POLYGON Sci-Fi Space.zip (the archive stays as it is)
  crate_small              in POLYGON Sci-Fi Space.zip  9f2c1d4ab077  /home/you/space-trucking/art/cache/unpacked/...
art: converting 1 of 1 with blender /usr/bin/blender
  converted crate_small              -> glb/9f2c1d4ab077...glb
art: wrote /home/you/space-trucking/art/cache/index.toml
```

Exit status 0, and `art/cache/index.toml` holds one `[asset.<id>]` table
per line of the manifest with the converted file and the four overrides.
A second `resolve` converts nothing, takes nothing out of any archive,
and needs no converter at all: the cache is addressed by the digest of
the source, and the file an archive would be opened for is the file that
is looked for first, so both "already converted" and "already taken out"
are questions about a path rather than about a timestamp.

## What a failure looks like

This is the message the tool exists for, because on a fresh machine it is
the first thing it will ever do:

```
crate_small is not on this machine.

  pack      POLYGON Sci-Fi Space
  download  POLYGON Sci-Fi Space, the Source Files download
  declared  art/manifest.toml:114
  wanted    /home/you/art/synty/POLYGON - Sci-Fi Space Pack/SourceFiles/FBX/SM_Prop_Crate_01.fbx
  found     nothing: /home/you/art/synty/POLYGON - Sci-Fi Space Pack does not exist

  fix       Download "POLYGON Sci-Fi Space, the Source Files download" from your Synty
            account's downloads and put it, exactly as it arrives, at
            /home/you/art/synty/POLYGON - Sci-Fi Space Pack
            Leave it zipped if it came zipped; the archives are read where they lie.
            The directory is $SYNTY_STORE (/home/you/art/synty) plus
            `dir = "POLYGON - Sci-Fi Space Pack"` on art/manifest.toml:39.
```

Every missing asset is reported in one run, not one per run. A pack that
is present and does not carry the path reads differently from one that
was never downloaded — the first is a search and the second is a
download, and they have nothing in common. A download sitting loose in
`$SYNTY_STORE` with no directory of its own is named, and the fix is to
make the directory and move it in; it is not to unzip it.

Three more refusals worth knowing:

- **Nothing that opens a zip** names both programs that do, says which
  platforms already have which, and does not report your art missing —
  the pack is here and it is the reader that is not. That distinction is
  the difference between a package manager and an afternoon in your
  downloads folder.

- **A digest that no longer matches** stops the run and prints both, with
  the command that rewrites the line. A pack updated in the store is
  otherwise a mesh that silently became a different mesh, with override
  numbers beside it measured against the old one.
- **No converter** names Blender, its download page, both override
  variables, and the fact that `check` works without any of them.

## Proving the pipeline, with no Synty art at all

There are two fixture packs in the repository, both of them geometry this
project wrote, so the whole thing can be proved before any pack is
downloaded. One is a loose Source Files tree. The other is shaped like a
download nobody has touched: a directory the store named, spaces and
capitals in it, holding an icon and a zip with the assets wrapped in a
folder inside it.

Run it from the repository root. Every path in it is relative to that
root, so the block works as written on Windows, macOS and Linux:

```bash
SYNTY_STORE=xtask/tests/fixtures/store \
ART_MANIFEST=xtask/tests/fixtures/manifest.toml \
ART_CACHE=art/cache/fixtures \
  cargo xtask art resolve
```

Success is exit 0, a `converted unit_cube` line and a `converted
unit_pyramid` line, and two non-empty `.glb` files under
`art/cache/fixtures/glb/`. The pyramid is the one that proves the
interesting half: it came out of `A Zipped Pack/POLYGON Fixture
Pack.zip`, which is still a zip afterwards, and
`art/cache/fixtures/unpacked/` holds the two files the manifest named out
of the five in it — the mesh and its texture — and nothing else.

`ART_CACHE` is set only to keep the fixture run out of the cache a real
`resolve` fills: without it the run would write its own two assets over
`art/cache/index.toml`. Both are under `art/cache/`, which is gitignored
and disposable.

If that works, the pipeline works and what is left is finding the right
paths inside your packs:

```bash
SYNTY_STORE="$HOME/art/synty" cargo xtask art find crate
```

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
the order the three places are looked in, the reconstruction of a tree
out of a synthetic `.unitypackage`, reading names out of a zip without
extracting it, taking only the named files out of one, refusing a member
name that would climb out of the tree, content addressing, the digest
check, that a search ranks the packs the manifest declares above the rest
and counts every directory it cut, that a `.unitypackage` beside a Source
Files archive goes unread, and that a partial resolve indexes nothing. The zips those guards
run against are written byte by byte in the guard file, so they depend on
nothing this repository did not write.

The cabin declares a cargo feature, `art`, defaulting off. Nothing reads
it yet. It is declared now because the seam is the expensive half: the
slice that consumes `art/cache/index.toml` has to re-enable `bevy_gltf`,
`bevy_scene` and the image decoders that were cut out of the cabin's
dependency list, which is a cold build going back from 9m59s towards the
13m41s it was before the trim. A guard holds `default = []` so that cost
is paid deliberately.

## The three claims, checked

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

**"`tar` sniffs the format, so the call that opens a `.unitypackage` will
open a zip too."** *False*, and it was run rather than assumed. GNU tar
1.35 answers `tar: This does not look like a tar archive` and exits 2.
The hope was reasonable — bsdtar is libarchive and libarchive reads zip
— and bsdtar is indeed `tar` on macOS and `tar.exe` on Windows 10 build
1803 and later, where the trick does extend. It is Linux, which ships GNU
tar, where it does not.

So a zip is offered to `tar` first, because where that works there is
nothing to install, and to `unzip` second, which is what Linux has. The
alternative was a vendored inflate, and it is the wrong trade here:
DEFLATE, the central directory and Zip64 for the packs past four
gigabytes are several hundred lines whose bugs are silently wrong bytes
in a mesh, in a repository with no Synty pack to test any of it against.
Shelling out keeps the same bargain `tar` already had — the failure is
loud, immediate, and names the program to install.

`.7z` and `.rar` are recognised and refused by name rather than ignored,
because a pack reported missing when its archive is sitting right there
is the worst answer available.

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
