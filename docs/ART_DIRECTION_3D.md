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

## The window is a hole

**The transit window shows a real outside, not a picture of one.**
`viewport.rs` used to paint the void into a texture: a hashed starfield,
a berth disc, a growing destination, streak dashes. It was good ink and
it was a lie — it looked identical from every angle, which is the one
thing a window cannot do.

What is there now:

- **The void is a place.** Stars, the world ahead, the berth alongside,
  whatever calls on the leg, and the ship's own hull all stand in world
  coordinates on their own render layer (`viewport::VOID_LAYER`). The
  cabin camera never sees that layer, so the hull stays solid from
  inside and no light out there leaks through a wall.
- **The pane is an aperture, and an aperture has a frustum.** A camera
  sits at the player's eye and looks out through the glass with an
  **off-axis projection** whose near plane is cut on the window itself
  (Kooima's generalized perspective). Move your head and the
  frustum shears; the same pane frames different space. Stand to one
  side of it and you see the space on the other side, which is what
  holes do and what pictures do not. Nothing about it is a parallax
  *effect*: the projection is simply correct.
- **It is cargo, so the void follows it.** The aperture is derived from
  the glass quad's live world pose — the piece rig's, wherever the crew
  rehung it. A window on the front wall looks forward; carried to the
  port wall it looks to port. No window aboard: no aperture, no camera,
  no view, and the hull is solid. Several windows aboard is the ordinary
  case, and "One wall, one sky" below is how it is paid for.
- **It obeys the crunch.** The porthole draws into its own small
  nearest-sampled target, cut to the glass at a fixed texel density
  (`PANE_DENSITY`) so the void reads about as chunky as the room around
  it. One knob, same discipline as `CRUNCH_W/H`.

### One wall, one sky

Windows are cargo, so a crew may own several — and the first exterior
pass gave each one its own camera and its own render target, which is a
bill that grows with the shopping. It does not any more.

**Panes bolted to the same plane share one render of the outside.** The
sky is drawn once through the rectangle that bounds all the glass on
that wall, and each pane reads its own rectangle back out of it (a
`uv_transform` on its material). This is exact, not an approximation:
two co-planar apertures seen from one eye have the same near plane and
the same view axis, so their projections differ by an affine map of that
plane and the larger render *contains* the smaller one texel for texel.
`viewport::sub_uv` is that map written out, and
`a_shared_sky_is_the_pane_s_own_sky` checks it against each pane's own
independently-built projection rather than against itself.

A "sky" is one pass over the whole outside — six thousand star quads,
every attached room's shell, the stream, the dock, and whatever is
calling this leg. That is the cost that was multiplying. Windows still
cost draw calls in the CABIN pass, like any other furniture, and
nothing else.

Measured with `--panes n --grouping wall|pane --view bay --gauge 120`:
one board, one camera pose, every pane in frame, and `--grouping pane`
as the **control arm** — the first pass's cost model, reached through
the same code. Absolute milliseconds are meaningless here (the
container renders in software, on llvmpipe); the shape is the reading.
Subtracting the `--panes 0` frame (72.9 ms, no exterior at all) leaves
what the outside actually costs:

| panes on one wall | 1 | 2 | 4 | 8 |
| --- | --- | --- | --- | --- |
| **one sky per wall** (ships) | 1 sky, 41 ms | 1 sky, 49 | 1 sky, 53 | 1 sky, **26** |
| **one sky per pane** (control) | 1 sky, 37 ms | 2 skies, 77 | 4 skies, 159 | 8 skies, **309** |

The control doubles with every doubling of the glass, which is the bill
this replaced. The shipped law does not trend at all — and it comes
*down* at eight, which is the `PANE_MAX` clamp visible in the data
rather than in an argument: a wall's worth of glass cuts one wide sky
at the texel ceiling, and a wide aperture on a fixed texel budget shades
less than a narrow one filled edge to edge by a neighbour's hull plate.
Coarser, not costlier, exactly as claimed.

Three bounds keep it honest, and all three are the same idea — *a
window may be free, but glass is never unbounded*:

1. **`viewport::MAX_SKIES`** is a hard ceiling on skies per frame, with
   the cameras and targets allocated once at boot and reused. It counts
   *walls with visible glass*, not windows, so eight is extravagant. Past
   it, the remaining panes go **dark**: honest black glass, never a
   stale sky and never somebody else's.
2. **A pane the eye cannot see costs nothing.** Panes behind the crew,
   or on the far side of their own glass, are dropped before any sky is
   planned — which is why the ceiling can be generous.
3. **`PANE_MAX` is degradation, not a budget.** A wall with glass spread
   across it cuts one wider sky at the same texel ceiling, so it crunches
   *coarser* rather than costing more. That clamp is the same one a single
   oversized pane always met; sharing did not loosen it.

The rehang rule is untouched by all of this, because the gathering is by
**plane** and a plane is a fact about the wall: move a window to another
wall and it joins that wall's sky, aimed that way, showing what is out
there. `two_walls_are_two_skies_pointed_two_ways` is the assertion.

#### Why not a stencil portal

The obvious answer to "N windows" is the classic one: mask each pane
into the stencil buffer, render the exterior **once** with the stencil
test, oblique-clip the near plane on the portal so interior geometry
cannot intrude. It was the leading candidate and it was not taken, for
two reasons in that order:

1. **It does not actually buy the thing.** A portal's projection is the
   off-axis frustum *through that pane*. Two panes on different walls
   want different projections, and one draw call has one projection —
   so a stencil portal still re-transforms the exterior once per pane.
   The stencil saves render *targets*, which were never the expensive
   part; the geometry was. Plane-grouping saves the passes,
   which is the part that scales. Where the two overlap — several panes
   on one wall — grouping already collapses them to one pass, and a
   stencil would collapse the same set to the same one.
2. **It is the most upgrade-fragile code we could write.** Bevy 0.19's
   standard PBR pipeline exposes no stencil ops (`StencilState`
   defaults off, no per-material hook), so this means a specialized
   pipeline and a render-graph node: engine-internal API, the surface
   that moves most between releases, and it fails *silently* — a wrong
   mask is a picture, not a compile error.

What shipped uses **only component-level API** — `Camera`,
`Projection::Custom`, `RenderTarget::Image`, `Frustum`,
`StandardMaterial::uv_transform` — and no render graph, no custom
pipeline, no shader. That is the isolation story: everything fragile is
a type the compiler checks, so a Bevy upgrade breaks `viewport.rs` **at
build time with a name in the error**, rather than shipping a window
that quietly shows the wrong sky. The two seams worth naming if the
engine does move are `Aperture`'s `CameraProjection` impl (Kooima's
generalized perspective in the pipeline's reverse-Z convention) and
`sub_uv` (the affine remap); both are pure functions with tests that
check them against each other rather than against a golden image.
- **The material distinction survives, inverted.** The old rule was "a
  tube shows a rendering, a window shows space", kept by painting the
  window differently from the CRTs. It is kept by *construction* now:
  the CRTs still wear `crt.rs`'s software raster, and the window wears
  `viewport::pane_glass` — dark glass, real specular, no scanlines and
  no phosphor ramp, whose gleam is the crew's own lamps reflecting
  rather than a highlight drawn into a canvas.

The sim remains the authority for everything out there: `ShipState`
decides which world grows off the bow and which shrinks astern,
`is_warp`/`stoked` set how hard the near field streams past,
`encounter`/`parade`/`advertising` bring the company, `Cue::Jump` floods
the aperture violet, and `light`/`omen` lean on the void's one light
exactly as they lean on the cabin's lamps.

### Room exteriors, and how a design agent dresses one

Every attached room wears a **shell** grown from the same `room::Plan`
pose its interior is built from (`room::hull_box`), so the trade room
you walked out of is bolted to your hull exactly where you left it. The
shell a room gets by default is deliberately plain — **hull plate, a
seam belt, corner posts, a running light or two** — because an exterior
with character already on it would fight whatever the station is
eventually given.

Two seams exist for the per-station passes, and they are the only two.
Both are now **keyed by station rather than by room kind**, because one
`Trade` kind serves twelve places and twelve identical shells was the
defect the character pass exists to fix:

1. **`poi::Character::outfit`** is the kit: the plate colour, the
   running lights' colour, and how many. A derelict burns none — that
   is its whole tell — and the furnace wears its soot outside as well
   as in. `viewport::outfit(kind, host)` is the lookup; the ship's own
   two rooms keep their kind's kit, everything alongside wears its
   station's.
2. **`poi::Character::dress`** is where hardware goes: a Guild mast, a
   refinery's flare stack, a casino's sign, and the owner's first idea
   for it — a planet-side POI's space-elevator ribbon running off the
   shell toward the world below. It is **data**, not a spawn callback,
   so the containment laws below are arithmetic a test runs rather than
   a habit a reviewer has to notice.

Anything a design agent adds is dressing on a box the lattice placed.
**The dressing may not move the room**, because the room's position is
the sim's and the shell is derived from it; if a station needs to sit
somewhere else, that is a `room::Plan` change, not an art change.

### Station character: one module per place

The full brief is the doc comment at **`crates/cabin/src/poi/mod.rs`**,
which a per-station design agent reads as its entire assignment. The
shape of it, for everyone else:

- **A room learns whose it is by derivation, not by storage.** `poi::of`
  reads the room's kind and `ShipState`: a `Trade` room is alongside
  exactly while the ship is docked, so the docked POI names it. Event
  rooms are one of a kind, so their kind is their identity. Riding rooms
  have no station. Nothing is written to the save, so nothing can
  disagree with it.
- **One file per station.** `poi/guild.rs`, `poi/venus.rs`, … each hold
  one `const CHARACTER`. Unfilled stations return `poi::NEUTRAL`, which
  reproduces the room this document described before the pass — so a
  half-finished fleet ships, and a station nobody has touched cannot
  regress.
- **What a character owns**: tile enamel and treatment per class, the
  handshake's form and its placement inside its declared cell, the
  pendant's colour/burn/shade/cage, interior decor, and the exterior kit
  and hardware.
- **What it cannot own**: the room's grid, its tile *classes*, its port
  declarations, and its box. There is no field on `Character` that names
  a cell, a class, a slot, or a pose — that is structural — and what is
  left is reach, which is asserted: interior fittings stay inside their
  own room, exterior fittings stay outside every room and inside
  `poi::DRESS_REACH`, nothing stands in a doorway, and the pendant's
  *range* is derived from the room rather than picked, so **a station
  still lights its own floor and never a riding room's**.
- **The art laws survive it structurally.** `Tiles` carries colours and
  no forms, so `Offer` stays hollow and `Stock` stays filled at every
  station (no signal on hue alone); `poi::Worn` does not offer the
  hazard material, so stripes stay `Consume`'s alone; every colour is a
  palette role and the purity test walks `poi/` too; geometry is the
  same five code-built primitives everything else uses.
- **Looking at your own work.** `--docked n` re-berths the developer
  fixture at any place on the chart, and `--view berth` stands outside,
  square on a caller's outboard face. Before those, eleven of the twelve
  rooms could not be photographed at all.

One structural rule falls out and is worth stating: a room mated flush
to the wall your window is on has its shell *inside your own hull's
thickness*, so the plates are double-sided and the shell you are
standing in is dropped each frame (`viewport::mind_the_hull`). A
neighbour filling the window with plate is the honest reading of "there
is a room bolted here"; a box drawn around your own eye is a blindfold.

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

### Two voices: the whitebox speaks the palette, a bought mesh speaks its atlas

The plan is two graphical implementations of every object, and the second
one arrives painted. A Synty pack paints a whole pack from one shared
atlas; the mesh names it, the converter carries it, and the material that
reaches the screen was authored by somebody who never read this file.

**The rule, stated so nobody has to guess it later: the palette governs
every material the cabin AUTHORS. A material that arrives inside a
purchased scene is exempt, by declaration, and is exempt because it is
not ours to author.** Repainting a bought mesh into the palette is not a
smaller change than it sounds — it is throwing away the thing that was
bought — and the honest position is that a dressed build looks like the
pack it was dressed from.

`palette_purity` needs no loosening for this, and the reason is a happy
accident of how it was written. It is a **source sweep**, not a material
sweep: it walks `crates/cabin/src/`, subdirectories and all, and fails
the build on `Color::srgb` and its siblings outside `palette.rs`. That is
already exactly "materials the cabin itself authors", in the strongest
available sense — the cabin authors a colour only by writing one down.
`art.rs` writes none: it hands the asset server a path and spawns what
comes back. So the test covers the art module and passes, and it covers
it in a **whitebox** build too, where the module is compiled but its
loading half is not.

Two consequences worth knowing.

**A dressed build is not a graded build.** The two voices will not agree,
and the lighting is the palette's — the cabin's lamps, fog and dust do
not change when a crate does. Judging that is a job for the nudge tool
and for eyes, not for a test.

**The whitebox is still the design.** Every rule in this file is about
geometry the repository cuts, because that is the version continuous
integration builds and the version that has to stand on its own. Art is
presentation over a description that does not change.

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

## The three tells: an outline, drawn round the body's own edge

A highlight is drawn **around a body, never on the ground under it**, and
it is a real outline: the piece is drawn a second time into a mask and a
full-screen pass finds that mask's edge (`crate::outline`). What comes
out follows whatever geometry was actually there, which is the whole
reason for the technique — the day a purchased mesh replaces a
hand-rolled `Cuboid`, the outline is re-cut with it and nothing in the
tell layer is told.

It was a **box** before, and the owner said so twice: twelve emissive
bars round `pieces::drawn_box`, cut into brackets, a ring or dashes. A
box round a body is not an outline of a body, and on anything that is not
a crate the difference is the whole of what you see.

Three readings can be worn at once — the crosshair is on this, the room
has claimed it, you have asked for it — so each is a different **form**
of the one line, measured in crunch texels off the silhouette:

- **the aim** — a thin line on the body's own rim, painted just *inside*
  it. The lightest reading, and the one that comes and goes with where
  you happen to be looking, sits ON the thing;
- **the mark** — a **broken** line hugging the outside. A stub reads as a
  mark *on* a thing, which is the room noting your interest;
- **the offer** — a **thick whole** line standing off outside both, with
  clear air between it and the body. The strongest claim gets the
  heaviest and most complete form, and a band round a thing is not a
  remark about it.

Hue does none of that work, the same as everywhere else — a cabin whose
lamps have been sold reads in one colour anyway. What hue does carry is
*which* aim: the carry's ruling is green or red and the crosshair's is
pale, on top of a shape channel that says the same thing (the refusal
slash, `pieces::carry_slash`).

**Two things the bars could not do, and this does.** An outline of the
mask is cut by whatever stands in front of the piece, so a good half
behind a counter is outlined round the half you can see and closed along
the counter's own edge — which is what a partly hidden thing looks like
in Blender and in Unity, and what a wireframe box gets wrong twice over.
And a rig ghosted by the focus x-ray keeps its outline with its body
gone, which is the entire reading there: something stands here, you are
flying through it.

**Where the mask lives is the cost decision.** It rides in the alpha
channel of the crunch target, written by proxy meshes drawn in the cabin
camera's own pass — so occlusion is the depth buffer's answer and there
is no second scene pass, no second clear, and no third camera. Every
opaque surface in this engine writes `a = 1` and every blend mode the
cabin uses leaves a destination alpha of 1 alone; bloom's composite and
the tonemapper hand it through untouched. So the scene arrives with alpha
1 everywhere and anything below 1 is the outline layer's — one small
number carrying which of five things the aim is doing, plus the room's
two claims.

The numbers, measured with `--gauge` at 480×270 on this container's
software rasteriser, where absolute times mean nothing and the deltas
mean everything:

| | mean of 3, ms/frame | spread | delta |
| --- | --- | --- | --- |
| no composite pass at all | 134.95 | 0.8 | — |
| the pass, drawing nothing | 141.18 | 2.3 | +6.2 |
| the pass, one tap per texel | 145.76 | 5.7 | +10.8 |
| the pass, the 29-tap disc it ships with | 152.81 | 8.0 | **+17.9** |

**The mask itself does not show up.** The same scene with the copies cut
for every part read 152.97 and with them cut only on demand 152.81,
which is well inside the spread of either — it is a handful of extra
draws inside a pass that was already running, and only on the frames
something is said about. It costs nothing at all until then: a part is
only MARKED as maskable when its rig is built, and the copy is cut the
first time the outline pass has something to say about that piece, so a
cabin nobody is pointing at carries no mask and no extra body.

What the table prices is the full-screen edge detect, and llvmpipe is
doing 3.6 M texel fetches a frame in software to produce it. The same
work is a fraction of a millisecond on the integrated graphics this is
aimed at, and if it ever bites, the pass can be cut down to the screen
rects of the outlined pieces instead of the whole picture: the pass with
no kernel at all is +10.8 and with this one +17.9, so the kernel is the
half that scales with area and the area is the knob.

**What was measured and not built.** An inverted hull per part is +5.4
draw calls mean and +20 worst with no batching, and it outlines an
assembly's internal seams — the couch's cushions get haloes where they
meet the frame. An inverted hull on the whole rig's box draws a solid
card, because a rig does not fill its own box. A stencil pass was priced
and rejected earlier in this project's life. None of those three follows
a purchased mesh; this one cannot do anything else.

## What you can click is what lights up

The tell wraps a body's three extents. For a long time what *answered*
used two: a rig standing off its chart carried a flat quad cut on its
silhouette, bound to the sub-rect of its own cells that silhouette
covers, and the aim was read where the ray crossed that plane.

A plane and a body are the same shape from exactly one direction. Walk a
quarter turn and they stop being: a column of brine pearls seen from the
side is a quad edge-on, so an aim at the top of it goes straight past and
lands on whatever deck cell is beyond. Measured on a ring of thirty-six
stances at three heights round a berthed body, before the cure — an aim
at the **top** of the body answered from:

| | of 36 stances |
| --- | --- |
| brine pearls | 18 |
| floor lamp | 18 |
| wardrobe | 26 |
| suspicious crate | 13 |

and in every case those were **two arcs square on to the body's front and
back**, with nothing at all in between. Only the bottom band answered all
the way round, and that was never the piece answering; it was the deck
cell under it, which is why a **tall thin** kind is the one that got
reported and a crate did not.

So a rig's face carries its body now (`SimSurface::deep`) and the aim
meets the box the tell draws (`SimSurface::strike`). The quad stays the
*reading* — wherever on the body the aim lands is laid back onto the
elevation the rig was drawn in, so a cabinet met on its flank at the
height of the third cubby reads the third cubby. After the cure every
one of those kinds answers from all 36 stances at every height, and
`pieces::tests::the_body_answers_from_all_round` sweeps 1,346 berths and
111,617 aims to keep it that way.

**Level wall cargo still reads through its chart, and that is a measured
trade rather than an omission.** There the chart lies in the rig's very
plane and answers for the rig's very cells, so the reading is right for
anything but a glancing aim at a deep body — a sconce reaching half a
cell off its wall lands a fraction of a cell over on 3 or 4 of the 15
stances the room allows in the same sweep, all of them at the grazing
ends of the arc. Curing it costs a seam: a wall berth's cells are where
a doorway's amber latch is bolted too, the latch stands two millimetres
proud of the plane, and a pane hung on the aft wall beside a cabin's own
doorway stands **across** that latch. Give that pane a pick body and it
outranks the latch from every stance in the room — with one, the input
monkey could not part a seam in 19,200 frames. The glancing read is
worth less than the seam.

The showcase used to supply that pane without meaning to: its bay window
rode the market, and the one berth the arbiter will bring a 2×2 home to
is that very cell. It berths aboard now and the porthole goes ashore in
its place (`fixture::tests::the_showcase_leaves_every_seam_latch_workable`).
That fixes a bad debug board and changes nothing above — the berth is
still legal, still a berth a player may take, and a crate a player puts
there is still their own business.

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
  shadow-casting lights to one. There is exactly one full-screen pass
  aboard (`crate::outline`) and its cost is written down with numbers; a
  second one needs the same measurement before it lands.

## The gauntlet: what a screenshot structurally cannot see

A human playtest of the station wave found some fifteen defects that
fifteen design agents and their screenshots had all signed off on, and
the post-mortem blamed the *shape* of the checking rather than anybody's
care: a still cannot see time, agents shot the framing they designed for
rather than the path a player walks, rooms were photographed empty, and
designers graded their own work. `crates/cabin/src/gauntlet.rs` is the
adversarial pass that closes those, and it splits in two:

- **Pure geometry, always on.** `cargo test -p cabin` sweeps every room
  in the game — the twelve stations, the three event rooms, the cabin
  and the burner — against a **loaded** board (`gauntlet::load` fills
  every legal berth through the sim's own arbiter, so decor has cargo to
  clip through), every cargo kind, and every doorway. Sixteen rule
  families: no fitting stands in a berth cargo may take at the height a
  rig occupies; nothing occludes a wall berth from its room; every berth
  stays workable from the walk envelope; no two drawn faces share a plane
  and a facing (the general z-fight detector); a rig's named features
  point where their names say (`pieces::features` — a sconce's cup INTO
  the room, a floor lamp's base plate FLAT on the deck); a rig draws
  inside the cells the sim gave it, so its pick face can be cut from its
  picture rather than its plan; the walked path stands in air; every body
  lands on the cargo grid; a part that names a seat meets it; a rig
  reaches the chart it is berthed on; a hung body says what holds it up;
  a rig fills the cells its berth spends rather than merely staying
  inside them; a rig stands up and shows the room its face; every cell of
  deck a body may set cargo on is walkable to from the door it comes in
  by; and a room's own worked hardware — the counter, the latch — can be
  reached and is not stood across.
- **Pixels, opt-in.** `--gauntlet-walk <dir>` drives the scripted room
  walk — in through the door, round the room, up to the counter — and
  writes one PNG per waypoint, then holds the camera still for ten
  frames and compares them (the flicker detector: the only mechanism
  that would ever have caught the every-other-frame lamp), then backs
  off along an approach sampling the room's own brightness (the
  light-pop detector). It needs a rasteriser, so it runs under `xvfb`.

**It measures descriptions, not the world**, and that is the first rule
this file cares about: anything drawn from something other than a pure
description of it — `poi::character_of`, `room::shell_boxes`,
`pieces::parts`, `room::seam_parts` — is invisible to every rule in the
sweep, however loud it is on screen. Twice a whole layer of the art was
built straight into the world and went unchecked for that reason: the
cargo rigs, and then the hardware in every doorway. **A new family of
thing gets a description before it gets a mesh.**

The second rule was learned the harder way, after four defects the owner
found by eye in bodies that had been described all along: a wall lamp's
mount pad, a station's beacon, a porthole, and a doorway's latch, each
hanging in the air off the surface it is bolted to. **A description says
where a body is; it takes a declaration to say what a body means.** A
fitting's `at` is a position, and a position is not a promise — nothing
could ask whether the beacon reached its wall until the beacon said it
had one. So the sweep asks bodies to declare: `Part::pointing` for the
way a part faces, `Part::seated` for the joint it makes, `Fitting::seated`
and `SeamPart`'s own seat for what holds a piece of furniture up. Where a
body genuinely hangs on nothing — the Wanderer's collar, its hum rings —
it declares nothing, and that stays legal. **When a rule cannot be
written, ask what the description leaves unsaid, not only what it fails
to enumerate.**

The third rule came off five hoops that five different stations had each
declared "set into the deck" and each drew hovering over it. Nobody had
mis-declared anything: a `Fitting`'s `half` is the box the unit body is
scaled into, and four of the five silhouettes fill that box exactly
while a torus's tube fills 18% of it. The declaration meant one thing
for a slab and another for a ring, and every author of a ring had been
quietly wrong about which. **A vocabulary term that means one thing for
most of its bodies and another for one of them will be read the common
way by everybody, so the odd body is what needs saying out loud** —
`poi::Shape::fill`, one statement of the fraction, read by the
containment law and by the sweep alike. It is worth looking for: the
suspicious shape is the one whose mesh does not fill its own box.

The fourth rule came off a crate the owner reported four times and the
harness passed every time. Eleven families all asked the same *kind* of
question — is the body inside its plan, inside its band, inside the
room, touching its chart — and every one of them is satisfied by a body
shoved hard against one edge of the ground it was given. On a deck that
is exactly what happened: the depth band is measured off the berth
plane, a wall has one and a deck does not, and every standing rig in the
game was composed half a cell out into the aisle on the one axis nobody
had a rule for. **A rule that only ever asks whether a body is inside
something cannot see a body that is inside it and in the wrong place, so
where a claim fixes two axes, ask what fixes the third.** `berth-filled`
is that question asked of the two axes a berth's rect pays for.

The fifth rule is about the other four. Each of them was learned from a
defect somebody found by playing, written down afterwards, and each one
is a point rather than a method: knowing that a defect once hid in an
undescribed layer, in an undeclared joint, in a vocabulary term read two
ways and in an unasked axis does not say where the next one is. The owner
said as much — patch a class, fail, catch a batch at integration test,
repeat — and the way out was already visible in the fourth, which was
found by *reasoning* rather than by looking: eleven families were listed,
what each one asked was written in a column, and the column had a gap in
it.

So the space itself gets written down. Every rule in the sweep is a
**triple** — a body, a relation, and the frame the relation is read in —
and the three lists are short and finite: what the game describes, what
one body can be to another, and the half-dozen frames anything is
measured in. Crossing them gives eight hundred triples, most of them
meaningless, and the work is dismissing those convincingly. What is left
is a list of questions with three columns: asked, unasked, and
**unaskable**, the last being a question a player could answer by looking
and nothing in the tree can be put to. **Enumerate the space a rule can
be about and close it, rather than adding a family per sighting** — and
an unaskable question is the first rule said from the other side, because
what it names is a description that does not exist yet. The map, its
dismissals, and the four families that came out of it are in
[GAUNTLET.md](GAUNTLET.md).

[GAUNTLET.md](GAUNTLET.md) is the operator's side — how to run it, how to
read a docket line, what each family looks like in the world when it
fires, and what the harness still cannot see.

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
- **The same view shot twice is the same file.** A screenshot run counts
  its clock instead of measuring it: the game clock is paused and
  stepped by exactly one sim tick per frame, so frame 46 lands at the
  same instant on every run and every breathing lamp, sweeping tube and
  drifting star is sampled at the same phase. Its world stops
  reading the wall clock with it, and its shaders are compiled in the
  frame that needs them rather than whenever they are ready. So a
  refactor that was meant to change nothing can be proved to have
  changed nothing by shooting the same view before and after and
  comparing the bytes — which is what
  `the_same_view_shot_twice_is_the_same_bytes` does on every view it
  covers. `--gauge` is deliberately not on that clock: it measures a
  duration, and a counted clock would only give it back the number it
  was told.
- Four `--view` names belong to the window and are *derived*, like
  every other preset: `pane`, `pane-port` and `pane-stbd` stand square
  on to the glass and a stride to either side of it, wherever the board
  actually hangs it. They exist to be **compared** — same pane, two
  eyes, two skies is the exterior's whole claim, and a shot pair is how
  it is checked. `panes` stands back from the mean of every window
  aboard along the mean of the walls they are set in, which is the
  corner shot that holds two *different* windows showing two different
  skies in one frame. A fifth, `drydock`, is the one view that is not
  from aboard: it lets the cabin camera see the void layer and parks it
  off the bow, so a change to the room shells can be *looked* at instead
  of inferred from a window. `--underway` boots a world that has already
  cast off (it charts and pulls the handle through the sim's own input
  frames), for the transit sky.
- Three flags exist for the exterior's scaling claim and nothing else.
  `--panes n` boots the starter ship with every window stripped and `n`
  hung on one wall through the sim's own placement arbiter
  (`fixture::panes_board`) — and `--panes 0` is the sold-window case, a
  ship whose hull had better be solid. `--grouping wall|pane` picks the
  law the exterior gathers panes by; `pane` is the control arm, the cost
  model the first exterior pass had. `--gauge f` settles, times `f`
  frames, prints one parseable line carrying the pane count and the sky
  count, and exits.

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
- **The window stopped being a picture** (`viewport.rs`): it was a
  painted canvas for a slice — no scanlines, no phosphor, stars
  glint-white — and the painting is gone, because a sky that looks the
  same from every angle is a poster. It is a real exterior seen through
  a real aperture now ("The window is a hole", above). Most of the
  painted vocabulary survived the translation as geometry: the streaks
  became a near field the ship actually passes, the destination a body
  that grows off the bow, the berth a world and a dock alongside, the
  jump a flood on the aperture's own near plane, and the whale, the
  parade, the meteors and the ad drone real things out there. What did
  not survive, and why: the **twinkle** (there is no air to twinkle
  through, and it was standing in for the parallax the aperture now
  supplies), the **parked dust motes and their sparkle** (a painted
  substitute for near field, which the stream is), and the **drawn
  gleam and inner shade line** on the glass (a real specular finish and
  a real frame do that job).

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
