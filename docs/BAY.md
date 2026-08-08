# The walkable bay: the hold unfolds into the room

The cargo experiment's second slice. The hold leaves the desk and
becomes the aft half of the cabin: cargo at furniture scale, placed by
walking up to it, with the first piece of storage furniture — the
cabinet — stretching the berth architecture. This document also records
the project's largest scope decision so far: the 2D console retires.

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
- Carrying has reach: the grab/place ray only bites within arm's
  length plus a step, so placement always happens near the body and
  the piece never teleports across the room.

The barter counter is untouched this pass and keeps its desk scale.
The story: **the counter is the broker's diorama** — deals are struck
over scale models of the goods; the bay is where the real things sit.
A piece glides between scales as it changes berth. The whole barter
surface is due for its own redesign once the economy design settles,
so no further investment lands there now.

## The cabinet: furniture that stores

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
stowed piece back to the first legal hold cell. One known seam of the
fold: a standing cabinet straddles the wall band and the deck strip, so
its upper cubbies are aimed on the rig itself while the lower two are
aimed on the deck plate at its feet — the sim's flat-rect mapping is
the law, and the hint glow shows where the aim actually is. Worth
revisiting if playtests trip on it.

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

## Coverings: the dressing layer (second slice)

The owner's cargo direction — away from raw materials, toward rugs,
wallpaper, paints — lands as a **dressing layer parallel to
occupancy**: `Loc::Laid { x, y }`, a berth class where the piece is
applied *into* a surface rather than standing on it. A laid piece
coexists with occupancy on the same cells (a couch stands on a laid
rug), and no two dressings share a cell. Conservation never blinks: a
laid rug is still the same piece, and peeling it up is an ordinary
grab.

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
  `lit_adjacent` read lamps use: the rat fears glow-painted corners,
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

## Where placement rules go next

- **Occupancy berths** (hold cells, cubbies, pads) hold exactly one
  piece; the cabinet shows berths can be *provided by pieces*, so
  shelving, crates-with-compartments, and display cases are the same
  shape with different numbers.
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
reads as a furnished room, not a warehouse diagram — and the couch was
recomposed with true depth (rigs began as desk-era bas-reliefs where
+Z meant relief height; standing rigs re-purpose +Z as room depth, so
asymmetric furniture must put its back at the wall — the convention is
now documented at the couch rig).

## The room grid (spec): everything is cargo, everywhere is grid

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
- **Paintings and UI instruments must be on the wall.**
- **Lamps may hang from the ceiling** (the ceiling lamp finally means
  it); wall sconces stay wall, floor lamps stay floor.
- Floor cargo has a 3D extent in cells (footprint × height): tall
  cargo standing against a wall **shadows** the wall cells behind it —
  no painting behind the wardrobe. This is the one genuinely 3D rule;
  everything else is per-plane.
- Coverings (`Loc::Laid`) extend to every plane the mount table allows
  — rugs stay floor; paint always coated any surface.

### The no-soft-lock invariant

The player must never be able to construct a state they cannot act out
of. Three guards, all sim-side and monkey-proven:

- **`Violation::Sealed`**: a floor placement that would split the free
  floor into more than one connected region is refused like any other
  violation (no enclosed pockets, no walling off the burner doorway —
  connectivity covers both). Conservative on purpose: it never needs
  to know where the player stands, so it lives in `placement_check`
  with the other rules.
- **The vital-minimum rule** (unchanged from the old spec): a
  `Tag::Vital` kind refuses every exit ceremony while it is the last
  of its kind aboard — give pad, burner hopper, casino wager — one
  predicate (`last_vital_aboard`), one violation name. Spares sell
  fine; stations occasionally stock used instruments.
- **The frontend refuses a drop into the cells the player occupies**
  (a reach-style gate, like the carry's REACH — the sim has no player
  position and must not grow one).

### Instruments as cargo

- **New wall-mount kinds** (appended as ever): `ChartTank` (2×2),
  `EtaGauge` (1×1), `LaunchLever` (1×1). The transit window is a hole
  in the hull, not cargo. The barter counter stays put deliberately
  until the barter redesign moves it.
- **The logical rects stay the law**: `layout::MAP_PANEL` et al. never
  move; the *binding* moves — a mounted instrument carries its
  station's `SimSurface` at its own cells, so rulings and tape format
  never hear about it. The fixed console retires piece by piece.
- **Function follows presence**: charting needs a `ChartTank` aboard,
  launching needs a `LaunchLever` — predicates in the shape of
  `transit_chit_aboard`. The ETA gauge is passive.
- **Focus poses become relative to the instrument's berth** (or retire
  where roam-scale reading suffices); occlusion of a pose is handled
  by rendering, never by placement bans — see below.

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
  is 2 wide × 1 deep on a real floor, not a 2×2 bas-relief).
- Save and replay formats bump (STV8 / RPL4) when the net lands; old
  saves migrate hold cells onto the aft-wall/floor charts they already
  present as.

Build order: occlusion fixes first (presentation-only, immediate),
then the net in the sim, then the net's presentation, then the
instruments, then `supports:top` hosts. Each lands green or not at
all.

## What stays out of this slice

Free (non-grid) placement, physics, multiple rooms, new pad surfaces,
and any barter redesign. The counter-as-diorama conceit and the
economy's shape are both expected to move once the barter design work
starts; the grid deliberately does not pre-empt it.
