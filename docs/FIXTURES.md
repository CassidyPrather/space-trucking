# Fixtures: cargo that furnishes the ship

The cargo experiment's first slice ("actuate the 3D space"): five new
kinds that live in the room — lamps in three affixations, a couch, a
painting — plus mechanics tying cargo to light, vermin, and value.
Everything here obeys the standing law from
[ART_DIRECTION_3D.md](ART_DIRECTION_3D.md)'s cargo section: the sim
stays discrete, presentation gets physical, and every mechanic has a 2D
analogue.

## The conceit: the hold grid is the room

The 6×4 hold grid's edges are the room's surfaces:

- **row 0 is the ceiling**, **row 3 is the floor**,
  **columns 0 and 5 are the walls**.

A fixture kind carries an *affix* — the surface its footprint must
touch — enforced as a placement rule in the existing ladder (bounds →
heavy → cryo → **affix** → overlap → volatile → suspicious), exactly the
shape `Heavy` and `Cryo` already have. No new `Loc` variants, no layout
change, no save-format change: old `STV4` saves keep loading, the
drag-monkey tests cover the new rule the moment it exists, and lockstep
never learns anything happened.

In 2D the grid edges *are* the walls and ceiling — the analogue is
exact. In 3D the hold rack grows a gantry frame (top rail, deck lip,
side stiles) that the fixtures visibly mount to, and lamps emit real
light into the cabin.

## The five kinds (appended: indices 16..=20)

| Kind | Cells | Affix | Story |
| --- | --- | --- | --- |
| `CeilingLamp` | 1×1 | ceiling | a hanging shade; lit while stowed |
| `WallLamp` | 1×1 | wall | a sconce off a repossessed liner |
| `FloorLamp` | 1×2 | floor | a standing lamp, shade up top |
| `Couch` | 2×1 | floor | somebody's living room, in transit |
| `Painting` | 2×1 | wall | gilt frame, subject debatable |

Appending kinds keeps old saves parsing. It does shift future
procedural rolls (shelf stocks sample the kind space) on resumed runs —
accepted during prototyping, per DESIGN.md's compatibility stance.

## Mechanics

1. **Lamps are lit while stowed.** A lamp in the hold casts light on its
   orthogonal neighbours (same adjacency the volatile rule uses). 2D:
   a warm halo over adjacent cells. 3D: a real point light on the rack,
   obeying `sim.light()` like every cabin lamp — the omen dims cargo too.
2. **Rats fear light.** The rat will not hop to, or nibble in, a cell
   adjacent to a lit lamp. If every candidate is lit, it skips its beat
   — light is deterrence, not damage.
3. **The couch tempts the rat.** With a couch aboard, the rat drifts
   toward it (deterministic pathing, splitmix tiebreaks) and *naps*
   there: no nibbling while on the couch, lazier hop cadence. A couch is
   rat insurance that costs two floor cells — and the nap is the tell.
4. **Well-lit art.** A `Painting` adjacent to a lit lamp is worth one
   more everywhere — the one cargo-cargo-light interaction in the
   economy, and it only ever raises a price (the dial's monotone law
   holds).
5. **Blooming seedlings** (presentation only): `Seedlings` adjacent to a
   lit lamp draw in bloom, both frontends. No sim state changes — it is
   a pure reading of placement, like the placement hints.

## Value columns (base `VALUE` additions, 0..=6, lore-directed)

Venus buys tack (`Painting` 6, lamps high); Earth rations light; Saturn
treasures working fixtures (salvage); the **Umbra Market pays zero for
every lamp** — they sell midnight and consider light a rival product;
the Hermitage pays best for the `Couch` (comfort for one lit window).
Exact numbers live in `VALUE` in `src/sim/barter.rs`, tuned to keep each
new kind top-three somewhere (the wants row must be able to ask for
them).

## What stays out of this slice

Carry-style interaction, walkable bays, and any new berth surfaces
outside the grid — the affix conceit covers the experiment without
touching the solid tier. If fixtures prove out, those become the next
conversation.
