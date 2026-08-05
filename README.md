# Space Trucking

An ambient game about hauling cargo across the solar system, built to be
played in the background: launch, let the ship fly while you do something
else, come back to barter. Design intent, lore, and the long-term (3D/VRChat)
ambitions live in [DESIGN.md](DESIGN.md); the recurring stay-on-target
checklist lives in [docs/DESIGN_REVIEW.md](docs/DESIGN_REVIEW.md). This file
sticks to what the prototype does today and how to work on it.

Rust + [macroquad](https://macroquad.rs/), compiled to wasm, deployed as a
static page. Native desktop builds work too. Initialized from
[CassidyPrather/game-template](https://github.com/CassidyPrather/game-template),
which is based on [rust-template](https://github.com/CassidyPrather/rust-template).

## Playing

The screen is the ship's console: a star map, a 6×4 cargo hold, and a barter
panel. The planets orbit the sun in real time — Venus through Neptune,
Saturn included, plus a Spacing Guild station running its orbit the wrong
way round — and a few stranger stops that only show themselves under the
right conditions. While docked, click a point of interest and pull the
launch lever: the course is charted to where the destination *will* be, and
the ship crawls there in real time. Journeys between the outer-ring worlds
run tens of minutes on purpose; this is a game meant to sit in a corner of
your day. Charting a direct course to the inner ring (Venus, Earth, Mars)
takes transit papers, which the Guild happens to broker.

Then barter — no currency, cargo for cargo, read off an eagerness dial. The
dial only reads true for goods you have traded at that station before;
unfamiliar goods fog the needle, and finding out what a station really pays
means pulling the lever and living with the answer. Stations have patience,
and three wasted pulls ends the visit's trading — though no station in the
system refuses a gift. Cargo has opinions about stowage (heavy rides low,
volatiles refuse adjacency, cryo hugs the hull), and one matte-black kind of
crate hums, vanishes into a Guild hangar on delivery, and fills an unlabeled
lamp plate with whatever is being counted.

| Input           | Effect                                                     |
| --------------- | ---------------------------------------------------------- |
| Mouse           | everything — select, pull levers, drag cargo               |
| `Shift`+click   | quick-move a piece to its obvious destination              |
| `Space`         | pause                                                      |
| `M`             | mute                                                       |
| `R`             | new run                                                    |

Accessibility: on the web the game honors your system's reduced-motion
preference (applied at load) — decorative idle animation freezes to a
readable static pose, while everything caused by play still moves. No signal
relies on color alone; refusals, warnings, and states all carry a shape,
brightness, or position tell alongside their hue.

The game auto-saves — to localStorage on the web, to a `local.data` file
natively (via quad-storage) — and on load fast-forwards up to six hours of
elapsed real time, so the ship keeps flying while the tab is closed. A
backgrounded tab catches up the same way the moment it wakes: real time
always passes. The save format is versioned (`STV4`) with no compatibility
promises before 1.0; an unreadable save becomes a fresh run, quietly.

### Privacy

Telemetry is opt-in and off by default. The web page asks once, before first
play, whether the game may keep anonymous play statistics — coarse counts
and whole-second durations only, no identity — stored in your own browser's
localStorage and sent nowhere. Decline, or never answer, and nothing is
recorded; any previously stored buffer is deleted on the next boot. Clearing
site data clears the choice and re-asks. Native builds never ask and never
collect. The full contract, including the exact schema, lives in
[docs/TELEMETRY.md](docs/TELEMETRY.md).

## Multiplayer

The deterministic core is multiplayer-ready: up to six players crew one ship
in input-only lockstep, and crews report hangar deliveries to a central
guild server whose counters cannot double-count. The architecture and its
required network-failure properties live in
[docs/NETWORKING.md](docs/NETWORKING.md); `cargo run --example convoy` runs
a six-client crew over a deliberately hostile simulated network. The live
multiplayer console is a later slice; the protocol it will speak (`SNP2`)
is already under test.

Sound is synthesised at startup in `src/synth.rs` — no audio assets —
ambient only, no music. `M` mutes. On the web it needs macroquad's `audio`
feature and quad-snd's `audio.js` plugin in `web/`, and browsers refuse to
make noise before the first click.

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

### Developer mode (fast-forward)

The game runs at 1× for everyone; the 16× fast-forward is a development
tool, hidden until asked for nicely. Natively, run with `--dev`. On the
web, open the page with `#pretty-please` in the URL and answer the shell's
one question honestly (`#no-thank-you` revokes). Developer mode reveals the
warp button and the `F` key.

### Flight recorder

The game keeps a black box: a recent save plus every input frame since,
stored beside the autosave under the key `space-trucking/replay`
(localStorage on the web, quad-storage's `local.data` natively) and re-based
on a rolling cap so it always holds the recent past. The sim is
deterministic, so that small text file *is* the session: copy it out and
`cargo run -- --replay <file>` plays it back natively, bit-identically,
with the version string tinted amber as the only tell. A recording attached
to a bug report is a perfect reproduction.

### Advanced

Benchmark: `cargo bench --bench sim_bench -- --quick`

Performance budgets (CI-enforced ceilings, see
[docs/BUDGETS.md](docs/BUDGETS.md)): `cargo test --release --test perf -- --ignored`

Security audit: `cargo audit` (requires `cargo install cargo-audit`)

Pre-commit hook: `git config core.hooksPath .githooks` (runs `cargo fmt`)

## More

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) for framework and
asset-source links, and [docs/DEPLOYING.md](docs/DEPLOYING.md) for how the
web build reaches GitHub Pages (or any static host).
