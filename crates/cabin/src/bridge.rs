//! The shell duties, ported from the 2D console's `main.rs`: own the [`Sim`],
//! feed it [`InputFrame`]s, keep the save and the flight recorder, replay
//! absences. Everything in here is bevy-free and testable — the Bevy side
//! only supplies a pointer position (already mapped into sim coordinates by
//! [`crate::surface`]), the button edges, and a frame dt.
//!
//! The contract survives the 2D console's retirement: the sim never reads
//! the wall clock or the window; whatever the cabin wants to tell it goes
//! in an `InputFrame`. The save reader still accepts the console's
//! `STV4` and writes whatever header the sim's own format is on, so a
//! console-era run walks aboard through the migration chain, and the
//! tape format is unchanged — one deterministic game, whatever the
//! window.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use space_trucking::replay::Recording;
use space_trucking::sim::room::{CABIN, RoomId};
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
/// the rest is the sim's own save string, written at the current header
/// and read back at that or any older one the chain still carries.
const SAVE_FILE: &str = "cabin.data";

/// The flight recorder's black box, same cadence as the save. The tape
/// format is the shared `Recording` — a cabin session replays in the 2D
/// console's `--replay` viewer bit-identically.
const REPLAY_FILE: &str = "cabin.replay";

/// The 2D console's own native save container (quad-storage writes a
/// JSON object `{"local":{key:value,..}}`). When the cabin has no save
/// of its own, an existing console run walks aboard from here — the
/// reader accepts its `STV4`, the same catch-up runs, and the next save
/// lands at the current header in the cabin's own slot. Adoption
/// happens once.
const CONSOLE_FILE: &str = "local.data";

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
    /// **Which room the body stands in**, derived from the camera by
    /// `room::occupy` (docs/ROOMS.md, "The one new input field"). The
    /// gates read this and nothing else about where anybody stands.
    pub occupied: RoomId,
    /// A detach asked for this frame — the door's own amber latch was
    /// clicked. The sim's gangway gates answer; a refusal is a cue.
    pub detach: Option<RoomId>,
}

impl Default for FrameInput {
    fn default() -> Self {
        Self {
            pointer: POINTER_PARKED,
            press: false,
            held: false,
            release: false,
            shift: false,
            key_pause: false,
            key_warp: false,
            key_mute: false,
            key_reseed: false,
            icon_pause: false,
            icon_warp: false,
            icon_mute: false,
            occupied: CABIN,
            detach: None,
        }
    }
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
// Four independent yes/no facts about the shell are four bools; a state
// machine here would be ceremony.
#[allow(clippy::struct_excessive_bools)]
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
    /// A `--fixture` boot: a throwaway world that must never write
    /// over the player's real save or tape.
    sandbox: bool,
}

impl Bridge {
    /// Load the save and replay the absence, or start fresh. `dev` unlocks
    /// the warp toggle, same as the 2D console's `--dev` — and a dev mode
    /// earned in the console (its stored pretty-please) carries over.
    #[must_use]
    pub fn boot(dev: bool) -> Self {
        let dev = dev || console_dev();
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
            sandbox: false,
        }
    }

    /// Boot the developer fixture (`--fixture`): the given save, no
    /// absence catch-up, dev unlocked, and sandboxed — this world never
    /// persists, so the player's real `cabin.data` survives the sweep.
    /// An unparseable fixture is a build error, not a fallback.
    #[must_use]
    pub fn boot_fixture(save: &str) -> Self {
        let sim = Sim::from_save(save).expect("the developer fixture must parse");
        let now = unix_now();
        let recording = Recording::new(sim.save_string());
        Self {
            sim,
            recording,
            dev: true,
            night: local_night(),
            last_save: now,
            last_frame: now,
            clock_check: SAVE_EVERY,
            arrived_while_away: false,
            sandbox: true,
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
            // The sim learns rooms, not positions (docs/ROOMS.md). The
            // body walks through doorways now, so this is the room whose
            // box the eye stands in — `room::occupy`'s answer, and the
            // only thing the gates learn about where anybody is.
            occupied: input.occupied,
            attach: None,
            // The detach gesture is the door's own amber latch, and it
            // rides the input schedule exactly like a pointer press: the
            // sim decides, and refuses with a cue if the seam would
            // strand something.
            detach: input.detach,
            reseed: input.key_reseed.then(fresh_seed),
        }
    }

    /// Write the save and the tape now. A sandboxed (fixture) world
    /// writes nothing, ever.
    fn persist(&mut self, wall_now: f64) {
        if self.sandbox {
            self.last_save = wall_now;
            return;
        }
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

/// Read the save: the cabin's own file first, else adopt the 2D
/// console's `local.data` sitting in the same directory.
fn load_save() -> Option<(String, f64)> {
    load_own().or_else(load_console)
}

/// The cabin's own slot: (sim save string, unix seconds it was written).
fn load_own() -> Option<(String, f64)> {
    let text = std::fs::read_to_string(save_path(SAVE_FILE)).ok()?;
    let (stamp, save) = text.split_once('\n')?;
    let saved_at = stamp.trim().parse::<f64>().ok()?;
    Some((save.to_string(), saved_at))
}

/// The console's slot, via its container format.
fn load_console() -> Option<(String, f64)> {
    let text = std::fs::read_to_string(save_path(CONSOLE_FILE)).ok()?;
    parse_console_save(&text)
}

/// Pull the save and its stamp out of quad-storage's JSON container. The
/// stamp is `f64::to_bits` in hex, exactly as `src/storage.rs` writes it.
fn parse_console_save(text: &str) -> Option<(String, f64)> {
    let root: serde_json::Value = serde_json::from_str(text).ok()?;
    let local = root.get("local")?;
    let save = local.get("space-trucking/save")?.as_str()?;
    let stamp = local.get("space-trucking/saved_at")?.as_str()?;
    let saved_at = f64::from_bits(u64::from_str_radix(stamp, 16).ok()?);
    saved_at.is_finite().then(|| (save.to_string(), saved_at))
}

/// Whether the console's stored developer mode says the pretty-please
/// was once said. Saying it once is saying it forever, across frontends.
fn console_dev() -> bool {
    let Ok(text) = std::fs::read_to_string(save_path(CONSOLE_FILE)) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|root| {
            root.get("local")?
                .get("space-trucking/dev")
                .map(|dev| dev.as_str() == Some("1"))
        })
        .unwrap_or(false)
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
            sandbox: false,
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
            occupied: CABIN,
            detach: None,
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
        assert!(!layout::CONSOLE.contains(POINTER_PARKED));
        assert!(layout::cell_at(POINTER_PARKED).is_none());
    }

    #[test]
    fn ticks_of_absence_round_down_sanely() {
        assert_eq!(ticks_of(0.0), 0);
        assert_eq!(ticks_of(1.0), 60);
        assert_eq!(ticks_of(-5.0), 0);
    }

    /// A run started in the 2D console walks aboard: the quad-storage
    /// container parses, the stamp's hex bits round-trip, and the save
    /// string inside satisfies the sim.
    #[test]
    fn adopts_the_console_container() {
        let save = Sim::new(9).save_string();
        let stamp = format!("{:x}", 1_700_000_000.0f64.to_bits());
        let container = serde_json::json!({
            "local": {
                "space-trucking/dev": "1",
                "space-trucking/save": save,
                "space-trucking/saved_at": stamp,
            }
        })
        .to_string();
        let (parsed, at) = parse_console_save(&container).expect("container parses");
        assert_eq!(parsed, save);
        assert!((at - 1_700_000_000.0).abs() < f64::EPSILON);
        assert!(Sim::from_save(&parsed).is_ok());
        // Garbage fails safe into a fresh run, never a panic.
        assert!(parse_console_save("not json").is_none());
        assert!(parse_console_save("{\"local\":{}}").is_none());
    }

    /// A bare shell around a sim, for the gate tests below.
    fn shell(sim: Sim) -> Bridge {
        Bridge {
            recording: Recording::new(sim.save_string()),
            sim,
            dev: false,
            night: false,
            last_save: unix_now(),
            last_frame: unix_now(),
            clock_check: SAVE_EVERY,
            arrived_while_away: false,
            sandbox: false,
        }
    }

    /// **The occupied-room field, end to end.** The cabin derives which
    /// room the body stands in from the camera (`room::occupy`), hands it
    /// over as one dense id, and the gangway law does the rest: standing
    /// in the station's own trade room, the launch lever refuses, because
    /// casting off would take the ship and leave the body behind. Walk
    /// back aboard and the very same pull flies.
    #[test]
    fn the_launch_gate_reads_the_room_the_body_stands_in() {
        use space_trucking::sim::room::RoomKind;
        use space_trucking::sim::{ShipState, layout};

        let sim = Sim::new(4);
        let trade = sim
            .rooms()
            .find(RoomKind::Trade)
            .expect("the Guild's room is alongside at the dock");
        assert_ne!(trade, CABIN);
        let mut bridge = shell(sim);
        let quiet = FrameInput::default();
        let lever = layout::LAUNCH_LEVER;
        let pull = |occupied| FrameInput {
            pointer: Vec2::new(lever.w.mul_add(0.5, lever.x), lever.h.mul_add(0.5, lever.y)),
            press: true,
            held: true,
            occupied,
            ..quiet
        };

        // Arm a course first, so the only thing left to refuse is us.
        let jupiter: space_trucking::sim::map::PoiId = 3;
        bridge.frame(
            0.02,
            &FrameInput {
                pointer: bridge.sim.poi_pos(jupiter),
                press: true,
                held: true,
                ..quiet
            },
        );
        assert_eq!(bridge.sim.ship().selected, Some(jupiter));

        // Standing in the trade room: refused, and nothing is lost to the
        // lever — the course stays armed.
        bridge.frame(0.02, &pull(trade));
        assert!(
            matches!(bridge.sim.ship().state, ShipState::Docked(_)),
            "the lever cast off with the body still ashore"
        );
        assert_eq!(bridge.sim.ship().selected, Some(jupiter));

        // Back aboard: the same pull flies.
        bridge.frame(0.02, &pull(CABIN));
        assert!(
            matches!(bridge.sim.ship().state, ShipState::Traveling { .. }),
            "the lever refused a legal launch: {:?}",
            bridge.sim.ship().state
        );
    }

    /// **The detach gesture, end to end.** The door's amber latch rides
    /// the input schedule like any other input; the sim's gangway gates
    /// answer it. Ask from inside the room and it refuses by name; ask
    /// from the cabin, with nothing of yours in there, and the seam parts.
    #[test]
    fn the_latch_asks_and_the_gangway_law_answers() {
        use space_trucking::sim::room::RoomKind;
        use space_trucking::sim::{Cue, Refusal};

        let sim = Sim::new(4);
        let trade = sim.rooms().find(RoomKind::Trade).expect("alongside");
        let mut bridge = shell(sim);

        // From inside: a seam that could strand you refuses to part.
        bridge.frame(
            0.02,
            &FrameInput {
                occupied: trade,
                detach: Some(trade),
                ..FrameInput::default()
            },
        );
        assert!(
            bridge.sim.cues().iter().any(|cue| matches!(
                cue,
                Cue::Refit {
                    refusal: Refusal::Aboard
                }
            )),
            "the latch parted a room with a body in it"
        );
        assert!(bridge.sim.rooms().get(trade).is_some());

        // From the cabin: the room goes, and its own goods go with it.
        bridge.frame(
            0.02,
            &FrameInput {
                occupied: CABIN,
                detach: Some(trade),
                ..FrameInput::default()
            },
        );
        assert!(
            bridge
                .sim
                .cues()
                .iter()
                .any(|cue| matches!(cue, Cue::Parted)),
            "the latch failed to part a room it was allowed to"
        );
        assert!(bridge.sim.rooms().get(trade).is_none());
    }

    /// The whole point of the cabin: a synthetic pointer, pressed where
    /// the surface mapper would put it, flies the ship exactly like a 2D
    /// mouse. Select Jupiter on the tank, pull the launch lever, travel.
    #[test]
    fn a_synthetic_pointer_flies_the_ship() {
        use space_trucking::sim::{ShipState, layout};

        let sim = Sim::new(42);
        let mut bridge = Bridge {
            recording: Recording::new(sim.save_string()),
            sim,
            dev: false,
            night: false,
            last_save: unix_now(),
            last_frame: unix_now(),
            clock_check: SAVE_EVERY,
            arrived_while_away: false,
            sandbox: false,
        };
        let quiet = FrameInput {
            pointer: POINTER_PARKED,
            press: false,
            held: false,
            release: false,
            shift: false,
            key_pause: false,
            key_warp: false,
            key_mute: false,
            key_reseed: false,
            icon_pause: false,
            icon_warp: false,
            icon_mute: false,
            occupied: CABIN,
            detach: None,
        };

        // Press on Jupiter's live tank position: the sim arms the course.
        let jupiter: space_trucking::sim::map::PoiId = 3;
        let press_at = |pointer| FrameInput {
            pointer,
            press: true,
            held: true,
            ..quiet
        };
        bridge.frame(0.02, &press_at(bridge.sim.poi_pos(jupiter)));
        bridge.frame(
            0.02,
            &FrameInput {
                release: true,
                ..quiet
            },
        );
        assert_eq!(bridge.sim.ship().selected, Some(jupiter));

        // Pull the lever (a press inside its rect): the ship departs.
        let lever = layout::LAUNCH_LEVER;
        bridge.frame(
            0.02,
            &press_at(Vec2::new(
                lever.w.mul_add(0.5, lever.x),
                lever.h.mul_add(0.5, lever.y),
            )),
        );
        assert!(
            matches!(
                bridge.sim.ship().state,
                ShipState::Traveling { to, .. } if to == jupiter
            ),
            "lever pull should cast off toward Jupiter, state: {:?}",
            bridge.sim.ship().state
        );
        assert_eq!(bridge.sim.legs(), 1);
    }
}
