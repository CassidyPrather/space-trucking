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
//! The sim is crewed: up to [`MAX_CREW`] players share one ship, each with
//! their own pointer and drag. [`Sim::crew_tick`] is the lockstep entry
//! point — one sealed [`CrewFrame`] in, exactly one fixed step out — and
//! [`Sim::advance`] is the solo frontend's wrapper around the same
//! machinery with player 0's input and the accumulator on top.
//!
//! Sound follows the same rule as pixels: the sim reports that something
//! happened and how hard, as a [`Cue`], and the frontend decides what that
//! sounds like. No audio types cross into this module.

pub mod barter;
pub mod cargo;
mod event;
pub mod layout;
pub mod map;
mod rats;
pub mod save;

use std::ops::{Add, AddAssign, Mul, MulAssign, Sub};

pub use barter::{Barter, EAGER_MAX, VALUE};
pub use cargo::{
    KIND_COUNT, Kind, Loc, Piece, Tag, Violation, first_fit, placement_check, placement_legal,
    player_owned,
};
use event::Omen;
pub use map::{
    COMET, GUILD, HERMITAGE, INNER_RING, POI_COUNT, POIS, Poi, PoiId, SATURN, SHIP_SPEED, SUN,
    Ship, ShipState, Track, UMBRA, WANDERER, comet_visible, leg_endpoints, poi_pos,
};
pub use rats::Rat;
use rats::Rats;
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

/// Mysterious crates ??? asks for, and consumes, in one exchange.
pub const WANDERER_TOLL: u32 = 3;

/// Salt for the comet harvest rolls, distinct from every other stream.
const SALT_HARVEST: u64 = 0xC0_3E71;

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

/// Index of one crew member, `0..MAX_CREW`. Player 0 is the solo player.
pub type PlayerId = u8;

/// Most players that can crew one ship.
pub const MAX_CREW: usize = 6;

/// One sealed tick's input for the whole crew; absent players are default.
pub type CrewFrame = [InputFrame; MAX_CREW];

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
    /// Quick-move modifier held (Shift): a press moves the piece under the
    /// pointer straight to its obvious destination instead of lifting it.
    pub shift: bool,
    /// Whether the player's own wall clock reads deep night (23:30–06:00
    /// local). The frontend measures it, the sim only ever consults it
    /// inside press handlers — never in the tick — so sparse recordings
    /// stay exact.
    pub night: bool,
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
    /// The Guild seized a suspicious crate at its dock and shuttled it to
    /// the hangar. Fires alongside `Arrive`, before that visit's barter.
    Delivered,
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
    /// Free cargo came aboard at a barterless dock: comet ice chipped at
    /// perihelion, `intensity` scaling with the haul.
    Harvest {
        intensity: f32,
    },
    /// ??? took three mysterious crates and left one very mysterious one.
    Exchange,
    /// Ambient hull creak while traveling.
    Creak {
        intensity: f32,
    },
    /// A rat stowed away as the ship cast off.
    RatAboard,
    /// The rat hopped to another hold cell. Quiet; ambient texture.
    RatSkitter {
        intensity: f32,
    },
    /// The rat gnawed the piece nearest its cell (see `rats` for the rule).
    RatNibble,
    /// A press on the rat's cell shooed it; it relocated instantly.
    RatChased,
    /// The rat left the ship — walked off at a lean-hold dock, or driven
    /// off by the third chase.
    RatLeft,
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

/// A piece mid-drag. Each crew member can hold at most one, and a piece
/// held by anyone is unGrabbable by everyone else.
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
    /// Each crew member's drag in progress, indexed by [`PlayerId`].
    held: [Option<Held>; MAX_CREW],
    /// Crates the Guild has seized at its dock, monotonic within a run.
    /// This is the counter cluster helms report to the guild server.
    deliveries: u32,
    /// The current visit's trade, `Some` iff docked.
    barter: Option<Barter>,
    /// The current visit's jittered value table; meaningful iff docked.
    values: [u8; KIND_COUNT],
    /// Times each POI has been docked at.
    visits: [u32; POI_COUNT],
    /// Departures so far, salting each leg's event schedules. Shared by the
    /// event siblings, so it lives here rather than in either of them.
    legs: u64,
    omen: Omen,
    rats: Rats,
    /// Pieces ever gifted to the Hermitage; its shelf grows from this.
    karma: u32,
    /// Whether any crew member's wall clock reads deep night, refreshed
    /// from the frame every application round. Transient: consulted only
    /// by press handlers (Umbra selection), never by the tick, and never
    /// serialized.
    night: bool,
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
                    gnawed: false,
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
        let (barter, goods) =
            barter::generate(seed, GUILD, 1, &pieces, 0, &mut rng, &mut next_piece);
        pieces.extend(goods);
        let pos = map::poi_pos(GUILD, 0);

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
            held: [None; MAX_CREW],
            deliveries: 0,
            barter: Some(barter),
            values: barter::visit_values(seed, GUILD, 1),
            visits,
            legs: 0,
            omen: Omen::new(),
            rats: Rats::new(),
            karma: 0,
            night: false,
            last_violation: None,
        }
    }

    /// Consume one frame's worth of real time as player 0, returning how
    /// many fixed ticks ran. `frame_dt` is clamped to [`MAX_FRAME_DT`]; warp
    /// multiplies both the frame and the clamp by [`WARP_FACTOR`]. The solo
    /// frontend's entry point; lockstep replicas call [`Sim::crew_tick`].
    pub fn advance(&mut self, frame_dt: f32, input: &InputFrame) -> u32 {
        // Cues describe this frame only; last frame's have been consumed.
        self.cues.clear();
        self.night = input.night;
        let mut placed = Vec::new();
        self.apply_input(0, input, &mut placed);
        if self.paused {
            return 0;
        }

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

    /// Apply one sealed tick's crew inputs and run exactly one fixed step.
    /// No accumulator is involved; [`Sim::alpha`] is untouched.
    ///
    /// This is the lockstep entry point: every replica calls it once per
    /// sealed tick, and inputs are applied **in player order
    /// `0..MAX_CREW`** — that ordering is the determinism. The rules that
    /// order implies:
    ///
    /// - Grabs: the lowest-index player pressing on a free piece wins it;
    ///   a press on a piece someone already holds (this tick or earlier)
    ///   does nothing, silently.
    /// - Drops: releases resolve in order against the world as earlier
    ///   players left it; a release refused only because an earlier player
    ///   just took the spot snaps back with a soft reject.
    /// - Toggles: each player's pause/warp edge flips the flag in order and
    ///   announces its cue, so two pause toggles in one tick net to no
    ///   change and read `Pause { true }` then `Pause { false }`.
    /// - Pause: a player whose input lands while the sim is paused still
    ///   toggles, but their pointer events are ignored, exactly as
    ///   [`Sim::advance`] ignores them.
    /// - Reseed: rebuilds the world where it lands (discarding earlier
    ///   players' cues with the rest of the old state), so the last reseed
    ///   in order wins.
    ///
    /// Warp does not multiply this function: one call is one tick, and a
    /// warped lockstep session realises the speed-up by sealing ticks
    /// faster. [`Sim::is_warp`] still reports the flag for that purpose.
    pub fn crew_tick(&mut self, inputs: &CrewFrame) {
        self.cues.clear();
        // Night is the union of the crew's clocks: if it is deep night for
        // anyone aboard, the Umbra Market will see the ship.
        self.night = inputs.iter().any(|input| input.night);
        let mut placed = Vec::new();
        for (player, input) in inputs.iter().enumerate() {
            self.apply_input(player as PlayerId, input, &mut placed);
        }
        if !self.paused {
            self.step();
        }
    }

    /// One player's input events: reseed, toggles, then — unless the sim is
    /// paused by the time their turn comes — pointer edges. `placed` carries
    /// the pieces successfully dropped earlier in the same application
    /// round, for same-tick drop contention.
    fn apply_input(&mut self, player: PlayerId, input: &InputFrame, placed: &mut Vec<u32>) {
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
            return;
        }

        // Pointer edges are per-frame events, exactly like the template's
        // burst was: handling them inside the tick loop would fire them once
        // per tick.
        self.handle_pointer(player, input, placed);
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
            // A default frame carries no pointer, so every drag in progress
            // snaps back exactly as N real advances would snap it.
            self.held = [None; MAX_CREW];
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

    /// The star map's points of interest (tracks and radii; positions are
    /// a function of the tick — see [`Sim::poi_pos`]).
    #[must_use]
    pub const fn pois(&self) -> &[Poi; POI_COUNT] {
        &POIS
    }

    /// Where POI `id` is right now.
    #[must_use]
    pub fn poi_pos(&self, id: PoiId) -> Vec2 {
        map::poi_pos(id, self.tick)
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

    /// The piece `player` is mid-drag with, if any. Out-of-range players
    /// hold nothing.
    #[must_use]
    pub fn held(&self, player: PlayerId) -> Option<&Held> {
        self.held.get(usize::from(player))?.as_ref()
    }

    /// Every drag in progress, in player order.
    pub fn all_held(&self) -> impl Iterator<Item = (PlayerId, &Held)> {
        self.held
            .iter()
            .enumerate()
            .filter_map(|(player, held)| held.as_ref().map(|held| (player as PlayerId, held)))
    }

    /// Crates the Guild has seized at its dock, monotonic within a run.
    /// The frontend's lamp plate reads this, and helms report it upstream.
    #[must_use]
    pub const fn deliveries(&self) -> u32 {
        self.deliveries
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
        self.omen.light
    }

    /// Departures so far this run: how many legs the ship has ever flown.
    /// Zero means a rig that has never left its first dock — which is what
    /// keeps the onboarding ghost's first lesson eligible.
    #[must_use]
    pub const fn legs(&self) -> u64 {
        self.legs
    }

    /// The stowaway rat, if one is aboard. The renderer derives its hop
    /// tween from `prev_cell` and `moved_at` plus [`Sim::alpha`].
    #[must_use]
    pub const fn rat(&self) -> Option<Rat> {
        self.rats.rat
    }

    /// Pieces ever gifted to the Hermitage this run.
    #[must_use]
    pub const fn karma(&self) -> u32 {
        self.karma
    }

    /// Whether POI `id` is currently on the map at all. The comet exists
    /// only near perihelion, the Umbra Market only while somebody's clock
    /// reads deep night, and ??? only while three mysterious crates hum in
    /// the hold. Everything else is always there.
    #[must_use]
    pub fn poi_visible(&self, id: PoiId) -> bool {
        match id {
            COMET => map::comet_visible(self.tick),
            UMBRA => self.night,
            WANDERER => self.mysterious_aboard() >= WANDERER_TOLL,
            _ => true,
        }
    }

    /// Mysterious crates stowed in the hold.
    #[must_use]
    pub fn mysterious_aboard(&self) -> u32 {
        self.pieces
            .iter()
            .filter(|piece| {
                matches!(piece.loc, Loc::Hold { .. }) && piece.kind == Kind::MysteriousCrate
            })
            .count() as u32
    }

    /// Whether a transit chit rides in the hold — the inner ring's toll.
    #[must_use]
    pub fn transit_chit_aboard(&self) -> bool {
        self.pieces.iter().any(|piece| {
            matches!(piece.loc, Loc::Hold { .. }) && piece.kind == Kind::TransitChit
        })
    }

    /// Whether charting to `id` is currently refused for want of papers.
    #[must_use]
    pub fn inner_ring_locked(&self, id: PoiId) -> bool {
        INNER_RING.contains(&id) && !self.transit_chit_aboard()
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
        self.omen.swell
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

    /// One frame of one player's pointer handling: presses lift or actuate,
    /// releases drop, and the held piece's legality tracks the pointer.
    fn handle_pointer(&mut self, player: PlayerId, input: &InputFrame, placed: &mut Vec<u32>) {
        if input.press {
            self.on_press(player, input.pointer, input.shift, placed);
        }
        if input.release {
            self.on_release(player, input.pointer, placed);
        }
        let slot = usize::from(player);
        if self.held[slot].is_some() && !input.held && !input.press && !input.release {
            // The release never arrived (window blur, touch cancel, a crew
            // member vanished mid-drag): snap back silently rather than
            // glue a piece to a phantom pointer.
            self.held[slot] = None;
        }
        if let Some(held) = self.held[slot] {
            let legal = self
                .pieces
                .iter()
                .find(|piece| piece.id == held.piece)
                .is_some_and(|piece| self.resolve_drop(piece, input.pointer).is_ok());
            if let Some(held) = &mut self.held[slot] {
                held.legal = legal;
            }
        }
    }

    /// Whether any crew member currently holds `piece`.
    fn held_by_crew(&self, piece: u32) -> bool {
        self.held.iter().flatten().any(|held| held.piece == piece)
    }

    /// A press either chases the rat, lifts a piece, or actuates whatever
    /// it landed on. The rat comes first: a press on its cell shoos it and
    /// does NOT lift the piece under it — piece picks happen only where no
    /// rat sits. Every other press path is unchanged.
    fn on_press(&mut self, player: PlayerId, p: Vec2, shift: bool, placed: &mut Vec<u32>) {
        if self.held(player).is_some() {
            return;
        }
        if self
            .rats
            .on_press(self.seed, self.tick, p, &self.pieces, &mut self.cues)
        {
            return;
        }
        if shift && self.quick_move(p, placed) {
            return;
        }
        let docked = matches!(self.ship.state, ShipState::Docked(_));
        let grabbed = self
            .pieces
            .iter()
            .find(|piece| {
                layout::piece_rect(piece).contains(p)
                    && (docked
                        || matches!(piece.loc, Loc::Hold { .. } | Loc::Flotsam { .. }))
            })
            .map(|piece| (piece.id, piece.loc));
        if let Some((id, origin)) = grabbed {
            if self.held_by_crew(id) {
                // Someone else got there first — this tick (first in
                // player order wins) or an earlier one. Losing a grab race
                // is not an error, so it makes no noise at all.
                return;
            }
            self.held[usize::from(player)] = Some(Held {
                piece: id,
                origin,
                legal: true,
            });
            self.cues.push(Cue::Pickup);
        } else if docked {
            self.on_press_docked(p);
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// A shift-press: move the piece under `p` straight to its one obvious
    /// destination — hold pieces to the give pads, shelf goods to the take
    /// pad, pad pieces back where they came from, received goods and
    /// flotsam into the first legal hold cell (per the QoL brief: the
    /// first legal spot, even if that is a bad idea). Returns whether the
    /// press was consumed; `false` means nothing shift-worthy sat under
    /// the pointer and the press falls through to the ordinary paths.
    fn quick_move(&mut self, p: Vec2, placed: &mut Vec<u32>) -> bool {
        let docked = self.barter.is_some();
        let Some(piece) = self
            .pieces
            .iter()
            .find(|piece| {
                layout::piece_rect(piece).contains(p)
                    && (docked
                        || matches!(piece.loc, Loc::Hold { .. } | Loc::Flotsam { .. }))
            })
            .copied()
        else {
            return false;
        };
        if self.held_by_crew(piece.id) {
            // Someone is dragging it; a modifier press cannot yank it away.
            return true;
        }
        let target = match piece.loc {
            Loc::Hold { .. } if docked => self.free_slot(Loc::GivePad { slot: 0 }, 4),
            Loc::StationShelf { .. } if docked => self.free_slot(Loc::TakePad { slot: 0 }, 4),
            Loc::TakePad { .. } if docked => self.free_slot(Loc::StationShelf { slot: 0 }, 4),
            Loc::GivePad { .. } | Loc::ReceivedShelf { .. } | Loc::Flotsam { .. } => {
                first_fit(&self.pieces, piece.id, piece.kind)
                    .map(|(x, y)| Loc::Hold { x, y })
            }
            _ => None,
        };
        match target {
            Some(loc) => {
                if let Some(stored) = self.pieces.iter_mut().find(|other| other.id == piece.id)
                {
                    stored.loc = loc;
                }
                self.last_violation = None;
                self.refresh_ready();
                placed.push(piece.id);
                self.cues.push(Cue::Place);
            }
            None => self.cues.push(Cue::Reject { hard: false }),
        }
        true
    }

    /// The first free slot in a row of `count` slots shaped like `proto`.
    fn free_slot(&self, proto: Loc, count: u8) -> Option<Loc> {
        (0..count)
            .map(|slot| match proto {
                Loc::GivePad { .. } => Loc::GivePad { slot },
                Loc::TakePad { .. } => Loc::TakePad { slot },
                Loc::StationShelf { .. } => Loc::StationShelf { slot },
                Loc::ReceivedShelf { .. } => Loc::ReceivedShelf { slot },
                Loc::Flotsam { .. } => Loc::Flotsam { slot },
                Loc::Hold { .. } => proto,
            })
            .find(|&loc| self.slot_free(loc, u32::MAX))
    }

    /// Docked-only press targets: POIs, the launch lever, the accept lever.
    fn on_press_docked(&mut self, p: Vec2) {
        let ShipState::Docked(at) = self.ship.state else {
            return;
        };
        for (i, poi) in POIS.iter().enumerate() {
            let id = i as PoiId;
            if id != at && (p - map::poi_pos(id, self.tick)).length() <= poi.radius {
                if !self.poi_visible(id) {
                    // Nothing there right now: the press falls on empty
                    // glass, exactly as if the POI did not exist.
                    continue;
                }
                if self.inner_ring_locked(id) {
                    // The inner ring checks papers at charting time. No
                    // transit chit in the hold, no course.
                    self.cues.push(Cue::Reject { hard: false });
                    return;
                }
                self.ship.selected = Some(id);
                self.cues.push(Cue::Select);
                return;
            }
        }
        if layout::LAUNCH_LEVER.contains(p) {
            let armed_valid = self.ship.selected.is_some_and(|to| {
                // Conditions can lapse between selection and the lever: the
                // comet sets, dawn closes the Umbra Market, the chit gets
                // gifted away. Re-check, and disarm a stale selection.
                self.poi_visible(to) && !self.inner_ring_locked(to)
            });
            if self.ship.selected.is_some() && !armed_valid {
                self.ship.selected = None;
            }
            if armed_valid && !self.pads_occupied() {
                self.depart();
            } else {
                // No destination, or pieces on a pad or the received shelf:
                // launching would strand them, so nothing is ever lost to
                // the lever.
                self.cues.push(Cue::Reject { hard: false });
            }
        } else if layout::ACCEPT_LEVER.contains(p) {
            if self.barter.is_some() {
                self.conclude();
            } else {
                // A comet or ??? has no counterparty; the lever is dead.
                self.cues.push(Cue::Reject { hard: false });
            }
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// A release drops the held piece: place it if the target is legal,
    /// snap it back otherwise. A drop never destroys or surrenders a piece
    /// — ownership changes only through the accept lever.
    fn on_release(&mut self, player: PlayerId, p: Vec2, placed: &mut Vec<u32>) {
        let Some(held) = self.held[usize::from(player)].take() else {
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
                placed.push(piece.id);
                self.cues.push(Cue::Place);
            }
            Err(violation) => {
                // Same-tick drop contention: refused only because an
                // earlier player's piece just landed there means this
                // player lost a race, not broke a stowage rule — soft
                // snap-back, no violation flash.
                let contested = violation == Some(Violation::Overlap)
                    && !placed.is_empty()
                    && self.contested_only(&piece, p, placed);
                let hard = violation.is_some() && !contested;
                if hard {
                    self.last_violation = violation;
                }
                self.cues.push(Cue::Reject { hard });
            }
        }
    }

    /// Whether dropping `piece` at `p` would have been legal without the
    /// pieces placed earlier this round — i.e. the refusal is pure
    /// same-tick contention. Slot drops need no such check: an occupied
    /// slot is already a soft reject.
    fn contested_only(&self, piece: &Piece, p: Vec2, placed: &[u32]) -> bool {
        let Some((x, y)) = layout::cell_at(p) else {
            return false;
        };
        let rest: Vec<Piece> = self
            .pieces
            .iter()
            .filter(|other| !placed.contains(&other.id))
            .copied()
            .collect();
        placement_check(&rest, piece.id, piece.kind, x, y).is_ok()
    }

    /// Where dropping `piece` at `p` would settle it, or which flavour of
    /// rejection it earns. `Err(Some(_))` is the hard reject — a stowage
    /// rule refused an in-grid drop — and names the rule; `Err(None)` is a
    /// soft, ignorable miss that snaps the piece home. Every arm gates on
    /// [`player_owned`], the same predicate [`Sim::drop_targets`] advertises
    /// from, so the glowing regions and the legal ones cannot drift apart.
    fn resolve_drop(&self, piece: &Piece, p: Vec2) -> Result<Loc, Option<Violation>> {
        let ours = player_owned(piece.loc);
        let flotsam = matches!(piece.loc, Loc::Flotsam { .. });
        if let Some((x, y)) = layout::cell_at(p) {
            if !ours && !flotsam {
                // Station goods never enter the hold before a trade.
                // Flotsam is nobody's: stowing it is how it becomes yours.
                return Err(None);
            }
            return match placement_check(&self.pieces, piece.id, piece.kind, x, y) {
                Ok(()) => Ok(Loc::Hold { x, y }),
                Err(violation) => Err(Some(violation)),
            };
        }
        if flotsam {
            // Not stowed: it can only go back to a drift slot.
            if let Some(slot) = layout::slot_at2(&layout::FLOTSAM_SLOTS, p) {
                let loc = Loc::Flotsam { slot };
                return self.slot_free(loc, piece.id).then_some(loc).ok_or(None);
            }
            return Err(None);
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

    /// Which regions would accept `player`'s held piece, for the renderer
    /// to glow. `None` while that player holds nothing. Derived from
    /// [`player_owned`] and the dock state exactly as [`Sim::resolve_drop`]
    /// is; per-slot freeness stays with the drop itself (a glowing row with
    /// one occupied socket is still an honest invitation).
    #[must_use]
    pub fn drop_targets(&self, player: PlayerId) -> Option<DropTargets> {
        let held = self.held(player)?;
        let piece = self.pieces.iter().find(|piece| piece.id == held.piece)?;
        let ours = player_owned(piece.loc);
        let docked = self.barter.is_some();
        Some(DropTargets {
            hold: ours || matches!(piece.loc, Loc::Flotsam { .. }),
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
        if let Some(barter) = &mut self.barter {
            barter.ready = barter::eagerness_of(
                &self.pieces,
                &self.values,
                barter::gnaw_loved(barter.station),
            )
            .1;
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
        let Some(station) = self.barter.as_ref().map(|barter| barter.station) else {
            return;
        };
        let gnaw_love = barter::gnaw_loved(station);
        let (_, ready) = barter::eagerness_of(&self.pieces, &self.values, gnaw_love);
        if !ready || self.received_occupied() {
            self.cues.push(Cue::Refuse);
            return;
        }
        let value = barter::deal_value(&self.pieces, &self.values, gnaw_love);
        if station == HERMITAGE {
            // The hermits remember every piece given, forever.
            self.karma += self
                .pieces
                .iter()
                .filter(|piece| matches!(piece.loc, Loc::GivePad { .. }))
                .count() as u32;
        }
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
        let leg_ticks = map::leg_ticks(from, to, self.tick);
        self.pieces
            .retain(|piece| matches!(piece.loc, Loc::Hold { .. }));
        self.barter = None;
        self.legs += 1;
        let suspicious = self.suspicious_aboard();
        self.omen
            .on_depart(self.seed, self.legs, leg_ticks, suspicious);
        self.ship.state = ShipState::Traveling {
            from,
            to,
            progress: 0,
            leg_ticks,
        };
        self.cues.push(Cue::Depart);
        // After the departure clunk: the stowaway slips in with the cargo.
        self.rats.on_depart(
            self.seed,
            self.legs,
            self.tick,
            &self.pieces,
            suspicious,
            &mut self.cues,
        );
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
            self.omen
                .travel_tick(&mut progress, leg_ticks, &mut self.cues);
            if let Some(cue) = event::creak(self.seed, self.tick) {
                self.cues.push(cue);
            }
            if progress >= leg_ticks {
                self.dock(to);
            } else {
                self.ship.pos = map::travel_pos(from, to, progress, leg_ticks, self.tick);
                self.ship.state = ShipState::Traveling {
                    from,
                    to,
                    progress,
                    leg_ticks,
                };
            }
        } else if let ShipState::Docked(at) = self.ship.state {
            // Docked means moored: the ship rides its planet's orbit.
            self.ship.pos = map::poi_pos(at, self.tick);
        }

        self.omen.on_tick();
        self.rats
            .on_tick(self.seed, self.tick, &mut self.pieces, &mut self.cues);

        if let Some(barter) = &mut self.barter {
            // The dial eases toward the trade's true ratio (capped at the
            // dial's peg); readiness tracks the ratio itself.
            barter.prev_eagerness = barter.eagerness;
            let (target, ready) = barter::eagerness_of(
                &self.pieces,
                &self.values,
                barter::gnaw_loved(barter.station),
            );
            barter.ready = ready;
            barter.eagerness = step_toward(
                barter.eagerness,
                target.clamp(0.0, EAGER_MAX),
                barter::EAGER_RATE * TICK_DT,
            );
        }
    }

    /// Arrive: snap to the pad, count the visit, and lay out its trade. At
    /// the Guild the hangar steal runs first, so the visit's barter never
    /// sees the crate.
    fn dock(&mut self, poi: PoiId) {
        self.ship.pos = map::poi_pos(poi, self.tick);
        self.ship.state = ShipState::Docked(poi);
        self.ship.selected = None;
        // Arriving anywhere drops out of warp: a developer fast-forwarding
        // should be looking when something happens.
        self.disengage_warp();
        self.omen.on_dock(&mut self.cues);
        if poi == GUILD {
            self.steal_crate();
        }
        // After any hangar steal, so the walk-off gate reads the hold as
        // the dock leaves it.
        self.rats.on_dock(&self.pieces, &mut self.cues);
        self.visits[usize::from(poi)] += 1;
        let visit = self.visits[usize::from(poi)];
        match poi {
            COMET => self.harvest_comet(visit),
            WANDERER => self.wanderer_exchange(),
            _ => {
                let (barter, goods) = barter::generate(
                    self.seed,
                    poi,
                    visit,
                    &self.pieces,
                    self.karma,
                    &mut self.rng,
                    &mut self.next_piece,
                );
                self.values = barter::visit_values(self.seed, poi, visit);
                self.pieces.extend(goods);
                self.barter = Some(barter);
            }
        }
        self.cues.push(Cue::Arrive);
    }

    /// Turn warp off if it is on, with its cue. Arrivals and event
    /// openings call this: fast-forward is a dev tool, and it must not
    /// carry anyone obliviously past the interesting parts.
    fn disengage_warp(&mut self) {
        if self.warp {
            self.warp = false;
            self.cues.push(Cue::Warp { engaged: false });
        }
    }

    /// Docked at the comet: chip off some ice, free. One to three shards
    /// (as the hold's edges allow — ice hugs the hull like any cryo cargo)
    /// plus, one visit in three, something odd frozen inside.
    fn harvest_comet(&mut self, visit: u32) {
        let h = splitmix(self.seed ^ SALT_HARVEST, u64::from(visit));
        let shards = 1 + h % 3;
        let mut placed = 0_u32;
        for _ in 0..shards {
            if !self.spawn_in_hold(Kind::CometIce) {
                break;
            }
            placed += 1;
        }
        if (h >> 32) % 3 == 0 && self.spawn_in_hold(Kind::MysteriousCrate) {
            placed += 1;
        }
        if placed > 0 {
            self.cues.push(Cue::Harvest {
                intensity: placed as f32 / 4.0,
            });
        }
    }

    /// Docked at ???: three mysterious crates become one very mysterious
    /// crate, no barter, no explanation. If the bigger crate cannot sit
    /// anywhere legally (a suspicious crate already hums in the hold, or
    /// no 2x2 gap survives), nothing is taken — the exchange never
    /// half-completes, which is the anti-softlock guarantee.
    fn wanderer_exchange(&mut self) {
        if self.mysterious_aboard() < WANDERER_TOLL {
            return;
        }
        // Candidate board: the three lowest-id mysterious crates removed.
        let mut doomed: Vec<u32> = self
            .pieces
            .iter()
            .filter(|piece| {
                matches!(piece.loc, Loc::Hold { .. }) && piece.kind == Kind::MysteriousCrate
            })
            .map(|piece| piece.id)
            .collect();
        doomed.sort_unstable();
        doomed.truncate(WANDERER_TOLL as usize);
        let remainder: Vec<Piece> = self
            .pieces
            .iter()
            .filter(|piece| !doomed.contains(&piece.id))
            .copied()
            .collect();
        let Some((x, y)) =
            first_fit(&remainder, self.next_piece, Kind::VeryMysteriousCrate)
        else {
            self.cues.push(Cue::Reject { hard: false });
            return;
        };
        for held in &mut self.held {
            if matches!(held, Some(h) if doomed.contains(&h.piece)) {
                *held = None;
            }
        }
        self.pieces = remainder;
        self.pieces.push(Piece {
            id: self.next_piece,
            kind: Kind::VeryMysteriousCrate,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc: Loc::Hold { x, y },
        });
        self.next_piece += 1;
        self.cues.push(Cue::Exchange);
    }

    /// Stow a fresh `kind` at the first legal cell, if any. Free cargo
    /// only: barter pieces arrive through [`barter::generate`].
    fn spawn_in_hold(&mut self, kind: Kind) -> bool {
        let Some((x, y)) = first_fit(&self.pieces, self.next_piece, kind) else {
            return false;
        };
        self.pieces.push(Piece {
            id: self.next_piece,
            kind,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc: Loc::Hold { x, y },
        });
        self.next_piece += 1;
        true
    }

    /// The hangar steal: any suspicious crate aboard is seized the moment
    /// the ship docks at the Guild — in front of the usual bartering — and
    /// counted on the delivery tally with a [`Cue::Delivered`]. The
    /// singleton rule caps this at one crate per docking, and a crate held
    /// mid-drag drops first: it is dock time.
    fn steal_crate(&mut self) {
        let Some(index) = self
            .pieces
            .iter()
            .position(|piece| matches!(piece.kind.tag(), Some(Tag::Suspicious)))
        else {
            return;
        };
        let id = self.pieces[index].id;
        let worth = if self.pieces[index].kind == Kind::VeryMysteriousCrate {
            // Whatever it is, the hangar counts it four times. Nobody
            // explains the arithmetic.
            4
        } else {
            1
        };
        for held in &mut self.held {
            if matches!(held, Some(held) if held.piece == id) {
                *held = None;
            }
        }
        self.pieces.remove(index);
        self.deliveries += worth;
        self.cues.push(Cue::Delivered);
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

    /// Uranus, an outer-ring test destination (the inner ring needs a
    /// transit chit, so generic travel tests chart outward).
    const URANUS: PoiId = 4;

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
        assert!(sim.held(0).is_some(), "nothing to lift at {from:?}");
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
            gnawed: false,
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
            gnawed: false,
            loc,
        });
        id
    }

    /// Test scaffolding: a rat mid-tenure, schedules wound close so the
    /// monkeys meet skitters and nibbles quickly.
    const fn inject_rat(sim: &mut Sim) {
        sim.rats.rat = Some(Rat {
            cell: (3, 1),
            prev_cell: (3, 1),
            moved_at: 0,
            next_move: 60,
            next_nibble: 120,
            chases: 0,
        });
    }

    /// Whether a stowed piece's footprint covers `cell`.
    fn cell_covered(sim: &Sim, cell: (u8, u8)) -> bool {
        sim.pieces().iter().any(|p| {
            let Loc::Hold { x, y } = p.loc else {
                return false;
            };
            let (w, h) = p.kind.cells();
            cell.0 >= x && cell.0 < x + w && cell.1 >= y && cell.1 < y + h
        })
    }

    /// Select `poi` and pull the launch lever; zero-dt frames run no ticks,
    /// so the sim is exactly at departure.
    fn launched_toward(seed: u64, poi: PoiId) -> Sim {
        let mut sim = Sim::new(seed);
        launch(&mut sim, poi);
        sim
    }

    /// Select `poi` on an already-docked sim and pull the lever. The depart
    /// frame may also carry a `RatAboard` on a crowded hold, so only the
    /// departure itself is asserted exactly.
    fn launch(sim: &mut Sim, poi: PoiId) {
        let target = sim.poi_pos(poi);
        sim.advance(0.0, &press_at(target.x, target.y));
        assert_eq!(sim.cues(), [Cue::Select]);
        assert_eq!(sim.ship().selected, Some(poi));
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues().first(), Some(&Cue::Depart));
        assert!(matches!(sim.ship().state, ShipState::Traveling { .. }));
    }

    fn launched(seed: u64) -> Sim {
        launched_toward(seed, SATURN)
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
        // Only the 30 dial frames above advance the clock, so the press
        // lands at tick 30 exactly.
        let mars = map::poi_pos(URANUS, 30);
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
        // Warp sixteen-fold through most of the leg — its length depends on
        // where the sky stands at the tick-30 departure — then coast in.
        let leg = map::leg_ticks(GUILD, URANUS, 30);
        let warp_frames = (leg / u64::from(WARP_FACTOR as u32)).saturating_sub(20);
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        s.push((0.0, warp));
        for _ in 0..warp_frames {
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
        assert_eq!(sim.ship().state, ShipState::Docked(SATURN));
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
        assert!(sim.held(0).is_some());

        // Drop onto the scrap at (0, 2): overlap, a hard reject.
        let scrap = rect_center(layout::cell_rect(0, 2));
        sim.advance(0.0, &release_at(scrap.x, scrap.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert!(sim.held(0).is_none());

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
        // STV1 predates the delivery tally: fail safe into a fresh game.
        assert_eq!(
            Sim::from_save("STV1\nseed 1").unwrap_err(),
            SaveError::UnsupportedVersion
        );
        // STV2 predates the rat and the gnaw token: same fail-safe refusal.
        assert_eq!(
            Sim::from_save("STV2\nseed 1").unwrap_err(),
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
        assert_eq!(a.ship().state, ShipState::Docked(URANUS));
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
        launch(&mut sim, SATURN);
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
        launch(&mut sim, SATURN);
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
        let held = sim.held(0).expect("lifted the vial").piece;
        let saved = sim.save_string();
        let restored = Sim::from_save(&saved).expect("mid-drag save parses");
        assert!(restored.held(0).is_none(), "held state is transient");
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
        launch(&mut base, SATURN);
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
        assert_eq!(sim.drop_targets(0), None, "nothing held, nothing invited");
        // Hold a player piece: hold and give invite; station rows never do.
        let vial = cell_center(0, 0);
        sim.advance(0.0, &press_at(vial.x, vial.y));
        assert_eq!(
            sim.drop_targets(0),
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
            sim.drop_targets(0),
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
        // The second event runs live under the monkey: skitters, nibbles,
        // and chases must leave every invariant standing.
        inject_rat(&mut sim);
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
                // The quick-move path runs under the monkey too: shift
                // presses must obey the same conservation rules as drags.
                shift: rng.bool(),
                night: rng.bool(),
                reseed: None,
            };
            sim.advance(TICK_DT, &input);
            // The two legitimate exits: the accept lever, and the Guild's
            // hangar steal.
            let ceded = sim
                .cues()
                .iter()
                .any(|cue| {
                    matches!(cue, Cue::Accept { .. } | Cue::Delivered | Cue::Exchange)
                });
            let after = owned(&sim);
            assert!(
                after >= before || ceded,
                "frame {frame}: {before} -> {after} player pieces with no accept"
            );
            // Held pieces must stay real: a lifted id always exists.
            if let Some(held) = sim.held(0) {
                assert!(
                    sim.pieces().iter().any(|p| p.id == held.piece),
                    "frame {frame}: held piece {} is a ghost",
                    held.piece
                );
            }
            // The rat, while it lasts, stays on the grid.
            if let Some(rat) = sim.rat() {
                assert!(
                    rat.cell.0 < layout::GRID_COLS && rat.cell.1 < layout::GRID_ROWS,
                    "frame {frame}: rat off the grid at {:?}",
                    rat.cell
                );
            }
            before = after;
        }
    }

    #[test]
    fn launch_gate_refuses_while_pads_hold_pieces() {
        let mut sim = Sim::new(3);
        let venus = sim.poi_pos(SATURN);
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

    /// Crates of any location, aboard or shelved.
    fn crates(sim: &Sim) -> usize {
        sim.pieces()
            .iter()
            .filter(|p| p.kind == Kind::SuspiciousCrate)
            .count()
    }

    #[test]
    fn each_guild_docking_steals_the_crate_and_counts_it() {
        let mut sim = Sim::new(0x0051_C0DE);
        for round in 1..=3_u32 {
            inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
            travel_to(&mut sim, SATURN);
            // Venus neither steals the crate nor offers a second: the
            // singleton aboard suppresses the shelf offer.
            assert_eq!(crates(&sim), 1, "round {round}: the singleton broke");
            assert_eq!(sim.deliveries(), round - 1);
            travel_to(&mut sim, GUILD);
            assert_eq!(crates(&sim), 0, "round {round}: the crate survived");
            assert_eq!(sim.deliveries(), round, "round {round}: not counted");
            // The steal ran in front of the barter: the visit is laid out
            // as usual, one piece per shelf slot.
            let barter = sim.barter().expect("docked means barter");
            assert_eq!(barter.station, GUILD);
            let shelved: Vec<u8> = sim
                .pieces()
                .iter()
                .filter_map(|p| match p.loc {
                    Loc::StationShelf { slot } => Some(slot),
                    _ => None,
                })
                .collect();
            assert!(!shelved.is_empty(), "the steal ate the shelf");
            let mut deduped = shelved.clone();
            deduped.sort_unstable();
            deduped.dedup();
            assert_eq!(deduped.len(), shelved.len(), "shelf slot conflict");
        }
    }

    #[test]
    fn guild_steal_fires_delivered_beside_arrive() {
        let mut sim = Sim::new(0xDE11);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        travel_to(&mut sim, SATURN);
        launch(&mut sim, GUILD);
        // Walk to the dock live, so the arrival frame's cues are visible.
        let mut arrival = Vec::new();
        for _ in 0..=leg_of(&sim) {
            sim.advance(TICK_DT, &InputFrame::default());
            if sim.cues().contains(&Cue::Arrive) {
                arrival = sim.cues().to_vec();
                break;
            }
        }
        let delivered = arrival
            .iter()
            .position(|c| matches!(c, Cue::Delivered))
            .expect("no Delivered on the arrival frame");
        let arrive = arrival
            .iter()
            .position(|c| matches!(c, Cue::Arrive))
            .expect("no Arrive on the arrival frame");
        assert!(delivered < arrive, "the steal happens in front of the dock");
        assert_eq!(sim.deliveries(), 1);
        assert_eq!(crates(&sim), 0);
        assert!(sim.barter().is_some(), "the barter must survive the steal");
    }

    #[test]
    fn docking_without_a_crate_never_delivers() {
        let mut sim = Sim::new(0x00DE);
        travel_to(&mut sim, SATURN);
        travel_to(&mut sim, GUILD);
        assert_eq!(sim.deliveries(), 0);
        let mut sim = Sim::new(0x00DE);
        travel_to(&mut sim, SATURN);
        launch(&mut sim, GUILD);
        let mut log = Vec::new();
        for _ in 0..=leg_of(&sim) {
            sim.advance(TICK_DT, &InputFrame::default());
            log.extend_from_slice(sim.cues());
        }
        assert_eq!(count_cues(&log, |c| matches!(c, Cue::Arrive)), 1);
        assert_eq!(count_cues(&log, |c| matches!(c, Cue::Delivered)), 0);
        assert_eq!(sim.deliveries(), 0);
    }

    #[test]
    fn a_crate_held_mid_drag_is_still_stolen_at_the_dock() {
        let mut sim = Sim::new(0xD00D);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        travel_to(&mut sim, SATURN);
        launch(&mut sim, GUILD);
        // Lift the crate mid-flight and keep dragging through the dock.
        let (crate_id, at) = {
            let piece = sim
                .pieces()
                .iter()
                .find(|p| p.kind == Kind::SuspiciousCrate)
                .expect("the crate rides along");
            (piece.id, rect_center(layout::piece_rect(piece)))
        };
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.held(0).map(|h| h.piece), Some(crate_id));
        while matches!(sim.ship().state, ShipState::Traveling { .. }) {
            sim.advance(TICK_DT, &held_at(at.x, at.y));
        }
        // Dock time: the drag dropped, the crate is gone, the count is in.
        assert_eq!(sim.held(0), None, "the steal must clear the drag");
        assert_eq!(crates(&sim), 0);
        assert_eq!(sim.deliveries(), 1);
    }

    #[test]
    fn deliveries_survive_the_save_round_trip() {
        let mut sim = Sim::new(0x5AFE);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        travel_to(&mut sim, SATURN);
        travel_to(&mut sim, GUILD);
        assert_eq!(sim.deliveries(), 1);
        let restored = Sim::from_save(&sim.save_string()).expect("own save must parse");
        assert_eq!(restored.deliveries(), 1);
        assert_save_continues(sim, 5_000);
    }

    #[test]
    fn omen_fires_exactly_once_at_the_derived_tick() {
        let seed = 0xE7E7;
        let mut sim = Sim::new(seed);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 3, 0);
        launch(&mut sim, SATURN);
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

    // -------------------------------------------------------------- crew --

    /// A sealed tick's frames with the given players' inputs set; everyone
    /// else is default, exactly as absent crew are in lockstep.
    fn crew(entries: &[(PlayerId, InputFrame)]) -> CrewFrame {
        let mut frame = [InputFrame::default(); MAX_CREW];
        for &(player, input) in entries {
            frame[usize::from(player)] = input;
        }
        frame
    }

    /// A crew schedule with some of everything: three players dragging
    /// different pieces in parallel (a gift, a restow, a snap-back-to-self),
    /// warp toggled on by one player and off by another, a destination
    /// selected, the gift accepted, a departure, and the Venus leg coasted
    /// onto the dock.
    fn crew_schedule() -> Vec<CrewFrame> {
        let vial = cell_center(0, 0);
        let scrap = cell_center(0, 2);
        let pearls = cell_center(2, 0);
        let give0 = slot_center(&layout::GIVE_SLOTS, 0);
        let restow = cell_center(4, 2);
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        // The Venus press lands on sealed tick 3 and the departure on
        // sealed tick 5; the sky is sampled for those exact moments.
        let venus = map::poi_pos(SATURN, 3);
        let launch_lever = rect_center(layout::LAUNCH_LEVER);
        let accept = rect_center(layout::ACCEPT_LEVER);
        let mut s = vec![
            crew(&[
                (1, press_at(vial.x, vial.y)),
                (2, press_at(scrap.x, scrap.y)),
                (4, warp),
            ]),
            crew(&[
                (1, held_at(give0.x, give0.y)),
                (2, held_at(restow.x, restow.y)),
                (3, press_at(pearls.x, pearls.y)),
            ]),
            crew(&[
                (1, release_at(give0.x, give0.y)),
                (2, release_at(restow.x, restow.y)),
                (3, release_at(pearls.x, pearls.y)),
            ]),
            crew(&[(0, press_at(venus.x, venus.y)), (5, warp)]),
            crew(&[(0, press_at(accept.x, accept.y))]),
            crew(&[(0, press_at(launch_lever.x, launch_lever.y))]),
        ];
        for _ in 0..=map::leg_ticks(GUILD, SATURN, 5) + 1 {
            s.push(crew(&[]));
        }
        s
    }

    #[test]
    fn crew_schedules_replay_bit_identically() {
        let schedule = crew_schedule();
        let mut a = Sim::new(0xCE11);
        let mut b = Sim::new(0xCE11);
        let mut log_a = Vec::new();
        let mut log_b = Vec::new();
        for frame in &schedule {
            a.crew_tick(frame);
            log_a.extend_from_slice(a.cues());
        }
        for frame in &schedule {
            b.crew_tick(frame);
            log_b.extend_from_slice(b.cues());
        }
        assert_eq!(log_a, log_b, "cue streams diverged");
        // The choreography really happened, once each.
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Pickup)), 3);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Place)), 3);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Warp { .. })), 2);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Select)), 1);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Accept { .. })), 1);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Depart)), 1);
        assert_eq!(count_cues(&log_a, |c| matches!(c, Cue::Arrive)), 1);
        assert_eq!(a.ship().state, ShipState::Docked(SATURN));
        assert_eq!(a.save_string(), b.save_string());
        assert_eq!(a.pieces(), b.pieces());
        assert_eq!(a.barter(), b.barter());
        assert_eq!(a.ship(), b.ship());
        assert_eq!(a.tick(), b.tick());
    }

    /// One frame per tick with only player 0 active: select Venus, launch,
    /// pause and resume mid-leg, coast onto the dock, gift the vial, accept.
    /// No warp toggles: warp multiplies ticks-per-frame in [`Sim::advance`],
    /// while a lockstep session realises it by sealing ticks faster.
    fn solo_schedule() -> Vec<InputFrame> {
        let venus = map::poi_pos(SATURN, 0);
        let launch_lever = rect_center(layout::LAUNCH_LEVER);
        let accept = rect_center(layout::ACCEPT_LEVER);
        let pause = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        let vial = cell_center(0, 0);
        let give0 = slot_center(&layout::GIVE_SLOTS, 0);
        let mut s = vec![
            press_at(venus.x, venus.y),
            press_at(launch_lever.x, launch_lever.y),
        ];
        for _ in 0..500 {
            s.push(InputFrame::default());
        }
        s.push(pause);
        for _ in 0..3 {
            s.push(InputFrame::default());
        }
        s.push(pause);
        for _ in 0..=map::leg_ticks(GUILD, SATURN, 1) + 1 {
            s.push(InputFrame::default());
        }
        s.push(press_at(vial.x, vial.y));
        s.push(held_at(give0.x, give0.y));
        s.push(release_at(give0.x, give0.y));
        for _ in 0..30 {
            s.push(InputFrame::default());
        }
        s.push(press_at(accept.x, accept.y));
        for _ in 0..30 {
            s.push(InputFrame::default());
        }
        s
    }

    /// The equivalence the whole lockstep slice rests on: a solo crew run
    /// through [`Sim::crew_tick`] is bit-identical, cues included, to the
    /// same inputs through [`Sim::advance`] one tick at a time.
    #[test]
    fn crew_tick_equals_advance_for_a_solo_crew() {
        let script = solo_schedule();
        let mut via_crew = Sim::new(0x5010);
        let mut via_advance = Sim::new(0x5010);
        for (i, input) in script.iter().enumerate() {
            via_crew.crew_tick(&crew(&[(0, *input)]));
            let ticks = via_advance.advance(TICK_DT, input);
            assert!(ticks <= 1, "solo equivalence needs one tick per frame");
            assert_eq!(
                via_crew.cues(),
                via_advance.cues(),
                "cues diverged at frame {i}"
            );
            if i % 512 == 0 {
                assert_eq!(
                    via_crew.save_string(),
                    via_advance.save_string(),
                    "state diverged by frame {i}"
                );
            }
        }
        assert_eq!(via_advance.ship().state, ShipState::Docked(SATURN));
        assert_eq!(via_crew.save_string(), via_advance.save_string());
        assert_eq!(via_crew.pieces(), via_advance.pieces());
        assert_eq!(via_crew.barter(), via_advance.barter());
        assert_eq!(via_crew.ship(), via_advance.ship());
        assert_eq!(via_crew.tick(), via_advance.tick());
        assert_eq!(via_crew.held(0), via_advance.held(0));
    }

    #[test]
    fn same_tick_grab_goes_to_the_lowest_player_in_silence() {
        let mut sim = Sim::new(5);
        let vial = cell_center(0, 0);
        let press = press_at(vial.x, vial.y);
        sim.crew_tick(&crew(&[(1, press), (4, press)]));
        let piece = sim.held(1).expect("player 1 wins the grab").piece;
        assert_eq!(sim.held(4), None, "player 4 must lose the grab");
        assert_eq!(
            sim.cues(),
            [Cue::Pickup],
            "one lift; the loser makes no noise"
        );
        // Across ticks the piece stays claimed, in the same silence.
        sim.crew_tick(&crew(&[(1, held_at(vial.x, vial.y)), (4, press)]));
        assert_eq!(sim.held(4), None, "a held piece is not grabbable");
        assert!(sim.cues().is_empty(), "a losing grab is silence, not buzz");
        let all: Vec<(PlayerId, u32)> = sim.all_held().map(|(p, h)| (p, h.piece)).collect();
        assert_eq!(all, [(1, piece)]);
    }

    #[test]
    fn same_tick_release_contention_first_wins_second_snaps_back() {
        let mut sim = Sim::new(5);
        let vial = cell_center(0, 0);
        let pearls = cell_center(2, 0);
        sim.crew_tick(&crew(&[
            (0, press_at(vial.x, vial.y)),
            (1, press_at(pearls.x, pearls.y)),
        ]));
        assert_eq!(sim.cues(), [Cue::Pickup, Cue::Pickup]);
        // Both release onto hold cell (5, 2): the earlier player lands it;
        // the later snaps home with a soft reject — losing a race is not a
        // stowage violation, so no rule icon flashes either.
        let target = cell_center(5, 2);
        sim.crew_tick(&crew(&[
            (0, release_at(target.x, target.y)),
            (1, release_at(target.x, target.y)),
        ]));
        assert_eq!(sim.cues(), [Cue::Place, Cue::Reject { hard: false }]);
        assert_eq!(sim.last_violation(), None);
        let loc = |id: u32| sim.pieces().iter().find(|p| p.id == id).unwrap().loc;
        assert_eq!(loc(1), Loc::Hold { x: 5, y: 2 }, "the winner lands");
        assert_eq!(loc(2), Loc::Hold { x: 2, y: 0 }, "the loser snaps home");

        // The same race over a barter slot: first fills it, second bounces.
        let shelf: Vec<Vec2> = sim
            .pieces()
            .iter()
            .filter(|p| matches!(p.loc, Loc::StationShelf { .. }))
            .map(|p| rect_center(layout::piece_rect(p)))
            .collect();
        assert!(shelf.len() >= 2, "a shelf always offers at least two");
        sim.crew_tick(&crew(&[
            (2, press_at(shelf[0].x, shelf[0].y)),
            (3, press_at(shelf[1].x, shelf[1].y)),
        ]));
        let take0 = slot_center(&layout::TAKE_SLOTS, 0);
        sim.crew_tick(&crew(&[
            (2, release_at(take0.x, take0.y)),
            (3, release_at(take0.x, take0.y)),
        ]));
        assert_eq!(sim.cues(), [Cue::Place, Cue::Reject { hard: false }]);
        let on_take = sim
            .pieces()
            .iter()
            .filter(|p| matches!(p.loc, Loc::TakePad { .. }))
            .count();
        assert_eq!(on_take, 1, "one slot takes one piece");
    }

    #[test]
    fn same_tick_pause_toggles_net_out_and_announce_in_order() {
        let mut sim = Sim::new(7);
        let toggle = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        sim.crew_tick(&crew(&[(0, toggle), (3, toggle)]));
        assert!(!sim.is_paused(), "two toggles must cancel");
        assert_eq!(
            sim.cues(),
            [Cue::Pause { paused: true }, Cue::Pause { paused: false }]
        );
        assert_eq!(sim.tick(), 1, "a cancelled pause still ticks");
        // An odd toggle count lands paused, and later players' pointer
        // events are dropped from the moment the flag flips.
        let vial = cell_center(0, 0);
        sim.crew_tick(&crew(&[(1, toggle), (2, press_at(vial.x, vial.y))]));
        assert!(sim.is_paused());
        assert_eq!(sim.cues(), [Cue::Pause { paused: true }]);
        assert_eq!(sim.held(2), None, "pointer input under pause is dropped");
        assert_eq!(sim.tick(), 1, "a paused tick must not step");
        // The mirror case: a press lands while still paused even though a
        // later player unpauses the same tick.
        sim.crew_tick(&crew(&[(1, press_at(vial.x, vial.y)), (5, toggle)]));
        assert!(!sim.is_paused());
        assert_eq!(sim.cues(), [Cue::Pause { paused: false }]);
        assert_eq!(sim.held(1), None, "the press came while still paused");
        assert_eq!(sim.tick(), 2, "the unpaused tick steps");
    }

    #[test]
    fn crew_reseed_last_in_order_wins() {
        let mut sim = Sim::new(1);
        let reseed = |seed: u64| InputFrame {
            reseed: Some(seed),
            ..InputFrame::default()
        };
        sim.crew_tick(&crew(&[(2, reseed(0xAAA)), (4, reseed(0xBBB))]));
        assert_eq!(sim.seed(), 0xBBB, "the last reseed in order wins");
        assert_eq!(sim.cues(), [Cue::Reseed], "one replacement announced");
        assert_eq!(sim.tick(), 1);
    }

    /// The conservation monkey, crewed: six chaotic players per tick
    /// through [`Sim::crew_tick`], with a rat aboard from the first frame.
    /// The solo monkey's guarantee holds under contention, plus the crew
    /// invariants — no held id is a ghost, no piece is held twice, no two
    /// pieces share a surface spot.
    #[test]
    fn no_crew_input_stream_loses_cargo_without_an_accept() {
        let mut sim = Sim::new(0xC0FF_EE01);
        inject_rat(&mut sim);
        let mut rng = fastrand::Rng::with_seed(0xF00D);
        let owned = |sim: &Sim| sim.pieces().iter().filter(|p| player_owned(p.loc)).count();
        let mut before = owned(&sim);
        for tick in 0_u32..4000 {
            let mut frames = [InputFrame::default(); MAX_CREW];
            for frame in &mut frames {
                *frame = InputFrame {
                    pointer: Vec2::new(rng.f32() * WORLD_W, rng.f32() * WORLD_H),
                    press: rng.bool(),
                    held: rng.bool(),
                    release: rng.bool(),
                    toggle_pause: rng.u8(..) < 2,
                    toggle_warp: rng.u8(..) < 2,
                    shift: rng.bool(),
                    night: rng.bool(),
                    reseed: None,
                };
            }
            sim.crew_tick(&frames);
            let ceded = sim
                .cues()
                .iter()
                .any(|cue| {
                    matches!(cue, Cue::Accept { .. } | Cue::Delivered | Cue::Exchange)
                });
            let after = owned(&sim);
            assert!(
                after >= before || ceded,
                "tick {tick}: {before} -> {after} player pieces with no accept"
            );
            let mut held_ids: Vec<u32> = sim.all_held().map(|(_, held)| held.piece).collect();
            for &id in &held_ids {
                assert!(
                    sim.pieces().iter().any(|p| p.id == id),
                    "tick {tick}: held piece {id} is a ghost"
                );
            }
            held_ids.sort_unstable();
            held_ids.dedup();
            assert_eq!(
                held_ids.len(),
                sim.all_held().count(),
                "tick {tick}: two players hold one piece"
            );
            let pieces = sim.pieces();
            for (i, a) in pieces.iter().enumerate() {
                for b in &pieces[i + 1..] {
                    assert!(
                        a.loc != b.loc,
                        "tick {tick}: pieces {} and {} share {:?}",
                        a.id,
                        b.id,
                        a.loc
                    );
                }
            }
            if let Some(rat) = sim.rat() {
                assert!(
                    rat.cell.0 < layout::GRID_COLS && rat.cell.1 < layout::GRID_ROWS,
                    "tick {tick}: rat off the grid at {:?}",
                    rat.cell
                );
            }
            before = after;
        }
    }

    // -------------------------------------------------------------- rats --

    /// Stow enough extra cargo that the hold crosses the boarding gate:
    /// starter cargo's 5 cells plus two ration bricks is 13 of 24.
    fn crowd_hold(sim: &mut Sim) {
        inject_hold(sim, Kind::RationBricks, 2, 2);
        inject_hold(sim, Kind::RationBricks, 4, 2);
    }

    /// Fill the hold to all 24 cells, so a boarding rat must perch on a
    /// piece. Laid out around the starter cargo with untagged kinds only.
    fn fill_hold(sim: &mut Sim) {
        for (x, y) in [(1, 0), (5, 0), (0, 1), (1, 1), (5, 1), (0, 3), (1, 3)] {
            inject_hold(sim, Kind::Seedlings, x, y);
        }
        inject_hold(sim, Kind::RationBricks, 3, 0);
        inject_hold(sim, Kind::RationBricks, 2, 2);
        inject_hold(sim, Kind::RationBricks, 4, 2);
    }

    /// A seed whose first departure wins the boarding roll.
    fn rat_seed() -> u64 {
        (0..500_u64)
            .find(|&s| splitmix(s ^ rats::SALT_BOARD, 1) % rats::BOARD_CHANCE == 0)
            .expect("a boarding roll within 500 seeds")
    }

    /// A crowded sim just departed for Venus on a boarding seed.
    fn rat_underway() -> Sim {
        let mut sim = Sim::new(rat_seed());
        crowd_hold(&mut sim);
        launch(&mut sim, SATURN);
        sim
    }

    #[test]
    fn a_crowded_departure_rolls_a_stowaway_deterministically() {
        let sim = rat_underway();
        assert!(
            sim.cues().contains(&Cue::RatAboard),
            "the boarding roll must announce itself: {:?}",
            sim.cues()
        );
        let rat = sim.rat().expect("the boarding roll hit");
        assert_eq!(rat.chases, 0);
        assert_eq!(rat.cell, rat.prev_cell, "a fresh stowaway has not hopped");
        assert!(
            !cell_covered(&sim, rat.cell),
            "with empty cells on offer it boards bare floor"
        );
        let again = rat_underway();
        assert_eq!(again.rat(), sim.rat());
        assert_eq!(again.save_string(), sim.save_string());
    }

    #[test]
    fn boarding_is_one_in_four_and_gated_by_crowding_and_the_crate() {
        // Crowded holds: the roll lands near one in four over many seeds.
        let mut boarded = 0_usize;
        for seed in 0..400_u64 {
            let mut sim = Sim::new(seed);
            crowd_hold(&mut sim);
            launch(&mut sim, SATURN);
            boarded += usize::from(sim.rat().is_some());
        }
        assert!(
            (60..=140).contains(&boarded),
            "{boarded}/400 boardings is far from one in four"
        );
        // A lean hold (starter cargo, 5 of 24 cells): never.
        for seed in 0..100_u64 {
            let mut sim = Sim::new(seed);
            launch(&mut sim, SATURN);
            assert!(
                sim.rat().is_none(),
                "seed {seed} boarded a rat onto a lean hold"
            );
        }
        // A suspicious crate aboard: never, even crowded on a boarding
        // seed — the hum unnerves them at the gangway.
        let mut sim = Sim::new(rat_seed());
        crowd_hold(&mut sim);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 4, 0);
        launch(&mut sim, SATURN);
        assert!(sim.rat().is_none(), "the hum should keep rats ashore");
        assert!(!sim.cues().contains(&Cue::RatAboard));
    }

    #[test]
    fn the_rat_skitters_and_nibbles_on_its_derived_schedule() {
        let mut sim = rat_underway();
        let rat = sim.rat().expect("boarding seed");
        let (mut due_move, mut due_nibble) = (rat.next_move, rat.next_nibble);
        let mut skitters = 0_usize;
        let mut nibbles = 0_usize;
        for _ in 0..6000 {
            sim.advance(TICK_DT, &InputFrame::default());
            for cue in sim.cues() {
                match cue {
                    Cue::RatSkitter { intensity } => {
                        assert!((0.0..=1.0).contains(intensity), "skitter {intensity}");
                        assert_eq!(sim.tick(), due_move, "skitter off schedule");
                        let rat = sim.rat().expect("it skittered, it is aboard");
                        assert_eq!(rat.moved_at, sim.tick());
                        assert_ne!(rat.cell, rat.prev_cell, "a hop must move");
                        assert!(
                            !cell_covered(&sim, rat.cell),
                            "11 empty cells: every hop lands on bare floor"
                        );
                        due_move = rat.next_move;
                        skitters += 1;
                    }
                    Cue::RatNibble => {
                        assert_eq!(sim.tick(), due_nibble, "nibble off schedule");
                        due_nibble = sim.rat().expect("it nibbled, it is aboard").next_nibble;
                        nibbles += 1;
                    }
                    _ => {}
                }
            }
        }
        // 13 of 24 cells stowed keeps it fed through the Venus dock.
        assert!(sim.rat().is_some(), "the rat left a well-stocked hold");
        // ~10 s hops and ~45 s nibbles across 100 s of sim time.
        assert!((7..=13).contains(&skitters), "{skitters} skitters in 100 s");
        assert!((1..=3).contains(&nibbles), "{nibbles} nibbles in 100 s");
        assert!(
            sim.pieces().iter().any(|p| p.gnawed),
            "a nibble must leave a mark"
        );
    }

    #[test]
    fn the_nibble_gnaws_the_nearest_piece_breaking_ties_low() {
        // Scripted: the rat placed by hand between the starter pieces —
        // scrap (id 0) at (0,2)-(1,2), vial (id 1) at (0,0), pearls (id 2)
        // at (2,0)-(2,1) — with the nibble due on the next tick.
        let mut sim = Sim::new(7);
        sim.rats.rat = Some(Rat {
            cell: (1, 1),
            prev_cell: (1, 1),
            moved_at: 0,
            next_move: u64::MAX,
            next_nibble: 1,
            chases: 0,
        });
        sim.advance(TICK_DT, &InputFrame::default());
        assert_eq!(sim.cues(), [Cue::RatNibble]);
        let gnawed = |sim: &Sim| -> Vec<u32> {
            sim.pieces()
                .iter()
                .filter(|p| p.gnawed)
                .map(|p| p.id)
                .collect()
        };
        // From (1,1) both the scrap and the pearls are one cell away; the
        // vial is two. The tie breaks to the lower id: the scrap.
        assert_eq!(gnawed(&sim), [0], "nearest rule or tie-break broke");
        // Perched on the pearls, distance zero wins outright.
        sim.rats.rat.as_mut().expect("still aboard").cell = (2, 1);
        sim.rats.rat.as_mut().expect("still aboard").next_nibble = 2;
        sim.advance(TICK_DT, &InputFrame::default());
        assert_eq!(sim.cues(), [Cue::RatNibble]);
        assert_eq!(gnawed(&sim), [0, 2]);
        // A re-gnaw of a bitten piece changes nothing but the sound.
        sim.rats.rat.as_mut().expect("still aboard").next_nibble = 3;
        let before = sim.pieces().to_vec();
        sim.advance(TICK_DT, &InputFrame::default());
        assert_eq!(sim.cues(), [Cue::RatNibble]);
        assert_eq!(sim.pieces(), before, "a re-gnaw must change nothing");
    }

    #[test]
    fn three_chases_evict_the_stowaway() {
        let mut sim = rat_underway();
        for round in 1..=2_u8 {
            let cell = sim.rat().expect("still aboard").cell;
            let at = cell_center(cell.0, cell.1);
            sim.advance(0.0, &press_at(at.x, at.y));
            assert_eq!(sim.cues(), [Cue::RatChased], "chase {round}");
            let rat = sim.rat().expect("two chases are survivable");
            assert_eq!(rat.chases, round);
            assert_ne!(rat.cell, cell, "a chased rat relocates instantly");
            assert!(sim.held(0).is_none(), "a chase lifts nothing");
        }
        let cell = sim.rat().expect("still aboard").cell;
        let at = cell_center(cell.0, cell.1);
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.cues(), [Cue::RatChased, Cue::RatLeft]);
        assert!(sim.rat().is_none(), "the third chase abandons ship");
    }

    #[test]
    fn a_press_on_a_perched_rat_chases_and_never_lifts_the_piece() {
        let mut sim = Sim::new(rat_seed());
        fill_hold(&mut sim);
        launch(&mut sim, SATURN);
        let rat = sim.rat().expect("a full hold still boards");
        assert!(
            cell_covered(&sim, rat.cell),
            "a full hold leaves only perches"
        );
        let at = cell_center(rat.cell.0, rat.cell.1);
        let before = sim.pieces().to_vec();
        sim.advance(0.0, &press_at(at.x, at.y));
        // Rat first: no Pickup, no cargo cues, nothing lifted or moved.
        assert_eq!(sim.cues(), [Cue::RatChased]);
        assert!(sim.held(0).is_none(), "the piece under the rat stays put");
        assert_eq!(sim.pieces(), before);
        // The rat hopped away, so the same press now lifts that piece: a
        // piece pick happens exactly where no rat sits.
        assert_ne!(sim.rat().expect("chased, not gone").cell, rat.cell);
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.cues(), [Cue::Pickup]);
        assert!(sim.held(0).is_some());
    }

    #[test]
    fn a_lean_hold_at_the_dock_sends_the_rat_ashore() {
        let mut sim = rat_underway();
        travel_to_dock(&mut sim);
        assert!(
            sim.rat().is_some(),
            "13 of 24 cells is plenty to eat: it stays aboard"
        );
        // Test scaffolding: unload the injected bricks as if traded away,
        // leaving the starter 5 of 24 cells — under the walk-off gate.
        sim.pieces
            .retain(|p| !(p.kind == Kind::RationBricks && matches!(p.loc, Loc::Hold { .. })));
        launch(&mut sim, GUILD);
        let mut left_at = None;
        while matches!(sim.ship().state, ShipState::Traveling { .. }) {
            sim.advance(TICK_DT, &InputFrame::default());
            if sim.cues().contains(&Cue::RatLeft) {
                left_at = Some(sim.tick());
                assert!(
                    sim.cues().contains(&Cue::Arrive),
                    "the walk-off happens at the dock, not mid-flight"
                );
            }
        }
        assert!(left_at.is_some(), "nothing to eat: the rat walks");
        assert!(sim.rat().is_none());
    }

    /// Fast-forward the current leg onto its dock.
    fn travel_to_dock(sim: &mut Sim) {
        let leg = leg_of(sim);
        sim.fast_forward(leg + 10);
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
    }

    #[test]
    fn a_gnawed_piece_still_trades_and_the_station_resells_it_bitten() {
        // A run whose opening shelf has a free slot, so the gifted piece
        // restocks instead of vanishing into the back room.
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
        // Bite the vial by hand (the schedule tests earn the nibble).
        let vial = sim
            .pieces
            .iter_mut()
            .find(|p| p.kind == Kind::PerfumeVial)
            .expect("starter vial");
        vial.gnawed = true;
        let vial = vial.id;
        // Gift it through the lever: the one door out.
        drag(
            &mut sim,
            cell_center(0, 0),
            slot_center(&layout::GIVE_SLOTS, 0),
        );
        let accept = rect_center(layout::ACCEPT_LEVER);
        sim.advance(0.0, &press_at(accept.x, accept.y));
        assert!(
            matches!(sim.cues(), [Cue::Accept { .. }]),
            "a gnawed gift still trades: {:?}",
            sim.cues()
        );
        // It restocked the shelf still bitten: the gnaw travels the economy.
        let resold = sim
            .pieces()
            .iter()
            .find(|p| p.id == vial)
            .expect("restocked, not destroyed");
        assert!(matches!(resold.loc, Loc::StationShelf { .. }));
        assert!(resold.gnawed, "the bite is permanent");
    }

    #[test]
    fn saves_continue_mid_rat_tenure_with_the_bite_intact() {
        let mut sim = rat_underway();
        // Deep enough into the leg that the rat has skittered and nibbled.
        let mut nibbled = false;
        for _ in 0..3000 {
            sim.advance(TICK_DT, &InputFrame::default());
            nibbled |= sim.cues().contains(&Cue::RatNibble);
        }
        assert!(nibbled, "the first nibble lands inside 50 s");
        assert!(sim.pieces().iter().any(|p| p.gnawed));
        let restored = Sim::from_save(&sim.save_string()).expect("own save must parse");
        assert_eq!(restored.rat(), sim.rat());
        assert_eq!(restored.pieces(), sim.pieces());
        assert_eq!(restored.legs(), sim.legs());
        assert_save_continues(sim, 8_000);
    }

    #[test]
    fn fast_forward_matches_stepwise_across_a_rat_tenure() {
        let base = rat_underway();
        // Through every skitter, the first nibble, and the Venus dock.
        let n = leg_of(&base) + 200;
        let mut ff = base.clone();
        ff.fast_forward(n);
        assert!(ff.cues().is_empty(), "fast_forward must suppress rat cues");
        let mut step = base;
        for _ in 0..n {
            step.advance(TICK_DT, &InputFrame::default());
        }
        assert_eq!(ff.save_string(), step.save_string());
        assert_eq!(ff.rat(), step.rat());
        assert_eq!(ff.pieces(), step.pieces());
    }
}
