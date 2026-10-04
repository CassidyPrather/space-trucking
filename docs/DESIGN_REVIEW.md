# Design Review Checklist

[DESIGN.md](../DESIGN.md) asks for "some sort of system for intermittent
design review checklists to make sure we're not getting distracted from our
core goals". This is that system. Run the checklist at the end of every work
stage and before any merge to `main`, and paste the checked list into the PR
or commit description — a checklist nobody sees is a checklist nobody ran.
If a box cannot be checked honestly, that is the finding; fix it or defer it
deliberately below.

## The Checklist

- [ ] Whimsy first: did this change add or remove delight? Name one whimsical
      detail it touched.
- [ ] Zero text: no rendered strings besides the version corner and the
      `Esc` menu's New run label (DESIGN.md says why); every new state
      communicates via shape/color/motion/sound.
- [ ] No currency: nothing countable functions as money — no credits, scores,
      or ratings.
- [ ] No progression creep: no upgrades, unlocks, or permanent power
      increases.
- [ ] Determinism proven: bit-identical, save-round-trip, and
      catch-up-equivalence tests cover every new mechanic and pass.
- [ ] Cargo is conserved: nothing the player owns vanishes or changes hands
      except through a named ceremony — a room's handshake, the burner's
      fire, the Guild's hangar steal, ???'s exchange, or a room parting
      with its own goods — and the carry-monkey tests (solo and
      six-player) cover any new interactive surface automatically,
      crossing seams as well as staying in one room; the cabin's gesture
      and carry synthesis are included, via its own monkeys.
- [ ] Netcode holds the lockstep contract: state never travels, only
      inputs; new protocol messages are idempotent under duplication and
      reordering, fuzz-parsed, and the six flaky-harness properties in
      `src/net/` stay green.
- [ ] Affordances derive from the rules: anything drawn as a drop target or
      invitation comes from `Sim::drop_targets()`, `placement_check`, or the
      shared `layout` rects — never from re-derived geometry or a restated
      ownership rule.
- [ ] Readings answer monotonically: an action that is better for a party
      reads better, never the reverse — one scale per reading, no
      special-case formulas that disagree at a boundary. The eagerness
      dial that this line was written against is gone with the counter;
      the reading it guards now is the composed offer, and the property
      belongs to `barter::compose` (see
      `a_room_composes_the_best_pile_the_proposal_covers`): more proposed
      value never buys less.
- [ ] Pause, fast-forward, and save/load all still reachable and exercised
      this session.
- [ ] `src/sim/` and `src/synth.rs` import no engine crate (no bevy, no
      windowing, no clocks); cues say what happened, never what it sounds
      like.
- [ ] Ambient soundscape only — no melodies; new loops pass the seam test.
- [ ] Budgets green: the release perf gates pass
      (`cargo test --release -p space-trucking --test perf -- --ignored`);
      any retuned ceiling is amended in [BUDGETS.md](BUDGETS.md) with a why.
- [ ] The gauntlet's work order is honest: `cargo test -p cabin` is green,
      and anything the sweep newly catches is either fixed or written into
      `crates/cabin/src/gauntlet.docket` with its numbers — never left to
      a loosened threshold or a line in `ALLOWED`. If this change draws a
      new *family* of thing, it has a pure description the sweep can read
      before it has a mesh; a layer nobody described is a layer nobody
      checks. See [GAUNTLET.md](GAUNTLET.md).
- [ ] Every asset, including vendored JS, has a CREDITS.md row; CC0/MIT
      preferred.
- [ ] Cargo tells the story: any new kind has a lore reason and a distinct
      silhouette.
- [ ] Visuals follow [ART_DIRECTION_3D.md](ART_DIRECTION_3D.md) (and its
      parent [ART_DIRECTION.md](ART_DIRECTION.md)) or amend them in the
      same change: palette roles only (the purity test enforces it),
      correct material family, deterministic wear, no shadow maps.
- [ ] Accessible by default: every new animation is filed as feedback,
      decoration, or instruction per the art docs' Motion sections — the
      split kept honest in code so the reduced-motion gate stays cheap to
      add when a build target carries the flag — and no signal rides on
      hue alone (No hue alone section).
- [ ] Deferred-deliberately list is current.

## Deferred deliberately

Out of scope on purpose, not forgotten. Revisit when the core loop stops
changing; remove a line only by shipping it or striking it in review.

- Mid-flight retargeting (orbital POI motion shipped: intercept courses
  re-aim automatically when the arrival tick moves, but there is still no
  steering once underway)
- Major modules (the Engine's cargo-incinerate idea shipped early as
  the burner — see BAY.md — so the engine no longer waits in this list;
  what remains of a dedicated engine module is nothing)
- Live multiplayer frontend (console ↔ lockstep session, remote pointers;
  the sim, protocol, and harness are in — see docs/NETWORKING.md; the
  topology a joining crewmate's area attaches through is decided in
  docs/ROOMS.md, and cabin-linking will reuse that same interface)
- Real transport adapter (WebSocket) behind `net`'s transport seam
- Guild-server hosting + wiring global progress into the console
- VRChat port (the Bevy cabin in `crates/cabin` is the first step: the
  sim's one frontend since the 2D console retired — see BAY.md for that
  decision and ART_DIRECTION_3D.md for direction; still deferred from
  the cabin: the tutor ghost, `--replay` playback, a wasm/web build
  (Bevy compiles to wasm when wanted), the telemetry consent surface,
  and the retired console's per-rule violation glyphs)
- Anything else in the `Esc` menu. The meta-controls landed there when
  the console face came off the wall — pause, fast-forward, mute, the
  delivery tally, all icons and no words. The new run followed with
  the one label the menu wears, because it is the one control nobody
  can take back. Settings, keybinds, and a save browser are all
  deferred on the same grounds the game defers text: the moment a menu
  starts explaining itself, it has started explaining the game
- ~~Barter redesign~~ — struck in review, decided by decree: the barter
  interface is *removed*, not redesigned, and stations become attached
  rooms cargo is carried into (docs/ROOMS.md). The economy survives its
  interface; the counter, pads, dial, patience, fog, and accept lever
  do not.
- Per-POI barter agents: the core slice runs one deterministic flow at
  every station (docs/ROOMS.md); differentiating each POI's look and
  behavior from the existing lore — temperament, discovery cost, the
  handshake's form — is the slice after
- Dead space inside the **trade surface**. The staging law gives a
  calling room its own ordinary deck (docs/ROOMS.md) and stops there: the
  `Stock` and `Offer` bands are still berths wall to wall, and a market
  eight cells wide paints forty `Stock` cells to put six goods on. That
  is why every station's hardware had to come off its own shopfront in
  the same change — a shop's fittings and a shop's goods want the same
  wall. Sizing those bands to the goods rather than to the room would
  hand the difference back as staging; it is a second change to
  `RoomKind::tile_of`, it needs a rule for how many cells a band keeps,
  and it is deferred rather than guessed at
- Apertures as cargo: doors, ladders, and hatches as re-arrangeable
  pieces with amber grab handles (docs/ROOMS.md's stretch goal, with
  its packing hazards already analysed). The port law is written so
  that day is not a rewrite
- Wallpaper and larger coverings: rugs and paints shipped as the
  dressing layer (BAY.md); wallpaper is the same shape with a bigger
  footprint, waiting on a reason
- **Which of a covering's two bodies a `dresses` line means.** A
  covering draws laid and packed and a binding names one mesh, so a
  dressed covering wears the same body on the deck and on the shelf.
  The rug and the paint tin live with it; the luminous paint is what
  made it a defect rather than a wart, because the coat is the only
  state in which it lights anything, and it is undressed for that
  reason (docs/ART_PIPELINE.md, "Light is not a drawing"). A `form`
  line, or a second binding, is the fix; either wants a rule for what a
  covering with only one of the two declared should draw for the other
- **A bought lamp's shade is glass by one number.** The three lamps
  wake and dim their own glass now (docs/ART_PIPELINE.md, "Light is
  not a drawing"), and the sconce's teardrop is drawn see-through so the
  bulb inside it reads. What makes it see-through is one alpha in
  `glow::glaze`, the same for every glass anybody names; the day a pack
  ships a frosted shade and a clear one, that wants to be the line's
  own number rather than the module's
- The rest of the shell's dressing (docs/ART_PIPELINE.md, "The fabric
  namespace"): the nudge bench takes cargo and not panels, so a panel's
  four numbers are typed and checked by `resolve` rather than nudged;
  a wall is one panel repeated, with no alternation between the pack's
  variants (`SM_Bld_Wall_01_Alt` is catalogued and unused); the vertical
  seam gets no surround when a hatch or ladder is mated, because no
  swept scene mates one yet; and the pack's emissive strips are painted
  rather than lit, which the lamps-are-cargo law would want anyway
- Additional star systems
- More events (mimics, ad bots, hull breaches, secret color-code objectives)
- Rat-gnaw repair: DESIGN.md's "requiring repair" reading is deliberately
  deferred — `gnawed` is permanent this pass, a scar the cargo carries
  through the economy. When repair lands it belongs in `src/sim/rats.rs`,
  next to the teeth.

## Decided without asking

The companion to the list above. Where a choice had a defensible answer
and no taste in it, it was made rather than escalated — but made in the
open, so catching up is a scan of this list and not a feat of memory.
Every line is reversible; strike one by overruling it.

- **The shell is dressed by role, and a room's colour is an atlas.** A
  `fabric/wall` binding dresses every wall aboard; `room::cladding`
  decides how many panels a wall takes and how wide each is. The
  alternative — one binding per room kind per wall — is six times the
  manifest for the same six meshes. A room kind that wants its own look
  says `room = "burner"` on a second table naming the same mesh with a
  different `texture`, because that is how the pack itself recolours: one
  atlas of flat swatches, shipped repainted, with the UVs held fixed.
  Nothing tints or repaints a bought material.
- **A wall panel's relief is squeezed into the notch**, by the shipped
  `scale` on z, so a kit plinth and cornice that stand a fifth of a
  metre proud at natural depth stand three centimetres proud here. The
  alternative was a shelf through every crate berthed along a wall; the
  choice is a manifest number and the law it keeps is the notch's.
- **A door is one mesh in two states, and that corrected an earlier
  reading of this pack.** The note here used to say every
  `Doorframe_0N` was sealed, so a shut door took `Doorframe_01`'s hub
  panel and a mated one took the `Doorframe_Outer_01` through-frame. It
  was measured off the whole file, and the whole file includes the leaf:
  raycast the frame alone and there is an opening behind it, with the
  leaf hung in it as a child object (`SM_Bld_Wall_Door_0N`). The cost of
  the misreading was visible — a doorway swapped one mesh for a
  different one when a room docked, so the wall appeared to change
  shape. Now `Doorframe_02` dresses both roles from one box, and
  `fabric/doorway` hides the leaf by name (`art::Open`). `_02` rather
  than `_01` because its opening is 2.61 × 2.92 in mesh units against a
  square two-cell aperture, a 12% stretch where `_01`'s would be 53%.
- **The lintel over a mated doorway is the wall panel upside down.** The
  panel's skirt then stands above the deckhead where nothing sees it and
  its cornice becomes the beam over the door; the alternative, a panel
  the right way up shrunk to two cells, hangs its plinth into the top of
  the doorway.
- **The cabin's hull is stamped by `room::rebuild` now**, with every
  other room's shell, so the one place deciding whether a plane is cut or
  bought is one place. `rig::structure` is still the list — the
  gauntlet and the exterior read it — and its slabs are drawn exactly as
  before when nothing is bought. The backer plates behind the cabin's
  aft wall and deck went with it: a backer stands in the notch a panel's
  relief fills.

- **A dressed covering keeps its one declared body in both forms**, and
  the two whitebox meta-guards ask their questions in the undressed
  frame. The alternative — refusing a `dresses` line on a covering until
  the manifest can declare per-form bodies — would have undone the first
  rough draft that dressed a rug; the gap is recorded as the fourth
  structural blind spot in docs/GAUNTLET.md instead.

- **A room is four cells tall** (`CEIL_Y` 2.20, `COURSES` still 3), not
  five. Five scales every station's decor by 1.22 vertically and leaves
  1.10 m of blank band over 1.65 m of wall, and DESIGN.md wants the
  space cramped. The cost is real and worth knowing: the band above the
  cornices went from 82 mm to 22 mm, and the parlor's coving had to
  become a 20 mm bead.
- **A riding room refuses to part.** `latch_at`'s doc always said a
  riding room's seam is not asked to part; `the_burner_parts_like_any_
  other_room` pinned the opposite and called selling your furnace
  "legal, foolish, and supported". One click destroying owned equipment
  with no gesture and no recovery is a missing safeguard rather than a
  stance, so the doc won and the test was inverted. A seam that cannot
  part is now drawn without a latch at all.
- **A rig is drawn one cell deep** (`RIG_FAR = RIG_NEAR + CELL`), which
  was the last length in the world off the cargo grid. It costs 31 mm —
  6.7% more air per wall berth.
- **Two spilling rig parts moved rather than the depth band widening.**
  They spill at opposite ends, so a band curing both costs 29% more air
  on *every* wall berth in the game to cure two kinds.
- **A hoop's claim on space is its tube, not the frame it lies in.**
  The other direction — scaling every `Shape::Ring`'s tube up to fill
  its declared box — was measured first and costs 20 new findings to
  cure 5: every frame in the game was authored against today's wafer.
- **`Violation::Athwart` was deleted rather than kept.** Once a
  footprint is stated in the frame of the wall it hangs on, there is
  nothing left for it to refuse.
- **Twenty cargo kinds were re-posed, not translated down.** A flat kind
  lies on a deck; it does not stand on one. Eleven sank, seven stood up
  (a glyph reads a cylinder end-on as a circle), and two — the transit
  chit and the casino chip — were laid on their backs.
- **A purchased body's placement frame is the berth box normalised to
  `[-1, 1]`**, which makes `offset` and `fill` mean exactly what
  `poi::Fitting`'s `at` and `half` mean. The alternative — `scale` in
  metres and `fill` derived from the mesh — was rejected because it puts
  the check out of `xtask`'s reach: the resolver cannot see a
  `cargo::Kind` and must not learn to, and a promise nothing can check is
  the defect `fill` exists to stop, one level up.
- **`scale` and `fill` are redundant on purpose.** Auto-fitting a mesh to
  its declared box would make `fill` unbreakable, and an unbreakable
  promise is not one. The redundancy is the mechanism: the promise is in
  git, the fact is in the mesh, and `resolve` is the one place both
  exist.
- **The fill slack is 0.02 of a berth half-extent** — 5 mm on a one-cell
  kind. Tighter refuses a correctly-rounded `0.18`; looser lets a mesh be
  a centimetre bigger than the box every containment rule reads for it.
- **The gauntlet sweeps `dresses` declarations in every build**, not only
  under `--features art`. The build that can draw a purchased mesh is the
  build CI cannot run, so a sweep gated on the feature would sweep the
  declarations nowhere.
- **`bevy_scene` is not in the `art` feature.** In Bevy 0.19 a loaded
  glTF scene is a `WorldAsset`, which `bevy_gltf` brings via
  `bevy_world_serialization`; `bevy_scene` is now the BSN authoring
  language and this game authors no scenes. One feature, 8 crates, and
  the default tree unchanged at 338 packages.
- **No image decoder was added either.** `png` was already on the list to
  *write* screenshots and Bevy's `png` feature is `image/png`, which
  decodes as well. A Blender `.glb` embeds PNG unless its source was a
  JPEG; `"jpeg"` is a one-word addition the day a pack needs it.
- **The dressed hitbox was deferred, not shipped silently.** Unifying it
  needs the runtime's loaded set at 20-odd call sites across four files,
  because `pieces::drawn_box` is a pure function of `Kind` and a bought
  body's box is a fact about the run. The gap is bounded to the art build
  and written into docs/GAUNTLET.md's blind-spot history.
- **A purchased body's outline is carried down by `art`, not by
  `outline`.** A rig's parts are marked as maskable in the breath they
  are spawned in, and a dressed kind's meshes appear frames later, so
  `build_kind` marks the scene's root and an `art`-gated system hands the
  mark to each body as it arrives. The other direction — the outline pass
  learning to walk a purchased scene — was refused on that module's own
  law: the day a bought mesh replaces a hand-rolled `Cuboid`, nothing in
  `outline` is told. Only the piece NUMBER travels, because what a piece
  is *wearing* is on no component at all: `paint` derives it afresh every
  frame, so a dressed body follows every reading of its piece — aim,
  claim, ghost — for nothing.
- **The placement bench is a launch FLAG, not a key chord.** `--nudge`,
  under `--features art`. Both gates are real but they are not equal: the
  feature decides whether there is a bought mesh to nudge, and the flag
  decides whether the process contains a system that can write to a
  tracked file at all. A chord is a runtime branch — it would leave the
  file-writing code live in every session anybody ever plays, one stuck
  modifier from a silent edit to `art/manifest.toml`. The cost is a
  relaunch to arm it, which is the same relaunch `--fixture` already
  costs.
- **The bench's arrows move the body in the BERTH's axes, not the
  viewer's.** Standing behind a body on the aft wall, `→` moves it to
  your left. The alternative reads better for a second and hides which of
  three numbers is moving; this bench exists to author numbers, and the
  overlay draws a tip on the plus end of every axis so which way is plus
  is shown rather than remembered.
- **One press is one step, and there is no key repeat.** A held arrow at
  sixty frames a second crosses a whole berth in a third of a second, and
  no hand stops it where it meant to. The coarse step is 0.05 of a berth
  half-unit (14 mm on a one-cell kind) and 15°; the fine one is 0.005 and
  1°, and a coarse step is a whole number of fine ones so a mixed nudge
  cannot leave the grid.
- **Every nudged number is snapped to a thousandth.** Without it the
  fourth press of `↑` writes `0.20000002` into the owner's manifest and
  every later diff carries it. A thousandth of a berth half-unit is a
  third of a millimetre — finer than the finest step, and far finer than
  the resolver's 0.02 slack.
- **A save writes three numbers and never `fill`.** Deriving `fill` from
  the measured mesh would make the promise unbreakable, which is the
  decision two lines above this one, inverted. So nudging `scale` can
  leave a `fill` that is no longer true; the save says so on stderr and
  the next `resolve` refuses with the line to paste.
- **A save writes the derived index as well as the manifest.** The
  manifest is the authority and its refusal is the one that stops the
  write; the index is best-effort, and it is written so that "survives a
  restart" is true before the next `resolve` rather than after it. Both
  go through the same surgical line editor, so the bench never learns to
  do `resolve`'s job.
- **The art seam is linted and tested in CI at `--features art`.** About
  25 s on a warm dependency cache (4 s clippy, 21 s tests); about 7 min
  the first time, which the cache then keeps. Code behind a feature
  nothing builds is code that rots.
- **The declared atlas is a positional third argument, not a flag or an
  environment variable.** `<program> <source> <destination> [texture]`
  keeps the whole of a conforming converter at `cp "$1" "$2"` and lets one
  tell "no atlas declared" from "an empty path" by counting arguments. The
  cost is small and real: `ART_CONVERTER=/bin/cp` no longer conforms,
  because `cp` means "into that directory" by a third argument. A two-line
  shim does, and the guards now use one.
- **The declaration fills a silence and never overrules a statement.** A
  material that already names an image *that loads* is left as the
  importer built it — the qualifier was missing here and cost a second
  grey crate; see the three lines below —
  and the atlas is still staged beside the mesh so an FBX that names its
  own texture resolves against it exactly as before. The other direction —
  the manifest's line winning outright — was rejected because it would
  make `texture` a way to repaint art, which is a thing the pipeline has
  no business being able to do quietly.
- **A converted file is named `<source digest>-<recipe>`, where the recipe
  covers the declared atlas and the converter script.** A cache addressed
  by the source mesh alone is a cache in which a fix to the script reaches
  nobody whose cache is warm — which is every machine that has ever
  resolved. The alternative, telling the owner to delete `art/cache/`, is
  a step nobody should have to be told about by a run that has both files
  in front of it.
- **Which converter program ran is deliberately NOT in that recipe.**
  Fingerprinting `$ART_CONVERTER` or a Blender build means finding a
  converter on every run, including the runs with nothing to do, and "a
  second `resolve` needs no converter at all" is worth more. Swapping
  converters is a `rm -r art/cache/glb/`, and it is written down.
- **An image reference that cannot be loaded is silence, not a
  statement.** The skip rule asks whether the pixels can be got at —
  decoded, packed, generated, or a filepath that resolves — rather than
  whether a datablock is attached, because Blender answers an
  unresolvable reference with an empty placeholder and the old question
  said yes to it. This does widen what the declaration may overwrite;
  the boundary that keeps it a fallback is the next line.
- **One loadable image anywhere in a material leaves the whole of it
  alone**, even when another slot is broken. Repainting the broken half
  of a material that demonstrably knows about a real image is how a
  fallback turns into a correction, and Synty's materials carry one
  texture reference, so the case is hypothetical and the rule is not.
- **A broken reference is rebound onto the importer's own node** rather
  than painted with a second node beside it. The nodes, the links and the
  UV coordinates are all exactly what the FBX asked for and only the
  pixels are missing; two Image Texture nodes claiming one Base Color
  input is a worse answer than a normal-map slot wearing a colour atlas.
- **A conversion handed an atlas that reached no material refuses.** Both
  grey crates were conversions that succeeded — exit zero, a measurement
  printed, a plausible file — and a `.glb` with no image in it is a valid
  `.glb`, so no later step can tell. Warning instead was rejected: a
  warning in a run that also prints a conversion table is a warning
  nobody reads until they are already looking at a grey box.
- **The converter script gets guards, run against a fake `bpy`.** Both
  defects were in its control flow rather than in Blender, so a stub
  scene and a trace of what got bound to what catches the class without a
  300 MB install. Python is not a dependency and does not become one: a
  machine with no interpreter says the guards did not run.
- **A berth is quantised to a sixteenth of a cell** (`cargo::FINE = 16`,
  about 34 mm). Finer buys nothing a player can see and costs every
  sweep a factor; coarser starts to show as a visible step when a crate
  is nudged along a wall. It is the frontend's existing finest cut, so
  the two agree on what "the smallest move" is.

  > Superseded by *A berth is quantised to a 256th of a cell* (below).
- **Two volatile pieces need half a cell of clear air, corners
  included.** The grid's rule was "no shared edge"; with free placement a
  gap of one sixteenth would satisfy that, which makes the rule a
  formality. Half a cell is the smallest buffer that still reads as a
  gap from across the room.

  > Superseded in part by *Volatile and light distances are Euclidean*
  > (below): the half cell stands, measured the same way at every angle.
- **Light reaches one cell, and corners count.** A lamp lights anything
  less than a cell away by Chebyshev gap and not wholly inside it. The
  grid's "orthogonal neighbours only" has no meaning once nothing sits
  on the grid: a lamp a sixteenth past a crate's corner is not darker
  than one a sixteenth past its edge. The rat fears a little more of the
  room than it did.

  > Superseded in part by *Volatile and light distances are Euclidean*
  > (below).
- **A tall piece shadows the wall while it stands less than a cell from
  it.** The grid's rule was "touching the baseboard"; a wardrobe half a
  cell off the wall still hides what hangs behind it. A whole cell out
  leaves room to hang a painting.

  > Superseded in part by docs/BAY.md, *Cargo stops colliding*: the
  > shadow refuses nothing. The distance stands as what the game keeps
  > clear of when it places something itself (`cargo::clear`).
- **The drop snaps within a quarter of a cell**, inclusive, onto chart
  edges first and neighbours' edges second. Enough to make flush (and so
  cryo) easy to hit without the piece visibly jumping; a neighbour never
  pulls a piece off the wall it was aimed at.

  > Superseded by *The wall snap reaches an eighth of a cell* and *The
  > neighbour snap is gone* (below).
- **Every rule that asks "what tile is this piece on" reads the tile
  under its centre**, with a centre on a seam reading the cell above and
  to the left. The classes that refuse cargo (stock, threshold, fixture)
  still refuse a footprint touching any of their cells. The tie-break
  is chosen so a piece at a whole-cell anchor reads exactly what the
  grid read.
- **The drop centres the footprint on the pointer**, so lifting a
  two-cell piece by one end and putting it straight back moves it half a
  cell. Keeping the grab offset is a frontend choice (it can aim the
  release where the piece's middle would be) and is left to the carry
  preview's pass.
- **No rotation.** A body's yaw still follows its chart. Free yaw is a
  likely follow-up and changes footprints, so it waits for the owner.

  > Superseded by docs/BAY.md, *Cargo turns*: the owner decided it, and
  > cargo turns at any angle on every surface.
- **Shelves, deals, fluff and the save reader's room-local walks keep
  whole-cell anchors.** They set things out a tile at a time; only
  `first_fit` and `dress_fit` learned the flush-against-a-neighbour
  anchors that find snug gaps.

  > Superseded in part by *An old save or tape starts a new run*
  > (below): the save reader walks nothing now.
- **The carry ghost stands on the berth, not at the crosshair.** It
  takes the pose the drop would land in, a tenth large and lifted 5 cm,
  so the piece can sit up to half its own footprint from the point
  aimed at. The alternative — hanging it at the hit and drawing the
  berth separately — shows two answers to one question.
- **Plain and staging deck paint nothing, ever.** The berth wells were
  the grid's invitation, and a lattice of sockets under a free drop
  invites the eye to places the piece does not go. The one footprint
  patch under the previewed berth is the whole placement hint.
- **A body half a cell or less from a seam turns its back on it.** The
  backing rule's threshold was a rounding allowance on the grid; off it,
  a couch exactly half a cell from the front wall used to face the wall.

  > Superseded in part by *Default facing is procedural only* (below):
  > the threshold stands, in the sim, for what the game places.
- **An old save or tape starts a new run.** This one was the owner's,
  not a call made without asking: "This is a scrappy prototype, save
  version invalidation isn't a concern yet. In fact, worth double
  checking we aren't carrying any weighty migration code and just start
  a new save." So the save reader takes exactly the header this build
  writes (`STV20` then, `STV21` since cargo turned) and the tape reader
  exactly its own (`RPL4` then, `RPL5`), and
  anything else is refused like an unreadable file, which the cabin
  answers with a fresh run. What went: the save's migration chain back
  to the console's `STV4` — its translations, re-seats and walks — and
  the cabin's first-boot adoption of the console's `local.data`, its
  run and its stored dev mode alike. Each existed to carry an old run
  forward, and a prototype pays for that on every format change. What
  stays is integrity, because a current save can still lie: its graph
  is replayed through the validated attach, its berths are
  bounds-checked, its stows and dressings are re-checked, and a save
  that fails any of it is refused whole. Bump a header whenever
  what a line *means* changes, not only its grammar, because a save read
  under the wrong rules loads a board nobody built. The replay, wire and
  telemetry readers already took only their own headers. DESIGN.md asked
  for this stance from the start ("Don't worry too much about
  forwards/backwards compatibility during the prototyping phase"); older
  passages in these docs that promise an old save keeps loading are
  marked superseded by this line.

  > Superseded in part by docs/BAY.md, *Cargo stops colliding*: there
  > are no stows to re-check, and a dressing is re-checked against the
  > room alone, line by line. The headers are `STV22` and `RPL6` now.
- **A berth is quantised to a 256th of a cell** (`cargo::FINE = 256`,
  about 2 mm). A sixteenth, 34 mm, is a step a VR hand can see, and the
  owner asked for no overly aggressive snapping. Finer buys nothing a
  hand can place, and the widest lane, 5,632 units, still fits a `u16`
  with room to spare.
- **A turn is a binary angle, 65,536 to the turn** (`cargo::Turn(u16)`,
  0.0055°). It wraps for free, a quarter is a power of two so the
  quarter turns are exact, and it is finer than any hand. The trig is a
  Q30 Taylor series on one octant, chosen over a table or CORDIC because
  it is the shortest integer code that lands within a hundred-millionth
  of true with the quarters exact.
- **Cryo reaches the hull within a sixteenth of a cell** (`HULL_TOUCH`,
  inclusive). A hand is not exact, and a body a degree off square meets
  a wall at one corner a hair before the other. The wall snap makes
  flush easy anyway, so a deliberate placement never needs the allowance.
- **Volatile and light distances are Euclidean.** Half a cell of clear
  air between two volatile pieces, and less than a cell from a lamp to
  what it lights. A Chebyshev gap is measured along the room's axes, and
  a rule that changes when the pieces turn is the room's rule rather
  than theirs.
- **The wall snap reaches an eighth of a cell**, inclusive, nearer edge
  first and a tie to the lower coordinate. The owner allowed "snap
  placement to wall"; an eighth (32 mm) makes flush easy to hit without
  a piece visibly jumping, where the grid's quarter cell was a jump.
- **The neighbour snap is gone.** The owner asked for no overly
  aggressive snapping, and a hand that set a crate a hair off its
  neighbour meant the hair. Pieces still stand flush: touching is legal.
- **Default facing is procedural only.** `cargo::default_turn` turns
  what the game places itself — the starting board, shelves, salvage,
  harvest, the exchange, the hopper, fluff budding, fixture boards — and
  never a player's drop, which keeps the turn it was carried at. It is
  the cabin's backing rule ported to integers unchanged, so the deckhead
  keeps the rule the cabin gave it (a pendant on the front row backs onto
  the front wall) rather than a flat `Turn(0)`: the gauntlet's
  `berth-turned` family holds a pendant to facing the room.
- **A fitting scan offers the default turn first, then a quarter turn on
  for a footprint that is not square**, and never the other two quarters:
  half a turn lays the very same ground, and the arbiter reads ground.
  A piece is only turned onto its side when nothing aboard takes it
  upright.
- **A carry starts at the held piece's own turn**, and a piece taken out
  of a cubby comes out at `Turn(0)`, the one turn a shelf has. A piece
  set down turned and lifted again carries on from the turn it stands
  at.

  > Superseded in part by docs/BAY.md, *Cargo stops colliding*: there
  > are no cubbies to take a piece out of.
- **The carry turns by 15° on the wheel, 1° on `Ctrl` + wheel, and 15°
  on `Q`, and `Shift+Q` is `Q`'s reverse** — the convenient angles the
  owner offered, as a frontend offer on input and never a rule. A notch
  turns to the *next* multiple of 15° its way, so from 7° one notch up
  is 15° and not 22°; `Ctrl` turns a degree from wherever the carry is,
  snapping to nothing. The wheel rolled up, away from you, turns the
  piece counter-clockwise as seen from the room — as you face the wall,
  for a wall piece. The reverse is `Shift` rather than a second letter
  because nothing reads `Shift` while a piece is in hand: quick-move is
  read on a press, and the click that ends a carry is a release. `R`
  was cut from the keyboard to keep one key from ending a run and is not
  brought back for this, `E` is focus, and the nudge bench's own `Shift`
  is its fine step on its own six keys, which nothing else answers.
- **The carry's facing is held in forty-fifths of a `Turn` unit**, the
  coarsest unit in which a degree and a `Turn` unit are both whole
  numbers, and the sim is sent the nearest `Turn`. So 360 degree-notches
  come back exactly, fifteen of them land on the very turn one plain
  notch does, and a notch that would send the sim the turn it already
  has (a piece set down at 15° and lifted again sits a hair past 15°)
  goes on to the next stop instead of doing nothing visible.
- **Only a carry turns, and only while the body roams.** With the `Esc`
  menu up or a station focused, the wheel and `Q` do what they did
  before, which is nothing; with an empty hand they turn nothing, and
  no wheel travel is saved up for the next carry. A wheel that reports
  pixels (a touchpad, a smooth wheel) is read at Bevy's hundred to the
  notch, and part-notches add up.
- **Volatile spacing kept.** The owner's decree took out every rule that
  sets one piece's body against another's, and kept the special-item
  conditions (docs/BAY.md, *Cargo stops colliding*). Two gas canisters
  keeping half a cell of clear air is both: the one rule left that
  names another piece, and a hazard rather than a clip. It was kept as
  a special item's own condition, without asking; strike it and
  `Violation::Volatile` goes with it, and placement asks nothing of any
  other piece but the one-suspicious-crate rule.
- **The game's tidiness counts both layers and a standing piece's
  shadow.** What the game sets down itself goes where nothing is
  (`cargo::clear`): a rug it lays avoids the couch, a crate it sets out
  avoids the rug, and a painting it hangs avoids the wall behind a
  wardrobe. The other reading — each layer minding only its own — would
  have the hopper bank a rug under the couch and the quick-move stand a
  crate on the heirloom.
- **A crowded spot beats none.** When nothing is clear, `cargo::tidy`
  takes the first spot the arbiter allows rather than refusing. So a full
  ship still takes the comet's ice and ???'s crate, a hopper banks
  everything that has a legal berth at all, and a shelf with no gap left
  still takes the last of a deal's restock. A deal still sends to the
  back room what has no stock tile left to stand on, one piece to a
  tile, as before.
- **The point-pick's order is standing, then smaller, then lower id.**
  Only presses with no aim read it — tests, monkeys, tapes — so it was
  chosen to be the least surprising guess rather than to match a
  picture: laid under standing as always, a small piece out of a big
  one's ground before the big one, and ids so the board's listing order
  never decides.
- **The aim only chooses among what the pointer is on.** The other
  reading — the sim lifts whatever the aim names — would make the aim a
  second hit-test the sim cannot check, and a stale aim in a focus could
  lift the chart tank instead of selecting a star. The sim keeps doing
  every hit-test; the aim breaks the tie.
- **A body set wholly inside another's drawn box is picked after it.**
  The crosshair's one ray meets the nearest pick box first (*The nearest
  rule*), so a vial standing inside a wardrobe is reached by lifting the
  wardrobe. A smarter rule (preferring a box nested in the one the ray
  entered) would pick a vial through a closed crate's walls too, and
  the picture would no longer be the answer.
- **The fixture's four cubby pieces were set down by the game.** The
  vial, fluff, chit and bottled midnight that rode the cabinet's cubbies
  were placed by `cargo::tidy` over the cabin deck's whole cells,
  skipping the aft doorway's doorstep (the board's own courtesy), and
  written by the new writer. They stand along the deck's aft row; one of
  them stands behind the wardrobe, which is where the game's first free
  cell was.
- **The gauntlet's loaded board stays tidy.** The arbiter alone would
  now stand a piece on every cell of every room, each half over the
  last; the load sets cargo out clear of what it already stood, so the
  board the families name their culprits on is the one it always was.
