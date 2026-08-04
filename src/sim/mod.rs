//! The whole game: pure, deterministic, and macroquad-free.
//!
//! Space Trucking is an ambient hauling loop — pick a destination while
//! docked, pull the launch lever, cruise in real time, auto-dock, and barter
//! cargo for cargo with no currency in sight. All of it lives here as a plain
//! library: the frontend's only channel in is an [`InputFrame`], and its only
//! channels out are the state getters plus [`Sim::cues`]. A given seed plus a
//! given input sequence produces a bit-identical run, which is what makes the
//! save format, the tests, and the benches possible.
//!
//! Time is fixed-step with an accumulator (the classic "Fix Your Timestep"
//! shape): [`Sim::advance`] runs whole [`TICK_DT`] steps and leaves the
//! leftover fraction in [`Sim::alpha`] for the renderer to interpolate with.
//! Warp stretches the wall-clock side of that equation by [`WARP_FACTOR`],
//! and [`Sim::fast_forward`] replays offline time through the same tick
//! function with cues suppressed.
//!
//! Sound follows the same rule as pixels: the sim reports that something
//! happened and how hard, as a [`Cue`], and the frontend decides what that
//! sounds like. No audio types cross into this module.

pub mod barter;
pub mod cargo;
mod event;
pub mod layout;
pub mod map;
pub mod save;

use std::ops::{Add, AddAssign, Mul, MulAssign, Sub};

pub use barter::{Barter, VALUE};
pub use cargo::{KIND_COUNT, Kind, Loc, Piece, Tag, placement_legal};
use event::Events;
pub use map::{GUILD, POI_COUNT, POIS, Poi, PoiId, SHIP_SPEED, Ship, ShipState};
pub use save::SaveError;

/// Length of one simulation step. Ticks are always exactly this long.
pub const TICK_DT: f32 = 1.0 / 60.0;

/// Logical world width. The renderer scales this onto the window, so the sim
/// never learns what a pixel is.
pub const WORLD_W: f32 = 800.0;

/// Logical world height.
pub const WORLD_H: f32 = 600.0;

/// How much faster sim time runs while warp is engaged.
pub const WARP_FACTOR: f32 = 16.0;

/// Longest frame the accumulator will bank, in unwarped seconds. A
/// backgrounded tab or a debugger pause hands us an enormous `frame_dt`;
/// without this cap the sim would try to catch up all at once and spiral.
const MAX_FRAME_DT: f32 = 0.25;

/// Starter cargo and where it is stowed: three pieces, placed legally
/// (the scrap is heavy, so it sits low).
const STARTER_CARGO: [(Kind, u8, u8); 3] = [
    (Kind::ScrapAlloy, 0, 2),
    (Kind::PerfumeVial, 0, 0),
    (Kind::BrinePearls, 2, 0),
];

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

/// Splitmix64's finalizer as a combiner: fold `salt` into `state` and
/// return an independent, well-mixed value.
///
/// Every derived randomness in the sim — visit shelves, jump schedules,
/// creaks — chains through this, so the one persistent RNG stays reserved
/// for cosmetic variant rolls.
#[must_use]
pub const fn splitmix(state: u64, salt: u64) -> u64 {
    let mut z = state
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Everything the sim is allowed to know about the outside world for one
/// frame. The sim never reads global input state, which is what makes replay
/// and testing possible.
// Five bools, but they are a fixed input record, not a state machine in
// disguise; a struct-per-button would be ceremony.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InputFrame {
    /// Pointer position in world coordinates.
    pub pointer: Vec2,
    /// Primary button went down this frame.
    pub press: bool,
    /// Primary button is down.
    pub held: bool,
    /// Primary button went up this frame.
    pub release: bool,
    /// Edge, precomputed by the frontend (Space or the pause icon).
    pub toggle_pause: bool,
    /// Edge, precomputed by the frontend (F or the warp icon).
    pub toggle_warp: bool,
    /// Restart with a new seed.
    pub reseed: Option<u64>,
}

/// Something that happened and is worth hearing. The sim says what and how
/// hard; choosing a waveform is the frontend's job. Intensities are `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// Destination chosen on the map.
    Select,
    Depart,
    Arrive,
    /// A piece was lifted.
    Pickup,
    /// A piece landed somewhere legal.
    Place,
    /// `hard` = a placement rule refused an in-grid drop; soft = an ignored
    /// click or drop.
    Reject {
        hard: bool,
    },
    /// Trade concluded; `value` is the generosity overshoot, `0..=1`.
    Accept {
        value: f32,
    },
    /// Accept lever pulled on a trade the station will not take.
    Refuse,
    OmenStart,
    Jump,
    OmenEnd,
    /// Ambient hull creak while traveling.
    Creak {
        intensity: f32,
    },
    /// Pause was toggled. `paused` is the state just entered.
    Pause {
        paused: bool,
    },
    /// Warp was toggled. `engaged` is the state just entered.
    Warp {
        engaged: bool,
    },
    /// The world was replaced.
    Reseed,
}

/// A piece mid-drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    /// Id of the piece being dragged.
    pub piece: u32,
    /// Where it was lifted from, and where it snaps back to.
    pub origin: Loc,
    /// Whether dropping at the current pointer would succeed.
    pub legal: bool,
}

/// What [`Sim::fast_forward`] lived through, so the frontend can summarise
/// an absence instead of replaying its cues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatchUp {
    /// Ticks actually run.
    pub ticks: u64,
    /// The ship docked somewhere along the way.
    pub arrived: bool,
    /// A suspicious-cargo jump fired along the way.
    pub jumped: bool,
}

/// The simulation. Same seed plus same [`InputFrame`] sequence gives
/// bit-identical states on every run.
#[derive(Clone, Debug)]
pub struct Sim {
    seed: u64,
    /// Persistent run RNG, spent only on cosmetic variant rolls.
    rng: fastrand::Rng,
    accumulator: f32,
    tick: u64,
    paused: bool,
    warp: bool,
    cues: Vec<Cue>,
    ship: Ship,
    pieces: Vec<Piece>,
    /// Next piece id; never reused within a run.
    next_piece: u32,
    held: Option<Held>,
    /// The current visit's trade, `Some` iff docked.
    barter: Option<Barter>,
    /// The current visit's jittered value table; meaningful iff docked.
    values: [u8; KIND_COUNT],
    /// Times each POI has been docked at.
    visits: [u32; POI_COUNT],
    events: Events,
}

impl Sim {
    /// Build a sim from a seed: docked at the Guild Station with the starter
    /// cargo stowed and the first visit's shelf laid out.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let mut next_piece = 0_u32;
        let mut pieces: Vec<Piece> = STARTER_CARGO
            .iter()
            .map(|&(kind, x, y)| {
                let piece = Piece {
                    id: next_piece,
                    kind,
                    variant: rng.u8(..cargo::VARIANTS),
                    loc: Loc::Hold { x, y },
                };
                next_piece += 1;
                piece
            })
            .collect();
        debug_assert!(
            (0..pieces.len()).all(|i| {
                let Loc::Hold { x, y } = pieces[i].loc else {
                    return false;
                };
                placement_legal(&pieces, pieces[i].id, pieces[i].kind, x, y)
            }),
            "starter cargo must be stowed legally"
        );

        let mut visits = [0_u32; POI_COUNT];
        visits[usize::from(GUILD)] = 1;
        let (barter, goods) = barter::generate(seed, GUILD, 1, &mut rng, &mut next_piece);
        pieces.extend(goods);
        let pos = POIS[usize::from(GUILD)].pos;

        Self {
            seed,
            rng,
            accumulator: 0.0,
            tick: 0,
            paused: false,
            warp: false,
            cues: Vec::new(),
            ship: Ship {
                pos,
                prev_pos: pos,
                state: ShipState::Docked(GUILD),
                selected: None,
            },
            pieces,
            next_piece,
            held: None,
            barter: Some(barter),
            values: barter::visit_values(seed, GUILD, 1),
            visits,
            events: Events::new(),
        }
    }

    /// Consume one frame's worth of real time, returning how many fixed
    /// ticks ran. `frame_dt` is clamped to [`MAX_FRAME_DT`]; warp multiplies
    /// both the frame and the clamp by [`WARP_FACTOR`].
    pub fn advance(&mut self, frame_dt: f32, input: &InputFrame) -> u32 {
        // Cues describe this frame only; last frame's have been consumed.
        self.cues.clear();

        if let Some(seed) = input.reseed {
            // Note the ordering: reseeding rebuilds `self`, cue list
            // included, so the cue has to be pushed after it.
            self.reseed(seed);
            self.cues.push(Cue::Reseed);
        }
        if input.toggle_pause {
            self.paused = !self.paused;
            self.cues.push(Cue::Pause {
                paused: self.paused,
            });
        }
        if input.toggle_warp {
            self.warp = !self.warp;
            self.cues.push(Cue::Warp { engaged: self.warp });
        }
        if self.paused {
            return 0;
        }

        // Pointer edges are per-frame events, exactly like the template's
        // burst was: handling them inside the tick loop would fire them once
        // per tick.
        self.handle_pointer(input);

        let scale = if self.warp { WARP_FACTOR } else { 1.0 };
        self.accumulator += (frame_dt * scale).clamp(0.0, MAX_FRAME_DT * scale);
        let mut ticks = 0;
        // The float condition is the fixed-timestep idiom; the clamp above
        // bounds the loop.
        #[allow(clippy::while_float)]
        while self.accumulator >= TICK_DT {
            self.accumulator -= TICK_DT;
            self.step();
            ticks += 1;
        }
        ticks
    }

    /// Run `ticks` default-input ticks for offline catch-up, suppressing cue
    /// accumulation, and report what happened. A paused sim stays exactly
    /// where it was left.
    pub fn fast_forward(&mut self, ticks: u64) -> CatchUp {
        self.cues.clear();
        let ran = if self.paused {
            0
        } else {
            for _ in 0..ticks {
                self.step();
            }
            ticks
        };
        let arrived = self.cues.iter().any(|cue| matches!(cue, Cue::Arrive));
        let jumped = self.cues.iter().any(|cue| matches!(cue, Cue::Jump));
        self.cues.clear();
        CatchUp {
            ticks: ran,
            arrived,
            jumped,
        }
    }

    /// How far the leftover accumulator has carried us into the next tick,
    /// in `0..1`. Feed this to the render interpolation.
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

    /// Seed this sim was built from.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Total elapsed ticks.
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Whether ticking is currently suspended.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Whether warp is engaged.
    #[must_use]
    pub const fn is_warp(&self) -> bool {
        self.warp
    }

    /// The star map's points of interest.
    #[must_use]
    pub const fn pois(&self) -> &[Poi; POI_COUNT] {
        &POIS
    }

    /// The freighter.
    #[must_use]
    pub const fn ship(&self) -> &Ship {
        &self.ship
    }

    /// Every live piece, wherever it sits.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// The piece mid-drag, if any.
    #[must_use]
    pub const fn held(&self) -> Option<&Held> {
        self.held.as_ref()
    }

    /// The current visit's trade. `Some` iff docked.
    #[must_use]
    pub const fn barter(&self) -> Option<&Barter> {
        self.barter.as_ref()
    }

    /// Console light, `0..=1`. The suspicious-jump omen dims it.
    #[must_use]
    pub const fn light(&self) -> f32 {
        self.events.light
    }

    /// Whether a suspicious piece is stowed in the hold.
    #[must_use]
    pub fn suspicious_aboard(&self) -> bool {
        self.pieces.iter().any(|piece| {
            matches!(piece.loc, Loc::Hold { .. })
                && matches!(piece.kind.tag(), Some(Tag::Suspicious))
        })
    }

    /// Omen intensity for the hum swell, `0..=1`; zero when idle.
    #[must_use]
    pub const fn omen(&self) -> f32 {
        self.events.omen
    }

    /// Serialise the whole run as versioned line-oriented text.
    #[must_use]
    pub fn save_string(&self) -> String {
        save::serialize(self)
    }

    /// Rebuild a sim from [`Sim::save_string`] output.
    pub fn from_save(s: &str) -> Result<Self, SaveError> {
        save::parse(s)
    }

    /// Throw away all state and start over from `seed`.
    fn reseed(&mut self, seed: u64) {
        let paused = self.paused;
        let warp = self.warp;
        *self = Self::new(seed);
        self.paused = paused;
        self.warp = warp;
    }

    /// One frame's pointer handling: presses lift or actuate, releases drop,
    /// and the held piece's legality tracks the pointer.
    fn handle_pointer(&mut self, input: &InputFrame) {
        if input.press {
            self.on_press(input.pointer);
        }
        if input.release {
            self.on_release(input.pointer);
        }
        if self.held.is_some() && !input.held && !input.press && !input.release {
            // The release never arrived (window blur, touch cancel): snap
            // back silently rather than glue a piece to a phantom pointer.
            self.held = None;
        }
        if let Some(held) = self.held {
            let legal = self
                .pieces
                .iter()
                .find(|piece| piece.id == held.piece)
                .is_some_and(|piece| self.resolve_drop(piece, input.pointer).is_ok());
            if let Some(held) = &mut self.held {
                held.legal = legal;
            }
        }
    }

    /// A press either lifts a piece or actuates whatever it landed on.
    fn on_press(&mut self, p: Vec2) {
        if self.held.is_some() {
            return;
        }
        let docked = matches!(self.ship.state, ShipState::Docked(_));
        if let Some(piece) = self.pieces.iter().find(|piece| {
            layout::piece_rect(piece).contains(p)
                && (docked || matches!(piece.loc, Loc::Hold { .. }))
        }) {
            self.held = Some(Held {
                piece: piece.id,
                origin: piece.loc,
                legal: true,
            });
            self.cues.push(Cue::Pickup);
        } else if docked {
            self.on_press_docked(p);
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// Docked-only press targets: POIs, the launch lever, the accept lever.
    fn on_press_docked(&mut self, p: Vec2) {
        let ShipState::Docked(at) = self.ship.state else {
            return;
        };
        for (i, poi) in POIS.iter().enumerate() {
            let id = i as PoiId;
            if id != at && (p - poi.pos).length() <= poi.radius {
                self.ship.selected = Some(id);
                self.cues.push(Cue::Select);
                return;
            }
        }
        if layout::LAUNCH_LEVER.contains(p) {
            if self.ship.selected.is_some() && !self.received_occupied() {
                self.depart();
            } else {
                // No destination, or received goods still waiting stowage.
                self.cues.push(Cue::Reject { hard: false });
            }
        } else if layout::ACCEPT_LEVER.contains(p) {
            self.conclude();
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// A release drops the held piece: place it if the target is legal,
    /// snap it back otherwise.
    fn on_release(&mut self, p: Vec2) {
        let Some(held) = self.held.take() else {
            return;
        };
        let Some(index) = self.pieces.iter().position(|piece| piece.id == held.piece) else {
            return;
        };
        let piece = self.pieces[index];
        match self.resolve_drop(&piece, p) {
            Ok(loc) => {
                self.pieces[index].loc = loc;
                self.cues.push(Cue::Place);
            }
            Err(hard) => self.cues.push(Cue::Reject { hard }),
        }
    }

    /// Where dropping `piece` at `p` would put it, or which flavour of
    /// rejection it earns. Hard means a stowage rule refused an in-grid
    /// drop; everything else that fails is a soft, ignorable miss.
    fn resolve_drop(&self, piece: &Piece, p: Vec2) -> Result<Loc, bool> {
        let ours = matches!(
            piece.loc,
            Loc::Hold { .. } | Loc::GivePad { .. } | Loc::ReceivedShelf { .. }
        );
        if let Some((x, y)) = layout::cell_at(p) {
            if !ours {
                // Station goods never enter the hold before a trade.
                return Err(false);
            }
            return if placement_legal(&self.pieces, piece.id, piece.kind, x, y) {
                Ok(Loc::Hold { x, y })
            } else {
                Err(true)
            };
        }
        if self.barter.is_some() {
            if let Some(slot) = layout::slot_at(&layout::GIVE_SLOTS, p) {
                let loc = Loc::GivePad { slot };
                return (ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(false);
            }
            if let Some(slot) = layout::slot_at(&layout::TAKE_SLOTS, p) {
                let loc = Loc::TakePad { slot };
                return (!ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(false);
            }
            if let Some(slot) = layout::slot_at(&layout::SHELF_SLOTS, p) {
                let loc = Loc::StationShelf { slot };
                return (!ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(false);
            }
            if let Some(slot) = layout::slot_at(&layout::RECEIVED_SLOTS, p) {
                let loc = Loc::ReceivedShelf { slot };
                return (matches!(piece.loc, Loc::ReceivedShelf { .. })
                    && self.slot_free(loc, piece.id))
                .then_some(loc)
                .ok_or(false);
            }
        }
        Err(false)
    }

    /// Whether no other piece occupies `loc`.
    fn slot_free(&self, loc: Loc, id: u32) -> bool {
        !self
            .pieces
            .iter()
            .any(|other| other.id != id && other.loc == loc)
    }

    /// Whether any traded-for goods still wait on the received shelf.
    fn received_occupied(&self) -> bool {
        self.pieces
            .iter()
            .any(|piece| matches!(piece.loc, Loc::ReceivedShelf { .. }))
    }

    /// Pull the accept lever: swap the pads if the station agrees, refuse
    /// otherwise. Received goods must be stowed before trading again, so the
    /// received shelf never double-books a slot.
    fn conclude(&mut self) {
        let Some(barter) = &self.barter else {
            return;
        };
        if !barter.ready || self.received_occupied() {
            self.cues.push(Cue::Refuse);
            return;
        }
        let value = (barter.eagerness - 1.0).clamp(0.0, 1.0);
        self.pieces
            .retain(|piece| !matches!(piece.loc, Loc::GivePad { .. }));
        for piece in &mut self.pieces {
            if let Loc::TakePad { slot } = piece.loc {
                piece.loc = Loc::ReceivedShelf { slot };
            }
        }
        self.cues.push(Cue::Accept { value });
    }

    /// Cast off toward the selected destination. Anything not stowed in the
    /// hold stays on the dock — the station keeps its shelf and pads.
    fn depart(&mut self) {
        let ShipState::Docked(from) = self.ship.state else {
            return;
        };
        let Some(to) = self.ship.selected else {
            return;
        };
        let leg_ticks = map::leg_ticks(from, to);
        self.pieces
            .retain(|piece| matches!(piece.loc, Loc::Hold { .. }));
        self.barter = None;
        self.events
            .on_depart(self.seed, leg_ticks, self.suspicious_aboard());
        self.ship.state = ShipState::Traveling {
            from,
            to,
            progress: 0,
            leg_ticks,
        };
        self.cues.push(Cue::Depart);
    }

    /// One fixed step.
    fn step(&mut self) {
        self.tick += 1;
        self.ship.prev_pos = self.ship.pos;

        if let ShipState::Traveling {
            from,
            to,
            mut progress,
            leg_ticks,
        } = self.ship.state
        {
            progress += 1;
            self.events
                .travel_tick(&mut progress, leg_ticks, &mut self.cues);
            if let Some(cue) = event::creak(self.seed, self.tick) {
                self.cues.push(cue);
            }
            if progress >= leg_ticks {
                self.dock(to);
            } else {
                let t = progress as f32 / leg_ticks as f32;
                self.ship.pos = POIS[usize::from(from)]
                    .pos
                    .lerp(POIS[usize::from(to)].pos, t);
                self.ship.state = ShipState::Traveling {
                    from,
                    to,
                    progress,
                    leg_ticks,
                };
            }
        }

        self.events.ease_tick();

        if let Some(barter) = &mut self.barter {
            barter.prev_eagerness = barter.eagerness;
            let (eagerness, ready) = barter::eagerness_of(&self.pieces, &self.values);
            barter.eagerness = eagerness;
            barter.ready = ready;
        }
    }

    /// Arrive: snap to the pad, count the visit, and lay out its trade.
    fn dock(&mut self, poi: PoiId) {
        let pos = POIS[usize::from(poi)].pos;
        self.ship.pos = pos;
        self.ship.state = ShipState::Docked(poi);
        self.ship.selected = None;
        self.events.on_arrive(&mut self.cues);
        self.visits[usize::from(poi)] += 1;
        let visit = self.visits[usize::from(poi)];
        let (barter, goods) =
            barter::generate(self.seed, poi, visit, &mut self.rng, &mut self.next_piece);
        self.values = barter::visit_values(self.seed, poi, visit);
        self.pieces.extend(goods);
        self.barter = Some(barter);
        self.cues.push(Cue::Arrive);
    }
}

/// Whether a press landed on one of the console icons the frontend already
/// translates into toggles; the sim stays quiet about those.
const fn icon_press(p: Vec2) -> bool {
    layout::PAUSE_BTN.contains(p) || layout::WARP_BTN.contains(p) || layout::SPEAKER.contains(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Venus, the first POI, as a test destination.
    const VENUS: PoiId = 0;

    fn press_at(x: f32, y: f32) -> InputFrame {
        InputFrame {
            pointer: Vec2::new(x, y),
            press: true,
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

    /// Select Venus and pull the launch lever; zero-dt frames run no ticks,
    /// so the sim is exactly at departure.
    fn launched(seed: u64) -> Sim {
        let mut sim = Sim::new(seed);
        let venus = POIS[usize::from(VENUS)].pos;
        sim.advance(0.0, &press_at(venus.x, venus.y));
        assert_eq!(sim.cues(), [Cue::Select]);
        assert_eq!(sim.ship().selected, Some(VENUS));
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Depart]);
        assert!(matches!(sim.ship().state, ShipState::Traveling { .. }));
        sim
    }

    #[test]
    fn same_seed_and_inputs_are_bit_identical() {
        let mut a = launched(0xDEAD_BEEF);
        let mut b = launched(0xDEAD_BEEF);
        for _ in 0..120 {
            a.advance(TICK_DT, &InputFrame::default());
            b.advance(TICK_DT, &InputFrame::default());
        }
        assert_eq!(a.save_string(), b.save_string());
    }

    #[test]
    fn different_seeds_diverge() {
        assert_ne!(Sim::new(1).save_string(), Sim::new(2).save_string());
    }

    #[test]
    fn new_sim_starts_docked_at_the_guild_with_starter_cargo() {
        let sim = Sim::new(7);
        assert_eq!(sim.ship().state, ShipState::Docked(GUILD));
        assert!(sim.barter().is_some(), "docked means barter");
        let hold: Vec<&Piece> = sim
            .pieces()
            .iter()
            .filter(|piece| matches!(piece.loc, Loc::Hold { .. }))
            .collect();
        assert_eq!(hold.len(), STARTER_CARGO.len());
        for piece in hold {
            let Loc::Hold { x, y } = piece.loc else {
                unreachable!()
            };
            assert!(
                placement_legal(sim.pieces(), piece.id, piece.kind, x, y),
                "starter {:?} stowed illegally",
                piece.kind
            );
        }
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
    fn warp_multiplies_time_and_its_clamp() {
        let mut sim = Sim::new(1);
        let toggle = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        assert_eq!(sim.advance(0.0, &toggle), 0);
        assert!(sim.is_warp());
        assert_eq!(sim.cues(), [Cue::Warp { engaged: true }]);

        assert_eq!(
            sim.advance(TICK_DT, &InputFrame::default()),
            16,
            "one warped frame should run WARP_FACTOR ticks"
        );
        // The frame clamp scales with warp: 0.25 s * 16 = 240 ticks.
        let ticks = sim.advance(10.0, &InputFrame::default());
        assert!((239..=240).contains(&ticks), "warped clamp ran {ticks}");
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

        let before = sim.save_string();
        assert_eq!(sim.advance(TICK_DT * 10.0, &InputFrame::default()), 0);
        assert_eq!(sim.tick(), 0);
        assert_eq!(sim.save_string(), before);
    }

    #[test]
    fn pause_warp_and_reseed_announce_themselves() {
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
        assert_eq!(sim.seed(), 6);
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
    fn launch_without_a_destination_soft_rejects() {
        let mut sim = Sim::new(3);
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
    }

    #[test]
    fn travel_arrives_and_docks() {
        let mut sim = launched(11);
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            unreachable!()
        };
        let caught_up = sim.fast_forward(leg_ticks + 10);
        assert!(caught_up.arrived, "leg of {leg_ticks} ticks never arrived");
        assert_eq!(sim.ship().state, ShipState::Docked(VENUS));
        assert!(sim.barter().is_some(), "docked means barter");
        assert!(sim.cues().is_empty(), "fast_forward must suppress cues");
    }

    #[test]
    fn drag_places_and_rejects() {
        let mut sim = Sim::new(5);
        // The starter vial sits at (0, 0); lift it.
        let vial = rect_center(layout::cell_rect(0, 0));
        sim.advance(0.0, &press_at(vial.x, vial.y));
        assert_eq!(sim.cues(), [Cue::Pickup]);
        assert!(sim.held().is_some());

        // Drop onto the scrap at (0, 2): overlap, a hard reject.
        let scrap = rect_center(layout::cell_rect(0, 2));
        sim.advance(0.0, &release_at(scrap.x, scrap.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert!(sim.held().is_none());

        // Lift again and drop on a free cell: placed.
        sim.advance(0.0, &press_at(vial.x, vial.y));
        let free = rect_center(layout::cell_rect(5, 3));
        sim.advance(0.0, &release_at(free.x, free.y));
        assert_eq!(sim.cues(), [Cue::Place]);
        let vial_piece = sim.pieces().iter().find(|p| p.id == 1).unwrap();
        assert_eq!(vial_piece.loc, Loc::Hold { x: 5, y: 3 });
    }

    #[test]
    fn save_round_trips() {
        let mut sim = launched(0xC0FF_EE00);
        for _ in 0..90 {
            sim.advance(TICK_DT, &InputFrame::default());
        }
        let saved = sim.save_string();
        let restored = Sim::from_save(&saved).expect("own save must parse");
        assert_eq!(restored.save_string(), saved);
        assert_eq!(restored.seed(), sim.seed());
        assert_eq!(restored.tick(), sim.tick());

        // A docked save rebuilds its barter too.
        let docked = Sim::new(9);
        let restored = Sim::from_save(&docked.save_string()).expect("docked save must parse");
        assert_eq!(restored.barter(), docked.barter());
    }

    #[test]
    fn from_save_rejects_garbage() {
        assert_eq!(Sim::from_save("").unwrap_err(), SaveError::BadMagic);
        assert_eq!(Sim::from_save("hello").unwrap_err(), SaveError::BadMagic);
        assert_eq!(
            Sim::from_save("STV9\nseed 1").unwrap_err(),
            SaveError::UnsupportedVersion
        );
        let mangled = Sim::new(1).save_string().replace("seed", "sneed");
        assert_eq!(
            Sim::from_save(&mangled).unwrap_err(),
            SaveError::Parse { line: 2 }
        );
    }
}
