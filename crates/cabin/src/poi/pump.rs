//! **The gas station's pump bay** — an event room, one of a kind. A
//! forecourt: come alongside, top up, cast off, and the fuel reaches the
//! furnace in a crewman's arms the way everything else in this game
//! travels (docs/ROOMS.md).
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: a forecourt is
//! lit like a forecourt, the handshake tops the tanks up rather than
//! striking a deal, and the shell wants a gantry.

use super::{Character, NEUTRAL};

/// The pump bay's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
