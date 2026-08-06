# Space Trucking

An ambient game about hauling cargo across the solar system, built to be
played in the background: launch, let the ship fly while you do something
else, come back to barter. Design intent, lore, and the long-term (3D/VRChat)
ambitions live in [DESIGN.md](DESIGN.md); the recurring stay-on-target
checklist lives in [docs/DESIGN_REVIEW.md](docs/DESIGN_REVIEW.md). This file
sticks to what the prototype does today and how to work on it.

The game is a first-person freighter cabin: Rust + [Bevy](https://bevy.org/)
(`crates/cabin`), native only for now. The game itself — sim, saves, replay
tapes, netcode, soundscape — is a pure, engine-free library (`src/`) the
frontend drives through input frames; everything interesting runs headless
in `cargo test`. The original 2D macroquad console that hammered out the
game logic retired when the walkable-bay work began — the decision and
what replaced its discipline live in [docs/BAY.md](docs/BAY.md), and the
console itself lives on in version history. Direction for the 3D pass
lives in [docs/ART_DIRECTION_3D.md](docs/ART_DIRECTION_3D.md).

Run it:

```bash
cargo run --release -p cabin          # --release is the way to *play*
cargo run --release -p cabin -- --dev # with the 16x warp unlocked
```

Dev builds trade frame rate for compile speed; dev tooling:
`-- --shot out.png --view desk` renders one screenshot and exits.

Controls: mouse looks and `WASD` walks; aim at a station and click
(or `E`) to focus it — the camera glides to a fitted viewpoint and the
cursor frees for the usual clicking and dragging. `Esc`, right-click, or
`E` steps back out of a station; `Esc` while roaming hands the cursor
back to your desktop (click the game to reclaim it). Cargo lives in the
aft bay and is carried, not dragged: walk up, aim the crosshair at a
piece and click to pick it up, walk it over, click a berth to set it
down — clicking at nothing (or right-clicking) sends it back where it
came from, and `Shift`+click quick-moves without carrying. Drop a small
item onto a cabinet and it takes a cubby; a loaded cabinet won't budge
until it's emptied. The launch and accept levers are pulls: grab, drag
to the end of the track, and the throw fires at the detent. `Space`
pauses, `M` mutes, `R` starts a new run (`F` warps, in dev mode).

Saves: the cabin keeps its own slot (`cabin.data` + `cabin.replay`
beside the working directory). On a boot with no slot of its own it
**adopts the retired 2D console's `local.data`** — same save string,
same offline catch-up, and a dev mode earned in the console carries
over. Adoption happens once; delete `cabin.data`/`cabin.replay` to
re-adopt.

## Playing

The cabin's stations are the ship's console made physical: a star map in
its chart nook, the console face by the window, a cargo bay, and a barter
counter. The planets orbit the sun in real time — Venus through Neptune,
Saturn included, plus a Spacing Guild station running its orbit the wrong
way round — and a few stranger stops that only show themselves under the
right conditions. While docked, click a point of interest and pull the
launch lever: the course is charted to where the destination *will* be, and
the ship crawls there in real time. Journeys between the outer-ring worlds
run tens of minutes on purpose; this is a game meant to sit in a corner of
your day. The three inner-ring factions barely tolerate each
other: charting a direct course from one inner world to another takes
transit papers, which the Guild happens to broker. Approaching from the
outer ring is nobody's business but yours.

Then barter — no currency, cargo for cargo, read off an eagerness dial. The
dial only reads true for goods you have traded at that station before;
unfamiliar goods fog the needle, and finding out what a station really pays
means pulling the lever and living with the answer. Stations have patience,
and three wasted pulls ends the visit's trading — though no station in the
system refuses a gift. Cargo has opinions about stowage (heavy rides low,
volatiles refuse adjacency, cryo hugs the hull, fixtures demand their
surface), and one matte-black kind of crate hums, vanishes into a Guild
hangar on delivery, and fills an unlabeled lamp plate with whatever is
being counted. The barter counter moonlights when no trade is open:
underway, its shelf row becomes the outboard rail — cargo put there rides
outside the hull, recoverable until the next port call or cast-off sweeps
it away (the humming crate refuses to go) — and its dial housing wears the
badge of whatever pulls alongside mid-leg. Encounter salvage drifts into
the same rail, and at stranger berths the counter shows stranger things.

| Input           | Effect                                                     |
| --------------- | ---------------------------------------------------------- |
| Mouse           | look, focus stations, pull levers                          |
| Click (bay)     | pick up / set down the aimed cargo                         |
| Right-click     | cancel a carry (the piece snaps home)                      |
| `WASD`          | walk the cabin                                             |
| `E`             | focus / unfocus the aimed station                          |
| `Shift`+click   | quick-move a piece to its obvious destination              |
| `Space`         | pause                                                      |
| `M`             | mute                                                       |
| `R`             | new run                                                    |

No signal relies on color alone; refusals, warnings, and states all carry
a shape, brightness, or position tell alongside their hue. (The retired
web build's reduced-motion support returns if a web target ever does.)

The game auto-saves and on load fast-forwards up to six hours of elapsed
real time, so the ship keeps flying while the window is closed. The save
format is versioned with no compatibility promises before 1.0; an
unreadable save becomes a fresh run, quietly.

### Privacy

The opt-in telemetry contract from the web prototype lives on in
[docs/TELEMETRY.md](docs/TELEMETRY.md) and `src/telemetry.rs`, but native
builds never ask and never collect — today, nothing is recorded, ever.

## Multiplayer

The deterministic core is multiplayer-ready: up to six players crew one ship
in input-only lockstep, and crews report hangar deliveries to a central
guild server whose counters cannot double-count. The architecture and its
required network-failure properties live in
[docs/NETWORKING.md](docs/NETWORKING.md); `cargo run --example convoy` runs
a six-client crew over a deliberately hostile simulated network. The live
multiplayer cabin is a later slice; the protocol it will speak (`SNP2`)
is already under test.

Sound is synthesised at startup in `src/synth.rs` — no audio assets —
ambient only, no music. `M` mutes.

## Development

Requires [Rust](https://rustup.rs/). On Linux, Bevy needs ALSA and udev
development files plus the wayland/xkbcommon headers
(`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev` on
Debian/Ubuntu) — without them the build fails at the link or
window-system probe step.

Build: `cargo build`

Run: `cargo run --release -p cabin`

Lint: `cargo clippy --workspace --all-targets -- -D warnings`

Format: `cargo fmt`

Test: `cargo test --workspace`

### Developer mode (fast-forward)

The game runs at 1× for everyone; the 16× fast-forward is a development
tool, hidden until asked for nicely: run with `-- --dev`. A dev-mode save
keeps the privilege across boots. Developer mode reveals the warp button
and the `F` key.

### Flight recorder

The game keeps a black box: a recent save plus every input frame since
(`cabin.replay`), re-based on a rolling cap so it always holds the recent
past. The sim is deterministic, so that small text file *is* the session
— the bridge replays it after a stall, and a recording attached to a bug
report is a perfect reproduction under `cargo test`. An in-cabin playback
mode is on the deferred list.

### Advanced

Benchmark: `cargo bench --bench sim_bench -- --quick`

Performance budgets (CI-enforced ceilings, see
[docs/BUDGETS.md](docs/BUDGETS.md)): `cargo test --release -p space-trucking --test perf -- --ignored`

Security audit: `cargo audit` (requires `cargo install cargo-audit`)

Pre-commit hook: `git config core.hooksPath .githooks` (runs `cargo fmt`)

Headless screenshots (works under xvfb + llvmpipe, for CI-shaped review):
`cargo run -p cabin -- --shot out.png --view tank`

## More

See [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) for framework and
asset-source links, and [docs/DEPLOYING.md](docs/DEPLOYING.md) for how
releases are built and shipped.
