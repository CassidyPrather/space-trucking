//! Opt-in play statistics: the `TLM1` buffer behind `docs/TELEMETRY.md`.
//!
//! The aggregator consumes the same [`Cue`] stream and coarse public state
//! the audio does — nothing here reaches into the sim's internals, and no
//! telemetry code is reachable *from* `src/sim/` or `src/net/`. What it
//! counts is exactly the closed schema table in `docs/TELEMETRY.md`, no
//! more and no less; adding a row is a design-review question answered in
//! that file first. Everything is deliberately blunt: seconds accumulate
//! fractionally in memory but serialise whole, trade values bucket to
//! quartiles, and nothing stored is a coordinate, a timestamp, or any
//! number precise enough to fingerprint.
//!
//! The buffer is one human-readable string under one storage key
//! ([`BUFFER_KEY`]), so anyone can read in their own storage exactly what
//! would ever be shared. [`Aggregate::merge`] is plain summed counters —
//! associative and commutative — so a future upload (same-origin, batched,
//! documented in `docs/TELEMETRY.md` first; none exists today) could batch
//! buffers without double-count anxiety.
//!
//! Consent gating, storage, and deletion-on-decline live in the frontend:
//! `src/main.rs` reads the consent key the web shell's card writes and
//! constructs an [`Aggregate`] only on a recorded `"yes"`. This module is
//! pure and macroquad-free so every row is testable. Parsing never panics:
//! a malformed buffer maps to [`TelemetryError`] and the caller starts
//! fresh.

use std::fmt;
use std::fmt::Write as _;

use crate::sim::{Cue, Loc, ShipState, Sim};

/// Storage key for the recorded consent choice, `"yes"` / `"no"`. Written
/// by the web shell's consent card; native builds have no card, so the key
/// is absent there and telemetry stays off.
pub const CONSENT_KEY: &str = "space-trucking/telemetry-consent";

/// Storage key for the `TLM1` buffer.
pub const BUFFER_KEY: &str = "space-trucking/telemetry";

/// Magic-plus-version header of every buffer this build writes. Bump on any
/// schema change; older versions fail safe as unsupported and start fresh.
const MAGIC: &str = "TLM1";

/// Longest single frame the clocks will believe, in seconds. A backgrounded
/// tab hands the frontend an enormous frame dt on return; the play it is
/// supposed to measure did not happen during it.
const MAX_FRAME: f32 = 1.0;

/// Why a telemetry buffer was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryError {
    /// Not a Space Trucking telemetry buffer at all.
    BadMagic,
    /// A buffer from a version this build does not read.
    UnsupportedVersion,
    /// Recognised header, malformed body; `line` is 1-based (0 = truncated).
    Parse { line: usize },
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a Space Trucking telemetry buffer"),
            Self::UnsupportedVersion => {
                write!(f, "telemetry buffer is from an unsupported version")
            }
            Self::Parse { line: 0 } => write!(f, "telemetry buffer ends too early"),
            Self::Parse { line } => write!(f, "malformed telemetry buffer at line {line}"),
        }
    }
}

impl std::error::Error for TelemetryError {}

/// The coarse public state one frame of aggregation reads — the same
/// altitude the audio flies at.
///
/// Take it off the sim *after* the `advance` whose cues are being observed;
/// [`Snapshot::of`] does exactly that.
// Five bools, but they are a fixed observation record, not a state machine
// in disguise — the same plea `InputFrame` makes.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    /// The ship is between stations.
    pub traveling: bool,
    /// The ship is docked.
    pub docked: bool,
    /// Warp is engaged.
    pub warp: bool,
    /// Ticking is suspended.
    pub paused: bool,
    /// Whether anything sits on the received shelf. The trade/gift split
    /// hangs on this: the accept ceremony refuses while the received shelf
    /// is occupied and moves the take pad onto it, so on the frame a
    /// [`Cue::Accept`] fires, an occupied received shelf means something
    /// was taken (a trade) and an empty one means the take pad was empty
    /// (a gift).
    pub received_occupied: bool,
}

impl Snapshot {
    /// Read the coarse state off the sim, post-`advance`.
    #[must_use]
    pub fn of(sim: &Sim) -> Self {
        Self {
            traveling: matches!(sim.ship().state, ShipState::Traveling { .. }),
            docked: matches!(sim.ship().state, ShipState::Docked(_)),
            warp: sim.is_warp(),
            paused: sim.is_paused(),
            received_occupied: sim
                .pieces()
                .iter()
                .any(|piece| matches!(piece.loc, Loc::ReceivedShelf { .. })),
        }
    }
}

/// The whole schema — every row of the table in `docs/TELEMETRY.md`, and
/// nothing else.
///
/// Clocks are `f64` seconds internally and whole seconds on the wire;
/// everything else is a plain counter. The state clocks overlap
/// deliberately: a paused, docked ship accrues `seconds`, `docked_seconds`,
/// and `paused_seconds` at once, because each row answers its own question.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aggregate {
    /// Boots with consent present.
    sessions: u64,
    /// Coarse time with the game open.
    seconds: f64,
    /// Coarse time in each state; travel and docked partition `seconds`,
    /// warp and paused overlap it.
    travel_seconds: f64,
    docked_seconds: f64,
    warp_seconds: f64,
    paused_seconds: f64,
    /// Departures.
    legs: u64,
    /// Dockings.
    arrivals: u64,
    /// Cargo handling volume.
    places: u64,
    pickups: u64,
    /// Friction signals; hard means a stowage rule refused an in-grid drop.
    rejects_soft: u64,
    rejects_hard: u64,
    /// Accepts, split by whether the take pad was empty (see
    /// [`Snapshot::received_occupied`] for how that is observed).
    trades: u64,
    gifts: u64,
    /// Accepts bucketed by generosity quartile.
    trade_value: [u64; 4],
    /// Lever pulls the station declined.
    refusals: u64,
    /// Hangar deliveries.
    deliveries: u64,
    /// Event exposure.
    omens: u64,
    jumps: u64,
    /// Fresh runs started.
    reseeds: u64,
    /// Absence replayed at boot.
    catch_up_seconds: f64,
}

impl Aggregate {
    /// Count one boot with consent present. Call once per session, at boot.
    pub const fn note_session(&mut self) {
        bump(&mut self.sessions);
    }

    /// Count the absence the startup catch-up actually replayed. Junk —
    /// negative or non-finite — counts as nothing.
    pub fn note_catch_up(&mut self, seconds: f64) {
        if seconds.is_finite() {
            self.catch_up_seconds += seconds.max(0.0);
        }
    }

    /// Fold one frame in: `cues` from the `advance` that just ran, `dt` the
    /// frame's wall-clock seconds, `at` the sim's coarse state after that
    /// advance. Cues not in the schema table are not collected.
    pub fn observe(&mut self, cues: &[Cue], dt: f32, at: Snapshot) {
        // A hostile or broken dt (a backgrounded tab's giant frame, NaN
        // from a stalled clock) must not poison the clocks: non-finite
        // counts as nothing and one frame counts at most MAX_FRAME.
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, MAX_FRAME))
        } else {
            0.0
        };
        self.seconds += dt;
        if at.traveling {
            self.travel_seconds += dt;
        }
        if at.docked {
            self.docked_seconds += dt;
        }
        if at.warp {
            self.warp_seconds += dt;
        }
        if at.paused {
            self.paused_seconds += dt;
        }
        for cue in cues {
            match *cue {
                Cue::Depart => bump(&mut self.legs),
                Cue::Arrive => bump(&mut self.arrivals),
                Cue::Place => bump(&mut self.places),
                Cue::Pickup => bump(&mut self.pickups),
                Cue::Reject { hard: false } => bump(&mut self.rejects_soft),
                Cue::Reject { hard: true } => bump(&mut self.rejects_hard),
                Cue::Accept { value } => {
                    bump(if at.received_occupied {
                        &mut self.trades
                    } else {
                        &mut self.gifts
                    });
                    bump(&mut self.trade_value[bucket(value)]);
                }
                Cue::Refuse => bump(&mut self.refusals),
                Cue::Delivered => bump(&mut self.deliveries),
                Cue::OmenStart => bump(&mut self.omens),
                Cue::Jump => bump(&mut self.jumps),
                Cue::Reseed => bump(&mut self.reseeds),
                // Not in the schema table, so not collected.
                Cue::Select
                | Cue::OmenEnd
                | Cue::Creak { .. }
                | Cue::RatAboard
                | Cue::RatSkitter { .. }
                | Cue::RatNibble
                | Cue::RatChased
                | Cue::RatLeft
                | Cue::Pause { .. }
                | Cue::Warp { .. } => {}
            }
        }
    }

    /// Sum `other` into this buffer: plain summed counters, associative and
    /// commutative (saturating at the `u64` ceiling, which no honest buffer
    /// approaches), so batching order can never double-count.
    pub fn merge(&mut self, other: &Self) {
        self.sessions = self.sessions.saturating_add(other.sessions);
        self.seconds += other.seconds;
        self.travel_seconds += other.travel_seconds;
        self.docked_seconds += other.docked_seconds;
        self.warp_seconds += other.warp_seconds;
        self.paused_seconds += other.paused_seconds;
        self.legs = self.legs.saturating_add(other.legs);
        self.arrivals = self.arrivals.saturating_add(other.arrivals);
        self.places = self.places.saturating_add(other.places);
        self.pickups = self.pickups.saturating_add(other.pickups);
        self.rejects_soft = self.rejects_soft.saturating_add(other.rejects_soft);
        self.rejects_hard = self.rejects_hard.saturating_add(other.rejects_hard);
        self.trades = self.trades.saturating_add(other.trades);
        self.gifts = self.gifts.saturating_add(other.gifts);
        for (mine, theirs) in self.trade_value.iter_mut().zip(other.trade_value) {
            *mine = mine.saturating_add(theirs);
        }
        self.refusals = self.refusals.saturating_add(other.refusals);
        self.deliveries = self.deliveries.saturating_add(other.deliveries);
        self.omens = self.omens.saturating_add(other.omens);
        self.jumps = self.jumps.saturating_add(other.jumps);
        self.reseeds = self.reseeds.saturating_add(other.reseeds);
        self.catch_up_seconds += other.catch_up_seconds;
    }

    /// Serialise the buffer as versioned line-oriented text — the exact
    /// string that sits inspectable in storage. The inverse of
    /// [`Aggregate::parse`], up to the whole-second flooring of the clocks.
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail, so the fmt plumbing is dropped.
        let _ = writeln!(out, "{MAGIC}");
        let _ = writeln!(out, "sessions {}", self.sessions);
        let _ = writeln!(out, "seconds {}", whole(self.seconds));
        let _ = writeln!(out, "travel_seconds {}", whole(self.travel_seconds));
        let _ = writeln!(out, "docked_seconds {}", whole(self.docked_seconds));
        let _ = writeln!(out, "warp_seconds {}", whole(self.warp_seconds));
        let _ = writeln!(out, "paused_seconds {}", whole(self.paused_seconds));
        let _ = writeln!(out, "legs {}", self.legs);
        let _ = writeln!(out, "arrivals {}", self.arrivals);
        let _ = writeln!(out, "places {}", self.places);
        let _ = writeln!(out, "pickups {}", self.pickups);
        let _ = writeln!(out, "rejects_soft {}", self.rejects_soft);
        let _ = writeln!(out, "rejects_hard {}", self.rejects_hard);
        let _ = writeln!(out, "trades {}", self.trades);
        let _ = writeln!(out, "gifts {}", self.gifts);
        let _ = writeln!(
            out,
            "trade_value {} {} {} {}",
            self.trade_value[0], self.trade_value[1], self.trade_value[2], self.trade_value[3]
        );
        let _ = writeln!(out, "refusals {}", self.refusals);
        let _ = writeln!(out, "deliveries {}", self.deliveries);
        let _ = writeln!(out, "omens {}", self.omens);
        let _ = writeln!(out, "jumps {}", self.jumps);
        let _ = writeln!(out, "reseeds {}", self.reseeds);
        let _ = writeln!(out, "catch_up_seconds {}", whole(self.catch_up_seconds));
        out
    }

    /// Rebuild a buffer from [`Aggregate::serialize`] output. The schema is
    /// closed, so every row must be present, in order, with nothing after —
    /// anything else refuses, and the caller starts a fresh buffer.
    pub fn parse(s: &str) -> Result<Self, TelemetryError> {
        let mut reader = Reader::new(s);
        match reader.next_line() {
            Ok(MAGIC) => {}
            Ok(other) if other.starts_with("TLM") => {
                return Err(TelemetryError::UnsupportedVersion);
            }
            _ => return Err(TelemetryError::BadMagic),
        }
        let aggregate = Self {
            sessions: reader.kv("sessions")?,
            seconds: reader.kv_seconds("seconds")?,
            travel_seconds: reader.kv_seconds("travel_seconds")?,
            docked_seconds: reader.kv_seconds("docked_seconds")?,
            warp_seconds: reader.kv_seconds("warp_seconds")?,
            paused_seconds: reader.kv_seconds("paused_seconds")?,
            legs: reader.kv("legs")?,
            arrivals: reader.kv("arrivals")?,
            places: reader.kv("places")?,
            pickups: reader.kv("pickups")?,
            rejects_soft: reader.kv("rejects_soft")?,
            rejects_hard: reader.kv("rejects_hard")?,
            trades: reader.kv("trades")?,
            gifts: reader.kv("gifts")?,
            trade_value: reader.kv_buckets("trade_value")?,
            refusals: reader.kv("refusals")?,
            deliveries: reader.kv("deliveries")?,
            omens: reader.kv("omens")?,
            jumps: reader.kv("jumps")?,
            reseeds: reader.kv("reseeds")?,
            catch_up_seconds: reader.kv_seconds("catch_up_seconds")?,
        };
        if reader.next_line().is_ok() {
            // The schema is closed: a buffer with extra rows is not ours.
            return Err(reader.err());
        }
        Ok(aggregate)
    }
}

/// Saturating counter bump. A lifetime of play cannot overflow a `u64`,
/// but a hand-mangled buffer could park one at the ceiling, and a tally
/// must never panic over it.
const fn bump(counter: &mut u64) {
    *counter = counter.saturating_add(1);
}

/// Quartile of a `0..=1` generosity value: `[0, ¼)`, `[¼, ½)`, `[½, ¾)`,
/// `[¾, 1]`. Out-of-range and NaN tally into the end buckets — this is a
/// tally, not a validator.
#[allow(clippy::cast_sign_loss)] // Clamped non-negative; NaN casts to 0.
fn bucket(value: f32) -> usize {
    ((value.clamp(0.0, 1.0) * 4.0) as usize).min(3)
}

/// A clock as it serialises: whole seconds, floored, never negative —
/// coarse on purpose, per the doc's anti-fingerprinting stance. The float
/// cast saturates, so no accumulator state can panic the wire.
#[allow(clippy::cast_sign_loss)] // max(0.0) guards the sign; NaN casts to 0.
const fn whole(seconds: f64) -> u64 {
    seconds.max(0.0) as u64
}

/// Line-by-line reader that remembers where it is, so every refusal can
/// name its line — the save reader's shape, in miniature.
struct Reader<'a> {
    lines: std::str::Lines<'a>,
    /// 1-based number of the line most recently read; 0 before the first.
    line: usize,
}

impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            lines: s.lines(),
            line: 0,
        }
    }

    /// A parse error naming the current line.
    const fn err(&self) -> TelemetryError {
        TelemetryError::Parse { line: self.line }
    }

    fn next_line(&mut self) -> Result<&'a str, TelemetryError> {
        match self.lines.next() {
            Some(line) => {
                self.line += 1;
                Ok(line)
            }
            None => Err(TelemetryError::Parse { line: 0 }),
        }
    }

    /// One token parsed as a counter.
    fn token(&self, token: Option<&str>) -> Result<u64, TelemetryError> {
        token
            .and_then(|t| t.parse().ok())
            .ok_or(TelemetryError::Parse { line: self.line })
    }

    /// A whole line of the form `key <count>`.
    fn kv(&mut self, key: &str) -> Result<u64, TelemetryError> {
        let line = self.next_line()?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some(key) {
            return Err(self.err());
        }
        self.token(tokens.next())
    }

    /// A `key <count>` line for a clock row: whole seconds on the wire,
    /// `f64` in memory.
    fn kv_seconds(&mut self, key: &str) -> Result<f64, TelemetryError> {
        self.kv(key).map(|seconds| seconds as f64)
    }

    /// The `trade_value` line: four bucket counters on one line.
    fn kv_buckets(&mut self, key: &str) -> Result<[u64; 4], TelemetryError> {
        let line = self.next_line()?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some(key) {
            return Err(self.err());
        }
        let mut buckets = [0_u64; 4];
        for bucket in &mut buckets {
            *bucket = self.token(tokens.next())?;
        }
        Ok(buckets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{InputFrame, Vec2, layout, splitmix};

    /// Seed whose first Guild trade is acceptable (the sim tests' odyssey
    /// seed), so the accept lever concludes a real trade.
    const SEED: u64 = 0x0DDE_55EA;

    fn press_at(p: Vec2) -> InputFrame {
        InputFrame {
            pointer: p,
            press: true,
            held: true,
            ..InputFrame::default()
        }
    }

    fn release_at(p: Vec2) -> InputFrame {
        InputFrame {
            pointer: p,
            release: true,
            ..InputFrame::default()
        }
    }

    fn rect_center(rect: layout::Rect) -> Vec2 {
        Vec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y))
    }

    /// Advance with `input` and observe the frame, exactly as the frontend
    /// wires it: the advance's cues with a snapshot taken after it.
    fn observed(sim: &mut Sim, aggregate: &mut Aggregate, input: &InputFrame) {
        sim.advance(0.0, input);
        aggregate.observe(sim.cues(), 0.0, Snapshot::of(sim));
    }

    /// Drag as two observed zero-dt frames: press, then release at `to`.
    fn drag(sim: &mut Sim, aggregate: &mut Aggregate, from: Vec2, to: Vec2) {
        observed(sim, aggregate, &press_at(from));
        assert!(sim.held(0).is_some(), "nothing to lift at {from:?}");
        observed(sim, aggregate, &release_at(to));
    }

    /// An aggregate with a distinct value in every field; whole-second
    /// clocks, so float sums stay exact for the merge algebra.
    fn sample(salt: u64) -> Aggregate {
        let n = |i: u64| splitmix(salt, i) % 1000;
        let s = |i: u64| n(i) as f64;
        Aggregate {
            sessions: n(0),
            seconds: s(1),
            travel_seconds: s(2),
            docked_seconds: s(3),
            warp_seconds: s(4),
            paused_seconds: s(5),
            legs: n(6),
            arrivals: n(7),
            places: n(8),
            pickups: n(9),
            rejects_soft: n(10),
            rejects_hard: n(11),
            trades: n(12),
            gifts: n(13),
            trade_value: [n(14), n(15), n(16), n(17)],
            refusals: n(18),
            deliveries: n(19),
            omens: n(20),
            jumps: n(21),
            reseeds: n(22),
            catch_up_seconds: s(23),
        }
    }

    #[test]
    fn every_schema_row_counts_from_a_synthetic_stream() {
        let mut aggregate = Aggregate::default();
        aggregate.note_session();
        aggregate.note_catch_up(90.9);
        let empty_handed = Snapshot {
            docked: true,
            ..Snapshot::default()
        };
        aggregate.observe(
            &[
                Cue::Depart,
                Cue::Arrive,
                Cue::Arrive,
                Cue::Pickup,
                Cue::Place,
                Cue::Reject { hard: false },
                Cue::Reject { hard: true },
                Cue::Reject { hard: false },
                Cue::Accept { value: 0.9 }, // received shelf empty: a gift
                Cue::Refuse,
                Cue::Delivered,
                Cue::OmenStart,
                Cue::Jump,
                Cue::Reseed,
            ],
            0.0,
            empty_handed,
        );
        let take_landed = Snapshot {
            docked: true,
            received_occupied: true,
            ..Snapshot::default()
        };
        aggregate.observe(&[Cue::Accept { value: 0.0 }], 0.0, take_landed);

        assert_eq!(aggregate.sessions, 1);
        assert_eq!(aggregate.legs, 1);
        assert_eq!(aggregate.arrivals, 2);
        assert_eq!(aggregate.pickups, 1);
        assert_eq!(aggregate.places, 1);
        assert_eq!(aggregate.rejects_soft, 2);
        assert_eq!(aggregate.rejects_hard, 1);
        assert_eq!(aggregate.gifts, 1);
        assert_eq!(aggregate.trades, 1);
        assert_eq!(aggregate.trade_value, [1, 0, 0, 1]);
        assert_eq!(aggregate.refusals, 1);
        assert_eq!(aggregate.deliveries, 1);
        assert_eq!(aggregate.omens, 1);
        assert_eq!(aggregate.jumps, 1);
        assert_eq!(aggregate.reseeds, 1);
        assert!((aggregate.catch_up_seconds - 90.9).abs() < 1e-9);
    }

    #[test]
    fn uncounted_cues_change_nothing() {
        let mut aggregate = Aggregate::default();
        aggregate.observe(
            &[
                Cue::Select,
                Cue::OmenEnd,
                Cue::Creak { intensity: 1.0 },
                Cue::Pause { paused: true },
                Cue::Warp { engaged: true },
            ],
            0.0,
            Snapshot::default(),
        );
        assert_eq!(aggregate, Aggregate::default());
    }

    /// The split the doc promises — accepts by whether the take pad was
    /// empty — read off a real sim through the received shelf, not
    /// synthetic bools: a pure gift and the odyssey trade.
    #[test]
    fn a_real_gift_and_a_real_trade_split_by_the_received_shelf() {
        let vial = rect_center(layout::cell_rect(0, 0));
        let scrap = rect_center(layout::cell_rect(0, 2));
        let pearls = rect_center(layout::cell_rect(2, 0));
        let accept = rect_center(layout::ACCEPT_LEVER);

        // A pure gift: one starter piece to the give pad, nothing taken.
        let mut sim = Sim::new(SEED);
        let mut aggregate = Aggregate::default();
        drag(
            &mut sim,
            &mut aggregate,
            vial,
            rect_center(layout::GIVE_SLOTS[0]),
        );
        observed(&mut sim, &mut aggregate, &press_at(accept));
        assert_eq!(aggregate.gifts, 1);
        assert_eq!(aggregate.trades, 0);
        assert_eq!(aggregate.trade_value.iter().sum::<u64>(), 1);

        // The odyssey trade: all three starter pieces given, the first
        // shelf good taken — the accept moves it to the received shelf.
        let mut sim = Sim::new(SEED);
        let mut aggregate = Aggregate::default();
        drag(
            &mut sim,
            &mut aggregate,
            vial,
            rect_center(layout::GIVE_SLOTS[0]),
        );
        drag(
            &mut sim,
            &mut aggregate,
            scrap,
            rect_center(layout::GIVE_SLOTS[1]),
        );
        drag(
            &mut sim,
            &mut aggregate,
            pearls,
            rect_center(layout::GIVE_SLOTS[2]),
        );
        drag(
            &mut sim,
            &mut aggregate,
            rect_center(layout::SHELF_SLOTS[0]),
            rect_center(layout::TAKE_SLOTS[0]),
        );
        observed(&mut sim, &mut aggregate, &press_at(accept));
        assert_eq!(aggregate.trades, 1);
        assert_eq!(aggregate.gifts, 0);
        assert_eq!(aggregate.pickups, 4);
        assert_eq!(aggregate.places, 4);
        assert_eq!(aggregate.trade_value.iter().sum::<u64>(), 1);
    }

    #[test]
    fn snapshot_reads_the_coarse_state() {
        let mut sim = Sim::new(SEED);
        let at = Snapshot::of(&sim);
        assert!(at.docked, "a fresh sim starts docked");
        assert!(!at.traveling && !at.warp && !at.paused && !at.received_occupied);

        // Select Venus, launch, then toggle warp and pause.
        sim.advance(0.0, &press_at(sim.poi_pos(0)));
        sim.advance(0.0, &press_at(rect_center(layout::LAUNCH_LEVER)));
        sim.advance(
            0.0,
            &InputFrame {
                toggle_warp: true,
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        let at = Snapshot::of(&sim);
        assert!(at.traveling && !at.docked && at.warp && at.paused);
    }

    #[test]
    fn state_seconds_follow_the_flags_and_serialise_whole() {
        let mut aggregate = Aggregate::default();
        let cruising = Snapshot {
            traveling: true,
            warp: true,
            ..Snapshot::default()
        };
        for _ in 0..3 {
            aggregate.observe(&[], 0.4, cruising);
        }
        aggregate.observe(
            &[],
            0.4,
            Snapshot {
                docked: true,
                paused: true,
                ..Snapshot::default()
            },
        );
        assert!((aggregate.seconds - 1.6).abs() < 1e-6);
        assert!((aggregate.travel_seconds - 1.2).abs() < 1e-6);
        assert!((aggregate.warp_seconds - 1.2).abs() < 1e-6);
        assert!((aggregate.docked_seconds - 0.4).abs() < 1e-6);
        assert!((aggregate.paused_seconds - 0.4).abs() < 1e-6);

        // Fractions survive in memory but the wire is whole seconds.
        let text = aggregate.serialize();
        assert!(text.contains("\nseconds 1\n"), "wire clock not floored");
        assert!(text.contains("\ntravel_seconds 1\n"));
        assert!(text.contains("\ndocked_seconds 0\n"));
        let parsed = Aggregate::parse(&text).expect("own buffer must parse");
        assert!((parsed.seconds - 1.0).abs() < 1e-9);
    }

    #[test]
    fn hostile_dt_cannot_poison_the_clocks() {
        let mut aggregate = Aggregate::default();
        let idle = Snapshot::default();
        aggregate.observe(&[], f32::NAN, idle);
        aggregate.observe(&[], f32::INFINITY, idle);
        aggregate.observe(&[], -5.0, idle);
        assert!(aggregate.seconds.abs() < 1e-9, "junk dt counted as time");
        // A backgrounded tab's giant frame counts as at most MAX_FRAME.
        aggregate.observe(&[], 3600.0, idle);
        assert!((aggregate.seconds - f64::from(MAX_FRAME)).abs() < 1e-9);
    }

    #[test]
    fn catch_up_ignores_junk_and_accumulates_seconds() {
        let mut aggregate = Aggregate::default();
        aggregate.note_catch_up(f64::NAN);
        aggregate.note_catch_up(f64::INFINITY);
        aggregate.note_catch_up(-30.0);
        assert!(aggregate.catch_up_seconds.abs() < 1e-9);
        aggregate.note_catch_up(45.5);
        aggregate.note_catch_up(10.0);
        assert!((aggregate.catch_up_seconds - 55.5).abs() < 1e-9);
        assert!(aggregate.serialize().contains("\ncatch_up_seconds 55\n"));
    }

    #[test]
    fn value_buckets_split_at_the_quartiles() {
        assert_eq!(bucket(0.0), 0);
        assert_eq!(bucket(0.2499), 0);
        assert_eq!(bucket(0.25), 1);
        assert_eq!(bucket(0.4999), 1);
        assert_eq!(bucket(0.5), 2);
        assert_eq!(bucket(0.7499), 2);
        assert_eq!(bucket(0.75), 3);
        assert_eq!(bucket(1.0), 3);
        // Out-of-range and NaN tally into the ends instead of panicking.
        assert_eq!(bucket(-1.0), 0);
        assert_eq!(bucket(9.0), 3);
        assert_eq!(bucket(f32::NAN), 0);
    }

    #[test]
    fn serialisation_round_trips_at_whole_seconds() {
        let mut aggregate = sample(1);
        aggregate.note_catch_up(12.75);
        let text = aggregate.serialize();
        let parsed = Aggregate::parse(&text).expect("own buffer must parse");
        // The wire floored the fraction; a second trip is exact.
        assert_eq!(parsed.serialize(), text, "re-serialisation drifted");
        assert_eq!(
            whole(parsed.catch_up_seconds),
            whole(aggregate.catch_up_seconds)
        );
        // A whole-second buffer round-trips to equality outright.
        let exact = sample(2);
        assert_eq!(
            Aggregate::parse(&exact.serialize()).expect("must parse"),
            exact
        );
    }

    #[test]
    fn merge_is_commutative_associative_with_zero_identity() {
        let (a, b, c) = (sample(1), sample(2), sample(3));
        let mut ab = a.clone();
        ab.merge(&b);
        let mut ba = b.clone();
        ba.merge(&a);
        assert_eq!(ab, ba, "merge is not commutative");

        let mut ab_c = ab.clone();
        ab_c.merge(&c);
        let mut bc = b.clone();
        bc.merge(&c);
        let mut a_bc = a.clone();
        a_bc.merge(&bc);
        assert_eq!(ab_c, a_bc, "merge is not associative");

        let mut a0 = a.clone();
        a0.merge(&Aggregate::default());
        assert_eq!(a0, a, "the fresh buffer is not the identity");

        // And the sum is really the sum.
        assert_eq!(ab.sessions, a.sessions + b.sessions);
        assert_eq!(ab.trade_value[2], a.trade_value[2] + b.trade_value[2]);
    }

    #[test]
    fn truncation_at_every_line_boundary_fails_safe() {
        let text = sample(7).serialize();
        assert!(
            Aggregate::parse(&text).is_ok(),
            "the untruncated buffer parses"
        );
        let lines: Vec<&str> = text.lines().collect();
        for keep in 0..lines.len() {
            let truncated = lines[..keep].join("\n");
            assert!(
                Aggregate::parse(&truncated).is_err(),
                "buffer truncated to {keep}/{} lines parsed anyway",
                lines.len()
            );
        }
    }

    #[test]
    fn mangling_any_line_fails_safe() {
        let text = sample(7).serialize();
        let lines: Vec<&str> = text.lines().collect();
        for target in 0..lines.len() {
            for garbage in [
                "",
                "!!! ???",
                "sessions -1",
                "sessions 99999999999999999999999",
                "\u{1F680}",
            ] {
                let mangled: String = lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| if i == target { garbage } else { line })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    Aggregate::parse(&mangled).is_err(),
                    "line {target} replaced by {garbage:?} parsed anyway"
                );
            }
        }
        // The closed schema also refuses extra rows.
        assert!(Aggregate::parse(&format!("{text}extra 1\n")).is_err());
    }

    #[test]
    fn foreign_magic_and_versions_refuse() {
        assert_eq!(Aggregate::parse(""), Err(TelemetryError::BadMagic));
        assert_eq!(Aggregate::parse("STV2\n"), Err(TelemetryError::BadMagic));
        assert_eq!(
            Aggregate::parse("TLM9\nsessions 0\n"),
            Err(TelemetryError::UnsupportedVersion)
        );
        assert_eq!(
            Aggregate::parse(MAGIC),
            Err(TelemetryError::Parse { line: 0 })
        );
    }

    /// Deterministic fuzz over a spicy alphabet: garbage never panics, and
    /// anything that happens to parse re-serialises without panicking.
    #[test]
    fn arbitrary_garbage_never_panics() {
        let alphabet: Vec<char> = "TLM1 sessions seconds trade_value 0-9\n\t\u{FFFD}\u{1F680}"
            .chars()
            .collect();
        for round in 0_u64..300 {
            let len = (splitmix(round, 0) % 200) as usize;
            let s: String = (0..len)
                .map(|i| alphabet[(splitmix(round, 1 + i as u64) as usize) % alphabet.len()])
                .collect();
            if let Ok(aggregate) = Aggregate::parse(&s) {
                let _ = aggregate.serialize();
            }
        }
    }

    #[test]
    fn errors_display_without_panicking() {
        assert_eq!(
            TelemetryError::BadMagic.to_string(),
            "not a Space Trucking telemetry buffer"
        );
        assert_eq!(
            TelemetryError::UnsupportedVersion.to_string(),
            "telemetry buffer is from an unsupported version"
        );
        assert_eq!(
            TelemetryError::Parse { line: 0 }.to_string(),
            "telemetry buffer ends too early"
        );
        assert_eq!(
            TelemetryError::Parse { line: 4 }.to_string(),
            "malformed telemetry buffer at line 4"
        );
    }
}
