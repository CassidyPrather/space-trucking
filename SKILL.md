---
name: space-trucking
description: Ambient space-freight bartering toy — macroquad, wasm, static deploy
---

# space-trucking

An ambient, background-playable game about hauling cargo across the solar
system: Rust + macroquad, compiled to wasm, shipped as a static page. Native
desktop builds also work. Crate `space-trucking`, lib `space_trucking`, bin
`space-trucking`. Code is AGPL-3.0-or-later. Design intent lives in
`DESIGN.md`; the recurring checklist in `docs/DESIGN_REVIEW.md` runs at the
end of every work stage.

The repo is two cargo workspaces on purpose: the root package (library +
2D console), and `crates/cabin` — the Bevy 3D first-person frontend, kept
out of the root workspace because Cargo allows only one `links = "alsa"`
package per graph (macroquad's quad-alsa-sys vs Bevy's alsa-sys). The
cabin path-depends on the library with `default-features = false`; the
`console` feature (default-on) carries the macroquad half. Both frontends
run the same sim, read the same `STV4` saves, and record the same replay
tapes. The cabin's art rules live in `docs/ART_DIRECTION_3D.md`; its
surface trick (3D panels mapped onto `layout` rects so the sim keeps
doing all hit-testing) lives in `crates/cabin/src/surface.rs`.

## Repo Map

- `src/sim/` — the simulation. Pure, deterministic, no macroquad. Most work
  belongs here.
  - `mod.rs` — `Sim`, `InputFrame`, `Cue`, `advance`, `fast_forward`.
  - `layout.rs` — shared console geometry, used for both hit-tests and
    rendering so they cannot disagree.
  - `map.rs` — points of interest, their orbits, and intercept travel.
    Positions are pure functions of the tick; nothing is stored.
  - `cargo.rs` — cargo kinds, pieces, and placement rules.
  - `barter.rs` — valuation and trade resolution.
  - `event.rs` / `rats.rs` / `encounter.rs` — the events, as siblings
    with a uniform hook shape (on_depart/on_dock/travel_tick/on_press +
    own save lines and cues); a new event should copy the shape, not
    invent a framework. `encounter.rs` holds both the travel encounters
    (derelict/gas station/casino/meteors/whale) and the ad drone.
  - `save.rs` — `STV4` serialization.
- `src/net/` — deterministic lockstep multiplayer per `docs/NETWORKING.md`:
  protocol messages, helm/client session state machines, the guild server
  (idempotent max-merge delivery counters), and the seeded flaky-network
  harness the tests run on. Pure and macroquad-free like `sim`; transports
  are a later adapter. `examples/convoy.rs` runs six clients in one command.
- `src/synth.rs` — procedural sound effects as WAV bytes. Also pure, also
  unit-tested; the game ships no audio assets.
- `src/main.rs` — thin macroquad frontend. Window, draw calls, input gathering.
- `src/audio.rs` — the other half of the frontend: turns `sim::Cue`s into
  playback. Binary-crate module, not part of the library.
- `src/storage.rs` — quad-storage wrapper: localStorage on the web, a
  `local.data` file natively. Binary-crate module, both targets.
- `build.rs` — embeds a `git describe` version string.
- `web/index.html`, `web/gl.js`, `web/audio.js`, `web/sapp_jsutils.js`,
  `web/quad-storage.js` — the static shell and the vendored miniquad
  plugins (load order is load-bearing; index.html documents it). Zero
  external requests; honors `prefers-color-scheme` and
  `prefers-reduced-motion`, mirrors the local deep-night window, and owns
  the `#pretty-please` developer-mode ceremony.
- `scripts/build-web.sh` — wasm build → `dist/web/`.
- `.github/workflows/ci-cd.yml` — lint, test, audit, size-budgeted web bundle,
  Pages deploy, release artifacts.
- `benches/` — criterion bench over the sim. Unit tests live in `src/sim/`.
- `crates/cabin/` — the Bevy 3D cabin (its own workspace; see above).
  `bridge.rs` owns the sim/save/tape (the 2D `main.rs`'s shell duties),
  `surface.rs` maps panels onto sim rects, `rig.rs` builds the room and
  the 480×270 pixel-crunch pipeline, `palette.rs` restates the palette
  discipline (purity test included), and the view modules (`nav`, `console`,
  `barter`, `pieces`, `fx`, `audio`) read sim accessors onto geometry.
  Native-only for now; run with `cargo run --manifest-path
  crates/cabin/Cargo.toml`.

## Commands

```bash
cargo build                                                   # build
cargo run                                                     # run natively
cargo clippy --all-targets --all-features -- -D warnings      # lint
cargo clippy --target wasm32-unknown-unknown -- -D warnings   # lint, wasm
cargo fmt                                                     # format
cargo test                                                    # test
./scripts/build-web.sh                                        # wasm -> dist/web/
python3 -m http.server --directory dist/web 8080              # serve it
cargo bench --bench sim_bench -- --quick                      # bench
cargo audit                                                   # audit
```

The web build needs `rustup target add wasm32-unknown-unknown`, and uses
`wasm-opt` from binaryen when it is on PATH.

## Solid vs. soft (change tolerance)

DESIGN.md work is ongoing and requirements will keep moving. Know which
walls are load-bearing:

- **Solid — change deliberately, with tests and a save-magic bump**:
  `src/sim/` (the game), `src/net/` (lockstep + guild), the save/tape
  formats, the `InputFrame` contract, and the cabin's bridge/surface
  contract (panels map layout rects; the sim does all hit-testing).
- **Soft — expected to churn freely**: every cabin view module (`crt`,
  `console`, `barter`, `pieces`, `viewport`, `fx`), the rig's room
  layout (data-first geometry + invariant/sightline tests make
  rearrangement cheap), palette values, canvas paintings, audio gains.
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

Do not read wall-clock time, macroquad state, or randomness from inside
`src/sim/`. If the frontend needs to tell the sim something, it goes in
`InputFrame`.

The sim has two output channels, and sound uses the second one exactly the
way rendering uses the first: scene accessors for what to draw, `Sim::cues()`
for what to play. A `Cue` says what happened and how hard, in `0..=1`, never
what it should sound like. Cues live for one `advance()` and are cleared by
the next. `fast_forward` (used for warp and offline catch-up) suppresses
cues, so six hours of catch-up does not arrive as six hours of clunks.

The wider rule: any module that imports macroquad is untestable — macroquad's
globals panic under `cargo test` (a thread assert), they do not fail politely.
Logic you want tested must live macroquad-free like `sim` and `synth` do;
frontend modules get verified by bot playouts or eyeballs.

## House Rules

No rendered text or dialogue anywhere except the version string in the
corner — every game state must communicate through shape, color, motion, and
sound, because the game is meant to be readable without a shared language.
Anything genuinely unavoidable gets isolated in one place for future
translation.

Cargo is conserved: a piece the player owns never vanishes or changes hands
except through four ceremonies — the accept lever, the Guild's hangar
steal on docking (`Cue::Delivered`, per DESIGN.md's Central Server section),
???'s three-for-one exchange (`Cue::Exchange`), and the outboard net's
sweep (`Cue::Jettison`): cargo dragged onto the net rides outside the hull,
recoverable until the next dock, departure, or encounter close carries it
off. The suspicious crate refuses the net. The casino only ever transmutes
a wagered piece (`Cue::CasinoLoss`), never destroys it.
No drag can destroy anything. The ownership rule lives in exactly one place
(`cargo::player_owned`), the drop matrix consumes it in `Sim::resolve_drop`,
and the renderer's affordances come from `Sim::drop_targets()` — never
restate any of them. The drag-monkey tests in `src/sim/mod.rs` feed
thousands of arbitrary input frames (solo and six-player) and fail the
moment any interaction loses a piece outside those two doors, so new
surfaces are guarded the moment they exist.

Aesthetics are directed, not defaulted: `docs/ART_DIRECTION.md` holds the
conceit (a worn instrument panel; screens vs metal), and all frontend color
lives in `src/palette.rs` — a purity test fails the build on any raw color
constructor elsewhere in the frontend. Follow the file or amend it in the
same change.

The save string is versioned (magic `STV4`), hand-rolled in
`src/sim/save.rs`, with no compatibility guarantees before 1.0. Bump the
magic on any breaking change; an old or corrupt save fails safe into a fresh
game, never a panic.

Every asset gets a `CREDITS.md` line at intake — source, author, license, URL.
CC0 first.

CI enforces a hard wasm size budget (`MAX_WASM_BYTES`, ~1.5 MB by default);
if a change blows it, shrink the change or retune the budget deliberately.

Audio is on: macroquad's `audio` feature plus quad-snd's `audio.js` plugin in
`web/`, which must load after `gl.js` and before `load()` runs. The
soundscape is synthesised in `src/synth.rs` — four seamless loops (engine,
warp engine, suspicious hum, station air) plus one-shots mapped from cues —
so there are no audio assets and nothing to credit. Ambient only, no
melodies. macroquad gives you volume and looping and no pitch control, so
pitch variation means baking another buffer.

Browsers keep the audio context suspended until a real gesture. `audio.js`
handles the resume; the loops additionally wait for the first press so they
never start mid-note. Anything long-lived and looping needs the same
treatment.

Two independent decoders read `synth`'s bytes — `audrey` natively and the
browser's `decodeAudioData` on the web — and the web one reports failure by
never calling back, which hangs macroquad's loader on a black screen. That is
why `synth.rs` has header tests.

See `docs/GETTING_STARTED.md` for framework and asset-source links.
