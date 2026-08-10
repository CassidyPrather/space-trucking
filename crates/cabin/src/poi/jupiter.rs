//! **Jupiter** — the outer ring's frontier, where matter extraction and
//! processing went once the inner ring was exhausted (DESIGN.md).
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: industry at
//! the wrong scale, a refinery's flare stack on the shell, and a room
//! that is a working floor rather than a shop.

use super::{Character, NEUTRAL};

/// Jupiter's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
