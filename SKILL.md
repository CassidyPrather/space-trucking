---
name: game-template
description: Cassidy's opinionated template for small Rust web toys — macroquad, wasm, static deploy
---

# game-template

A template for small web toys: Rust + macroquad, compiled to wasm, shipped as a
static page. Native desktop builds also work. Crate `game-template`, lib
`game_template`, bin `game-template`. Code is AGPL-3.0-or-later.

## Repo Map

- `src/sim.rs` — the simulation. Pure, deterministic, no macroquad. Most work
  belongs here.
- `src/synth.rs` — procedural sound effects as WAV bytes. Also pure, also
  unit-tested; the toy ships no audio assets.
- `src/main.rs` — thin macroquad frontend. Window, draw calls, input gathering.
- `src/audio.rs` — the other half of the frontend: turns `sim::Cue`s into
  playback. Binary-crate module, not part of the library.
- `build.rs` — embeds a `git describe` version string.
- `web/index.html`, `web/gl.js`, `web/audio.js` — the static shell, the
  vendored miniquad loader, and the vendored quad-snd audio plugin. Zero
  external requests; honors `prefers-color-scheme` and
  `prefers-reduced-motion`.
- `scripts/build-web.sh` — wasm build → `dist/web/`.
- `.github/workflows/ci-cd.yml` — lint, test, audit, size-budgeted web bundle,
  Pages deploy, release artifacts.
- `benches/` — criterion bench over the sim. Unit tests live in `src/sim.rs`.

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

## The Determinism Contract

The sim advances on a fixed 60 Hz timestep via an accumulator with a frame-dt
clamp, seeded through `fastrand`, and receives input only as an `InputFrame`
struct — so a given seed plus a given input sequence always produces the same
run, and rendering interpolates between the last two states using an alpha.

Do not read wall-clock time, macroquad state, or randomness from inside
`src/sim.rs`. If the frontend needs to tell the sim something, it goes in
`InputFrame`.

The sim has two output channels, and sound uses the second one exactly the way
rendering uses the first: `Sim::particles()` for what to draw, `Sim::cues()`
for what to play. A `Cue` says what happened and how hard, in `0..=1`, never
what it should sound like. Cues live for one `advance()` and are cleared by
the next.

The wider rule: any module that imports macroquad is untestable — macroquad's
globals panic under `cargo test` (a thread assert), they do not fail politely.
Logic you want tested must live macroquad-free like `sim` and `synth` do;
frontend modules get verified by bot playouts or eyeballs.

## House Rules

Every asset gets a `CREDITS.md` line at intake — source, author, license, URL.
CC0 first.

CI enforces a hard wasm size budget (`MAX_WASM_BYTES`, ~1.5 MB by default);
if a change blows it, shrink the change or retune the budget deliberately.

Audio is on: macroquad's `audio` feature plus quad-snd's `audio.js` plugin in
`web/`, which must load after `gl.js` and before `load()` runs. Sounds are
synthesised in `src/synth.rs` rather than loaded, so there are no audio assets
and nothing to credit. macroquad gives you volume and looping and no pitch
control, so pitch variation means baking another buffer.

Browsers keep the audio context suspended until a real gesture. `audio.js`
handles the resume; the looping drone additionally waits for the first press
so it never starts mid-note, and the HUD says `press or click to start` until
it does. Anything long-lived and looping needs the same treatment.

Two independent decoders read `synth`'s bytes — `audrey` natively and the
browser's `decodeAudioData` on the web — and the web one reports failure by
never calling back, which hangs macroquad's loader on a black screen. That is
why `synth.rs` has header tests.

See `docs/GETTING_STARTED.md` for framework and asset-source links.
