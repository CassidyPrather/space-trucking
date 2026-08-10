//! **Venus** — where the rich went to live, and they are unimaginably
//! tacky (DESIGN.md). The barter row says it plainly: Venus buys tack,
//! and it buys coverings and gilt above everything else.
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks the lore is
//! asking for: a glare a customer cannot look away from, gilt where
//! honest brass would do, a shell dressed in more hardware than it needs.
//! `palette::POI_VENUS` and `palette::accent::VENUS_HALO` are already the
//! chart's idea of the place.

use super::{Character, NEUTRAL};

/// Venus's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
