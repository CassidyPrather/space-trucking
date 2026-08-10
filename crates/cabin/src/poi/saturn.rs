//! **Saturn** — the planet the outer-ring roster never mentions, and the
//! ring-barons like it that way (DESIGN.md). The rings are a debris field
//! of a thousand failed hauling companies; salvage is the whole economy,
//! scrap trades like treasure, and every docking clamp on the station was
//! pulled off a repossessed freighter — including, possibly, yours.
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. Hooks: nothing in the
//! room matching anything else in it, a shell visibly assembled from four
//! other people's hulls, and the yard-blue frame Saturn paints round
//! everything it cuts (`Kind::BayWindow`'s hue is that blue, and the bay
//! pane comes out of somebody's ring).

use super::{Character, NEUTRAL};

/// Saturn's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
