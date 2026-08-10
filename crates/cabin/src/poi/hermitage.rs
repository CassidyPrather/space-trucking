//! **The Hermitage** — a hollowed rock in the asteroid belt. The hermits
//! do not trade with strangers; they remember gifts, forever, and shelves
//! slowly grow things for people who gave first (DESIGN.md). **Nobody has
//! seen more than one lit window.**
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: the one lit
//! window is an exterior fitting and a low pendant, not a bright room;
//! the handshake is a **bell** (docs/ROOMS.md names it); and rock, not
//! plate, wherever a station would normally put metal.

use super::{Character, NEUTRAL};

/// The Hermitage's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
