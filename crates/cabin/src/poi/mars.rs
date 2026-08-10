//! **Mars** — broke off in a rebellion and is now a scrappy republic
//! (DESIGN.md). Rust country: the barter row has Mars paying best of
//! anyone for enamel.
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: field repairs
//! showing, hardware that does not match itself, a shell patched in
//! `palette::POI_MARS` over somebody else's plate.

use super::{Character, NEUTRAL};

/// Mars's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
