# The walkable bay: the hold unfolds into the room

The cargo experiment's second slice. The hold leaves the desk and
becomes the aft half of the cabin: cargo at furniture scale, placed by
walking up to it, with the first piece of storage furniture — the
cabinet — stretching the berth architecture. This document also records
the project's largest scope decision so far: the 2D console retires.

> Superseded in part by *Cargo stops colliding*: the cabinet is
> furniture that stores nothing, and cargo no longer collides with
> cargo. And by *Lift, and the keys*: a vase goes on the cabinet's top
> by standing a lift off the deck, and the carry's keys are a table the
> `Esc` menu rebinds.

## The grid comes out

The owner's decree (2026-10-03): "get rid of the grid-based system and
transition to free-form placement & movement of cargo for now". Details
are to follow from the owner, so this pass makes conservative defaults,
writes each one down here with its reason, and keeps every law that is
not *about* cell-snapping. Passages below that the decree overrules are
marked "Superseded by *The grid comes out*" rather than deleted.

**What moved: where a piece may stand.** A berth is no longer a cell. It
is any position on a chart, quantised by a unit fine enough that a
player never sees it. `Loc::Hold` and `Loc::Laid` carry the footprint's
top-left corner in sixteenths of a cell (`cargo::FINE = 16`, about
34 mm, the frontend's own finest cut of the world). Positions are
integers, so no floating point reaches the arbiter and a crew in
lockstep agrees to the unit. One helper, `cargo::Foot`, holds the rect
arithmetic — the cells a footprint touches, the cell under its centre,
intersection, and the gap between two footprints — so no caller
re-derives it.

> Superseded by *Cargo turns*: a fine unit is a 256th of a cell now,
> and a berth names its footprint's centre and turn rather than its
> top-left corner.

**What did not move.** The room lattice, ports, the pad, room nets,
charts and tile classes and their paint: rooms are architecture and stay
on the lattice, and a tile class is now a region a piece stands in.
Cubbies stay discrete slots inside a piece (superseded by *Cargo stops
colliding*: there are none). Footprints stay whole cells
in size (`Kind::extent`, `Kind::face_on`); only their position is free.
There is no rotation in this pass — a body's yaw still follows its chart
— because free yaw is a separate decision. There is no physics, and the
sim still learns no player position, because determinism, lockstep and
the input-frame spine are load-bearing.

> Superseded in part by *Cargo turns*: cargo turns now, at any angle,
> on every chart, and the turn is the sim's. Physics stays out.

**The arbiter, restated over fine rects.** Same ladder, same order, same
`Violation` names:

- **Bounds**: every cell the footprint touches exists and lies in one
  chart — the chart of the cell holding its top-left corner — because a
  piece bent over a fold is still nowhere.
- **Threshold, Fixture**: refused if the footprint touches any cell of
  that class, because half a crate in a doorway is a crate in a doorway.
- **Affix**: unchanged; the chart class still has to suit the mount.
- **Cryo**: an edge of the footprint lies ON a floor-chart edge (gap
  zero), because the cold is in the plating and a sixteenth of air is
  insulation. The drop's edge snap makes flush easy to hit.
- **Overlap**: half-open rect intersection with another standing piece
  in the room. Touching is legal, so pieces may stand flush.
- **Shadow**: a tall floor piece shadows the wall behind any floor edge
  it stands *less than one cell* from, across its own extent along the
  seam and up its stature (three courses at most), because a wardrobe
  half a cell off the wall still hides what hangs behind it.
- **Volatile**: two volatile pieces need half a cell of clear air,
  corners included (Chebyshev gap `< FINE / 2` refuses), because the old
  "no shared edge" would be defeated by a gap of one sixteenth.
- **Suspicious**: unchanged.
- **Dressings**: one dressing per point of the room, and the pinned
  rule as rect overlap with standing pieces, both directions.

> Superseded by *Cargo turns*: the ladder is restated over oriented
> footprints — bounds by corners, separating axes for overlap and
> dressings, cryo within a sixteenth of the hull rather than on it, the
> shadow off the box round a turned body, and Euclidean distance for
> the volatile rule.

> Superseded in part by *Cargo stops colliding*: Overlap, Shadow and
> both Dressings rules are gone from the ladder. The overlap and the
> shadow survive only as the game's own tidiness (`cargo::clear`).

**Which tile a piece stands on.** The classes that refuse cargo refuse
a footprint that touches any of their cells (above, and the drop's own
gate). Everything else that asks what a piece stands on — ownership,
`staying`, the offer area and the fire, the drop's exits, the launch
gate, the burner's beat — reads **the tile under the footprint's
centre** (`cargo::berth_tile`), because a piece half on the chalk is on
whichever side its middle is. A centre lying exactly on a seam reads
the cell above and to the left, which is chosen for what it keeps: at
any whole-cell anchor every kind the game has reads its anchor cell,
which is what the grid's rules always read.

**Light reaches a cell.** A cell or a footprint is lit when a lit
lamp's footprint (or a laid luminous coat's) in the same room is less
than one cell away by Chebyshev gap and does not wholly contain it.
**Corners count now**, because a lamp a sixteenth past a crate's corner
is not darker than one a sixteenth past its edge. The rat still asks of
one cell (`lit_adjacent`); everything else asks of a footprint
(`lit_within_reach`).

> Superseded in part by *Cargo turns*: "less than one cell away" is
> Euclidean now.

**The rat stays on its cells.** It is not cargo: it walks its lattice
as before, perches on a cell any footprint touches, nibbles the piece
whose footprint is nearest its cell (zero when the footprint touches
it), and the crowding gate counts floor area in cells.

**Placing a piece for the player.** Shift-press, the comet harvest, the
??? exchange, the hopper's banking and the save reader's rehoming all go
through `first_fit` / `dress_fit`. Their candidates are every whole-cell
anchor plus every anchor flush against the right or bottom edge of a
piece already in the room, sorted (room order, row, column), first legal
wins — which finds a snug gap between pieces that do not sit on the grid
without scanning 256 positions per cell. A chart's own far edges need no
anchors of their own, because a chart and a footprint are both whole
cells. The room-local scans — a shelf setting out its goods, a deal's
answer, the offer area a migration walks a proposal onto, a fluff
budding a whole cell over — keep whole-cell anchors, because they set
things out a tile at a time.

> Superseded in part by DESIGN_REVIEW.md, *An old save or tape starts a
> new run*: no migration walks anything now.

> Superseded in part by *Cargo turns*: a candidate corner is a
> footprint's top-left set down at the turn the game gives a body
> there, then — for a footprint that is not square — a quarter turn on;
> the berth it stores is the centre.

> Superseded in part by *Cargo stops colliding*: "first legal wins" is
> now the fallback. The first candidate standing clear of everything
> else in its room wins, and only failing one the first legal
> (`cargo::tidy`), the shelves, the deals' restock and a fluff's bud
> included.

**The drop.** The pointer was always continuous; now the berth is too.
A release resolves a position before it asks any rule about it, in the
sim and deterministically (`Sim::settle`):

1. The cell under the pointer names the room and the chart; off the net
   is a soft miss, as it always was.
2. The held kind is planned on that chart and **centred** on the
   pointer. The pointer becomes fine units once, rounded to the nearest
   sixteenth with halves away from zero (`layout::fine_at`), and every
   step after that is integer arithmetic.
3. The rect is **clamped** into the chart's bounding rect
   (`RoomKind::chart_rect`), so aiming at the edge of a wall slides the
   piece flush instead of refusing it for bounds.
4. Each axis **snaps** on its own: an edge within a quarter cell
   (`FINE / 4`, inclusive) of a chart edge, or of a same-chart
   neighbour's edge, moves flush onto it. Occupancy snaps to occupancy
   and dressings to dressings. Chart edges win outright, then the
   nearest edge, then the lower coordinate, so the answer never depends
   on the order the board was read in.
5. Then the tile-class gate, the cubby, dressing against occupancy, and
   the arbiter, exactly as before, of the resolved footprint.

`Sim::drop_preview(player, p)` is that resolution asked early, so a carry
preview can draw the berth and the verdict the release will get. The
release runs the same function on the same board, and the same-tick
race reading (`contested_only`) runs it too, against the board as it
stood before the winner landed; the ghost and the drop cannot disagree.
One consequence is worth knowing: a press and a release at the same
point re-centre a piece on that point, so a two-cell piece lifted by its
end and put straight back moves half a cell.

> Superseded in part by *Cargo turns*: the footprint is centred at the
> carry's turn, and the snap reaches an eighth of a cell, onto a
> chart's edge only — the neighbour snap is gone.
> `Sim::drop_preview` takes the turn as well.

> Superseded in part by *Cargo stops colliding*: step 5 has no cubby
> and no dressing-against-occupancy, and there is no same-tick race
> reading: a drop cannot take a spot from the next one.

**Saves and tapes.** Saves are `STV20`: a `hold` or `laid` line carries
fine coordinates, and every older document's cells load as `cell *
FINE`, the same ground exactly, before the rest of the migration chain
runs. Tapes are `RPL4` with an unchanged grammar: the same frames now
settle cargo somewhere else, so an `RPL3` tape fails safe as unsupported
instead of replaying into a different game.

> Superseded in part by DESIGN_REVIEW.md, *An old save or tape starts a
> new run*: an older document is not read at all, in cells or
> otherwise, and the game starts a new run. Tapes were already there.

> Superseded by *Cargo turns*: saves are `STV21` and tapes `RPL5`.

**Sweeps.** A test that means "every berth" sweeps every whole-cell
anchor plus a fixed sample of sixteenths on each axis
(`cargo::FRACTIONS`, `{0, 1, 7, 8, 15}`), which keeps the runtime sane
and names the same berth on every failure.

> Superseded in part by *Cargo turns*: the sample is `{0, 1, 127, 128,
> 255}` in 256ths, crossed with a sample of turns.

**The frontend.** `layout::piece_rect` is the berth's true rect,
fractions and all, so every body, pick face and halo placed from it
follows the piece off the grid. Everything a carry previews is drawn
from `Sim::drop_preview` and nothing from the aimed cell, so what the
cabin shows and where the release lands are one answer:

- **The ghost stands where the piece will land.** While roaming and
  aimed at the room, the carried rig takes the pose of the berth the
  preview names — the same `site_on` pose a settled piece takes, at
  that berth's rect — a tenth large about the middle of its plan and
  lifted the carry's 5 cm off its chart. It follows the aim a
  sixteenth at a time because the berth does, and the drop's glide
  has nothing left to travel but the lift. Grown about the plan's
  middle rather than the rig's origin, so a body standing on a deck
  still stands on it while it is a tenth too big. A cubby drop
  (`Loc::Stow`) hovers at the hit as it always did (superseded by
  *Cargo stops colliding*: there is no cubby drop), and aimed at
  nothing the carry stays hitched on the arm.

  > Superseded by *Lift, and the keys*: the ghost is the berth's pose
  > exactly — no tenth large, no 5 cm — at the berth's own lift, and it
  > reads as a ghost by its outline and its slash.
- **One footprint patch replaces the per-cell hint plates.** A single
  plate for the whole ship, moved and resized every frame onto the
  previewed berth's rect: `LAMP_OK` when the drop would land, `LAMP_NO`
  with a slash corner to corner when it would be refused (illegality
  never rides hue alone), dark on a soft miss — a room's own goods,
  which simply go home. The plates lit the cells a footprint anchored
  on the aimed cell would cover, which is ground a centred, clamped,
  snapped drop never took.
- **The berth wells are retired.** There are no berths to show: a berth
  is a position, not a cell, and a lattice of sockets would be a grid
  of invitations to places the drop does not take. Plain and staging
  deck paint nothing (the deck's own art or whitebox already reads as
  a floor). The painted regions that mean something — offer chalk,
  station stock, the burner's hazard field — stay exactly as they
  were: they are regions now, and a piece reads the one under its
  centre.
- **The refusal flash** burns over the berth the carry previewed on its
  last frame — where the ghost stood and the patch burned red — since
  the release has let go before the cue arrives. A refusal with nothing
  previewed (a bare grab of a full cabinet, a cubby that will not take
  the piece) flashes the one cell under the hand.

  > Superseded in part by *Cargo stops colliding*: neither of those
  > refusals exists any more.
- **The backing rule's seam is a rule now, not a rounding allowance.**
  A deck or deckhead body turns its back on a seam it stands half a
  cell from or less, so a body that faces a seam always keeps more
  than half a cell of deck in front of it. While every berth was a
  whole cell the threshold only ever met gaps of zero or a cell; a
  berth exactly half a cell from the front wall used to face it.

  > Superseded by *Cargo turns*: the backing rule is no longer the
  > cabin's, and no law of placement. It is the turn the sim gives a
  > body the game places itself (`cargo::default_turn`); a player's
  > drop keeps the turn it was carried at.

The cabin's berth sweep (`every_kind_hangs_true_on_every_legal_berth`)
and the gauntlet's berths take whole-cell anchors and the
`cargo::FRACTIONS` sample on each axis. The sweep holds the ghost to
the preview: aimed at the middle of a berth, the drop has to name that
berth (or, within a quarter cell of a chart edge, the flush one the
snap moves it to) with the verdict the arbiter gives, and the ghost has
to be that berth's pose plus the lift. The gauntlet re-keyed its air
from cells to footprints; docs/GAUNTLET.md says how.

> Superseded in part by *Cargo turns*: the snap reaches an eighth of a
> cell, and the ghost is asked at the berth's own turn. And by *Lift,
> and the keys*: the ghost is that berth's pose and nothing added, and
> the sweep asks it lifted to its cap as well.

## Cargo turns

The owner's decisions (2026-10-04): "Any angle eventually; things will
end up being placed by somebody grabbing / moving in VR, I don't want
overly aggressive snapping. We could do something like, *snap* to
convenient angles with the tool or something, but the core logic needs
to be resilient against arbitrary locations / angles." Then: "By that
logic, all surfaces should be able to turn. A 'snap placement to wall'
for things like paintings is fine." And of the backing rule: "Automatic
facing probably won't survive in its current form but may be handy for
procedural assets (e.g. default object placings and orientations at
POIs) so it is probably fine."

So **the sim is the authority on a piece's position and on its turn,
and both are arbitrary**, quantised only by integer units nobody can
see. Snapping is a convenience layered on input, never something a rule
depends on. The sim and the cabin's drawing of what it says landed
first; the controls that turn a carry, and the cabin's reading of a body
at an odd angle, landed after them (*The controls*, below). Passages the
decisions overrule are marked "Superseded by *Cargo turns*".

**What did not move.** The sim is deterministic and engine-free, and a
crew in lockstep has to agree on every ruling on every platform, so no
float transcendental (`sin`, `cos`, `sqrt`, ...) reaches sim state and
everything the arbiter decides is integer arithmetic. Rooms, nets,
charts, tile classes and ports stay on the lattice; cubbies stay
discrete slots (superseded by *Cargo stops colliding*: there are none);
footprint SIZES stay whole cells (`Kind::extent`,
`Kind::face_on`). There is no physics and no tilt: a body lies on, hangs
on, or hangs from its chart, turned about that chart's normal.

**The units.**

- **Position.** `cargo::FINE` is 256 to the cell, about 2 mm in the
  cabin, because a 34 mm step is a grid a VR hand can see. The widest
  lane, 22 cells, is 5,632 units, well inside a `u16` (24 cells and
  6,144 units since ROOMS.md's *The walls reach the deckhead*, and
  still well inside). `Loc::Hold` and
  `Loc::Laid` carry the footprint's **centre** and its turn,
  `{ room, x, y, turn }`: once a footprint turns, its top-left corner is
  not a stable anchor.
- **Turn.** `cargo::Turn(u16)`, a binary angle: 65,536 to the turn
  (0.0055°), wrapping. Positive is counter-clockwise **as seen by a
  person in the room looking at the surface** — down at the deck, up at
  the deckhead, straight at a wall. `Turn(0)` is the chart's upright
  frame: on a wall up is up, and on the deck and the deckhead a body
  faces the room's front.
- **Trig.** `Turn::cos` and `Turn::sin` are integers in Q30
  (`cargo::TRIG_ONE`, 2³⁰), computed by integer-only code: a Taylor
  series in Horner form over one octant, and symmetry for the rest. The
  quarter turns are exact — `cos(QUARTER)` is zero, not nearly — so an
  axis-aligned turn lays exactly the rectangle the grid always drew. A
  quarter on and a turn back are identities in the bits at all 65,536
  turns, every value is within a hundred-millionth of the true one, and
  a footprint four cells across has its corners within one unit of true.

**One mapping from chart to net** (`cargo::net_angle(surf, turn)`). The
net is the room unfolded and seen from OUTSIDE: a chart's +x can read
mirrored from inside, the front chart unfolds downward, and the flanks'
courses climb the sheet's x. One function turns a chart and a turn into
the angle a body's across axis (its right, as you face it) lies at on
the sheet, measured from +x toward +y, which runs down the sheet. It is
a base per chart plus the turn:

| Chart | Base | An upright body's right lies along |
|---|---|---|
| Front wall, deckhead | 0 | the sheet's +x |
| Port flank | a quarter | +y |
| Aft wall, deck | a half | −x |
| Starboard flank | three quarters | −y |

**The handedness is the same on every chart**, which is why the turn is
simply added: the net is one sheet folded into one box, every chart
shows the sheet the same side out, and so turning from a chart's +x
toward its +y is counter-clockwise from inside the room on all six. The
cabin's charts say the same in 3D: all six normals point out of the
room. Everything that needs a footprint asks this function (`Foot::of`,
and through it the arbiter, the drop, the light and the rat), and the
cabin poses every body with its answer (`pieces::across_on`). A sim test
pins the table against the net's own folds, and a cabin sweep
(`every_chart_draws_the_turn_the_sim_lays_on_its_net`) holds the drawing
to it: on every chart, at the quarters and at 1°, 15°, 45° and a seventh
of a turn, the body a berth draws covers the footprint the sim lays,
each axis with its sign, and nothing more.

**The footprint** (`cargo::Foot`) is an oriented rectangle: a centre,
half-extents from the planned size, and the net angle. Its two half-axis
vectors are rounded to whole units once, and everything after that is
exact integer geometry on its corners: `aabb`; `contains`, closed;
`cells`, every cell the footprint's INTERIOR meets, so a corner grazing
a cell does not take it; `centre_cell`; `overlaps`, by separating axes,
where touching is legal and any positive area is not; and
`clearance_below`, which decides whether a Euclidean distance is under a
radius exactly — squares compared, rationals cross-multiplied, never a
square root.

**The arbiter, restated over oriented footprints.** Same ladder, same
order, same `Violation` names:

- **Bounds**: every corner lies inside one chart's rect, and every cell
  the footprint covers is that chart's — no hole, no fold.
- **Threshold, Fixture**: any covered cell of that class refuses.
- **Affix**: unchanged.
- **Cryo**: the footprint comes within `cargo::HULL_TOUCH` — a sixteenth
  of a cell, inclusive — of the floor chart's edge. A hand is not exact,
  and a body a degree off square meets a wall at one corner a hair
  before the next; the wall snap makes flush easy anyway, so the
  allowance only ever forgives a hand.
- **Overlap**: separating axes against every other occupancy piece in
  the room. Touching is legal, so pieces stand flush at any angle.
- **Shadow**: a tall floor piece whose box comes within one cell of a
  floor edge shadows the wall behind it, across its projection onto the
  seam and up its stature (three courses at most). A footprint is
  nearest a straight edge at a corner and its shadow on a seam is the
  span of its corners, so the box round it says both, exactly, at any
  angle.

  > Superseded in part by ROOMS.md, *The walls reach the deckhead*: a
  > stature is capped at `sim::room::COURSES`, four, which is still all
  > the wall there is. No kind stands taller than two.
- **Volatile**: two volatile pieces need half a cell of clear air,
  **Euclidean**. The grid's Chebyshev gap measured along the room's
  axes, and a rule that changes when the pieces turn is the room's rule,
  not theirs.
- **Suspicious**: unchanged.
- **Dressings**: one dressing per point (separating axes among laid
  pieces), and the pinned rule is separating axes against standing
  pieces, both ways.

  > Superseded by *Cargo stops colliding*: Overlap, Shadow and both
  > Dressings rules left the arbiter. The separating axes and the shadow
  > are the game's tidiness now (`cargo::clear`), and a rule of nothing.
- **Light**: a lit source lights a target less than one cell away,
  Euclidean, that it does not wholly contain. Corners count, as before.
- **Which tile** a piece stands on is the tile under its centre, and a
  refusing class refuses any covered cell, as before. The rat keeps to
  its cells: it perches on a cell any footprint covers, and nibbles the
  piece whose covered cells are nearest its own.

**Hit-testing happens in the piece's own frame.** `layout::piece_at`
asks the oriented footprint, so the air beside a turned couch grabs
nothing. A sub-rect declared in a piece's own units — a cabinet's
cubbies (superseded by *Cargo stops colliding*: there are none), an
instrument's amber handle — is asked by carrying the pointer
INTO the piece's frame (`layout::piece_contains`, `layout::piece_frame`),
never by turning the sub-rect out into a box. The pointer is read once,
by basic IEEE arithmetic, into whole sub-units of the net; every step
after that is an integer.

**Default facing is procedural only.** The backing rule left the cabin
for the sim as `cargo::default_turn(host, kind, chart, centre)`, an
integer port of what the cabin drew: a deck body within half a cell of a
seam turns its back to that wall — the aft seam first, then the front,
then a flank for a body one cell across — and anywhere else it faces the
front. The deckhead takes the same rule, as it did in the cabin, and a
wall takes `Turn(0)`. It is used by **everything the game places
itself** — the starting board, shelves, furniture, salvage, harvest, the
exchange, banking the hopper, fluff budding and fixture boards — and
never by a player's drop: a carried piece keeps the turn it was carried
at. `first_fit` and `dress_fit` offer every candidate corner aboard at
the default turn first, then, for a footprint that is not square, a
quarter turn on from it, so a body is turned onto its side only when
nothing aboard takes it upright. Half a turn lays the very same ground
and is never asked.

**The drop** (`Sim::settle`; `Sim::drop_preview(player, p, turn)` is the
same resolution asked early). The chart under the pointer names the
room and the chart; the footprint is centred on the pointer at the
carry's turn; the box round it is clamped inside the chart's rect; then
the **wall snap**: an axis whose box lies within an eighth of a cell of
a chart edge, inclusive, slides flush onto it, the nearer edge winning
and a tie going to the lower coordinate. That is the only position snap.
**The neighbour snap is gone** — a hand that set a crate a hair off its
neighbour meant the hair — and the sim snaps no angle at all. The input
frame carries the carry's facing as an absolute `Turn`, because a VR
hand reports a pose and not a key press, and only the release and the
preview read it, so a sparse recording stays exact.

**Saves, tapes and the wire.** Saves are `STV21` (`hold {room} {x} {y}
{turn}`: centre and turn), tapes `RPL5`, the wire `SNP4` with the
facing after the occupied room. A document from any other version
starts a new run (DESIGN_REVIEW.md, *An old save or tape starts a new
run*).

> Superseded by *Cargo stops colliding*: saves are `STV22`, tapes
> `RPL6`, and the wire `SNP5` with the aim after the facing.

**Sweeps.** `cargo::FRACTIONS` is `{0, 1, 127, 128, 255}` in the new
unit — on the line, a unit past it, either side of the middle, a unit
short of the next — and the sim's every-berth sweep crosses it with
upright, a quarter turn, an eighth and a seventh. The cabin's sweeps
share one turn sample (`pieces::TURNS`): the four quarters, 1°, 15°,
45° and a seventh of a turn, each the `Turn` nearest. They ask every
placement at the turn the game gives it and every whole-cell placement
at every turn of the sample, on every chart — the product with the
fractions as well would be eight times the sweep for nothing the grid
does not already ask, since what a turn changes is the ground and the
frame, and neither is any different a unit off the grid.

**The cabin draws the sim's turn and nothing else.** Every rig is posed
in its chart's upright frame turned by the piece's `Turn` about the
chart's normal, through the one mapping (`pieces::turned_frame`): the
upright rule's remaining job is the frame, and which way a body faces
in it is the sim's. `floor_facing` is gone. A carry's facing is the
frontend's own (`bridge::Bridge::facing`): it starts at the held
piece's own turn when the piece is lifted, the player turns it, and the
ghost, the footprint patch and the release are all asked at it.

**The controls.** While carrying, and only while the body roams with a
piece in hand:

| Input | Turns the carry |
|---|---|
| Wheel | to the next multiple of 15° that way |
| `Ctrl` + wheel | one degree a notch, from wherever it is |
| `Q` | the wheel's 15° step, counter-clockwise |
| `Shift` + `Q` | the same step back |

- **Which way.** The wheel rolled up, away from you, turns the piece
  counter-clockwise as seen from the room: looking down at the deck, up
  at the deckhead, or straight at the wall it is bound for. `Q` turns
  the way the wheel's up does.
- **The next multiple, strictly.** From 7° one notch up lands on 15°,
  not 22°, and the next on 30°. Those are the convenient angles the
  owner offered "with the tool"; the sim snaps nothing, and `Ctrl`
  leaves every angle a few notches away. `Q` is the plain notch for a
  hand with no wheel, and `Shift+Q` is its reverse because nothing
  reads `Shift` while a piece is in hand — quick-move is read on a
  press, and the click that ends a carry is a release. `R` stays cut,
  and `E` is focus.
- **Held finer than a turn.** The bridge holds the facing in
  forty-fifths of a `Turn` unit, the coarsest unit in which a degree and
  a `Turn` unit are both whole, and sends the sim the nearest `Turn`; so
  a degree a notch is a degree exactly however many notches are turned,
  and a notch that would send the sim the turn it already has goes on to
  the next stop rather than doing nothing visible.
- **Only a carry, only in the room.** With the `Esc` menu up or a
  station focused the wheel and `Q` do what they always did, which is
  nothing; with an empty hand they turn nothing, and nothing is saved up
  for the next carry. A wheel that reports pixels — a touchpad, a smooth
  wheel — is read at a hundred to the notch, and part-notches add up.
- **The release keeps the facing**, the preview asks at it, and a piece
  set down turned and lifted again carries on from the turn it stands at.

> Superseded in part by *Lift, and the keys*: `Q` and `E` are the two
> turn keys, read off a table the `Esc` menu rebinds, and `Shift+Q` is
> gone. `Shift` with the wheel lifts now. `E` turns with a piece in hand
> and is focus with an empty one.

**What the cabin does at any angle.**

- **The ghost** stands at the berth the sim previews at the carry's
  facing, turned with it, so a notch of the wheel turns the ghost and
  re-rules it the same frame.
- **The footprint patch** is the footprint's own quad, laid along the
  half-axes the sim rounded it to (`pieces::Ground`), not the box round
  it, with its slash from the footprint's own top left to its own bottom
  right. The **refusal flash** burns a bar along each of the refused
  footprint's four edges; its glyph stays upright on its chart, up as
  up, over the footprint's middle.
- **Standing faces read exactly.** A standing body's pick face lays its
  reading along the same half-axes (`SimSurface::axes`), so the point
  the aim lands on is the point the sim carries back into the piece's
  frame, at any angle: a cabinet a seventh of a turn round answers each
  cubby a hair either side of its rack's middle lines (each quarter of
  its face, since *Cargo stops colliding*), and a tank hung
  that crooked routes carry and focus a hair either side of its amber
  band's edges. A face used to read along the sheet's own two axes,
  which can say a quarter turn and nothing else.
- **Windows and instruments needed nothing new**, and are pinned by
  tests at odd angles: a pane hung a seventh of a turn round, sharing a
  wall with a square one, reads its own crooked piece of the wall's one
  sky (`viewport::sub_uv` is affine and carries the turn), and a chart
  tank hung 15° off true is focused square on to its own glass, rolled
  with it (`rig::focus_pose` reads the station the turned berth hangs).
- **The sweeps ask the turns.** `pieces`' every-berth sweep holds every
  body at every turn of the sample to its ground, its frame, its face,
  its handle and its ghost. The gauntlet asks its berth families the
  sample too, and cuts each cell's share of a berth's air from the
  footprint itself — the box round the part of the footprint over that
  cell — rather than from the box round the footprint. `berth-filled`
  reads a turned ground to the two fine units its rounding leaves
  (`TURNED_SLACK`), and `berth-turned` asks only the turns the game
  gives a body itself: a window a player hangs upside down is turned the
  way they turned it.

## Cargo stops colliding

The owner's decisions (2026-10-04, after a playtest): "While the
placement rules can stay with respect to special item conditions (e.g.
certain things needing to be placed up against the wall), we shouldn't
be enforcing that player-placed cargo clip into (or does not clip into)
each-other anymore. Recall, we are moving broadly from a
inventory-tetris-y puzzle to more of a decorative/taste casual one (with
some sample props to test & make sure the mechanics work, not tastefully
designed yet)". And: "On that note, the entire system related to
shelving units with special extra slots can be axed now- it shouldn't
matter anymore, physical placement is more or less an entirely to-taste
thing."

So **where a piece may stand is the room's question and the special
item's, never another piece's.** A crate may stand in a wardrobe, a
painting may hang behind one, two rugs may lie one over the other, and
nothing the arbiter says depends on what else is already there unless a
kind's own condition names it. Passages the decisions overrule are
marked "Superseded by *Cargo stops colliding*".

**What went: every rule whose subject is one body against another.**

- **Overlap** (`Violation::Overlap`): two standing footprints sharing
  ground.
- **The standing shadow**: no painting behind the wardrobe. A wardrobe
  half a cell off a wall still hides what hangs there, and a player who
  hangs it there anyway has decided to.
- **One dressing per point**, which was `Overlap` again in the dressing
  layer.
- **The pinned rule**, both ways (`Violation::Occupied`): laying a rug
  under standing cargo, and lifting one out from under it.
- **Same-tick drop contention** (`contested_only`, and the soft
  snap-back it bought the player who lost): a race only existed because
  one drop could take a spot from the next. Two crew dropping on one
  spot in one tick now both land there.

`Violation::Overlap` and `Violation::Occupied` left with their last
users. `dressing_check` takes no board at all now, because nothing in a
dressing's ladder asks about one.

**What stayed, and why each is not about another body:**

- **Bounds and the chart**: a piece bent over a fold or across a hole
  is nowhere. The room's.
- **Threshold and Fixture**: a doorway belongs to two rooms, and the
  counter's deck and the pendant's ceiling are the room's own hardware.
  The room's, not another piece's.
- **Mounts** (`Affix`): a painting on a wall, a pendant from the
  deckhead. The owner's own example of a special item condition.
- **Cryo against the hull**: the cold is in the plating. A special item's.
- **Volatile spacing**: two canisters keep half a cell of clear air. It
  is the ONE rule left that sets a piece against another piece, and it
  stays as a special item's own condition — a hazard, not a clip — but
  it was kept without asking, and DESIGN_REVIEW.md lists it for the
  owner to strike.
- **One suspicious piece aboard**: the Guild's crate is a plot device
  rather than a body; it would object to a second one anywhere.
- **The vital minimum**: guards ability, not space.
- **The tile-class gates**: the room's stock shelf, the offer chalk, the
  furnace's hazard tiles. The room's.

**The cabinet stores nothing.** `Kind::Cabinet` is a wardrobe prop, as
much furniture as the couch and no more: `Loc::Stow`, `CABINET_SLOTS`, `stowable`,
`free_cubby`, `cabinet_occupied`, the drop's cubby branch,
`DropTargets::stow`, the save's `stow` line, the cabin's cubby glows,
cubby sub-rects and the shrunken minis that rode in them, and the
empty-it-first refusal are all gone. So is everything that *emerged*
from a cubby being its own berth class (*The cabinet: furniture that
stores*, below): nothing is rat-proof, no fluff is boxed out of
breeding, no lamp is dark in a drawer, ??? counts every crate, and a
trade sees whatever is set out. A vial set inside the wardrobe stands
there at full size, in reach of the rat like any other.

**The game still places things tidily.** Nothing the arbiter allows is
refused for want of elbow room, but what the GAME sets down itself —
`first_fit` and `dress_fit`, and through them the quick-move, salvage,
the comet's ice, the exchange and banking the hopper; a station's stock
(`Sim::free_berth_in`) and its restock after a deal; a fluff's bud; the
frontend's fixture boards — looks for free space first. One helper
states the preference once (`cargo::tidy`): of the candidates in the
caller's own deterministic order, the first the arbiter allows that
stands clear of every other body in its room (`cargo::clear`), and
failing that, the first the arbiter allows at all. "Clear" counts both
layers, because free space is space nothing is in — a rug the game lays
goes on bare deck, and a crate it sets down goes beside the rug — and
it counts a standing piece's shadow up the wall, because a wardrobe's
bulk stands in front of that wall whether or not a rule says so. The
shadow and the separating-axis overlap survive for exactly this, and
for nothing a rule reads. A full room still takes one more piece, on
top of something: a crowded spot beats none.

**Which piece a press means.** With bodies sharing ground, one point of
a chart can be on several pieces, and the point alone cannot say which
one the player meant. The cabin always could: it casts ONE ray and
resolves the nearest body along it (*The nearest rule*). That answer
now reaches the sim as `InputFrame::aim`, an `Option<u32>` piece id,
resolved by the frontend and consulted only in the press handler — like
`night`, `occupied` and `facing`, no tick reads it, so a sparse tape
stays exact.

- **The aim only chooses; the sim still hit-tests.** `layout::pick`
  honours the aim when it names a piece the pointer is on, and
  otherwise falls back to the point-pick (`layout::piece_at`). An aim at
  a piece the pointer is not on — stale, mistaken or hostile — reaches
  nothing a pointer could not reach already.
- **The fallback order**, for tests, monkeys and tapes that send no
  aim: a standing piece before a laid one (the couch takes the press,
  and a bare stretch of rug answers for the rug, as before); then the
  smaller footprint, because a small piece inside a big one's ground
  could otherwise never be reached by a point, while the big one still
  answers everywhere else; then the lower id, so the answer never
  depends on the order the board happens to be listed in.
- **One resolution in the cabin.** The pointer carries the piece whose
  own face the ray struck (`VirtualPointer::piece`, from the face's
  `Riding`), and `VirtualPointer::aimed` hands both to `layout::pick`.
  The hover glint, the outline, the handle-versus-focus routing
  (`rig::handle_route`, which now takes the resolved piece) and the
  press's aim all ask it, of the same pointer and the same board, so
  they cannot disagree. On a chart, which rides nothing, the sim's own
  order decides for all of them alike.
- **A consequence worth knowing.** A pick body is the box the tell
  draws (`pieces::drawn_box`), and the ray meets the nearest box first.
  A vial set wholly inside a wardrobe's box is behind the wardrobe's
  front as far as the ray is concerned, so the crosshair lifts the
  wardrobe, and the vial after it. A body that pokes out of another is
  picked by the part that pokes out.

**Saves, tapes and the wire.** Saves are `STV22` (no `stow` line; a
`laid` line passes the dressing rules on its own, a `hold` line may not
stand on a doorway), tapes `RPL6`, the wire `SNP5` with the aim after
the facing (`-` or the id). Another version starts a new run, as ever
(DESIGN_REVIEW.md, *An old save or tape starts a new run*). The fixture
board was re-saved through the new writer; its four cubby pieces stand
along the aft row of the deck where `cargo::tidy` set them, off the
doorstep.

> Superseded by ROOMS.md, *The walls reach the deckhead*: saves are
> `STV23`, tapes `RPL7`, and the wire `SNP6`, because every room's net
> grew a course on each wall and the same berth or pointer names
> another place. And by *Lift, and the keys*: `STV24`, `RPL8` and
> `SNP7`, for the lift.

**The gauntlet already judged the room.** Every berth family
(`berth-clear`, `berth-seen`, `berth-reached`, and the rest of
docs/GAUNTLET.md's) asks which berths exist of the arbiter on an EMPTY
board, so a station's furniture is judged against what the player may
place there, and cargo sharing air with cargo is no finding, by design.
What changed is the loaded board the findings name their culprits on:
the arbiter alone would now stand a piece on every cell of every room,
so the load sets cargo out the way the game does, clear of what it
already stood (`gauntlet::load`), and it is the board it always was. The
docket stays empty.

## Lift, and the keys

The owner, after a playtest: "Q + Shift Q feel weird, maybe Q + E
instead? Ctrl felt fine. Maybe time to add a keybind menu option to this
one." And, of the cabinet's cubbies: "the entire system related to
shelving units with special extra slots can be axed now ... We probably
need controls to nudge things up and down to re-create the visual of
things in the same way, more or less (and I never really liked the
re-scaling part of it anyway)."

The cubbies went with *Cargo stops colliding*, and with them the only
way anything ever stood above the deck. A vase on a cabinet now is a
vase put where the cabinet's top is, and that takes a third coordinate.
So **a standing berth has a lift**, the ghost is drawn at the size the
piece lands at, and **the carry's keys are a table the player rebinds**
on a page of the `Esc` menu. Passages this overrules are marked
"Superseded by *Lift, and the keys*".

### Lift: a berth's height off its surface

- **The unit and the direction.** `Loc::Hold` carries `lift`, in
  `cargo::FINE` units (256 to the cell, about 2 mm), along the chart's
  normal and away from it into the room: up off the deck, down from the
  deckhead, out from a wall. `Loc::lift` is nought for a laid covering,
  which stays flush: a coat has no height, and a rug carried at a lift
  is laid on the deck.
- **The cap, stated once.** `cargo::lift_cap(host, kind, surf)` is the
  room's section off the chart (`RoomKind::section`: `COURSES` from deck
  to deckhead, the deck's depth off the aft and front walls, its width
  off the flanks) less the body's own reach off it (`Kind::proud`: its
  height on a deck or a deckhead, its depth on a wall), so a lifted body
  stays inside the room's box. A wardrobe two courses tall raised as far
  as it goes has its top at the deckhead; a vial goes three courses up;
  a pendant lowered all the way stands on the deck; a painting carried
  out from the cabin's aft wall stops a cell short of the front one. The
  drop holds a carry to it, a save refuses a berth past it, the frontend
  asks it, and a gauntlet test holds the sim's cells to the cabin's
  metres in every room
  (`gauntlet::tests::every_berth_is_asked_lifted_and_its_cap_stops_inside_the_room`).
- **A taste coordinate.** The sim stores it, saves it and sends it, and
  no rule reads it. The arbiter is asked about the plane (`Loc::spot`
  carries no lift), so light reach, volatile spacing, the rat's walk and
  every tile class read the ground under the body: a lamp lifted onto a
  cabinet lights what is around it in plan, as it did standing there,
  and two canisters keep their half cell of air in plan at whatever
  heights they stand (`sim::tests::no_rule_reads_a_lift`).
- **Carry state, like the facing.** The frontend owns the carry's lift
  (`bridge::Bridge::lift`) and sends it absolute in `InputFrame::lift`,
  which only the release and the preview read, so a sparse tape stays
  exact. A carry starts at the held piece's own lift. **When the carry
  is aimed at a chart of another class than the one it was last on** —
  deck to wall, wall to deckhead (`Mount::of`) — **the lift starts again
  from that surface**, because a height off the deck means nothing on a
  wall; between two walls, or aimed off every net, it is kept. The
  bridge holds it to the cap of the chart it was last aimed at, so a key
  held past the deckhead saves nothing up that a key the other way would
  have to spend.
- **One resolution.** `Sim::drop_preview(player, p, turn, lift)` and the
  release are the same `Sim::settle`: the lift is held to the cap for
  the kind on the chart under the pointer, then the rest of the drop
  runs as it did.
- **The cabin poses every rig with its lift.** `pieces::site_on` carries
  the whole pose along the chart's inward normal by the lift
  (`pieces::lift_off`) and moves nothing else — the frame, the turn, the
  scale and the stand-off onto the plan are the berth's. An instrument's
  glass and a standing body's pick face ride the lifted pose, and a wall
  body lifted off its chart carries a pick face of its own as a turned
  one does, because the chart a lift behind it reads somewhere else at
  every angle but square on. So the crosshair finds a vase on a cabinet
  by the vase.
- **Saves are `STV24`** (`hold {room} {x} {y} {turn} {lift}`), tapes
  `RPL8`, the wire `SNP7` with the lift after the facing. Another version
  starts a new run, as ever (DESIGN_REVIEW.md, *An old save or tape
  starts a new run*). The fixture board gained a nought on every `hold`
  line and nothing else.

### The ghost is true size

The ghost stood a tenth large and lifted 5 cm off its berth
(`HOVER_FIT`, `CARRY_LIFT`), which said "not landed yet" by promising a
pose the piece would not take. It now **stands exactly where and how
the piece will land**: `site_on` at the berth the preview names, at its
turn and its lift, at the berth's own scale, and nothing added. It
reads as a ghost by its outline, which wears the drop's ruling
(`crate::outline`), and by its refusal slash — not by its size. The
two constants survive only as the focus drag's (`GLASS_LIFT`,
`GLASS_FIT`): a piece carried over a station's glass, where nothing
lands, still floats off it a shade large.

- **Aimed at nothing placeable**, which now includes a station's glass,
  the carry is hitched on the arm, compact. That is a carry pose, not a
  placement preview, and it is kept.
- **The ghost cannot move, so the patch does.** The footprint patch,
  its slash and the refusal flash are laid at their rungs of the decal
  ladder off the berth's own plane — the chart's, carried out by the
  berth's lift (`pieces::Ground::off`). So they stand in the same place
  against the ghost's faces at every lift, and the coplanar detector the
  gauntlet judges every room with finds no plane a ghost shares with its
  patch or its slash, in any body any kind draws, on any chart it may
  take — nor with its own surface at any fine unit of lift the room has
  for it (`gauntlet::tests::no_ghost_fights_its_patch_or_its_surface`).
  A vase raised onto a cabinet lights the cabinet's top.
- **The sweeps ask it.** The cabin's every-berth sweep asks the drop at
  a lift past any room's and holds it to the cap, on the same ground,
  with the ghost there the berth's own pose carried that far along the
  normal and nowhere else; an App-driven test holds the lifted ghost,
  its scale and its riding patch to the berth the release lands on. The
  gauntlet asks `berth-reached` of every berth lifted to its kind's cap,
  and `berth-filled` and `berth-turned` of every whole-cell berth lifted
  to its cap, on deck, wall and deckhead; the docket stays empty.
  `berth-clear` and `berth-seen` stay on the ground: what a room hangs in
  the air above its deck is no more a clip with a lifted crate than one
  crate is with another (*Cargo stops colliding*).

### The keys

| Action | Default |
|---|---|
| Turn counter-clockwise, to the next 15° | `Q`, wheel up |
| Turn clockwise, to the next 15° | `E`, wheel down |
| Turn one degree | `Ctrl` + wheel |
| Raise, away from the surface | `X`, `Shift` + wheel up |
| Lower, toward it | `Z`, `Shift` + wheel down |
| Raise or lower one fine unit | `Ctrl` + `Shift` + wheel |

- **The lift step** is a sixteenth of a cell, about 34 mm, a key press or
  a notch (`bridge::LIFT_STEP`) — the grid's old quantum, which a hand
  can see and a hand asks for one press at a time; `Ctrl` + `Shift` +
  wheel is a single fine unit, about 2 mm, for the rest. Two keys of one
  pair pressed together cancel.
- **`E` turns with a piece in hand and focuses with an empty one.** `E`
  was focus, and still is with an empty hand. Roaming with a piece in
  hand, a carry action holds `E` by default, so `E` turns the carry
  clockwise and focuses nothing; a click still focuses with a piece in
  hand, exactly as before (`rig::steer`). At a station or mid-glide
  nothing turns a carry, so there `E` is the way back out as it always
  was. Rebound off `E`, the turn takes its new key and `E` is focus in
  either hand; bind another carry action to `E` and `E` does that with a
  full hand.
- **`Shift+Q` is gone.** `Shift` is read with the wheel, for the lift,
  and on a press, for quick-move; neither is a press that ends a carry.
- **One table.** Every place that reads a carry key reads
  `keys::Bindings` (`main::hands`, `rig::steer`); nothing reads one by
  its `KeyCode`. The wheel's gestures are not in it: they are a wheel
  and two modifiers, and this pass rebinds keys.
- `R` stays nobody's, `F` is warp, `Space` pause, `M` mute, `W` `A` `S`
  `D` walk; the nudge bench's keys are its own (`--nudge`), and its
  clash test reads the table's defaults and asks the page's gate of
  every bench key.

### The keys page

A fourth face on the `Esc` menu's icon row, a stamped keyboard, swaps
the panel's lower half between the reading (the tally and the **New
run** bar) and the keys, and its lamp is lit while the keys show. The
page is a table of actions, so the next thing a key does is a row: each
action drawn as a glyph, stamped the way the menu's other controls are
— a loop with its arrow either way for the turns, an arrow off or onto
a bar for the lift — and beside it **a keycap with its key's name**.
Under the table a keycap reading **Defaults** puts every key back; it
sits sunk to the socket while there is nothing to put back.

- **Click a keycap, press a key, done.** A clicked keycap goes blank and
  amber, waiting; the next key binds. The key that answers a waiting
  cap is the cap's and nothing else's — it is taken off the frame's
  input before the game reads it, so binding `F` warps nothing.
- **Binding a key another action holds swaps the two**, so no action is
  ever unbound and no key does two things (`keys::Bindings::bind`).
- **`Esc` while a cap waits stops the waiting** and leaves the menu
  standing; the next `Esc` closes it as ever. The menu opens on its
  reading next time.
- **Refused keys** leave the cap waiting, framed red with the refusal
  slash over it for a moment — shape, not hue alone (`keys::refusal`
  says why each is kept):
  - the game's own: `Esc`; `W` `A` `S` `D`; `Space`, `F` and `M`, live in
    every mode; `Shift` and `Ctrl`, read with the wheel and the click;
    `Alt` and `Super`, the system's;
  - the nudge bench's: the arrows, the brackets, `T`, `R`, `G`, `Tab`,
    `Enter` and `Backspace`, in every build, because the bindings are
    kept in a file every build reads;
  - and any key the page has no name for — it prints a key by name, from
    a closed list of short plain names its font has.
- **Kept beside the save**: `cabin.keys` in the working directory, next
  to `cabin.data`, in a line format like the save's — a `KEYS1` header,
  then one `action key` line per action (`turn-ccw KeyQ`). A missing or
  unreadable file is the defaults; a malformed line is skipped and its
  action keeps its default, never a failed boot; the lines are read
  through the page's own swap, so a hand-edited file cannot make one
  key do two things. A browser build has no working directory and keeps
  its bindings for the session.
- **The keycaps are the second sanctioned exception to the text-free
  law** (DESIGN.md): a key's name is text by nature, and a keyboard's
  own caps are where a player has always read it. The **Defaults** cap
  wears its word for the reason the **New run** bar does.
- `--menu keys` boots with the page standing, for a screenshot.

## The decision: the 2D console retires

The owner weighed keeping the 2D frontend as a forcing function for
frontend/backend separation against the cost of designing cargo-bay /
interior-decorating mechanics under a "must have a 2D analogue"
constraint, leaned toward dropping 2D, and left the final call here.
The call: **drop it.** Reasons, in order of weight:

1. **The volatile design areas need cheap iteration.** Playtests say
   interactions and the barter economy are the weak spots and will be
   redesigned repeatedly until they click. Implementing every attempt
   twice taxes exactly the work that needs the most attempts.
2. **The separation the console enforced no longer needs it.** The
   valuable constraint was never "two renderers" — it was that the sim
   is a pure, deterministic, engine-free library driven by
   [`InputFrame`]s. That constraint is load-bearing (saves, tapes,
   lockstep, every monkey test) and it is enforced by tests, not by the
   macroquad build: `src/sim` and `src/synth` import no engine crate,
   and the whole game still runs headless in `cargo test`.
3. **Interior design is a 3D problem.** The fixture slice's "the grid
   edges are the room" conceit was already the 2D analogue at full
   stretch; rugs, wallpaper, and walkable placement would snap it.

What replaces the old "every mechanic has a 2D analogue" law:

> **Every mechanic must remain expressible and testable in the sim's
> logical space.** The sim keeps its 800×600 logical world, pointer
> frames, and discrete berths; frontends map presentation onto that
> space, never the reverse. If a proposed mechanic cannot be driven by
> `InputFrame`s against `layout` rects, it is a presentation effect,
> not a mechanic.

The console itself stays in version history as the fun artifact it is —
the commit titled "The 2D console retires" removes it, so its parent is
the last commit where `cargo run` still opened the CRT. Its save file
still walks aboard: the cabin adopts a `local.data` on first boot, and
the save reader keeps accepting the console's `STV4` format.

> Superseded by DESIGN_REVIEW.md, *An old save or tape starts a new
> run*: the cabin no longer reads `local.data` and the reader no longer
> accepts `STV4`, so a console-era run starts over.

## The conceit: the grid unfolds

The fixture slice declared the 6×4 hold grid's edges to be the room's
surfaces. The bay makes that literal by unfolding the grid onto the
cabin's aft half, like opening a cardboard box:

- **Rows 0–2 are the aft wall**: row 0 runs along the ceiling line,
  rows 1–2 are bracket-and-shelf wall berths.
- **Row 3 folds onto the deck**: a strip of six floor plates in front
  of the wall. A couch stands on the deck with its back to the wall; a
  1×2 piece anchored at row 2 (floor lamp, cabinet) stands on its
  floor cell and rises across the fold onto the wall band.
- **Columns 0 and 5 meet the side walls**, exactly where the wall-affix
  rule already points.

The sim is untouched by any of this: cells, footprints, the placement
ladder, the rat's hops, lamp adjacency — all keep their grid meaning.

> Superseded by *The grid comes out* (above): a footprint now stands
> anywhere a sixteenth of a cell can name, and lamplight reaches a cell
> every way, corners included. The rat's hops keep their grid meaning.
The fold is presentation, expressed as two `SimSurface`s (the wall band
and the deck strip) that map into the same `layout` grid rect the desk
rack used to.

## Carry, not drag

In the bay you carry cargo the conventional-FPS way: aim the crosshair
at a piece, click to pick it up, walk, aim at a berth, click to set it
down. Under the hood nothing new reaches the sim:

- The roam crosshair ray projects through the bay surfaces into sim
  pointer coordinates — the same mapping focus-mode cursors use.
- A grab synthesizes the press; while carrying, the gesture layer holds
  `held = true` with the pointer tracking the aim (parked when aiming
  off the bay); the placing click synthesizes the release over the
  target cell. This is the carry design ART_DIRECTION_3D.md promised:
  the sim sees ordinary drag frames, so every rule, cue, refusal flash,
  and conservation test applies verbatim, and `RPL2` tapes stay valid.
- Placement hints are physical: the aimed cell's plate glows the
  legal/illegal answer the sim already computes for the held piece,
  and a refusal flashes the violation glyph on the plate itself.

  > Superseded by *The grid comes out*: a drop no longer anchors on the
  > aimed cell, so per-cell plates read a berth the drop does not take.
  > One footprint patch now lies under the berth `Sim::drop_preview`
  > names, the ghost stands on that berth, and the flash burns there.
- Carrying has reach: the grab/place ray only bites within arm's
  length plus a step, so placement always happens near the body and
  the piece never teleports across the room.

The barter counter is untouched this pass and keeps its desk scale.
The story: **the counter is the broker's diorama** — deals are struck
over scale models of the goods; the bay is where the real things sit.
A piece glides between scales as it changes berth. The whole barter
surface is due for its own redesign once the economy design settles,
so no further investment lands there now.

> Settled since, by decree: the counter is **removed**, not redesigned,
> and the drag grammar goes with it — carry is the only gesture left.
> Stations become attached rooms you carry cargo into. See
> [ROOMS.md](ROOMS.md). The console face went the same way a slice
> later: its readings had already left as cargo, and what remained —
> pause, warp, mute, the hangar tally — moved to the `Esc` menu, which
> is where controls that are *about* the game rather than in it belong.

## The cabinet: furniture that stores

> Superseded by *Cargo stops colliding*: the cabinet is a wardrobe prop
> that stores nothing. `Loc::Stow`, the cubbies and everything below
> that emerged from them are gone; a piece set inside the wardrobe
> stands there at full size, and the empty-it-first refusal went with
> `Violation::Occupied`. Kept for the history.

`Kind::Cabinet`, the architecture stretch: a piece that *provides
berths*. A slim two-cell wardrobe, floor-affixed like the floor lamp,
with four stow cubbies behind its doors.

| Property | Value |
| --- | --- |
| Footprint | 1×2, floor-affixed (anchor row 2) |
| Cubbies | 4 (`Loc::Stow { cabinet, slot }`, slots 0..=3) |
| Stows | 1×1 kinds only |
| Refuses | cryo (needs the hull), anything suspicious (future-proofing; nothing suspicious is 1×1 today) |
| While occupied | the cabinet itself cannot be lifted or quick-moved — empty it first (`Violation::Occupied`) |

Stowing is a drop: release a 1×1 piece over a hold cabinet's footprint
and it takes the first free cubby. Lifting a cubby piece works like any
grab — cubby sub-rects hit-test before the cabinet's own body, so the
pointer never has to fight the furniture. Shift quick-move pops a
stowed piece back to the first legal hold cell. The seam this used to
have — a standing cabinet's cubbies aimed on the flat chart behind it,
so the aim arrived skewed and mirrored — is closed by the standing rule
below; the cubby you look at is the cubby you get.

Everything below **emerges** from `Loc::Stow` being its own berth class
rather than a hold cell — none of it is special-cased:

- **Rat-proof**: the rat schedules against hold cells, so cubby cargo
  can never be nibbled. Storage furniture is vermin insurance.
- **Fluff containment**: only hold fluffs breed. Boxed fluff is
  inventory, not a population.
- **Lamps go dark inside** (`lamp_lit` requires the hold), and a
  stowed painting shows nobody anything.
- **??? does not open your furniture**: the exchange counts mysterious
  crates in the hold and on the rail. A crate in a cabinet sits out
  the ceremony.
- **Trades never see cubbies**: valuation happens on the pads, and an
  occupied cabinet cannot reach a pad. Selling the cabinet means
  emptying it, deliberately, piece by piece.

Value column (base, 0..=6): Saturn 5 (working fixtures), Earth 4 and
the Hermitage 4 (practical people), Venus 3, Mars 3, the Umbra Market 3
(a box that keeps light in its place), everyone else 1–2. The wants
row can ask for one anywhere the jitter reaches.

## Save and replay

`STV5` adds one location form — `stow <cabinet-id> <slot>` — and
nothing else. The reader accepts `STV4` (a save with no stow lines) so
console-era and fixture-era runs load unchanged; the writer emits
`STV5`. Stow lines are validated on load: the referenced cabinet must
exist in the hold, the slot must be in range and unshared, and the
piece must be stowable — a save that lies fails safe into a fresh run,
as ever. Replays stay `RPL2`: carry synthesizes ordinary pointer
frames, so the tape format never heard about any of this.

> Superseded in part by DESIGN_REVIEW.md, *An old save or tape starts a
> new run*: the reader accepts only the header the build writes. The
> stow validation stands.

> Superseded by *Cargo stops colliding*: there is no `stow` line
> (`STV22`), and nothing to validate it against.

## Coverings: the dressing layer (second slice)

The owner's cargo direction — away from raw materials, toward rugs,
wallpaper, paints — lands as a **dressing layer parallel to
occupancy**: `Loc::Laid { x, y }`, a berth class where the piece is
applied *into* a surface rather than standing on it. A laid piece
coexists with occupancy on the same cells (a couch stands on a laid
rug), and no two dressings share a cell. Conservation never blinks: a
laid rug is still the same piece, and peeling it up is an ordinary
grab.

> Superseded in part by *Cargo stops colliding*: dressings may share a
> cell, with each other and with what stands on them, and the pinned
> rule is gone both ways — a rug lifts out from under a couch, by an
> aim at the rug. The interior-design game is taste, not the shuffle
> below.

Three kinds carry the slice, appended as indices 22..=24:

| Kind | Cells | Covers | Story |
| --- | --- | --- | --- |
| `Rug` | 2×1 | deck cells only | somebody's heirloom, gnawably soft |
| `PaintTin` | 1×1 | any one cell | ship enamel, color by the tin's roll |
| `LuminousPaint` | 1×1 | any one cell | glows; the Umbra Market sells it snuffed, in blackout tins |

Coverings have **no hold-occupancy form aboard**: dropped on the grid
they lay (rolled/canned forms exist only on shelves, pads, the rail,
and in cubbies — a paint tin rides a cabinet fine). The rules reuse
the existing violation ladder whole: off-surface is `Affix(Floor)`,
dressing-on-dressing is `Overlap`, and the pinned rule is `Occupied`,
symmetric in both directions — you can neither lay a dressing under
standing cargo nor lift one out from under it. Painting behind the
sconce means moving the sconce; that shuffle *is* the interior-design
game.

Two mechanics ride along:

- **Luminous coats join the light economy.** A laid `LuminousPaint`
  footprint lights its orthogonal neighbours through the same
  `lit_adjacent` read lamps use (superseded by *The grid comes out*:
  everything within a cell, corners included, through
  `lit_within_reach`): the rat fears glow-painted corners,
  seedlings bloom beside them, a hold painting catches their
  spotlight. The pad-side well-lit-art price bonus stays lamp-only —
  a coat is ambiance, not gallery lighting — and the omen dims coats
  like everything else. The Umbra Market prices luminous paint at
  zero, files it under local produce, and shelves it cheap: light
  remains a rival product.
- **Rugs are gnawable.** The rat's nibble reaches laid rugs exactly
  as it reaches hold cargo; a gnawed rug keeps its notch and its
  discount forever. Vermin control (lamps, glow paint, a couch to nap
  on) is now genuinely part of decorating.

The house refuses wagers on coverings — the casino badge simply does
not take carpets — because a transmuted chip could not legally stay
laid. Saves bump to `STV6` for the one new `laid x y` line form; the
reader keeps accepting `STV5` and `STV4`.

> Superseded by DESIGN_REVIEW.md, *An old save or tape starts a new
> run*: neither is read now.

## Where placement rules go next

- **Occupancy berths** (hold cells, cubbies, pads) hold exactly one
  piece; the cabinet shows berths can be *provided by pieces*, so
  shelving, crates-with-compartments, and display cases are the same
  shape with different numbers.

  > Superseded by *Cargo stops colliding*: no berth holds exactly one
  > piece, and no piece provides berths. A shelf is a prop things are
  > set on, to taste.
- **Dressings** (`Loc::Laid`) cover cells without occupying them;
  wallpaper is the same shape as paint with a bigger footprint, and a
  future "finish" tier (deck plating, wall panelling) could stack
  beneath dressings if the game ever wants it.
- **Networking is unaffected** by any of it: lockstep ships input
  frames, berth transitions stay discrete, and the conservation
  monkeys keep proving no interleaving loses a piece.

## The burner: jettison learns to push the ship

Cor's merge (relayed by the owner): junk disposal and
"incinerate cargo to go faster" are one mechanic. It solves what
jettison was for, and it retires the engine as the odd module out of
the far-future modules list — there was never a way to make a bare
engine fun without stacking weirdness on it, and the fire is that fun
now. The sim's outboard rail — the same four `Flotsam` slots, same
rects, same tapes — is the **fuel hopper**, and the airlock annex off
the starboard wall is the **burner room**, coal-train flavored: four
hazard-bordered tiles, each bound to one rail slot through its own
`SimSurface`, live exactly when the sim's rail rule holds (no barter
open).

> The annex becomes a room proper in [ROOMS.md](ROOMS.md): hopper
> staging moves into that room's own grid as hazard-class tiles, and
> the `FLOTSAM_SLOTS == SHELF_SLOTS` exclusivity below dies with the
> barter it was sharing rects with. The fire's mechanics are unchanged.

The mechanics, all sim-side and replay-safe:

- **Every kind knows how it burns** (`Kind::flammable`, 0–3):
  upholstery, fur, and fuel roar (couch, rug, fluff, gas canister);
  wood, organics, and finery burn honestly; chits, tins, and lamps
  barely catch; metal, stone, and ice are slag — the stoker still
  shovels them through, disposal is disposal, they just push nothing.
  The suspicious kinds never reach the hopper at all; they refuse the
  rail as ever.
- **The stoker's beat**: underway, with nothing alongside to watch,
  every twelve seconds the lowest-slot hopper piece goes into the fire
  (`Cue::Burn`) — slow enough to snatch a mistake back off a tile.
- **Fire is way**: each flammability point banks 900 boost ticks
  (`stoke`, carried by STV7 saves); while any remain the ship makes
  double progress, cruise and warp alike. A couch is forty-five
  seconds of double time.
- **Nothing is swept underway anymore.** Cast-off keeps the hopper
  loaded — fuel rides. Encounter close leaves salvage on the tiles
  (grabbable, because the stoker pauses while anything is alongside).
  Docking *banks* the hopper: unburned pieces walk back aboard to the
  first legal berth, and only true overflow is tipped over the side
  (`Cue::Jettison`, the one ceremony that still discards).

  > Superseded in part by *Cargo stops colliding*: whatever the game
  > still walks back aboard (`Sim::walk_aboard`, through `Sim::fit`)
  > takes the first berth clear of everything already there, and the
  > first legal one only when none is, so "overflow" would be a ship
  > with no legal berth at all rather than a full one.

Presentation derives, never restates: the outer hatch is a firebox
door whose glass flares on a feeding and breathes ember with the
banked stoke (the same number the sim spends on way); the burn sounds
as a furnace-clunk scaled by the flame it bought; the window's
star-streaks stretch to double while the fire pushes. The beacon
keeps its old jobs — amber breathe while fuel is staged, red strobe
for a dock overflow.

The chamber and its doorway are sized so the largest footprint in the
game fits — and only JUST, both bounds proven by test, because a roomy
burner room is a second cargo bay. The tile grid crowds toward the
door so every tile stays inside the carry's reach; the slack pools at
the firebox, and a big crate on a near tile pokes into the doorway.

Two smaller reads landed with it: **berth tiles are contextual** — the
bay's socket grid fades in only while a carry is live, so an idle bay
reads as a furnished room, not a warehouse diagram (superseded by *The
grid comes out*: the wells are retired, and the footprint patch under
the previewed berth answers "where can this go?") — and the couch was
recomposed with true depth (rigs began as desk-era bas-reliefs where
+Z meant relief height; standing rigs re-purpose +Z as room depth, so
asymmetric furniture must put its back at the wall — the convention is
now documented at the couch rig).

**The other twenty kinds have since had the same done to them**, and it
took a rule rather than a pass of the eye. A glyph's height is a drawing
on a flat console; given depth and a deck it becomes a standing pose,
and for two thirds of the kinds that pose was wrong — so twenty of them
stood between one and fifteen centimetres above their own deck cells
with nothing under them, which nothing could see until the gauntlet grew
a family for it (`rig-seated`, docs/GAUNTLET.md).

The rule is **a flat kind lies on a deck; it does not stand on one.**
Settling all twenty downward cures the arithmetic and makes two of the
pictures worse: a transit chit or a casino chip translated onto the deck
is a playing card balanced on its edge. So the question per kind is what
pose the object takes on a floor. Eleven were posed right and wanted
settling; seven were lying down because a glyph reads a cylinder end-on
as a circle, and stood up; two lay down. Where a sole lands is one
function (`pieces::sole_of`), read off the plane a deck berth stands a
rig on, so a kind is composed against the floor it will stand on rather
than against the middle of a cell.

## The room grid (spec): everything is cargo, everywhere is grid

> Superseded in part by *The grid comes out*: rooms, nets and charts
> stay on the grid, and cargo does not. A berth is a position on a
> chart in sixteenths of a cell, and `Loc::Hold { x, y }` names that
> position rather than a cell.

Owner's requirements, spelled out after the burner round — this
section supersedes the earlier "instruments come off the wall" spec,
which under-read the ask. The instruments-as-cargo idea *fundamentally
begets* two constraints the old spec dodged:

1. The instruments' current locations are **arbitrary** — initial
   berths, not privileged geometry.
2. **Any** cargo may be placed where an instrument sits today, once it
   moves.

Which means the unit of architecture is not "the hold grid plus some
stations" — it is **the entire rectangular room divided into a 3D
grid**, and everything standing in it is cargo under placement rules.

### The net

The room's dimensions are not sacred (owner's words); the room resizes
to fit the grid, not the other way round. Every interior surface snaps
to whole `BAY_CELL` (0.55) multiples — the working plan is a floor of
6×5 cells, four walls 3 cells tall, and a matching 6×5 ceiling, with
the burner annex keeping its carved doorway (its floor joins the
walkable net through it).

> Superseded in part by ROOMS.md, *The walls reach the deckhead*: every
> room's four walls are `sim::room::COURSES` tall, four, and each runs
> from the baseboard to the deckhead. The deckhead is four cells over
> the deck, as it has been since the lattice took the room's section,
> and nothing stands between the cornices and it. The cabin's net is
> 24×15.

The sim stays 2D-logical: the box unfolds into a **net** (a cross of
six charts — floor, four walls, ceiling) laid into the same 800×600
logical space at `CELL` (34) — the whole net is ~408×544, it fits.
`Loc::Hold { x, y }` keeps its shape over one unified grid with a
validity mask (cells outside the cross don't exist); a classifier
(`surface_of(x, y)`) says which plane a cell lies in, and the fold
seams are watertight exactly as the bay's fold is today, proven the
same way. The current 6×4 hold IS this net's embryo — the dressing
rules already classify rows as floor/wall/ceiling by position; the net
makes the classification total.

### Cell vocabulary

Common language, derived from which planes a cell touches — never
stored, always classified:

- **baseboard** — a wall cell in the bottom row (touches the floor)
- **cornice** — a wall cell in the top row (touches the ceiling)

  > True in the room as well as on the sheet only since ROOMS.md's *The
  > walls reach the deckhead*. For as long as the deckhead stood a cell
  > over three courses of wall, a cornice touched a band of fabric no
  > chart reached, and the ceiling chart's seam to it was the one
  > declared trim seam in the net. They meet now, and every fold of the
  > net is watertight.
- **corner** — the seam column where two walls meet; a *floor corner*
  is a floor cell touching two walls
- (the set extends as rules need it; the classifier is one function)

### Placement rules

Per-kind mounts, generalizing today's affix rule — one table, consumed
by `placement_check`, never restated in views:

- Unless otherwise specified, cargo goes on **floor** cells.
- **"tiny" cargo may go on cabinets** — shipped today as `Loc::Stow`
  cubbies; the class generalizes.
- **`supports:top` cargo can carry other cargo on top** — the cabinet
  pattern turned outward: a host kind declares top slots the way the
  cabinet declares cubbies (`Loc::Stow { host, slot }` generalizes;
  crates and the cabinet's flat top are the first hosts).

  > Superseded by *Cargo stops colliding*: neither class exists, and
  > neither will as a slot. Placement is to taste: anything may be set
  > where anything else stands.
- **Paintings and UI instruments must be on the wall.**
- **Lamps may hang from the ceiling** (the ceiling lamp finally means
  it); wall sconces stay wall, floor lamps stay floor.
- Cargo has a 3D extent in cells — across, deep, tall
  (`cargo::Kind::extent`) — and a berth spends whichever two of the
  three its own chart is for. Tall cargo standing against a wall
  **shadows** the wall cells behind it, no painting behind the
  wardrobe; and a deck berth spends the plan rather than the height,
  which is what took the apron off the front of every standing piece
  (see "A footprint is stated in the wall's own frame", below).

  > Superseded in part by *Cargo stops colliding*: the shadow refuses
  > nothing. It is what the GAME keeps clear of when it sets something
  > down itself (`cargo::clear`); a player may hang a painting behind
  > their own wardrobe.
- Coverings (`Loc::Laid`) extend to every plane the mount table allows
  — rugs stay floor; paint always coated any surface.

### The no-soft-lock invariant

> Superseded in part by [ROOMS.md](ROOMS.md): player-character collision
> with cargo is removed, so `Violation::Sealed`, `Violation::Aisle`, and
> the frontend's own-cells refusal all retire — with no collision there
> is no box to be boxed into. The doorway threshold stays clear for a
> reason that survives (an aperture belongs to two rooms), and the
> **vital-minimum rule below is untouched**: it guards ability, not
> mobility.

The player must never be able to construct a state they cannot act out
of. Three guards, all sim-side and monkey-proven:

- **`Violation::Sealed`**: a floor placement that would split the free
  floor into more than one connected region is refused like any other
  violation (no enclosed pockets, no walling off the burner doorway —
  connectivity covers both). Conservative on purpose: it never needs
  to know where the player stands, so it lives in `placement_check`
  with the other rules.
- **The vital-minimum rule** (landed with the instruments): a vital
  kind (`Kind::vital()` — the chart tank and the launch handle)
  refuses every exit ceremony while it is the last of its kind in the
  player's keeping — the outboard rail (jettison and hopper both), the
  give pads, the casino wager — one predicate (`last_vital_aboard`),
  one violation name (`Violation::Vital`). Possession counts only
  berths that are *staying* (hold, cubbies, laid, received shelf): a
  spare already staged on a pad or the rail is on its way out, and
  counting it would let both of a pair be staged and both be lost.
  Spares sell fine; stations occasionally stock used instruments.
- **The frontend refuses a drop into the cells the player occupies**
  (a reach-style gate, like the carry's REACH — the sim has no player
  position and must not grow one).

### Instruments as cargo

- **New wall-mount kinds** (appended as ever): `Window` (2×1),
  `ChartTank` (2×2), `EtaGauge` (1×1), `DestPreview` (1×1),
  `LaunchLever` (1×1). The transit window is cargo after all — whimsy
  dictates it shares placement rules with paintings, and the void
  follows it to any wall — so the front cornice punch-out healed into
  ordinary cells (its old aperture is just the window's traditional
  berth now). The barter counter stays put deliberately: the barter
  interface is slated for removal, not migration — and the removal is
  now specified in [ROOMS.md](ROOMS.md).
- **Yellow handles**: click-functional cargo wears the same AMBER
  handle hardware — the launch handle's glowing pull is the archetype,
  the chart tank's grab rail follows it. Passive glass (window, ETA
  gauge, destination preview) earns no handle; the handle IS the
  affordance.
- **Saves bump to STV9**: a pre-STV9 document carries no instrument
  pieces, so the reader hangs the missing ones at their traditional
  berths on load (first legal cell when the board claims a berth),
  chaining with the pre-STV8 console-grid translation.

  > Superseded by DESIGN_REVIEW.md, *An old save or tape starts a new
  > run*: a pre-STV9 document is not read, so nothing is hung for it.
- **The logical rects stay the law**: `layout::MAP_PANEL` et al. never
  move; the *binding* moves — a mounted instrument carries its
  station's `SimSurface` at its own cells, so rulings and tape format
  never hear about it. The fixed console retired piece by piece, and
  then entirely: **the hull owns no panels.** The face's last tenants
  were the pause/warp/mute buttons and the hangar tally, which were
  never readings at all — they are the `Esc` menu now (an overlay, not
  a station), and their icon rects survive in `layout` only because the
  sim still asks whether a press landed on one. Their hardware — the
  stamped icons, the lamp feel, the tally strip's delivery blink —
  waits compiled and unused in `console.rs`, written against a plain
  `SimSurface`, for whoever makes it a `Kind`.
- **Function follows presence**: charting needs a `ChartTank` aboard,
  launching needs a `LaunchLever` — predicates in the shape of
  `transit_chit_aboard`. The ETA gauge is passive.
- **Focus poses become relative to the instrument's berth** (or retire
  where roam-scale reading suffices); occlusion of a pose is handled
  by rendering, never by placement bans — see below.
- **The readings ride the pieces first, the controls follow.** Landed:
  the chart tank and destination preview wear the CRT's live textures,
  and the ETA gauge's needle reads the leg against a scale — the
  console's arc and preview glass retired with them, and the plate they
  were screwed to followed. The scale is the gauge's second pass and it
  came out of a playtest: a hand sweeping a bare face says a leg is
  running and never says how much of one is left, so the sweep is
  pipped at each quarter and its empty end wears a mark of its own,
  wider and longer than a pip, which burns up as the hand closes on it.
  Nothing to read — a notch, a size, a colour and a motion, and the
  reading survives any three of them. **The window is not one of them.** It used to wear a
  painted sky, which made it a picture of space rather than a way of
  seeing it; its glass is a **hole in the hull** now
  ([ART_DIRECTION_3D.md](ART_DIRECTION_3D.md), "The window is a hole").
  The exterior is real geometry in world space and the pane is the
  aperture an off-axis camera looks out through, so the void is
  genuinely on the other side of it: step across the cabin and the same
  pane frames different space; look at it from one side and you see
  what is out the other. Sell your window and the hull is simply solid
  — no pane, no aperture, no view. **Rehang it and the void follows**,
  literally and by construction: the aperture is derived from wherever
  the piece hangs, so the wall the glass ends up on is the direction it
  looks.
- **The window is a family, and a crew may own several.** The hull
  launches with the one (`Window`, 2×1) and buys the rest: a
  `Porthole` (1×1) — the cheapest hole anybody ever cut, and the only
  size that fits a wall of every room in the game — and Saturn's
  `BayWindow` (2×2), square, the same footprint as the chart tank
  because that is the biggest thing this ship already knows how to hang
  on a wall. Bigger was tried and the *arithmetic* refused it: a calling
  room's shelf is its aft wall with a handshake in the middle and a
  doorway through the corner, and nothing three cells wide and two
  courses tall can stand anywhere on it — a window no station can put
  out is a window nobody can buy, which `barter`'s shelf-fit test now
  fails the build over. Three sizes, one construction (`pieces::window_rig`, three bezels), one
  set of rules: all wall-affixed, none flammable, none vital, and none
  stowable, because a hole in the hull is not a thing you put in a
  drawer. **Nobody launches with more than one**, so the second and
  third are shopping: the bay window has one source (Saturn, whose ring
  is salvage all the way round — somebody else's hull is the only place
  four flawless cells of glass were coming from) and the porthole has an odd one (the
  Umbra Market fences them off ships, exactly as it fences lamps —
  glass that lets starlight in is a rival product there, and prices
  accordingly). Owning several is the *ordinary* case now, so the
  exterior was rebuilt to serve it: panes on one plane share one sky
  ([ART_DIRECTION_3D.md](ART_DIRECTION_3D.md), "One wall, one sky").

  > Superseded in part by ROOMS.md, *The walls reach the deckhead*: the
  > shelf is four courses tall now, and a pane three cells wide and two
  > courses tall would stand over the counter. The window family is the
  > size it was until somebody asks for a bigger one.
- **The handle rule** (the click-vs-carry answer, by decree): a
  click-functional piece wears a physical AMBER carry handle, and the
  handle IS the carry hitbox — hover the handle and a click means
  "move this cargo"; hover anywhere else on the piece and a click is
  the traditional focus interaction (glide in, freed cursor, work the
  instrument, `Esc` out). The handle region is a declared sub-rect of
  the piece's footprint and the rig draws the amber grab exactly
  there, so hitbox and geometry cannot drift apart (the fixture
  sweep's selection-mismatch lesson). Passive cargo (window, gauges,
  crates, furniture) has no function to guard, so it needs no handle:
  its whole body grabs.
- **The nearest rule** (which affordance a press belongs to at all): a
  press reaches the nearest thing the player is actually looking at,
  and nothing standing behind something else answers for it. The
  crosshair casts ONE ray, and every affordance a click could resolve
  to is settled against that one cast and the depth it reached. A
  doorway's amber detach latch used to cast a second ray of its own
  and take the click from in front of it, so a latch behind a crate
  ate every press aimed at the crate — the crate could then be neither
  lifted nor focused, and the first such press sent a room away
  instead. Two rays are two opinions, and the picture only ever
  offered one.

  > Extended by *Cargo stops colliding*: the body that one cast met
  > first now travels to the sim as the press's aim
  > (`InputFrame::aim`), because a point on a chart can be on several
  > pieces once they share ground, and only the ray knows which one is
  > in front.

### The standing rule: cargo that stands maps its own body

Reported from playtest: aiming at the top-right of a standing cabinet
highlighted the top-LEFT cubby. The room net's charts are flat; a rig
that STANDS on one — floor cargo on the deck, a pendant under the
ceiling, a crate on a hopper tile — has its whole body somewhere the
chart is not, so projecting the crosshair onto the chart answers about
a plate the player is not looking at. Screen-vertical came back as
floor-depth, and the backing rule's yaw (the cabinet against the port
seam turns to face starboard) mirrored what was left.

The rule, the same shape as the instruments' stations: **a standing
piece carries its own `SimSurface`, riding the pose the rig actually
took — yaw, roll and all — bound to that piece's own rect.** The ray
meets the piece where the piece is, and whatever the sim hit-tests
inside it — the cabinet's cubby sub-rects today — is read in the frame
the rig drew it in. Consequences, all falling out of the one binding:
selection, the hover glint, the carry's grab and its stow drop cannot
disagree with each other or with the picture.

> Superseded in part by *Cargo stops colliding*: there are no cubby
> sub-rects and no stow drop; the amber handle band is what is read
> inside a body now. And the face the ray struck names its piece, so
> where two bodies share ground the selection, the glint and the grab
> agree on which one (`VirtualPointer::aimed`). Wall cargo the upright
rule leaves level needs no face — it hangs IN its chart's plane, where
the chart already is the piece — but wall cargo the rule ROLLS does,
for the reason below.

**The face is cut from the picture, not from the plan.** It was bound
to the whole footprint at first, which made the region that answered
for a piece its plan rather than its body: the brine pearls are three
spheres in a column filling 62% of their cells across, so a third of a
cell of air on either flank picked them up, and the playtest reported
the hitbox as horizontal while the item was plainly vertical. A rig
describes its own bodies (`pieces::parts`), so the silhouette they add
up to is derivable — `pieces::silhouette` — and the face is that box,
held to the cells so it can never read a neighbour's berth. Binding the
sub-rect the silhouette covers rather than the whole rect keeps the
mapping one-to-one in the rig's own local units, so the handle band and
the cubbies, which are declared in those units, do not move when the
face shrinks. The hover glint is cut from the same box: what lights up
is what answers.

Every one of the thirty-two kinds had a face wider or taller than its
body, from the rug at 98% × 96% of its cells to the bottled midnight at
48% × 48%, and the wall sconce's sat a third of a cell off centre as
well. The gauntlet's `face-fits` family holds the other side of it: a
body that reached OUTSIDE its cells would be paint the aim cannot
follow, since the face has to stop at the cell edge.

The sim never hears about any of it: the pointer it receives is still a
point in its own rect space. Which surface produced that point is the
cabin's business, and the cabin's whole job is that the answer be the
one the player was looking at.

The seam this section used to leave standing is closed, and the shape
of the answer was the one it sketched. The net is the room unfolded as
seen from OUTSIDE, so a chart's +x reads mirrored from inside — which
is why a rig's own frame, not the chart, is the law over the rig's own
body, and that law now reaches the walls as well. It took two turns.

**The upright rule learned to count turns, not corners.** A HALF turn
maps any footprint onto itself, so it is always affordable; only a
QUARTER turn trades width for height and needs a square footprint to
pay for it. That distinction is what the front wall was waiting for:
the front chart unfolds DOWNWARD off the floor's front edge, so its
rows climb the wall and everything on it inherits a half turn — the
window hung its sky upside down there, and a painting its horizon.
Now the whole front cluster stands up. Readable content that occupies
no cells at all takes the roll unconditionally, since it has nothing to
leave: the violation glyphs have a top like any other drawn thing, and
a hazard triangle that points down is not a hazard triangle.

> Superseded in part by *Cargo turns*: the upright rule's remaining
> job is the frame. Which way a body faces in it is the sim's `Turn`,
> measured from upright on every chart, so nothing rolls a body to
> stand it up any more.

**A rolled wall piece carries its own face**, a standoff proud of the
chart so it outranks the coplanar plane behind it (an instrument
answers on its own glass; everything else on a small standoff — a tie
between two quads in the same plane is settled by query order, which is
not an answer). A rolled rig's sub-rects lie the rig's way while the
chart's y still lies the chart's, which is exactly how the tank's amber
handle band came to sit a quarter turn off the bar drawn from those
very numbers, and how the front wall's launch handle came to route its
carry along the band above its own grab.

What keeps it closed is a sweep rather than a case. Every kind, at
every berth the sim's own arbiter allows, must hang its body ON its own
cells, read up-is-up wherever the footprint can afford the turn, hand
the carried ghost exactly the berth's own rotation (preview and berth
share one derivation, and the test pins them so no refactor can split
them again), and — where the kind wears amber — route every texel of
the grab it DRAWS as a carry, with the body around it still answering
for the instrument. Every defect in this class held on the wall it was
written against and nowhere else, so only a table sweep can say "on
every wall" and mean it.

### A footprint is stated in the wall's own frame

**A footprint keeps its shape on every wall it may take**, and the rule
that says so is the arbiter's, not the drawing's. A non-square
footprint on a side wall used to lie portrait with its cells, because
the quarter turn it would need is one it could not pay for — written up
here as crooked on purpose, and reported by the next playtest as the
starting window rotating ninety degrees the moment it was carried one
wall over. It was never the roll: the CELLS turn. The net's side flaps
fold out sideways, so the two cells that lie level on the aft wall
stand one above the other on a flank, and the body lies on its cells
like everything else.

That was contained first and cured second. The containment refused a
non-square footprint on a flank by name (`Violation::Athwart`), which
made the game right about the shape of things by taking two of the four
walls away from every window and every painting. The cure is the thing
the containment said it was waiting for: **a kind states its extent in
its own frame** — across, deep, tall (`cargo::Kind::extent`) — and a
berth spends whichever two of the three its chart is for
(`Kind::face_on`):

- A chart a body lies **on** — the deck, the deckhead — spends the
  plan: across by deep.
- A chart a body hangs **against** spends the elevation: across by
  tall, and a flank takes that elevation **transposed**, because a
  flank's courses climb the sheet's x where the aft and front walls'
  climb its y.

So the cells turn under the body and the body does not turn at all,
which is what was true from the start. `Athwart` is gone with the wall
it used to close; the upright rule lost its affordability clause with
it (the roll a flank wants is now always the one its cells have paid
for); and a saved board carrying a flank window comes out hanging level
where the player left it (`STV19`).

> Superseded in part by DESIGN_REVIEW.md, *An old save or tape starts a
> new run*: a board saved before `STV19` is not read, so it comes out
> as a new run rather than as a level window.

The same statement takes the **deck apron** off the front of every
standing piece, and that is the half a playtest can point at. The
retired console's glyph `(w, h)` was doing all three axes' work, and
its second number was an elevation — so a deck berth read a wardrobe's
HEIGHT as depth and claimed 1.06 m of deck for a body reaching 0.53 m
into the room. Half a metre of bare deck in front of every wardrobe,
crate and floor lamp answered for the piece: the sim answers "which
piece is at this point", the point was inside its rect, and aiming at
a cabinet reaches into its cubbies — so a click on the floor came back
holding a transit chit. A deck berth spends across by deep now, one
cell deep, which is the same sentence `pieces::RIG_NEAR..RIG_FAR` says
in the frontend's units.

**The two sentences were the same length and not in the same place**,
which is the last of the apron and took four reports to find. A rig's
own `z = 0` is its BERTH PLANE, and `RIG_NEAR..RIG_FAR` runs from just
behind that plane to one cell out into the room — right on a wall,
where the plane is the chart the rig is screwed to. A deck berth has no
plane: the rect it is given spends the depth, so the cells own the
ground on both sides of their own middle and there is nothing for the
band to hang off. It hung off the middle of the cell anyway, and every
deck and deckhead berth in the game spent its air 0.2329 m out into the
aisle — 0.42 of a cell, on the one axis the plan spends its depth on,
with the other exact to the last bit, which is why the report was always
"half-way between cells on ONE axis". The bodies drawn inside that band
stood 0.117 m to 0.250 m off, kind by kind. `pieces::site_on` draws a deck or deckhead berth
back onto its cells now (`pieces::rig_mid`), and the carried ghost
promises the whole offset rather than the height alone. The harness
grew a family for it, because eleven of them had only ever asked
whether a body stayed INSIDE something (`berth-filled`, GAUNTLET.md).

The sweep learned the same sentence. It used to assert the crooked
ruling — up on the aft and front charts, sideways on the flanks — which
is a test restating the branch it is testing, and it passed on every
one of its two thousand berths while the window turned its corner. It
asks the player's question now: **a hung body reads up-is-up on every
wall it may take**, and **a berth owns the ground its body stands on
and not a cell more**, with the fold direction read off the net itself
rather than off the arbiter's own table.

### Occlusion: a defect class, named

Two failure modes surfaced in playtest and both are presentation
problems, to be solved in rendering — placement must never be refused
for camera reasons:

- **The carried piece blinds the carrier** (the brine pearls filling
  the view). The carry renders the held piece as a translucent ghost —
  the legality frame stays solid, the body goes see-through — so the
  room stays legible through it at every carry position.
- **Placed cargo can block a focus pose** (legal cargo between the
  fitted camera and its panel). Anything intersecting the camera→panel
  sightline while focused gets **x-ray treatment**: the occluder drops
  to a translucent outline until the pose releases. The static
  sightline tests keep proving the *architecture* never blocks a
  panel; x-ray covers what mobile cargo does at runtime.

### Consequences accepted

- Berth capacity inflates (floor 30 + walls + ceiling + hosts, versus
  the hold's 24). Scarcity is an economy lever and the economy is
  already queued for redesign; the grid does not pre-balance it.
- Every kind re-authors its extent for the plane it mounts (the couch
  is 2 wide × 1 deep on a real floor, not a 2×2 bas-relief). **Landed**
  — `cargo::Kind::extent` states across, deep and tall, and
  `Kind::face_on` spends the pair the chart is for.
- Save and replay formats bump (STV8 / RPL4) when the net lands; old
  saves migrate hold cells onto the aft-wall/floor charts they already
  present as.

  > Superseded by DESIGN_REVIEW.md, *An old save or tape starts a new
  > run*: no save migrates; one from before a bump starts a new run.

Build order: occlusion fixes first (presentation-only, immediate),
then the net in the sim, then the net's presentation, then the
instruments, then `supports:top` hosts. Each lands green or not at
all.

> Superseded in part by *Cargo stops colliding*: there will be no
> `supports:top` hosts.

## Lights are cargo

The ship owns no light. Every lumen aboard is a piece — the starter
ceiling lamp hanging over mid-floor, the starter sconce at the port
cornice, floor lamps, luminous coats, the firebox — all of it
tradeable, jettisonable, burnable, and all of it dimmed by the omen.
What remains when the last lamp goes is an ambient starlight floor
(silhouettes survive; the room reads black) and the ship's own glow:
screens and phosphor readings were always emissive, the icon etchings
and the brass hardware carry a faint radium-paint self-glow, and the
furnace's hazard tape carries the same floor in ember, because it is
warm iron — so a fully darkened ship stays playable on technicality,
chart to lever. Selling your last lamp is a legal, foolish, extremely
funny decision, and the game holds up its end.

**The law is about lumens, and none of those is one.** A light source
casts on its neighbours, so a source the ship owned would be a lumen
nobody bought and the economy would leak; an emissive lights its own
face and nothing else, which is why the floor has always been spelled
that way. The furnace is the case that made the distinction worth
writing down: it is the one riding room with no lamp of its own — its
light is whatever the crew last fed it — and every cell of it destroys
what stands there, so a ship that has not stoked its fire had a hazard
room nobody could find. Warm iron answers that; a lamp in the furnace
would not, because it would be a lamp the crew never bought and could
never sell.

## What stays out of this slice

Free (non-grid) placement, physics, multiple rooms, new pad surfaces,
and any barter redesign. The counter-as-diorama conceit and the
economy's shape are both expected to move once the barter design work
starts; the grid deliberately does not pre-empt it.

Two of those have since been taken up, and the grid's restraint paid:
multiple rooms and the barter redesign are specified together in
[ROOMS.md](ROOMS.md) — rooms attach to the cabin at declared ports and
carry room nets of their own, and the barter interface is deleted
rather than redesigned. Free placement and physics remain out, forever
as far as anyone can tell.

> Superseded by *The grid comes out*: free placement is in, by decree.
> Physics remains out.

> Superseded by *Cargo turns*: and it turns, at any angle, on every
> surface. Physics remains out.
