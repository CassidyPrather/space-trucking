//! The shell duties, ported from the 2D console's `main.rs`: own the [`Sim`],
//! feed it [`InputFrame`]s, keep the save and the flight recorder, replay
//! absences. Everything in here is bevy-free and testable — the Bevy side
//! only supplies a pointer position (already mapped into sim coordinates by
//! [`crate::surface`]), the button edges, and a frame dt.
//!
//! The contract survives the 2D console's retirement: the sim never reads
//! the wall clock or the window; whatever the cabin wants to tell it goes
//! in an `InputFrame` — one deterministic game, whatever the window. A
//! save from any version but the sim's own is not read, and the boot
//! below starts a new run, as it does for any save it cannot read
//! (`docs/DESIGN_REVIEW.md`).

use std::path::PathBuf;

use space_trucking::replay::Recording;
use space_trucking::sim::room::{CABIN, RoomId};
use space_trucking::sim::{Cue, InputFrame, Sim, Turn, Vec2};
// `std::time` on native, `Date.now()` on wasm32, where std's clock is a
// panic rather than a number. The market hours below ask chrono instead,
// which the manifest routes the same way for wasm32 (`wasmbind`).
use web_time::{SystemTime, UNIX_EPOCH};

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

/// The cabin's save file, in the working directory. First line is the
/// unix timestamp of the save; the rest is the sim's own save string,
/// written at the current header and read back only at that one.
const SAVE_FILE: &str = "cabin.data";

/// The flight recorder's black box, same cadence as the save. The tape
/// format is the shared `Recording`, and a tape replays bit-identically
/// through `Recording::replay` against the sim alone, which is what the
/// replay tests do with it; no frontend plays one back to watch.
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
    /// The `Esc` menu's pause / fast-forward / mute controls, worked this
    /// frame. These arrive as plain edges rather than as pointer presses
    /// on a rect: the console face that carried those icon rects came
    /// off the wall (`crate::menu`), and a meta-control is not a place
    /// in the room. The folding below is unchanged — the shell turns
    /// them into the same toggles the keys throw, same as the 2D
    /// frontend did with its icons.
    pub menu_pause: bool,
    pub menu_warp: bool,
    pub menu_mute: bool,
    /// The `Esc` menu's new-run bar, worked this frame. No key throws
    /// this one: a run ends only where the menu says so in words.
    pub menu_reseed: bool,
    /// **Which room the body stands in**, derived from the camera by
    /// `room::occupy` (docs/ROOMS.md, "The one new input field"). The
    /// gates read this and nothing else about where anybody stands.
    pub occupied: RoomId,
    /// A detach asked for this frame — the door's own amber latch was
    /// clicked. The sim's gangway gates answer; a refusal is a cue.
    pub detach: Option<RoomId>,
    /// **The hands on the carry's facing this frame**: the wheel, `Ctrl`
    /// and `Q` (docs/BAY.md, "Cargo turns"). The bridge turns the carry
    /// by them ([`Bridge::facing`]); the sim only ever hears the facing.
    pub turning: Turning,
}

/// **One frame of turning the carry**, as the hands gave it.
///
/// Gathered by the cabin only while the body roams with a piece in hand
/// — the menu, a focus and an empty hand all send nothing — and spent by
/// the bridge on the carry's facing, which it alone holds
/// ([`Bridge::facing`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turning {
    /// How far the wheel rolled, in notches: positive is rolled up, away
    /// from you, which turns the carry counter-clockwise as seen from the
    /// room. A fine wheel and a touchpad send fractions of a notch, and
    /// the bridge adds them up until they make one.
    pub wheel: f32,
    /// `Ctrl` is held: a notch turns one degree, from wherever the carry
    /// is, instead of to the next multiple of fifteen.
    pub fine: bool,
    /// `Q` went down this frame: `1` for one fifteen-degree step the way
    /// the wheel rolled up turns, `-1` for `Shift+Q`, the other way, and
    /// `0` for neither.
    pub key: i8,
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
            menu_pause: false,
            menu_warp: false,
            menu_mute: false,
            menu_reseed: false,
            occupied: CABIN,
            detach: None,
            turning: Turning::default(),
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
    /// A world whose frames arrive on a fixed step, so the wall clock is
    /// not its clock. See [`Bridge::steady`].
    steady: bool,
    /// The carry and the facing it is sent at ([`Bridge::facing`]).
    carry: Carry,
}

impl Bridge {
    /// Load the save and replay the absence, or start fresh. `dev` unlocks
    /// the warp toggle, same as the 2D console's `--dev`.
    #[must_use]
    pub fn boot(dev: bool) -> Self {
        let now = unix_now();
        // An absent or unreadable save becomes a fresh run, quietly, and
        // a save from another version is an unreadable one.
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
            steady: false,
            carry: Carry::default(),
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
            steady: false,
            carry: Carry::default(),
        }
    }

    /// Whether developer mode (the warp unlock) is on.
    #[must_use]
    pub const fn dev(&self) -> bool {
        self.dev
    }

    /// **Take this world off the wall clock.** Its frames arrive on a
    /// fixed step, so the seconds between two of them are the machine
    /// being slow and not the player being away, and the absence replay
    /// below must not fire: a world that fast-forwards by however long
    /// the shaders took to build is a different world every run.
    ///
    /// The dev modes that judge a picture ask for this. A played session
    /// never does — a frozen window there really is time the player lost.
    pub const fn steady(&mut self) {
        self.steady = true;
    }

    /// One shell frame: stall catch-up, input synthesis, record, advance,
    /// save when worthy. Call exactly once per rendered frame — the sim
    /// drains pointer edges once per `advance`.
    pub fn frame(&mut self, dt: f32, input: &FrameInput) -> FrameOutcome {
        let mut outcome = FrameOutcome::default();

        // A frozen window does not pause the world: replay the gap. A
        // steady world has no gaps to replay — its clock is counted.
        let wall_now = unix_now();
        let wall_gap = wall_now - self.last_frame;
        if !self.steady && wall_gap > STALL_SECONDS {
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

        // The hands turn the carry before the frame is built, so a release
        // this frame lands at the turn they left it at.
        self.turn_carry(input.turning);
        let frame = self.input_frame(input);
        // Mute is the shell's business; the sim never hears about it.
        outcome.toggle_mute = input.key_mute || input.menu_mute;
        self.recording.record_frame(self.sim.tick(), &frame);
        self.sim.advance(dt, &frame);
        // A grab this frame starts a carry at the piece's own turn, and a
        // drop ends one.
        self.follow();

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
            toggle_pause: input.key_pause || input.menu_pause,
            toggle_warp: self.dev && (input.key_warp || input.menu_warp),
            shift: input.shift,
            night: self.night,
            // The sim learns rooms, not positions (docs/ROOMS.md). The
            // body walks through doorways now, so this is the room whose
            // box the eye stands in — `room::occupy`'s answer, and the
            // only thing the gates learn about where anybody is.
            occupied: input.occupied,
            facing: self.facing(),
            attach: None,
            // The detach gesture is the door's own amber latch, and it
            // rides the input schedule exactly like a pointer press: the
            // sim decides, and refuses with a cue if the seam would
            // strand something.
            detach: input.detach,
            reseed: input.menu_reseed.then(fresh_seed),
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

    /// **The facing the carry is sent at** (`InputFrame::facing`): the
    /// carry's own, which starts at the held piece's own turn when it is
    /// lifted and goes wherever the player turns it; `Turn(0)` while
    /// nothing is held.
    ///
    /// The one place the cabin keeps it, so the release and the preview
    /// the ghost and the footprint patch are drawn from (`crate::pieces`)
    /// ask with the same turn. A carry the bridge has not seen begin — a
    /// sim handed to it mid-carry — is at the piece's own turn until it
    /// is turned.
    #[must_use]
    pub fn facing(&self) -> Turn {
        match self.sim.held(0) {
            Some(held) if self.carry.piece == Some(held.piece) => self.carry.facing.turn(),
            _ => lifted_at(&self.sim),
        }
    }

    /// Keep the carry with the piece in hand: a carry that begins starts
    /// at the turn the piece was lifted at ([`lifted_at`]), and one that
    /// ends forgets its facing and any wheel travel short of a notch, so
    /// the next carry starts from its own piece and nothing else.
    fn follow(&mut self) {
        let piece = self.sim.held(0).map(|held| held.piece);
        if self.carry.piece != piece {
            self.carry = Carry {
                piece,
                facing: Facing::of(lifted_at(&self.sim)),
                wheel: 0.0,
            };
        }
    }

    /// **Turn the carry by this frame's hands.** Nothing turns while
    /// nothing is held.
    ///
    /// A notch of the wheel turns to the next multiple of fifteen degrees
    /// its way ([`Facing::stop`]), and with `Ctrl` held one degree
    /// exactly, from wherever the carry is ([`Facing::by`]); `Q` and
    /// `Shift+Q` take the plain notch's step. Wheel travel short of a
    /// whole notch waits for the next frame's.
    fn turn_carry(&mut self, turning: Turning) {
        self.follow();
        if self.carry.piece.is_none() {
            return;
        }
        self.carry.wheel += turning.wheel;
        let notches = self.carry.wheel.trunc();
        self.carry.wheel -= notches;
        let notches = notches as i64;
        let facing = &mut self.carry.facing;
        if turning.fine {
            *facing = facing.by(notches * Facing::DEGREE);
        } else if notches != 0 {
            // Past the first, every step starts on a stop, and twenty-four
            // of them from a stop are a whole turn back to it.
            for _ in 0..=(notches.unsigned_abs() - 1) % 24 {
                *facing = facing.stop(notches.signum());
            }
        }
        if turning.key != 0 {
            *facing = facing.stop(i64::from(turning.key.signum()));
        }
    }
}

/// **The turn a carry starts at**: the held piece's own — the turn it
/// was lifted at, which the sim keeps as the carry's origin — or the
/// upright frame for a piece lifted out of a cubby, and while nothing is
/// held.
fn lifted_at(sim: &Sim) -> Turn {
    sim.held(0)
        .and_then(|held| held.origin.spot())
        .map_or(Turn::ZERO, |spot| spot.turn)
}

/// **The carry, as the frontend keeps it**: which piece is in hand, the
/// facing it is carried at, and the wheel's travel short of a notch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Carry {
    piece: Option<u32>,
    facing: Facing,
    wheel: f32,
}

/// **A carry's facing, held finer than the sim's `Turn`**: in
/// forty-fifths of a `Turn` unit, 2,949,120 to the turn.
///
/// Forty-five is 360 over the largest power of two that divides it, so
/// this is the coarsest unit in which a whole degree and a whole `Turn`
/// unit are both whole numbers: a degree is 8,192 of them and fifteen is
/// 122,880. So a degree a notch is a degree exactly however many notches
/// are turned, every multiple of fifteen is a whole number to land on,
/// and the turn a piece was lifted at is held exactly. What the sim is
/// sent is the nearest `Turn` ([`Facing::turn`]), and the sim snaps
/// nothing: a convenient angle is the wheel's offer and no rule's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Facing(i64);

impl Facing {
    /// Parts to one `Turn` unit.
    const PER: i64 = 45;
    /// A whole turn.
    const WHOLE: i64 = 65_536 * Self::PER;
    /// One degree.
    const DEGREE: i64 = Self::WHOLE / 360;
    /// The wheel's convenient angle: fifteen degrees.
    const STOP: i64 = 15 * Self::DEGREE;

    /// Exactly `turn`.
    fn of(turn: Turn) -> Self {
        Self(i64::from(turn.0) * Self::PER)
    }

    /// The `Turn` the sim is sent: the nearest one. Forty-five is odd, so
    /// no facing lies halfway between two.
    fn turn(self) -> Turn {
        let nearest = (self.0 + Self::PER / 2) / Self::PER % 65_536;
        Turn(u16::try_from(nearest).unwrap_or(0))
    }

    /// `parts` on from here, counter-clockwise for positive, wrapping.
    const fn by(self, parts: i64) -> Self {
        Self((self.0 + parts).rem_euclid(Self::WHOLE))
    }

    /// **The next multiple of fifteen degrees `way`** — counter-clockwise
    /// for positive — that the sim would be sent as a different `Turn`.
    ///
    /// The next one strictly past this facing: from 7° one step up is 15°
    /// and not 22°, and from 15° it is 30°. A piece lifted at the `Turn`
    /// nearest 15° is held a hair past 15°, so the stop at 15° itself is
    /// skipped on the way down, because the sim would be sent the turn it
    /// already has and the notch would do nothing anybody could see.
    fn stop(self, way: i64) -> Self {
        let here = self.turn();
        let mut k = if way > 0 {
            self.0.div_euclid(Self::STOP) + 1
        } else {
            (self.0 - 1).div_euclid(Self::STOP)
        };
        loop {
            let next = Self((k * Self::STOP).rem_euclid(Self::WHOLE));
            if next.turn() != here {
                return next;
            }
            k += if way > 0 { 1 } else { -1 };
        }
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

/// Where the cabin keeps a data file: the working directory.
fn save_path(name: &str) -> PathBuf {
    PathBuf::from(name)
}

/// The cabin's save: (sim save string, unix seconds it was written).
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
            sandbox: false,
            steady: false,
            carry: Carry::default(),
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
            menu_pause: true,
            menu_warp: false,
            menu_mute: false,
            menu_reseed: false,
            occupied: CABIN,
            detach: None,
            turning: Turning::default(),
        });
        assert!(frame.press && frame.held && !frame.release && frame.shift);
        // A menu edge folds into the pause toggle, exactly where the
        // console face's icon rect used to.
        assert!(frame.toggle_pause);
        // Warp stays locked without dev mode, key or no key.
        assert!(!frame.toggle_warp);
        assert!(frame.night);
        assert!(frame.reseed.is_none());
    }

    /// The stall replay is the wall clock reaching into the sim, and a
    /// run that has to reproduce cannot have one. A busy machine takes
    /// well over a second to reach its first frame and drops whole
    /// seconds mid-run: a played session owes the player those, and
    /// pays them as sixty to a hundred and twenty ticks of world — a
    /// different number every run, which is not a thing a screenshot
    /// may be built out of.
    #[test]
    fn a_steady_world_replays_no_absence() {
        let hour_ago = unix_now() - 3600.0;
        let played = {
            let mut bridge = Bridge::boot_fixture(&Sim::new(7).save_string());
            bridge.last_frame = hour_ago;
            bridge.frame(1.0 / 60.0, &FrameInput::default())
        };
        let judged = {
            let mut bridge = Bridge::boot_fixture(&Sim::new(7).save_string());
            bridge.steady();
            bridge.last_frame = hour_ago;
            bridge.frame(1.0 / 60.0, &FrameInput::default())
        };
        assert!(
            played.stalled_ticks > 0,
            "a played session owes the player the hour the window was frozen"
        );
        assert_eq!(
            judged.stalled_ticks, 0,
            "a judged run owes nobody anything: its clock is counted"
        );
    }

    /// A frame of input that demonstrably moves a world which is allowed
    /// to move: the pointer sweeping the star map, presses and releases
    /// cycling, a detach asked for. The pause guards below are worth
    /// nothing without it — a script that does nothing proves nothing —
    /// so each of them runs it against a running world as a control and
    /// checks that the world it is allowed to move actually moved.
    fn busy_frame(n: u32) -> FrameInput {
        use space_trucking::sim::layout;
        let along = f32::from(u16::try_from(n % 600).unwrap_or(0)) / 600.0;
        let phase = n % 6;
        FrameInput {
            pointer: Vec2::new(
                layout::MAP_PANEL.w.mul_add(along, layout::MAP_PANEL.x),
                layout::MAP_PANEL.h.mul_add(0.5, layout::MAP_PANEL.y),
            ),
            press: phase == 0,
            held: phase == 1 || phase == 2,
            release: phase == 3,
            shift: phase == 4,
            detach: (phase == 5).then_some(CABIN),
            ..FrameInput::default()
        }
    }

    /// A sandboxed world from one seed: writes nothing, dev unlocked.
    fn world() -> Bridge {
        Bridge::boot_fixture(&Sim::new(7).save_string())
    }

    /// **A paused world does not advance.**
    ///
    /// The pause mark under the crosshair (`crate::menu`) is only worth
    /// drawing if the thing it names is real, and *real* is a stronger
    /// claim than "the tick counter stopped": it is that nothing the
    /// player can do reaches the world at all while pause stands, so a
    /// paused game is a place where state cannot change. That is the
    /// property the mark is a reading of, and this is where it is
    /// pinned rather than assumed.
    ///
    /// Asked in the vocabulary the sim already proves itself in
    /// (`same_seed_and_inputs_are_bit_identical`): the save string is
    /// the whole of the world that survives, so ten seconds of busy
    /// input must leave it byte for byte where it was, with no tick run
    /// and nothing announced. The control arm is the other half — the
    /// same script, unpaused, moves the world, emits cues and rewrites
    /// the save, so what stopped it was the pause and not the script.
    ///
    /// One thing does still move while paused, and is entitled to: the
    /// OS clock refreshes `night` on the save cadence. It is an input
    /// the sim mirrors and never accumulates — no tick reads it, and it
    /// is not in the save — which is why the byte comparison below holds
    /// straight through a midnight.
    #[test]
    fn a_paused_world_does_not_advance() {
        let dt = space_trucking::sim::TICK_DT;

        let mut paused = world();
        paused.frame(
            dt,
            &FrameInput {
                key_pause: true,
                ..FrameInput::default()
            },
        );
        assert!(paused.sim.is_paused(), "the toggle must reach the sim");
        let sealed = paused.sim.save_string();
        let tick = paused.sim.tick();
        let mut announced = 0;
        for n in 0..600 {
            let outcome = paused.frame(dt, &busy_frame(n));
            announced += paused.sim.cues().len();
            assert_eq!(outcome.stalled_ticks, 0, "a paused world caught up");
        }
        assert_eq!(paused.sim.tick(), tick, "a paused world ran a tick");
        assert_eq!(announced, 0, "a paused world had something to announce");
        assert!(
            paused.sim.save_string() == sealed,
            "ten seconds of input changed a paused world"
        );

        let mut running = world();
        let before = running.sim.save_string();
        let mut heard = 0;
        for n in 0..600 {
            running.frame(dt, &busy_frame(n));
            heard += running.sim.cues().len();
        }
        assert!(running.sim.tick() > tick, "the control arm never ticked");
        assert!(
            heard > 0,
            "the control arm said nothing; the script is inert"
        );
        assert!(
            running.sim.save_string() != before,
            "the control arm left the world untouched; the script is inert"
        );
    }

    /// **An absence spent paused is not repaid.**
    ///
    /// This is the way pause is decorative rather than false. The shell
    /// owes a played session the time its window was frozen and pays it
    /// in ticks ([`STALL_SECONDS`], [`Bridge::frame`]), so a world left
    /// paused for ten minutes has a bill sitting against it, and a debt
    /// settled on the way out is a pause the player only thought they
    /// had. The sim refuses the payment while it is paused, and the
    /// gap is spent rather than banked — the frame clock is stamped
    /// whether or not anything was replayed — so the minutes are gone
    /// on both sides of the resume.
    ///
    /// The running arm is what makes this a finding rather than a
    /// tautology: the very same ten minutes, on a world that is not
    /// paused, arrive as ten minutes of world.
    #[test]
    fn an_absence_spent_paused_is_not_repaid() {
        let dt = space_trucking::sim::TICK_DT;
        let away = 600.0;

        let mut paused = world();
        paused.frame(
            dt,
            &FrameInput {
                key_pause: true,
                ..FrameInput::default()
            },
        );
        let tick = paused.sim.tick();
        paused.last_frame -= away;
        let outcome = paused.frame(dt, &FrameInput::default());
        assert_eq!(
            outcome.stalled_ticks, 0,
            "ten minutes of absence were paid into a paused world"
        );
        assert_eq!(paused.sim.tick(), tick, "a paused world caught up anyway");

        // And they are not waiting on the other side of the resume: a
        // second of frames buys a second of world, no more.
        paused.frame(
            dt,
            &FrameInput {
                key_pause: true,
                ..FrameInput::default()
            },
        );
        assert!(!paused.sim.is_paused(), "the toggle must reach the sim");
        let resumed = paused.sim.tick();
        for _ in 0..60 {
            paused.frame(dt, &FrameInput::default());
        }
        assert_eq!(
            paused.sim.tick() - resumed,
            60,
            "the minutes spent paused were repaid after the resume"
        );

        let mut running = world();
        let ran = running.sim.tick();
        running.last_frame -= away;
        let outcome = running.frame(dt, &FrameInput::default());
        assert!(
            outcome.stalled_ticks > 0 && running.sim.tick() > ran,
            "a frozen window owes the player its ticks; nothing was replayed"
        );
    }

    /// **The one thing a paused world still answers is a new world.**
    ///
    /// A boundary is only clean if its doors are named. Every other
    /// input the shell can send dies at the pause gate, and one does
    /// not: a reseed is applied ahead of it (`Sim::apply_input`), on
    /// purpose, because throwing the world away is not a move inside
    /// the world. What comes back is a *replacement* and not an
    /// advance — the tick counter is at nought rather than further on —
    /// and the pause itself survives, so the player who was stopped is
    /// still stopped in the world they now have.
    ///
    /// Two laws hold the door to one door wide, and this is only one of
    /// them: `a_paused_world_does_not_advance` is the half that says
    /// nothing else gets through, and this is the half that says what
    /// the one thing does when it does.
    #[test]
    fn only_a_reseed_reaches_a_paused_world() {
        let dt = space_trucking::sim::TICK_DT;
        let mut paused = world();
        paused.frame(
            dt,
            &FrameInput {
                key_pause: true,
                ..FrameInput::default()
            },
        );
        for n in 0..120 {
            paused.frame(dt, &busy_frame(n));
        }
        let unmoved = paused.sim.save_string();

        paused.frame(
            dt,
            &FrameInput {
                menu_reseed: true,
                ..FrameInput::default()
            },
        );
        assert!(
            paused.sim.save_string() != unmoved,
            "a reseed is the one input that crosses the gate, and it did not"
        );
        assert_eq!(
            paused.sim.tick(),
            0,
            "a reseed replaced the world; it must not have advanced one"
        );
        assert!(
            paused.sim.is_paused(),
            "the player who was stopped is still stopped in the new world"
        );
    }

    /// **Fast-forward advances by exactly its multiplier.**
    ///
    /// The other half of the same boundary, opposite sign: the chevrons
    /// under the crosshair say the world is running at
    /// [`space_trucking::sim::WARP_FACTOR`] times speed, and that has to
    /// be the number rather than "faster". A frame is worth one tick at
    /// rest; the same frames warped are worth sixteen each, and neither
    /// the accumulator's leftovers nor the frame clamp is allowed to
    /// round the count.
    #[test]
    fn warp_advances_by_exactly_its_own_multiplier() {
        let dt = space_trucking::sim::TICK_DT;
        let frames = 120;

        let mut rest = world();
        let from = rest.sim.tick();
        for _ in 0..frames {
            rest.frame(dt, &FrameInput::default());
        }
        let plain = rest.sim.tick() - from;
        assert_eq!(plain, frames, "a frame of the sim's own step is one tick");

        let mut fast = world();
        fast.frame(
            dt,
            &FrameInput {
                key_warp: true,
                ..FrameInput::default()
            },
        );
        assert!(
            fast.sim.is_warp(),
            "a fixture world unlocks the warp toggle"
        );
        let from = fast.sim.tick();
        for _ in 0..frames {
            fast.frame(dt, &FrameInput::default());
        }
        assert!(
            fast.sim.is_warp(),
            "nothing was supposed to drop out of warp"
        );
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let multiplier = space_trucking::sim::WARP_FACTOR as u64;
        assert_eq!(
            fast.sim.tick() - from,
            plain * multiplier,
            "warp ran the world at some other speed than its own"
        );
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
            steady: false,
            carry: Carry::default(),
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
            steady: false,
            carry: Carry::default(),
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
            menu_pause: false,
            menu_warp: false,
            menu_mute: false,
            menu_reseed: false,
            occupied: CABIN,
            detach: None,
            turning: Turning::default(),
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

    /// The `Turn` a carry is sent at `d` whole degrees round.
    fn degrees(d: i64) -> Turn {
        Facing(d.rem_euclid(360) * Facing::DEGREE).turn()
    }

    /// **A notch of the wheel turns to the next multiple of fifteen
    /// degrees its way**, from anywhere: from seven, up is fifteen and not
    /// twenty-two, and down is nought and then three hundred and
    /// forty-five. A seventh of a turn, which is about 51.4°, goes up to
    /// sixty and down to forty-five.
    ///
    /// And from a fifteen it is the next one along either way. A piece
    /// set down at fifteen degrees stands at the `Turn` nearest them,
    /// which is a hair past, so lifting it again starts the carry a hair
    /// past fifteen; a notch down from there to fifteen itself would send
    /// the sim the turn it already has, which is a notch that does
    /// nothing, so it goes on to nought.
    #[test]
    fn a_notch_turns_to_the_next_fifteen() {
        let seven = Facing(7 * Facing::DEGREE);
        assert_eq!(seven.stop(1).turn(), degrees(15));
        assert_eq!(seven.stop(1).stop(1).turn(), degrees(30));
        assert_eq!(seven.stop(-1).turn(), degrees(0));
        assert_eq!(seven.stop(-1).stop(-1).turn(), degrees(345));
        let seventh = Facing::of(Turn(9362));
        assert_eq!(seventh.stop(1).turn(), degrees(60));
        assert_eq!(seventh.stop(-1).turn(), degrees(45));
        let lifted = Facing::of(degrees(15));
        assert_ne!(
            lifted,
            Facing(15 * Facing::DEGREE),
            "fifteen is not a whole Turn"
        );
        assert_eq!(lifted.stop(1).turn(), degrees(30));
        assert_eq!(lifted.stop(-1).turn(), degrees(0));
        // Twenty-four of them from a stop are a whole turn, either way.
        let round = |way: i64| (0..24).fold(Facing::default(), |at, _| at.stop(way));
        assert_eq!(round(1), Facing::default());
        assert_eq!(round(-1), Facing::default());
    }

    /// **`Ctrl` turns one degree a notch and snaps nothing.** From a
    /// seventh of a turn a degree on is a degree on, exactly, in the unit
    /// the facing is held in; fifteen of them from nought are fifteen
    /// degrees, the very `Turn` a plain notch lands on, so the notch after
    /// them is thirty; and three hundred and sixty of them come back to
    /// where they started, to the part, which a degree rounded to a
    /// `Turn` unit (182 of them, a shade short) would not.
    #[test]
    fn ctrl_turns_one_degree_and_snaps_nothing() {
        let seventh = Facing::of(Turn(9362));
        assert_eq!(seventh.by(Facing::DEGREE).0 - seventh.0, Facing::DEGREE);
        assert_eq!(seventh.by(-Facing::DEGREE).0 - seventh.0, -Facing::DEGREE);
        let fifteen = (0..15).fold(Facing::default(), |at, _| at.by(Facing::DEGREE));
        assert_eq!(fifteen.turn(), Facing::default().stop(1).turn());
        assert_eq!(fifteen.stop(1).turn(), degrees(30));
        let round = (0..360).fold(seventh, |at, _| at.by(Facing::DEGREE));
        assert_eq!(
            round, seventh,
            "a full turn of degrees came back somewhere else"
        );
        assert_eq!(Facing::default().by(Facing::DEGREE).turn(), Turn(182));
    }

    /// A shell holding a fresh run, with the cabin's first standing piece
    /// lifted off the deck: the shell, the piece, and where it stood.
    fn lifting() -> (Bridge, u32, Vec2) {
        use space_trucking::sim::cargo::Foot;
        use space_trucking::sim::layout;
        use space_trucking::sim::room::Surf;

        let mut bridge = shell(Sim::new(7));
        let sim = &bridge.sim;
        let host = sim.rooms().kind(CABIN).expect("the cabin");
        let piece = *sim
            .pieces()
            .iter()
            .find(|piece| {
                piece.loc.spot().is_some_and(|spot| spot.room == CABIN)
                    && Foot::at(sim.rooms(), piece)
                        .is_some_and(|(_, foot)| foot.chart(host) == Some(Surf::Floor))
            })
            .expect("the starting board stands something on the deck");
        let rect = layout::piece_rect(sim.rooms(), sim.pieces(), &piece);
        let at = Vec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y));
        bridge.frame(
            0.0,
            &FrameInput {
                pointer: at,
                press: true,
                held: true,
                ..FrameInput::default()
            },
        );
        assert_eq!(bridge.sim.held(0).map(|held| held.piece), Some(piece.id));
        (bridge, piece.id, at)
    }

    /// **The carry's facing is the bridge's own, and the drop lands on
    /// it.** A carry starts at the piece's own turn; the hands turn it —
    /// half a notch and half again are one, `Ctrl` turns degrees, `Q`
    /// and its reverse take the plain notch — and nothing turns while the
    /// hand is empty. The release lands the piece at the facing the
    /// frame carried, and the piece lifted again starts its carry there.
    #[test]
    fn the_hands_turn_the_carry_and_the_drop_lands_turned() {
        let (mut bridge, id, home) = lifting();
        let own = lifted_at(&bridge.sim);
        assert_eq!(
            bridge.facing(),
            own,
            "a carry starts at the piece's own turn"
        );
        let carry = |bridge: &mut Bridge, turning: Turning| {
            bridge.frame(
                0.0,
                &FrameInput {
                    pointer: home,
                    held: true,
                    turning,
                    ..FrameInput::default()
                },
            );
            bridge.facing()
        };
        let wheel = |wheel: f32| Turning {
            wheel,
            ..Turning::default()
        };
        let next = Facing::of(own).stop(1).turn();
        assert_eq!(
            carry(&mut bridge, wheel(0.5)),
            own,
            "half a notch is not one"
        );
        assert_eq!(carry(&mut bridge, wheel(0.5)), next, "and half again is");
        let fine = carry(
            &mut bridge,
            Turning {
                wheel: -3.0,
                fine: true,
                ..Turning::default()
            },
        );
        assert_eq!(fine, Facing::of(next).by(-3 * Facing::DEGREE).turn());
        let q = |key: i8| Turning {
            key,
            ..Turning::default()
        };
        assert_eq!(carry(&mut bridge, q(1)), next, "Q is the plain notch up");
        assert_eq!(carry(&mut bridge, q(-1)), own, "and Shift+Q the notch back");
        let back = carry(
            &mut bridge,
            Turning {
                wheel: 7.0,
                fine: true,
                ..Turning::default()
            },
        );
        assert_eq!(back, Facing::of(own).by(7 * Facing::DEGREE).turn());
        assert!(!back.square(), "the carry was meant to end up off square");

        // Let go where it was lifted from: it lands at the turn it was
        // carried at, not the one it was lifted at.
        bridge.frame(
            0.0,
            &FrameInput {
                pointer: home,
                release: true,
                ..FrameInput::default()
            },
        );
        assert!(bridge.sim.held(0).is_none());
        let landed = bridge
            .sim
            .pieces()
            .iter()
            .find(|piece| piece.id == id)
            .and_then(|piece| piece.loc.spot())
            .expect("on the deck");
        assert_eq!(
            landed.turn, back,
            "the drop forgot the facing it was carried at"
        );
        assert_eq!(bridge.facing(), Turn::ZERO, "an empty hand has no facing");
        let still = carry(&mut bridge, wheel(4.0));
        assert_eq!(still, Turn::ZERO, "an empty hand turned something");

        // Lifted again, it carries on at the turn it stands at.
        bridge.frame(
            0.0,
            &FrameInput {
                pointer: home,
                press: true,
                held: true,
                ..FrameInput::default()
            },
        );
        assert_eq!(bridge.sim.held(0).map(|held| held.piece), Some(id));
        assert_eq!(bridge.facing(), back, "a piece lifted again lost its turn");
    }
}
