//! Game/toy template crate.
//!
//! The split this template exists to demonstrate: [`sim`] is the whole
//! simulation as a pure, deterministic, dependency-light library, and the
//! binary is a thin macroquad shell that turns input into
//! [`sim::InputFrame`]s and particles into circles. No macroquad types appear
//! anywhere in here, so the interesting half runs headless in `cargo test` and
//! `cargo bench` at whatever speed the CPU allows.

pub mod sim;
pub mod synth;

/// `git describe` version, embedded by `build.rs`.
pub const VERSION: &str = env!("GIT_DESCRIBE_VERSION");
