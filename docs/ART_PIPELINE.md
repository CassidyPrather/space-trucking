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
takes any program run as `<program> <source> <destination.glb> [texture]`,
which is very nearly what FBX2glTF and its forks already are — the third
argument is the atlas the manifest declared, and a converter that ignores
it still conforms. See [the converter
contract](#the-converter-contract-extended).

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

## The seven commands

`cargo xtask` is an alias defined in `.cargo/config.toml`; without it the
long spelling is `cargo run -p xtask -- art check`.

```bash
cargo xtask art check          # find every asset, hash it, report; converts nothing
cargo xtask art resolve        # check, then convert what is not cached, then write the index
cargo xtask art hash [id ...]  # print the `sha256` lines to paste into the manifest
cargo xtask art find <text>    # search the packs by file name, print the manifest lines
cargo xtask art unpack <pack>  # rebuild the trees inside one pack's .unitypackage files
cargo xtask art describe [text]# render each mesh, measure it, write what it looks like
cargo xtask art dex [text]     # read that catalogue back, searching what it says
```

The last two are [the catalogue](#the-catalogue-searching-art-by-what-it-looks-like),
and they are the only ones that write outside the cache: `art/dex/` is in
git, because what it holds is names, numbers and English rather than
anything derived from a mesh.

Everything else they write goes under `art/cache/`, which is gitignored
and disposable:

```
art/cache/
  unpacked/<pack>/<archive file name>/
                               what came out of one of the pack's archives: the
                               files a manifest named, out of a zip, or a whole
                               tree rebuilt out of a .unitypackage
  stage/<digest>/              a mesh and its textures, as the converter saw them
  glb/<digest>-<recipe>.glb    the converted asset. <digest> is the source mesh's
                               and <recipe> covers what else the conversion read:
                               the declared atlas and the converter script. See
                               "What the cache is addressed by" below
  glb/<digest>-<recipe>.aabb   the tight box the converter measured round it
  index.toml                   what resolved, the overrides for each, and what
                               each one dresses
  dex/<digest>/preview.png     the four views of one mesh a describer was shown
  dex/<digest>/prompt.txt      what it was asked, response.json what came back,
                               and request.json only when the call went wrong
  dex/<digest>/jobs.txt        the chunk of meshes one previewer launch was given,
                               filed under the first of them
  blender/fbx_to_gltf.py       the converter script, written out from the binary
  blender/fbx_to_preview.py    the preview script, which imports the converter's
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

The numbers that put the mesh in its berth come last, and they do not
have to be typed: add a `dresses` line, resolve, and then run the bench —
`--nudge`, below — which writes them back into this very table from in
front of the body.

## The catalogue: searching art by what it looks like

`find` answers the question a file name can answer. Ask it for a barrel
across a hundred packs and it says there are a hundred and fifty-five of
them, all called `SM_Prop_Barrel_01`, `SM_Prop_BarrelStack_01`,
`SM_Prop_Barrel_02`. Which of those is one barrel, which is a stack of
six on a pallet, and which is a tarpaulin thrown over a stack, is not in
any of those names — and the only way to find out used to be opening
Blender a hundred and fifty-five times.

So there is a second index, filed beside the manifest:

```bash
cargo xtask art describe              # every asset art/manifest.toml names
cargo xtask art describe barrel       # whatever is in the packs, up to --limit
cargo xtask art dex "hazard stripe"   # search what was written, not what things are called
```

`describe` renders each mesh, measures it, shows the picture to a vision
model, and writes one `[mesh.<id>]` table per mesh into
`art/dex/<pack>.toml`:

```toml
[mesh.sm_prop_barrelstack_01_tarp]
name = "SM_Prop_BarrelStack_01_Tarp"
pack = "polygon_apocalypse"
source = "Source_Files/FBX/SM_Prop_BarrelStack_01_Tarp.fbx"
sha256 = "491aeefb52ea9e0432b96026206a793866996a626a82984939be2c607eaa8fa4"
atlas = "PolygonApocalypse_Texture_01_A.png"
textures = "PolygonApocalypse_Texture_01_A.png"
description = "A low, wide, roughly rectangular mound of heavy blue-grey tarp, draped with stiff, angular folds that hang down at the corners and edges. Dark, square patches are scattered across the top surface, suggesting wear or repairs, while the underside is shadowed and uneven, hinting at the stacked barrels hidden beneath."
described_by = "deepseek/deepseek-v4-flash-vision-exp"
triangles = 476
meshes = 1
materials = 1
size = [6.5787573, 5.29675, 6.981569]
```

That is a real entry, written by the real command against a real pack.
`source` is the line to paste into `art/manifest.toml`, and an entry for a
mesh the manifest already names carries an `asset` line saying which.

### It is hybrid, and the numbers are the half that is true

A vision model shown a picture of a crate writes a confident sentence
about a crate. It cannot tell you the crate is 476 triangles or 6.6 units
across, and asked, it will guess. So the two halves come from two places
and stay in separate fields: **Blender measures**, with the file open, and
the model is asked only for what a picture can answer — shape, markings,
colour, wear, and what the thing appears to be for. The measurements are
also in the prompt, so the model has no reason to invent them.

**And the model is told what it is looking at.** That is the difference
between a catalogue worth searching and four hundred lines reading "a
low-poly 3D model of a tree". The prompt carries the pack's own name for
the asset — `SM_Tree_Pine_04`, and `Tree Pine 04` beside it — the pack it
came from, and every measurement, and then asks for the sentence the name
does *not* contain, with "do not begin with 'This is'" and "do not repeat
the name" spelled out. There is a guard on it in
`xtask/src/describe.rs` and another end to end in `xtask/tests/dex.rs`,
because it is the single assumption the whole command is worth anything
under.

### The second look: what tells the variants apart

A description is written about one mesh, and **six tenths of a real
library is numbered variants of something** — 29,479 meshes in 8,158
families, median size three. Describing those one at a time does not work,
and it fails in a way that is worse than a blank. Five light panels,
described in isolation:

```text
_01  A slim, elongated rectangular fixture with a dark grey, bevelled frame and a
     recessed, pale grey translucent panel...
_03  A low, elongated surface-mounted fixture with a dark grey housing and a bright,
     recessed white strip, framed by a raised rim...
```

All five said the same thing. Worse, where they differ — "translucent
panel" against "bright white strip" — a reader cannot tell whether that is
a difference between the meshes or between two rolls of the same die. The
model never saw the others; it had nothing to compare against.

So a **second pass looks at a whole family at once** and writes one line
per member into `differs`:

```text
_01  The only unlit, frosted panel; plainest and most minimal, for ambient wall mounting.
_02  The only one with blocky feet and a single emissive strip, a floor-mounted utility light.
_03  The only one with a bright white lens and lighter bezel; the most standard ceiling light.
_04  The only one with an orange stripe and cylindrical nodes, a damaged or tech-accented look.
_05  The only one with three parallel glowing tubes; the most detailed and largest.
```

Those are real, from the real command. Each one agrees with what the
isolated pass independently noticed about that mesh, which is the reason
to believe them.

It costs **no new rendering at all**: a chat message may carry several
images, so the pass sends the preview sheets already on the disk. One call
per family, about $0.0008 for a family of five.

Three rules it keeps:

- **Answers are matched by name, never by position.** The model is told to
  begin each line with the asset's own name; a line naming nothing in the
  family is dropped, and a member nobody wrote a line for keeps nothing. A
  differentiator filed against the wrong sibling sends somebody to the
  wrong mesh, confidently, and is worse than the blank it replaced.
- **A family is a shared name plus a trailing `_NN` or `_A`, in one
  pack**, and only the last part comes off — `SM_Bld_Wall_Corner_01` is
  not a variant of `SM_Bld_Wall_01`. A family past eight is compared in
  groups, because ninety-two modular pieces is not a question anybody can
  ask in one message.
- **A comparison outlives a re-description of the same bytes.** It is
  bought separately, so `--force` over one member does not throw away the
  comparison for the family — unless the mesh itself changed, when a
  comparison of the old geometry is not a fact about the new.

**And identical bytes are one mesh, however many names a pack gives it.**
A pack ships the same geometry under two names about one time in a
hundred; those are looked at once and every name gets the same answer.
That is cheaper, and it is the truer answer — describing one mesh twice
produces two sentences differing in adjectives and not in fact, which is
the same defect arriving from the other direction. It was found the hard
way: three copies of one mesh, and the third came back describing the
second, because the picture and the prompt beside it are addressed by the
digest of the geometry.

### What it needs, and what it costs

| | |
| --- | --- |
| Blender | the same install `resolve` converts with. `$ART_PREVIEW` overrides it with any program run as `<program> <jobs file>` |
| `$OPENROUTER_API_KEY` | a hosted vision model, reached with `curl`. `--model` or `$ART_DESCRIBER_MODEL` picks another; the default is a cheap one that reads pictures |
| `$ART_DESCRIBER` | instead of that: any program run as `<program> <prompt.txt> <picture.png>` that prints a description — a local model, or something of your own |
| neither of those two | `--offline`, or simply no key: the entry is written from the measurements and `described_by` says `measurements alone` |

A run over found meshes stops at **24** and says how many it left, because
`describe crate` over a library is hundreds of renders and hundreds of
model calls — a bill arriving because somebody typed a common word.
`--limit` raises it. `--jobs` (eight by default) is how many chunks are
looked at at once. A mesh already described against the bytes on this
machine is skipped, so a second run costs nothing and `--force` is how you
overrule that.

**The catalogue is written as it goes**, at the end of every chunk, so a
sweep is something you can interrupt and pick up again: what was described
stays described, and the next run skips exactly those.

One thing worth knowing about the count: a Source Files download ships
the same mesh as `FBX/X.fbx` **and** `OBJ/X.obj`, and describing both
would spend two renders and two model calls writing the same sentence
twice. The FBX wins. Two different meshes that merely share a name —
`Props/SM_Crate.fbx` and `Buildings/SM_Crate.fbx` — are both kept, and
the second gets `sm_crate_2`.

### What a whole library costs

Measured on a twelve-core Windows machine against 115 packs holding
**49,398** describable meshes — one entry per `(pack, name)`, FBX
preferred over OBJ:

| | per mesh | 49,398 meshes |
| --- | --- | --- |
| time, `--jobs 8` | 0.54 s | **7.4 hours** |
| hosted model | $0.000085 | **$4.20** |
| preview cache | 70 KB | **3.3 GB**, gitignored |
| catalogue text | 786 bytes | **37 MB**, in git |

The comparison pass is on top of that and needs no rendering: about
8,158 calls for the families, roughly **$5 and half an hour** — call the
whole thing **eight hours and under ten dollars**.

A single pack is the practical unit: the median pack is 225 meshes — two
minutes and two cents — and the largest is 2,593, about twenty-three
minutes. Only 2.5% of the library is collision geometry and none of it is
LOD copies, so there is little to be gained by filtering those out.

Two of those numbers were four times worse before they were measured, and
the fixes are worth knowing because they are the levers if this ever needs
to be faster again:

- **One launch per chunk, not per mesh.** Blender costs about 2.2 seconds
  to start before it has looked at anything, against about 5 seconds of
  work, so a third of a sweep went on starting up. Chunks of up to 32 took
  the per-mesh cost from 5.05 s to 0.82 s at the same `--jobs 4`.
- **256 pixels square, not 1024.** Cycles time, PNG bytes and the tokens
  an image costs all scale with the pixel count, and the descriptions do
  not get better with more of them: on the one mesh where a 512 and a 256
  run disagreed — a crate of drums — the 256 answer was the more accurate,
  having seen an olive drum the 512 answer missed. What neither size fixes
  is counting: "three hoops" and "four hoops" came from the same barrel,
  because that is the model rather than the pixels. `$ART_PREVIEW_SIZE`
  raises it for a pack of something more detailed.

### The picture

Four views of the mesh in one 256-pixel PNG, turned a quarter turn
between them, on a plain grey ground, painted with the atlas: a model
shown one three-quarter view of a crate can say nothing about the back of
it, and four views cost one render because the copies share their mesh
data. It is `art/cache/dex/<digest>/preview.png`, and it stays there —
with the prompt and the answer — for the same reason the staging
directories do. **A description that reads wrong is usually a right
description of a bad render**, and that directory is where you find out
which.

The previewer is handed a **file of jobs** rather than one mesh, which is
what lets one Blender launch cover a chunk of them:

```text
<program> <jobs file>          # one `source|picture.png|texture` per line
```

and it answers with a `look <n>` block per job, headed by the job's line
number, carrying the same `tris`/`meshes`/`materials`/`image`/`aabb`
lines. A job it cannot do prints `trouble <n> <why>` and the launch
carries on, because one unreadable FBX in a chunk of thirty-two is not a
reason to lose the other thirty-one. `|` is the separator because Windows
forbids it in a path and a shell stand-in can split on it in one line.

One Windows trap is worth knowing, because it looks like a broken pack.
A path into the cache is a cache root plus a pack plus the archive's own
name plus the tree inside it, which goes past Windows' 260-character
limit on ordinary packs — `POLYGON - Alpine Mountain`'s ice axe does it at
268. Rust's standard library reaches those files without being asked and
Blender's Python does not, so the resolver found, hashed and handed over
meshes that then came back as `no such source file`: a quarter of one
sweep. Both scripts now spell a long path with the `\\?\` prefix
(`openable` in `fbx_to_gltf.py`), so this is fixed rather than something
to work around — but it is why a very deep `$ART_CACHE` is worth avoiding.

The atlas is the manifest's `texture` line for an asset that has one, and
otherwise the pack's own shared atlas, picked by name. That guess is
recorded as `atlas` in the entry, because a preview rendered untextured
is a grey mesh that a model then describes, accurately and uselessly, as
grey — and the entry should say when the colours it describes came from a
guess. `textures` is what the scene actually bore.

### What it is, and is not

`art/dex/` is **in git**, and it holds the same kind of thing
`art/manifest.toml` holds: file names, digests, counts and English. No
geometry, no pixels, nothing that could be turned back into a mesh. It is
tracked rather than cached because each line costs a Blender launch and a
hosted model call, and a gitignored copy would be bought again on every
clone.

It is a **catalogue, not a manifest**. Every line of the manifest is a
promise somebody typed and something checks; every `description` here is
a sentence a language model wrote about a picture, and nothing in this
repository can check it. Search it, read it, then look at the mesh —
`described_by` says what saw it, and an entry nothing looked at says so
in the description itself rather than reading like one that did.

### What is proved, and what needs your disk

`xtask/tests/dex.rs` runs the whole command against the fixture packs
with stand-ins for the two things CI has not got: a previewer and a
describer. The describer stand-in answers with the prompt it was handed,
which is what lets a guard read the catalogue and prove the asset's own
name and pack reached the model. Proved there: that a run writes a
catalogue that reads back, that measured numbers reach it, that a pack
the manifest never declared is still catalogued, that a described mesh is
left alone until it changes or `--force` says otherwise, that the search
answers a word from a description, that a describer answering nothing
writes nothing, and that a mistyped option is a refusal.

Not proved there, and only provable on a machine with the packs: that
Blender renders four legible views of a Synty prop, and that a hosted
model writes something true about the picture. Both were checked by hand
against `POLYGON - Apocalypse` and `POLYGON Sci-Fi Space` on the owner's
machine — the tarpaulin entry above is one of the answers.

## What a successful run looks like

```
$ cargo xtask art resolve
art: 1 asset over 2 packs, from /home/you/art/synty
art: taking POLYGON - Sci-Fi Space Pack/SourceFiles/FBX/SM_Prop_Crate_01.fbx out of
     /home/you/art/synty/POLYGON - Sci-Fi Space Pack/POLYGON Sci-Fi Space.zip (the archive stays as it is)
  crate_small              in POLYGON Sci-Fi Space.zip  9f2c1d4ab077  /home/you/space-trucking/art/cache/unpacked/...
art: converting 1 of 1 with blender /usr/bin/blender
  converted crate_small              -> glb/9f2c1d4ab077...-36488a626354.glb
art: wrote /home/you/space-trucking/art/cache/index.toml
```

Exit status 0, and `art/cache/index.toml` holds one `[asset.<id>]` table
per line of the manifest with the converted file and the four overrides.
A second `resolve` converts nothing, takes nothing out of any archive,
and needs no converter at all: the cache is addressed by everything the
conversion read, and the file an archive would be opened for is the file
that is looked for first, so both "already converted" and "already taken
out" are questions about a path rather than about a timestamp.

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

## The four overrides, and the frame they are written in

Every asset table carries four per-axis numbers:

```toml
scale = [1.0, 1.0, 1.0]
offset = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
fill = [1.0, 1.0, 1.0]
```

They were here from the first day, before anything read them, and the
reason is worth keeping said. Geometry in this game is a *pure
description* — `pieces::parts`, `room::seam_parts`, `room::charts`,
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

### The placement frame

Something reads them now, so the frame they are written in is an API, and
this is it.

A cargo kind's description claims a box: its `Kind::upright` cells across
and up, and the one cell of depth every rig is composed within
(`pieces::RIG_NEAR..RIG_FAR`). **That box is `[-1, 1]` on every axis of
the placement frame** — the same normalised frame `poi::Fitting` states a
station's hardware in, one box down, so the vocabulary is one vocabulary.

| | what it means |
| --- | --- |
| `scale` | the converted file's own units, carried into berth half-units |
| `rotation` | degrees about x, then y, then z, taken in the box's own axes |
| `offset` | where the body's middle sits, in berth half-units — this is `poi::Fitting`'s `at` |
| `fill` | what the body then occupies of the box, per axis — this is `Fitting`'s `half`, and `Shape::fill`'s meaning |

In that order. The mesh is carried onto its own measured middle, scaled,
turned, and set down at the offset:

```text
world = berth pose ∘ T(offset ⊙ berth half) ∘ R(rotation) ∘ S(scale ⊙ berth half) ∘ T(−measured middle)
```

`|offset| + fill ≤ 1` on an axis is exactly "the body stays inside its
berth". It is **not enforced** at read time, deliberately: a body that
leaves its berth is a finding, and the gauntlet's families already know
how to say so — a `fill` past the cells is `face-fits`, an `offset` out
of the band is `berth-clear`.

Three consequences to know before writing numbers.

**The mesh is centred on its own bounds, not on its origin.** A Synty
prop often sits on its own base. `offset = [0, 0, 0]` has to mean
"centred in its berth" for the arithmetic above it to mean anything, so
the converter's measured middle is subtracted first. A converter that
reports no bounds leaves the mesh on its own origin, which is the only
honest thing to do with a body whose size nobody knows.

**A berth box is a cube for a one-cell kind and 1:2:1 for a `1×2` one**,
so the frame is anisotropic on the tall kinds. A rotation that is not a
quarter turn shears a body there. A per-axis `scale` is how to answer
that, or turn the mesh in Blender and leave `rotation` alone.

**The game draws the index's numbers, not the manifest's.** An edit to
`art/manifest.toml` reaches the game through `resolve`, which is also the
only moment it is checked. The gauntlet is the other way round: it reads
the manifest, because the manifest is what is in git. The bench writes
both, for exactly this reason — see "The bench", below.

### `dresses`: which body a mesh stands in for

One optional key per asset says what the mesh is *for*:

```toml
dresses = "cargo/suspicious_crate"
```

The namespace is there from the first line so that `fitting/...` can
exist later without re-spelling anything; `cargo` is the only one today
and anything else is a refusal naming what it knows. The name after the
slash is the cargo kind's own spelling in snake case — `Kind::BayWindow`
is `bay_window` — derived rather than tabled, so no second list can fall
out of step with `Kind::ALL`.

The resolver checks the *shape* of a binding and deliberately not its
meaning: `xtask` cannot see a `cargo::Kind` and should not learn to. The
other half is a guard in the cabin
(`art::tests::every_dressed_name_in_the_manifest_is_a_body_this_game_has`)
which reads this very file and refuses a name the game has no body for.

The thirty-two names, in `Kind::index` order:

```text
perfume_vial        gilded_idol       ration_bricks    scrap_alloy
seedlings           gas_canister      cryo_core        brine_pearls
suspicious_crate    mysterious_crate  very_mysterious_crate
comet_ice           bottled_midnight  fluff            transit_chit
casino_chip         ceiling_lamp      wall_lamp        floor_lamp
couch               painting          cabinet          rug
paint_tin           luminous_paint    window           chart_tank
eta_gauge           dest_preview      launch_lever     porthole
bay_window
```

### The promise, and where it meets the fact

`scale` and `fill` are **redundant on purpose**, and the redundancy is
the mechanism. `fill` is a promise living in a public repository, which
is what lets continuous integration sweep it with no art on the machine.
`scale` times the mesh's own measured size is the fact, and it lives on
the owner's disk. `cargo xtask art resolve` is the one place both exist
at once, so it is where they are made to meet:

```text
crate_small is not the size cargo/suspicious_crate says it is.

  axis      x
  declared  fill [1.0, 1.0, 1.0]
  measured  [0.5, 0.5, 0.5] of its berth box
  from      a mesh [0.5, 0.5, 0.5] half-units across, at scale [1.0, 1.0, 1.0]
  off by    -0.5000, and 0.02 is the slack

  fix       Either the mesh moved under the line or the line was a guess. If the
            mesh is the one you want, paste this and the promise is true again:

              fill = [0.5, 0.5, 0.5]

            If it is the SIZE that is wrong rather than the claim, this scale puts
            the mesh exactly in its berth, and `fill = [1.0, 1.0, 1.0]` with it:

              scale = [2.0, 2.0, 2.0]
```

The slack is **0.02 of a berth half-extent** — a fiftieth of the
half-box, about 5 mm on a one-cell kind. That is the same order as the
gauntlet's own clip slack and coarser than the two decimals a `fill` is
written with by hand; tighter and a correctly-rounded `0.18` is a
refusal, looser and a mesh can be a centimetre bigger than the box every
containment rule reads for it.

**Only an asset with a `dresses` line is asked.** The identity
`fill = [1.0, 1.0, 1.0]` is the default, and it is the claim most
imported meshes turn out to break; refusing it before anything reads it
would make the manifest impossible to write in the order people write it
— a path first, a digest second, the numbers last.

### The converter contract, extended

A converter is run with the mesh, the file to write, and — when the
manifest declared one — the atlas to paint with:

```text
<program> <source> <destination.glb> [texture]
```

**The `texture` line is a declaration, and this is where it is spoken.**
It used to be a hint acted on by arrangement: the resolver copied the
atlas beside the mesh and into a `Textures/` folder, and left it to the
FBX's own relative texture reference to find one of them. That works for
a mesh file that names its atlas, and Synty's commonly do not — they
assign their materials in Unity, through the `.unitypackage`'s `.mat`
files, so the FBX carries a bare material naming no image. A file that
names nothing resolves nothing, and the crate renders grey.

So the atlas is now an argument, and the Blender script this repository
ships assigns it as the **Base Color of every material that has no image
texture of its own**, creating a material where a mesh has none, before
it exports. The staged copies stay exactly where they were, because **an
FBX that names its own texture still wins**: it resolves against them
during import, the material comes out of the importer already carrying an
image, and `paint_with` leaves it alone. The declaration fills a silence;
it never overrules a statement.

**A reference is not knowledge.** That distinction cost a second grey
crate. Synty's FBX files often DO name a texture — by the path of the
tree they were exported from, sometimes a `.psd` that was never in the
pack — and Blender answers a reference it cannot resolve with a
placeholder image datablock: a name, a filepath pointing nowhere, no
pixels. The old rule asked "is there an image on this node?", the
placeholder answered yes, and the material was skipped as one that knew
its own texture. So what `usable_image` asks now is whether the pixels
can be got at at all — decoded in memory, packed into the file,
generated, or a filepath that resolves to a file that is there — and a
material whose every image reference is a placeholder counts as silence.
Such a material has the atlas **rebound onto the importer's own node**,
because the nodes, links and UV coordinates around it are exactly what
the FBX asked for and only the pixels are missing.

The boundary is deliberate: **one loadable image anywhere in a material
leaves the whole of it alone**, even if another slot is broken.
Overruling half of what a file says is how a fallback turns into a
correction.

**And a flag is not pixels.** That cost a third. With the atlas declared,
staged, handed over and the rebind in place, the same crate came out grey
under Blender 5.0, whose importer answers the same unresolvable `.psd`
reference with a placeholder that reports `has_data` — and a size of
nought by nought. The rule that asked whether the pixels were already in
memory took the flag at its word, and the refusal below, asking the same
question, agreed with it. So `usable_image` now asks for the image's
**size**, which Blender can only answer by opening the file: a reference
that resolves nowhere comes back 0×0, and 0×0 is silence whatever the
flag beside it says.

**And a conversion handed an atlas it painted nothing with is refused.**
Before exporting, the script checks that at least one material in the
scene bears an image that can be loaded, and if none does it exits
nonzero, names the atlas, and writes no file:

```text
the declared atlas reached no material: <path to the staged atlas>
  <source> exported N material(s), and not one of them bears
  an image that can be loaded — so this .glb would be grey wherever the
  atlas should be. A conversion handed a texture and painting nothing is
  the defect the texture argument exists to fix, and it is silent, so it
  is refused here rather than discovered in the cabin.
```

This is part of the converter contract and not a nicety. All three grey
crates were conversions that **succeeded**: exit zero, a measurement
printed, a file of an entirely plausible size — and a `.glb` with no
image in it is a perfectly valid `.glb`, so nothing between the converter
and the cabin could tell. The script is the only program in the pipeline
that can see a material, so the check belongs there and nowhere else. It
applies only when a texture was handed over: a manifest with no `texture`
line has declared nothing, and a grey result there is an unfinished
manifest rather than a failed conversion.

It is also exactly as sharp as the question it asks. The third crate
walked through it, because the refusal counts an image by `usable_image`
and `usable_image` was the thing that was wrong. That is deliberate and
stays so: one predicate, used by the painter and the refusal alike, so
the two can never disagree about what counts as painted — which means a
fix to what counts is a fix to that one function.

A converter that ignores the third argument is **not in breach** — the
atlas simply resolves nothing, which is the honest outcome for a program
that has never heard of this repository. One thing to know if you wrote a
converter against the older contract: `ART_CONVERTER=/bin/cp` no longer
works, because `cp` means something specific by a third argument. A
two-line shim does:

```sh
#!/bin/sh
cp "$1" "$2"
```

The measurement has to come from the only program in the pipeline that
can see a mesh. So a converter may print one line on standard output:

```text
aabb <min x> <min y> <min z> <max x> <max y> <max z>
```

in the **converted file's** axes, not Blender's — `export_yup` turns
Z-up into Y-up on the way out, and a number that names the wrong axis is
worse than no number. The Blender script this repository ships does it
(`report_bounds` in `xtask/blender/fbx_to_gltf.py`). The whole of a
conforming converter is still `cp` and a `printf`, which is the property
the contract was shaped for.

A converter that prints nothing is **not in breach** either: FBX2glTF has
never heard of this repository. The run says which promises went
unchecked and what to print to be checked, rather than refusing what it
cannot see.

The measurement is filed under the same name the converted file is, as
`glb/<name>.aabb`, so a warm cache keeps the check running. A check that
only ran on the run that happened to convert would be a check that stops
running the moment the cache is warm.

### What the cache is addressed by

A converted file is named after **everything that conversion read**:

```text
glb/<source digest>-<recipe>.glb
```

The source digest is the front of it, because that is the number a person
can check by hand — the manifest carries it, the index records it, and
`sha256sum` prints it. The twelve hex digits after it are a digest of
what else went in: the digest of the declared atlas, and the digest of
the Blender script this binary carries.

That second half is not decoration. Before it existed the cache was
addressed by the source mesh alone, so the fix above would have reached
nobody: the owner's crate is converted from a mesh that did not change,
and the next `resolve` after pulling a fix to the *script* would have
said "already converted" and left the grey crate where it was. Folding
the script and the atlas into the name means **nothing has to be deleted
by hand**. Old entries are not cleaned up either; they are simply at
names nothing looks at any more, and `art/cache/` is gitignored and
disposable in one command.

What is deliberately *not* in the name is which program on your machine
did the converting. Answering that would mean finding a converter on
every run, including the runs with nothing to do — and "a second
`resolve` needs no converter at all" is a property worth more than
noticing that you swapped Blender for FBX2glTF. If you do swap, delete
`art/cache/glb/`.

## The loading path: what `--features art` actually turns on

The cabin's `art` feature was declared long before anything read it,
because the seam is the expensive half. It reads it now.

**One Bevy feature, not the three that were predicted.** `bevy_gltf`
brings `bevy_world_serialization` with it, and in Bevy 0.19 a loaded
glTF scene is a `WorldAsset` spawned through a `WorldAssetRoot` — so
`bevy_scene`, which is now the BSN authoring language, stays out; this
game authors no scenes. And no image decoder was added: `png` has been on
the cabin's list since screenshots needed it to *write*, Bevy's `png`
feature is `image/png`, and a Blender-exported `.glb` embeds its textures
as PNG unless the source was a JPEG. If a pack ever ships one that is
not, `"jpeg"` on the `art` line is the fix.

**That prediction stayed a prediction for two fixes longer than it
should have.** It was written down as checked, on the day the atlas first
became an argument — and it was not, because that conversion painted
nothing, and neither did the one after it: each `.glb` was byte for byte
the size of the colourless one before it, with no PNG signature anywhere
inside. It is a fact now. The first `resolve` after the third fix ("a
flag is not pixels", above) wrote a `.glb` eleven times the size of the
grey ones, with the atlas embedded as `image/png` and bound as the one
material's base colour; the cabin read it through the loader with no
complaint on stderr and drew the crate in the atlas's colours — checked
on the owner's machine with `--fixture --shot`, which is the only place
it can be. The reasoning that held it up in the meantime still holds: a
glTF with an embedded image is read by the same loader as one without,
the image chunk is decoded by the same registry `--shot` writes through,
and the atlases the manifest names are PNG
(`PolygonSciFiSpace_Texture_01_A.png`, on `art/manifest.toml`'s only
asset line), so `"jpeg"` stays one word away and unneeded.

Eight crates: `bevy_gltf`, `bevy_world_serialization`, `gltf`,
`gltf-json`, `gltf-derive`, `base64`, `byteorder`, `inflections`.

**The default build pays nothing**, and that is measured rather than
asserted: 338 packages before and 338 after, the same 338.

At boot, under the feature and only under it, the cabin reads
`$ART_CACHE/index.toml` and asks the asset server for every `.glb` a
`dresses` line names. The art cache is the asset root — nothing else in
this game reads a file through the asset server, so there is no
`assets/` directory to share and a path out of the index is a path the
server takes verbatim. `pieces::build_kind` then spawns that scene in the
rig's place **instead of** stamping the whitebox parts: two graphical
implementations of one object means the player sees one of them.

**Everything about it fails soft.** No cache directory, no index, an
index that will not parse, an entry naming a file that is not there, a
`dresses` naming a body this build has no kind for: each leaves the kind
undressed, draws the whitebox, and puts a sentence on stderr. Nothing
reaches the screen as text — the zero-text law covers what is drawn.

## The bench: nudging a body into its berth

The four numbers have to come from somewhere, and until now that
somewhere was a text editor, a guess, `resolve`, a relaunch and a look —
a minute-long loop for a change worth a thousandth of a berth. The bench
is the same loop with the editor and the relaunch taken out.

```sh
cargo run -p cabin --features art -- --nudge --fixture
```

Stand in front of a purchased body, take it, move it until it sits
right, and press `Enter`. The three numbers go back into that asset's
table in `art/manifest.toml`, where they ride version control like every
other promise this repository makes.

**Two gates, and the second is the one that matters.** `--features art`
decides whether there is a bought mesh to nudge at all. `--nudge` decides
whether this process contains a system that can write to a tracked file —
without the flag the bench's systems are never added to the schedule, so
an ordinary player session is *incapable* of editing the repository
rather than merely unlikely to. A key chord would have been fewer
characters to type and would have left the file-writing code live in
every session anybody ever plays.

### What the hands do

| | |
| --- | --- |
| `Tab` | take the dressed body under the crosshair, or let go |
| `T` `R` `G` | what the six direction keys move: offset, rotation, scale |
| `←` `→` | the berth's own x, minus and plus |
| `↑` `↓` | its y, plus and minus |
| `[` `]` | its z, minus and plus — into the wall and out of it |
| `Shift` | the fine step |
| `Backspace` | put the numbers back to what the file says |
| `Enter` | write them into the manifest |

One press is one step: a coarse move is 0.05 of a berth half-unit (about
14 mm on a one-cell kind) and a fine one is 0.005; a coarse turn is 15°
and a fine one 1°. There is no key repeat, because a held arrow at sixty
steps a second crosses a whole berth in a third of a second. Every number
lands on a thousandth, so the fourth press of `↑` writes `0.2` into the
owner's file and not `0.20000002`.

**The arrows move the body in the berth's own axes, not in yours.**
Standing behind a body on the aft wall, `→` moves it to your left, and
that is correct: what the key moves is the *number*, in the frame the
number is written in ("The placement frame", above), so what you press is
what the diff says. Which way is plus is not left to be remembered — the
overlay draws a tip on the plus end of every axis.

Every copy of that kind aboard moves together, because one declaration
dresses them all.

### What it draws

Shapes, and only shapes — the zero-text law covers everything rendered.
Rods along the axes for the offset, rings round them for the turn,
calipers across them for the size, a tip on every plus end, and above the
body **a ring that is whole while what you are looking at is what the
file says, and broken into dashes while it is not**. Broken-means-
provisional is the cabin's existing vocabulary, not a new one: it is what
a room's mark on a good already says. The overlay's description carries
no colour at all, which is the strongest form of the no-hue-alone rule
available.

Confirmations, refusals and file names go to **stderr**, which the law
has never covered and where the rest of this pipeline already talks.

### What a save does, and what it will not

It writes `scale`, `offset` and `rotation` into `[asset.<id>]` — the id
comes from the index, which is where the numbers being drawn came from —
as a **surgical line edit**. The value on each line is replaced and
nothing else in the file is touched: not the prose, not the blank lines,
not the spacing round the `=`, not a comment on the value's own line, not
the order tables stand in, not even the line endings. A key the table
never had is added among that table's own keys. The number style is held
to the real file by a guard that rewrites the shipped manifest's own
table with the numbers already in it and requires the result to be the
same bytes: saving what was already there is a diff of nothing.

The manifest is found the way `xtask` finds it — `$ART_MANIFEST`, and
`art/manifest.toml` otherwise — so a nudge and a resolve cannot end up
reading different files. An id the manifest has no table for is a refusal
that writes nothing at all, which is the case that matters when an index
has drifted from the manifest.

Then the same three lines are carried into `$ART_CACHE/index.toml`,
best-effort, so the body is still where you put it after a restart rather
than back where the last `resolve` left it. That file is derived and
gitignored; the next `resolve` rewrites it from the manifest, which by
then says the same thing.

**It does not write `fill`.** `fill` is the promise the mesh is measured
against, and a bench that derived it from the mesh would make it
unbreakable — precisely the thing it exists not to be. So nudging `scale`
can leave a `fill` that is no longer true. The save says so on stderr,
and the next `cargo xtask art resolve` refuses with the line to paste.

### What is proven, and what needs eyes

The transform arithmetic, the gesture vocabulary, the state machine, the
writer and the round trip are all proven headless, and the round trip
goes out through the writer and back through **the loader's own parse** —
what the owner was looking at when they pressed the key is asserted to be
the pose the file describes, never a second reader written for the test.
Three scripted sessions drive the real systems over a real sim with real
key edges, the way `crate::session` drives the cabin's own input.

What none of that can answer is whether the overlay is *legible* — how
the rings read against a dark cabin, whether the calipers are told apart
from the rods at a glance, whether the plus tip is big enough to find.
That needs a window and an eye. It is recorded in
[GAUNTLET.md](GAUNTLET.md) rather than assumed.

One thing to know while using it: the crosshair still picks a dressed
body by its **whitebox** box, because the pick face comes from
`pieces::drawn_box`, which is a pure function of `Kind` (GAUNTLET.md's
blind-spot list). A bought mesh much smaller than its berth is taken by
aiming where the whitebox was.

## What continuous integration does, and does not

CI **never** resolves art and never will: the payload is not in the
repository, so there is nothing for it to resolve. It builds and tests
the whitebox, which is where all sixteen gauntlet families and the
determinism guards live.

What CI does run is `xtask`'s own guards, which are about the resolver's
rules and need no art: the manifest dialect, the missing-asset message,
the order the three places are looked in, the reconstruction of a tree
out of a synthetic `.unitypackage`, reading names out of a zip without
extracting it, taking only the named files out of one, refusing a member
name that would climb out of the tree, content addressing, the digest
check, that a search ranks the packs the manifest declares above the rest
and counts every directory it cut, that a `.unitypackage` beside a Source
Files archive goes unread, that a partial resolve indexes nothing, that a
`dresses` line and a measured size reach the index, that a `fill` which
disagrees with a measured mesh stops the run, that a declared atlas is
handed to the converter as an argument and a manifest declaring none
hands it nothing, and that a conversion made before the atlas was
declared — or under a name the older pipeline used — is made again. The
zips and the recording, measuring and copying converters those guards run
against are written in the guard file, so they depend on nothing this
repository did not write.

It runs [the catalogue's](#the-catalogue-searching-art-by-what-it-looks-like)
guards the same way, through the same kind of seam: `$ART_PREVIEW` stands
in for Blender and `$ART_DESCRIBER` for a hosted model, and the describer
stand-in answers with the prompt it was handed — which is what lets a
guard read the written catalogue and prove the model was told the asset's
own name. No key is ever needed, and every variable this command reads is
cleared before each run, so a developer with `$OPENROUTER_API_KEY`
exported does not have the suite spend money on a fixture cube.

**There is no Blender in continuous integration or in the container this
was written in**, and for a while that meant those guards stopped at the
argument. They no longer do. All three grey crates were defects in the
converter script's **control flow** rather than in anything Blender did,
and control flow is a thing a fake `bpy` can execute:
`xtask/tests/fixtures/blender/bpy.py` builds a scene, records every image
binding and node the script makes, and `xtask/tests/converter_script.rs`
runs the real `fbx_to_gltf.py` against it under `python3` and reads the
decisions back. Proved there: that a broken texture reference is
repainted rather than skipped, that a placeholder claiming data and
measuring nought by nought is repainted too, that the repaint reuses the
importer's own node instead of building a second one beside it, that a
reference which resolves is left alone, that a material naming nothing
still gets a node built for it, that an atlas which reached no material
is refused and no file written, that a conversion handed no texture is
unchanged, and that a Blender which has shed `Material.use_nodes` — 5.0
deprecates it and expects 6.0 to remove it — is painted rather than
crashed. A machine with no Python says so and skips, rather than passing
quietly.

One lesson of the third crate is about the fake itself. Its `Image`
answered `has_data` the way the script hoped a placeholder would, so the
guard for the second crate passed against a Blender that does not exist.
The fake now answers `size` the way Blender does — by whether the file is
there — and the scene for the third crate pins the flag high over a size
of nothing, which is what Blender 5.0 actually hands back.

What that stops at is Blender itself. That a repainted material survives
`export_scene.gltf`, that the resulting `.glb` carries the PNG, and that
it renders in colour is proved on the owner's machine and nowhere else —
`cargo xtask art resolve` and then a look at the crate in the cabin.

**It also builds and tests the art seam**, with `--features art`, and
that is new. The feature is off everywhere else, so without those two
steps the only code in the repository that loads a purchased mesh would
compile nowhere and rot unlinted. They still resolve nothing and prove
nothing about a Synty mesh. What they prove is that the seam compiles,
that its guards pass, and that the cabin's own glTF path reads a binary
glTF — because the test writes one, byte by byte: a unit cube, header,
both chunks, accessors, a mesh, a node and a scene, with no new
dependency. A fixture built by the library under test would only prove
that the library agrees with itself.

The cost, measured on a four-core runner: about **25 seconds** once the
dependency cache holds both feature sets — 4 s of clippy and 21 s of
tests — and about **7 minutes** on the first run after it lands, because
the art feature is a second compilation of Bevy's upper crates plus the
eight the glTF loader brings. `Swatinem/rust-cache` keeps that.

`the_cabin_ships_the_whitebox_unless_art_is_asked_for` still holds
`default = []`, so the whitebox stays the build everything else means.

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

**And a fourth claim, which was wrong: "an FBX names its own texture, so
staging the atlas beside it is enough."** It is not, and the first real
Synty mesh this pipeline converted proved it — the crate came out
colourless. Synty assign their materials in **Unity**, through the
`.unitypackage`'s `.mat` files, and the FBX commonly carries a bare
material naming no image at all. A file that names nothing resolves
nothing, however carefully the file beside it was staged. See
[the converter contract](#the-converter-contract-extended) for what the
`texture` line does now.

**And a fifth, which was the same mistake one level in: "a material that
names an image knows where its texture is."** It does not. The second
real Synty mesh came out grey with the atlas declared, staged and handed
over, because that FBX named a texture by a path from the machine it was
exported on and Blender turned the unresolvable reference into an empty
placeholder image — which the skip rule read as knowledge. A reference is
not knowledge; only pixels are. Same section.

**And a sixth, from Blender 5.0: "an image whose `has_data` is true has
pixels."** It does not. The new FBX importer's placeholder for a
reference it cannot resolve says `has_data` and measures nought by
nought, while the atlas the script loads itself says the opposite — no
data yet, two thousand and forty-eight square — because it has not been
read into memory. The flag says whether Blender holds a buffer; the size
says whether there is anything in it. That was the third grey crate, and
the one the refusal did not catch, because the refusal asked the same
flag. Same section.

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
