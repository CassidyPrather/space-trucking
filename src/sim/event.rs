//! Travel-time events: the suspicious-cargo jump, and ambient hull creaks.
//!
//! Departing with a suspicious crate aboard schedules an omen somewhere in
//! the middle half of the leg. The omen dims the console light and swells the
//! hum, the jump skips most of the remaining distance, and then everything
//! eases back to normal. All timing derives from `splitmix(seed, leg)`, so
//! the whole episode replays and saves exactly.

use super::{Cue, TICK_DT, splitmix, step_toward};

/// Omen duration before the jump fires: 3 s.
const OMEN_LEAD_TICKS: u32 = 180;

/// Wake duration after the jump before the omen ends: 1.5 s.
const OMEN_TAIL_TICKS: u32 = 90;

/// Console light while an omen is on.
const LIGHT_DIM: f32 = 0.2;

/// Light easing rate: full-to-dim over 2 s.
const LIGHT_RATE: f32 = (1.0 - LIGHT_DIM) / 2.0;

/// Omen swell rate: silence-to-full over 2 s.
const OMEN_RATE: f32 = 0.5;

/// Fraction of the remaining leg a jump skips, as a ratio.
const JUMP_SKIP_NUM: u64 = 3;
const JUMP_SKIP_DEN: u64 = 4;

/// Ticks per creak window: roughly one creak every 8 s of travel.
const CREAK_WINDOW: u64 = 480;

/// Salt so creak hashes never collide with visit or leg hashes.
const CREAK_SALT: u64 = 0xC4EA_4B00;

/// Where the current leg's event is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// Dimming and swelling, counting down to the jump.
    Omen {
        elapsed: u32,
    },
    /// Jump fired; easing back toward normal.
    Wake {
        elapsed: u32,
    },
}

/// Per-leg event state plus the eased ambience values it drives.
#[derive(Clone, Debug)]
pub struct Events {
    /// Departures so far, salting each leg's schedule hash.
    pub leg_counter: u64,
    /// Progress tick the omen starts at, if this leg carries one.
    pub jump_at: Option<u64>,
    pub phase: Phase,
    /// Console light, `0..=1`, eased every tick.
    pub light: f32,
    /// Omen intensity for the hum swell, `0..=1`, eased every tick.
    pub omen: f32,
}

impl Events {
    pub const fn new() -> Self {
        Self {
            leg_counter: 0,
            jump_at: None,
            phase: Phase::Idle,
            light: 1.0,
            omen: 0.0,
        }
    }

    /// Called once per departure: count the leg and, if a suspicious crate
    /// is aboard, schedule an omen in `[leg_ticks / 2, leg_ticks * 3 / 4)`.
    pub fn on_depart(&mut self, seed: u64, leg_ticks: u64, suspicious: bool) {
        self.leg_counter += 1;
        self.jump_at = suspicious.then(|| {
            let lo = leg_ticks / 2;
            let hi = leg_ticks * 3 / 4;
            lo + splitmix(seed, self.leg_counter) % (hi - lo).max(1)
        });
    }

    /// One travel tick: walk the omen phases, skipping `progress` ahead when
    /// the jump fires.
    pub fn travel_tick(&mut self, progress: &mut u64, leg_ticks: u64, cues: &mut Vec<Cue>) {
        match self.phase {
            Phase::Idle => {
                if self.jump_at.is_some_and(|at| *progress >= at) {
                    self.phase = Phase::Omen { elapsed: 0 };
                    cues.push(Cue::OmenStart);
                }
            }
            Phase::Omen { elapsed } => {
                if elapsed + 1 >= OMEN_LEAD_TICKS {
                    *progress += (leg_ticks - *progress) * JUMP_SKIP_NUM / JUMP_SKIP_DEN;
                    self.phase = Phase::Wake { elapsed: 0 };
                    cues.push(Cue::Jump);
                } else {
                    self.phase = Phase::Omen {
                        elapsed: elapsed + 1,
                    };
                }
            }
            Phase::Wake { elapsed } => {
                if elapsed + 1 >= OMEN_TAIL_TICKS {
                    self.phase = Phase::Idle;
                    self.jump_at = None;
                    cues.push(Cue::OmenEnd);
                } else {
                    self.phase = Phase::Wake {
                        elapsed: elapsed + 1,
                    };
                }
            }
        }
    }

    /// Docked before the episode finished (a jump can land nearly on the
    /// pad): close it out so the omen cues always pair up.
    pub fn on_arrive(&mut self, cues: &mut Vec<Cue>) {
        if self.phase != Phase::Idle {
            self.phase = Phase::Idle;
            cues.push(Cue::OmenEnd);
        }
        self.jump_at = None;
    }

    /// Every tick, traveling or docked: ease light and omen toward what the
    /// current phase asks for.
    pub fn ease_tick(&mut self) {
        let (light_target, omen_target) = match self.phase {
            Phase::Idle => (1.0, 0.0),
            Phase::Omen { .. } => (LIGHT_DIM, 1.0),
            Phase::Wake { .. } => (LIGHT_DIM, 0.0),
        };
        self.light = step_toward(self.light, light_target, LIGHT_RATE * TICK_DT);
        self.omen = step_toward(self.omen, omen_target, OMEN_RATE * TICK_DT);
    }
}

/// A deterministic hull creak, at one random tick per [`CREAK_WINDOW`] of
/// travel. Derived purely from the window number, so it needs no state.
pub fn creak(seed: u64, tick: u64) -> Option<Cue> {
    let h = splitmix(seed ^ CREAK_SALT, tick / CREAK_WINDOW);
    if tick % CREAK_WINDOW == h % CREAK_WINDOW {
        let intensity = ((h >> 32) % 800) as f32 / 1000.0 + 0.2;
        Some(Cue::Creak { intensity })
    } else {
        None
    }
}
