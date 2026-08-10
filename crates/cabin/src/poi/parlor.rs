//! **The casino's parlor** — an event room, one of a kind, whose whole
//! conceit is that it has no visible doors. Stake cargo on its offer area
//! and work the handshake: double, or a commemorative chip (docs/ROOMS.md).
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: the doorway
//! you came in by should be the only thing in the room that admits to
//! being one, the handshake is the wheel, and the light is the light of a
//! place with no clocks.

use super::{Character, NEUTRAL};

/// The parlor's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
