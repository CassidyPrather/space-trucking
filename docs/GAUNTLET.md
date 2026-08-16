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
rigs: FloorLamp base plate reaches 0.0690 m out of the -0.031..0.466 m band
      every rig is composed within, so it hangs out of the box the carry
      tell wraps it in [berth-clear]
```

So the loop when a line appears is: read it in the docket to know what
moved, then run `--gauntlet` to get the millimetres.

## The six families

Each one names a class of defect that a screenshot could not have caught.
What matters when you are diagnosing is the third column: what the
violation actually looks like if you go and stand in front of it.

### `berth-clear` — no fitting stands where cargo stands

A station's furniture occupies a cell cargo may legally take, at the
height a rig occupies there. In the world: a crate drawn inside a
bollard, or a console growing through a chart tank. The finding names the
worst cell, the piece standing in it on the loaded board, and the
overlap in metres on all three axes.

Spent on cargo too, where it means something related but not the same: a
part reaching outside `pieces::RIG_NEAR..RIG_FAR`, the depth every kind
is composed within. In the world that is a body hanging out of the
wireframe box the carry tell draws round the rig you are holding. On a
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

Being deterministic is no longer the obstacle: on the counted clock, two
walks of one room print the same twenty-seven lines to the last digit and
write twenty-seven identical PNGs. What keeps the pixel half out of the
ordinary suite is that the ordinary suite has no window, and adding one
to CI is a decision about CI rather than about the harness.
