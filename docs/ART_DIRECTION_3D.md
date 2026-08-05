# Art Direction — the 3D cabin

The 2D prototype's art doc ([ART_DIRECTION.md](ART_DIRECTION.md)) promised
that the 3D pass would inherit its *decisions*, not its pixels. This file is
that inheritance, for `crates/cabin`. Same contract as before: everything
rendered should be traceable to a rule here, and anything wanting a
different rule amends this file in the same change.

## The conceit

**You are aboard. The console became a room.**

The 2D console's regions are physical stations bolted into a cramped
freighter cabin, arranged so muscle memory transfers: the star map is a
recessed phosphor tank upper-left, the console face (destination screen,
ETA gauge, launch lever, icon buttons) upper-right, the hold is a racked
tray low-left, the barter counter low-right. DESIGN.md calls the first 3D
pass "an enclosed box with flavor"; the flavor is ribs, pipes, and panel
mass, not detail assets.

Two camera postures, and the camera **never trails the cursor** — every
move is deliberate, nothing to get seasick over:

- **Roaming**: conventional first person. Pointer locked, mouse looks,
  WASD walks a clamped envelope, a small glint crosshair marks aim. Aim
  at a station and its frame invites with a glint outline.
- **Focused**: click (or `E`) glides the camera (~0.4 s, eased, no
  overshoot) to that station's authored viewpoint; the cursor frees and
  precise sim interaction works exactly as in 2D. `Esc`, right-click, or
  `E` steps back out. The two desk panels share one viewpoint so cargo
  drags can cross from hold to counter without leaving focus.

Focus viewpoints are *fitted*, not eyeballed: each is derived from its
panel group's extents and the camera FOV, so panels fill the view and a
retuned panel moves its viewpoint with it. Stations only need to look
composed from their focus pose and presentable in passing — the focused
view is the contract, the roam is atmosphere.

The sim is the same one the 2D console runs — same save string, same
flight-recorder tape. Every interactive panel is a `SimSurface`: an
oriented quad bound to a rect of the sim's 800×600 logical world. The
cursor ray maps through the panel into sim coordinates, so the sim keeps
making every ruling, and hit-tests can never disagree with the rules.

## Two material families, again

| Family | Elements | Treatment |
| --- | --- | --- |
| **Worn metal** | walls, panel slabs, wells, levers, buttons, dial housing | lit `StandardMaterial`, high roughness, palette roles; brass gets real metalness |
| **Phosphor & lamps** | tank readings, route lines, lamps, crate glow, dial fill | emissive materials on near-black bodies; bloom supplies the halo |

The rule of thumb: if the 2D console drew it on a CRT or as a lamp, it is
emissive here; if it drew it as metal, it is a lit surface. Lamps are
still glass first — a dark `GLASS` body whose emissive wakes, never a
recolored box.

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
anywhere else in the crate. Cargo keeps its sixteen identity hues; POIs
keep their enamel identities. What changed is interpretation: a role is
now either a surface color or an emissive color, and *light itself is a
palette instrument*:

- The cabin is lit warm (`GLINT` overhead, `PLATE_LIT` fill) with a low
  green spill by the tank (`PHOSPHOR`), so the metal reads green-gray as
  the 2D plates did.
- **The omen dims the actual lights.** `sim.light()` scales every
  `Dimmable` source; a violet `EERIE` source swells with `sim.omen()`;
  the jump is a sub-half-second `EERIE_BRIGHT` flash. Screens and lamps
  keep glowing in the dark — phosphor doesn't care about the room, which
  is exactly why a dimmed cabin feels wrong in the right way.

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
  `SimSurface`; never restate a hit-test in 3D terms.
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

The class of defect where furniture swallows a panel edge, or a viewpoint
clips a wall, is handled structurally, not by eyeballing:

- Structural masses are **data before entities** (`rig::structure()`),
  and anything that must relate to a panel — the desk supports, the
  focus viewpoints — is **derived from the panel's own corners and
  extents**, never authored twice.
- Unit tests walk the invariants: no slab may contain any sample point
  of any panel face, every focus pose must be a legal camera position
  facing its panels, and the roaming envelope must be clear of every
  slab. Break the layout and the build breaks.
- **Sightlines are tested, not eyeballed**: from each station's focus
  viewpoint, every panel corner and every interactive control (levers,
  dial, slots, buttons, grid cells) must sit inside the camera frustum
  with an unoccluded line from the eye — checked by frustum math plus
  occlusion rays against every slab and panel plate. A control that a
  refit pushes off-screen or behind furniture fails the build with the
  blocking geometry named.
- The cabin has a **screenshot mode** (`--shot out.png`, optionally
  `--view tank|console|desk`) that renders, saves one capture, and
  exits. It runs headless under xvfb with llvmpipe, so visual review
  happens in CI-shaped environments too — geometry changes should come
  with fresh captures, looked at.

## Open questions for the aesthetic experiment

Deliberately unsettled, to be answered by iteration in this crate:

- Whether the crunch target should breathe with window size or stay
  fixed 16:9 letterboxed.
- Whether panel wear (the 2D deterministic scratches) returns as decals,
  vertex-color grime, or stays implied by the low light.
- Whether the porthole (travel feel) earns a place on the back wall or
  stays a nav-tank-only fiction.
- How far bloom can carry the CRT reading before it goes syrupy.
