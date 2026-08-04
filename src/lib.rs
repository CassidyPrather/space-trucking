//! Space Trucking — an ambient space-freight bartering toy.
//!
//! The split this crate is built around: [`sim`] is the whole game — hauling,
//! docking, bartering, saving — as a pure, deterministic, dependency-light
//! library, and the binary is a thin macroquad shell that turns input into
//! [`sim::InputFrame`]s and sim state into pixels and sound. No macroquad
//! types appear anywhere in here, so the interesting half runs headless in
//! `cargo test` and `cargo bench` at whatever speed the CPU allows.
//!
//! [`net`] extends the same purity to multiplayer: lockstep protocol,
//! session state machines, the guild server, and the hostile-network
//! harness that proves them, all driven by messages and sim time alone.
//!
//! [`replay`] is the black-box flight recorder built on the same property:
//! a session is (base save + input log), so a small text file replays a run
//! bit-identically and a bug report becomes a failing test.
//!
//! [`telemetry`] is the opt-in play-statistics aggregator behind
//! `docs/TELEMETRY.md`: cue-derived counters only, folded into one local,
//! human-readable buffer, gated by the web shell's consent card, and
//! transmitted nowhere.

pub mod net;
pub mod replay;
pub mod sim;
pub mod synth;
pub mod telemetry;

/// `git describe` version, embedded by `build.rs`.
pub const VERSION: &str = env!("GIT_DESCRIBE_VERSION");
