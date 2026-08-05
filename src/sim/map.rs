//! The star map and the ship that crawls across it.
//!
//! POIs orbit the sun. Every position is a pure function of the tick —
//! `(tick + phase) % period` reduces in integer space before any float
//! touches it — so saves and replays reconstruct the sky exactly without
//! storing a single coordinate. Travel is integer ticks along a straight
//! intercept line: the course is charted at departure toward where the
//! destination *will* be, and because the lerp target is "the destination's
//! position at the arrival tick", anything that moves the arrival tick (an
//! omen jump, say) re-aims the course automatically. There is no steering;
//! the geometry is the aesthetic.

use super::{TICK_DT, Vec2};

/// Index into [`POIS`].
pub type PoiId = u8;

/// Number of points of interest on the map.
pub const POI_COUNT: usize = 12;

/// The Guild Station, where every run begins.
pub const GUILD: PoiId = 6;

/// Saturn, the planet the outer-ring roster never mentions.
pub const SATURN: PoiId = 7;

/// The Umbra Market, open only while the player's own clock reads night.
pub const UMBRA: PoiId = 8;

/// The Hermitage, tucked in the asteroid belt. Gift economy.
pub const HERMITAGE: PoiId = 9;

/// The comet. Dockable only while it is near the sun, tail lit.
pub const COMET: PoiId = 10;

/// `???`. It is only there when three mysterious crates hum in the hold.
pub const WANDERER: PoiId = 11;

/// The inner-ring planets a direct course cannot be charted to without a
/// transit chit aboard: Venus, Earth, Mars.
pub const INNER_RING: [PoiId; 3] = [0, 1, 2];

/// The sun sits at the centre of the map panel; every orbit is around it.
pub const SUN: Vec2 = Vec2::new(260.0, 220.0);

/// Orbit eccentricity of the comet.
const COMET_ECC: f32 = 0.7;

/// Comet ellipse semi-latus rectum: perihelion ~35, aphelion ~200.
const COMET_LATUS: f32 = 60.0;

/// Fixed rotation of the comet's ellipse, radians.
const COMET_TILT: f32 = 2.2;

/// The comet is visible and dockable while nearer the sun than this — the
/// perihelion dive, tail blazing. About a quarter of its period.
pub const COMET_VISIBLE_R: f32 = 40.0;

/// How a POI moves. Everything derives from the tick; nothing is stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    /// Circular orbit around [`SUN`]: radius, period in ticks, phase in
    /// ticks, and whether it runs the wrong way round (the Guild does;
    /// nobody asks why).
    Circle {
        orbit: f32,
        period: u64,
        phase: u64,
        retrograde: bool,
    },
    /// The comet's eccentric ellipse, one focus at the sun.
    Ellipse { period: u64, phase: u64 },
    /// Parked in space, ignoring gravity. `???` does this.
    Fixed(Vec2),
}

/// A destination on the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poi {
    /// How it moves.
    pub track: Track,
    /// Click target and draw radius.
    pub radius: f32,
}

/// Ticks per minute, for writing orbital periods legibly.
const MIN: u64 = 3600;

/// A circular track: orbit radius, period in minutes, phase in degrees.
const fn circle(orbit: f32, period_min: u64, phase_deg: u64) -> Track {
    Track::Circle {
        orbit,
        period: period_min * MIN,
        phase: period_min * MIN * phase_deg / 360,
        retrograde: false,
    }
}

/// Every POI, in the fixed order the barter value table rows follow.
///
/// Venus, Earth, Mars, Jupiter, Uranus, Neptune, Guild Station, Saturn,
/// Umbra Market, Hermitage, comet, `???`. The first seven keep their
/// historical indices; newcomers append. Orbital speeds are aesthetic —
/// inner planets visibly quicker — but every linear speed stays well under
/// [`SHIP_SPEED`], or intercept courses could never close.
pub const POIS: [Poi; POI_COUNT] = [
    // Venus
    Poi {
        track: circle(50.0, 116, 10),
        radius: 10.0,
    },
    // Earth
    Poi {
        track: circle(72.0, 188, 100),
        radius: 11.0,
    },
    // Mars
    Poi {
        track: circle(94.0, 281, 200),
        radius: 9.0,
    },
    // Jupiter
    Poi {
        track: circle(126.0, 440, 40),
        radius: 18.0,
    },
    // Uranus
    Poi {
        track: circle(172.0, 750, 260),
        radius: 14.0,
    },
    // Neptune
    Poi {
        track: circle(192.0, 914, 330),
        radius: 14.0,
    },
    // Guild Station: between Saturn and Uranus, orbiting the wrong way.
    Poi {
        track: Track::Circle {
            orbit: 160.0,
            period: 559 * MIN,
            phase: 559 * MIN * 300 / 360,
            retrograde: true,
        },
        radius: 9.0,
    },
    // Saturn: the outer ring's unmentioned planet, ringed in salvage.
    Poi {
        track: circle(150.0, 582, 150),
        radius: 16.0,
    },
    // Umbra Market: in Mercury's shadow, open only at night.
    Poi {
        track: circle(30.0, 63, 45),
        radius: 8.0,
    },
    // Hermitage: hollowed rock in the asteroid belt.
    Poi {
        track: circle(110.0, 360, 220),
        radius: 8.0,
    },
    // The comet. The long period keeps its aphelion sweep slower than the
    // ship, which the intercept solver requires.
    Poi {
        track: Track::Ellipse {
            period: 240 * MIN,
            phase: 240 * MIN * 80 / 360,
        },
        radius: 7.0,
    },
    // ???
    Poi {
        track: Track::Fixed(Vec2::new(475.0, 45.0)),
        radius: 12.0,
    },
];

/// Ship speed in world units per second of sim time. Slow on purpose:
/// typical legs between the outer-ring worlds run tens of minutes, per the
/// ambient-game brief. Warp is a developer tool, not a pace.
pub const SHIP_SPEED: f32 = 0.12;

/// Intercept solver iterations. Fixed count, so the arithmetic — and
/// therefore the leg length — is bit-identical everywhere. Generous: the
/// comet at aphelion contracts slowly, and the solve runs once per depart.
const INTERCEPT_STEPS: u32 = 48;

/// Where POI `id` is at `tick`. Pure; the whole sky derives from this.
#[must_use]
pub fn poi_pos(id: PoiId, tick: u64) -> Vec2 {
    match POIS[usize::from(id)].track {
        Track::Circle {
            orbit,
            period,
            phase,
            retrograde,
        } => {
            let angle = track_angle(tick, period, phase, retrograde);
            Vec2::new(
                angle.cos().mul_add(orbit, SUN.x),
                angle.sin().mul_add(orbit, SUN.y),
            )
        }
        Track::Ellipse { period, phase } => {
            let angle = track_angle(tick, period, phase, false);
            let r = COMET_LATUS / COMET_ECC.mul_add(angle.cos(), 1.0);
            Vec2::new(
                (angle + COMET_TILT).cos().mul_add(r, SUN.x),
                (angle + COMET_TILT).sin().mul_add(r, SUN.y),
            )
        }
        Track::Fixed(pos) => pos,
    }
}

/// Orbit angle in radians at `tick`. The modulo happens in u64, so the
/// float only ever sees one period's worth of ticks and precision never
/// decays however long the run.
fn track_angle(tick: u64, period: u64, phase: u64, retrograde: bool) -> f32 {
    let t = (tick + phase) % period.max(1);
    let frac = t as f32 / period.max(1) as f32;
    let angle = frac * std::f32::consts::TAU;
    if retrograde { -angle } else { angle }
}

/// The comet's distance from the sun at `tick`; its tail — and its dock —
/// only exist while this is under [`COMET_VISIBLE_R`].
#[must_use]
pub fn comet_sun_distance(tick: u64) -> f32 {
    (poi_pos(COMET, tick) - SUN).length()
}

/// Whether the comet is currently near enough to visit.
#[must_use]
pub fn comet_visible(tick: u64) -> bool {
    comet_sun_distance(tick) < COMET_VISIBLE_R
}

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

/// The two ends of the current leg's charted line: where the ship cast
/// off, and where the destination will be when it docks.
///
/// Both are derived — the departure tick is `tick - progress` and the
/// arrival tick is `tick - progress + leg_ticks` — so a progress-skipping
/// jump moves the arrival tick earlier and the far endpoint re-aims itself
/// at the planet's true position for the new arrival. The one place this
/// geometry lives.
#[must_use]
pub fn leg_endpoints(
    from: PoiId,
    to: PoiId,
    progress: u64,
    leg_ticks: u64,
    tick: u64,
) -> (Vec2, Vec2) {
    let depart = tick.saturating_sub(progress);
    (poi_pos(from, depart), poi_pos(to, depart + leg_ticks))
}

/// Where the ship sits mid-leg. Shared by the tick function and the save
/// loader so the two can never disagree by a bit.
#[must_use]
pub fn travel_pos(from: PoiId, to: PoiId, progress: u64, leg_ticks: u64, tick: u64) -> Vec2 {
    let (a, b) = leg_endpoints(from, to, progress, leg_ticks, tick);
    a.lerp(b, progress as f32 / leg_ticks.max(1) as f32)
}

/// Whole ticks the intercept leg from `from` to `to` takes when departing
/// at `depart_tick`; never zero, so travel arithmetic stays divisible.
///
/// Fixed-point pursuit: guess a flight time, look up where the target will
/// be then, refine. Every POI's linear speed is below [`SHIP_SPEED`], so
/// the iteration contracts; [`INTERCEPT_STEPS`] rounds land it well inside
/// a tick of the true meet.
#[must_use]
#[allow(clippy::cast_sign_loss)] // clamped non-negative before the cast
pub fn leg_ticks(from: PoiId, to: PoiId, depart_tick: u64) -> u64 {
    let start = poi_pos(from, depart_tick);
    let flight = |ticks: f32| {
        let arrival = depart_tick + ticks.max(0.0) as u64;
        (poi_pos(to, arrival) - start).length() / SHIP_SPEED / TICK_DT
    };
    let mut guess = flight(0.0);
    for _ in 0..INTERCEPT_STEPS {
        guess = flight(guess);
    }
    let ticks = guess.ceil() as i64;
    u64::try_from(ticks).map_or(1, |t| t.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::layout::MAP_PANEL;

    /// Ticks per second, for legibility.
    const SEC: u64 = 60;

    #[test]
    fn every_orbit_stays_inside_the_map_panel() {
        // Sample generously across every period; each POI plus its click
        // radius must stay on the glass at all times.
        for id in 0..POI_COUNT as PoiId {
            let radius = POIS[usize::from(id)].radius;
            for i in 0..720_u64 {
                let tick = i * 7207; // coprime stride, ~14h of sky
                let p = poi_pos(id, tick);
                assert!(
                    p.x - radius >= MAP_PANEL.x
                        && p.x + radius <= MAP_PANEL.x + MAP_PANEL.w
                        && p.y - radius >= MAP_PANEL.y
                        && p.y + radius <= MAP_PANEL.y + MAP_PANEL.h,
                    "POI {id} leaves the panel at tick {tick}: {p:?}"
                );
            }
        }
    }

    #[test]
    fn positions_are_periodic_and_pure() {
        for id in 0..POI_COUNT as PoiId {
            let (period, fixed) = match POIS[usize::from(id)].track {
                Track::Circle { period, .. } | Track::Ellipse { period, .. } => (period, false),
                Track::Fixed(_) => (1, true),
            };
            let a = poi_pos(id, 12_345);
            let b = poi_pos(id, 12_345 + period);
            assert_eq!(a, b, "POI {id} failed to repeat after its period");
            if !fixed {
                let later = poi_pos(id, 12_345 + period / 2);
                assert_ne!(a, later, "POI {id} never moves");
            }
        }
    }

    #[test]
    fn every_orbit_is_slower_than_the_ship() {
        // The intercept solver's contraction condition: no POI may outrun
        // the freighter, or a charted course could chase forever. Sampled
        // across each POI's whole period, aphelion sweeps included.
        for id in 0..POI_COUNT as PoiId {
            let period = match POIS[usize::from(id)].track {
                Track::Circle { period, .. } | Track::Ellipse { period, .. } => period,
                Track::Fixed(_) => continue,
            };
            let step = (period / 2000).max(SEC);
            let mut worst: f32 = 0.0;
            let mut t = 0;
            while t < period + step {
                let moved = (poi_pos(id, t + step) - poi_pos(id, t)).length();
                worst = worst.max(moved / (step as f32 * TICK_DT));
                t += step;
            }
            assert!(
                worst < SHIP_SPEED * 0.85,
                "POI {id} moves at {worst} u/s against ship speed {SHIP_SPEED}"
            );
        }
    }

    #[test]
    fn intercept_courses_actually_meet_the_target() {
        // Chart from everywhere to everywhere at scattered departure times;
        // the planet at the predicted arrival tick must sit exactly
        // leg-length away from the departure point (that is the meet).
        for from in 0..POI_COUNT as PoiId {
            for to in 0..POI_COUNT as PoiId {
                if from == to {
                    continue;
                }
                for depart in [0_u64, 90_000, 1_234_567] {
                    let leg = leg_ticks(from, to, depart);
                    let start = poi_pos(from, depart);
                    let meet = poi_pos(to, depart + leg);
                    let flown = SHIP_SPEED * TICK_DT * leg as f32;
                    let short = (meet - start).length() - flown;
                    assert!(
                        short.abs() < SHIP_SPEED * TICK_DT * 90.0,
                        "{from}->{to} at {depart}: leg {leg} misses by {short}"
                    );
                }
            }
        }
    }

    #[test]
    fn travel_pos_starts_at_the_departure_point_and_ends_on_the_dock() {
        let depart = 5_000_u64;
        let leg = leg_ticks(GUILD, SATURN, depart);
        let start = travel_pos(GUILD, SATURN, 0, leg, depart);
        assert_eq!(start, poi_pos(GUILD, depart));
        let end = travel_pos(GUILD, SATURN, leg, leg, depart + leg);
        assert_eq!(end, poi_pos(SATURN, depart + leg));
    }

    #[test]
    fn the_comet_comes_and_goes() {
        let Track::Ellipse { period, .. } = POIS[usize::from(COMET)].track else {
            unreachable!("the comet lost its ellipse")
        };
        let mut seen_visible = false;
        let mut seen_hidden = false;
        for i in 0..360_u64 {
            let tick = i * period / 360;
            if comet_visible(tick) {
                seen_visible = true;
            } else {
                seen_hidden = true;
            }
        }
        assert!(seen_visible, "the comet never approaches");
        assert!(seen_hidden, "the comet never leaves");
    }

    #[test]
    fn starting_planet_journeys_land_in_the_ambient_band() {
        // The brief: un-warped journeys from the starting dock (the Guild)
        // to the outer-ring worlds should run on the order of ten minutes
        // to under an hour. Conjunction legs may dip shorter; nothing may
        // run absurdly long.
        for to in [3, SATURN, 4, 5] {
            for depart in [0_u64, 400_000, 3_000_000] {
                let leg = leg_ticks(GUILD, to, depart);
                let minutes = leg as f32 / (60.0 * 60.0);
                assert!(
                    (2.0..=70.0).contains(&minutes),
                    "Guild->{to} at {depart} takes {minutes} min"
                );
            }
        }
    }
}
