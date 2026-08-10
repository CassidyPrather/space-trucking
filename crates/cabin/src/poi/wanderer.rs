//! **`???`** — only there when three mysterious crates hum in a hold at
//! once. It trades three for one bigger one. The Guild counts the bigger
//! one four times. Nobody explains the arithmetic, least of all Cor
//! (DESIGN.md).
//!
//! It brings a trade room and **stocks nothing**: the room comes
//! alongside empty, and the only thing in it is whatever you carried in.
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: a room that is
//! not quite a room, `palette::EERIE_BRIGHT`, and the good sense to
//! explain nothing.

use super::{Character, NEUTRAL};

/// `???`'s own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
