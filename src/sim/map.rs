//! The star map and the ship that crawls across it.
//!
//! POIs are fixed points in map-panel world coordinates; the sim only knows
//! positions and radii, and which glyph a POI wears is the renderer's
//! business. Travel is integer ticks along a straight line, which keeps it
//! exactly reproducible and trivially serialisable.

use super::{TICK_DT, Vec2};

/// Index into [`POIS`].
pub type PoiId = u8;

/// Number of points of interest on the map.
pub const POI_COUNT: usize = 7;

/// The Guild Station, where every run begins.
pub const GUILD: PoiId = 6;

/// A destination on the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poi {
    /// Position in map-panel world coordinates.
    pub pos: Vec2,
    /// Click target and draw radius.
    pub radius: f32,
}

/// Every POI, in the fixed order the barter value table rows follow:
/// Venus, Earth, Mars, Jupiter, Uranus, Neptune, Guild Station.
pub const POIS: [Poi; POI_COUNT] = [
    Poi {
        pos: Vec2::new(195.0, 165.0),
        radius: 10.0,
    },
    Poi {
        pos: Vec2::new(330.0, 190.0),
        radius: 11.0,
    },
    Poi {
        pos: Vec2::new(280.0, 305.0),
        radius: 9.0,
    },
    Poi {
        pos: Vec2::new(455.0, 90.0),
        radius: 18.0,
    },
    Poi {
        pos: Vec2::new(75.0, 80.0),
        radius: 14.0,
    },
    Poi {
        pos: Vec2::new(445.0, 375.0),
        radius: 14.0,
    },
    Poi {
        pos: Vec2::new(60.0, 370.0),
        radius: 9.0,
    },
];

/// Ship speed in world units per second of sim time.
pub const SHIP_SPEED: f32 = 4.0;

/// Where the ship is in its docked/traveling cycle. Travel is measured in
/// whole ticks so a leg replays and saves exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipState {
    Docked(PoiId),
    Traveling {
        from: PoiId,
        to: PoiId,
        /// Ticks elapsed on this leg.
        progress: u64,
        /// Ticks the whole leg takes.
        leg_ticks: u64,
    },
}

/// The freighter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ship {
    pub pos: Vec2,
    /// Last tick's position, for render interpolation.
    pub prev_pos: Vec2,
    pub state: ShipState,
    /// Destination picked on the map, if any.
    pub selected: Option<PoiId>,
}

impl Ship {
    /// Position to draw at, blending `prev_pos` and `pos` by `Sim::alpha`.
    #[must_use]
    pub fn interpolated(&self, alpha: f32) -> Vec2 {
        self.prev_pos.lerp(self.pos, alpha)
    }
}

/// Whole ticks the straight-line leg between two POIs takes; never zero, so
/// travel arithmetic stays divisible.
#[must_use]
pub fn leg_ticks(from: PoiId, to: PoiId) -> u64 {
    let distance = (POIS[usize::from(to)].pos - POIS[usize::from(from)].pos).length();
    let ticks = (distance / SHIP_SPEED / TICK_DT).ceil() as i64;
    u64::try_from(ticks).map_or(1, |t| t.max(1))
}
