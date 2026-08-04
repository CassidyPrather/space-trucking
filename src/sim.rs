//! The whole simulation: pure, deterministic, and macroquad-free.
//!
//! The frontend's only channel into the sim is an [`InputFrame`], and its only
//! channel out is [`Sim::particles`] plus [`Sim::alpha`]. That split is the
//! point of the template — the interesting half is a plain library you can
//! unit-test at full speed, and the renderer is a thin shell around it.
//!
//! Time is fixed-step with an accumulator (the classic "Fix Your Timestep"
//! shape): [`Sim::advance`] runs whole [`TICK_DT`] steps and leaves the
//! leftover fraction in [`Sim::alpha`] for the renderer to interpolate with.
//!
//! Sound follows the same rule as pixels: the sim reports that something
//! happened and how hard, as a [`Cue`], and the frontend decides what that
//! sounds like. No audio types cross into this module either.

use std::ops::{Add, AddAssign, Mul, MulAssign, Sub};

/// Length of one simulation step. Ticks are always exactly this long.
pub const TICK_DT: f32 = 1.0 / 60.0;

/// Logical world width. The renderer scales this onto the window, so the sim
/// never learns what a pixel is.
pub const WORLD_W: f32 = 800.0;

/// Logical world height.
pub const WORLD_H: f32 = 600.0;

/// Particles created per seed.
pub const PARTICLE_COUNT: usize = 600;

/// Longest frame the accumulator will bank. A backgrounded tab or a debugger
/// pause hands us an enormous `frame_dt`; without this cap the sim would try
/// to catch up all at once and spiral.
const MAX_FRAME_DT: f32 = 0.25;

/// Fraction of velocity kept per tick.
const DAMPING: f32 = 0.97;

/// Numerator of the inverse-linear attraction toward the pointer.
const ATTRACT_STRENGTH: f32 = 60_000.0;

/// Distance floor for attraction and bursts, so the force stays finite when a
/// particle sits exactly under the pointer.
const MIN_DIST: f32 = 24.0;

/// Spring constant easing idle particles back toward their spawn point.
const HOME_SPRING: f32 = 4.0;

/// Fraction of speed kept after hitting a wall.
const RESTITUTION: f32 = 0.6;

/// Bursts fall off linearly and reach no further than this.
const BURST_RADIUS: f32 = 260.0;

/// Burst impulse at the burst origin, before falloff.
const BURST_IMPULSE: f32 = 900.0;

/// Per-axis spawn speed range.
const SPAWN_SPEED: f32 = 40.0;

/// Smallest spawned particle radius, in world units.
const RADIUS_MIN: f32 = 1.5;

/// Spread of spawned particle radii above [`RADIUS_MIN`].
const RADIUS_SPAN: f32 = 2.5;

/// Speed a particle must carry into a wall for the hit to count as audible.
/// Particles pinned against a boundary by the pointer graze it every tick at
/// almost no speed; those are not impacts and must not be heard.
const IMPACT_SPEED: f32 = 120.0;

/// Simultaneous audible wall hits that saturate [`Cue::Impact`].
const IMPACT_FULL: f32 = 80.0;

/// Quiet period after an impact cue, in seconds of sim time. Six hundred
/// particles raining on a wall would otherwise cue every single tick.
const IMPACT_COOLDOWN: f32 = 0.05;

/// A 2D vector, kept deliberately tiny: the sim needs four operations and a
/// length, and pulling in a math crate for that would be silly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Construct a vector.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean length.
    #[must_use]
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Linear interpolation toward `other`, `t` clamped to `0..=1`.
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            (other.x - self.x).mul_add(t, self.x),
            (other.y - self.y).mul_add(t, self.y),
        )
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

/// One particle. `prev_pos` is last tick's position, kept so the renderer can
/// interpolate instead of stuttering when the frame rate and tick rate differ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub pos: Vec2,
    pub prev_pos: Vec2,
    pub vel: Vec2,
    /// Spawn point; idle particles drift back to it.
    pub home: Vec2,
    /// World-space radius.
    pub radius: f32,
    /// Hue in `0..1`, for the renderer to colour with.
    pub hue: f32,
}

impl Particle {
    /// Position to draw at, blending `prev_pos` and `pos` by [`Sim::alpha`].
    #[must_use]
    pub fn interpolated(&self, alpha: f32) -> Vec2 {
        self.prev_pos.lerp(self.pos, alpha)
    }
}

/// Everything the sim is allowed to know about the outside world for one
/// frame. The sim never reads global input state, which is what makes replay
/// and testing possible.
#[derive(Clone, Copy, Debug, Default)]
pub struct InputFrame {
    /// Pointer position in world coordinates.
    pub pointer: Vec2,
    /// Pull particles toward `pointer` for as long as this holds.
    pub attract: bool,
    /// One-shot radial shove at `pointer`, applied once per frame regardless
    /// of how many ticks the frame turns into.
    pub burst: bool,
    /// Edge-triggered by the caller: flips the pause flag.
    pub toggle_pause: bool,
    /// Restart with a new seed.
    pub reseed: Option<u64>,
}

/// Something that happened and is worth hearing. The sim says what and how
/// hard; choosing a waveform is the frontend's job.
///
/// Intensities are `0..=1` so the frontend can map them to gain without
/// knowing any of the sim's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// A burst moved particles. Intensity is the share of the world it shoved.
    Burst { intensity: f32 },
    /// Particles struck a wall together. Intensity rises with how many.
    Impact { intensity: f32 },
    /// Pause was toggled. `paused` is the state just entered.
    Pause { paused: bool },
    /// The world was replaced.
    Reseed,
}

/// The simulation. Same seed plus same [`InputFrame`] sequence gives
/// bit-identical states on every run.
#[derive(Clone, Debug)]
pub struct Sim {
    seed: u64,
    rng: fastrand::Rng,
    particles: Vec<Particle>,
    accumulator: f32,
    paused: bool,
    cues: Vec<Cue>,
    /// Sim-time left before another [`Cue::Impact`] may fire.
    impact_cooldown: f32,
}

impl Sim {
    /// Build a sim from a seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let particles = (0..PARTICLE_COUNT).map(|_| spawn(&mut rng)).collect();
        Self {
            seed,
            rng,
            particles,
            accumulator: 0.0,
            paused: false,
            cues: Vec::new(),
            impact_cooldown: 0.0,
        }
    }

    /// Seed this sim was built from.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Whether ticking is currently suspended.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Current particle state.
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// How far the leftover accumulator has carried us into the next tick, in
    /// `0..1`. Feed this to [`Particle::interpolated`].
    #[must_use]
    pub const fn alpha(&self) -> f32 {
        self.accumulator / TICK_DT
    }

    /// What the most recent [`Sim::advance`] produced worth hearing. Valid
    /// until the next call, which clears it.
    #[must_use]
    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }

    /// Throw away all state and start over from `seed`.
    pub fn reseed(&mut self, seed: u64) {
        let paused = self.paused;
        *self = Self::new(seed);
        self.paused = paused;
    }

    /// Consume one frame's worth of real time, returning how many fixed ticks
    /// ran. `frame_dt` is clamped to [`MAX_FRAME_DT`].
    pub fn advance(&mut self, frame_dt: f32, input: &InputFrame) -> u32 {
        // Cues describe this frame only; last frame's have been consumed.
        self.cues.clear();

        if let Some(seed) = input.reseed {
            // Note the ordering: reseeding rebuilds `self`, cue list included,
            // so the cue has to be pushed after it rather than before.
            self.reseed(seed);
            self.cues.push(Cue::Reseed);
        }
        if input.toggle_pause {
            self.paused = !self.paused;
            self.cues.push(Cue::Pause {
                paused: self.paused,
            });
        }
        if self.paused {
            return 0;
        }

        // Impulses are per-frame events, not per-tick ones: applying a burst
        // inside the loop would multiply it by the frame's tick count.
        if input.burst {
            let intensity = self.burst(input.pointer);
            if intensity > 0.0 {
                self.cues.push(Cue::Burst { intensity });
            }
        }

        self.accumulator += frame_dt.clamp(0.0, MAX_FRAME_DT);
        let mut ticks = 0;
        // The float condition is the fixed-timestep idiom; the clamp above
        // bounds the loop at MAX_FRAME_DT / TICK_DT iterations.
        #[allow(clippy::while_float)]
        while self.accumulator >= TICK_DT {
            self.accumulator -= TICK_DT;
            self.tick(input);
            ticks += 1;
        }
        ticks
    }

    /// One fixed step.
    fn tick(&mut self, input: &InputFrame) {
        let mut hits = 0_usize;
        for particle in &mut self.particles {
            particle.prev_pos = particle.pos;

            let accel = if input.attract {
                let offset = input.pointer - particle.pos;
                let dist = offset.length().max(MIN_DIST);
                // Inverse-linear falloff: a firm pull up close without the
                // singularity a true inverse-square would have.
                offset * (ATTRACT_STRENGTH / (dist * dist))
            } else {
                (particle.home - particle.pos) * HOME_SPRING
            };

            particle.vel += accel * TICK_DT;
            particle.vel *= DAMPING;
            particle.pos += particle.vel * TICK_DT;

            let struck = bounce(&mut particle.pos.x, &mut particle.vel.x, WORLD_W)
                + bounce(&mut particle.pos.y, &mut particle.vel.y, WORLD_H);
            if struck >= IMPACT_SPEED {
                hits += 1;
            }
        }

        self.impact_cooldown = (self.impact_cooldown - TICK_DT).max(0.0);
        if hits > 0 && self.impact_cooldown <= 0.0 {
            self.impact_cooldown = IMPACT_COOLDOWN;
            self.cues.push(Cue::Impact {
                intensity: (hits as f32 / IMPACT_FULL).min(1.0),
            });
        }
    }

    /// Radial shove centred on `at`, fading to nothing at [`BURST_RADIUS`].
    /// Returns the share of the world it moved, in `0..=1`.
    fn burst(&mut self, at: Vec2) -> f32 {
        let mut moved = 0_usize;
        for particle in &mut self.particles {
            let offset = particle.pos - at;
            let dist = offset.length().max(MIN_DIST);
            if dist >= BURST_RADIUS {
                continue;
            }
            let falloff = 1.0 - dist / BURST_RADIUS;
            // A little per-particle jitter so repeat clicks never look canned.
            let jitter = self.rng.f32().mul_add(0.4, 0.8);
            particle.vel += offset * (BURST_IMPULSE * falloff * jitter / dist);
            moved += 1;
        }
        moved as f32 / PARTICLE_COUNT as f32
    }
}

/// Clamp one axis into `0..=limit`, reflecting velocity on contact. Returns
/// the speed the wall absorbed, or zero if the particle never reached it.
fn bounce(pos: &mut f32, vel: &mut f32, limit: f32) -> f32 {
    if *pos < 0.0 {
        *pos = 0.0;
    } else if *pos > limit {
        *pos = limit;
    } else {
        return 0.0;
    }
    let speed = vel.abs();
    *vel = -*vel * RESTITUTION;
    speed
}

/// Draw one particle's worth of randomness. Call order matters for
/// determinism, so this stays a single function.
fn spawn(rng: &mut fastrand::Rng) -> Particle {
    let pos = Vec2::new(rng.f32() * WORLD_W, rng.f32() * WORLD_H);
    Particle {
        pos,
        prev_pos: pos,
        vel: Vec2::new(
            rng.f32().mul_add(2.0, -1.0) * SPAWN_SPEED,
            rng.f32().mul_add(2.0, -1.0) * SPAWN_SPEED,
        ),
        home: pos,
        radius: rng.f32().mul_add(RADIUS_SPAN, RADIUS_MIN),
        hue: rng.f32(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attract_at(x: f32, y: f32) -> InputFrame {
        InputFrame {
            pointer: Vec2::new(x, y),
            attract: true,
            ..InputFrame::default()
        }
    }

    fn positions(sim: &Sim) -> Vec<Vec2> {
        sim.particles().iter().map(|p| p.pos).collect()
    }

    #[test]
    fn same_seed_and_inputs_are_bit_identical() {
        let input = attract_at(WORLD_W * 0.25, WORLD_H * 0.75);
        let mut a = Sim::new(0xDEAD_BEEF);
        let mut b = Sim::new(0xDEAD_BEEF);
        for _ in 0..120 {
            a.advance(TICK_DT, &input);
            b.advance(TICK_DT, &input);
        }
        assert_eq!(positions(&a), positions(&b));
    }

    #[test]
    fn different_seeds_diverge() {
        assert_ne!(positions(&Sim::new(1)), positions(&Sim::new(2)));
    }

    #[test]
    fn accumulator_runs_whole_ticks_and_carries_the_remainder() {
        let mut sim = Sim::new(1);
        assert_eq!(sim.advance(TICK_DT * 2.5, &InputFrame::default()), 2);
        assert!(
            (sim.alpha() - 0.5).abs() < 1e-3,
            "alpha was {}",
            sim.alpha()
        );
    }

    #[test]
    fn alpha_stays_in_unit_range() {
        let mut sim = Sim::new(1);
        // Deliberately awkward frame times, none of them a tick multiple
        for step in 1_u16..200 {
            sim.advance(0.0007 * f32::from(step), &InputFrame::default());
            let alpha = sim.alpha();
            assert!((0.0..1.0).contains(&alpha), "alpha was {alpha}");
        }
    }

    #[test]
    fn long_frame_dt_is_clamped() {
        let mut sim = Sim::new(1);
        let ticks = sim.advance(10.0, &InputFrame::default());
        // 10 s of real time is 600 ticks; the clamp caps one frame at 0.25 s
        assert!(ticks <= 15, "clamped frame ran {ticks} ticks");
        assert!(ticks >= 14, "clamped frame ran only {ticks} ticks");
    }

    #[test]
    fn pause_freezes_state() {
        let mut sim = Sim::new(7);
        let toggle = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        assert_eq!(sim.advance(TICK_DT, &toggle), 0);
        assert!(sim.is_paused());

        let before = sim.particles().to_vec();
        let busy = attract_at(0.0, 0.0);
        assert_eq!(sim.advance(TICK_DT * 10.0, &busy), 0);
        assert_eq!(sim.particles(), before.as_slice());
    }

    #[test]
    fn burst_changes_velocities() {
        let mut sim = Sim::new(3);
        let idle = InputFrame::default();
        for _ in 0..60 {
            sim.advance(TICK_DT, &idle);
        }
        let before: Vec<Vec2> = sim.particles().iter().map(|p| p.vel).collect();

        // Zero dt runs no ticks, so any change is the burst's doing
        sim.advance(
            0.0,
            &InputFrame {
                pointer: Vec2::new(WORLD_W * 0.5, WORLD_H * 0.5),
                burst: true,
                ..idle
            },
        );
        let after: Vec<Vec2> = sim.particles().iter().map(|p| p.vel).collect();
        assert!(before.iter().zip(&after).any(|(a, b)| a != b));
    }

    #[test]
    fn particles_stay_in_bounds() {
        let mut sim = Sim::new(11);
        // Pointer parked well outside the world, dragging everything at a wall
        let input = attract_at(WORLD_W * 3.0, -WORLD_H * 2.0);
        for _ in 0..600 {
            sim.advance(TICK_DT, &input);
        }
        for particle in sim.particles() {
            assert!(
                (0.0..=WORLD_W).contains(&particle.pos.x),
                "x escaped: {}",
                particle.pos.x
            );
            assert!(
                (0.0..=WORLD_H).contains(&particle.pos.y),
                "y escaped: {}",
                particle.pos.y
            );
        }
    }

    #[test]
    fn reseed_replaces_the_world() {
        let mut sim = Sim::new(5);
        let before = positions(&sim);
        sim.advance(
            TICK_DT,
            &InputFrame {
                reseed: Some(6),
                ..InputFrame::default()
            },
        );
        assert_eq!(sim.seed(), 6);
        assert_ne!(positions(&sim), before);
    }

    #[test]
    fn burst_cues_report_what_it_moved() {
        let mut sim = Sim::new(3);
        // Zero dt runs no ticks, so the burst cue is the only one possible
        sim.advance(
            0.0,
            &InputFrame {
                pointer: Vec2::new(WORLD_W * 0.5, WORLD_H * 0.5),
                burst: true,
                ..InputFrame::default()
            },
        );
        let [Cue::Burst { intensity }] = sim.cues() else {
            panic!("expected one burst cue, got {:?}", sim.cues());
        };
        assert!(
            (0.0..=1.0).contains(intensity),
            "intensity {intensity} is outside the contract"
        );
        assert!(*intensity > 0.0, "a burst mid-world should move something");
    }

    #[test]
    fn pause_and_reseed_announce_themselves() {
        let mut sim = Sim::new(7);
        let toggle = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };

        sim.advance(TICK_DT, &toggle);
        assert_eq!(sim.cues(), [Cue::Pause { paused: true }]);
        sim.advance(TICK_DT, &toggle);
        assert_eq!(sim.cues(), [Cue::Pause { paused: false }]);

        sim.advance(
            TICK_DT,
            &InputFrame {
                reseed: Some(6),
                ..InputFrame::default()
            },
        );
        assert_eq!(sim.cues(), [Cue::Reseed]);
    }

    #[test]
    fn cues_do_not_outlive_the_frame_that_made_them() {
        let mut sim = Sim::new(7);
        sim.advance(
            TICK_DT,
            &InputFrame {
                reseed: Some(6),
                ..InputFrame::default()
            },
        );
        assert!(!sim.cues().is_empty());

        sim.advance(TICK_DT, &InputFrame::default());
        assert!(sim.cues().is_empty(), "stale cues: {:?}", sim.cues());
    }

    #[test]
    fn impact_cues_are_rate_limited() {
        const TICKS: usize = 300;

        let mut sim = Sim::new(11);
        // Pointer just outside a wall: close enough for the inverse-linear
        // pull to really fling particles at it, which is the worst case.
        let input = attract_at(WORLD_W + 10.0, WORLD_H * 0.5);

        let mut impacts = 0;
        for _ in 0..TICKS {
            sim.advance(TICK_DT, &input);
            impacts += sim
                .cues()
                .iter()
                .filter(|cue| matches!(cue, Cue::Impact { .. }))
                .count();
        }

        assert!(impacts > 0, "600 particles hit a wall and made no sound");
        // Without the cooldown this would be one cue per tick, and 600
        // particles raining on a wall would sound like a dial-up modem.
        let seconds = TICK_DT * TICKS as f32;
        let ceiling = seconds / IMPACT_COOLDOWN;
        assert!(
            impacts as f32 <= ceiling,
            "{impacts} impact cues in {seconds}s, over the {ceiling} the cooldown allows"
        );
    }

    #[test]
    fn interpolation_walks_from_prev_to_current() {
        let mut sim = Sim::new(9);
        sim.advance(TICK_DT * 4.0, &attract_at(WORLD_W * 0.5, WORLD_H * 0.5));
        let particle = &sim.particles()[0];
        assert_eq!(particle.interpolated(0.0), particle.prev_pos);
        assert_eq!(particle.interpolated(1.0), particle.pos);
    }
}
