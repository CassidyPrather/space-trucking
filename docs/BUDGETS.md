# Performance Budgets

DESIGN.md asks for profiling metrics to enforce, not merely observe. These
are them. Every budget is a hard gate somewhere in CI; a change that blows
one either shrinks or retunes the number **in this file, in the same
change**, with a sentence of why. Ceilings carry ~50–100× headroom over
measured values — they catch regressions in *kind* (an accidental O(n²), a
busy-wait, an asset creep), not percentage drift; `cargo bench` remains the
tool for watching drift.

| Budget | Ceiling | Measured | Enforced by |
| --- | --- | --- | --- |
| Offline catch-up, 1 sim-hour | 250 ms | ~2 ms | `tests/perf.rs` (release) |
| 100k crew ticks × 6 players | 250 ms | ~13 ms | `tests/perf.rs` (release) |
| Convoy delivery voyage (6 replicas, hostile links) | 5,000 ms | ~100 ms | `tests/perf.rs` (release) |
| 1000 save round-trips | 500 ms | ~40 ms | `tests/perf.rs` (release) |
| Black box at cap: serialize / parse / replay 50k entries | 500 / 500 / 1000 ms | ~21 / 16 / 3 ms | `tests/perf.rs` (release) |
| Debug test suite wall time | keep under ~5 s | ~0.5 s | courtesy, not a gate |
| Cabin edit-compile loop (debug) | keep under ~5 s | ~2.7 s | courtesy, not a gate |
| Debug binary | keep under ~1 GB | ~552 MB | courtesy, not a gate |

The two build rows are new and they are not gates, because a build time
measured on a shared runner says more about the runner than the tree. They
are here so a change that puts them back has something to be compared
against. Both came off one pass, on four cores:

- **Debug info.** Cargo's dev default writes full DWARF for every crate,
  and `[profile.dev.package."*"]` overrides only opt-level, so the whole
  graph described every type it had. `debug = "line-tables-only"` took
  the edit-compile loop from 6.9 s to 3.1 s and the binary from 1.74 GiB
  to 619 MiB. A backtrace still names a file and a line; stepping through
  a dependency wants `debug = true` back for that run.
- **Engine features.** Naming what the cabin draws instead of taking
  Bevy's four default umbrellas took the graph from 369 packages to 331,
  a clean build from 698 s to 619 s, and the binary to 552 MB.

**The linker was measured and left alone.** Linking the cabin costs about
1.9 s of the loop, and on this hardware GNU ld does it in 1.86 s against
gold's 2.17 s and lld's 2.01 s. The default is already the quick one, so
an override would buy nothing and cost a build dependency.

Not yet enforced, deliberately: frame time (needs a headless GPU story —
revisit with the live-multiplayer slice). The wasm payload budget retired
with the 2D console's web build (docs/BAY.md); if a web target returns,
so does the budget.

Run locally: `cargo test --release -p space-trucking --test perf -- --ignored`
