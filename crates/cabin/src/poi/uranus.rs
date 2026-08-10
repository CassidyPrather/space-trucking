//! **Uranus** — outer ring, and the chart draws it with its rings
//! (`palette::accent::URANUS_RING`).
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example.

use super::{Character, NEUTRAL};

/// Uranus's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
