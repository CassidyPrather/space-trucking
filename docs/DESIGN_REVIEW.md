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
      except through the accept lever or the Guild's hangar steal, and the
      drag-monkey tests (solo and six-player) cover any new interactive
      surface automatically — the cabin's gesture and carry synthesis
      included, via its own monkeys.
- [ ] Netcode holds the lockstep contract: state never travels, only
      inputs; new protocol messages are idempotent under duplication and
      reordering, fuzz-parsed, and the six flaky-harness properties in
      `src/net/` stay green.
- [ ] Affordances derive from the rules: anything drawn as a drop target or
      invitation comes from `Sim::drop_targets()`, `placement_check`, or the
      shared `layout` rects — never from re-derived geometry or a restated
      ownership rule.
- [ ] Instruments read monotonically: an action that is better for a party
      moves its gauge toward better, never the reverse — one scale per
      gauge, no special-case formulas that disagree at a boundary, and the
      property test proves it (see `dial_reading_is_monotone_under_pad_changes`).
- [ ] Pause, fast-forward, and save/load all still reachable and exercised
      this session.
- [ ] `src/sim/` and `src/synth.rs` import no engine crate (no bevy, no
      windowing, no clocks); cues say what happened, never what it sounds
      like.
- [ ] Ambient soundscape only — no melodies; new loops pass the seam test.
- [ ] Budgets green: the release perf gates pass
      (`cargo test --release -p space-trucking --test perf -- --ignored`);
      any retuned ceiling is amended in [BUDGETS.md](BUDGETS.md) with a why.
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
- Major modules
- Live multiplayer frontend (console ↔ lockstep session, remote pointers;
  the sim, protocol, and harness are in — see docs/NETWORKING.md)
- Real transport adapter (WebSocket) behind `net`'s transport seam
- Guild-server hosting + wiring global progress into the console
- VRChat port (the Bevy cabin in `crates/cabin` is the first step: the
  sim's one frontend since the 2D console retired — see BAY.md for that
  decision and ART_DIRECTION_3D.md for direction; still deferred from
  the cabin: the tutor ghost, `--replay` playback, a wasm/web build
  (Bevy compiles to wasm when wanted), the telemetry consent surface,
  and the retired console's per-rule violation glyphs)
- Barter redesign: playtests call the trade minigame and economy
  unengaging; both are expected to be redesigned until click-y, so no
  deep investment lands on the counter meanwhile (the desk-scale
  "broker's diorama" conceit is a placeholder, per BAY.md)
- Bay coverings (rugs, wallpaper, paint): the owner's cargo direction
  beyond furniture — a per-cell dressing layer parallel to occupancy,
  sketched in BAY.md, deliberately not built this slice
- Additional star systems
- More events (mimics, ad bots, hull breaches, secret color-code objectives)
- Rat-gnaw repair: DESIGN.md's "requiring repair" reading is deliberately
  deferred — `gnawed` is permanent this pass, a scar the cargo carries
  through the economy. When repair lands it belongs in `src/sim/rats.rs`,
  next to the teeth.
