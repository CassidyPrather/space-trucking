# Art Direction — the 3D cabin

The 2D prototype's art doc ([ART_DIRECTION.md](ART_DIRECTION.md)) promised
that the 3D pass would inherit its *decisions*, not its pixels. This file is
that inheritance, for `crates/cabin`. Same contract as before: everything
rendered should be traceable to a rule here, and anything wanting a
different rule amends this file in the same change.

## The conceit

**You are aboard. The console became a room.**

The 2D console's regions began as physical stations bolted into a
cramped freighter cabin — tank on the left wall, console face
front-right, barter counter low-right — and then, one at a time, they
stopped being bolted to anything. **The hull owns no panels now.** The
readings and the pulls are cargo you hang where you like (BAY.md,
"Instruments are cargo"), the counter left with the barter interface
(ROOMS.md), and the console face's last tenants — pause, warp, mute,
the hangar tally — were never in the room to begin with: they are the
`Esc` menu, an overlay that admits it is one. What the ship itself owns
is architecture and the **walkable cargo bay**, where the room net
unfolds onto wall and deck at furniture scale (see [BAY.md](BAY.md)).
DESIGN.md calls the first 3D pass "an enclosed box with flavor"; the
flavor is ribs, pipes, and hull mass, not detail assets.

Two camera postures, and the camera **never trails the cursor** — every
move is deliberate, nothing to get seasick over:

- **Roaming**: conventional first person. Pointer locked, mouse looks,
  WASD walks a clamped envelope, a small glint crosshair marks aim. Aim
  at an instrument and its own rig invites with a glint outline — the
  tell belongs to the piece, because the station does.
- **Focused**: click (or `E`) glides the camera (~0.4 s, eased, no
  overshoot) to that station's viewpoint, wherever the cargo carrying it
  hangs; the cursor frees and precise sim interaction works exactly as
  in 2D. `Esc`, right-click, or `E` steps back out.
- **The menu**: `Esc` while roaming raises the meta-controls — pause,
  fast-forward (dev), mute, the delivery tally — as a screen overlay,
  and frees the cursor exactly as `Esc` always did. It is deliberately
  *not* diegetic: those four are things you do to the game, not things
  aboard the ship, and the previous arrangement had the cabin claiming
  to contain its own volume knob. Zero text like everything else: the
  icons are stamped from bit rows into colored nodes, palette roles
  only, with a lamp under each button and a slash that carries mute by
  shape. The sim keeps ticking behind it — the only pause in the game is
  the sim's own, folded in through the same `InputFrame` toggles the
  keys throw.

Focus viewpoints are *fitted*, not eyeballed: each is derived from its
surface group's extents and the camera FOV, so the station fills the
view and moving the cargo moves the viewpoint with it. Stations only
need to look composed from their focus pose and presentable in passing —
the focused view is the contract, the roam is atmosphere.

The sim is the same one the 2D console ran — same save string, same
flight-recorder tape; the console itself retired once the bay work
began (the decision is recorded in [BAY.md](BAY.md)). Every interactive
surface is a `SimSurface`: an oriented quad bound to a rect of the sim's
800×600 logical world. The cursor ray — or, in the bay, the roaming
crosshair ray — maps through the surface into sim coordinates, so the
sim keeps making every ruling, and hit-tests can never disagree with
the rules.

## Two material families, again

| Family | Elements | Treatment |
| --- | --- | --- |
| **Worn metal** | walls, hull slabs, wells, levers, handles, rig frames | lit `StandardMaterial`, high roughness, palette roles; brass gets real metalness |
| **Phosphor & lamps** | tank readings, route lines, lamps, crate glow, dial fill | emissive materials on near-black bodies; bloom supplies the halo |

The rule of thumb: if the 2D console drew it on a CRT or as a lamp, it is
emissive here; if it drew it as metal, it is a lit surface. Lamps are
still glass first — a dark `GLASS` body whose emissive wakes, never a
recolored box.

**The screens are the 2D renderer, kept.** After trying a diegetic
shadow-box orrery, the map and destination preview went back to the 2D
prototype's rendering discipline on purpose: `crt.rs` is a software
rasterizer — the 2D `Canvas` reborn — that repaints both screens into
small emissive textures each frame at the 2D's own pixel density
(2 world units per texel), porting `draw_map`/`draw_preview` semantics
whole: every glyph identity and accent, the sweep and its afterglow,
scanlines, vignette, the omen tint on every stroke. Surfaces stay
consciously material: the tube shows a *rendering*, and the room shows
the tube. The canvas paints headless, so screen content is unit-testable
without a GPU.

## Pixel crunch, third dimension edition

The whole cabin renders into a **480×270 target upscaled
nearest-neighbour** to the window (`CRUNCH_W/H` in `rig.rs` — one knob,
same as ever). This is the design doc's "textures with smoothing off"
applied to the world itself: geometry aliases into hard pixels, emissives
bloom into chunky halos, and the low target resolution does the work that
hand-pixeled sprites did in 2D. The version string renders outside the
crunch at native resolution: dev information, not fiction.

Geometry is **low-poly primitives built in code** — cuboids, cylinders,
spheres, cones, hand-rolled meshes when a silhouette needs it. No asset
files, nothing to credit, nothing to download; the 85MB budget stays
untouched at three orders of magnitude of headroom.

## Palette

All color lives in `crates/cabin/src/palette.rs`, and the roles and hex
values are the 2D palette's, verbatim — retuning still happens in one
place, and a purity test still fails the build on raw color constructors
anywhere else in the crate. (Rooms the 2D console never had may add
roles there, documented at the constant — the burner's `EMBER` firebox
glow was the first, and its `SOOT` deck the second.) Cargo keeps its identity hues; POIs keep their
enamel identities. What changed is interpretation: a role is
now either a surface color or an emissive color, and *light itself is a
palette instrument*:

- The cabin is lit warm (`GLINT` overhead, `PLATE_LIT` fill) with a low
  green spill by the tank (`PHOSPHOR`), so the metal reads green-gray as
  the 2D plates did.
- **No runtime shadow maps, on purpose.** DESIGN.md's lighting direction
  is light *volumes* — authored, placed light, not simulated occlusion.
  Depth comes from placement, wear, ambient fill, and atmosphere; a
  shadow map that sneaks in is a default to be removed, not a feature.
- **The air is visible.** A gentle `HULL`-toned distance fog softens the
  far corners, and deterministic dust motes drift where the light pools
  — denser under the lamps, violet-tinged when the omen swells,
  dimming with the room. Particulate is decoration: slow, seeded,
  the same air every boot.
- **The omen dims the actual lights.** `sim.light()` scales every
  `Dimmable` source; a violet `EERIE` source swells with `sim.omen()`;
  the jump is a sub-half-second `EERIE_BRIGHT` flash. Screens and lamps
  keep glowing in the dark — phosphor doesn't care about the room, which
  is exactly why a dimmed cabin feels wrong in the right way.
- **A light is sized to the room that owns it.** There are no shadow
  maps, so a lamp's `range` is the only wall it has: a source whose
  reach exceeds its own room lights the neighbours *through the hull*,
  and the lights-out economy (BAY.md — lamps are cargo, darkness is a
  legal state) is then paid for by whoever happens to be docked. Two
  sources are not the crew's own cargo, and both are held to it: a
  **calling room's pendant** reaches its own far floor corner and a step
  and dies before the middle of any riding room, and the **burner's
  fire** — which is cargo, burning — reaches its own chamber and stops
  at the doorway wall. Both carry `Dimmable` at the brightness they
  actually burn, so neither can quietly opt out of the omen.

## Colored tiles: form, not stacking

The room grid's tile classes (ROOMS.md) are a signal system, and the
rule that keeps them legible is the 2D "no hue alone" law with a second
edge: **no pattern on pattern either.** A class is told by the *kind* of
mark it wears — solid field, struck line, edge banding, sparse studs —
at its own density and with its own edge treatment; **stripes belong to
exactly one class** (hazard tape, `Consume`), because a striped mark on
a striped ground on a striped deck is what the playtest called stripe
soup; and a mark is drawn on a **region's rim**, never stamped into
every cell, because per-cell stamping turns a painted bay into bathroom
tiling. Each class must read against a bare deck and against a coated
one, which is why the two trading classes are *filled against hollow*
rather than two hues of the same fill.

Anything flat drawn over a chart rides a **named rung** of the decal
ladder (`rig::layer`), and one reading gets one rung: field, mark, and
tread are three. A new reading adds a rung and a row in the ladder test,
which fails the build; a new reading sharing a rung is a shimmer at
every tile boundary, which fails only the eye.

## Motion

The 2D taxonomy holds: **feedback** (answers a sim event; finishes inside
half a second; always runs), **decoration** (idle loops — lamp shimmer,
crate breathing, sweep), **instruction** (the tutor ghost, unported so
far). The cabin is native-only for now and native builds never carried the
reduced-motion flag, so decoration simply runs; if the cabin ever reaches
a browser, decoration must gate on the flag exactly as the 2D console
does, freezing to legible midpoints. Keep the split honest in code —
decoration phases derive from elapsed time, feedback from cues — so that
gate stays cheap to add.

No signal rides on hue alone, same as ever: refusals carry a slash or a
shake, lamps carry lit-versus-dark-glass, rows sit at fixed stations.

## Do / Don't

- **Do** bind every interactive quad to its `layout` rect via
  `SimSurface`; never restate a hit-test in 3D terms. A rig that stands
  off its chart binds its own body (BAY.md, "The standing rule") — the
  aim must meet cargo where the cargo is.
- **Do** draw new furniture from `Skin` materials + palette roles; new
  glows through `glow::phosphor`/`glow::set_lamp`.
- **Do** give any dynamic brightness its own material instance — shared
  handles light every lamp on the ship at once.
- **Don't** introduce text (version corner excepted), asset files,
  pure black or pure white, or physics on cargo.
- **Don't** animate past the juice conventions; the cabin must be
  glanceable-away-from for minutes at a time.
- **Don't** let the GPU budget creep: the target is integrated graphics
  at 60fps into a 480×270 buffer. Prefer emissives to lights; keep
  shadow-casting lights to one.

## Keeping geometry honest

The class of defect where a mass swallows a face's edge, or a viewpoint
clips a wall, is handled structurally, not by eyeballing:

- Structural masses are **data before entities** (`rig::structure()`),
  and anything that must relate to a surface — the focus viewpoints, a
  rig's own face — is **derived from that surface's corners and
  extents**, never authored twice. Nothing is measured off a panel any
  more, because there are no panels.
- Unit tests walk the invariants: every focus pose must be a legal
  camera position facing its station, the roaming envelope must be clear
  of every slab, and **every cell of the net must be workable** — that
  last one used to forgive cells a station panel stood in front of, and
  the exemption retired with the panels. Break the layout and the build
  breaks.
- **Sightlines are tested, not eyeballed**: from each station's focus
  viewpoint, every corner of its face and every interactive control
  (levers, grid cells) must sit inside the camera frustum with an
  unoccluded line from the eye — checked by frustum math plus occlusion
  rays against every slab. Hull is the only thing that can be in the
  way, and a control a refit pushes off-screen or behind it fails the
  build with the blocking geometry named.
- The cabin has a **screenshot mode** (`--shot out.png`, optionally
  `--view tank|lever|bay|front|starboard` or a room's name, and `--menu`
  to raise the `Esc` menu for the shot) that renders, saves one capture,
  and exits. It runs headless under xvfb with llvmpipe, so visual review
  happens in CI-shaped environments too — geometry changes should come
  with fresh captures, looked at.

## The cargo question

The owner's playtest note: drag-and-drop cargo between little slots is a
2D prototype artifact, not the design — 3D space wants cargo to be
*significant*, and the system must plan for extensibility (crews,
networking). The architectural answer, so the experiment can run free:

**The sim's discrete cargo model is the network model, and it is already
right.** A piece is `(id, kind, variant, gnawed, Loc)` where `Loc` is a
discrete berth — hold cell, pad slot, rail slot. Lockstep multiplayer
ships only `InputFrame`s; cargo state never travels, and the drag-monkey
tests prove no interleaving of six players' inputs can lose a piece.
Every future presentation must keep that spine: **presentation may be as
physical as it likes, but what the sim sees must remain discrete berth
transitions driven by input frames.** DESIGN.md's "cargo must not have
physics" is this same rule seen from the other side.

What that frees us to do in 3D, next experiments in rough order:

1. **Bigger, heavier presentation**: crates at furniture scale in a
   walkable bay, not desk trinkets; a berth is a floor plate or wall
   bracket, still exactly one sim cell.
2. **Carry instead of drag**: grabbing a crate parks it "in hand" (the
   sim already models a held piece per player); walking to a berth and
   clicking places it. The bridge synthesizes the same press/held/release
   frames it does today — the gesture layer already shows how.
3. **Placement rules become physical staging**: heavy-rides-low means
   floor plates versus high shelves; cryo-hugs-the-hull means berths on
   the outer wall; violatile adjacency reads as spacing between plates.
   The rules stay in `cargo.rs`; the room *is* the diagram.
4. **Crew ownership** maps to lockstep player indices unchanged — two
   players carrying crates in 3D is exactly two `held` slots the sim
   already simulates.

What must NOT happen: cargo positions as free 3D coordinates in sim
state, physics on pieces, or any frontend-authoritative cargo movement.
That would break saves, tapes, lockstep, and the conservation tests in
one stroke.

The owner's actuating idea for the experiment, queued behind the
smaller polish: new cargo kinds that *live in the room* — lamps (floor-,
wall-, or ceiling-affixed), couches, paintings — plus mechanics that let
cargo interact with light, atmosphere, and other cargo, tying the 3D
space together. The fixture slice ([FIXTURES.md](FIXTURES.md)) landed
that idea; the walkable bay ([BAY.md](BAY.md)) is the next slice —
carry-style interaction at furniture scale, and the cabinet as the
first berth-providing piece. With the 2D console retired, the "2D
analogue" law became the logical-space law: every mechanic must remain
expressible and testable through `InputFrame`s against `layout` rects.

## Open questions for the aesthetic experiment

Answered by iteration so far:

- **Wear returned as procedural multiplier textures** (`wear.rs`):
  splitmix-seeded 96×96 tiles — value-noise blotches, fourteen
  scratches, scuffs, edge grime — multiplying the palette tint through
  `base_color_texture`, never darker than ~3% net. The hull, plates,
  and deck each get their own character; hazard striping paints the
  bay's front lip. No unweathered surfaces, no asset files.
- **The window earned the front wall** (`viewport.rs`): a painted
  canvas, not a CRT — no scanlines, no phosphor; stars are glint-white.
  Streaks stretch under warp, the destination grows on approach, the
  berth hangs outside while docked, and the whale swims past when it
  calls. The distinction between a *screen showing a reading* and a
  *window showing space* is the material discipline, kept.

- **The hold left the desk** ([BAY.md](BAY.md)): the grid unfolds onto
  the cabin's walls and deck at furniture scale and carry replaces drag.
  The counter that kept desk scale as the broker's diorama is gone —
  stations are rooms you carry cargo into ([ROOMS.md](ROOMS.md)) — and
  with the console face following it off the wall, the fixed UI is done:
  every reading aboard rides a piece, and the four controls that were
  never readings live in the `Esc` menu.

Deliberately unsettled, still:

- Whether the crunch target should breathe with window size or stay
  fixed 16:9 letterboxed.
- How far bloom can carry the CRT reading before it goes syrupy.
- How much the `Esc` menu should eventually hold. It is four controls
  and a tally today, deliberately: a menu is the easiest place in a
  wordless game to start explaining things, and it must not.
