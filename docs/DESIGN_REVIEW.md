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
- [ ] Zero text: no rendered strings besides the version corner; every new
      state communicates via shape/color/motion/sound.
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
  delivery tally, all icons and no words — and settings, keybinds, and
  a save browser are all deferred on the same grounds the game defers
  text: the moment a menu starts explaining itself, it has started
  explaining the game
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
