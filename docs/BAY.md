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

## Where placement rules go next

The bay is the proving ground for the owner's cargo direction: away
from raw-materials cargo, toward rugs, furnishings, windows, wallpaper,
paints. The berth architecture this slice establishes is meant to take
that weight:

- **Occupancy berths** (hold cells, cubbies, pads) hold exactly one
  piece; the cabinet shows berths can be *provided by pieces*, so
  shelving, crates-with-compartments, and display cases are the same
  shape with different numbers.
- **Coverings** (rugs, wallpaper, paint) will not be occupancy at all
  but a parallel per-cell layer — a surface *dressing* that coexists
  with a piece standing on it. That layer does not exist yet; nothing
  in this slice blocks it, because cells already have stable
  identities for it to key on.
- **Networking is unaffected** by any of it: lockstep ships input
  frames, berth transitions stay discrete, and the conservation
  monkeys keep proving no interleaving loses a piece.

## What stays out of this slice

Free (non-grid) placement, physics, multiple rooms, new pad surfaces,
and any barter redesign. The counter-as-diorama conceit and the
economy's shape are both expected to move once the barter design work
starts; the bay deliberately does not pre-empt it.
