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

## The map: what a rule can be about

Twelve of the sixteen families below were written the same way. Somebody
played the game, saw something wrong, and a family was written that would
have caught it. That is a harness which is always exactly one round
behind whoever is playing, and the owner said so:

> "It's becoming increasingly clear that this strategy — try to patch a
> class of defects, fail, catch a batch during user integration test,
> repeat ad-nauseam — just isn't really the best."

The eighth blind spot is the proof that something better is available.
Eleven families asked whether a body **exceeded** something — its cells,
its plan, its depth band. Not one compared a body against its own rect's
*position*, so a depth band anchored to a wall's plane stood 0.42 of a
cell forward of every deck berth in the game while all twelve stayed
silent. Nobody had to find that by playing. It was a hole in a space
that can be written down.

So write the space down. Every rule in this file is a **triple**: a
**body**, a **relation**, and the **frame** the relation is read in.
Enumerate the three axes, dismiss the triples that mean nothing, and what
is left is a list of questions — the ones that are asked, the ones that
are not, and the ones that cannot be asked because a description does not
exist yet. The last kind is the most valuable thing on the list, because
an unaskable question is this codebase's first standing lesson said from
the other side: *a new family of thing gets a description before it gets
a mesh.*

### The bodies

Only what something **describes** can be measured — that is this file's
oldest lesson and it bounds the table before it starts.

| Body | Description | Since |
| --- | --- | --- |
| a cargo rig, whole | `pieces::berth_box`, `pieces::berth_pose` | the cargo layer |
| a rig's part | `pieces::parts` (`Part::seated`, `Part::body`) | the cargo layer |
| a rig's named feature | `pieces::features` (`Feature::axis`, `want`) | `prop-points` |
| what a rig answers the aim with | `pieces::standing_surface`, off `drawn_box` | the tell layer, and read by no family — see below |
| a highlight's outline | the mask a rig's parts draw (`outline::MaskProxy`) and the bands `outline` paints off it | the tell layer, and read by no family — see below |
| a room's shell | `room::shell_boxes`, `rig::structure` | the room layer |
| a doorway's hardware | `room::seam_parts` (`room::Seat`, `Dress`) | the doorway layer |
| a station's furniture | `poi::character_of` (`poi::Seat`, `Shape::fill`) | the room layer |
| a room's own worked hardware | `room::handshake_face`, `Dress::Grab` | `fixture-reached` |
| the painted fields | `gauntlet::tile_fields`, off `RoomKind::tile_of` | the room layer |
| a berth | `gauntlet::berths`, off the sim's arbiter | the room layer |
| the player | `rig::EYE_HEIGHT`, `REACH`, `PITCH_LIMIT`, `room::walk_boxes` | the room layer |

### The frames

| Frame | What it is |
| --- | --- |
| the rig's own upright | `cargo::Kind::upright`, which no berth turns |
| the chart | a `SimSurface`'s `u`, `v` and inward normal |
| the room's box | `poi::Frame`, `Placed::lo`/`hi` |
| the lattice | `GRID_STEP`, a sixteenth of a cargo cell |
| the world | metres, axis-aligned |
| the eye | a stance, its reach and its pitch limit |
| **the net** | the room in CELLS: tile classes, ports, doorsteps |

The seventh was missing from the first cut of this list and it is where
the second of the owner's two open items turned out to live. Every other
frame is measured in metres; the net is measured in cells, it is the
sim's own frame, and a rule read in it needs no geometry at all.

### The relations, and what each one is worth asking of

Twelve relations against ten bodies against seven frames is 840 triples,
and almost all of them mean nothing. What follows is the whole space with
the meaningless dismissed in a line, so that the short list at the end is
the interesting one.

| Relation | Asked as | Of what, in which frame |
| --- | --- | --- |
| **is contained by** | `berth-clear`, `face-fits`, `grid-fits`, `walk-clear`, `poi`'s containment test | a rig in its band; a rig in its plan; a shell in its room's cells; the player in the envelope; a fitting in the room's box |
| **fills** | `berth-filled` | a rig against the ground its plan owns |
| **is centred in** | `berth-filled` | the same, one clause up |
| **rests on** | `rig-seated` | a rig against the chart its berth promises |
| **meets** | `part-seated`, `furniture-seated` | a part against a part; a hung body against its declared seat |
| **shares a plane with** | `coplanar-faces` | anything drawn, against anything drawn, in the world |
| **is aligned to** | `grid-fits` | a shell body against the lattice |
| **faces** | `prop-points`, **`berth-turned`** | a feature in its rig's frame; **a rig in its chart's and its room's** |
| **occludes** | `berth-seen`, **`fixture-seen`** | a fitting against a wall berth; **a fitting or a stocked berth against a control** |
| **is reachable from** | `berth-reached`, **`fixture-reached`**, **`deck-reached`** | a berth from a stance; **a control from a stance**; **the deck from a door, in the net** |
| **is walkable past** | `walk-clear` | the scripted walk's waypoints against what is hung |
| **is beside / is above** | nothing, and nothing should | composition, which is the art direction's and not a measurement |

### The cells that were meaningful and unasked

Four, and all four are now asked. Each was invisible for a different
reason, and the reasons are worth more than the fixes.

| Triple | Why nothing asked it | Now |
| --- | --- | --- |
| a rig **faces** its chart and its room | every family measured a rig as an axis-aligned BOX, and a box is the same box after a half turn — and after a quarter turn whenever the footprint is square. The turn is the half of a pose no box carries. | `berth-turned` |
| the deck is **reachable from** a door, in the net | every rule in this file measures metres. The one frame with no geometry in it had no rule in it either. | `deck-reached` |
| a control is **reachable from** a stance | a handshake is not a cell of the net — `RoomKind::surface_of` punches a hole where it stands — so `berths` never produced one and no rule about berths was ever about one. | `fixture-reached` |
| a control is **occluded by** what a room hangs | `berth-seen` reads one way. Its subject is a berth and its culprit is a fitting, and nobody wrote the sentence with the nouns swapped. | `fixture-seen` |

### The cells that are meaningful and cannot be asked yet

These are the descriptions that do not exist. Each is a real question a
player could answer by looking, and none of them can be put to anything
in the tree today.

| Triple | The description that is missing |
| --- | --- |
| a station's fitting **faces** the room | A `Fitting` declares a shape, a coat, a position and now a seat. It does not declare a **front**. So "the bench faces the wall" is unaskable, and the day a purchased asset replaces a `Cuboid` it will be the first thing anybody notices. The fix is a `poi::Fitting::front`, declared, and read back exactly as `prop-points` reads a `Feature`. |
| a rig **has** a front at all | Same hole one layer down, and it is why `berth-turned` deliberately does not ask about the turn of a square-planned body in the middle of a deck. A `CeilingLamp`'s mount plate is 9 × 5 and the rest of it is a body of revolution; nothing says which parts of a kind have an orientation worth reading. |
| a fitting **rests on** another fitting *it does not name* | `furniture-seated` reads a **declared** seat and refuses to guess, for the stated reason that guessing would report every bollard that happens to stand near a wall. The Wanderer's fourth collar hangs on nothing on purpose. What is unaskable is the inverse: nothing says a body was *supposed* to declare one, so a fitting that quietly hangs in mid-air with no `Seat` is legal and invisible. A `poi::Fitting::floats` — an explicit "this hangs on nothing" — would turn the silence into a claim. |
| a body **is walkable past** | `walk-clear` asks about the waypoints of one scripted walk. Whether a room is *traversable* — whether a body can get from any stance to any other without passing through a fitting — has no description, because nothing describes a body's own width against a gap. `rig::HEAD` is half a head at eye height and it is spent on the walk's own samples. |
| a shell body **meets** another shell body | Bounded rather than closed: `grid-fits` puts every face of every shell on a sixteenth of a cell, so a gap in the fabric is at least 34 mm and reads as a slot of void in every screenshot of that wall. It is a defect a still CAN see, which is this file's own dividing line. |
| paint **fills** its own cells | Not missing — **derived**. `tile_fields` and the runtime's own painter both cut a field from `layout::cell_rect`, so there is no number a builder could get wrong, exactly as `pieces::laid_on` leaves a covering nothing to get wrong. |

### The cell that was asked, measured, and taken back out

Worth recording as loudly as the ones that stuck, because it is the only
answer this method gave that was *no*.

**A berth's air crossing a control.** `fixture-seen` was first written to
ask it of every berth, not only of the ones a room fills, and it fired at
once: the cabin's own seam latch is crossed by three — (5, 1) on the aft
wall beside the jamb and (5, 3) and (5, 4) on the deck in front of it —
the worst standing across **100%** of the amber. Nothing is wrong with
the latch. Every wall cell beside an aperture is a berth and every deck
cell in front of one is a berth, so a control bolted beside a doorway
shares air with a berth *by construction*, and there is nowhere to move
it to: the frame's own cells are the aperture and everything else is
somebody's berth. The rule would have forbidden the latch.

So it asks about what the room hangs and what the room **stocks**, and
the residue — a player standing their own crate in front of their own
latch — is in the bounded blind spots below. It is the same narrowing
`berth-clear` spends on a jamb and `berth-reached` spends on furniture,
and it is written down with the numbers rather than assumed.

## The sixteen families

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
point" with. The picture is what a player aims at, so the pick body a
standing rig carries is cut from the picture (`pieces::drawn_box`) and
held to the cells — a face reaching past them would read a neighbour's
berth.

This family is the price of that holding. In the world: a piece with a
visible edge that does not answer — a crate you can see the corner of
and cannot click, a shade overhanging the berth beside it. The finding
names the part and how far outside its own `w × h` plan it reaches.

One direction is allowed and the number is written down (`SOLE_SINK`,
1 cm): a rig's sole is BURIED. A sole flush with the deck shares a
plane with it and a sole above it is furniture floating, so the
cabinet's four feet and the couch's four sit a ladder step under their
own bottom edge on purpose — `pieces::SOLE_BURY`, which is
`pieces::GLAZE` one plane down, because meeting a deck and meeting a
bezel are the same joint. Deeper than a centimetre is a body through
the deck, which is not a foot.

**The sole is whichever face meets the chart**, and the allowance
follows it rather than following gravity: a pendant's canopy is buried
in its deckhead by the same step, so the ceiling lamp's mount plate
spends the centimetre upward. Which face that is comes from the charts
the kind may be berthed on, which is the arbiter's list
(`cargo::mount_accepts`) and not this file's.

It asks about the plan and not the depth, and it asks in only one
direction. The depth is `berth-clear`'s question, measured against the
band every rig is composed within; this one is measured against the
only extent the sim ever states. Whether the sole gets to its chart at
all is `rig-seated`'s, and the two are the same joint read from its two
sides.

### `part-seated` — a part that names a seat meets it

The first family that measures a rig against **itself**. Every other family asks about a part and the world: the band
it is composed within, the plane it fights, the cells it draws inside,
the direction its own name claims. A joint is not any of those. A
couch's foot standing under a couch it does not touch is inside the
band, shares no plane, draws well within its cells and claims no
direction — it satisfies all eight of the others and it is four stilts
of air.

The claim is **declared** on the part that makes it
(`pieces::Part::seated`) and read back off the rig, exactly as
`prop-points` reads a direction. A sweep that tried to infer joints
instead — "these two are close, they probably meet" — would report every
crate standing near its own lid and would say nothing about the one part
whose name is a promise. A part that is composition declares no seat and
is asked nothing, which is why `ALLOWED` needs no entry here: there is
nothing to forgive, only things nobody claimed.

Two readings, one tolerance:

- **Daylight.** The gap between a part's box and its seat's, on the
  widest axis. `SEAT_GAP` is one fight-free step of the decal ladder
  (`rig::layer::STEP`, 4 mm) plus the thickest paint that could be
  riding on the seat's own face (`SKIN`, 1.5 mm). The step is the FLOOR
  of a joint rather than its ceiling — two bodies meeting on one plane
  is a coin toss in the depth buffer, so a joint that has to read as a
  joint stands a step off instead of none, and `pieces::GLAZE` is that
  step said in the units a rig is composed in.
- **A name nothing answers to.** A seat naming a body the rig does not
  draw is reported as its own finding.

Several parts may answer to one name: a pane glazed behind four lips
meets whichever lip it reaches, so the reading is the smallest gap to
any of them.

Its first pass found three joints, all of them a shade under a
centimetre and all of them invisible to a screenshot because a
centimetre at furniture scale is a hairline you read as shadow. The
couch's four feet stopped 8.7 mm short of the seat they carry. The
destination preview's glass floated 9.3 mm off the bezel it is glazed
into — and only in the build people look at, because the headless
fallback body is a slab that overlapped the brass. The porthole's sky
pane missed its own bolt ring by the same 9.3 mm. All three are now
drawn with the joint spent explicitly: a step in, or a step proud.

### `rig-seated` — a rig reaches the chart it is berthed on

The newest family, and `part-seated`'s closest sibling: the same joint,
one plane down and one body out. That family asks whether a part meets
another part of the same rig. This one asks whether the rig meets the
one thing outside it that a berth actually promises — the deck it
stands on, the deckhead it hangs from, the wall it is screwed to.

Nothing caught it. `berth-clear` measures the depth a rig is composed
within and never where inside that depth a body stops. `face-fits` is
the nearest miss and looks the other way on purpose: it catches a body
reaching *outside* its own plan, forgives a centimetre of a sole buried
in its own deck, and asks nothing at all about a sole a hand's breadth
above one. `part-seated` is part-against-part. So a crate that stops
seven centimetres above its own deck cell satisfies all ten of the
others and is a crate standing on nothing.

**Which plane is a question about the chart and never about the body.**
A kind is composed once, in the upright frame no berth turns, and
`pieces::site_on` berths it: a deck berth stands the rig half its own
height above the chart, a deckhead berth hangs it half its height
below, and a wall berth lays it flat on the plane. So the chart is at a
known plane of the rig's own frame — `-tall/2` for a deck, `+tall/2`
for a deckhead, `z = 0` for any of the four walls — and the face that
has to reach it is the sole, the canopy and the back respectively. The
sweep is written per chart class rather than per mount for that reason,
and which classes a kind may take is asked of the arbiter
(`cargo::mount_accepts`) rather than restated here.

The tolerance is `SEAT_GAP`, the same number `part-seated` spends,
because it is the same joint: one fight-free step of the decal ladder
plus the thickest paint that could be riding on the seat's own face. A
chart's face carries paint too — a tile field, a class's mark — and a
rig meets it the way a pane meets a bezel, by going a step into it
(`pieces::SOLE_BURY`), so a builder spending exactly that sits inside
the rule with room to spare. `SOLE_SINK` holds the other side. Between
them a rig's sole has a band to land in, and the band is a hair either
side of the chart.

A laid covering is not asked. `pieces::laid_on` lays it ON its chart
and lifts it one rung of the decal ladder, so its joint is a derivation
rather than a composition and there is nothing a builder could have got
wrong. What a berth STANDS is what is asked.

Its first pass found twenty-two. Two came off at once, and both were
the same shape as the wall lamp's pad: a fitting composed at the middle
of the band instead of at the plane it is bolted to. The ceiling lamp's
canopy stopped 8.4 mm under the deckhead it is screwed to. The
porthole's whole assembly — glass, bolt ring and six studs — hung
32.6 mm out in front of its wall, with the room's own paint visible
under the brim, which was the last of the nine wall kinds to put its
backmost body anywhere but on the plane. The other twenty were one
class: the 2D console's glyphs, given depth and never given a floor,
standing between one and fifteen centimetres above their own deck cells
— the re-authoring docs/BAY.md had been carrying since the net landed.
They are all drawn standing now, and what it took is worth knowing
because the obvious cure was the wrong one.

**A flat kind lies on a deck; it does not stand on one.** Translating
twenty bodies down cures twenty findings and makes two of the pictures
worse: a transit chit or a casino chip settled onto the deck standing is
a playing card balanced on its edge. The height a glyph had was a
drawing on a flat console, and reading it as a standing pose is the
defect — so the question per kind is what pose the object takes on a
floor, not how far down it goes. Eleven were posed right already and
wanted settling. Seven were LYING DOWN, because a glyph reads a cylinder
end-on as a circle and a cone end-on as a disc, and stood up: a vial, a
plant pot, a cryo core, a shard of comet, a bottle of midnight, and both
paints' tins. Two lay down. Where a sole lands is one function now
(`pieces::sole_of`), read off the plane `pieces::site_on` stands a deck
berth on.

Two things the settle dragged along, and both were invisible while every
kind was composed centred in its own cell. The carried ghost hung a
rig's ORIGIN at the point the crosshair struck: a body centred on its
origin sat half in the deck and read as roughly resting on it, and a
body drawn wholly above its origin sits wholly under it. The hover reads
the berth's own stand-off now (`pieces::hover_pose`), so the ghost
promises the berth's position as well as its turn. And the sweep that
aims at each corner of a drawn body drew the aim back toward the rig's
origin, which is the body's own middle only while the body is centred
there.

### `furniture-seated` — a hung body meets what it says holds it up

`part-seated`'s question and `rig-seated`'s tolerance, asked of the one
layer of the world that had no way to answer it. A rig declares the chart
it is berthed on because the sim berths it. A station's `Fitting` is a
fraction of a room's box and a doorway's hardware is world units off a
site, and until this family neither of them declared **anything at all**
— so a beacon bolted to thin air and a latch floating in front of a wall
were invisible to every rule in this file, and both were found by a
player looking at the screen.

Four defects of one shape had been found by the time it was written, and
three of the four by eye rather than by the harness:

| what | how far off its surface | how it was found |
| --- | --- | --- |
| the wall lamp's mount pad | spanned a band no other wall kind's began in | the owner |
| the porthole's back | 32.6 mm in front of its wall | `rig-seated` — a rig, so it was catchable |
| the Guild's seizure beacon | 0.58 m off the aft wall, 0.42 m under the ceiling | the owner |
| the seam's detach latch | 0.0931 m off the wall it screws to | the owner |

**The claim is declared and then checked, never guessed at**, which is
the same reason `ALLOWED` needs no entry for `part-seated`: there is
nothing here to forgive, only things nobody claimed. A sweep that
inferred joints would report every bollard that happens to stand near a
wall — and it would be *wrong* about the things that are meant to hang on
nothing. The Wanderer's fourth collar has nothing under it and nothing
through it, and its three hum rings "hang on nothing either"; they stay
legal by saying nothing, and a rule that forced everything to touch
something would have to be lied to about them.

Two vocabularies, because the two layers are composed in different
frames, and one reading:

- A station writes `poi::Seat` — `Face(..)`, one of the six sides of the
  box the fitting is measured off (for `Character::decor` that is the
  room's own, floor to deckhead), or `On(..)`, another fitting by the
  name it declares with `Fitting::called`.
- A doorway writes `room::Seat` — `Plane(..)`, the surface a piece of
  hardware bolts to as a point on it and the way the part reaches, or
  `On(..)`, another part of the same doorway by its own `what`. Any part
  whose name *begins* with the claim answers to it, so a body drawn
  several times over (`leaf[0]`, `leaf[1]`) is named once.

Several bodies may answer to one name and the reading is the smallest
gap to any of them; nothing holds itself up, so a body sharing a name
with its seat is looking for the nearest *other* one. A name nothing
answers to is its own finding.

**The face a fitting names is the room's own box**, which is the same
box the containment law means by "inside this room" — so a fitting can
be flush with a face and never through one, and the family only ever
reads daylight. A doorway's hardware is measured in world units and is
held to the plane the room actually *shows*, which is its chart
(`room::chart_inset`, a notch inside the box face) and not the box face
the frame straddles. The two differ by that notch, and the difference is
why: a 20 mm latch plate screwed to the box face is a plate behind the
wall's own paint, and the coplanar detector said so the first time it
was seated there.

Two hundred and sixty-four claims are declared on its first pass, across
fifteen stations and every doorway in the game, and curing what they
found moved 133 of the 292 fittings those rooms hang, every rivet on
every shut door, and one bracket that was not there at all — the
Hermitage's third ledge had no corbel under it. The shapes repeat: four
rivets on each of three shut leaves drawn a plate thickness out past the
**back** of the plate they fasten, where nobody in the room could see
them; three seized pendants sharing one stem length at three heights,
two of them hanging off nothing; a candle 60 mm over the sill it stands
on; three cryo cores 55 mm under their rail and five blackout tins 56 mm
under theirs; a stack of hull plate 105 mm off the deck; a main riser
that runs "deck to cornice" starting 176 mm above the deck; three
pillars "floor to deckhead" that reached neither; and a dozen plates,
patches, plaques and boards standing between 12 and 77 mm off the walls
they are bolted to. The ten that were left are gone too: five were the
comet's quarried face, re-composed as one body with steps left standing
on it, and five were the vocabulary defect below.

**Five of those ten were a finding about the vocabulary rather than
about a station**, and they are the most useful thing this family has
found. Five hoops laid flat "on the deck" — a moon pool's collar, a core
cradle, three coiled hoses — could not reach it and could not be made
to. A `Shape::Ring`'s drawn tube is 18% of the frame `Fitting::half`
declares, and the containment law was measuring the frame, so a hoop
whose tube met the deck had a frame a tenth of a room deep in the floor
and no number a station's author could write cured it.

The fraction is stated once now (`poi::Shape::fill`), `Fitting::span`
reads the body through it — a claim on space belongs to the thing that
occupies it — and the sweep cuts its own boxes from the same fraction
instead of carrying a second table. `Fitting::meeting` is the other
half: a hoop is placed by naming the face it lands on rather than by a
decimal its author worked out from the fraction.

**The other direction was tried and measured, and it prices worse.**
Scaling a torus up until its tube filled its frame would make `half` the
body's half-extents outright and need no new reading at all — but every
frame in the game was authored against today's wafer, so every hoop
became five and a half times thicker: eleven waypoints of the scripted
walk ended up inside a lamp cage, six wall berths went behind a hoop and
three more were stood in. Twenty new findings to cure five, and it is
recorded here because "make the body match the claim" is the tidier
sentence and the wrong one.

`the_furniture_family_is_asked_about_every_room_and_answers_when_a_body_lifts_off`
is the guard. It counts the claims (every room declares some, and both
kinds are spent), refuses a name nothing answers to, and then lifts every
seated body in the game a hand's breadth off the thing it names, in the
direction its own claim points, and requires the reading to turn from a
joint into daylight.

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

### `berth-filled` — a rig fills the cells its berth spends

The twelfth family, and the one that closes the gap between the two
claims a berthed piece makes. The sim states a footprint and the cabin
draws a body, and every rule before this one asked whether the body
stayed **inside** something. `face-fits` holds a rig to its own `w × h`
plan and forgives everything short of it; `berth-clear` holds it to the
depth every rig is composed within and forgives everything short of
that; `rig-seated` asks about the one face that has to touch a chart and
nothing about the other five. **Not one of them asks where inside its
berth the body actually is** — so a rig could sit hard against one edge
of the cells it was given, or half out of them on the axis nothing
measured, and stay green in all eleven.

That is what it did, on every deck and every deckhead berth in the game.
A rig's own `z = 0` is the **berth plane** and its body is composed from
just behind it to one cell out into the room (`pieces::RIG_NEAR`,
`RIG_FAR`) — which is the truth on a wall, where the plane is the chart
the rig is screwed to and the room is in front of it. A deck berth has
no such plane. Its rect is a **plan**, the cells own the ground on both
sides of their own middle, and the band laid off a plane that is not
there stood every deck and deckhead berth 0.2329 m out into the aisle —
0.42 of a cell, on the one axis the plan spends its depth on and never
on the other. (The bodies composed inside that band came out 0.117 m to
0.250 m off, kind by kind; the band is what a berth costs and the band
is what this measures.) That is the shape the owner reported four times
— "it's like, half-way between cells on one axis" — and it is why the
axis was the load-bearing half of the report.

**It asks about the two axes the rect pays for and not the third.** A
berth's rect spends two of a kind's three extents and the chart fixes
the other (`cargo::Kind::plan_on`): a deck berth spends across by deep
and the deck fixes the height, a wall berth spends across by tall and
the wall fixes the depth. What the chart fixes is `rig-seated`'s
question from one side and `berth-clear`'s from the other, and on a wall
it is deliberately off centre — the band begins a hair *behind* the
plane, so a rig's back sinks into the paint it is screwed over. What the
cells pay for is nobody else's question, and on those axes a body is
centred or it is misplaced.

Two clauses and one reading:

- **Where.** The box a berth spends is centred on the ground its plan
  owns, to within `GRID_EPS` — the same millimetre `grid-fits` calls a
  face on its line, because this is the same question one layer up. In
  the world: the berth wells light under a carry and the crate is not
  in one.
- **How much.** And it is `pieces::BAY_FIT` of that ground, which is the
  one margin a rig wears, said on the axes a plan spends. A body
  claiming ground it does not fill is a berth measured in the wrong
  place, which is then what `berth-clear` tells a station's furniture
  about.

The finding is filed under `rigs` and keyed by kind and chart class,
because the same crate stands in every room in the game and a defect in
how a deck berths it is not fifteen defects.

`the_fill_family_is_asked_about_every_berth_and_answers_when_a_body_slides`
is the guard: every chart class is actually measured, every berth in the
game reads centred and full, and the reading itself is put to a body
that has moved — the same box slid half a notch along its chart, and the
same box shrunk to half the ground it claims, both have to stop reading
as a berth filled.

### `berth-turned` — a rig stands up and shows the room its face

The thirteenth family, and the first of four written from the map above
rather than from a sighting. It closes the half of a berth's pose no box
has ever carried.

**Every rule before it measured a rig as an axis-aligned box.** A box is
the same box after a half turn, and it is the same box after a quarter
turn whenever the footprint is square — so the whole of "which way is it
looking" fell through eleven families, and the twelfth caught only the
quarter turns a non-square plan pays for. `prop-points` is the nearest
thing there was and it looks one body in: it asks whether a sconce's cup
points where the word "cup" says, **inside the rig's own frame**, and a
rig hung backwards carries every one of its features faithfully backwards
with it.

Two claims, and between them they pin the turn on every chart the game
has:

- **It stands up.** A rig's own up is the room's up. On a wall that is
  the upright rule's whole purpose (`pieces::wall_upright` rolls a
  chart's lie back onto the room's); on a deck and under a deckhead it is
  what "standing" and "hanging" mean. A quarter turn about the face
  normal breaks it and so does an upside-down one, which makes this the
  clause that catches a **square** footprint — the one case `berth-filled`
  is blind to by construction, because a square plan's world box is the
  same box either way round.
- **It shows its face to the room.** On a wall, the face a rig turns to
  the room is the wall's own inward normal. On a deck or under a deckhead
  there is no such normal, so the claim is the player's instead: **the
  deck a standing rig is looking at is deck of the same room.** A couch
  with its face in the front wall is the defect, and it reads as one
  without this file ever learning the backing rule's branches.

That last point is the whole reason the family is worth trusting. A sweep
that recomputed `pieces::floor_facing` and compared it with itself would
pass two thousand berths and mean nothing — which is a mistake this file
has already made once (see `the_body_hangs_true`, below).

Its first pass found **every deckhead berth in the game**. A deck took
the backing rule and a deckhead took one fixed turn, facing the front of
the room from every cell of every ceiling whether or not the front of the
room was a hand's breadth away — the couch-facing-the-wall defect stood
on its head, and above eye level where nobody looks. A pendant takes the
backing rule now. Mid-room the rule's own default *is* the turn that was
hardcoded, so nothing away from a seam moved.

**What it deliberately does not ask** is the turn of a square-planned
body in the middle of a deck. A crate there may face any of four ways and
every one of them is a room a player can walk round; which one is
composition, and composition is the art direction's. That is a real hole
and it is bounded by the missing description in the map above: nothing
says which kinds have a front.

### `deck-reached` — the deck you may use is walkable to from the door

The fourteenth, and the first rule in this file that is about a room's
**net** rather than about anything drawn in it. Every other rule measures
metres; this one counts cells, and it counts them through the sim's own
declaration (`RoomKind::marooned`) rather than through a second one of
its own — the cabin may not restate a sim rule, and *which cells the
player may use* is as sim a rule as there is.

Two clauses:

- **A door stands on deck a body may use.** Every cell of a declared
  door's own step takes the player's cargo.
- **And nothing is walled off behind it.** From that step, every cell of
  the room's deck the player may use is reachable across such cells
  alone.

It exists because the owner walked a defect the entry-path law read green
for. That law clears a chalked band out of the straight run in from a
door, and it holds. What it is about is a **lane**; what the owner was
doing was a **journey**, and the journey's first step landed on the
shopfront:

> "Users have to walk straight through the station's offer area to get to
> the place to put their own items."

`Trade` and `Wreck` hang their goods along the wall they present to
whatever they came alongside, which is the wall their one door is punched
through. So the two deck cells a body lands in on the way in were
`Tile::Stock`, and with the class blocked, the family reports **every
usable cell in the room** as marooned: 48 in a trade room, 10 in a wreck.
The cure is in the sim (docs/ROOMS.md, "The doorstep law"): a doorstep
reads as the room's ordinary deck, which is the declaration
`Sim::free_berth_in` was already making, said in the one place the paint
reads too.

The law itself and its catch-out live in `sim::room`, where the tile
classes are —
`a_rooms_own_goods_do_not_stand_between_its_door_and_its_deck` walls a
room off course by course and requires the walk to say so. What the
gauntlet owns is the half the cabin owns: that the report reaches every
room kind and every declared door.

Findings are filed under the room **kind**, lower-cased, and not under a
station. A net is folded the same way in every station that has one, so a
defect in how `Trade` lays its deck out is one defect and not twelve —
the same argument `rigs` makes one layer down.

### `fixture-reached` — a room's own worked hardware can be worked

The fifteenth, and `berth-reached`'s missing half. That family asks
whether every **berth** is workable from somewhere a body may stand, and
a berth is a cell of the net cargo may legally take. A handshake is not a
cell of the net — `RoomKind::surface_of` punches a hole where it stands,
so the arbiter never offers it and `berths` never produces it — and a
latch hangs on bare wall beside a jamb. Between them they are the only
two things in a room that answer a **press**, which is the only verb in
this game that is not a carry, and until this family a station could hang
a beacon in front of its own counter with the build green.

The probe is the fixture's own pick face (`room::handshake_face`, and a
latch's `Dress::Grab` quad), so the question is asked of the surface the
runtime actually answers through. A fixture does not occlude itself, so
its own brasswork comes off the list of things that could be in the way.

Its guard is worth reading before writing another reachability rule,
because two obvious catch-outs were **answered by the rule** rather than
catching it:

- A plate the size of the cell, a hand's breadth in front of the brass,
  does not put it out of reach. A body inside two metres of the counter
  can stand well to one side and lean in, and the ray goes round the
  edge.
- A partition across the whole room a hand's breadth out does not either.
  The walk envelope reaches to within five centimetres of a wall, so a
  body simply stands behind it.

Neither is a hole. "Workable from anywhere a body may stand" is what the
family asks, and standing beside a plate is somewhere. What does catch it
is a body bolted **over** the brass — which is the shape the defect
actually takes: the Guild's seizure beacon hung 0.58 m off its aft wall,
and a beacon over a counter is the same fitting one cell along.

### `fixture-seen` — nothing a room hangs or stocks stands across a control

The sixteenth, and `berth-seen` with the nouns swapped. That family asks
whether a station's furniture stands between a wall **berth** and the
room; this asks whether anything stands between the room and a thing a
hand has to find and work.

- **What the room hangs.** A fitting or a piece of doorway hardware
  standing across a control it did not draw. No argument needed: a beacon
  over a counter is a beacon over a counter.
- **What the room stocks.** A berth of `Tile::Stock` — the one class a
  room puts its own goods on — spending its air across a control. The
  doorstep law's sibling one layer out: a room does not lay its goods
  where a body has to work.

**It is asked of every room of the staged ship and not only of the staged
room.** A latch is drawn by the room with the lower id and hangs on that
room's side of the wall, so the only latches in the game hang in a cabin
— and the roster's own cabin is a yard-fresh one with nothing alongside
it, which has no seam to part and therefore no latch at all. Asked of the
staged room alone, this family would have swept fifteen stations and
never once looked at the control the owner reported.

A third clause was written, measured, and taken out again; the map above
carries the numbers and the argument.

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

**The file is empty today**, so the equality it asserts is against a
sweep that finds nothing and the next line to appear is a defect that
arrived after this was written. An empty docket is not a finished
harness — read "What the harness can see, and what it cannot" below,
and go and look at something it cannot.

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
box has six faces, which is what a leaning chevron and a perfume vial
turned corner-on to the room need — the vial is square on the axis it
stands on and on neither of the other two, so two of its box's sides are
faces and four are wrapper.

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

**And there is a second shape, which is what a described layer does not
DECLARE.** Everything above is about a body nothing could enumerate.
Three of the four defects `furniture-seated` was written for were in
layers the harness had enumerated for a long time: `poi::character_of`
has always handed over every fitting a station hangs, and
`room::seam_parts` has handed over every body in a doorway since the
doorway layer was described. What none of them said was **what holds it
up**. A rig had that all along because the sim berths it and the berth
names a chart; a fitting had a shape, a coat and a position, and a
position is not a promise. So the analogue of "what is on screen that
nothing enumerates" is "what does a description leave unsaid that a
player can see is wrong" — and the answer that took four defects to find
was a joint.

| Layer | Was invisible because | Closed by | Found |
| --- | --- | --- | --- |
| Furniture and doorway hardware | enumerated all along, and declaring nothing about what carried it | `poi::Seat` and `room::Seat`, read back by `furniture-seated` | 264 claims over 15 stations and every doorway; 133 fittings and every shut door's rivets moved onto what holds them, 10 docketed |

**And there is a third shape, which is what every rule happens to ASK.**
Cargo was described and cargo declared its chart, so neither of the two
above covers it — and a crate still stood half a cell off its berth
through four passes of somebody looking for it. Eleven families all
asked one kind of question: is the body *inside* its plan, inside its
band, inside the room, touching its chart. Every one of those is
satisfied by a body shoved hard against one edge of the ground it was
given. Nothing asked where inside.

| Layer | Was invisible because | Closed by | Found |
| --- | --- | --- | --- |
| A berthed rig's position within its own cells | described and declared, and asked only ever whether it stayed inside something | `berth-filled`, which measures the box a berth spends against the ground its plan owns | 33: every deck and deckhead berth in the game, 0.2329 m out into the aisle on the axis the rect pays for |

So the third thing to ask of a green sweep, after "what is on screen that
nothing enumerates" and "what does a description leave unsaid", is
**"where a claim fixes two axes, what fixes the third"** — and then
whether any rule in the file actually asks it.

**And there is a fourth shape, which is the one that ends the list.**
Each of the three above is a lesson learned from a defect somebody found,
and each was written down *after* the finding. That is the pattern the
owner called out, and the honest thing to say about the first three is
that they are three points, not a method: knowing that a defect hid in an
undescribed layer, in an undeclared joint, and in an unasked axis does
not tell you where the fourth one is.

What does is the map at the top of this file. The three shapes are all
the same shape read at different depths — **a triple nobody wrote down**
— and a triple has only three parts. So instead of asking "what have we
been surprised by lately", enumerate the bodies, the relations and the
frames, cross them, dismiss what is meaningless, and read off what is
left. It found four, none of them by playing:

| Layer | Was invisible because | Closed by | Found |
| --- | --- | --- | --- |
| A berthed rig's turn | every family measured it as an axis-aligned box, and a box carries no turn | `berth-turned` | every deckhead berth in the game, taking one fixed turn from every cell of every ceiling |
| A room's net, as a place you walk | every rule in the file measured metres; the one frame with no geometry in it had no rule in it | `deck-reached` | `Trade` and `Wreck` standing their own doorways on their own shopfront: 48 and 10 usable cells behind it |
| A room's own worked hardware | it is not a cell of the net, so no rule about berths was ever about it | `fixture-reached`, `fixture-seen` | nothing yet, and the reason is in the map: one clause of it had to come back out |

The method's own cost is worth stating too, because it is not free: the
enumeration is 840 triples, most of them meaningless, and the work is in
dismissing them convincingly rather than in writing the four rules. What
it buys is that the dismissals are **written down**, so the next pass
argues with a list instead of with a memory.

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
- **A doorway's hardware standing in a berth.** `berth-clear` asks about
  a station's own furniture and not about a seam's: a frame dresses the
  aperture and the cells beside an aperture are berths, so a jamb stands
  in one by construction — which is the same exemption `grid-fits`
  spends on hardware, for the same reason. Asked of doorways it reports
  seventy-one lintels and stiles doing their job.
- **Anything about colour, taste, or composition.** It measures shapes.
  A room can pass every rule here and look like nothing.

And three structural blind spots that are worth knowing about because
they are not closed, only bounded:

- **A latch and a berth want the same piece of wall, and always will.**
  This used to read "`berth-seen` only looks one way", and the other way
  is `fixture-seen` now — but only for what a room hangs and what a room
  stocks. Asked of every berth it fires on the cabin's own seam latch:
  three berths cross it, (5, 1) on the aft wall beside the jamb and
  (5, 3) and (5, 4) on the deck in front of it, the worst standing across
  **100%** of the amber. That is not a defect and it cannot be cured.
  Every wall cell beside an aperture is a berth and every deck cell in
  front of one is a berth, so a control bolted beside a doorway shares
  air with a berth by construction, and the frame's own cells — the only
  cells that are not berths — are the doorway you walk through. **What is
  left of the class is a player standing their own crate in front of
  their own latch**, which is a crate they can pick up again; the
  runtime's pointer takes the nearest mapped surface, so while it stands
  there the latch is not pickable. Nothing in the harness will tell you,
  by design.
- **A pose is one pose.** The roster mates every station at the first
  door the spawn walk takes, deterministically, because a sweep that took
  a minute would not be run. The rules are all measured in a room's own
  frame, so the pose is not what any of them is about — but a defect that
  needs a *particular* attachment order to appear would not show up.
- **A stance may stand five centimetres from a wall.** `stances` samples
  the walk envelope on an 8 × 8 grid per box, and the envelope reaches to
  within about 0.05 m of a chart. So the two reachability families are
  lenient by construction: anything on a wall is workable from a body
  pressed against that wall, and only something that fills the air in
  front of it can fail. That is measured rather than assumed — it is what
  answered the second of `fixture-reached`'s two rejected catch-outs.
- **Every ship the sweep stages is one storey.** The roster mates a
  caller at the first door the spawn walk takes, so no ladder and no
  hatch is ever mated in a swept scene: a hatch's coaming, hinge and pull
  are only ever measured in the shut state, and a two-cabin amalgamation
  — the shape the port law exists for — is never swept at all. This is
  inherited rather than chosen. `room::walk_boxes` pins every envelope
  box to `EYE_HEIGHT` in WORLD y and gives a non-door seam no connector,
  so the body itself is single-storey today; the two reachability
  families sample that envelope and can only stand on storey 0. Measured:
  sweeping a cabin mated above another through the ladder reports six
  findings, all of them the upper room being unreachable and its walk
  standing outside the envelope, and none of them about its geometry.
  When the vertical seam becomes something a player can walk, the
  envelope is what has to learn about storeys, and this comes right with
  it.
- **A covering's turn is asked on the cabin's charts only.**
  `berth-turned` skips coverings, because `pieces::laid_on` lays one flat
  on its chart and its turn is a derivation rather than a composition;
  what asks it is `pieces::tests::every_kind_hangs_true_on_every_legal_berth`,
  which sweeps the cabin. A calling room whose charts lay differently
  would not be asked.
- **The pixel half is opt-in, and CI has no window at all.**
  `.github/workflows/ci-cd.yml` installs no `xvfb` and no Vulkan ICD, so
  the flicker and light-pop detectors run only when a human or an agent
  runs them.

**And there is one described layer no family reads, on purpose.** The
tells — the aim's rim line, the mark's dashes, the offer's standing band
— are not part of the world at all now, and cannot be. They are not
geometry: a piece is drawn a second time into a mask and a full-screen
pass paints the outline off that mask's edge (`crate::outline`), so
there is no body in the room for a family measured in metres to measure.
Every family here reads a description of something DRAWN AT a place, and
a screen-space line is drawn at a place on the screen.

That is a ruling rather than an omission, and it was one before the
outline landed too: a tell is drawn at a stand-off from a body on
purpose, and it is hidden except in the moment it is saying something. A
sweep that read it would report the reading as the defect, for every
claimed crate in the game.

What holds the layer instead is three rules of its own, asked in the
layer's own frame — the bands the forms are cut from are declared in
Rust and written into the shader, precisely so that something can be
said about them. `outline::tests::only_the_aim_is_drawn_on_the_body` is
the containment question and it carries the one exemption the
vocabulary allows; `outline::tests::no_two_readings_draw_one_line_over_another`
is the coplanar question asked between the three readings that may be
worn at once; and `outline::tests::an_outline_has_no_holes_in_it` is a
question the bars never had to answer, because a screen-space distance
is quantised and a band drawn between two of the values it can take
comes out as a dotted arc.

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
