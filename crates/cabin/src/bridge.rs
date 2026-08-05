//! The shell duties, ported from the 2D console's `main.rs`: own the [`Sim`],
//! feed it [`InputFrame`]s, keep the save and the flight recorder, replay
//! absences. Everything in here is bevy-free and testable — the Bevy side
//! only supplies a pointer position (already mapped into sim coordinates by
//! [`crate::surface`]), the button edges, and a frame dt.
//!
//! The contract is identical to the 2D frontend's: the sim never reads the
//! wall clock or the window; whatever the cabin wants to tell it goes in an
//! `InputFrame`. A save written here loads in the 2D console and vice versa
//! (same `STV4` string), and a tape recorded here replays there — the two
//! frontends are two windows onto one deterministic game.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use space_trucking::replay::Recording;
use space_trucking::sim::{Cue, InputFrame, Sim, Vec2};

/// Seconds between wall-clock autosaves; cue-driven saves come sooner.
const SAVE_EVERY: f64 = 10.0;

/// Longest absence the startup catch-up replays, in seconds (six hours).
const MAX_CATCH_UP: f64 = 6.0 * 3600.0;

/// Sim ticks per wall-clock second of absence.
const CATCH_UP_RATE: f64 = 60.0;

/// A frame gap larger than this means the window was frozen or the machine
/// slept: real time kept passing, so the missing ticks are replayed through
/// `fast_forward` instead of being clamped away.
const STALL_SECONDS: f64 = 1.0;

/// The cabin's save file, beside the working directory like the 2D
/// console's `local.data`. First line is the unix timestamp of the save;
/// the rest is the sim's own `STV4` string, portable between frontends.
const SAVE_FILE: &str = "cabin.data";

/// The flight recorder's black box, same cadence as the save. The tape
/// format is the shared `Recording` — a cabin session replays in the 2D
/// console's `--replay` viewer bit-identically.
const REPLAY_FILE: &str = "cabin.replay";

/// A virtual pointer position that no rect contains and no POI is near:
/// where the pointer rests while the cursor touches nothing mapped.
pub const POINTER_PARKED: Vec2 = Vec2::new(-1000.0, -1000.0);

/// What the Bevy side gathered this frame, in sim terms. The pointer is
/// already in sim world coordinates (or [`POINTER_PARKED`]).
// An input snapshot is honestly a pile of booleans, same as InputFrame.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug)]
pub struct FrameInput {
    pub pointer: Vec2,
    pub press: bool,
    pub held: bool,
    pub release: bool,
    pub shift: bool,
    pub key_pause: bool,
    pub key_warp: bool,
    pub key_mute: bool,
    pub key_reseed: bool,
    /// Pointer presses the surface mapper resolved onto the console's
    /// pause / warp / mute icons this frame (the sim ignores those rects;
    /// the shell folds them into toggles, same as the 2D frontend).
    pub icon_pause: bool,
    pub icon_warp: bool,
    pub icon_mute: bool,
}

/// What one shell frame concluded, for the systems downstream.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameOutcome {
    /// The mute toggle is a frontend affair; the audio system consumes it.
    pub toggle_mute: bool,
    /// Ticks replayed silently because the window stalled.
    pub stalled_ticks: u64,
    /// A stall replay crossed an arrival (dock pulse worthy).
    pub stall_arrived: bool,
}

/// The shell: sim, tape, and the clocks that keep them honest.
pub struct Bridge {
    pub sim: Sim,
    recording: Recording,
    dev: bool,
    night: bool,
    /// Wall-clock unix seconds of the last persisted save.
    last_save: f64,
    /// Wall-clock unix seconds of the last frame, for stall detection.
    last_frame: f64,
    /// Seconds until the cheap clock work (night window) reruns.
    clock_check: f64,
    /// The ship docked somewhere during the boot catch-up.
    pub arrived_while_away: bool,
}

impl Bridge {
    /// Load the save and replay the absence, or start fresh. `dev` unlocks
    /// the warp toggle, same as the 2D console's `--dev`.
    #[must_use]
    pub fn boot(dev: bool) -> Self {
        let now = unix_now();
        // An absent or unreadable save becomes a fresh run, quietly.
        let (sim, arrived_while_away) = load_save()
            .and_then(|(save, saved_at)| Sim::from_save(&save).ok().map(|sim| (sim, saved_at)))
            .map_or_else(
                || (Sim::new(fresh_seed()), false),
                |(mut sim, saved_at)| {
                    let elapsed = (now - saved_at).clamp(0.0, MAX_CATCH_UP);
                    let caught_up = sim.fast_forward(ticks_of(elapsed));
                    (sim, caught_up.arrived)
                },
            );
        let recording = Recording::new(sim.save_string());
        Self {
            sim,
            recording,
            dev,
            night: local_night(),
            last_save: now,
            last_frame: now,
            clock_check: SAVE_EVERY,
            arrived_while_away,
        }
    }

    /// Whether developer mode (the warp unlock) is on.
    #[must_use]
    pub const fn dev(&self) -> bool {
        self.dev
    }

    /// One shell frame: stall catch-up, input synthesis, record, advance,
    /// save when worthy. Call exactly once per rendered frame — the sim
    /// drains pointer edges once per `advance`.
    pub fn frame(&mut self, dt: f32, input: &FrameInput) -> FrameOutcome {
        let mut outcome = FrameOutcome::default();

        // A frozen window does not pause the world: replay the gap.
        let wall_now = unix_now();
        let wall_gap = wall_now - self.last_frame;
        if wall_gap > STALL_SECONDS {
            let missed = (wall_gap - f64::from(dt)).clamp(0.0, MAX_CATCH_UP);
            let caught_up = self.sim.fast_forward(ticks_of(missed));
            outcome.stalled_ticks = caught_up.ticks;
            outcome.stall_arrived = caught_up.arrived;
        }
        self.last_frame = wall_now;

        // The night window creeps; check it on the save cadence, not per
        // frame — the OS clock is not free and midnight is not in a hurry.
        self.clock_check -= f64::from(dt);
        if self.clock_check <= 0.0 {
            self.night = local_night();
            self.clock_check = SAVE_EVERY;
        }

        let frame = self.input_frame(input);
        // Mute is the shell's business; the sim never hears about it.
        outcome.toggle_mute = input.key_mute || (input.press && input.icon_mute);
        self.recording.record_frame(self.sim.tick(), &frame);
        self.sim.advance(dt, &frame);

        if self.sim.cues().iter().any(|cue| matches!(cue, Cue::Reseed)) {
            // The black box tells one run's story: a new world, a new tape.
            self.recording = Recording::new(self.sim.save_string());
        }

        if save_worthy(&self.sim) || wall_now - self.last_save >= SAVE_EVERY {
            self.persist(wall_now);
        }
        outcome
    }

    /// Fold the cabin's gathered frame into the sim's input contract,
    /// mirroring the 2D console's `gather_input`.
    fn input_frame(&self, input: &FrameInput) -> InputFrame {
        InputFrame {
            pointer: input.pointer,
            press: input.press,
            held: input.held,
            release: input.release,
            toggle_pause: input.key_pause || (input.press && input.icon_pause),
            toggle_warp: self.dev && (input.key_warp || (input.press && input.icon_warp)),
            shift: input.shift,
            night: self.night,
            reseed: input.key_reseed.then(fresh_seed),
        }
    }

    /// Write the save and the tape now.
    fn persist(&mut self, wall_now: f64) {
        store_save(&self.sim.save_string(), wall_now);
        if self.recording.is_full() && self.sim.held(0).is_none() {
            // Roll the tape. Saves drop drags, so only cut between them.
            self.recording
                .rebase(self.sim.save_string(), self.sim.tick());
        }
        self.recording.seal(self.sim.tick());
        let _ = std::fs::write(save_path(REPLAY_FILE), self.recording.serialize());
        self.last_save = wall_now;
    }
}

/// Whether this frame produced a cue worth writing the save for.
fn save_worthy(sim: &Sim) -> bool {
    sim.cues().iter().any(|cue| {
        matches!(
            cue,
            Cue::Arrive
                | Cue::Depart
                | Cue::Accept { .. }
                | Cue::Place
                | Cue::Pause { .. }
                | Cue::Reseed
        )
    })
}

/// Wall-clock seed for fresh runs; determinism starts once the sim owns it.
fn fresh_seed() -> u64 {
    unix_now().to_bits()
}

/// Seconds since the unix epoch, as the shared save timestamp base.
fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |dur| dur.as_secs_f64())
}

/// Ticks worth of a wall-clock absence.
fn ticks_of(seconds: f64) -> u64 {
    u64::try_from((seconds * CATCH_UP_RATE) as i64).unwrap_or(0)
}

/// Whether it is deep night (23:30–06:00) on the player's clock — the
/// Umbra Market's opening hours. Same window as the 2D native build.
fn local_night() -> bool {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let minutes = now.hour() * 60 + now.minute();
    !(360..1410).contains(&minutes)
}

/// Where the cabin keeps a data file: the working directory, matching the
/// 2D console's `local.data` convention.
fn save_path(name: &str) -> PathBuf {
    PathBuf::from(name)
}

/// Read the save file: (sim save string, unix seconds it was written).
fn load_save() -> Option<(String, f64)> {
    let text = std::fs::read_to_string(save_path(SAVE_FILE)).ok()?;
    let (stamp, save) = text.split_once('\n')?;
    let saved_at = stamp.trim().parse::<f64>().ok()?;
    Some((save.to_string(), saved_at))
}

/// Write the save file, timestamp first. Failure is silent by design: a
/// read-only directory costs persistence, not the session.
fn store_save(save: &str, wall_now: f64) {
    let _ = std::fs::write(save_path(SAVE_FILE), format!("{wall_now}\n{save}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_frame_mirrors_the_2d_gather() {
        let bridge = Bridge {
            sim: Sim::new(7),
            recording: Recording::new(Sim::new(7).save_string()),
            dev: false,
            night: true,
            last_save: 0.0,
            last_frame: 0.0,
            clock_check: SAVE_EVERY,
            arrived_while_away: false,
        };
        let frame = bridge.input_frame(&FrameInput {
            pointer: Vec2::new(4.0, 5.0),
            press: true,
            held: true,
            release: false,
            shift: true,
            key_pause: false,
            key_warp: true,
            key_mute: false,
            key_reseed: false,
            icon_pause: true,
            icon_warp: false,
            icon_mute: false,
        });
        assert!(frame.press && frame.held && !frame.release && frame.shift);
        // Icon press folds into the pause toggle, same as the 2D shell.
        assert!(frame.toggle_pause);
        // Warp stays locked without dev mode, key or no key.
        assert!(!frame.toggle_warp);
        assert!(frame.night);
        assert!(frame.reseed.is_none());
    }

    #[test]
    fn parked_pointer_touches_nothing() {
        use space_trucking::sim::layout;
        assert!(!layout::MAP_PANEL.contains(POINTER_PARKED));
        assert!(!layout::BARTER_PANEL.contains(POINTER_PARKED));
        assert!(layout::cell_at(POINTER_PARKED).is_none());
    }

    #[test]
    fn ticks_of_absence_round_down_sanely() {
        assert_eq!(ticks_of(0.0), 0);
        assert_eq!(ticks_of(1.0), 60);
        assert_eq!(ticks_of(-5.0), 0);
    }
}
