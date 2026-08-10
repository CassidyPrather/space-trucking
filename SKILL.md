---
name: space-trucking
description: Ambient space-freight bartering toy — Bevy first-person cabin over a pure deterministic sim
---

# space-trucking

An ambient, background-playable game about hauling cargo across the solar
system: a Bevy first-person freighter cabin (`crates/cabin`, native) over a
pure, engine-free game library (root package). Crate `space-trucking`, lib
`space_trucking`; the cabin package is `cabin` and its `[[bin]]` keeps the
`space-trucking` name. Code is AGPL-3.0-or-later. Design intent lives in
`DESIGN.md`; the recurring checklist in `docs/DESIGN_REVIEW.md` runs at the
end of every work stage.

One workspace, two packages. The original 2D macroquad console retired when
the walkable-bay slice began — `docs/BAY.md` records the decision and the
law that replaced the "2D analogue" rule: **every mechanic must remain
expressible and testable through `InputFrame`s against `layout` rects.**
The sim keeps its 800×600 logical world; the cabin maps 3D surfaces onto it
(`crates/cabin/src/surface.rs`), so the sim does all hit-testing. The
cabin's art rules live in `docs/ART_DIRECTION_3D.md`.

## Repo Map

- `src/sim/` — the simulation. Pure, deterministic, engine-free. Most work
  belongs here.
  - `mod.rs` — `Sim`, `InputFrame`, `Cue`, `advance`, `fast_forward`.
  - `layout.rs` — logical-world geometry, the shared hit-test space.
  - `map.rs` — points of interest, their orbits, and intercept travel.
    Positions are pure functions of the tick; nothing is stored.
  - `cargo.rs` — cargo kinds, pieces, and placement rules.
  - `barter.rs` — valuation and trade resolution.
  - `event.rs` / `rats.rs` / `encounter.rs` — the events, as siblings
    with a uniform hook shape (on_depart/on_dock/travel_tick/on_press +
    own save lines and cues); a new event should copy the shape, not
    invent a framework. `encounter.rs` holds both the travel encounters
    (derelict/gas station/casino/meteors/whale) and the ad drone.
  - `save.rs` — versioned save serialization.
- `src/net/` — deterministic lockstep multiplayer per `docs/NETWORKING.md`:
  protocol messages, helm/client session state machines, the guild server
  (idempotent max-merge delivery counters), and the seeded flaky-network
  harness the tests run on. Engine-free like `sim`; transports are a later
  adapter. `examples/convoy.rs` runs six clients in one command.
- `src/synth.rs` — procedural sound effects as WAV bytes. Also pure, also
  unit-tested; the game ships no audio assets.
- `src/replay.rs` — the flight-recorder tape format (`RPL2`).
- `src/telemetry.rs` — the opt-in play-statistics contract; dormant (no
  frontend collects today).
- `build.rs` — embeds a `git describe` version string.
- `.github/workflows/ci-cd.yml` — lint, test, perf budgets, audit,
  release artifacts.
- `benches/` — criterion bench over the sim. Unit tests live in `src/sim/`.
- `crates/cabin/` — the game people play. `bridge.rs` owns the sim/save/
  tape (shell duties), `surface.rs` maps 3D quads onto sim rects, `rig.rs`
  builds the room, camera (roam + focus), and the 480×270 pixel-crunch
  pipeline with invariant/sightline tests, `gesture.rs` synthesizes
  pointer frames (lever pulls, carry), `palette.rs` restates the palette
  discipline (purity test included), `canvas.rs`+`crt.rs` are the software
  rasterizer behind the phosphor screens, and the view modules
  (`console`, `barter`, `pieces`, `viewport`, `fx`, `audio`) read sim
  accessors onto geometry. Native-only for now.

## Commands

```bash
cargo run --release -p cabin                         # play
cargo build                                          # build
cargo clippy --workspace --all-targets -- -D warnings # lint
cargo fmt                                            # format
cargo test --workspace                               # test
cargo bench --bench sim_bench -- --quick             # bench
cargo audit                                          # audit
cargo run -p cabin -- --shot out.png --view desk     # headless screenshot
```

## Solid vs. soft (change tolerance)

DESIGN.md work is ongoing and requirements will keep moving. Know which
walls are load-bearing:

- **Solid — change deliberately, with tests and a save-magic bump**:
  `src/sim/` (the game), `src/net/` (lockstep + guild), the save/tape
  formats, the `InputFrame` contract, and the cabin's bridge/surface
  contract (surfaces map layout rects; the sim does all hit-testing).
- **Soft — expected to churn freely**: every cabin view module (`crt`,
  `console`, `barter`, `pieces`, `viewport`, `fx`), the rig's room
  layout (data-first geometry + invariant/sightline tests make
  rearrangement cheap), palette values, canvas paintings, audio gains.
  The barter economy and its presentation are *expected* to be redesigned
  until they click — playtest verdict — so avoid deep investment there.
- **Amend-in-the-same-change**: the art direction docs and
  `docs/DESIGN_REVIEW.md`'s deferred list — divergence without amendment
  is the failure mode, not change itself.

## The Determinism Contract

The sim advances on a fixed 60 Hz timestep via an accumulator with a frame-dt
clamp, and receives input only as an `InputFrame` struct: pointer edges
(press/held/release with position) plus the toggles. A given seed plus a
given input sequence produces a bit-identical run — determinism is
structural, via splitmix RNG streams derived from the seed, not incidental —
and rendering interpolates between the last two states using an alpha.

Do not read wall-clock time, engine state, or randomness from inside
`src/sim/`. If the frontend needs to tell the sim something, it goes in
`InputFrame`.

The sim has two output channels, and sound uses the second one exactly the
way rendering uses the first: scene accessors for what to draw, `Sim::cues()`
for what to play. A `Cue` says what happened and how hard, in `0..=1`, never
what it should sound like. Cues live for one `advance()` and are cleared by
the next. `fast_forward` (used for warp and offline catch-up) suppresses
cues, so six hours of catch-up does not arrive as six hours of clunks.

The wider rule: logic you want tested must live engine-free like `sim` and
`synth` do; frontend modules get verified by the cabin's invariant and
sightline tests, headless screenshots, or eyeballs.

## House Rules

No rendered text or dialogue anywhere except the version string in the
corner — every game state must communicate through shape, color, motion, and
sound, because the game is meant to be readable without a shared language.
Anything genuinely unavoidable gets isolated in one place for future
translation.

Cargo is conserved: a piece the player owns never vanishes or changes hands
except through four ceremonies — a room's handshake (`Cue::Accept`: the
one physical act that commits a standing offer, and the only place
ownership crosses), the Guild's hangar steal on docking
(`Cue::Delivered`, per DESIGN.md's Central Server section), ???'s
three-for-one exchange (`Cue::Exchange`), and the burner taking a
feeding (`Cue::Burn`: cargo staged on the furnace room's hazard tiles
rides recoverable until the stoker's beat shovels it into the fire for
boost). Fuel simply stays staged, so nothing is ever tipped over the
side and conservation is total. The suspicious crate refuses the fire.
The casino only ever transmutes a wagered piece (`Cue::CasinoLoss`),
never destroys it. A room that parts takes its OWN goods with it, and the
gangway law makes sure nothing of the player's is aboard it.
No interaction can destroy anything. The ownership rule lives in exactly one
place (`cargo::player_owned`), the drop matrix consumes it in
`Sim::resolve_drop`, and the renderer's affordances come from
`Sim::drop_targets()` — never restate any of them. The drag-monkey tests in
`src/sim/mod.rs` feed thousands of arbitrary input frames (solo and
six-player) and fail the moment any interaction loses a piece outside those
doors, so new surfaces are guarded the moment they exist. The cabin's
gesture monkey extends the same guarantee through the lever/carry
synthesis layer.

Aesthetics are directed, not defaulted: `docs/ART_DIRECTION_3D.md` holds
the conceit, and all frontend color lives in `crates/cabin/src/palette.rs`
— a purity test fails the build on any raw color constructor elsewhere in
the crate. Follow the file or amend it in the same change.

The save string is versioned, hand-rolled in `src/sim/save.rs`, with no
compatibility guarantees before 1.0. Bump the magic on any breaking change;
an old or corrupt save fails safe into a fresh game, never a panic.

Every asset gets a `CREDITS.md` line at intake — source, author, license,
URL. CC0 first. (Today there are none: geometry, textures, and sound are
all code.)

The soundscape is synthesised in `src/synth.rs` — four seamless loops
(engine, warp engine, suspicious hum, station air) plus one-shots mapped
from cues — ambient only, no melodies. `synth.rs` keeps header tests
because decoder failures on malformed WAV bytes are silent hangs, not
errors.

See `docs/GETTING_STARTED.md` for framework and asset-source links.
