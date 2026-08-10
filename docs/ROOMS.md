# Rooms: the ship attaches what it meets

The owner's decree, turned into law. The ship stops being one box with
annexes bolted to it and becomes a **graph of rooms** that instantiate,
attach, and part again — at points of interest, at events, and (later,
through this same interface) when a crew member joins. Cargo is dragged
between rooms the way it is carried across the cabin today, because
cargo is the only interface this game has left.

Two things land in one stroke, and they are the same thing seen from
either side: **UI-as-cargo, fully committed** (the last screen-shaped
interface in the game is removed rather than redesigned), and
**rooms as the unit of architecture** (the thing you attach a POI to,
an event to, a burner to, and one day a crewmate to).

Read [BAY.md](BAY.md) first for the room net, the handle rule, and the
standing rule; this file generalizes all three from one room to many.
[NETWORKING.md](NETWORKING.md) owns the lockstep law that the
attachment interface must satisfy, and does.

## The decision: the barter interface is removed, not redesigned

[DESIGN_REVIEW.md](DESIGN_REVIEW.md) has carried "barter redesign" on
the deferred list since the playtests called the trade minigame
unengaging. The redesign is now decided, and the decision is **delete**:
the pads, the eagerness dial, patience, the fog, the accept lever, the
station shelf, and the whole desk-scale counter minigame come out.

The proximate reason is a paradigm mismatch nobody should pay to fix.
The bay speaks **carry** — click to pick up, walk, click to set down
(BAY.md, "Carry, not drag"). The counter speaks **drag** — press, hold,
release over a slot. Two grammars for one verb, joined by a gesture
layer that synthesizes one from the other, in a surface already
scheduled for demolition. Squaring them is real work spent on a corpse.

The deeper reason is the direction the project has been walking for
three slices: the console retired into a room, the instruments came off
the wall and became cargo, the light became cargo. The barter counter
was the last fixed console face in the game. It goes the same way
everything else went — except it does not become cargo, it becomes a
**place**: the station's own room, attached to yours, with its own
grid and its own floor.

**One gesture, everywhere: carry.** After this there is no press-hold-
release interaction in the game at all.

Rejected alternatives, for the record:

- **Square the two grammars** (make the desk accept carry). Cheapest,
  and buys a mechanic the owner has already decided to remove.
- **Keep the counter as the broker's diorama** (BAY.md's conceit) and
  redesign only the economy. Rejected: the diorama was always labelled
  a placeholder, and a scale-model counter cannot host cargo the player
  physically walks over to.
- **A modal trading screen.** Rejected on sight — text-free, screen-free
  and diegetic are load-bearing (DESIGN.md), and a modal screen is the
  2D console coming back through a window it was carried out of.

What survives the demolition is stated once, as a law:

> **The economy survives its interface.** `barter::VALUE`, the per-visit
> ±1 jitter, the wants row, the `familiar` masks, `deal_value`,
> `GNAW_MALUS`, the Umbra Market's gnaw premium, the well-lit-art
> bonus, and the Hermitage's karma are *economy*, not *interface*. They
> keep working, unchanged, behind the new flow. Only what the player
> touched with a mouse dies.

## The port law

> **A room declares only the ports it needs.**

The owner's decree, which this law is, verbatim:

> "The incinerator room and spacing-guild room do **not** need to have 4
> doors and ladder/hatch (i.e. they can be dead-ends) - in fact, I think
> it makes them look way too busy. It's only the player cabins which need
> to be extensible - after all, they're going to be melded together in
> huge whacky amalgamations like linkin' logs later on!"

There are **six slots**, and the slot *is* the position:

| Slot | Plane | May hold |
| --- | --- | --- |
| 0–3 | the wall of the same index (aft, starboard, front, port) | one door, or nothing |
| 4 | ceiling | the ladder, or nothing |
| 5 | floor | the hatch, or nothing |

Six is a **bound, not a quota**. What each kind fills is a design
decision, argued at the declaration and pinned by a test:

| Kind | Declares | Because |
| --- | --- | --- |
| **Cabin** | four doors, ladder, hatch | The extensible piece. Cabins are the linkin' logs the crew melds into amalgamations, so a cabin faces four ways and any cabin mates any cabin on any side — and the vertical pair is the frontier the whole ship's growth rests on (below). |
| **Burner** | one door, port wall | The incinerator, named in the decree. It has hung off the cabin's starboard wall since the first slice and that is the seam it keeps. No hatch: a furnace mouth in the deck is a second way the fire can travel, and a hopper you can fall into is not a hopper. |
| **Trade** | one door, aft | The Guild's room, also named in the decree. A market is a place you visit, not a corridor. A second door was considered — two cabins sharing one market — and rejected: the graph already lets both crews walk to it through their own ship, so the second door buys nothing but the clutter the decree objects to. |
| **Wreck** | one door, aft | A derelict has exactly one seam worth trusting: the one you just mated. The rest of the hull is vacuum with edges. |
| **Parlor** | one door, aft | The casino's parlor, whose whole conceit is that it has no visible doors. It gets the one it cannot do without. |
| **Pump** | one door, aft | A forecourt: come alongside, top up, cast off. Fuel reaches the furnace in a crewman's arms, the way everything else in this game travels. |

Four clauses, each doing work:

1. **A port's position is data, never an assumption.** Which wall a
   door is on, and which cells of that wall it punches, are declared by
   the room and read from the declaration everywhere — by the embedder,
   by the grid's validity mask, by the rig that draws it. No rule and
   no rig may assume a port is where it was last time, because the
   stretch goal (below) makes doors, ladders, and hatches into
   re-arrangeable cargo with amber grab handles, and that day must not
   be a rewrite. Code that hard-codes "the door is on the starboard
   wall" is the defect this clause exists to forbid.
2. **An undeclared slot is not a port; it is a wall.** Naming one is
   `Absent` — the same refusal a door carried off its wall will give
   when the stretch goal lands, which is not a coincidence: a room that
   never had a door and a room that lost one are the same room to every
   rule downstream. An undeclared slot punches no cells, opens no seam,
   mates nothing, and is drawn as the plane it sits in. **Fewer plates,
   because fewer ports** — the presentation derives its doorways from
   this declaration and nothing else, so sparing a room its ports is
   what makes it stop looking busy.
3. **At most one door per wall**, which stopped being a rule anyone can
   break and became arithmetic: the slot index *is* the wall. Two
   apertures on one plane would make the closure arithmetic below
   ambiguous, and now they cannot be expressed.
4. **The vertical pair is the cabin's.** The ladder and the hatch are
   the escape hatch in the literal and the engineering sense, and the
   cabin is the room that carries them (see "The escape-hatch
   guarantee"). Nothing else declares either, and a leaf room that did
   would be claiming an extensibility it has no use for.

Three invariants bind every declaration, whatever it declares: an
aperture lands **wholly within** its own wall, floor, or ceiling; two of
a room's ports **never share a cell**; and mating apertures are
**identical** (the closure law below). Those are the geometry's, not the
decree's, and they did not move.

Ports are the **only** way a room connects to another room. There is no
open floor plan, no shared volume, no seam that is not a declared port
mated to a declared port. A room's box is otherwise sealed.

Rejected, for the record: **a port count as tuning per instance** (this
room got three doors from the seed). Rejected because a kind's ports are
its architecture — the save's edge list names them by slot, the paint
reads them, the spawn walk plans around them — and a number the seed
picks is a number no law can rely on.

## The graph is general, and cycles are allowed

The attachment graph is a **general graph**: rooms are nodes, mated port
pairs are edges, and there is no acyclicity constraint. Two rooms may be
joined by two different doors. A ring of four rooms may close on itself.
A cabin may reach degree six; a leaf room reaches degree one, and that
is what "dead end" means in a graph.

The owner chose this against the simpler options, and the reasons are
worth keeping:

- **A tree** (every room has exactly one parent) is far easier: it
  always embeds if any free direction exists, detach is a subtree
  prune, and no cycle ever has to close. Rejected because a tree makes
  every room a cul-de-sac, and DESIGN.md's multiplayer section asks for
  the opposite — attached areas "as high traffic as possible", with the
  generic parts of the ship boring and off to the side. Traffic is a
  loop. Cor's "hallway module" note is the same observation from the
  other end.
- **A star** (everything attaches to the cabin, nothing to anything
  else) is easier still and caps the design forever at six neighbours
  and zero chaining.
- **A fixed floor plan** (rooms slot into authored bays) removes the
  embedding problem entirely and removes the whimsy with it: the ship's
  shape stops being something the trip did to it.

The cost of the general graph is paid in one place, deliberately:

> **A cycle in the graph must close in space, exactly, or it is not an
> edge.** The graph does not float free of the room it describes.

## The lattice: every room occupies real space

Attachment is a **geometric** operation validated before it is a
topological one.

- **The lattice.** One shared integer lattice for the whole ship, in
  units of `BAY_CELL` (the room grid's cell). Every room is an
  **axis-aligned box**: an integer footprint (width × depth in cells)
  at an integer origin, with an integer yaw of 0/90/180/270. No free
  rotation, no fractional offset, **no floating point anywhere in the
  attach contract**. Determinism is not a hope here; it is integer
  arithmetic on integers.
- **One storey, everywhere.** Every room's walls are the cabin's height
  (3 cells). Uniform height is what makes any door mate any door and
  makes a ladder's neighbour sit exactly one storey up — the vertical
  mate is exact for the same reason the horizontal one is. Rooms of
  varying height were rejected: they need ramps or stairs, they break
  vertical exactness, and DESIGN.md wants the space cramped.
- **Walls have no thickness on the lattice.** A room occupies its
  interior; the partition between two attached rooms is a boundary, not
  a volume. Two rooms may share a plane and may never share a cell. The
  shared partition is **drawn by the room with the lower id**, once, so
  the seam cannot z-fight itself (the interpenetration convention at
  the couch rig, one scale up).
- **The pose is derived, never authored.** Given the anchor room's
  pose and a port pair, the attaching room's pose is *determined*: the
  two apertures must be coincident and their planes opposed (a door
  mates a door on the facing wall; a ladder mates a hatch one storey
  up), which fixes translation and yaw together. **Attachment has zero
  degrees of freedom.** Nothing chooses where a room lands; the mate
  computes it, and every replica computes the same one.

### Validation, in order

An attach request `(anchor, anchor_port, room_kind, port)` is validated
as a whole before anything changes:

1. **Ports exist and are free.** Both named ports are declared by their
   rooms and unmated. (A door carried off the wall does not exist:
   `Absent`.)
2. **Kinds mate.** Door↔door, ladder↔hatch, hatch↔ladder. Nothing else.
3. **The pose is computed** from the mate — the only pose there is.
4. **The box is clear.** The new room's interior must not intersect any
   placed room's interior. Sharing boundary planes is expected; sharing
   one cell is refusal.
5. **Every induced seam closes** (next section).

Only then does the room enter the lattice, the graph, and the save.

## Closure: what happens where two rooms touch

Placing a room can bring its walls flush against rooms it was never
attached to. That is where cycles come from, and where clipping would
come from if the contract were sloppy. The law:

> **Identical or disjoint.** Wherever two rooms share a boundary plane,
> the apertures those two rooms punch in that plane must be either
> exactly coincident (same cells, opposed, same aperture kind) or
> completely disjoint. Any partial overlap refuses the attach.

Three consequences, all intended:

- **Coincident apertures mate automatically.** An implicit edge forms;
  the cycle closes; you can walk the loop. This is the *only* way a
  cycle is ever created, and it costs no new interface — you attach a
  room and discover the ship now has a ring, which is exactly the kind
  of thing this game should let a trip do to a ship.
- **A door facing blank wall is sealed, not refused.** It is a door
  that will not open, drawn shut. Refusing here would make packing
  brittle for no gain: a shut door is honest, and re-siting the wall's
  other neighbour may open it later.
- **A half-overlapping aperture is refused.** An opening that leads
  half into a room and half into solid wall is a geometric
  contradiction, not a lie the renderer should be asked to tell.

Exactness is available because the lattice is integer. There is no
epsilon, no tolerance, no "close enough to close" — a cycle that would
need a half-cell of slack does not close, and the attach that would
have made it is refused. **There is no rubber hallway.**

### The other side of the plate

A room has an outside, and the outside is the same room.

> **One pose, two sides.** A room's exterior shell is derived from the
> very `Plan` pose its interior is built from (`room::hull_box`), and
> from nothing else. The exterior may not be authored, offset, or
> "adjusted to look right": if a station needs to sit somewhere else,
> that is a pose, and the sim owns poses.

This is what makes the transit window honest. Look out of it at a wall
a room is mated to and you are looking at that room's plate, ten
centimetres away, because that is where the lattice put it; look along
the hull and the rooms you have collected are strung out exactly as
they are strung out inside. Nothing about the view is a second opinion
about the ship's shape (see [ART_DIRECTION_3D.md](ART_DIRECTION_3D.md),
"The window is a hole").

The shell a room wears by default is deliberately plain — plate, a seam
belt, corner posts, a running light or two — and the per-POI design
agents dress it through two named seams (`poi::Character`'s `outfit` for
the kit and `dress` for hardware), documented at the art direction.
The cabin is the one room whose shell is not a lattice box: its hull
was hand-built before the lattice and stands a working gutter forward
of its floor box, so its outside is the union of the masses that ARE it
(`rig::structure`) — the same exception, and the same declaration of
it, that `chart_inset` already carries.

## Refusal semantics

> **Overlap is prevented by law, never discovered as clipping.**

- **Attach is atomic.** Validate the whole thing — pose, box, every
  induced seam — then commit or refuse. There is no partial attach and
  no rollback path, because nothing was written.
- **Refusals are named**, one variant per rule, in the same shape as
  `Violation` so the presentation can flash the matching tell:
  `Absent` (no such port — no such room, no such slot, or a slot this
  kind does not declare), `Mated` (port already in use), `Blocked`
  (the box intersects placed geometry), `Aperture` (a seam that will
  not close identical-or-disjoint), `Full` (the room budget).
- **A walk reports the deepest refusal it reached**, not the last one
  it happened to try. With most slots empty on most kinds, `Absent` is
  the cheapest answer and the least true one: a ship with no space left
  must say `Blocked`, and a ship whose seams will not close must say
  `Aperture`.
- **A refusal is a cue, not an error.** The sim pushes a refusal cue
  and nothing else happens. Same posture as `placement_check`: the
  arbiter answers, and every affordance the frontend draws derives from
  that answer instead of re-deriving geometry (DESIGN_REVIEW.md's
  affordance line).
- **A door re-arrangement is an attach-shaped operation** and runs the
  same validation, for the same reason.

## The escape-hatch guarantee

The vertical pair exists so the ship can always grow. It used to be
justified by every room carrying a ladder and a hatch; the port law
spared the leaf rooms theirs, so the guarantee is restated on the
structure that actually carries it — **the cabins** — rather than
quietly weakened.

**The spawn contract.** When the game must attach a room — docking at a
POI, an event firing, later a crewmate joining — it walks candidate
port pairs in a fixed deterministic order (the anchor's ports by slot:
doors by wall index, then the ladder, then the hatch; then outward
through the graph in id order) and takes the first that validates.
Slots the kind does not declare are refused `Absent` in passing, which
costs a lookup and keeps the order stated in one place.

**The guarantee, in two clauses.**

1. **A cabin always finds a berth.** While the room budget holds, the
   walk cannot come back empty for a cabin. Only cabins declare the
   vertical pair, so a storey can only ever be reached by a hatch
   mating a ladder — which means **every occupied storey holds a
   cabin**, the topmost of them has an unmated ladder, and the storey
   above it has never been reachable by anything. The mate that lands
   there is clear by construction: same footprint, empty level,
   nothing below it with an opening to argue about.
2. **A caller always fits the cabin that just arrived.** A calling room
   carries one door and mates a cabin's. A cabin arrives with four
   unmated doors on a storey nothing else has reached, so a caller of
   any kind fits one of them.

**What this does *not* claim, said plainly.** Doors are finite. A ship
whose doors are all spent refuses the next caller by name, and that
refusal is honest rather than a bug: the yard-fresh ship seats **three**
callers (the cabin's four doors, less the furnace's), which is more than
the game asks for — a POI's room and one unresolved event — and when a
crew wants more doorway, clause 1 is the answer. *The refusal is never
permanent.* That is the whole promise, and it is the one the game needs:
nothing can strand the ship in a shape it cannot grow out of.

This is stated as an obligation, not a hope: **the implementation must
carry a property test that builds ragged ships in adversarial spawn and
part orders, and on every one of them attaches a cabin and then a caller
of every kind against it.** It must also pin the honest half — that
three callers fit the yard-fresh ship and the fourth is refused — so
nobody mistakes the bound for a bug later. If clause 1 or 2 can be made
to fail, the guarantee is wrong and the spawn contract — not the test —
gets fixed.

Clause 1 leans on one property of the graph the ship actually holds:
**it is connected**. The gangway law sees to that — a room parts with
everything standing behind it, so no orphan is ever left floating at a
storey with no cabin on it.

The vertical points are also why the movable-aperture stretch goal
below must never let a cabin's ladder or hatch be carried away: see
there.

## The attachment interface

This is the contract multiplayer inherits. It is written now, wired
later. DESIGN.md's "when a player connects to the instance, they attach
their area to the ship somehow — I'm not sure topologically how that's
going to work" is answered here: **a crewmate is a room, joining is an
attach, leaving is a detach, and their door being shut is the sealed
port from the closure law.** The room a crewmate brings is a **cabin**,
which is why the cabin is the kind that keeps all six ports: the
amalgamation the decree describes is cabins melded to cabins, and every
face of one has to be able to take another.

### Lifecycle

| Stage | What happens |
| --- | --- |
| **Instantiate** | A room kind is realized from `(seed, cause, tick)` — the POI docked at, the encounter rolled, the joining player's index. Its grid, tile classes, ports, and starting stock all derive; nothing is authored at runtime. |
| **Attach** | The validated mate above. The room takes a dense `RoomId`, enters the lattice and the graph, and its grid becomes addressable. |
| **Live** | Ordinary cargo rules apply across the seam: carry, berths, coverings, violations. A room is not a mode. |
| **Detach** | The gangway law's gates pass, the edge (or edges) is cut, the room leaves the lattice, and whatever is still the room's own goes with it. |
| **Dispose** | The `RoomId` is freed for reuse. Rooms are not pieces; they carry no serial identity, and the graph is the only truth about them. |

### What crosses the interface

| Crosses | Never crosses |
| --- | --- |
| `Attach { anchor, anchor_port, kind, port }` — four small integers, as an input | Room poses, boxes, or lattice occupancy |
| `Detach { room }` — one integer, as an input | Cargo contents, counts, or berths |
| The acting player's **occupied room id** (see below) | Player positions, of any kind |
| Nothing else | Validation results and refusals — each replica computes its own, identically |

Everything above rides the input schedule as ordinary input, so
NETWORKING.md's one big decision holds verbatim: **nobody ever transmits
game state; only inputs travel.** The room graph is a pure function of
(seed, input schedule) exactly as the cargo board is. A save string is
still a join ticket, because the save carries the graph as its **edge
list in attach order** and every pose is re-derived from it on load — a
save cannot disagree with the lattice, because it does not store the
lattice.

**Room kinds are an appended table**, like `Kind`: new kinds go on the
end and old saves keep parsing (FIXTURES.md's convention, unchanged).

### The one new input field

The launch and detach gates need to know **which room a player's body is
in**, and BAY.md is explicit that the sim has no player position and
must not grow one. The resolution:

> **The sim learns rooms, not positions.** `InputFrame` grows one field:
> the room the player's body occupies. A room id is a discrete berth-
> shaped datum, not a coordinate; it travels as input, seals into the
> schedule, replays off the tape, and gives the gates a law instead of
> six frontends' private opinions.

Rejected: keeping the aboard gate frontend-only. Under lockstep that
diverges in the worst possible direction — a remote crewmate standing
in the POI room would not block *my* launch, and the game's one
unbreakable promise is that nobody and nothing gets left behind. The
replay tape bumps a version for the added field; that is the whole cost.

### Determinism obligations

- Attach, detach, and refusal are pure functions of sim state and
  input. No clocks, no engine types, no iteration over hash maps.
- The spawn walk's candidate order is fixed and documented at the code.
- The flaky harness's six properties (NETWORKING.md) must stay green
  with rooms attaching and detaching mid-session, and the conservation
  monkeys must run **across seams** — a carry that crosses a doorway is
  the interesting interleaving now.

## Room grids and colored tiles

**Every room is a room net.** BAY.md's net — the box unfolded into a
cross of six charts, laid into the sim's logical space, with a validity
mask and a `surface_of` classifier — generalizes from a singleton to a
family. The cabin is simply the room you start in.

- Each room kind declares its own net: floor extent, wall height (3),
  aperture punch-outs, and tile classes.
- Each attached room is allocated a **net lane** — a reserved rect of
  the sim's logical space, indexed by its dense `RoomId`, big enough for
  the largest room net. Lanes are fixed, so a room's logical rects are a
  pure function of its id and no attach ever reflows another room's
  coordinates. `SimSurface` binds quads to those rects exactly as it
  does today; the sim keeps making every ruling.
- `Loc` gains a room qualifier: hold cells and laid coverings are
  `{ room, x, y }`. Cubbies do not need one — a cabinet knows what room
  it stands in. The cabin is room 0, which is also how pre-rooms saves
  migrate: every old berth is a room-0 berth.

### The tile-class vocabulary

Certain cells are **colored**: designated regions whose look is a
reading of their behavior. The classes, closed in the sim and open in
this document — a new class lands here with its behavior *and* its look
in the same change:

| Class | Reads as | Behavior |
| --- | --- | --- |
| `Plain` | bare deck, wall, ceiling — the ground the others are read against | an ordinary berth; the default |
| `Offer` | bare deck inside a chalk line struck round the whole area | the fundamental colored tile. Player cargo berthed here is **proposed**, not surrendered: it stays the player's until a resolution says otherwise |
| `Stock` | the room's enamel, filled, and bordered where the paint ends | the room's own goods. Not player-owned; may not be carried out until a resolution grants them |
| `Consume` | scorched plate with hazard tape round its rim | anything berthed here is scheduled for destruction on the room's own beat — the burner's hopper is the first and today's only instance |
| `Threshold` | a studded tread and a brass sill, on the deck the door stands on | an aperture's footprint. Never a berth (see the threshold rule) |

Four laws over that table:

- **The color is the behavior's own reading, never decoration.** The
  tile class is declared once by the room kind; the rules and the paint
  read the same declaration. A tile that *looks* like an offer area and
  does not behave like one is the affordance defect DESIGN_REVIEW.md
  already forbids.
- **Ownership is a function of tile class**, not of a hand-written list.
  `player_owned` stops enumerating `Loc` variants and starts asking the
  berth's room and class. This is THE ownership rule and it must never
  be restated anywhere.
- **No signal on hue alone** (ART_DIRECTION_3D.md): every class carries
  a form as well as a hue. The per-POI agents vary the *look* of a class
  freely; they may never vary its *behavior*.
- **And no pattern on pattern.** The first cut of this table gave every
  class a *stamp* and stacked them: hazard tiles on a hazard-striped
  deck under ember edges, hatched enamel under a striped doormat. The
  playtest read it as noise, correctly, and it is now forbidden — a
  class is told by the **kind** of mark it wears (solid field, struck
  line, edge banding, sparse studs), a mark is drawn on the **rim** of a
  region rather than stamped into every cell of it, and **stripes belong
  to `Consume` alone**, because hazard tape is the real-world idiom for
  *this will hurt you* and a second claimant makes both meaningless. The
  full vocabulary, and the three-rung decal ladder that keeps a field, a
  mark, and a tread from sharing a plane, live at `cabin::room::tiles`.

### The threshold rule

An aperture's cells belong to two rooms at once. Cargo berthed there
would sit in two grids, and a detach would have to pick which grid
keeps it. So: **nothing berths on a threshold.** `Violation::Threshold`
is the refusal, and it is the *principled* replacement for the dying
`Violation::Aisle` — the doorway stays clear because it is shared
space, not because a walking body needs the room.

## The new barter: six beats

The core slice, one flow for every POI. Per-POI agents differentiate
look and behavior later, from the lore that already exists; the flow
below is what they differentiate *from*.

1. **Carry cargo out of your ship.** The ordinary carry, through the
   doorway, into the attached POI room. (The decree says "drags"; the
   gesture is BAY.md's carry, which is the one gesture left.)
2. **Set it on the offer area.** `Offer` tiles, in the POI's room. The
   pieces are still yours — this is a proposal, and a proposal you can
   pick back up.
3. **Click stock to indicate interest.** The handle rule already
   reserved the body-click for function: on a piece of the room's
   `Stock`, that function is *I want that one*. Marks are a hint to the
   offer, never a demand, and they clear on resolution.
4. **The room answers with an offer.** Core slice: the existing
   deterministic economy does the arithmetic — the station's `VALUE`
   row, jittered ±1 for this visit, its wants row, the gnaw malus and
   the Umbra Market's premium, the well-lit-art bonus. The room
   composes the best pile of its own stock the proposed value covers,
   preferring marked kinds, ties broken deterministically, and **places
   those pieces on its offer area**. The offer is not a number and not a
   needle: it is goods, on tiles, that you can see. No currency, no
   text, nothing to read.
5. **Accept, or keep bartering.** Acceptance is a physical act at the
   room's own **handshake** — one click-functional room fixture whose
   form is per-POI (a chit press at the Guild, a bell at the Hermitage,
   whatever Venus thinks is tasteful) and whose behavior is fixed: it
   commits the standing offer, and ownership crosses. Otherwise add,
   remove, or re-mark and the room answers again. (Implicit acceptance
   — ownership crossing the moment you carry something aboard — was
   rejected: it cannot tell "I accept" from "I changed my mind about
   offering this", and it makes the one moment that matters invisible.)
6. **Carry what is yours back aboard.** Only then may you launch. The
   gate is the next section.

What is deliberately *not* in the core slice: patience, the fog of
unfamiliar goods, the shuttering, and the eagerness dial. Discovery
cost and temperament are exactly the kind of thing a per-POI agent
should own, and the `familiar` masks keep being learned meanwhile so
those agents have something to read when they arrive.

## The gangway law

> **A seam that could strand anything refuses to part.** Nothing
> detaches while it holds something of yours; nothing of yours detaches
> while it holds you.

Two gates, sharing one predicate family.

**Detach gates** — a detach is refused unless:

1. **Aboard**: no player's occupied room is the detaching room, or any
   room reachable only through it.
2. **Cargo aboard**: no player-owned piece rests in the detaching room
   (or in anything reachable only through it), and no piece is held in
   hand across the seam — a held piece snaps home first, exactly as it
   does for a vanished pointer (NETWORKING.md's leave rule).
3. **Resolved**: the room's business is finished — no pending offer on
   an `Offer` tile, no unresolved event.

**The launch gate** — the launch handle refuses unless:

1. every crew body is in a **riding** room,
2. no player-owned piece rests in a **calling** room,
3. every attached POI room's offer is resolved,
4. no unresolved event room is attached.

Rooms are one of two kinds, and this is the distinction the gates turn
on:

- **Riding rooms** travel with the ship: the cabin, the burner, and
  (later) crew modules.
- **Calling rooms** come alongside and leave: POI rooms, event rooms.

Departure detaches every calling room as a consequence of casting off,
which is safe precisely because the gate ran first. This generalizes
today's `pads_occupied` refusal — "no destination, or pieces on a pad or
the received shelf: launching would strand them, so nothing is ever lost
to the lever" — from four slots to the whole graph. The vital rule
(BAY.md) is untouched and still guards ability: a ship that cannot chart
or launch is a soft-lock whether or not anyone is stranded.

**An unresolved event blocks the next takeoff.** A POI room and an
unresolved event room may be attached at the same time — there are
plenty of attachment points, and being interrupted mid-trade by
something knocking is good.

## Events as rooms

> **An event with a counterparty or a place becomes a room. An event
> that is weather stays a schedule.**

The derelict has a hold, the gas station has a pump bay, the casino has
a parlor with no visible doors, the whale has *something* — those
attach. The omen, the meteor shower, the rats, and the ad drone are
weather and stay exactly what they are: schedules hashed off the seed,
with cues.

Two rules on event rooms:

- **Resolution always includes a free way out.** DESIGN.md's law is
  that events "must be feasible to ignore (disengagement is
  participation)". So an event room's resolution set always contains
  *shut the door* — one deliberate act at the aperture, costing
  nothing, available immediately. "Unresolved blocks the next takeoff"
  therefore means "you must at least close the seam", never "you must
  play the event".
- **Salvage arrives in the event's own room.** Encounter flotsam stops
  needing an outboard rail: the derelict's cargo is on the derelict's
  floor, and taking it is a carry through a doorway. This retires the
  last use of `Loc::Flotsam` that was not the fuel hopper, and with it
  the `FLOTSAM_SLOTS == SHELF_SLOTS` exclusivity hack (see "What dies").

## The burner room

Today the burner is the odd one out: an airlock annex carved off the
starboard wall, four hazard-bordered tiles bound to `Loc::Flotsam`
slots that are *the same rects as the station shelf*, live only while
no barter is open. It becomes an ordinary room, attached through the
ordinary interface, and every one of those special cases dissolves:

- It is a **riding room**, attached at a port and travelling with the
  ship. It is detachable, with the gates — selling your furnace is
  legal, foolish, and supported, in the tradition of selling your last
  lamp.
- **Hopper staging moves into its grid** as `Consume` tiles. Staging is
  an ordinary berth transition into an ordinary room; snatching a piece
  back out is an ordinary carry.
- **The stoker's beat** reads the lowest occupied `Consume` cell in the
  burner room, in the room's own row-major cell order. Same twelve-
  second cadence, same `Cue::Burn`, same flammability arithmetic, same
  banked stoke.
- **The exclusivity hack dies with the barter.** There is no shelf row
  to share, so there is no "the rail IS the shelf row" law, no
  no-barter-open gate on the tiles, and no save-parser rule refusing
  staged flotsam beside an open barter.
- **Fuel simply stays staged.** With the hopper no longer a contested
  surface, docking has no reason to bank it: unburned fuel waits in the
  furnace room across the dock, and `Cue::Jettison` — "the one ceremony
  that still discards" — retires. Conservation becomes total, which is
  a checklist line the game has been asking for. This is the spec's
  call rather than the decree's, and it is cheap to reverse if the
  banking ceremony turns out to be missed.

## The cabin widens

Landing in parallel, stated here as fact: **the cabin's floor grows from
6×5 to 8×7 cells — one tile in each direction — and the walls stay 3
tall.** The ceiling follows the floor. The net's bounding grid grows to
match by the same cross arithmetic the current net uses.

Two notes for whoever holds the ruler: the widened floor is the cabin's
dimension, not a universal — other room kinds declare their own
footprints — and the extra floor is a berth-capacity change the economy
does not pre-balance, on the same terms BAY.md already accepted.

## What dies

**The barter machinery**, entire:

- `Loc::GivePad`, `Loc::TakePad`, `Loc::StationShelf`,
  `Loc::ReceivedShelf`, `Loc::Flotsam`.
- `Barter`'s `eagerness`, `prev_eagerness`, `ready`, `fog`, `patience`;
  `Sim::conclude`, `spend_patience`, `restock_from_give_pads`,
  `received_occupied`, `pads_occupied`.
- `layout`'s `BARTER_PANEL`, `SHELF_SLOTS`, `RECEIVED_SLOTS`,
  `GIVE_SLOTS`, `TAKE_SLOTS`, `ACCEPT_LEVER`, `DIAL_CENTER`,
  `ENCOUNTER_BADGE`, `FLOTSAM_SLOTS` — and with `FLOTSAM_SLOTS`, the
  `FLOTSAM_SLOTS == SHELF_SLOTS` aliasing and every rule that existed to
  keep those two meanings from colliding.
- The cabin's counter rig and its 1,600 lines of desk (`barter.rs`),
  the counter apron's aisle cells, and the counter's sightline
  exemption in the workability test.

**The Sealed invariant and the Aisle rule**, because **player-character
collision with cargo is removed**. The no-soft-lock invariant existed to
stop a player boxing themselves in; with no collision there is no box,
and a connectivity check on the floor is machinery guarding a state that
cannot occur. `Violation::Sealed`, `seals_the_floor`, `layout::AISLE`
and `Violation::Aisle` all go. What replaces the doorway's clear
threshold is the threshold rule above, which keeps it clear for a reason
that survives: an aperture belongs to two rooms.

The frontend's reach-style refusal of a drop into the player's own cells
goes with the collision. The vital rule stays — it guards *ability*, not
mobility.

### Migration posture

- **Legacy mid-trade saves resolve on load**: every player-owned piece
  on a pad or the received shelf walks back aboard to the first legal
  berth (conservation before convenience, the pre-STV8 posture), and the
  station's own stock — shelf and take pad — is dropped, because the
  station it belonged to no longer exists as a place.
- **Saves bump** for the room graph (edge list in attach order, plus the
  room qualifier on berths) and the tape bumps for the input frame's new
  room field. A save that lies fails safe into a fresh run, as ever.
- **The port law bumped the save again** (`STV12`). The slot numbering
  did not move — a door is still its wall's index, the ladder 4, the
  hatch 5 — so the edge list's grammar is untouched and every edge
  through a port its kind still declares replays exactly. What a
  document written under the old law *can* now say is a slot nobody
  fills: a pump bay under the cabin's ladder. Such a room is **re-seated
  through the spawn walk**, keeping its id and everything berthed in it.
  Conservation before convenience: the ship comes back a different
  shape, never a shorter one. Only old documents get that mercy — a
  current save that disagrees with the lattice is a save that lies.
- The design-review **checklist** keeps its current wording until the
  removal actually lands: two lines will need re-pointing on that day
  (the "except through the accept lever" clause of the conservation
  line, and the monotone-instruments line, whose subject is the dial).
  They are honest today and this file is a spec, not a diff.

## Stretch goal: apertures as cargo

Doors, ladders, and hatches become **re-arrangeable cargo with amber
grab handles** — the handle rule applied to architecture. A door is a
wall-mount kind, click-functional (it opens and shuts), so it wears a
handle and its body-click works the door while its handle carries it.
Carrying a door off a wall leaves blank wall; hanging it on another wall
puts a port there. The ship's plan becomes something the crew arranges.

The packing hazards, named, with the rules that pay for them:

1. **Tearing a live edge.** Lifting a mated door would cut a seam with
   a room on the other side of it. Refused: **a mated aperture cannot be
   lifted.** This is the cabinet's `Occupied` rule one class wider —
   empty it first, detach first. It also disposes of the whole class of
   "a cycle closed through a door the player later moved", for free.
2. **Two doors on one wall.** Refused by the port law; `Aperture` is
   the name.
3. **Carrying away the guarantee.** If a cabin's ladder or hatch could
   be removed, the escape-hatch guarantee would be a lie. So **ladders
   and hatches are vital** in the existing `Kind::vital()` sense: the
   last of each in the player's keeping refuses every exit ceremony,
   with one predicate and one violation name already written and
   tested. A cabin's vertical pair is not optional *because* it is
   vital cargo. (A leaf room declares neither, so there is nothing to
   carry off one — which is the port law paying for itself here.)
4. **Authoring an unhostable ship.** A player may re-site doors until
   the horizontal frontier is useless. Allowed — space is not owed to
   you — because a cabin still lands on the vertical frontier, which
   clause 3 keeps intact, and it lands with four doors on it.
5. **Re-arrangement is an attach.** A door move changes a room's port
   set, so it re-runs the full validation against every neighbouring
   room: new coincidences mate (this is how a crew hand-builds a ring),
   new partial overlaps refuse, newly-blank walls seal their neighbours'
   doors shut.
6. **Cost of the search.** With movable doors the spawn walk considers
   more candidates, and validation is box-vs-box across all rooms.
   Both are trivial at this scale: rooms are capped (`MAX_ROOMS`, on the
   order of the cabin plus burner plus six crew plus two callers), and
   an O(rooms²) integer intersection test at that size does not
   register.

## Build order

Each stage lands green or not at all, in the project's usual way.

1. **Sim.** Rooms, ports, the lattice, the attach/detach contract, the
   room-qualified `Loc`, tile classes, the new barter flow over the
   surviving economy, the gangway gates. Barter machinery, `Sealed`, and
   `Aisle` come out in the same slice — a half-removed interface is
   worse than either end state. Save and tape bumps, migration, and the
   property tests (no overlap ever, closure exact, the cabin frontier
   never starves, conservation across seams).
2. **Cabin presentation.** The widened cabin, rooms and their seams
   drawn, apertures, colored tiles, carry across doorways, the standing
   and upright rules re-swept over the new charts. The x-ray and ghost
   rules are unchanged and should stay unchanged.
3. **Per-POI agents.** Look and behavior per POI, from the lore that
   already exists. The **seam for the look half landed**: a station's
   character is one `const CHARACTER` in one file under
   `crates/cabin/src/poi/`, keyed by a `Host` a room *derives* rather
   than stores (kind plus `ShipState` — the same law the save's own
   reconstruction of a trade runs on), with a `NEUTRAL` default that is
   the room this document already described. A character may change
   look, light, form and flavor and may not touch the grid, the tile
   classes, the ports, or the box; the contract is the doc comment at
   `poi/mod.rs` and a section of ART_DIRECTION_3D.md, and the Guild is
   the worked example. The **behavior** half is still ahead, and it is
   the interesting one: Venus unimaginably tacky, Earth rationing, Mars
   scrappy, the Guild seizing rather than paying, Saturn trading salvage
   like treasure — and, since the window became a family, cutting the
   bay pane out of that ring, because somebody else's hull is the only
   place four flawless cells of glass were coming from — the Umbra
   Market paying a premium for rat-gnawed goods, pricing light at zero,
   and fencing seized portholes beside the seized lamps for the same
   reason, the Hermitage remembering gifts forever and showing one lit
   window, the comet's free ice, and whatever ??? is doing with three
   crates and its arithmetic.

## Non-goals

Explicitly out of scope for this work, so nobody builds them by
accident:

- **No multiplayer transport.** The topology and the interface are
  decided here; no socket, no remote pointer, no crew room ships in
  this work. NETWORKING.md's protocol is the one it will speak.
- **No chained-room content requirements.** Nothing in the game may
  *require* a room reachable only through another room. Chaining is
  permitted by the topology and depended on by nothing.
- **No free placement and no physics.** Cargo stays discrete berths
  driven by input frames (ART_DIRECTION_3D.md's cargo law); rooms stay
  integer boxes on the lattice.
- **No economy redesign.** The values, wants, and jitter are the ones
  already shipped. The interface changed; the arithmetic did not.
- **No room interiors beyond grids and tile classes.** A room is a net
  with colored cells and cargo in it. Anything more is per-POI work.

## Open questions for the implementation

- **Aperture size.** The working shape is the burner doorway's: a door
  is 2 cells wide × 2 tall, and the ladder and hatch punch the same
  footprint in the ceiling and floor. Four floor cells and four ceiling
  cells is a real tax, and the port law halved the bill by handing it to
  the one kind that can afford it: only the cabin's 8×7 floor pays it,
  and a 3×3 pump bay now spends nothing on openings it never used. The
  number is still tuning, not law — the law is only that mating
  apertures be identical.
- **Whether `RoomId` reuse is safe for the tape.** The spec says rooms
  carry no serial identity and ids are dense and reused. If a replay
  turns out to want stable ids for legibility, that is a save-format
  question, not a law.
- **How a room's stock is generated per visit** once the shelf's 2–4
  roll no longer exists — the same roll on the room's `Stock` tiles is
  the obvious first answer, and its slot count is now the room's, not a
  fixed four.
- **What the handshake looks like in the core slice**, before per-POI
  agents differentiate it. One shared fixture is enough; it needs a form
  and a sound.
- **Whether the burner keeps a banking ceremony.** This file retires
  `Cue::Jettison` and lets fuel ride; if playtest misses the red strobe,
  that is a small reversal.
