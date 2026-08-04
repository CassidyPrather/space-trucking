# Game Template

Cassidy's opinionated template for small Rust web toys. Enter at your own peril.

Rust + [macroquad](https://macroquad.rs/), compiled to wasm, deployed as a
static page. Based on [rust-template](https://github.com/CassidyPrather/rust-template)
Native desktop builds work too.

Sound is wired up and works in the browser, which is fiddlier than it sounds:
it needs macroquad's `audio` feature, quad-snd's `audio.js` plugin in `web/`,
and something to do about browsers refusing to make noise before the user
clicks. The effects themselves are synthesised in `src/synth.rs` rather than
loaded, so there are no audio assets to ship or credit. `M` mutes.

## Development

Requires [Rust](https://rustup.rs/). Native builds on Linux also need ALSA's
development files (`libasound2-dev` on Debian/Ubuntu, `alsa-lib-devel` on
Fedora) — without them the link step fails with `unable to find library
-lasound`. The wasm build needs none of this; the browser handles audio.

Build: `cargo build`

Run (native): `cargo run`

Lint: `cargo clippy --all-targets --all-features -- -D warnings`

Lint (wasm): `cargo clippy --target wasm32-unknown-unknown -- -D warnings`

Format: `cargo fmt`

Test: `cargo test`

Web build: `./scripts/build-web.sh` (needs
`rustup target add wasm32-unknown-unknown`; uses `wasm-opt` from
[binaryen](https://github.com/WebAssembly/binaryen) if installed)

Serve the result: `python3 -m http.server --directory dist/web 8080`

### Advanced

Benchmark: `cargo bench --bench sim_bench -- --quick`

Security audit: `cargo audit` (requires `cargo install cargo-audit`)

Pre-commit hook: `git config core.hooksPath .githooks` (runs `cargo fmt`)

## Template Setup

1. **Create a new repository** from this template on GitHub (click "Use this
   template")

2. **Clone your new repository** and navigate to it

3. **Update project metadata** in `Cargo.toml`:
   - Change `name` to your toy's name (kebab-case, e.g. `orbit-fidget`)
   - Update the `[lib]` name to match (snake_case, e.g. `orbit_fidget`)
   - Update the `[[bin]]` name to match (kebab-case)
   - Update `description`, `repository`, and `keywords`
   - Update dependencies as required

4. **Update the binary name everywhere it is hardcoded**:
   - `web/index.html`: the `.wasm` filename it loads
   - `scripts/build-web.sh`: `BIN=`
   - `.github/workflows/ci-cd.yml`: `BIN=` in the package step

5. **Update `.vscode/launch.json`** (if using VS Code):
   - Replace the old crate name in the `cargo` sections

6. **Tune `MAX_WASM_BYTES`** in `.github/workflows/ci-cd.yml` — the default
   budget is sized for a toy, not for your sprite atlas

7. **Update the README**:
   - Replace the title and description
   - Remove or customize this Template Setup section

8. **Verify everything works**:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ./scripts/build-web.sh
   ```

### Replacing the demo

The fidget exists so the template ships exercised, not because it is precious.
Its full blast radius, in gutting order:

1. `src/sim.rs` — replace wholesale. Keep the shape: seed and `InputFrame` in,
   fixed ticks, `particles()`/`cues()` and an interpolation alpha out.
   Turn-based toys can swap the accumulator for one-command-per-advance; the
   determinism contract does not care.
2. `src/synth.rs` — replace the sound recipes; keep the header tests, which
   exist because the browser hangs instead of erroring on a bad header.
3. `src/main.rs` and `src/audio.rs` — `gather_input`, `draw`, and the
   cue-to-sound mapping are demo-specific. `window_conf`, the main loop, the
   `View` letterboxing, and the mute/resume plumbing are genre-agnostic.
4. `benches/sim_bench.rs` — rewrite against your sim's hot path.

Nothing else references the demo. Keep new logic in the sim like the old logic
was: modules that import macroquad panic under `cargo test` (see SKILL.md).

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) for generated tips: Frameworks, asset
sources, shader references, and so on and so forth.
