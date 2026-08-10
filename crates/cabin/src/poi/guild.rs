//! **The Guild Station** — the home station, where every run begins and
//! the room the player sees most (DESIGN.md's Spacing Guild: shady legal
//! areas, shipping contracts, and a massive inexplicable hangar that
//! immediately steals suspicious cargo in front of the usual bartering).
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`].

use super::{Character, NEUTRAL};

/// The Guild's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
