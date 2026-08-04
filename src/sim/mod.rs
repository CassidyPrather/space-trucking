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

pub use barter::{Barter, EAGER_MAX, VALUE};
pub use cargo::{
    KIND_COUNT, Kind, Loc, Piece, Tag, Violation, placement_check, placement_legal, player_owned,
};
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

/// Move `value` toward `target` by at most `step`. The shared easing
/// primitive: it lands exactly on the target, so eased values settle to
/// bit-stable numbers instead of chasing an asymptote.
pub(crate) const fn step_toward(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
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

/// Which regions could accept the held piece.
///
/// Derived from the same ownership matrix [`Sim::resolve_drop`] applies.
/// The renderer glows exactly these, so what the console invites is always
/// what a release does — affordances are computed from the rules, never
/// restated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// One independent flag per console region is the honest shape here; folding
// them into an enum would misrepresent that several can invite at once.
#[allow(clippy::struct_excessive_bools)]
pub struct DropTargets {
    /// The hold grid (player pieces, anywhere, any time).
    pub hold: bool,
    /// The give pad (player pieces, while docked).
    pub give: bool,
    /// The take pad (station pieces, while docked).
    pub take: bool,
    /// The station shelf (station pieces re-shelving, while docked).
    pub shelf: bool,
    /// The received shelf (received pieces re-slotting, while docked).
    pub received: bool,
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
    /// The rule behind the most recent hard reject, for the renderer's
    /// icon flash. Transient UI feedback: never serialized.
    last_violation: Option<Violation>,
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
        let (barter, goods) = barter::generate(seed, GUILD, 1, &pieces, &mut rng, &mut next_piece);
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
            last_violation: None,
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
    /// where it was left. Equivalent to `ticks` calls of [`Sim::advance`]
    /// with [`TICK_DT`] and a default input, minus the cues.
    pub fn fast_forward(&mut self, ticks: u64) -> CatchUp {
        self.cues.clear();
        let ran = if self.paused {
            0
        } else {
            // A default frame carries no pointer, so a drag in progress
            // snaps back exactly as N real advances would snap it.
            self.held = None;
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

    /// The stowage rule behind the most recent hard `Cue::Reject`, for the
    /// renderer's icon flash. Set when that cue fires and cleared by the
    /// next successful placement, so read it in the cue's frame.
    #[must_use]
    pub const fn last_violation(&self) -> Option<Violation> {
        self.last_violation
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

    /// Throw away all state and start over from `seed`. The pause flag
    /// survives (a paused player stays paused); warp does not — a fresh run
    /// starts at cruising speed.
    fn reseed(&mut self, seed: u64) {
        let paused = self.paused;
        *self = Self::new(seed);
        self.paused = paused;
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
            if self.ship.selected.is_some() && !self.pads_occupied() {
                self.depart();
            } else {
                // No destination, or pieces on a pad or the received shelf:
                // launching would strand them, so nothing is ever lost to
                // the lever.
                self.cues.push(Cue::Reject { hard: false });
            }
        } else if layout::ACCEPT_LEVER.contains(p) {
            self.conclude();
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// A release drops the held piece: place it if the target is legal,
    /// snap it back otherwise. A drop never destroys or surrenders a piece
    /// — ownership changes only through the accept lever.
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
                self.last_violation = None;
                self.refresh_ready();
                self.cues.push(Cue::Place);
            }
            Err(violation) => {
                if violation.is_some() {
                    self.last_violation = violation;
                }
                self.cues.push(Cue::Reject {
                    hard: violation.is_some(),
                });
            }
        }
    }

    /// Where dropping `piece` at `p` would settle it, or which flavour of
    /// rejection it earns. `Err(Some(_))` is the hard reject — a stowage
    /// rule refused an in-grid drop — and names the rule; `Err(None)` is a
    /// soft, ignorable miss that snaps the piece home. Every arm gates on
    /// [`player_owned`], the same predicate [`Sim::drop_targets`] advertises
    /// from, so the glowing regions and the legal ones cannot drift apart.
    fn resolve_drop(&self, piece: &Piece, p: Vec2) -> Result<Loc, Option<Violation>> {
        let ours = player_owned(piece.loc);
        if let Some((x, y)) = layout::cell_at(p) {
            if !ours {
                // Station goods never enter the hold before a trade.
                return Err(None);
            }
            return match placement_check(&self.pieces, piece.id, piece.kind, x, y) {
                Ok(()) => Ok(Loc::Hold { x, y }),
                Err(violation) => Err(Some(violation)),
            };
        }
        if self.barter.is_some() {
            if let Some(slot) = layout::slot_at(&layout::GIVE_SLOTS, p) {
                let loc = Loc::GivePad { slot };
                return (ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(None);
            }
            if let Some(slot) = layout::slot_at(&layout::TAKE_SLOTS, p) {
                let loc = Loc::TakePad { slot };
                return (!ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(None);
            }
            if let Some(slot) = layout::slot_at(&layout::SHELF_SLOTS, p) {
                // The shelf is the station's: only its own goods re-shelve
                // here. Player cargo leaves the player through the accept
                // lever (a gift trade), never through a stray drop.
                let loc = Loc::StationShelf { slot };
                return (!ours && self.slot_free(loc, piece.id))
                    .then_some(loc)
                    .ok_or(None);
            }
            if let Some(slot) = layout::slot_at(&layout::RECEIVED_SLOTS, p) {
                let loc = Loc::ReceivedShelf { slot };
                return (matches!(piece.loc, Loc::ReceivedShelf { .. })
                    && self.slot_free(loc, piece.id))
                .then_some(loc)
                .ok_or(None);
            }
        }
        Err(None)
    }

    /// Which regions would accept the held piece, for the renderer to glow.
    /// `None` while nothing is held. Derived from [`player_owned`] and the
    /// dock state exactly as [`Sim::resolve_drop`] is; per-slot freeness
    /// stays with the drop itself (a glowing row with one occupied socket
    /// is still an honest invitation).
    #[must_use]
    pub fn drop_targets(&self) -> Option<DropTargets> {
        let held = self.held?;
        let piece = self.pieces.iter().find(|piece| piece.id == held.piece)?;
        let ours = player_owned(piece.loc);
        let docked = self.barter.is_some();
        Some(DropTargets {
            hold: ours,
            give: ours && docked,
            take: !ours && docked,
            shelf: !ours && docked,
            received: docked && matches!(piece.loc, Loc::ReceivedShelf { .. }),
        })
    }

    /// Re-derive the lever's readiness from the pads this instant. Called
    /// whenever a piece lands or a trade concludes, so the lever lights the
    /// frame a trade becomes viable instead of one tick later. The dial's
    /// sweep still eases in the tick; only readiness is instantaneous.
    fn refresh_ready(&mut self) {
        let ready = barter::eagerness_of(&self.pieces, &self.values).1;
        if let Some(barter) = &mut self.barter {
            barter.ready = ready;
        }
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

    /// Whether any piece sits somewhere departure would strand it: a pad or
    /// the received shelf. The launch lever refuses while this holds.
    fn pads_occupied(&self) -> bool {
        self.pieces.iter().any(|piece| {
            matches!(
                piece.loc,
                Loc::GivePad { .. } | Loc::TakePad { .. } | Loc::ReceivedShelf { .. }
            )
        })
    }

    /// Pull the accept lever: swap the pads if the station agrees, refuse
    /// otherwise. Received goods must be stowed before trading again, so the
    /// received shelf never double-books a slot. Readiness and the deal
    /// value come from the instantaneous pads, not the eased dial, so a
    /// quick pull is never judged by an animation.
    fn conclude(&mut self) {
        if self.barter.is_none() {
            return;
        }
        let (_, ready) = barter::eagerness_of(&self.pieces, &self.values);
        if !ready || self.received_occupied() {
            self.cues.push(Cue::Refuse);
            return;
        }
        let value = barter::deal_value(&self.pieces, &self.values);
        self.restock_from_give_pads();
        for piece in &mut self.pieces {
            if let Loc::TakePad { slot } = piece.loc {
                piece.loc = Loc::ReceivedShelf { slot };
            }
        }
        self.refresh_ready();
        self.cues.push(Cue::Accept { value });
    }

    /// The station consumes the give pads: pieces restock free shelf slots
    /// in pad order — it resells what you sold it — and the overflow
    /// vanishes into the back room.
    fn restock_from_give_pads(&mut self) {
        let mut free: Vec<u8> = (0..layout::SHELF_SLOTS.len() as u8)
            .filter(|&slot| {
                !self
                    .pieces
                    .iter()
                    .any(|piece| piece.loc == Loc::StationShelf { slot })
            })
            .collect();
        free.reverse(); // pop() then yields ascending slots
        let mut given: Vec<(u8, usize)> = self
            .pieces
            .iter()
            .enumerate()
            .filter_map(|(i, piece)| match piece.loc {
                Loc::GivePad { slot } => Some((slot, i)),
                _ => None,
            })
            .collect();
        given.sort_unstable();
        let mut doomed = Vec::new();
        for (_, index) in given {
            match free.pop() {
                Some(slot) => self.pieces[index].loc = Loc::StationShelf { slot },
                None => doomed.push(self.pieces[index].id),
            }
        }
        self.pieces.retain(|piece| !doomed.contains(&piece.id));
    }

    /// Cast off toward the selected destination. Only the station's shelf
    /// stays behind: the launch gate guarantees the pads and the received
    /// shelf are already clear, so no player piece is ever lost here.
    fn depart(&mut self) {
        let ShipState::Docked(from) = self.ship.state else {
            return;
        };
        let Some(to) = self.ship.selected else {
            return;
        };
        debug_assert!(!self.pads_occupied(), "launch gate must clear the pads");
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
            // The dial eases toward the trade's true ratio (capped at the
            // dial's peg); readiness tracks the ratio itself.
            barter.prev_eagerness = barter.eagerness;
            let (target, ready) = barter::eagerness_of(&self.pieces, &self.values);
            barter.ready = ready;
            barter.eagerness = step_toward(
                barter.eagerness,
                target.clamp(0.0, EAGER_MAX),
                barter::EAGER_RATE * TICK_DT,
            );
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
        let (barter, goods) = barter::generate(
            self.seed,
            poi,
            visit,
            &self.pieces,
            &mut self.rng,
            &mut self.next_piece,
        );
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

    /// Mars, the odyssey's destination.
    const MARS: PoiId = 2;

    /// Jupiter: the longest leg from the Guild, for creak statistics.
    const JUPITER: PoiId = 3;

    /// Seed for the scripted odyssey; chosen so its trade is acceptable.
    const ODYSSEY_SEED: u64 = 0x0DDE_55EA;

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

    /// Drag as three zero-dt frames: press, mid-drag hold, release.
    fn drag(sim: &mut Sim, from: Vec2, to: Vec2) {
        sim.advance(0.0, &press_at(from.x, from.y));
        assert!(sim.held().is_some(), "nothing to lift at {from:?}");
        sim.advance(0.0, &held_at(to.x, to.y));
        sim.advance(0.0, &release_at(to.x, to.y));
    }

    /// The same drag appended to a script instead of played live.
    fn drag_frames(script: &mut Vec<(f32, InputFrame)>, from: Vec2, to: Vec2) {
        script.push((0.0, press_at(from.x, from.y)));
        script.push((0.0, held_at(to.x, to.y)));
        script.push((0.0, release_at(to.x, to.y)));
    }

    /// Play a script, logging every cue in order.
    fn play(sim: &mut Sim, script: &[(f32, InputFrame)]) -> Vec<Cue> {
        let mut log = Vec::new();
        for (dt, input) in script {
            sim.advance(*dt, input);
            log.extend_from_slice(sim.cues());
        }
        log
    }

    /// Advance `n` frames of exactly one tick each with default input.
    fn coast(sim: &mut Sim, n: u64) {
        for _ in 0..n {
            sim.advance(TICK_DT, &InputFrame::default());
        }
    }

    /// Test scaffolding: stow an extra piece directly in the hold.
    fn inject_hold(sim: &mut Sim, kind: Kind, x: u8, y: u8) -> u32 {
        let id = sim.next_piece;
        sim.next_piece += 1;
        sim.pieces.push(Piece {
            id,
            kind,
            variant: 0,
            loc: Loc::Hold { x, y },
        });
        assert!(
            placement_legal(&sim.pieces, id, kind, x, y),
            "test piece stowed illegally at ({x}, {y})"
        );
        id
    }

    /// Test scaffolding: park an extra piece on a barter surface.
    fn inject_at(sim: &mut Sim, kind: Kind, loc: Loc) -> u32 {
        let id = sim.next_piece;
        sim.next_piece += 1;
        sim.pieces.push(Piece {
            id,
            kind,
            variant: 0,
            loc,
        });
        id
    }

    /// Select `poi` and pull the launch lever; zero-dt frames run no ticks,
    /// so the sim is exactly at departure.
    fn launched_toward(seed: u64, poi: PoiId) -> Sim {
        let mut sim = Sim::new(seed);
        launch(&mut sim, poi);
        sim
    }

    /// Select `poi` on an already-docked sim and pull the lever.
    fn launch(sim: &mut Sim, poi: PoiId) {
        let target = POIS[usize::from(poi)].pos;
        sim.advance(0.0, &press_at(target.x, target.y));
        assert_eq!(sim.cues(), [Cue::Select]);
        assert_eq!(sim.ship().selected, Some(poi));
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Depart]);
        assert!(matches!(sim.ship().state, ShipState::Traveling { .. }));
    }

    fn launched(seed: u64) -> Sim {
        launched_toward(seed, VENUS)
    }

    /// Launch toward `poi` and fast-forward past the arrival.
    fn travel_to(sim: &mut Sim, poi: PoiId) {
        launch(sim, poi);
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            unreachable!()
        };
        sim.fast_forward(leg_ticks + 10);
        assert_eq!(sim.ship().state, ShipState::Docked(poi));
    }

    /// The current leg's length in ticks.
    fn leg_of(sim: &Sim) -> u64 {
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            panic!("not traveling")
        };
        leg_ticks
    }

    /// The full scripted voyage: offer all three starter pieces, take the
    /// first shelf good, accept, bump into the launch gate, stow, depart
    /// for Mars, warp most of the leg, and coast onto the dock.
    fn odyssey_script() -> Vec<(f32, InputFrame)> {
        let mut s = Vec::new();
        // Starter cargo to the give pads (vial, scrap, pearls).
        drag_frames(
            &mut s,
            cell_center(0, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        drag_frames(
            &mut s,
            cell_center(0, 2),
            slot_center(&layout::GIVE_SLOTS, 1),
        );
        drag_frames(
            &mut s,
            cell_center(2, 0),
            slot_center(&layout::GIVE_SLOTS, 2),
        );
        // First shelf good to the take pad.
        drag_frames(
            &mut s,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        // Let the dial swing a moment, then pull accept.
        for _ in 0..30 {
            s.push((TICK_DT, InputFrame::default()));
        }
        let accept = rect_center(layout::ACCEPT_LEVER);
        s.push((0.0, press_at(accept.x, accept.y)));
        // Select Mars and hit the lever too early: received still loaded.
        let mars = POIS[usize::from(MARS)].pos;
        s.push((0.0, press_at(mars.x, mars.y)));
        let launch = rect_center(layout::LAUNCH_LEVER);
        s.push((0.0, press_at(launch.x, launch.y)));
        // Stow the received good — (0, 2) suits every kind on an empty
        // board — and launch for real.
        drag_frames(
            &mut s,
            slot_center(&layout::RECEIVED_SLOTS, 0),
            cell_center(0, 2),
        );
        s.push((0.0, press_at(launch.x, launch.y)));
        // Warp sixteen-fold through most of the leg, then coast in.
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        s.push((0.0, warp));
        for _ in 0..200 {
            s.push((TICK_DT, InputFrame::default()));
        }
        s.push((0.0, warp));
        for _ in 0..400 {
            s.push((TICK_DT, InputFrame::default()));
        }
        s
    }

    /// How many cues in `log` match `pred`.
    fn count_cues(log: &[Cue], pred: impl Fn(&Cue) -> bool) -> usize {
        log.iter().filter(|cue| pred(cue)).count()
    }

    /// Save `original`, load it, then drive both through the same frames
    /// (with a warp toggle sprinkled in) and demand identical states and
    /// identical re-saves.
    fn assert_save_continues(mut original: Sim, frames: usize) {
        let saved = original.save_string();
        let mut restored = Sim::from_save(&saved).expect("own save must parse");
        for i in 0..frames {
            let input = if i % 977 == 0 {
                InputFrame {
                    toggle_warp: true,
                    ..InputFrame::default()
                }
            } else {
                InputFrame::default()
            };
            original.advance(TICK_DT, &input);
            restored.advance(TICK_DT, &input);
        }
        assert_eq!(original.save_string(), restored.save_string());
        assert_eq!(original.pieces(), restored.pieces());
        assert_eq!(original.barter(), restored.barter());
        assert_eq!(original.ship(), restored.ship());
        assert_eq!(original.tick(), restored.tick());
        assert_eq!(original.light().to_bits(), restored.light().to_bits());
        assert_eq!(original.omen().to_bits(), restored.omen().to_bits());
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

    #[test]
    fn long_scripted_run_is_bit_identical() {
        let script = odyssey_script();
        let mut a = Sim::new(ODYSSEY_SEED);
        let mut b = Sim::new(ODYSSEY_SEED);
        let cues_a = play(&mut a, &script);
        let cues_b = play(&mut b, &script);
        assert_eq!(cues_a, cues_b, "cue streams diverged");
        // The choreography really happened: a concluded trade, a refused
        // early launch, one departure, warp both ways, one arrival.
        assert_eq!(count_cues(&cues_a, |c| matches!(c, Cue::Accept { .. })), 1);
        assert_eq!(count_cues(&cues_a, |c| matches!(c, Cue::Depart)), 1);
        assert_eq!(count_cues(&cues_a, |c| matches!(c, Cue::Arrive)), 1);
        assert_eq!(count_cues(&cues_a, |c| matches!(c, Cue::Warp { .. })), 2);
        assert!(count_cues(&cues_a, |c| matches!(c, Cue::Reject { hard: false })) >= 1);
        assert_eq!(a.ship().state, ShipState::Docked(MARS));
        assert!(a.barter().is_some());
        assert_eq!(a.save_string(), b.save_string());
        assert_eq!(a.pieces(), b.pieces());
        assert_eq!(a.barter(), b.barter());
        assert_eq!(a.ship(), b.ship());
        assert_eq!(a.tick(), b.tick());
    }

    #[test]
    fn save_load_continues_mid_travel() {
        let mut sim = Sim::new(ODYSSEY_SEED);
        play(&mut sim, &odyssey_script());
        launch(&mut sim, VENUS);
        coast(&mut sim, 1000);
        assert!(matches!(sim.ship().state, ShipState::Traveling { .. }));
        assert_save_continues(sim, 10_000);
    }

    #[test]
    fn save_load_continues_docked_with_a_composed_trade() {
        let mut sim = Sim::new(ODYSSEY_SEED);
        play(&mut sim, &odyssey_script());
        coast(&mut sim, 7000);
        // Compose but do not conclude, then save with the dial mid-swing.
        drag(
            &mut sim,
            cell_center(0, 2),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        drag(
            &mut sim,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        coast(&mut sim, 7);
        assert!(
            sim.pieces()
                .iter()
                .any(|p| matches!(p.loc, Loc::TakePad { .. })),
            "the composed trade fell apart"
        );
        assert_save_continues(sim, 10_000);
    }

    #[test]
    fn save_load_continues_mid_omen() {
        let mut sim = Sim::new(0xB00);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        launch(&mut sim, VENUS);
        // Walk to the omen's opening, then a beat further: light mid-dim,
        // hum mid-swell, jump still pending.
        for _ in 0..leg_of(&sim) {
            sim.advance(TICK_DT, &InputFrame::default());
            if sim.cues().contains(&Cue::OmenStart) {
                break;
            }
        }
        coast(&mut sim, 60);
        assert!(
            sim.omen() > 0.0 && sim.light() < 1.0,
            "not inside the omen: omen {} light {}",
            sim.omen(),
            sim.light()
        );
        assert_save_continues(sim, 8_000);
    }

    #[test]
    fn save_load_continues_mid_drag_with_piece_at_origin() {
        let mut sim = Sim::new(11);
        let vial = cell_center(0, 0);
        sim.advance(0.0, &press_at(vial.x, vial.y));
        let held = sim.held().expect("lifted the vial").piece;
        let saved = sim.save_string();
        let restored = Sim::from_save(&saved).expect("mid-drag save parses");
        assert!(restored.held().is_none(), "held state is transient");
        let loc_of = |s: &Sim| s.pieces().iter().find(|p| p.id == held).unwrap().loc;
        assert_eq!(
            loc_of(&restored),
            Loc::Hold { x: 0, y: 0 },
            "held piece must serialize at its origin"
        );
        assert_eq!(loc_of(&restored), loc_of(&sim));
        assert_save_continues(sim, 300);
    }

    #[test]
    fn fast_forward_matches_stepwise_across_a_dock() {
        let base = launched(0xCAFE);
        let n = leg_of(&base) + 123;
        let mut ff = base.clone();
        let report = ff.fast_forward(n);
        assert_eq!(
            report,
            CatchUp {
                ticks: n,
                arrived: true,
                jumped: false
            }
        );
        assert!(ff.cues().is_empty(), "fast_forward must suppress cues");
        let mut step = base;
        for _ in 0..n {
            step.advance(TICK_DT, &InputFrame::default());
        }
        assert_eq!(ff.save_string(), step.save_string());
        assert_eq!(ff.pieces(), step.pieces());
        assert_eq!(ff.barter(), step.barter());
        assert_eq!(ff.ship(), step.ship());
    }

    #[test]
    fn fast_forward_matches_stepwise_across_an_omen_jump() {
        let mut base = Sim::new(0xD1CE);
        inject_hold(&mut base, Kind::SuspiciousCrate, 3, 0);
        launch(&mut base, VENUS);
        // Past the latest possible jump, not necessarily past the dock.
        let n = leg_of(&base) * 3 / 4 + 200;
        let mut ff = base.clone();
        let report = ff.fast_forward(n);
        assert!(report.jumped, "no jump despite the crate");
        assert_eq!(report.ticks, n);
        assert_eq!(
            report.arrived,
            matches!(ff.ship().state, ShipState::Docked(_))
        );
        assert!(ff.cues().is_empty());
        let mut step = base;
        for _ in 0..n {
            step.advance(TICK_DT, &InputFrame::default());
        }
        assert_eq!(ff.save_string(), step.save_string());
        assert_eq!(ff.light().to_bits(), step.light().to_bits());
        assert_eq!(ff.omen().to_bits(), step.omen().to_bits());
    }

    #[test]
    fn fast_forward_while_paused_stays_put() {
        let mut sim = launched(9);
        sim.advance(
            0.0,
            &InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        let before = sim.save_string();
        let report = sim.fast_forward(50_000);
        assert_eq!(
            report,
            CatchUp {
                ticks: 0,
                arrived: false,
                jumped: false
            }
        );
        assert_eq!(sim.save_string(), before);
    }

    /// Offer all three starters against the first shelf good, as zero-dt
    /// frames on a fresh sim.
    fn compose_starter_trade(sim: &mut Sim) {
        drag(sim, cell_center(0, 0), slot_center(&layout::GIVE_SLOTS, 0));
        drag(sim, cell_center(0, 2), slot_center(&layout::GIVE_SLOTS, 1));
        drag(sim, cell_center(2, 0), slot_center(&layout::GIVE_SLOTS, 2));
        drag(
            sim,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
    }

    /// Ids of pieces on the give pads, in pad order.
    fn give_ids(sim: &Sim) -> Vec<u32> {
        let mut given: Vec<(u8, u32)> = sim
            .pieces()
            .iter()
            .filter_map(|p| match p.loc {
                Loc::GivePad { slot } => Some((slot, p.id)),
                _ => None,
            })
            .collect();
        given.sort_unstable();
        given.into_iter().map(|(_, id)| id).collect()
    }

    #[test]
    fn accept_restocks_the_shelf_and_delivers_to_received() {
        let mut sim = Sim::new(ODYSSEY_SEED);
        compose_starter_trade(&mut sim);
        let taken = sim
            .pieces()
            .iter()
            .find(|p| matches!(p.loc, Loc::TakePad { .. }))
            .expect("something on the take pad")
            .id;
        let given = give_ids(&sim);
        assert_eq!(given.len(), 3);
        let free: Vec<u8> = (0..layout::SHELF_SLOTS.len() as u8)
            .filter(|&slot| {
                !sim.pieces()
                    .iter()
                    .any(|p| p.loc == Loc::StationShelf { slot })
            })
            .collect();
        let accept = rect_center(layout::ACCEPT_LEVER);
        sim.advance(0.0, &press_at(accept.x, accept.y));
        let &[Cue::Accept { value }] = sim.cues() else {
            panic!("lever did not accept: {:?}", sim.cues())
        };
        assert!((0.0..=1.0).contains(&value), "overshoot {value} unclamped");
        // The taken good waits on the received shelf, same slot number.
        let loc_of = |s: &Sim, id: u32| s.pieces().iter().find(|p| p.id == id).map(|p| p.loc);
        assert_eq!(loc_of(&sim, taken), Some(Loc::ReceivedShelf { slot: 0 }));
        // The given goods restock the freed shelf slots in pad order; any
        // overflow is gone for good.
        for (i, &id) in given.iter().enumerate() {
            match free.get(i) {
                Some(&slot) => {
                    assert_eq!(loc_of(&sim, id), Some(Loc::StationShelf { slot }));
                }
                None => assert_eq!(loc_of(&sim, id), None, "overflow piece survived"),
            }
        }
        assert!(give_ids(&sim).is_empty(), "give pads must be consumed");
        // A received good may return to the give pad for the next round.
        drag(
            &mut sim,
            slot_center(&layout::RECEIVED_SLOTS, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        assert_eq!(sim.cues(), [Cue::Place]);
        assert_eq!(loc_of(&sim, taken), Some(Loc::GivePad { slot: 0 }));
    }

    #[test]
    fn accept_lever_gates_on_the_received_shelf() {
        let mut sim = Sim::new(ODYSSEY_SEED);
        compose_starter_trade(&mut sim);
        let accept = rect_center(layout::ACCEPT_LEVER);
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert!(matches!(sim.cues(), [Cue::Accept { .. }]));
        // Compose a provably fair second trade while the received good
        // still waits: enough pearls to cover the asked good's cost.
        let ask = sim
            .pieces()
            .iter()
            .find(|p| p.loc == Loc::StationShelf { slot: 0 })
            .expect("restocked shelf")
            .id;
        drag(
            &mut sim,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        let ask_kind = sim.pieces().iter().find(|p| p.id == ask).unwrap().kind;
        let cost = usize::from(sim.values[ask_kind.index()]) + 1;
        assert!(cost <= 4, "cost {cost} needs more give slots than exist");
        for slot in 0..cost as u8 {
            inject_at(&mut sim, Kind::BrinePearls, Loc::GivePad { slot });
        }
        let before = sim.pieces().to_vec();
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert_eq!(sim.cues(), [Cue::Refuse], "received shelf must gate");
        assert_eq!(sim.pieces(), before, "a refusal moves nothing");
        // Stow the received good; the same pull now concludes.
        drag(
            &mut sim,
            slot_center(&layout::RECEIVED_SLOTS, 0),
            cell_center(0, 2),
        );
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert!(
            matches!(sim.cues(), [Cue::Accept { .. }]),
            "cleared shelf still refused: {:?}",
            sim.cues()
        );
    }

    #[test]
    fn refuse_moves_nothing() {
        let mut sim = Sim::new(7);
        // Ask without offering: never ready.
        drag(
            &mut sim,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        let before = sim.pieces().to_vec();
        let accept = rect_center(layout::ACCEPT_LEVER);
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert_eq!(sim.cues(), [Cue::Refuse]);
        assert_eq!(sim.pieces(), before);
    }

    #[test]
    fn own_cargo_dropped_on_the_shelf_snaps_back() {
        // A run whose opening shelf has a free slot, so the refusal below is
        // an ownership call and not a full-slot technicality.
        let seed = (0_u64..64)
            .find(|&s| {
                Sim::new(s)
                    .pieces()
                    .iter()
                    .filter(|p| matches!(p.loc, Loc::StationShelf { .. }))
                    .count()
                    < 4
            })
            .expect("no shelf with a free slot in 64 seeds");
        let mut sim = Sim::new(seed);
        let free_slot = (0..4)
            .find(|&slot| {
                !sim.pieces()
                    .iter()
                    .any(|p| p.loc == Loc::StationShelf { slot })
            })
            .unwrap();
        let gap = Vec2::new(
            layout::SHELF_SLOTS[0].x + layout::SHELF_SLOTS[0].w + 3.0,
            layout::SHELF_SLOTS[0].y + 20.0,
        );
        let before = sim.pieces().to_vec();
        // Neither an open shelf slot nor the gap between slots takes player
        // cargo: the shelf is the station's. Nothing is ever lost to a drop.
        for target in [
            slot_center(&layout::SHELF_SLOTS, usize::from(free_slot)),
            gap,
        ] {
            drag(&mut sim, cell_center(0, 0), target);
            assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
            assert_eq!(sim.pieces(), before);
        }
        // From a give pad the shelf refuses all the same.
        drag(
            &mut sim,
            cell_center(0, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        drag(
            &mut sim,
            slot_center(&layout::GIVE_SLOTS, 0),
            slot_center(&layout::SHELF_SLOTS, usize::from(free_slot)),
        );
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert_eq!(sim.pieces().len(), before.len());
    }

    #[test]
    fn a_pure_gift_clears_the_hold_through_the_lever() {
        let mut sim = Sim::new(1);
        let owned_before = sim.pieces().iter().filter(|p| player_owned(p.loc)).count();
        // Vial to the give pad, take pad empty: the dial must arm.
        drag(
            &mut sim,
            cell_center(0, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        let barter = sim.barter().unwrap();
        assert!(barter.ready, "a pure gift must arm the accept lever");
        let accept = rect_center(layout::ACCEPT_LEVER);
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert!(
            matches!(sim.cues(), [Cue::Accept { .. }]),
            "gift refused: {:?}",
            sim.cues()
        );
        // The vial left the player's side through the ceremony — the only
        // door cargo ever leaves through.
        let owned_after = sim.pieces().iter().filter(|p| player_owned(p.loc)).count();
        assert_eq!(owned_after, owned_before - 1);
        assert!(
            !sim.pieces()
                .iter()
                .any(|p| matches!(p.loc, Loc::GivePad { .. })),
            "give pad should be consumed"
        );
    }

    #[test]
    fn drop_targets_come_from_the_ownership_matrix() {
        let mut sim = Sim::new(1);
        assert_eq!(sim.drop_targets(), None, "nothing held, nothing invited");
        // Hold a player piece: hold and give invite; station rows never do.
        let vial = cell_center(0, 0);
        sim.advance(0.0, &press_at(vial.x, vial.y));
        assert_eq!(
            sim.drop_targets(),
            Some(DropTargets {
                hold: true,
                give: true,
                take: false,
                shelf: false,
                received: false,
            })
        );
        sim.advance(0.0, &release_at(vial.x, vial.y));
        // Hold a station piece: only its own furniture invites.
        let shelf_piece = sim
            .pieces()
            .iter()
            .find(|p| matches!(p.loc, Loc::StationShelf { .. }))
            .expect("opening shelf is never empty");
        let at = rect_center(layout::piece_rect(shelf_piece));
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(
            sim.drop_targets(),
            Some(DropTargets {
                hold: false,
                give: false,
                take: true,
                shelf: true,
                received: false,
            })
        );
    }

    /// The conservation drag-monkey: thousands of arbitrary press/hold/
    /// release frames — including nonsense the frontend would never send —
    /// must never cost the player a piece outside an accept. This is the
    /// standing guard for the whole class of "dragged it somewhere and it
    /// vanished" bugs; every new interactive surface is automatically under
    /// test the moment it exists.
    #[test]
    fn no_input_stream_loses_cargo_without_an_accept() {
        let mut sim = Sim::new(0xC0FF_EE00);
        let mut rng = fastrand::Rng::with_seed(0xF00D);
        let owned = |sim: &Sim| sim.pieces().iter().filter(|p| player_owned(p.loc)).count();
        let mut before = owned(&sim);
        for frame in 0_u32..6000 {
            let input = InputFrame {
                pointer: Vec2::new(rng.f32() * WORLD_W, rng.f32() * WORLD_H),
                press: rng.bool(),
                held: rng.bool(),
                release: rng.bool(),
                toggle_pause: rng.u8(..) < 3,
                toggle_warp: rng.u8(..) < 3,
                reseed: None,
            };
            sim.advance(TICK_DT, &input);
            let accepted = sim
                .cues()
                .iter()
                .any(|cue| matches!(cue, Cue::Accept { .. }));
            let after = owned(&sim);
            assert!(
                after >= before || accepted,
                "frame {frame}: {before} -> {after} player pieces with no accept"
            );
            // Held pieces must stay real: a lifted id always exists.
            if let Some(held) = sim.held() {
                assert!(
                    sim.pieces().iter().any(|p| p.id == held.piece),
                    "frame {frame}: held piece {} is a ghost",
                    held.piece
                );
            }
            before = after;
        }
    }

    #[test]
    fn launch_gate_refuses_while_pads_hold_pieces() {
        let mut sim = Sim::new(3);
        let venus = POIS[usize::from(VENUS)].pos;
        sim.advance(0.0, &press_at(venus.x, venus.y));
        let lever = rect_center(layout::LAUNCH_LEVER);
        // Give pad occupied: refused.
        drag(
            &mut sim,
            cell_center(0, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
        // Cleared back to the hold, but now the take pad is loaded: refused.
        drag(
            &mut sim,
            slot_center(&layout::GIVE_SLOTS, 0),
            cell_center(0, 0),
        );
        drag(
            &mut sim,
            slot_center(&layout::SHELF_SLOTS, 0),
            slot_center(&layout::TAKE_SLOTS, 0),
        );
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
        // Ask withdrawn: the lever finally throws.
        drag(
            &mut sim,
            slot_center(&layout::TAKE_SLOTS, 0),
            slot_center(&layout::SHELF_SLOTS, 0),
        );
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Depart]);
    }

    #[test]
    fn station_goods_never_enter_the_hold_or_give_pads() {
        let mut sim = Sim::new(3);
        let shelf0 = slot_center(&layout::SHELF_SLOTS, 0);
        let before = sim.pieces().to_vec();
        drag(&mut sim, shelf0, cell_center(5, 3));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        drag(&mut sim, shelf0, slot_center(&layout::GIVE_SLOTS, 0));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert_eq!(sim.pieces(), before);
        // Shelf to take pad remains the legal path.
        drag(&mut sim, shelf0, slot_center(&layout::TAKE_SLOTS, 1));
        assert_eq!(sim.cues(), [Cue::Place]);
    }

    #[test]
    fn hard_rejects_name_their_rule() {
        let mut sim = Sim::new(5);
        assert_eq!(sim.last_violation(), None);
        // The vial dropped onto the scrap: overlap.
        drag(&mut sim, cell_center(0, 0), cell_center(0, 2));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert_eq!(sim.last_violation(), Some(Violation::Overlap));
        // The scrap lifted high into the rack: heavy.
        drag(&mut sim, cell_center(0, 2), cell_center(3, 0));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert_eq!(sim.last_violation(), Some(Violation::Heavy));
        // A soft miss over dead space leaves the record alone...
        drag(&mut sim, cell_center(0, 0), Vec2::new(255.0, 300.0));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert_eq!(sim.last_violation(), Some(Violation::Heavy));
        // ...and a successful placement clears it.
        drag(&mut sim, cell_center(0, 0), cell_center(5, 3));
        assert_eq!(sim.cues(), [Cue::Place]);
        assert_eq!(sim.last_violation(), None);
    }

    #[test]
    fn guild_never_offers_a_crate_while_one_is_aboard() {
        let mut sim = Sim::new(0x0051_C0DE);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        for _ in 0..6 {
            travel_to(&mut sim, VENUS);
            travel_to(&mut sim, GUILD);
            let crates = sim
                .pieces()
                .iter()
                .filter(|p| p.kind == Kind::SuspiciousCrate)
                .count();
            assert_eq!(crates, 1, "the singleton rule broke");
        }
    }

    #[test]
    fn omen_fires_exactly_once_at_the_derived_tick() {
        let seed = 0xE7E7;
        let mut sim = Sim::new(seed);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        launch(&mut sim, VENUS);
        let leg = leg_of(&sim);
        let lo = leg / 2;
        let jump_at = lo + splitmix(seed, 1) % (leg * 3 / 4 - lo).max(1);
        let mut starts = Vec::new();
        let mut jumps = Vec::new();
        let mut ends = Vec::new();
        let mut skipped_to = None;
        let mut light_at_jump = None;
        let mut peak_omen = 0.0_f32;
        while matches!(sim.ship().state, ShipState::Traveling { .. }) {
            sim.advance(TICK_DT, &InputFrame::default());
            for cue in sim.cues() {
                match cue {
                    Cue::OmenStart => starts.push(sim.tick()),
                    Cue::Jump => {
                        jumps.push(sim.tick());
                        light_at_jump = Some(sim.light());
                        if let ShipState::Traveling { progress, .. } = sim.ship().state {
                            skipped_to = Some(progress);
                        }
                    }
                    Cue::OmenEnd => ends.push(sim.tick()),
                    _ => {}
                }
            }
            let (light, omen) = (sim.light(), sim.omen());
            peak_omen = peak_omen.max(omen);
            assert!((0.0..=1.0).contains(&light), "light {light} out of range");
            assert!((0.0..=1.0).contains(&omen), "omen {omen} out of range");
            assert!(sim.tick() < leg * 2, "the leg never ended");
        }
        // Launch was tick zero, so progress and tick agree: the episode
        // lands exactly where the hash said, once.
        assert_eq!(starts, [jump_at]);
        assert_eq!(jumps, [jump_at + 180]);
        assert_eq!(ends, [jump_at + 270]);
        let pre = jump_at + 180;
        assert_eq!(
            skipped_to,
            Some(pre + (leg - pre) * 3 / 4),
            "jump must skip three quarters of the remaining leg"
        );
        // Three seconds of omen more than covers the two-second eases: the
        // light bottoms out at its dim floor and the hum peaks at full.
        let light = light_at_jump.unwrap();
        assert!((light - 0.2).abs() < 1e-6, "light at jump was {light}");
        assert!((peak_omen - 1.0).abs() < 1e-6, "hum peaked at {peak_omen}");
        // Back at a dock the ambience settles exactly.
        coast(&mut sim, 150);
        assert_eq!(sim.light().to_bits(), 1.0_f32.to_bits());
        assert_eq!(sim.omen().to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn without_a_crate_the_leg_never_jumps() {
        let mut sim = launched(0x30);
        let leg = leg_of(&sim);
        for _ in 0..=leg {
            sim.advance(TICK_DT, &InputFrame::default());
            assert!(
                sim.cues()
                    .iter()
                    .all(|c| !matches!(c, Cue::OmenStart | Cue::Jump | Cue::OmenEnd)),
                "omen without a crate"
            );
            assert_eq!(sim.light().to_bits(), 1.0_f32.to_bits());
            assert_eq!(sim.omen().to_bits(), 0.0_f32.to_bits());
        }
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
    }

    #[test]
    fn creaks_pepper_travel_and_never_the_dock() {
        let mut sim = launched_toward(0xC4EA, JUPITER);
        let mut creaks = 0_i64;
        for _ in 0..6000 {
            sim.advance(TICK_DT, &InputFrame::default());
            for cue in sim.cues() {
                if let Cue::Creak { intensity } = cue {
                    creaks += 1;
                    assert!((0.0..=1.0).contains(intensity), "creak {intensity}");
                }
            }
        }
        assert!(
            matches!(sim.ship().state, ShipState::Traveling { .. }),
            "the Jupiter leg should outlast the sample"
        );
        // One creak is scheduled per window; allow slack at the borders.
        let windows = 6000 / 480;
        assert!(
            (windows - 2..=windows + 2).contains(&creaks),
            "{creaks} creaks in {windows} windows"
        );
        // Docked, the hull is quiet.
        let mut docked = Sim::new(0xC4EA);
        for _ in 0..2000 {
            docked.advance(TICK_DT, &InputFrame::default());
            assert!(
                docked
                    .cues()
                    .iter()
                    .all(|c| !matches!(c, Cue::Creak { .. })),
                "creak at the dock"
            );
        }
    }

    #[test]
    fn reseed_preserves_pause_and_resets_warp() {
        let mut sim = Sim::new(1);
        sim.advance(
            0.0,
            &InputFrame {
                toggle_warp: true,
                ..InputFrame::default()
            },
        );
        sim.advance(
            0.0,
            &InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        assert!(sim.is_warp() && sim.is_paused());
        sim.advance(
            0.0,
            &InputFrame {
                reseed: Some(2),
                ..InputFrame::default()
            },
        );
        assert_eq!(sim.cues(), [Cue::Reseed]);
        assert!(sim.is_paused(), "pause must survive a reseed");
        assert!(!sim.is_warp(), "warp must reset with the world");
        assert_eq!(sim.seed(), 2);
        assert_eq!(sim.tick(), 0);
    }
}
