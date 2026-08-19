# The gauntlet: reading a red build

`crates/cabin/src/gauntlet.rs` is an adversarial pass over the game's
geometry. It walks every room the game has, every cargo kind, and every
doorway, and it reports what it finds as one line per violation. This
file is for the moment it turns the build red and you have to work out
what it is telling you.

The short version: **the sweep is asserted equal to a file.** If
`the_gauntlet_finds_exactly_the_docket` fails, either the sweep found
something `crates/cabin/src/gauntlet.docket` does not carry (a defect
arrived) or the docket carries something the sweep no longer finds (a
defect left, and its line has to be struck by hand). Both are red on
purpose. The design behind it is in
[ART_DIRECTION_3D.md](ART_DIRECTION_3D.md), "The gauntlet"; the code is
its own commentary; this is the operator's side.

## Running it

```bash
cargo build -p cabin

./target/debug/space-trucking --gauntlet          # the report, with numbers
./target/debug/space-trucking --gauntlet-docket   # the same, as docket lines
cargo test -p cabin                               # the ratchet, in the suite
```

`--gauntlet` prints every finding grouped by rule and exits non-zero only
on a finding the docket does not already carry — a defect somebody has
already written down is not news. `--gauntlet-docket` prints the sweep in
the docket's own `room | rule | offender` form, which is how the file is
regenerated after a fixing pass instead of transcribed by hand. Neither
needs a window, a GPU or a clock: they are arithmetic over descriptions.

The pixel half does need a window, and there is exactly one incantation:

```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json WGPU_BACKEND=vulkan \
  xvfb-run -a ./target/debug/space-trucking --gauntlet-walk <dir> --docked 7

VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json WGPU_BACKEND=vulkan \
  xvfb-run -a cargo test -p cabin -- --ignored
```

`xvfb-run` supplies the X display and lavapipe the software Vulkan
device. `--docked n` picks one of the twelve stations' rooms and
`--alongside wreck|parlor|pump` one of the three event rooms, exactly as
they do for a screenshot run. `--gauntlet-walk` writes one PNG per
waypoint plus the still and approach strips, prints a `gauntlet-walk`
line per sample carrying the pose, the mean luminance and the fraction of
the picture that moved, and exits non-zero if a frame moved while the
camera did not or the room's brightness stepped along the approach.

A single picture, for looking at what a finding is talking about:

```bash
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json WGPU_BACKEND=vulkan \
  xvfb-run -a ./target/debug/space-trucking --fixture --shot out.png --view starboard
```

## Reading a docket line

```
rigs | berth-clear | FloorLamp base plate
Guild | coplanar-faces | decor[3] Slab / Stock field (2, 1)
```

Three fields: **where**, **which rule**, **what**. The where is a
station's name, an event room's, the ship's own `cabin` or `burner`, or
`rigs` — cargo answers as its own place because the same crate stands in
every room in the game. The what is named the way its own source names
it, so `decor[3]` is the fourth fitting in that character's `decor` array
and `seam[1] stile[0]` is the first stile of the frame on port 1.

**The line carries no numbers, and that is deliberate.** It is the
identity of a defect, so a retune that moves a violation by a millimetre
does not read as a new one. The numbers are in `--gauntlet`'s report,
which prints the same finding with the measurement attached:

```
rigs: FloorLamp base plate reaches 0.0690 m out of the -0.031..0.497 m band
      every rig is composed within, so it hangs out of the box the carry
      tell wraps it in [berth-clear]
```

So the loop when a line appears is: read it in the docket to know what
moved, then run `--gauntlet` to get the millimetres.

## The eight families

Each one names a class of defect that a screenshot could not have caught.
What matters when you are diagnosing is the third column: what the
violation actually looks like if you go and stand in front of it.

### `berth-clear` — no fitting stands where cargo stands

A station's furniture occupies a cell cargo may legally take, at the
height a rig occupies there. What a berth owns is measured as how far a
rig **reaches** off its own chart, not how thick its box is: the band
begins just behind the berth plane, so a wall rig's box straddles the
chart it hangs on, and laying that thickness off the cell face put every
wall berth's air — and the occlusion window measured past it — a near
face too far into the room. In the world: a crate drawn inside a
bollard, or a console growing through a chart tank. The finding names the
worst cell, the piece standing in it on the loaded board, and the
overlap in metres on all three axes.

Spent on cargo too, where it means something related but not the same: a
part reaching outside `pieces::RIG_NEAR..RIG_FAR`, the depth every kind
is composed within — one cell of the cargo grid, wearing the same
`BAY_FIT` the width and the height wear, so a rig fills the same
fraction of its berth on all three axes. In the world that is a body
hanging out of the wireframe box the carry tell draws round the rig you
are holding. On a
**wall** kind it is also the air its berth spends, so every berth it may
take is measured too shallow and a fitting could stand in the part with
nothing to say so; on a floor or ceiling kind it is not, because a deck
berth's air is its own cell's column and the band lies flat into the
aisle a standing rig may reach into.

### `berth-seen` — nothing stands between a wall berth and the room

A fitting covering more than a quarter of a wall cell's face, from the
room's side. In the world: the window you hung is behind the station's
signage; the painting reads as a dark rectangle behind a pipe.

### `berth-reached` — every berth is workable from somewhere

There is no stance in the walk envelope from which the crosshair can
reach the berth within arm's length and inside the pitch limit, with
nothing drawn in the way. In the world: a corner of the net you can see
and cannot work. The finding names the body blocking the nearest viable
stance, because an "unreachable" with no culprit is a puzzle rather than
a work order.

### `coplanar-faces` — no two drawn faces share a plane and a facing

The general z-fight detector, and the family that finds the most. In the
world it is a surface that flickers as you walk, or a plate that is
there in one run of a screenshot command and gone in the next: two faces
at one depth, and which one draws is a question of batch order.

Three things make it a fight and all three have to hold:

- **The same plane.** Within `FIGHT_EPS`, which is `rig::layer::SKIN`
  (1.5 mm) — the thickest a flat paint riding a rung of the decal ladder
  may be, so two faces inside one skin of each other cannot be told apart
  by depth. The ladder's `STEP` (4 mm) was the other candidate and it is
  the wrong one: it is the minimum step the ladder *guarantees*, and
  using a safety margin as a detection threshold is how a detector ends
  up reporting four millimetres of daylight as a defect.
- **The same facing.** Two boxes stacked share a plane too, and that
  plane carries one face up and one face down, which the depth buffer
  settles every time. It is two faces looking the *same way from the same
  place* that has no answer.
- **A footprint.** More than `FIGHT_FOOT` (1 cm) of genuine overlap on
  both of the other two axes. Abutting is not overlapping: neighbouring
  tile fields share a plane and an edge by design, and a shared edge
  draws nothing twice.

Almost every violation found so far has been one of three shapes, and
recognising them is most of the fix:

1. **A body written down twice.** Two entries, one transform. No number
   cures it; the second entry comes out.
2. **Two members of one fitting cut to one length**, so they share a
   plane wherever they cross. The cure is that one of them ends where the
   other begins — a lintel spanning the opening between its stiles, a rim
   bar standing aside where the bar across it owns the corner.
3. **A part run out flush with its parent's own edge.** The cure is to
   stop being flush: straddle the edge (which is what a door lining is
   for), or begin somewhere the parent does not.

### `prop-points` — a rig's named features point where their names say

A sconce's cup points into the room, a floor lamp's base plate lies flat
on the deck. In the world: the sconce lighting the wall it is bolted to,
the base plate standing on edge. Both of those were real. The turn is
derived from the claim now (`pieces::Feature::turn`), so what is left for
this to catch is a claim that points nowhere — a degenerate axis or want
— and any part that goes back to being turned by hand.

### `face-fits` — a rig draws inside the cells the sim gave it

A footprint the sim states and a body the rig draws are two claims
about one object. The footprint is the law: it is what placement is
checked against, and it is what the sim answers "which piece is at this
point" with. The picture is what a player aims at, so the pick face a
standing rig carries is cut from the picture (`pieces::silhouette`) and
held to the cells — a face reaching past them would read a neighbour's
berth.

This family is the price of that holding. In the world: a piece with a
visible edge that does not answer — a crate you can see the corner of
and cannot click, a shade overhanging the berth beside it. The finding
names the part and how far outside its own `w × h` plan it reaches.

One direction is allowed and the number is written down (`SOLE_SINK`,
1 cm): a standing rig's soles are BURIED. A sole flush with the deck
shares a plane with it and a sole above it is furniture floating, so
the cabinet's four feet and the couch's four sit about seven
millimetres under their own bottom edge on purpose. Deeper than a
centimetre is a body through the deck, which is not a foot.

It asks about the plan and not the depth. The depth is `berth-clear`'s
question, measured against the band every rig is composed within; this
one is measured against the only extent the sim ever states.

### `grid-fits` — the world is built of the cargo grid and aligned to it

The owner's rule, swept. It is the newest family and it is the one that
exists because of the *shape* of four previous passes rather than
because of anything a playtest saw: each of those fixed instances of bad
placement and none of them touched the class, and the class was that the
fabric was only **mostly** on the lattice. The grid governed a room's
plan and stopped at its section (a deckhead at 4.109 cells). A wall's
thickness and a chart's trim were numbers chosen by eye (0.18 and 0.055
cells). The cabin's own hull was measured by hand two centimetres off
its own box. None of those is visible in a screenshot, and every one of
them is visible to the next thing that has to line up against it.

Two clauses:

- **Does it stand where it belongs?** A **shell** body stands inside its
  own room's cells, grown by the one wall its fabric may be proud by; a
  doorway's **hardware** stands in a doorway. Nothing is anywhere else.
  In the world this is the incinerator inside the cabin, which was
  reported three times, and the seam latch floating in mid-air with the
  wall it was bolted to punched away behind it.
- **Does the shell land on the grid?** Every face of every shell body is
  a whole number of `GRID_STEP`s — a sixteenth of a cell — from the
  lattice: cells across the plan, cells up from the room's own deck.

**The named exemptions**, which are the interesting half of any rule
like this:

1. **A doorway's hardware** — a frame, a jamb lamp, a latch, a leaf
   drawn shut and its rivets, a hatch's coaming, hinge and pull. Held to
   the first clause and not the second. A room is *constructed of* its
   deck, its deckhead, its walls and its passages, and those are what
   everything else lines up against; a hinge barrel and a twelve-
   millimetre coaming rim are bolted to the construction rather than
   part of it. Holding those to a thirty-four millimetre notch would be
   the grid deciding what things look like instead of where they are,
   which the decree does not ask for and the art direction forbids.
2. **Paint** — treads, sills, tile fields, every rung of `rig::layer`.
   Sub-cell by definition, with a law of its own
   (`pieces::tests::the_decal_ladder_never_z_fights`).
3. **Anything composed as a fraction of a box** — a station's fittings,
   a cargo rig's parts, the pendant. Those are somebody's drawing rather
   than the world's construction, and `face-fits` and `berth-clear`
   already measure them against the cells.

The family found 126 things on the day it was written. All 126 were
cured rather than docketed, in three groups: the cabin's ribs (0.7 m
apart off a start of −1.20, which is 1.27 cells off no line in
particular), the doorway's own girths (a jamb at 0.06, a shut leaf at
0.05, a latch at 0.07 × 0.16 — all now whole notches), and the punch
plane a hatch cuts with. Two of the cures produced fresh coplanar
findings, which is the grid's own hazard and worth knowing about: once
everything lands on one lattice, things that never used to line up start
to. The ribs moved onto the cell **centres** for it — an aperture's
edges land on the cell lines and a frame straddles them, so a rib on a
line is a rib sharing two faces with a stile — and the coaming's rim
went from two notches to four, because two was exactly the depth a rib
stands proud of its wall.

`the_grid_family_is_asked_about_every_room_and_answers_when_a_body_moves`
is the guard against the family going quietly toothless: it asserts that
every room has fabric to measure, that a mated doorway has a passage in
it, that a body nudged half a notch is caught, and that the whole shell
shoved a cell sideways stops being at home.

### `walk-clear` — the walked path stands in air

Every waypoint of the scripted walk is inside the walk envelope, and
nothing hung is standing in the eye. In the world: you walk into a
fitting, and a still shot from that pose renders as an interesting
abstract.

## The ratchet, and why `ALLOWED` is empty

The docket is a **work order**, not a baseline and not an allowlist.
`the_gauntlet_finds_exactly_the_docket` asserts set equality, so:

- a new defect fails the build, with the whole report in the failure;
- a fixed defect *also* fails the build, until somebody strikes its line.

That second half is the point. A baseline nobody strikes lines out of is
a list that only grows, and the fixing pass is the thing this exists to
provoke. The working loop is: strike a line, run `cargo test -p cabin`,
and let the sweep tell you whether the thing is actually gone.

`ALLOWED` is the other mechanism entirely — pairs the coplanar detector
is *wrong* about, each with the reason. **It is empty, and its emptiness
is a finding.** Every case that has come up so far has been answered by
making the detector truer rather than by forgiving a pair. Concentric
bodies were the honest candidate: a firebox glass inside a firebox drum,
a collar round a pipe, a boss cut round a post from one centre. They
share a box and no face at all, because a cylinder meets each of the four
planes round its flank along a single line and a sphere meets all six at
a point — so `Faces` learned which sides of a body's box are really
faces, and the pairs stopped being reported. The same argument later
taught it that a body has to land *squarely* on a world axis before its
box has six faces, which is what a leaning chevron and a vial standing on
its corner need.

If you are about to add a line to `ALLOWED`, the reason has to be about
the geometry and not about the number. A loosened threshold stops finding
the thing the detector was built for.

## The clock, and what happens without it

A screenshot is only evidence if it reproduces. These did not: two runs
of one `--shot` command used to disagree over anywhere from 1% of the
picture to 57% of it. Not because the art moved — because the shutter
fired at a frame *number* while everything in the frame read the wall
clock, so frame 46 landed wherever the container's shader build happened
to leave it.

A judged run therefore takes the wall clock away from the game.
`main::FRAME_STEP` pauses `Time<Virtual>` and advances it by exactly one
tick per frame, so frame N is at N × step whatever the machine did to get
there; the step is the sim's own `TICK_DT` to the last bit an `f32` has,
so one rendered frame is exactly one simulated tick
(`a_pinned_frame_is_one_tick_of_the_sim`). `Bridge::steady` takes the
same world off the absence replay, so a slow shader build is not read as
the player having been away.

Two more things fail without them, and both were observed:

- **The scene goes missing.** Bevy builds pipelines off the frame loop
  and draws without the meshes whose pipelines are not built yet — the
  right trade for a game, the wrong one for a shutter. Under parallel
  load, one shot in eight came out as the clear colour with the whole
  scene absent. A judged run sets `synchronous_pipeline_compilation`,
  which costs it a slower start and nothing else.
- **The proof gets expensive.** With the clock loose, a refactor that
  changed nothing had to be proved with a listing of every entity in the
  world, because the two pictures were never going to agree.

`the_same_view_shot_twice_is_the_same_bytes` is the guard that keeps it
there. It shoots four views twice each and compares bytes: one from
inside the ship, one from outside it (the star field and the sky clock
have the furthest to drift), and the two that used to be bimodal —
`starboard` and `parlor`, whose amber seam latch was drawn twice at one
transform and came out lit in five runs of six and gone in the sixth.

**One thing still reads the OS clock at boot**, and it is why there is no
checked-in golden image. `bridge::local_night()` asks whether it is deep
night on the machine's own clock, and every boot — the fixture included —
carries the answer. A fixture shot at 02:00 is a night-market shot. Two
shots taken in the same run of an agent agree; a picture committed to the
repository and compared next week does not, and would fail for the one
reason that is not a defect. So the reproducibility guard compares a pair
taken now, and never a pair separated by a night.

**The guard covers four views, and a fifth is known not to hold.** Shot
twice each on an idle machine, thirteen of the fourteen named viewpoints
come back byte-identical; `flank` does not. It disagrees with itself over
about 213 pixels of 921,600 — two hundredths of one per cent of the
frame, a couple of levels of grey apiece. Whatever drifts there is small,
it is not the seam latch that made `starboard` and `parlor` bimodal, and
nobody has chased it down. Until somebody does, a refactor proved by
shooting the same view before and after should read a difference that
size on `flank` as the noise it is, and a difference anywhere else as a
finding.

**The bottom-right corner changes with every commit.** The version text
`rig::spawn` draws is `git describe` output, so two shots of one view
taken either side of a commit — or with the tree merely dirty — differ in
that overlay and nowhere else. A before-and-after comparison that spans a
commit has to read the frame with that corner excluded, or it reports a
change on every one of them.

## What the harness can see, and what it cannot

This is the most useful section when the build is *green* and something
is still wrong.

**It sees descriptions, not the world.** Everything it measures comes
from a pure function that also builds the thing: `poi::character_of` for
a station's fittings, `room::shell_boxes` and `rig::structure` for the
fabric, `pieces::parts` for a cargo kind, `room::seam_parts` for a
doorway. Anything spawned outside one of those descriptions is invisible
to every rule in the file, however loud it is on screen.

That has now been the cause three times, and the shape is identical each
time:

| Layer | Was invisible because | Closed by | Found |
| --- | --- | --- | --- |
| Rooms | nothing; this is where the harness started | — | three families, one cause (a calling room owned no deck of its own; `Tile::Staging`) |
| Cargo | `pieces::build_kind` composed a kind straight into a live Bevy world, so nothing pure could enumerate a rig's parts | `pieces::parts`, the description the builder now stamps | 61 findings across 32 kinds |
| Doorways | `room::doorways` composed stiles, lintels, lamps, latches, leaves, coamings and treads straight into the world | `room::seam_parts`, ditto | 53 findings, and the bimodal screenshot |

The lesson worth carrying: **the next defect is in the layer nobody has
thought to describe yet.** Each of these was invisible not because a rule
was too loose but because there was nothing for a rule to be asked about.
Before trusting a green sweep, ask what is on screen that nothing in the
list above enumerates.

Other things it deliberately does not see, each for a stated reason:

- **The exterior dressing.** It hangs in the void by law, and
  `poi::tests::a_character_stays_inside_the_room_it_dresses` holds it
  there. Nothing outside a room can stand in one of its berths.
- **Rim marks and the decal ladder's own rungs.** Their spacing is the
  ladder's law and `pieces::tests::the_decal_ladder_never_z_fights`
  already holds every rung of it. The sweep asks about paint only where
  something *hung* fights it.
- **Fabric against fabric.** Same reason. The coplanar detector only
  reports a pair where at least one side is something somebody hung.
- **Staging cells.** A room that leaves owns the deck it lends the
  player between one launch and the next; a crate clipping a bollard
  while it waits there is the owner's explicit ruling of nobody's defect.
  What is still checked is that a room cannot *fence off* its own
  staging, which would strand a crate and hold the launch forever
  (`a_stations_dressing_is_not_in_the_aiming_path`).
- **Anything about colour, taste, or composition.** It measures shapes.
  A room can pass every rule here and look like nothing.

And two structural blind spots that are worth knowing about because they
are not closed, only bounded:

- **A pose is one pose.** The roster mates every station at the first
  door the spawn walk takes, deterministically, because a sweep that took
  a minute would not be run. The rules are all measured in a room's own
  frame, so the pose is not what any of them is about — but a defect that
  needs a *particular* attachment order to appear would not show up.
- **The pixel half is opt-in, and CI has no window at all.**
  `.github/workflows/ci-cd.yml` installs no `xvfb` and no Vulkan ICD, so
  the flicker and light-pop detectors run only when a human or an agent
  runs them.

## Where the pixel half stands

The two pixel detectors are steady but not equally ready to be asked for
on every commit, and the numbers are recorded rather than guessed:

- **Flicker** — over seven walks of three room kinds, the worst sample
  came in at 0.00017 of a picture against a `FLICKER_TOL` of 0.02. That
  is 118× of headroom, and it is the family that would have caught the
  every-other-frame lamp the playtest found.
- **Light pop** — the same walks reached 0.155 against a `POP_TOL` of
  0.18, which is 86% of the way to red: one re-hung lamp from failing.
  Steady is not the same as safe. This family wants either more headroom
  or a tolerance argued from a room rather than from a sample before it
  gates anything.
- **A dark room** — the one reading that asks whether anything drew at
  all. Every `--shot` now prints its frame's mean brightness and, beside
  it, the fraction of the picture standing clear of the ground
  (`gauntlet::READ_FLOOR`, a luma of 0.10):

  ```text
  shot path=out.png lum=0.04500 read=0.06889
  ```

  `lum` alone cannot answer the question — pure black is banned, so the
  darkest frame the game can draw still means out around 0.037 and a
  room going from unreadable to readable moves it by a fifth. `read`
  answers it: the furnace with its fire out sat at 0.0016 before its
  tape carried the lights-out floor and at 0.069 after, and the 0.0016
  was the version corner and the crosshair. `ROOM_READS` (0.02) is the
  alarm, and the guard that spends it is
  `a_furnace_with_its_fire_out_is_still_a_room_you_can_find`.

Being deterministic is no longer the obstacle: on the counted clock, two
walks of one room print the same twenty-seven lines to the last digit and
write twenty-seven identical PNGs. What keeps the pixel half out of the
ordinary suite is that the ordinary suite has no window, and adding one
to CI is a decision about CI rather than about the harness.
