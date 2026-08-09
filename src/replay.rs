//! The black-box flight recorder: a session as (base save + input log).
//!
//! The sim is a pure function of (state, input sequence) — the sealed-log
//! idea from `docs/NETWORKING.md`, reused offline. A [`Recording`] keeps a
//! starting save (the same save string the autosave writes) plus a sparse,
//! tick-stamped list of the frames that actually said anything, and
//! [`Recording::replay`] rebuilds the sim from the save and re-applies the
//! log. The result is bit-identical to the live session at the same tick —
//! [`Sim::save_string`] equality is the tested contract — so a copied
//! recording attached to a bug report is a perfect reproduction, and a
//! rolling re-base ([`Recording::rebase`]) keeps the black box the story of
//! the recent past.
//!
//! Replay mechanics: each entry is applied at its recorded tick with a
//! zero-dt [`Sim::advance`] — input lands, no time passes — because several
//! frames can share one tick exactly as several ticks can pass in one frame.
//! The ticks between entries run through [`Sim::crew_tick`] as player 0 with
//! a continuation frame: edges and toggles stripped, pointer and hold
//! carried while the last entry was pressing or holding, so a gap the live
//! session never saw cannot snap a mid-drag piece home. The one frame the
//! log keeps *beyond* the flagged ones is the frame right after a press or
//! hold: that is the frame that can silently snap a phantom drag home
//! (window blur mid-drag), and the replay must snap at the same tick.
//!
//! Format `RPL2`, line-oriented like the save and the wire: a header, the
//! byte-length-prefixed base save embedded verbatim, then one `SNP2 input`
//! line per entry — the recorder reuses the lockstep wire codec
//! ([`Message::Input`]) rather than invent a second frame encoding, so
//! pointer floats travel as exact bit patterns. Parsing never panics; every
//! malformed payload maps to a [`ReplayError`], and a recording that would
//! run backwards, stall paused, or grind past [`MAX_REPLAY_TICKS`] is
//! refused rather than looped on.

use std::fmt;
use std::fmt::Write as _;

use crate::net::Message;
use crate::sim::{CrewFrame, InputFrame, SaveError, Sim, Vec2};

/// Magic-plus-version header of every recording this build writes. Bump on
/// any breaking change; older versions fail safe as unsupported.
const MAGIC: &str = "RPL2";

/// Rolling cap on recorded entries.
///
/// The recorder never drops a frame — that would break exactness — so this
/// is a soft cap: the frontend checks [`Recording::is_full`] on its save
/// cadence and calls [`Recording::rebase`], which snapshots the present
/// into a new base and clears the log.
pub const MAX_ENTRIES: usize = 50_000;

/// Most ticks one replay will run before refusing with
/// [`ReplayError::TooLong`].
///
/// An anti-hang guard against hostile tick stamps, not a gameplay limit: it
/// is over three sim-weeks, and a re-based black box stays orders of
/// magnitude under it.
pub const MAX_REPLAY_TICKS: u64 = 100_000_000;

/// Why a recording was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// Not a Space Trucking recording at all.
    BadMagic,
    /// A Space Trucking recording from a version this build does not read.
    UnsupportedVersion,
    /// Recognised header, malformed body; `line` is 1-based (0 = truncated).
    Parse { line: usize },
    /// The embedded base save was refused by the save parser.
    BadSave(SaveError),
    /// Entry ticks run backwards with no reseed to explain them.
    OutOfOrder,
    /// The recording claims ticks passed while the sim was paused.
    Stalled,
    /// Replaying would step more than [`MAX_REPLAY_TICKS`] ticks.
    TooLong,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a Space Trucking recording"),
            Self::UnsupportedVersion => write!(f, "recording is from an unsupported version"),
            Self::Parse { line: 0 } => write!(f, "recording ends too early"),
            Self::Parse { line } => write!(f, "malformed recording at line {line}"),
            Self::BadSave(err) => write!(f, "recording's base save: {err}"),
            Self::OutOfOrder => write!(f, "recording's entries run backwards"),
            Self::Stalled => write!(f, "recording claims ticks passed while paused"),
            Self::TooLong => write!(f, "recording is too long to replay"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Why a replay plan cannot run, from [`plan`]. Internal: [`Recording::parse`]
/// maps these onto line-numbered [`ReplayError::Parse`] values, while
/// [`Recording::replay`] maps them onto the runtime variants.
enum Flaw {
    /// Entry `index` is stamped before the floor its predecessors set.
    Backwards(usize),
    /// The recorded end tick sits before the last entry.
    EndsEarly,
    /// The plan steps more than [`MAX_REPLAY_TICKS`] ticks.
    TooLong,
}

/// Whether a frame carries any input at all. An all-default frame applies as
/// a no-op, so the log can skip it — with the one phantom-drag exception
/// [`Recording::record_frame`] documents.
const fn active(frame: &InputFrame) -> bool {
    frame.press
        || frame.held
        || frame.release
        || frame.toggle_pause
        || frame.toggle_warp
        || frame.reseed.is_some()
}

/// The flight recorder.
///
/// Feed it every frame the live loop feeds the sim (via
/// [`Recording::record_frame`], before the `advance`) and it keeps the
/// sparse log that [`Recording::replay`] turns back into the identical run.
#[derive(Clone, Debug)]
pub struct Recording {
    /// The starting state: a save string, embedded verbatim.
    base: String,
    /// Tick-stamped interesting frames, in the order they were applied.
    /// Several entries may share a tick (frames outnumber ticks at high
    /// frame rates); ticks only rewind across a reseed entry.
    entries: Vec<(u64, InputFrame)>,
    /// The tick the replay must coast to after the last entry, so trailing
    /// input-free travel is part of the story. Kept fresh by every
    /// [`Recording::record_frame`] and by [`Recording::seal`].
    end: u64,
    /// Whether the previous observed frame pressed or held — the lookbehind
    /// that makes the recorder keep the frame a phantom drag snaps back on.
    prev_active: bool,
}

impl Recording {
    /// Start a recording from `base`, a [`Sim::save_string`] of the moment
    /// recording begins.
    #[must_use]
    pub fn new(base: String) -> Self {
        // An empty recording must replay to its base, so the end tick
        // starts at the base's own tick. An unreadable base leaves 0; its
        // replay will refuse with `BadSave` anyway.
        let end = Sim::from_save(&base).map_or(0, |sim| sim.tick());
        Self {
            base,
            entries: Vec::new(),
            end,
            prev_active: false,
        }
    }

    /// Observe one live frame. Call this every loop iteration with the tick
    /// *before* the `advance` that will consume `frame` — the entry stamp is
    /// the tick the input lands on. Uninteresting frames cost nothing but
    /// still push the recording's end tick forward, so input-free cruising
    /// is replayed as the time it was.
    ///
    /// One frame beyond the flagged ones is kept: the frame right after a
    /// press or hold. That is the frame that can snap a phantom drag home
    /// (window blur mid-drag — the release never arrives), and the replay
    /// must snap at the same tick the live sim did.
    pub fn record_frame(&mut self, tick_before_advance: u64, frame: &InputFrame) {
        self.end = tick_before_advance;
        let interesting = active(frame) || self.prev_active;
        self.prev_active = frame.press || frame.held;
        if interesting {
            self.entries.push((tick_before_advance, *frame));
        }
    }

    /// Refresh the recording's end tick without recording anything — call
    /// with [`Sim::tick`] *after* the last `advance` before serialising, so
    /// the tape covers the ticks that frame ran.
    pub const fn seal(&mut self, tick: u64) {
        self.end = tick;
    }

    /// Roll the tape: `save` (a [`Sim::save_string`] taken at `tick`)
    /// becomes the new base and the log clears, so the black box always
    /// holds the recent past. Saves drop drags, so call this only while
    /// nothing is held — a drag spanning the cut would be lost from the
    /// story.
    pub fn rebase(&mut self, save: String, tick: u64) {
        self.base = save;
        self.entries.clear();
        self.end = tick;
    }

    /// Whether the log has reached [`MAX_ENTRIES`] — time for the frontend
    /// to [`Recording::rebase`]. Recording continues past the cap rather
    /// than drop frames.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.entries.len() >= MAX_ENTRIES
    }

    /// Recorded entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entries have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The embedded base save.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The sim at the recording's start.
    pub fn base_sim(&self) -> Result<Sim, ReplayError> {
        Sim::from_save(&self.base).map_err(ReplayError::BadSave)
    }

    /// Serialise for storage. The inverse of [`Recording::parse`]: parsing
    /// the output reproduces base, entries, and end tick bit-exactly.
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail, so the fmt plumbing is dropped.
        let _ = writeln!(out, "{MAGIC}");
        let _ = writeln!(out, "end {}", self.end);
        // Byte-length prefix, so the base survives verbatim whatever its
        // last line looks like; entries begin right after it.
        let _ = writeln!(out, "base {}", self.base.len());
        out.push_str(&self.base);
        for &(tick, frame) in &self.entries {
            // Each entry is literally a lockstep wire message: the same
            // codec, the same exactness guarantees, no second encoding.
            out.push_str(
                &Message::Input {
                    tick,
                    player: 0,
                    frame,
                }
                .to_wire(),
            );
        }
        out
    }

    /// Rebuild a recording from [`Recording::serialize`] output. Validates
    /// shape and schedule: the base must parse as a save, entry ticks must
    /// never run backwards (except across a reseed, which rewinds the
    /// clock), and the whole plan must fit [`MAX_REPLAY_TICKS`].
    pub fn parse(s: &str) -> Result<Self, ReplayError> {
        // The header is split off by hand rather than through `lines()`
        // because the base body must survive byte-exact.
        let (magic, rest) = s.split_once('\n').unwrap_or((s, ""));
        match magic {
            MAGIC => {}
            other if other.starts_with("RPL") => return Err(ReplayError::UnsupportedVersion),
            _ => return Err(ReplayError::BadMagic),
        }
        let (end_line, rest) = rest
            .split_once('\n')
            .ok_or(ReplayError::Parse { line: 0 })?;
        let end: u64 = keyed(end_line, "end").ok_or(ReplayError::Parse { line: 2 })?;
        let (base_line, rest) = rest
            .split_once('\n')
            .ok_or(ReplayError::Parse { line: 0 })?;
        let base_len: usize = keyed(base_line, "base").ok_or(ReplayError::Parse { line: 3 })?;
        // `get` refuses out-of-range and non-char-boundary cuts alike.
        let base = rest.get(..base_len).ok_or(ReplayError::Parse { line: 3 })?;
        let tail = rest.get(base_len..).unwrap_or("");

        let mut entries = Vec::new();
        for (index, line) in tail.lines().enumerate() {
            let at = entry_line(base, index);
            let message = Message::from_wire(line).map_err(|_| ReplayError::Parse { line: at })?;
            let Message::Input {
                tick,
                player: 0,
                frame,
            } = message
            else {
                return Err(ReplayError::Parse { line: at });
            };
            entries.push((tick, frame));
        }

        let base_sim = Sim::from_save(base).map_err(ReplayError::BadSave)?;
        match plan(base_sim.tick(), &entries, end) {
            Ok(_) => {}
            Err(Flaw::Backwards(index)) => {
                return Err(ReplayError::Parse {
                    line: entry_line(base, index),
                });
            }
            Err(Flaw::EndsEarly) => return Err(ReplayError::Parse { line: 2 }),
            Err(Flaw::TooLong) => return Err(ReplayError::TooLong),
        }

        let prev_active = entries
            .last()
            .is_some_and(|(_, frame)| frame.press || frame.held);
        Ok(Self {
            base: base.to_owned(),
            entries,
            end,
            prev_active,
        })
    }

    /// Replay the whole recording and return the resulting sim — the
    /// correctness contract: bit-identical (by [`Sim::save_string`]) to the
    /// live session the log observed, at the same tick.
    pub fn replay(&self) -> Result<Sim, ReplayError> {
        let mut sim = self.base_sim()?;
        // Validate the schedule up front, so a hand-built recording with
        // hostile tick stamps refuses instead of grinding.
        match plan(sim.tick(), &self.entries, self.end) {
            Ok(_) => {}
            Err(Flaw::TooLong) => return Err(ReplayError::TooLong),
            Err(_) => return Err(ReplayError::OutOfOrder),
        }
        let mut cursor = self.cursor();
        while cursor.next_tick(&mut sim)? {}
        Ok(sim)
    }

    /// A stepwise cursor over this recording, for playing it back at real
    /// time. Drive the sim from [`Recording::base_sim`] with one
    /// [`ReplayCursor::next_tick`] per elapsed [`crate::sim::TICK_DT`].
    #[must_use]
    pub fn cursor(&self) -> ReplayCursor<'_> {
        ReplayCursor {
            entries: &self.entries,
            end: self.end,
            index: 0,
            primed: false,
            carry: None,
            pointer: Vec2::default(),
            steps: 0,
        }
    }
}

/// A replay in progress: how far into the entry log the sim has advanced,
/// and what the continuation frame currently carries.
#[derive(Debug)]
pub struct ReplayCursor<'a> {
    entries: &'a [(u64, InputFrame)],
    end: u64,
    /// Next entry to apply.
    index: usize,
    /// Whether the entries due at the base tick have been applied yet.
    primed: bool,
    /// Pointer to carry while the last entry pressed or held, so gap ticks
    /// cannot snap a live drag home.
    carry: Option<Vec2>,
    /// The recorded player's last seen pointer, for the frontend to draw.
    pointer: Vec2,
    /// Ticks stepped so far, bounding runaway replays.
    steps: u64,
}

impl ReplayCursor<'_> {
    /// Advance the replay by one tick: apply every entry due at the current
    /// tick (several frames can share one), then run one fixed step with the
    /// continuation frame. Returns `Ok(false)` once the recording's end tick
    /// is reached — the sim then holds the final state.
    ///
    /// The first call applies the entries due at the base tick without
    /// stepping, so tick zero's input is not skipped.
    pub fn next_tick(&mut self, sim: &mut Sim) -> Result<bool, ReplayError> {
        if !self.primed {
            self.primed = true;
            self.apply_due(sim)?;
            return Ok(!self.finished(sim));
        }
        if self.finished(sim) {
            return Ok(false);
        }
        if sim.is_paused() {
            // Ticks cannot pass while paused; entries during a live pause
            // share its tick and were drained on arrival, so needing a step
            // here means the recording lies.
            return Err(ReplayError::Stalled);
        }
        if self.steps >= MAX_REPLAY_TICKS {
            return Err(ReplayError::TooLong);
        }
        self.steps += 1;
        sim.crew_tick(&self.neutral());
        self.apply_due(sim)?;
        Ok(!self.finished(sim))
    }

    /// The recorded player's most recent pointer position, for the replay
    /// frontend to render as the ghost cursor.
    #[must_use]
    pub const fn pointer(&self) -> Vec2 {
        self.pointer
    }

    /// Apply every entry stamped with the sim's current tick, in recorded
    /// order, without letting time pass — a zero-dt `advance` applies input
    /// and runs no step. A reseed entry rewinds the clock to zero and the
    /// loop simply continues from there.
    fn apply_due(&mut self, sim: &mut Sim) -> Result<(), ReplayError> {
        while let Some(&(tick, frame)) = self.entries.get(self.index) {
            if tick > sim.tick() {
                break;
            }
            if tick < sim.tick() {
                return Err(ReplayError::OutOfOrder);
            }
            sim.advance(0.0, &frame);
            self.carry = (frame.press || frame.held).then_some(frame.pointer);
            self.pointer = frame.pointer;
            self.index += 1;
        }
        Ok(())
    }

    /// Whether every entry is applied and the end tick reached.
    const fn finished(&self, sim: &Sim) -> bool {
        self.index >= self.entries.len() && sim.tick() >= self.end
    }

    /// The continuation crew frame for a gap tick: all defaults, except that
    /// player 0 keeps holding at the carried pointer while the last entry
    /// pressed or held — the live session ran these ticks inside one
    /// `advance`, where no input event could interrupt the drag.
    fn neutral(&self) -> CrewFrame {
        let mut frames = CrewFrame::default();
        if let Some(pointer) = self.carry {
            frames[0] = InputFrame {
                pointer,
                held: true,
                ..InputFrame::default()
            };
        }
        frames
    }
}

/// One `key <value>` header line, lenient about extra whitespace like the
/// save reader is.
fn keyed<T: std::str::FromStr>(line: &str, key: &str) -> Option<T> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some(key) {
        return None;
    }
    tokens.next()?.parse().ok()
}

/// 1-based line number of entry `index` in the serialized text: three header
/// lines, the base's own lines, then the entries.
fn entry_line(base: &str, index: usize) -> usize {
    4 + base.bytes().filter(|&b| b == b'\n').count() + index
}

/// Walk the schedule and count the ticks a replay must step. Entry ticks
/// must never precede the floor their predecessors set; a reseed entry
/// rewinds the floor to zero because the sim's clock restarts. The total
/// must fit [`MAX_REPLAY_TICKS`].
fn plan(base_tick: u64, entries: &[(u64, InputFrame)], end: u64) -> Result<u64, Flaw> {
    let mut floor = base_tick;
    let mut steps: u64 = 0;
    for (index, (tick, frame)) in entries.iter().enumerate() {
        if *tick < floor {
            return Err(Flaw::Backwards(index));
        }
        steps = steps.saturating_add(tick - floor);
        floor = if frame.reseed.is_some() { 0 } else { *tick };
    }
    if end < floor {
        return Err(Flaw::EndsEarly);
    }
    steps = steps.saturating_add(end - floor);
    if steps > MAX_REPLAY_TICKS {
        return Err(Flaw::TooLong);
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Cue, ShipState, layout, poi_pos, splitmix};

    /// Mars, the scripted session's destination.
    const URANUS: u8 = 4;

    /// Seed whose first Guild trade is acceptable (the sim tests' odyssey
    /// seed), so the script can pull the accept lever meaningfully.
    const SEED: u64 = 1;

    /// Prime-millisecond frame times, so frames and ticks never line up.
    const PRIMES: [f32; 8] = [0.013, 0.017, 0.019, 0.023, 0.029, 0.031, 0.037, 0.041];

    fn press_at(x: f32, y: f32) -> InputFrame {
        InputFrame {
            pointer: Vec2::new(x, y),
            press: true,
            held: true,
            ..InputFrame::default()
        }
    }

    fn held_at(x: f32, y: f32) -> InputFrame {
        InputFrame {
            pointer: Vec2::new(x, y),
            held: true,
            ..InputFrame::default()
        }
    }

    fn release_at(x: f32, y: f32) -> InputFrame {
        InputFrame {
            pointer: Vec2::new(x, y),
            release: true,
            ..InputFrame::default()
        }
    }

    fn rect_center(rect: layout::Rect) -> Vec2 {
        Vec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y))
    }

    fn cell_center(x: u8, y: u8) -> Vec2 {
        rect_center(layout::cell_rect(x, y))
    }

    fn slot_center(slots: &[layout::Rect; 4], i: usize) -> Vec2 {
        rect_center(slots[i])
    }

    /// A drag with deliberately awkward frame times: press, a multi-tick
    /// mid-drag hold, a zero-dt hold, release.
    fn drag(s: &mut Vec<(f32, InputFrame)>, from: Vec2, to: Vec2) {
        s.push((0.0, press_at(from.x, from.y)));
        s.push((0.101, held_at(to.x, to.y)));
        s.push((0.0, held_at(to.x, to.y)));
        s.push((0.013, release_at(to.x, to.y)));
    }

    /// `n` input-free frames cycling the prime frame times.
    fn coast(s: &mut Vec<(f32, InputFrame)>, n: usize) {
        for i in 0..n {
            s.push((PRIMES[i % PRIMES.len()], InputFrame::default()));
        }
    }

    /// The live session the exactness contract is tested against: awkward
    /// frame times throughout, a phantom drag (hold that ends with no
    /// release), a click-in-place, the full odyssey trade, a pause held
    /// mid-drag, zero-dt bursts, warp with a pause inside it, and a docking
    /// spanned mid-warp. Returns the script and a coasting index where a
    /// re-base is safe (nothing held).
    fn thorough_script() -> (Vec<(f32, InputFrame)>, usize) {
        let vial = cell_center(3, 3);
        let scrap = cell_center(4, 5);
        let pearls = cell_center(6, 3);
        let accept = rect_center(layout::ACCEPT_LEVER);
        let launch = rect_center(layout::LAUNCH_LEVER);
        let mars = poi_pos(URANUS, 0);
        let pause = |p: Vec2, held| InputFrame {
            pointer: p,
            held,
            toggle_pause: true,
            ..InputFrame::default()
        };
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };

        let mut s = Vec::new();
        coast(&mut s, 5);
        // A phantom drag: the hold just stops (window blur), no release.
        s.push((0.013, press_at(vial.x, vial.y)));
        s.push((0.017, held_at(vial.x - 20.0, vial.y - 10.0)));
        s.push((0.019, InputFrame::default()));
        coast(&mut s, 3);
        // A click-in-place: press, hold, and release in a single frame.
        s.push((
            0.0,
            InputFrame {
                pointer: scrap,
                press: true,
                held: true,
                release: true,
                ..InputFrame::default()
            },
        ));
        // The odyssey trade: all three starter pieces to the give pads, the
        // first shelf good to the take pad.
        drag(&mut s, vial, slot_center(&layout::GIVE_SLOTS, 0));
        drag(&mut s, scrap, slot_center(&layout::GIVE_SLOTS, 1));
        drag(&mut s, pearls, slot_center(&layout::GIVE_SLOTS, 2));
        drag(
            &mut s,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        // Pause toggled mid-drag and released after: pick up the second
        // shelf good, pause while holding it, wait, resume, put it back.
        let shelf1 = slot_center(&layout::SHELF_SLOTS, 1);
        s.push((0.013, press_at(shelf1.x, shelf1.y)));
        s.push((0.017, pause(shelf1, true)));
        s.push((0.019, held_at(shelf1.x + 5.0, shelf1.y)));
        s.push((0.023, held_at(shelf1.x - 5.0, shelf1.y)));
        s.push((0.029, pause(shelf1, true)));
        s.push((0.0, release_at(shelf1.x, shelf1.y)));
        // Let the dial swing, pull accept, select Mars, bump the launch
        // gate (received still loaded), stow, and launch for real.
        coast(&mut s, 30);
        s.push((0.0, press_at(accept.x, accept.y)));
        s.push((0.017, press_at(mars.x, mars.y)));
        s.push((0.0, press_at(launch.x, launch.y)));
        drag(
            &mut s,
            slot_center(&layout::RECEIVED_SLOTS, 0),
            cell_center(3, 7),
        );
        s.push((0.013, press_at(launch.x, launch.y)));
        // Traveling: a re-base-safe stretch, then a zero-dt burst of
        // no-op presses (several entries on one tick).
        coast(&mut s, 10);
        let rebase_at = s.len() - 1;
        s.push((0.0, press_at(400.0, 555.0)));
        s.push((0.0, release_at(400.0, 555.0)));
        // Warp most of the leg, pause briefly inside the warp, and let the
        // docking land mid-warp; then drop out of warp and coast in.
        s.push((0.0, warp));
        for i in 0..60 {
            s.push((PRIMES[i % PRIMES.len()], InputFrame::default()));
        }
        s.push((0.017, pause(Vec2::default(), false)));
        s.push((0.023, InputFrame::default()));
        s.push((0.019, pause(Vec2::default(), false)));
        // Enough warped 0.031 s frames (just under 30 ticks each) to carry
        // the whole leg; the leg length is read off the sky near departure.
        let leg = crate::sim::map::leg_ticks(crate::sim::GUILD, URANUS, 60);
        for _ in 0..leg / 29 + 60 {
            s.push((0.031, InputFrame::default()));
        }
        // No closing warp toggle: the dock disengages warp itself now, and
        // that auto-off is exactly what this script wants on tape.
        coast(&mut s, 40);
        (s, rebase_at)
    }

    /// Drive a live sim through `script` exactly as the frontend would,
    /// recording each frame before its advance, and seal the tape.
    fn record_session(seed: u64, script: &[(f32, InputFrame)]) -> (Sim, Recording, Vec<Cue>) {
        let mut sim = Sim::new(seed);
        let mut recording = Recording::new(sim.save_string());
        let mut cues = Vec::new();
        for (dt, frame) in script {
            recording.record_frame(sim.tick(), frame);
            sim.advance(*dt, frame);
            cues.extend_from_slice(sim.cues());
        }
        recording.seal(sim.tick());
        (sim, recording, cues)
    }

    /// How many cues in `log` match `pred`.
    fn count_cues(log: &[Cue], pred: impl Fn(&Cue) -> bool) -> usize {
        log.iter().filter(|cue| pred(cue)).count()
    }

    /// The load-bearing test: a live session full of awkward frame times,
    /// drags, toggles, and a docking replays bit-identically from the
    /// recording — through the in-memory log, and again through the
    /// serialized text.
    #[test]
    fn recorded_live_session_replays_bit_identically() {
        let (script, _) = thorough_script();
        let (live, recording, cues) = record_session(SEED, &script);
        // The choreography really happened.
        assert_eq!(live.ship().state, ShipState::Docked(URANUS));
        assert_eq!(count_cues(&cues, |c| matches!(c, Cue::Accept { .. })), 1);
        assert_eq!(count_cues(&cues, |c| matches!(c, Cue::Depart)), 1);
        assert_eq!(count_cues(&cues, |c| matches!(c, Cue::Arrive)), 1);
        assert_eq!(count_cues(&cues, |c| matches!(c, Cue::Warp { .. })), 2);
        assert_eq!(count_cues(&cues, |c| matches!(c, Cue::Pause { .. })), 4);
        // The log is sparse: coasting frames were skipped.
        assert!(
            recording.len() < script.len(),
            "{} entries for {} frames — nothing was skipped",
            recording.len(),
            script.len()
        );

        let replayed = recording.replay().expect("own recording must replay");
        assert_eq!(replayed.tick(), live.tick());
        assert_eq!(replayed.save_string(), live.save_string());
        assert_eq!(replayed.pieces(), live.pieces());
        assert_eq!(replayed.ship(), live.ship());
        assert_eq!(replayed.barter(), live.barter());

        // And byte-exactly through the text form.
        let text = recording.serialize();
        let parsed = Recording::parse(&text).expect("own text must parse");
        assert_eq!(parsed.serialize(), text, "re-serialisation drifted");
        let reparsed = parsed.replay().expect("parsed recording must replay");
        assert_eq!(reparsed.save_string(), live.save_string());
    }

    /// The phantom-drag frame (hold ends, no release) is kept and replayed:
    /// the piece snaps home at the same tick, and a later grab of the same
    /// piece behaves identically.
    #[test]
    fn interrupted_drag_snaps_back_on_replay() {
        let vial = cell_center(0, 0);
        let mut script = vec![
            (0.013, press_at(vial.x, vial.y)),
            (0.017, held_at(vial.x + 30.0, vial.y)),
            // The blur: hold simply stops. Live snaps the drag home here.
            (0.019, InputFrame::default()),
        ];
        coast(&mut script, 5);
        // Prove the state stayed in sync: grab the same piece again.
        script.push((0.0, press_at(vial.x, vial.y)));
        script.push((0.023, release_at(vial.x, vial.y)));
        coast(&mut script, 3);
        let (live, recording, _) = record_session(SEED, &script);
        assert!(live.held(0).is_none(), "the blur must end the live drag");
        let replayed = recording.replay().expect("recording must replay");
        assert_eq!(replayed.save_string(), live.save_string());
    }

    /// Pointer floats survive as exact bit patterns, NaN payloads included —
    /// the entry codec is the lockstep wire codec.
    #[test]
    fn thorny_pointer_bits_survive_the_wire() {
        let thorny = Vec2::new(f32::from_bits(0x7FC0_1234), f32::NEG_INFINITY);
        let mut script = vec![
            (0.013, press_at(thorny.x, thorny.y)),
            (0.0, release_at(thorny.x, thorny.y)),
        ];
        coast(&mut script, 4);
        let (live, recording, _) = record_session(SEED, &script);
        let text = recording.serialize();
        let parsed = Recording::parse(&text).expect("thorny floats must parse");
        assert_eq!(parsed.serialize(), text);
        let replayed = parsed.replay().expect("thorny floats must replay");
        assert_eq!(replayed.save_string(), live.save_string());
    }

    /// A recording re-based mid-session replays to the same final state as
    /// the uncut one — the black box loses history, never truth.
    #[test]
    fn rebase_mid_session_replays_to_the_same_state() {
        let (script, rebase_at) = thorough_script();
        let (live, uncut, _) = record_session(SEED, &script);

        let mut sim = Sim::new(SEED);
        let mut rebased = Recording::new(sim.save_string());
        for (i, (dt, frame)) in script.iter().enumerate() {
            rebased.record_frame(sim.tick(), frame);
            sim.advance(*dt, frame);
            if i == rebase_at {
                assert!(
                    sim.held(0).is_none(),
                    "the re-base point must not interrupt a drag"
                );
                rebased.rebase(sim.save_string(), sim.tick());
            }
        }
        rebased.seal(sim.tick());

        assert!(rebased.len() < uncut.len(), "the re-base must shed entries");
        let a = uncut.replay().expect("uncut recording must replay");
        let b = rebased.replay().expect("re-based recording must replay");
        assert_eq!(a.save_string(), live.save_string());
        assert_eq!(b.save_string(), live.save_string());
    }

    /// A reseed inside a recording replays exactly: the clock rewinds with
    /// the world and the entries continue on the new run's ticks. (The
    /// frontend starts a fresh recording on reseed; the lib still has to
    /// get the general case right.)
    #[test]
    fn reseed_mid_recording_replays_exactly() {
        let vial = cell_center(0, 0);
        let mut script = Vec::new();
        coast(&mut script, 6);
        script.push((
            0.017,
            InputFrame {
                reseed: Some(0xFEED),
                ..InputFrame::default()
            },
        ));
        coast(&mut script, 4);
        // Interact with the new world to prove the log stayed aligned.
        script.push((0.0, press_at(vial.x, vial.y)));
        script.push((0.019, held_at(vial.x + 40.0, vial.y + 20.0)));
        script.push((0.013, release_at(vial.x, vial.y)));
        coast(&mut script, 5);
        let (live, recording, _) = record_session(SEED, &script);
        assert_eq!(live.seed(), 0xFEED, "the reseed must have landed");
        let replayed = recording.replay().expect("reseed recording must replay");
        assert_eq!(replayed.save_string(), live.save_string());
        // And through the text form, whose validation must allow the
        // rewound ticks.
        let parsed = Recording::parse(&recording.serialize()).expect("reseed text must parse");
        assert_eq!(
            parsed
                .replay()
                .expect("parsed reseed must replay")
                .save_string(),
            live.save_string()
        );
    }

    /// An entry-free recording replays to exactly its base.
    #[test]
    fn empty_recording_replays_to_its_base() {
        let mut sim = Sim::new(7);
        sim.fast_forward(123);
        let base = sim.save_string();
        let recording = Recording::new(base.clone());
        assert!(recording.is_empty());
        let replayed = recording.replay().expect("empty recording must replay");
        assert_eq!(replayed.save_string(), base);
        let parsed = Recording::parse(&recording.serialize()).expect("empty text must parse");
        assert_eq!(
            parsed
                .replay()
                .expect("parsed empty must replay")
                .save_string(),
            base
        );
    }

    /// Every line after the embedded base is a plain lockstep wire message —
    /// the reuse is literal, not stylistic.
    #[test]
    fn entry_lines_reuse_the_lockstep_wire_format() {
        let (script, _) = thorough_script();
        let (_, recording, _) = record_session(SEED, &script);
        assert!(!recording.is_empty());
        let text = recording.serialize();
        let base_lines = recording.base().lines().count();
        for line in text.lines().skip(3 + base_lines) {
            match Message::from_wire(line) {
                Ok(Message::Input { player: 0, .. }) => {}
                other => panic!("entry line {line:?} is not a wire input: {other:?}"),
            }
        }
    }

    /// Truncation at every char boundary either refuses or yields a shorter
    /// recording that re-serialises to exactly the truncated text; nothing
    /// panics.
    #[test]
    fn truncation_at_every_boundary_fails_safe() {
        let vial = cell_center(0, 0);
        let mut script = vec![
            (0.013, press_at(vial.x, vial.y)),
            (0.017, release_at(vial.x, vial.y)),
        ];
        coast(&mut script, 6);
        let (_, recording, _) = record_session(SEED, &script);
        let text = recording.serialize();
        assert!(Recording::parse(&text).is_ok(), "the whole text parses");
        for (i, _) in text.char_indices() {
            if let Ok(parsed) = Recording::parse(&text[..i]) {
                // A cut that lands on an entry boundary is a legally
                // shorter recording; it must still round-trip byte-exactly.
                // (A cut that only shaved the final newline re-grows it:
                // `lines` is lenient about the last line's terminator.)
                let round = parsed.serialize();
                assert!(
                    round == text[..i] || round == format!("{}\n", &text[..i]),
                    "truncation at {i} parsed into something else"
                );
            }
        }
    }

    /// Any line replaced by garbage refuses; headers, base lines, and entry
    /// lines alike. The one legitimate survivor: deleting the final entry
    /// line outright just makes a shorter recording, which must still parse.
    #[test]
    fn mangling_any_line_fails_safe() {
        let vial = cell_center(0, 0);
        let mut script = vec![
            (0.013, press_at(vial.x, vial.y)),
            (0.017, release_at(vial.x, vial.y)),
        ];
        coast(&mut script, 4);
        let (_, recording, _) = record_session(SEED, &script);
        let text = recording.serialize();
        let lines: Vec<&str> = text.lines().collect();
        for target in 0..lines.len() {
            for garbage in ["", "!!! ???", "seed NaN", "\u{1F680}"] {
                let mangled: String = lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| if i == target { garbage } else { line })
                    .collect::<Vec<_>>()
                    .join("\n");
                let result = Recording::parse(&mangled);
                if target == lines.len() - 1 && garbage.is_empty() {
                    assert!(result.is_ok(), "dropping the last entry must parse");
                } else {
                    assert!(
                        result.is_err(),
                        "line {target} replaced by {garbage:?} parsed anyway"
                    );
                }
            }
        }
    }

    /// Structural refusals: wrong magic, future versions, foreign players,
    /// backwards ticks, a lying end tick, an oversized base length.
    #[test]
    fn malformed_recordings_refuse() {
        assert!(matches!(Recording::parse(""), Err(ReplayError::BadMagic)));
        assert!(matches!(
            Recording::parse("XXX\nend 0\nbase 0\n"),
            Err(ReplayError::BadMagic)
        ));
        assert!(matches!(
            Recording::parse("RPL9\nend 0\nbase 0\n"),
            Err(ReplayError::UnsupportedVersion)
        ));
        assert!(matches!(
            Recording::parse("RPL2"),
            Err(ReplayError::Parse { line: 0 })
        ));
        assert!(matches!(
            Recording::parse("RPL2\nend NaN\nbase 0\n"),
            Err(ReplayError::Parse { line: 2 })
        ));
        assert!(matches!(
            Recording::parse("RPL2\nend 0\nbase 99999999999999999999999\n"),
            Err(ReplayError::Parse { line: 3 })
        ));

        let base = Sim::new(1).save_string();
        let entry = |tick: u64, player| {
            Message::Input {
                tick,
                player,
                frame: InputFrame::default(),
            }
            .to_wire()
        };
        let build = |end: u64, entries: &str| {
            format!("RPL2\nend {end}\nbase {}\n{base}{entries}", base.len())
        };
        // A player other than 0 has no business in a solo black box.
        assert!(Recording::parse(&build(5, &entry(1, 3))).is_err());
        // Ticks running backwards with no reseed.
        let backwards = format!("{}{}", entry(10, 0), entry(5, 0));
        assert!(Recording::parse(&build(20, &backwards)).is_err());
        // An end tick before the last entry.
        assert!(Recording::parse(&build(3, &entry(9, 0))).is_err());
        // A schedule past the replay ceiling.
        assert!(matches!(
            Recording::parse(&build(MAX_REPLAY_TICKS + 1, "")),
            Err(ReplayError::TooLong)
        ));
    }

    /// Deterministic fuzz over a spicy alphabet: garbage never panics, and
    /// anything that happens to parse re-serialises without panicking.
    #[test]
    fn arbitrary_garbage_never_panics() {
        let alphabet: Vec<char> = "RPL2 SNP\nend base input 0-9abcdefx \u{FFFD}\u{1F680}\t"
            .chars()
            .collect();
        for round in 0_u64..300 {
            let len = (splitmix(round, 0) % 160) as usize;
            let s: String = (0..len)
                .map(|i| alphabet[(splitmix(round, 1 + i as u64) as usize) % alphabet.len()])
                .collect();
            if let Ok(recording) = Recording::parse(&s) {
                let _ = recording.serialize();
            }
        }
    }

    /// A base that is not a save refuses with `BadSave`, in memory and
    /// through the text form.
    #[test]
    fn bad_base_refuses_with_bad_save() {
        let recording = Recording::new("not a save at all".to_owned());
        assert!(matches!(recording.replay(), Err(ReplayError::BadSave(_))));
        assert!(matches!(
            Recording::parse(&recording.serialize()),
            Err(ReplayError::BadSave(_))
        ));
    }

    /// Hand-built recordings with impossible schedules refuse instead of
    /// hanging: ticks demanded while paused, and entries out of order.
    #[test]
    fn impossible_schedules_refuse() {
        // Paused base, but the log claims a press five ticks later.
        let mut sim = Sim::new(3);
        sim.advance(
            0.0,
            &InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        assert!(sim.is_paused());
        let mut stalled = Recording::new(sim.save_string());
        stalled.record_frame(sim.tick() + 5, &press_at(10.0, 10.0));
        stalled.seal(sim.tick() + 5);
        assert!(matches!(stalled.replay(), Err(ReplayError::Stalled)));

        // Entries running backwards in memory.
        let mut backwards = Recording::new(Sim::new(3).save_string());
        backwards.record_frame(10, &press_at(10.0, 10.0));
        backwards.record_frame(5, &press_at(10.0, 10.0));
        backwards.seal(20);
        assert!(matches!(backwards.replay(), Err(ReplayError::OutOfOrder)));

        // An end tick past the replay ceiling.
        let mut endless = Recording::new(Sim::new(3).save_string());
        endless.seal(MAX_REPLAY_TICKS + 1);
        assert!(matches!(endless.replay(), Err(ReplayError::TooLong)));
    }

    #[test]
    fn errors_display_without_panicking() {
        assert_eq!(
            ReplayError::BadMagic.to_string(),
            "not a Space Trucking recording"
        );
        assert_eq!(
            ReplayError::Parse { line: 0 }.to_string(),
            "recording ends too early"
        );
        assert_eq!(
            ReplayError::Parse { line: 7 }.to_string(),
            "malformed recording at line 7"
        );
        assert_eq!(
            ReplayError::BadSave(SaveError::BadMagic).to_string(),
            "recording's base save: not a Space Trucking save"
        );
        assert!(!ReplayError::OutOfOrder.to_string().is_empty());
        assert!(!ReplayError::Stalled.to_string().is_empty());
        assert!(!ReplayError::TooLong.to_string().is_empty());
        assert!(!ReplayError::UnsupportedVersion.to_string().is_empty());
    }
}
