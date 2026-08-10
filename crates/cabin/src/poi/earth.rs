//! **Earth** — a dystopia of some sort; pick a creative one (DESIGN.md).
//! The economy already picked one edge of it: Earth **rations light**,
//! and it is practical about furniture.
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: a pendant that
//! is metered rather than generous, an outside seen through its own smog
//! (`palette::accent::SMOG`), and — the owner's standing idea for a
//! planet-side POI — a space elevator's ribbon running off the shell
//! toward the world below.

use super::{Character, NEUTRAL};

/// Earth's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
