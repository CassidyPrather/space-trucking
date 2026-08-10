//! **A derelict's hold** — an event room, and one of a kind by nature, so
//! its kind is its identity. A derelict has exactly one seam worth
//! trusting: the one you just mated. The rest of the hull is vacuum with
//! edges (docs/ROOMS.md).
//!
//! It is **not** neutral, and it never was: a derelict burns nothing at
//! all, and that is its whole tell — dark plate, no running lights, and
//! the only reason you can see it is that your own hull is lit. The rest
//! of the character is still the neutral room, and the hooks are obvious:
//! salvage rather than stock, no working pendant, a hull that stopped
//! being maintained a long time ago.

use super::{Character, NEUTRAL, Outfit};
use crate::palette;

/// The derelict's own room.
pub const CHARACTER: Character = Character {
    outfit: Outfit {
        plate: palette::PLATE_SHADE,
        lamp: palette::SHADOW,
        // A derelict burns nothing at all. That is the whole tell, and
        // `viewport::every_room_kind_is_dressed_for_the_void` holds a
        // future design agent to it.
        lamps: 0,
    },
    ..NEUTRAL
};
