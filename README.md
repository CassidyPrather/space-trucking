# Space Trucking

An ambient game about hauling cargo across the solar system, built to be
played in the background: launch, let the ship fly while you do something
else, come back to barter. There is no currency — trade is cargo for cargo,
read off an eagerness dial — and no text or dialogue anywhere but the version
string, because everything the game has to say it says with shape, color,
motion, and sound. The full design notepad (including the eventual 3D/VRChat
ambitions) lives in [DESIGN.md](DESIGN.md); the recurring
stay-on-target checklist lives in
[docs/DESIGN_REVIEW.md](docs/DESIGN_REVIEW.md).

Rust + [macroquad](https://macroquad.rs/), compiled to wasm, deployed as a
static page. Native desktop builds work too. Initialized from
[CassidyPrather/game-template](https://github.com/CassidyPrather/game-template),
which is based on [rust-template](https://github.com/CassidyPrather/rust-template).

## Playing

The screen is the ship's console: a star map (Venus, Earth, Mars, Jupiter,
Uranus, Neptune, and a Spacing Guild station), a 6×4 cargo hold, and a barter
panel. While docked, click a point of interest and pull the launch lever. The
ship travels there in real time — roughly half a minute to two minutes a
leg — and docks itself on arrival. Then barter: drag your goods onto the give
pad and the station's shelf goods onto the take pad until the eagerness dial
looks agreeable, then pull the accept lever. A give pad with nothing asked in
return is a gift, and stations never refuse gifts — no stray drop can ever
lose a piece; cargo leaves you only through that lever, or through one other
door described below.

The cargo carries the lore, and it has opinions about where it sits: heavy
pieces ride the bottom rows, volatiles refuse adjacency, cryo hugs the hull
edge, and at most one Suspicious Crate comes aboard — a matte-black box that
audibly hums, surfaces on far stations' shelves without explanation, and
occasionally does something about it mid-flight. Dock at the Guild carrying
one and it is gone before the bartering opens, shuttled off to a hangar
nobody mentions; an unlabeled lamp plate on the console fills, slowly, with
whatever it is that is being counted.

| Input   | Effect                                          |
| ------- | ----------------------------------------------- |
| Mouse   | everything — select, pull levers, drag cargo    |
| `Space` | pause                                           |
| `F`     | fast-forward, 16× (warp)                        |
| `M`     | mute                                            |
| `R`     | new run                                         |

The game auto-saves — to localStorage on the web, to a `local.data` file
natively (via quad-storage) — and on load fast-forwards up to six hours of
elapsed real time, so the ship keeps flying while the tab is closed. The save
format is versioned (`STV2`) with no compatibility promises before 1.0; an
unreadable save becomes a fresh run, quietly.

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
in input-only lockstep (nobody transmits state — a save string is a join
ticket), and ship crews report hangar deliveries to a central guild server
whose counters cannot double-count. The architecture and its six required
network-failure properties live in [docs/NETWORKING.md](docs/NETWORKING.md);
`cargo run --example convoy` runs a six-client crew over a deliberately
hostile simulated network and prints the convergence trace. The live
multiplayer console (and a real transport) are later slices; the protocol
they will speak is already under test.

Sound is wired up and works in the browser, which is fiddlier than it sounds:
it needs macroquad's `audio` feature, quad-snd's `audio.js` plugin in `web/`,
and something to do about browsers refusing to make noise before the user
clicks. The soundscape — engine drones, dock clunks, hull creaks, the hum —
is synthesised in `src/synth.rs` at startup rather than loaded, so there are
no audio assets to ship or credit. Ambient only; no music. `M` mutes.

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
