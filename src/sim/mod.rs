//! The whole game: pure, deterministic, and macroquad-free.
//!
//! Space Trucking is an ambient hauling loop — pick a destination while
//! docked, pull the launch lever, cruise in real time, auto-dock, and carry
//! cargo into the station's own room to trade it for other cargo, with no
//! currency in sight. All of it lives here as a plain library: the
//! frontend's only channel in is an [`InputFrame`], and its only channels
//! out are the state getters plus [`Sim::cues`]. A given seed plus a given
//! input sequence produces a bit-identical run, which is what makes the
//! save format, the tests, and the benches possible.
//!
//! The ship is a **graph of rooms** (`room`, docs/ROOMS.md). The cabin is
//! room 0; the burner rides bolted to its starboard door; a point of
//! interest attaches its own trade room at the dock and takes it away
//! again at cast-off. Attach and detach ride the input schedule as four
//! small integers, so the graph is a pure function of (seed, inputs)
//! exactly as the cargo board is.
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
mod encounter;
mod event;
pub mod layout;
pub mod map;
mod rats;
pub mod room;
pub mod save;

use std::ops::{Add, AddAssign, Mul, MulAssign, Sub};

pub use barter::{Barter, VALUE};
pub use cargo::{
    KIND_COUNT, Kind, Loc, Mount, Piece, Tag, Violation, first_fit, lamp, lamp_lit, lit_adjacent,
    placement_check, placement_legal, player_owned,
};
pub use encounter::{AD_SWATS, Drone, Encounter, EncounterKind};
use encounter::{Drones, Encounters};
use event::Omen;
pub use map::{
    COMET, GUILD, HERMITAGE, INNER_RING, POI_COUNT, POIS, Poi, PoiId, SATURN, SHIP_SPEED, SUN,
    Ship, ShipState, Track, UMBRA, WANDERER, comet_visible, leg_endpoints, poi_pos,
};
pub use rats::Rat;
use rats::Rats;
pub use room::{CABIN, MAX_ROOMS, PortId, Refusal, RoomId, RoomKind, Rooms, Tile};
pub use save::SaveError;

/// Length of one simulation step. Ticks are always exactly this long.
pub const TICK_DT: f32 = 1.0 / 60.0;

/// Logical world width. The renderer scales this onto the window, so the sim
/// never learns what a pixel is.
///
/// Grown from the console-era 800 when the room net moved in east of the
/// classic rects — growing the world keeps the star map's distance space,
/// and therefore every journey length, exactly as it was. It grew again
/// when the cabin widened, and again when every room got a **net lane** of
/// its own: [`MAX_ROOMS`] lanes side by side, each big enough for the
/// widest room net, so a room's logical rects are a pure function of its id.
pub const WORLD_W: f32 = 8642.0;

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

/// Salt for the fluff breeding windows.
const SALT_FLUFF: u64 = 0xF1_0FF5;

/// Deliveries that fill the hangar counter (the last lamp on the plate)
/// and set the Grand Parade loose.
pub const PARADE_AT: u32 = 32;

/// How long the Grand Parade takes to cross the sky, in ticks.
pub const PARADE_TICKS: u64 = 7200;

/// Ticks between fluff breeding windows: about three minutes, each fluff
/// lineage deciding independently whether this window is the one.
const FLUFF_WINDOW: u64 = 10_800;

/// Most fluffs the hold will breed to. Mercy, not realism.
const FLUFF_CAP: usize = 8;

/// The stoker's beat, ticks: underway with the alongside quiet, one
/// hopper piece goes into the fire this often. Twelve seconds — slow
/// enough to snatch a mistake back off a tile.
const STOKE_PERIOD: u64 = 720;

/// Boost ticks per point of flammability: a couch (3) pushes the ship
/// at double speed for forty-five seconds. Public so views can scale
/// "how much fire is banked" against the same number the fire uses.
pub const STOKE_PER_FLAM: u64 = 900;

/// The drone hangs this far from the ship while advertising.
const DRONE_ORBIT: f32 = 22.0;

/// Click radius for swatting the drone.
const DRONE_RADIUS: f32 = 10.0;

/// Every kind's bit set in a discovery-ledger mask (see `Sim::familiar`).
///
/// Written as a shift down rather than a shift up because the table has
/// reached the mask's width: `1 << 32` is not a `u32`, and the day
/// `KIND_COUNT` grows again the ledger widens with it (see
/// `cargo::KIND_COUNT`).
pub(crate) const KNOWN_ALL: u32 = u32::MAX >> (u32::BITS as usize - KIND_COUNT);

/// The discovery ledger a fresh contractor starts with: the Guild is home
/// turf — every kind reads true there — and everywhere else is fog.
fn home_familiar() -> [u32; POI_COUNT] {
    let mut familiar = [0_u32; POI_COUNT];
    familiar[usize::from(GUILD)] = KNOWN_ALL;
    familiar
}

/// Starter cargo and where it is stowed in the cabin: trade goods low in
/// the room, and the ship's fittings — light and instruments alike — hung
/// at their traditional berths, every one a movable piece.
const STARTER_CARGO: [(Kind, u8, u8); 10] = [
    (Kind::ScrapAlloy, 4, 5),
    (Kind::PerfumeVial, 3, 3),
    (Kind::BrinePearls, 6, 3),
    // The ship's one light. Every other lumen aboard is cargo too —
    // lights-out is a legal state and the emissive instruments carry it
    // (docs/BAY.md, "Lights are cargo") — so the starter lamp hangs
    // over mid-floor where losing it is a choice, not an accident. The
    // ceiling chart folds over the starboard cornice, so its columns
    // run BACKWARDS against the floor's: (18, 6) is the pendant over
    // floor cell (6, 6), the middle of the wider room.
    (Kind::CeilingLamp, 18, 6),
    // The instruments (BAY.md, "Instruments are cargo"): the window at
    // its old cornice punch-out, the gauges and the lever clustered on
    // the front wall beside it, and the chart tank on the starboard
    // wall by the burner doorway — off the baseboard ring, and behind
    // the floor cells flanking the doorway, where tall furniture
    // rarely stands, so cargo and the tank's housing seldom fight over
    // the wall. Every one of them clears the cabin's four doorways,
    // which the threshold rule keeps empty.
    (Kind::Window, 4, 12),
    // And the porthole every hull of this class was launched with,
    // mid-course on the port flank where a bunk would be. It is not an
    // instrument — nobody hangs one to fly by — but a working ship has
    // more than one hole in it, and a starter board with exactly one
    // window is a starter board that never exercises the case the
    // exterior was rebuilt for (docs/ART_DIRECTION_3D.md, "One wall,
    // one sky"). Two windows, two walls, two skies, from the first boot.
    (Kind::Porthole, 1, 8),
    (Kind::ChartTank, 12, 5),
    (Kind::EtaGauge, 5, 11),
    (Kind::DestPreview, 3, 12),
    (Kind::LaunchLever, 5, 10),
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
/// Every derived randomness in the sim — visit stock, jump schedules,
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

/// An attach request: four small integers.
///
/// The only thing about the room graph that ever crosses the interface
/// (docs/ROOMS.md, "The attachment interface"). Poses, boxes, and lattice
/// occupancy never do — every replica computes its own, identically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attach {
    pub anchor: RoomId,
    pub anchor_port: PortId,
    pub kind: RoomKind,
    pub port: PortId,
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
    /// Quick-move modifier held (Shift): a press moves the piece under the
    /// pointer straight to its obvious destination instead of lifting it.
    pub shift: bool,
    /// Whether the player's own wall clock reads deep night (23:30–06:00
    /// local). The frontend measures it, the sim only ever consults it
    /// inside press handlers — never in the tick — so sparse recordings
    /// stay exact.
    pub night: bool,
    /// **The room this player's body occupies.**
    ///
    /// The sim learns rooms, not positions (docs/ROOMS.md): a room id is a
    /// discrete berth-shaped datum, not a coordinate, and it gives the
    /// launch and detach gates a law instead of six frontends' private
    /// opinions. Like `night` it is consulted only inside press handlers,
    /// so sparse recordings stay exact. Default is the cabin.
    pub occupied: RoomId,
    /// Attach a room this frame — the multiplayer interface, written now
    /// and wired later (a crewmate joining is an attach).
    pub attach: Option<Attach>,
    /// Detach a room this frame. Shutting the door on an event room is
    /// this, and it is always available: disengagement is participation.
    pub detach: Option<RoomId>,
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
    /// the hangar. Fires alongside `Arrive`, before that visit's room.
    Delivered,
    /// A piece was lifted.
    Pickup,
    /// A piece landed somewhere legal.
    Place,
    /// `hard` = a placement rule refused an in-room drop; soft = an ignored
    /// click or drop.
    Reject {
        hard: bool,
    },
    /// A deal was struck at a room's handshake; `value` is the generosity
    /// overshoot, `0..=1`.
    Accept {
        value: f32,
    },
    /// A handshake worked with nothing to commit.
    Refuse,
    /// A room came alongside and mated.
    Attached,
    /// A room parted.
    Parted,
    /// An attach or detach was refused, and the law that refused it.
    Refit {
        refusal: Refusal,
    },
    /// A press marked (or unmarked) a piece of a room's stock: *I want
    /// that one*. A hint to the offer, never a demand.
    Mark {
        on: bool,
    },
    OmenStart,
    Jump,
    OmenEnd,
    /// Free cargo came aboard at a roomless dock: comet ice chipped at
    /// perihelion, `intensity` scaling with the haul.
    Harvest {
        intensity: f32,
    },
    /// ??? took three mysterious crates and left one very mysterious one.
    Exchange,
    /// Something pulled alongside for a stretch of the leg.
    EncounterStart,
    /// It fell astern.
    EncounterEnd,
    /// The gas station topped the tanks up: a sliver of leg skipped.
    GasBoost,
    /// The casino paid out: the stake stands and a prize is on the floor.
    CasinoWin,
    /// The house won. Enjoy the commemorative chip.
    CasinoLoss,
    /// The whale sang; `intensity` is how near the verse felt.
    WhaleSong {
        intensity: f32,
    },
    /// An ad drone latched onto the hull; every screen is ads now.
    AdStart,
    /// A swat landed on the drone.
    AdSwat,
    /// The ads stopped — swatted off, bored, or docked away.
    AdEnd,
    /// A fluff became two fluffs. Nobody saw it happen.
    FluffBirth,
    /// The burner took a piece from the hopper: it is gone, and the
    /// fire pushes. `intensity` is the kind's flammability over the
    /// scale's top — slag burns at zero and merely disposes.
    Burn {
        intensity: f32,
    },
    /// The hangar counter filled: the Grand Parade is leaving the dock.
    ParadeStart,
    /// Ambient hull creak while traveling.
    Creak {
        intensity: f32,
    },
    /// A rat stowed away as the ship cast off.
    RatAboard,
    /// The rat hopped to another cabin cell. Quiet; ambient texture.
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

/// A piece mid-carry. Each crew member can hold at most one, and a piece
/// held by anyone is unGrabbable by everyone else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    /// Id of the piece being carried.
    pub piece: u32,
    /// Where it was lifted from, and where it snaps back to.
    pub origin: Loc,
    /// Whether dropping at the current pointer would succeed.
    pub legal: bool,
}

/// Which classes of berth could accept the held piece.
///
/// Derived from the same ownership rule [`Sim::resolve_drop`] applies —
/// the berth's room and tile class, and nothing else. The renderer glows
/// exactly these, so what a room invites is always what a release does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// One independent flag per tile class is the honest shape here; folding
// them into an enum would misrepresent that several can invite at once.
#[allow(clippy::struct_excessive_bools)]
pub struct DropTargets {
    /// Ordinary cells of any attached room.
    pub berth: bool,
    /// Some cabinet's cubbies (a stowable piece, a free cubby).
    pub stow: bool,
    /// A calling room's offer area (the player's own cargo, proposed).
    pub offer: bool,
    /// The incinerator's hazard tiles.
    pub consume: bool,
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
    /// The room graph: what the ship is, and what is currently alongside.
    rooms: Rooms,
    pieces: Vec<Piece>,
    /// Next piece id; never reused within a run.
    next_piece: u32,
    /// Each crew member's carry in progress, indexed by [`PlayerId`].
    held: [Option<Held>; MAX_CREW],
    /// Pieces of a room's stock the player has marked: *I want that one*.
    /// A hint to the composed offer; cleared on resolution.
    marks: Vec<u32>,
    /// Crates the Guild has seized at its dock, monotonic within a run.
    /// This is the counter cluster helms report to the guild server.
    deliveries: u32,
    /// The current visit's trade, `Some` iff a trading POI's room is
    /// alongside.
    barter: Option<Barter>,
    /// The current visit's jittered value table; meaningful iff trading.
    values: [u8; KIND_COUNT],
    /// Times each POI has been docked at.
    visits: [u32; POI_COUNT],
    /// Departures so far, salting each leg's event schedules. Shared by the
    /// event siblings, so it lives here rather than in either of them.
    legs: u64,
    omen: Omen,
    rats: Rats,
    encounters: Encounters,
    drones: Drones,
    /// The tick the hangar counter filled and the Grand Parade cast off,
    /// if it ever has. Set once per run; the sky is different after.
    parade_at: Option<u64>,
    /// The comet apparition (perihelion pass) already harvested, if any.
    /// One haul of ice per pass: after that the comet is picked clean
    /// until it swings out and back again.
    comet_visit: Option<u64>,
    /// Boost ticks left in the burner: while positive, each travel tick
    /// spends one and gains one — double speed until the fire dies down.
    /// Banked across docks; the stoker wastes nothing.
    stoke: u64,
    /// Pieces ever gifted to the Hermitage; its stock grows from this.
    karma: u32,
    /// Per-station bitmask of kinds the player has traded there — the
    /// discovery ledger. Bit k set means kind k's value has been seen
    /// demonstrated in public at that station.
    familiar: [u32; POI_COUNT],
    /// Whether any crew member's wall clock reads deep night, refreshed
    /// from the frame every application round. Transient: consulted only
    /// by press handlers (Umbra selection), never by the tick, and never
    /// serialized.
    night: bool,
    /// Which room each crew member's body is in, refreshed from the frame
    /// every application round. Transient for the same reason `night` is:
    /// the gates consult it inside press handlers only.
    occupied: [RoomId; MAX_CREW],
    /// The rule behind the most recent hard reject, for the renderer's
    /// icon flash. Transient UI feedback: never serialized.
    last_violation: Option<Violation>,
}

impl Sim {
    /// Build a sim from a seed: docked at the Guild Station with the starter
    /// cargo stowed, the burner bolted on, and the Guild's trade room
    /// alongside with its first visit's goods out.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let mut next_piece = 0_u32;
        let rooms = Rooms::new();
        let pieces: Vec<Piece> = STARTER_CARGO
            .iter()
            .map(|&(kind, x, y)| {
                let piece = Piece {
                    id: next_piece,
                    kind,
                    variant: rng.u8(..cargo::VARIANTS),
                    gnawed: false,
                    loc: Loc::Hold { room: CABIN, x, y },
                };
                next_piece += 1;
                piece
            })
            .collect();
        debug_assert!(
            (0..pieces.len()).all(|i| {
                let Loc::Hold { room, x, y } = pieces[i].loc else {
                    return false;
                };
                placement_legal(&rooms, &pieces, pieces[i].id, pieces[i].kind, room, x, y)
            }),
            "starter cargo must be stowed legally"
        );

        let mut visits = [0_u32; POI_COUNT];
        visits[usize::from(GUILD)] = 1;
        let pos = map::poi_pos(GUILD, 0);

        let mut sim = Self {
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
            rooms,
            pieces,
            next_piece,
            held: [None; MAX_CREW],
            marks: Vec::new(),
            deliveries: 0,
            barter: None,
            values: [0; KIND_COUNT],
            visits,
            legs: 0,
            omen: Omen::new(),
            rats: Rats::new(),
            encounters: Encounters::new(),
            drones: Drones::new(),
            parade_at: None,
            comet_visit: None,
            stoke: 0,
            karma: 0,
            familiar: home_familiar(),
            night: false,
            occupied: [CABIN; MAX_CREW],
            last_violation: None,
        };
        sim.open_trade(GUILD, 1);
        // Construction is not a frame: the dock's own attach happened
        // before anybody was listening.
        sim.cues.clear();
        sim
    }

    /// Consume one frame's worth of real time as player 0, returning how
    /// many fixed ticks ran. `frame_dt` is clamped to [`MAX_FRAME_DT`]; warp
    /// multiplies both the frame and the clamp by [`WARP_FACTOR`]. The solo
    /// frontend's entry point; lockstep replicas call [`Sim::crew_tick`].
    pub fn advance(&mut self, frame_dt: f32, input: &InputFrame) -> u32 {
        // Cues describe this frame only; last frame's have been consumed.
        self.cues.clear();
        self.night = input.night;
        self.occupied = [CABIN; MAX_CREW];
        self.occupied[0] = input.occupied;
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
    /// - Attach and detach: applied in player order too, against the graph
    ///   as earlier players left it, and refused by name when the laws say
    ///   so.
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
        for (player, input) in inputs.iter().enumerate() {
            self.occupied[player] = input.occupied;
        }
        let mut placed = Vec::new();
        for (player, input) in inputs.iter().enumerate() {
            self.apply_input(player as PlayerId, input, &mut placed);
        }
        if !self.paused {
            self.step();
        }
    }

    /// One player's input events: reseed, toggles, the graph, then — unless
    /// the sim is paused by the time their turn comes — pointer edges.
    /// `placed` carries the pieces successfully dropped earlier in the same
    /// application round, for same-tick drop contention.
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
        if let Some(attach) = input.attach {
            self.request_attach(attach);
        }
        if let Some(room) = input.detach {
            self.request_detach(room);
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
            // A default frame carries no pointer, so every carry in progress
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

    /// The room graph: every attached room, its pose on the lattice, and
    /// its mates. The presentation derives its whole floor plan from this.
    #[must_use]
    pub const fn rooms(&self) -> &Rooms {
        &self.rooms
    }

    /// Every live piece, wherever it sits.
    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// The piece `player` is mid-carry with, if any. Out-of-range players
    /// hold nothing.
    #[must_use]
    pub fn held(&self, player: PlayerId) -> Option<&Held> {
        self.held.get(usize::from(player))?.as_ref()
    }

    /// Boost ticks left in the burner. Frontends scale roar, glow, and
    /// streak-stretch from this; the push itself is in the travel tick.
    #[must_use]
    pub const fn stoke(&self) -> u64 {
        self.stoke
    }

    /// Whether the fire is pushing right now.
    #[must_use]
    pub const fn stoked(&self) -> bool {
        self.stoke > 0
    }

    /// Every carry in progress, in player order.
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

    /// The current visit's trade. `Some` iff a trading POI's room is
    /// alongside.
    #[must_use]
    pub const fn barter(&self) -> Option<&Barter> {
        self.barter.as_ref()
    }

    /// Pieces of a room's stock the player has marked as interesting.
    #[must_use]
    pub fn marks(&self) -> &[u32] {
        &self.marks
    }

    /// The pile the trading room would hand over if the handshake were
    /// worked right now — piece ids, ascending. **The offer is not a
    /// number and not a needle**: it is goods, on tiles, that you can see.
    /// Derived, never stored, so it can never disagree with the board.
    #[must_use]
    pub fn composed(&self) -> Vec<u32> {
        self.trade_room()
            .map(|room| self.compose_for(room))
            .unwrap_or_default()
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

    /// Whether the player has ever traded `kind` at the current dock, so
    /// its value has been seen demonstrated. Meaningless while traveling.
    #[must_use]
    pub fn kind_familiar(&self, kind: Kind) -> bool {
        let Some(barter) = &self.barter else {
            return true;
        };
        self.familiar[usize::from(barter.station)] & (1 << kind.index()) != 0
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
            HERMITAGE => {
                // Known givers are always answered; strangers only catch
                // the window lit.
                self.karma > 0
                    || map::hermitage_lit(self.tick)
                    || self.ship.state == ShipState::Docked(HERMITAGE)
            }
            _ => true,
        }
    }

    /// Whether the comet is around but already picked clean this
    /// apparition: physically there, nothing left to chip off.
    #[must_use]
    pub fn comet_spent(&self) -> bool {
        map::comet_visible(self.tick) && self.comet_visit == Some(map::comet_apparition(self.tick))
    }

    /// Whether a course can be charted to `id` right now: it is there,
    /// its papers (if any) are satisfied, and it has something left to
    /// visit. The renderer's hover invite reads this too, so the console
    /// never invites a click the sim would refuse.
    #[must_use]
    pub fn poi_chartable(&self, id: PoiId) -> bool {
        self.poi_visible(id) && !self.inner_ring_locked(id) && !(id == COMET && self.comet_spent())
    }

    /// Mysterious crates aboard: berthed in a room that rides. Deliberately
    /// NOT counted: crates boxed in a cabinet — ??? does not open your
    /// furniture (docs/BAY.md), which makes the cabinet the one place a
    /// crate can ride without a summons — nor crates lying in a room that
    /// is only alongside.
    #[must_use]
    pub fn mysterious_aboard(&self) -> u32 {
        self.pieces
            .iter()
            .filter(|piece| {
                piece.kind == Kind::MysteriousCrate
                    && matches!(piece.loc, Loc::Hold { room, .. } if self.rooms.riding(room))
            })
            .count() as u32
    }

    /// Whether a transit chit rides aboard — the inner ring's toll.
    #[must_use]
    pub fn transit_chit_aboard(&self) -> bool {
        self.pieces.iter().any(|piece| {
            piece.kind == Kind::TransitChit
                && matches!(piece.loc, Loc::Hold { room, .. } if self.rooms.riding(room))
        })
    }

    /// Whether charting to `id` is currently refused for want of papers.
    ///
    /// The three inner-ring factions barely tolerate each other: a direct
    /// course from one inner world to another needs a transit chit aboard.
    /// Arriving from the outer ring (or the Guild) is nobody's business
    /// but yours.
    #[must_use]
    pub fn inner_ring_locked(&self, id: PoiId) -> bool {
        let ShipState::Docked(at) = self.ship.state else {
            return false;
        };
        INNER_RING.contains(&at)
            && INNER_RING.contains(&id)
            && at != id
            && !self.transit_chit_aboard()
    }

    /// This leg's encounter, if one is scheduled or alongside.
    #[must_use]
    pub const fn encounter(&self) -> Option<&Encounter> {
        self.encounters.current.as_ref()
    }

    /// Whether the ad drone is attached and advertising.
    #[must_use]
    pub fn advertising(&self) -> bool {
        self.drones.advertising()
    }

    /// Swats already landed on the drone, for the renderer's wobble.
    #[must_use]
    pub fn drone_swats(&self) -> u8 {
        self.drones.drone.map_or(0, |drone| drone.swats)
    }

    /// Where the ad drone hangs while advertising: a tight orbit around
    /// the ship, position derived from the tick so the sim's hit-test and
    /// the renderer agree exactly.
    #[must_use]
    pub fn drone_pos(&self) -> Option<Vec2> {
        if !self.drones.advertising() {
            return None;
        }
        let angle = (self.tick % 720) as f32 / 720.0 * std::f32::consts::TAU;
        Some(Vec2::new(
            angle.cos().mul_add(DRONE_ORBIT, self.ship.pos.x),
            angle.sin().mul_add(DRONE_ORBIT, self.ship.pos.y),
        ))
    }

    /// The Grand Parade's crossing, `0..=1`, while it is in the sky.
    #[must_use]
    pub fn parade(&self) -> Option<f32> {
        let at = self.parade_at?;
        let elapsed = self.tick.saturating_sub(at);
        (elapsed < PARADE_TICKS).then(|| elapsed as f32 / PARADE_TICKS as f32)
    }

    /// Whether the hangar counter has ever filled. The sky is different
    /// after: mysterious crates surface on ordinary stock.
    #[must_use]
    pub const fn paraded(&self) -> bool {
        self.parade_at.is_some()
    }

    /// Whether a suspicious piece rides aboard.
    #[must_use]
    pub fn suspicious_aboard(&self) -> bool {
        self.pieces.iter().any(|piece| {
            matches!(piece.kind.tag(), Some(Tag::Suspicious))
                && matches!(piece.loc, Loc::Hold { room, .. } if self.rooms.riding(room))
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

    // ---- The room graph ----

    /// Which room and cell `p` names, if it names one. The arbiter: the
    /// lane says which room, the room's own net says whether that cell
    /// exists.
    #[must_use]
    pub fn cell_at(&self, p: Vec2) -> Option<(RoomId, u8, u8)> {
        let (room, x, y) = layout::cell_at(p)?;
        self.rooms.tile(room, x, y).map(|_| (room, x, y))
    }

    /// The tile class of a piece's berth, following a cubby to its host.
    #[must_use]
    fn tile_of(&self, piece: &Piece) -> Option<Tile> {
        match piece.loc {
            Loc::Hold { room, x, y } | Loc::Laid { room, x, y } => self.rooms.tile(room, x, y),
            Loc::Stow { .. } => Some(Tile::Plain),
        }
    }

    /// The room a piece is berthed in, following a cubby to its cabinet.
    #[must_use]
    fn room_of(&self, piece: &Piece) -> Option<RoomId> {
        piece.loc.room(&self.pieces)
    }

    /// The attached trade room, if a counterparty is alongside.
    #[must_use]
    fn trade_room(&self) -> Option<RoomId> {
        self.rooms.find(RoomKind::Trade)
    }

    /// An attach asked for over the input schedule. Every refusal is
    /// named and nothing else happens.
    fn request_attach(&mut self, attach: Attach) {
        match self
            .rooms
            .attach(attach.anchor, attach.anchor_port, attach.kind, attach.port)
        {
            Ok(_) => self.cues.push(Cue::Attached),
            Err(refusal) => self.cues.push(Cue::Refit { refusal }),
        }
    }

    /// A detach asked for over the input schedule — shutting the door.
    fn request_detach(&mut self, room: RoomId) {
        match self.part_check(room) {
            Ok(()) => self.part(room),
            Err(refusal) => self.cues.push(Cue::Refit { refusal }),
        }
    }

    /// The gangway law's detach gates: **a seam that could strand anything
    /// refuses to part.** Nothing detaches while it holds something of
    /// yours; nothing of yours detaches while it holds you.
    fn part_check(&self, room: RoomId) -> Result<(), Refusal> {
        if room == CABIN {
            return Err(Refusal::Root);
        }
        if self.rooms.get(room).is_none() {
            return Err(Refusal::Absent);
        }
        if self.occupied.iter().any(|&at| self.rooms.beyond(room, at)) {
            return Err(Refusal::Aboard);
        }
        for piece in &self.pieces {
            let Some(at) = self.room_of(piece) else {
                continue;
            };
            if !self.rooms.beyond(room, at) {
                continue;
            }
            if player_owned(&self.rooms, &self.pieces, piece.loc) {
                return Err(Refusal::Cargo);
            }
        }
        if self.offer_pending(room) {
            return Err(Refusal::Pending);
        }
        Ok(())
    }

    /// Whether a proposal still lies on `room`'s offer area.
    fn offer_pending(&self, room: RoomId) -> bool {
        self.pieces.iter().any(|piece| {
            matches!(piece.loc, Loc::Hold { room: at, x, y }
                if self.rooms.beyond(room, at) && self.rooms.tile(at, x, y) == Some(Tile::Offer))
        })
    }

    /// Cut a room loose and let whatever is still the room's own go with
    /// it. Only ever called through [`Sim::part_check`], so nothing of the
    /// player's is ever in there.
    fn part(&mut self, room: RoomId) {
        let doomed: Vec<u32> = self
            .pieces
            .iter()
            .filter(|piece| {
                self.room_of(piece)
                    .is_some_and(|at| self.rooms.beyond(room, at))
            })
            .map(|piece| piece.id)
            .collect();
        self.pieces.retain(|piece| !doomed.contains(&piece.id));
        self.marks.retain(|id| !doomed.contains(id));
        for held in &mut self.held {
            if matches!(held, Some(h) if doomed.contains(&h.piece)) {
                *held = None;
            }
        }
        let gone: Vec<RoomId> = self
            .rooms
            .iter()
            .map(|(id, _)| id)
            .filter(|&id| self.rooms.beyond(room, id))
            .collect();
        for id in gone {
            let _ = self.rooms.detach(id);
        }
        if self.trade_room().is_none() {
            self.barter = None;
        }
        self.cues.push(Cue::Parted);
    }

    /// Attach a room the game itself asked for — a dock, an event — through
    /// the spawn contract's deterministic walk.
    fn call_room(&mut self, kind: RoomKind) -> Option<RoomId> {
        match self.rooms.spawn(kind, CABIN) {
            Ok(id) => {
                self.cues.push(Cue::Attached);
                Some(id)
            }
            Err(refusal) => {
                self.cues.push(Cue::Refit { refusal });
                None
            }
        }
    }

    /// Detach every calling room, walking anything of the player's back
    /// aboard first. Departure and arrival both do this; the launch gate
    /// has already run by the time cast-off calls it, so the walk-back is
    /// belt and braces at a dock and a no-op at a launch.
    fn dismiss_callers(&mut self) {
        let callers: Vec<RoomId> = self
            .rooms
            .iter()
            .filter(|(_, room)| !room.kind.riding())
            .map(|(id, _)| id)
            .collect();
        for room in &callers {
            self.walk_aboard(*room);
        }
        for room in callers {
            if self.rooms.get(room).is_some() {
                self.part(room);
            }
        }
        self.marks.clear();
    }

    /// Move every player-owned piece out of `room` to its first legal
    /// berth aboard. Conservation before convenience.
    fn walk_aboard(&mut self, room: RoomId) {
        let strays: Vec<u32> = self
            .pieces
            .iter()
            .filter(|piece| {
                self.room_of(piece) == Some(room)
                    && player_owned(&self.rooms, &self.pieces, piece.loc)
            })
            .map(|piece| piece.id)
            .collect();
        for id in strays {
            let Some(index) = self.pieces.iter().position(|piece| piece.id == id) else {
                continue;
            };
            let piece = self.pieces[index];
            let berth =
                if piece.kind.covering() {
                    cargo::dress_fit(&self.rooms, &self.pieces, id, piece.kind)
                        .map(|(room, x, y)| Loc::Laid { room, x, y })
                } else {
                    first_fit(&self.rooms, &self.pieces, id, piece.kind)
                        .map(|(room, x, y)| Loc::Hold { room, x, y })
                };
            if let Some(loc) = berth {
                self.pieces[index].loc = loc;
            }
        }
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
            // member vanished mid-carry): snap back silently rather than
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

    /// A press chases the rat, works a room's handshake, lifts a piece,
    /// marks a room's stock, or actuates whatever it landed on. The rat
    /// comes first: a press on its cell shoos it and does NOT lift the
    /// piece under it.
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
        if matches!(self.ship.state, ShipState::Traveling { .. }) {
            if let Some(at) = self.drone_pos() {
                if (p - at).length() <= DRONE_RADIUS && self.drones.on_press(&mut self.cues) {
                    return;
                }
            }
        }
        if let Some(room) = self.handshake_at(p) {
            self.work_handshake(room);
            return;
        }
        if shift && self.quick_move(p, placed) {
            return;
        }
        let grabbed = layout::piece_at(&self.pieces, p).map(|piece| {
            (
                piece.id,
                piece.kind,
                piece.loc,
                self.tile_of(piece),
                cargo::laid_pinned(&self.pieces, piece),
            )
        });
        if let Some((id, kind, origin, tile, pinned)) = grabbed {
            if tile == Some(Tile::Stock) {
                // The handle rule already reserved the body-click for
                // function; on a piece of a room's stock, that function is
                // *I want that one*.
                self.toggle_mark(id);
                return;
            }
            if self.held_by_crew(id) {
                // Someone else got there first — this tick (first in
                // player order wins) or an earlier one. Losing a grab race
                // is not an error, so it makes no noise at all.
                return;
            }
            if (kind == Kind::Cabinet && cargo::cabinet_occupied(&self.pieces, id)) || pinned {
                // Furniture full of goods, or a dressing with cargo
                // standing on it: it stays put until cleared.
                self.last_violation = Some(Violation::Occupied);
                self.cues.push(Cue::Reject { hard: true });
                return;
            }
            self.held[usize::from(player)] = Some(Held {
                piece: id,
                origin,
                legal: true,
            });
            self.cues.push(Cue::Pickup);
        } else if matches!(self.ship.state, ShipState::Docked(_)) {
            self.on_press_docked(p);
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// Mark or unmark a piece of a room's stock. Marks are a hint to the
    /// offer, never a demand, and they clear on resolution.
    fn toggle_mark(&mut self, id: u32) {
        if let Some(at) = self.marks.iter().position(|&other| other == id) {
            self.marks.remove(at);
            self.cues.push(Cue::Mark { on: false });
        } else {
            self.marks.push(id);
            self.marks.sort_unstable();
            self.cues.push(Cue::Mark { on: true });
        }
    }

    /// Which room's handshake fixture `p` landed on, if any. The fixture
    /// is set into the room's own fabric, so its cell is not a berth and
    /// no cargo can ever cover it.
    fn handshake_at(&self, p: Vec2) -> Option<RoomId> {
        let (room, x, y) = layout::cell_at(p)?;
        let kind = self.rooms.kind(room)?;
        (kind.handshake() == Some((x, y))).then_some(room)
    }

    /// Work a room's handshake: the one physical act that commits.
    fn work_handshake(&mut self, room: RoomId) {
        match self.rooms.kind(room) {
            Some(RoomKind::Trade) => self.strike_deal(room),
            Some(RoomKind::Wreck) => self.claim_salvage(room),
            Some(RoomKind::Parlor) => self.spin_wager(room),
            Some(RoomKind::Pump) => self.gas_top_up(),
            _ => self.cues.push(Cue::Reject { hard: false }),
        }
    }

    /// Pieces of the player's lying on `room`'s offer area, ascending.
    fn proposal(&self, room: RoomId) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .pieces
            .iter()
            .filter(|piece| {
                matches!(piece.loc, Loc::Hold { room: at, x, y }
                    if at == room && self.rooms.tile(at, x, y) == Some(Tile::Offer))
            })
            .map(|piece| piece.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// The room's own goods, in tile order.
    fn stock_of(&self, room: RoomId) -> Vec<u32> {
        let mut ids: Vec<(u8, u8, u32)> = self
            .pieces
            .iter()
            .filter_map(|piece| match piece.loc {
                Loc::Hold { room: at, x, y }
                    if at == room && self.rooms.tile(at, x, y) == Some(Tile::Stock) =>
                {
                    Some((y, x, piece.id))
                }
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.into_iter().map(|(_, _, id)| id).collect()
    }

    /// What the trading room would compose against the standing proposal:
    /// the best pile of its own stock the proposal's value covers, marked
    /// kinds preferred, ties broken deterministically.
    fn compose_for(&self, room: RoomId) -> Vec<u32> {
        let Some(barter) = &self.barter else {
            return Vec::new();
        };
        let gnaw = barter::gnaw_loved(barter.station);
        let value = |id: u32| {
            self.pieces
                .iter()
                .find(|piece| piece.id == id)
                .map_or(0, |piece| {
                    barter::piece_value(&self.rooms, piece, &self.pieces, &self.values, gnaw)
                })
        };
        let budget: u32 = self.proposal(room).into_iter().map(value).sum();
        let candidates: Vec<(u32, u32, bool)> = self
            .stock_of(room)
            .into_iter()
            .map(|id| {
                (
                    id,
                    value(id) + barter::STOCK_MARKUP,
                    self.marks.contains(&id),
                )
            })
            .collect();
        barter::compose(budget, &candidates)
    }

    /// The handshake at a trading room: the standing offer commits and
    /// ownership crosses. What you proposed becomes the room's stock (it
    /// resells what you sold it, and the overflow goes into the back
    /// room); what the room composed lands on its own deck, yours to carry
    /// aboard.
    fn strike_deal(&mut self, room: RoomId) {
        let Some(station) = self.barter.as_ref().map(|barter| barter.station) else {
            self.cues.push(Cue::Reject { hard: false });
            return;
        };
        let given = self.proposal(room);
        if given.is_empty() {
            // Nothing proposed: there is no deal to strike.
            self.cues.push(Cue::Refuse);
            return;
        }
        let taken = self.compose_for(room);
        let gnaw = barter::gnaw_loved(station);
        let worth = |sim: &Self, id: u32| {
            sim.pieces
                .iter()
                .find(|piece| piece.id == id)
                .map_or(0, |piece| {
                    barter::piece_value(&sim.rooms, piece, &sim.pieces, &sim.values, gnaw)
                })
        };
        let given_value: u32 = given.iter().map(|&id| worth(self, id)).sum();

        // The room's answer lands on its own deck first, so the goods it
        // gave up leave room for the goods it took. Anything the deck
        // cannot hold is not granted at all — a room never hands over
        // what it has nowhere to set down, and the arithmetic prices
        // exactly what crossed.
        let stock_tiles = barter::tiles_of(&self.rooms, room, Tile::Stock);
        let mut taken_value = 0_u32;
        let mut granted = Vec::new();
        for id in &taken {
            let Some(index) = self.pieces.iter().position(|piece| piece.id == *id) else {
                continue;
            };
            let kind = self.pieces[index].kind;
            let asking = worth(self, *id) + barter::STOCK_MARKUP;
            if let Some(loc) = self.deck_berth(room, *id, kind) {
                self.pieces[index].loc = loc;
                taken_value += asking;
                granted.push(*id);
            }
        }
        let taken = granted;
        let value = barter::deal_value(given_value, taken_value);

        if station == HERMITAGE && taken.is_empty() {
            // The hermits remember every piece given, forever.
            self.karma += given.len() as u32;
        }
        // Every kind that crossed is learned here for good: the deal's
        // arithmetic was just demonstrated in public.
        let learned: Vec<Kind> = given
            .iter()
            .chain(taken.iter())
            .filter_map(|&id| self.pieces.iter().find(|piece| piece.id == id))
            .map(|piece| piece.kind)
            .collect();
        let mask = &mut self.familiar[usize::from(station)];
        for kind in learned {
            *mask |= 1 << kind.index();
        }

        // What was proposed becomes the room's, restocked in tile order;
        // whatever will not fit goes into the back room.
        let mut doomed = Vec::new();
        let mut used: Vec<(u8, u8)> = Vec::new();
        for id in given {
            let Some(index) = self.pieces.iter().position(|piece| piece.id == id) else {
                continue;
            };
            let kind = self.pieces[index].kind;
            let tile = stock_tiles.iter().copied().find(|&(x, y)| {
                !used.contains(&(x, y))
                    && placement_legal(&self.rooms, &self.pieces, id, kind, room, x, y)
            });
            match tile {
                Some((x, y)) => {
                    used.push((x, y));
                    self.pieces[index].loc = Loc::Hold { room, x, y };
                }
                None => doomed.push(id),
            }
        }
        self.pieces.retain(|piece| !doomed.contains(&piece.id));
        for held in &mut self.held {
            let orphaned = matches!(held, Some(h) if !self.pieces.iter().any(|p| p.id == h.piece));
            if orphaned {
                *held = None;
            }
        }
        self.marks.clear();
        self.cues.push(Cue::Accept { value });
    }

    /// The handshake in a wreck: nobody is watching, and what you marked
    /// is what you carry. The derelict asks nothing and answers nothing.
    fn claim_salvage(&mut self, room: RoomId) {
        let claimed: Vec<u32> = self
            .stock_of(room)
            .into_iter()
            .filter(|id| self.marks.contains(id))
            .collect();
        if claimed.is_empty() {
            self.cues.push(Cue::Refuse);
            return;
        }
        let mut lifted = Vec::new();
        for id in &claimed {
            let Some(index) = self.pieces.iter().position(|piece| piece.id == *id) else {
                continue;
            };
            let kind = self.pieces[index].kind;
            if let Some(loc) = self.deck_berth(room, *id, kind) {
                self.pieces[index].loc = loc;
                lifted.push(*id);
            }
        }
        if lifted.is_empty() {
            self.cues.push(Cue::Refuse);
            return;
        }
        self.marks.retain(|id| !lifted.contains(id));
        self.cues.push(Cue::Accept { value: 0.0 });
    }

    /// The handshake in the parlor: the coin comes down once for whatever
    /// is staked on the offer area. Win and a gilded idol lands on the
    /// house's floor beside your stake; lose and the stake IS still there
    /// — transmuted into one commemorative casino chip, which the house
    /// insists is priceless. Conservation holds either way: no piece is
    /// ever destroyed, merely disrespected.
    fn spin_wager(&mut self, room: RoomId) {
        let staked = self.proposal(room);
        if staked.is_empty() {
            self.cues.push(Cue::Refuse);
            return;
        }
        if Encounters::casino_coin(self.seed, self.tick) {
            for _ in &staked {
                self.spawn_in(room, Kind::GildedIdol);
            }
            let _ = room;
            self.cues.push(Cue::CasinoWin);
        } else {
            for id in staked {
                let Some(index) = self.pieces.iter().position(|piece| piece.id == id) else {
                    continue;
                };
                self.pieces[index].kind = Kind::CasinoChip;
                self.pieces[index].variant = self.rng.u8(..cargo::VARIANTS);
            }
            self.cues.push(Cue::CasinoLoss);
        }
    }

    /// The handshake in the pump bay: top up, once, and skip a sliver of
    /// the remaining leg.
    fn gas_top_up(&mut self) {
        let Some(enc) = &mut self.encounters.current else {
            self.cues.push(Cue::Reject { hard: false });
            return;
        };
        if enc.kind != EncounterKind::GasStation || enc.used {
            self.cues.push(Cue::Reject { hard: false });
            return;
        }
        enc.used = true;
        if let ShipState::Traveling {
            from,
            to,
            progress,
            leg_ticks,
        } = self.ship.state
        {
            // Inexplicable fuel: five percent of what remains, skipped.
            let boosted = progress + (leg_ticks - progress) / 20;
            self.ship.state = ShipState::Traveling {
                from,
                to,
                progress: boosted,
                leg_ticks,
            };
        }
        self.cues.push(Cue::GasBoost);
    }

    /// Where a room sets down goods that have just become the player's:
    /// its own ordinary deck first, then its offer area, so a cramped
    /// room still hands over what it agreed to.
    fn deck_berth(&self, room: RoomId, id: u32, kind: Kind) -> Option<Loc> {
        self.free_berth_in(room, Some(id), kind, Tile::Plain)
            .or_else(|| self.free_berth_in(room, Some(id), kind, Tile::Offer))
    }

    /// The first free berth of class `class` in `room` for `kind`.
    fn free_berth_in(
        &self,
        room: RoomId,
        ignore: Option<u32>,
        kind: Kind,
        class: Tile,
    ) -> Option<Loc> {
        let id = ignore.unwrap_or(u32::MAX);
        barter::tiles_of(&self.rooms, room, class)
            .into_iter()
            .find(|&(x, y)| placement_legal(&self.rooms, &self.pieces, id, kind, room, x, y))
            .map(|(x, y)| Loc::Hold { room, x, y })
    }

    /// A shift-press: move the piece under `p` straight to its one obvious
    /// destination — cargo aboard onto the calling room's offer area,
    /// anything of the player's lying in a calling room back to its first
    /// legal berth aboard (per the quality-of-life brief: the first legal
    /// spot, even if that is a bad idea). Returns whether the press was
    /// consumed.
    fn quick_move(&mut self, p: Vec2, placed: &mut Vec<u32>) -> bool {
        let Some(piece) = layout::piece_at(&self.pieces, p).copied() else {
            return false;
        };
        if self.tile_of(&piece) == Some(Tile::Stock) {
            // A room's own goods do not quick-move; the body-click marks.
            return false;
        }
        if self.held_by_crew(piece.id) {
            // Someone is carrying it; a modifier press cannot yank it away.
            return true;
        }
        if (piece.kind == Kind::Cabinet && cargo::cabinet_occupied(&self.pieces, piece.id))
            || cargo::laid_pinned(&self.pieces, &piece)
        {
            // Same refusal as the grab: full furniture and pinned
            // dressings stay put.
            self.last_violation = Some(Violation::Occupied);
            self.cues.push(Cue::Reject { hard: true });
            return true;
        }
        let at = self.room_of(&piece);
        let aboard = at.is_some_and(|room| self.rooms.riding(room));
        let target = if aboard {
            self.rooms
                .iter()
                .find(|(_, room)| !room.kind.riding())
                .map(|(id, _)| id)
                .filter(|_| !cargo::last_vital_aboard(&self.rooms, &self.pieces, &piece))
                .and_then(|room| self.free_berth_in(room, Some(piece.id), piece.kind, Tile::Offer))
        } else if piece.kind.covering() {
            cargo::dress_fit(&self.rooms, &self.pieces, piece.id, piece.kind)
                .map(|(room, x, y)| Loc::Laid { room, x, y })
        } else {
            first_fit(&self.rooms, &self.pieces, piece.id, piece.kind)
                .map(|(room, x, y)| Loc::Hold { room, x, y })
        };
        match target {
            Some(loc) => {
                if let Some(stored) = self.pieces.iter_mut().find(|other| other.id == piece.id) {
                    stored.loc = loc;
                }
                self.last_violation = None;
                placed.push(piece.id);
                self.cues.push(Cue::Place);
                self.wanderer_retry();
            }
            None => self.cues.push(Cue::Reject { hard: false }),
        }
        true
    }

    /// Docked-only press targets: POIs and the launch lever.
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
                if self.inner_ring_locked(id) || (id == COMET && self.comet_spent()) {
                    // Refused with feedback: papers missing for an
                    // inner-to-inner hop, or a comet already picked clean
                    // this pass — visibly there, not chartable.
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
                self.poi_chartable(to)
            });
            if self.ship.selected.is_some() && !armed_valid {
                self.ship.selected = None;
            }
            if armed_valid && self.launch_gate().is_ok() {
                self.depart();
            } else {
                // No destination, or the gangway law says somebody or
                // something would be left behind: nothing is ever lost to
                // the lever.
                self.cues.push(Cue::Reject { hard: false });
            }
        } else if !icon_press(p) {
            self.cues.push(Cue::Reject { hard: false });
        }
    }

    /// A release drops the held piece: place it if the target is legal,
    /// snap it back otherwise. A drop never destroys or surrenders a piece
    /// — ownership crosses only at a room's handshake.
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
                placed.push(piece.id);
                self.cues.push(Cue::Place);
                self.wanderer_retry();
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
    /// same-tick contention.
    fn contested_only(&self, piece: &Piece, p: Vec2, placed: &[u32]) -> bool {
        let Some((room, x, y)) = self.cell_at(p) else {
            return false;
        };
        let rest: Vec<Piece> = self
            .pieces
            .iter()
            .filter(|other| !placed.contains(&other.id))
            .copied()
            .collect();
        placement_check(&self.rooms, &rest, piece.id, piece.kind, room, x, y).is_ok()
    }

    /// Where dropping `piece` at `p` would settle it, or which flavour of
    /// rejection it earns. `Err(Some(_))` is the hard reject — a stowage
    /// rule refused an in-room drop — and names the rule; `Err(None)` is a
    /// soft, ignorable miss that snaps the piece home. Every arm gates on
    /// the tile class, the same reading [`Sim::drop_targets`] advertises
    /// from, so the glowing regions and the legal ones cannot drift apart.
    fn resolve_drop(&self, piece: &Piece, p: Vec2) -> Result<Loc, Option<Violation>> {
        let Some((room, x, y)) = self.cell_at(p) else {
            return Err(None);
        };
        let Some(tile) = self.rooms.tile(room, x, y) else {
            return Err(None);
        };
        if !player_owned(&self.rooms, &self.pieces, piece.loc) {
            // A room's own goods do not move: they cross at the handshake.
            return Err(None);
        }
        match tile {
            Tile::Threshold => return Err(Some(Violation::Threshold)),
            // Nothing of the player's is ever laid on a room's own shelf;
            // ownership crosses at the handshake, never through a drop.
            Tile::Stock => return Err(None),
            Tile::Offer | Tile::Consume => {
                // Both are exits — the piece is leaving the ship, one way
                // or another — so the vital rule stands at both doors.
                if cargo::last_vital_aboard(&self.rooms, &self.pieces, piece) {
                    return Err(Some(Violation::Vital));
                }
                if tile == Tile::Consume && piece.kind == Kind::SuspiciousCrate {
                    // One thing will not go into the fire. It prefers to
                    // stay.
                    return Err(Some(Violation::Suspicious));
                }
            }
            Tile::Plain => {}
        }
        // A drop over a cabinet's body reaches for its cubbies first — but
        // only with something cubby-sized. Anything bigger falls through to
        // the grid and collides like furniture does.
        if piece.kind.cells() == (1, 1) {
            let host = self.pieces.iter().find(|other| {
                other.id != piece.id
                    && other.kind == Kind::Cabinet
                    && matches!(other.loc, Loc::Hold { .. })
                    && layout::piece_rect(&self.pieces, other).contains(p)
            });
            if let Some(host) = host {
                if !cargo::stowable(piece.kind) {
                    // The one-cell kinds a cubby refuses, named: the cold
                    // ones need the hull; a suspicious one (none is 1×1
                    // today) would name its own rule.
                    let violation = match piece.kind.tag() {
                        Some(cargo::Tag::Suspicious) => Violation::Suspicious,
                        _ => Violation::Cryo,
                    };
                    return Err(Some(violation));
                }
                return cargo::free_cubby(&self.pieces, host.id).map_or(
                    Err(Some(Violation::Occupied)),
                    |slot| {
                        Ok(Loc::Stow {
                            cabinet: host.id,
                            slot,
                        })
                    },
                );
            }
        }
        // Coverings lay into the room instead of occupying it: the
        // dressing layer's own check, same violation ladder.
        if piece.kind.covering() {
            return match cargo::dressing_check(
                &self.rooms,
                &self.pieces,
                piece.id,
                piece.kind,
                room,
                x,
                y,
            ) {
                Ok(()) => Ok(Loc::Laid { room, x, y }),
                Err(violation) => Err(Some(violation)),
            };
        }
        match placement_check(&self.rooms, &self.pieces, piece.id, piece.kind, room, x, y) {
            Ok(()) => Ok(Loc::Hold { room, x, y }),
            Err(violation) => Err(Some(violation)),
        }
    }

    /// Which classes of berth would accept `player`'s held piece, for the
    /// renderer to glow. `None` while that player holds nothing. Derived
    /// from the tile classes exactly as [`Sim::resolve_drop`] is; per-cell
    /// freeness stays with the drop itself (a glowing area with one
    /// occupied cell is still an honest invitation).
    #[must_use]
    pub fn drop_targets(&self, player: PlayerId) -> Option<DropTargets> {
        let held = self.held(player)?;
        let piece = self.pieces.iter().find(|piece| piece.id == held.piece)?;
        let ours = player_owned(&self.rooms, &self.pieces, piece.loc);
        // The exits that would certainly hard-refuse this piece do not
        // glow — the invitation and the arbiter must agree.
        let vital = cargo::last_vital_aboard(&self.rooms, &self.pieces, piece);
        Some(DropTargets {
            berth: ours,
            stow: ours
                && cargo::stowable(piece.kind)
                && self.pieces.iter().any(|other| {
                    other.id != piece.id
                        && other.kind == Kind::Cabinet
                        && matches!(other.loc, Loc::Hold { .. })
                        && cargo::free_cubby(&self.pieces, other.id).is_some()
                }),
            offer: ours && !vital && self.rooms.iter().any(|(_, room)| !room.kind.riding()),
            consume: ours && !vital && piece.kind != Kind::SuspiciousCrate,
        })
    }

    /// The gangway law's launch gate. The lever refuses unless every crew
    /// body is in a riding room, nothing of the player's rests in a
    /// calling room, every attached trade room's offer is resolved, and no
    /// unresolved event room is attached. Departure detaches every calling
    /// room as a consequence of casting off, which is safe precisely
    /// because this ran first.
    fn launch_gate(&self) -> Result<(), Refusal> {
        if self.occupied.iter().any(|&at| !self.rooms.riding(at)) {
            return Err(Refusal::Aboard);
        }
        for piece in &self.pieces {
            let Some(at) = self.room_of(piece) else {
                continue;
            };
            if !self.rooms.riding(at) && player_owned(&self.rooms, &self.pieces, piece.loc) {
                return Err(Refusal::Cargo);
            }
        }
        for (id, room) in self.rooms.iter() {
            match room.kind {
                RoomKind::Cabin | RoomKind::Burner => {}
                RoomKind::Trade => {
                    if self.offer_pending(id) {
                        return Err(Refusal::Pending);
                    }
                }
                // An unresolved event blocks the next takeoff — and the
                // free way out is always there: shut the door.
                RoomKind::Wreck | RoomKind::Parlor | RoomKind::Pump => {
                    return Err(Refusal::Pending);
                }
            }
        }
        Ok(())
    }

    /// Cast off toward the selected destination. The launch gate has
    /// already run, so detaching every calling room strands nothing.
    fn depart(&mut self) {
        let ShipState::Docked(from) = self.ship.state else {
            return;
        };
        let Some(to) = self.ship.selected else {
            return;
        };
        debug_assert!(self.launch_gate().is_ok(), "launch gate must have passed");
        let leg_ticks = map::leg_ticks(from, to, self.tick);
        // Casting off ignites the burner: hopper cargo RIDES — it is this
        // leg's fuel, fed to the fire on the stoker's beat. Only what is
        // alongside stays behind, and it leaves with its own room.
        self.dismiss_callers();
        self.barter = None;
        self.legs += 1;
        let suspicious = self.suspicious_aboard();
        self.omen
            .on_depart(self.seed, self.legs, leg_ticks, suspicious);
        self.encounters.on_depart(self.seed, self.legs, leg_ticks);
        self.drones.on_depart(self.seed, self.legs, leg_ticks);
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
            // The stoked fire pushes: one boost tick spent, one extra
            // tick of way made good — double speed until it dies down.
            if self.stoke > 0 {
                self.stoke -= 1;
                progress += 1;
            }
            self.feed_burner();
            self.omen
                .travel_tick(&mut progress, leg_ticks, &mut self.cues);
            if let Some(cue) = event::creak(self.seed, self.tick) {
                self.cues.push(cue);
            }
            let seedlings = self.pieces.iter().any(|piece| {
                piece.kind == Kind::Seedlings
                    && matches!(piece.loc, Loc::Hold { room, .. } if self.rooms.riding(room))
            });
            let watched = self.encounters.current.map(|enc| enc.opened);
            let spawn = self.encounters.travel_tick(
                self.seed,
                self.legs,
                progress,
                self.tick,
                seedlings,
                &mut self.cues,
            );
            let opened =
                watched == Some(false) && self.encounters.current.is_some_and(|enc| enc.opened);
            self.open_encounter_room(opened, &spawn);
            self.drones.travel_tick(progress, &mut self.cues);
            if self
                .cues
                .iter()
                .any(|cue| matches!(cue, Cue::EncounterStart | Cue::AdStart | Cue::OmenStart))
            {
                // Whatever just started, a fast-forwarding developer
                // should be looking at it.
                self.disengage_warp();
            }
            self.breed_fluffs();
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
    }

    /// An encounter with a counterparty or a place brings its own room
    /// alongside; weather stays a schedule. `salvage` is whatever the
    /// encounter machine wants materialised this tick.
    fn open_encounter_room(&mut self, opened: bool, salvage: &[Kind]) {
        if opened {
            match self.encounters.current.as_ref().map(|enc| enc.kind) {
                Some(EncounterKind::Derelict) => {
                    if let Some(room) = self.call_room(RoomKind::Wreck) {
                        for &kind in salvage {
                            self.stock_in(room, kind);
                        }
                        return;
                    }
                }
                Some(EncounterKind::Casino) => {
                    self.call_room(RoomKind::Parlor);
                }
                Some(EncounterKind::GasStation) => {
                    self.call_room(RoomKind::Pump);
                }
                // The meteor shower and the whale are weather: schedules
                // hashed off the seed, with cues, and nothing to attach.
                _ => {}
            }
        }
        for &kind in salvage {
            // Whatever a closing window leaves behind — the meteor's
            // souvenir, embedded in the hull — is simply aboard.
            self.spawn_in_hold(kind);
        }
    }

    /// Arrive: snap to the pad, count the visit, and bring the station's
    /// own room alongside. At the Guild the hangar steal runs first, so
    /// the visit's room never sees the crate.
    fn dock(&mut self, poi: PoiId) {
        self.ship.pos = map::poi_pos(poi, self.tick);
        self.ship.state = ShipState::Docked(poi);
        self.ship.selected = None;
        // Arriving anywhere drops out of warp: a developer fast-forwarding
        // should be looking when something happens.
        self.disengage_warp();
        self.omen.on_dock(&mut self.cues);
        self.encounters.on_dock(&mut self.cues);
        self.drones.on_dock(&mut self.cues);
        // Whatever was alongside falls astern with its room, and anything
        // of the player's inside walks aboard first.
        self.dismiss_callers();
        self.barter = None;
        if poi == GUILD {
            self.steal_crate();
        }
        // After any hangar steal, so the walk-off gate reads the cabin as
        // the dock leaves it.
        self.rats.on_dock(&self.pieces, &mut self.cues);
        self.visits[usize::from(poi)] += 1;
        let visit = self.visits[usize::from(poi)];
        match poi {
            COMET => self.harvest_comet(visit),
            // ??? brings its room but stocks nothing and asks in crates.
            WANDERER => {
                self.call_room(RoomKind::Trade);
            }
            _ => self.open_trade(poi, visit),
        }
        self.cues.push(Cue::Arrive);
    }

    /// Bring a trading POI's own room alongside and put its goods out.
    fn open_trade(&mut self, poi: PoiId, visit: u32) {
        let Some(room) = self.call_room(RoomKind::Trade) else {
            return;
        };
        self.values = barter::visit_values(self.seed, poi, visit);
        self.barter = Some(barter::open(self.seed, poi, visit));
        let cap = barter::tiles_of(&self.rooms, room, Tile::Stock).len();
        let kinds = barter::stock_kinds(
            self.seed,
            poi,
            visit,
            &self.pieces,
            self.karma,
            self.parade_at.is_some(),
            cap,
        );
        for kind in kinds {
            self.stock_in(room, kind);
        }
    }

    /// Put one piece of a room's own goods on its first free stock tile.
    fn stock_in(&mut self, room: RoomId, kind: Kind) {
        let Some(loc) = self.free_berth_in(room, None, kind, Tile::Stock) else {
            return;
        };
        self.pieces.push(Piece {
            id: self.next_piece,
            kind,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc,
        });
        self.next_piece += 1;
    }

    /// Drop a fresh piece onto a room's ordinary deck, if there is room.
    fn spawn_in(&mut self, room: RoomId, kind: Kind) {
        let Some(loc) = self.free_berth_in(room, None, kind, Tile::Plain) else {
            return;
        };
        self.pieces.push(Piece {
            id: self.next_piece,
            kind,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc,
        });
        self.next_piece += 1;
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
    /// (as the room's edges allow — ice hugs the hull like any cryo cargo)
    /// plus, one visit in three, something odd frozen inside. Once per
    /// apparition: a comet is not a vending machine, and a second dock in
    /// the same perihelion pass finds only chisel marks.
    fn harvest_comet(&mut self, visit: u32) {
        let apparition = map::comet_apparition(self.tick);
        if self.comet_visit == Some(apparition) {
            self.cues.push(Cue::Reject { hard: false });
            return;
        }
        self.comet_visit = Some(apparition);
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

    /// The offering at ???: nothing is automatic. The player carries
    /// mysterious crates into the room ??? brings alongside and lays them
    /// on its offer area, and the moment three sit there with a legal 2×2
    /// berth waiting aboard, ??? takes them and leaves one very mysterious
    /// crate. Three offered crates with no berth buzz softly and wait:
    /// nothing is ever half-taken. Checked after every placement while
    /// docked here.
    fn wanderer_retry(&mut self) {
        if self.ship.state != ShipState::Docked(WANDERER) {
            return;
        }
        let Some(room) = self.trade_room() else {
            return;
        };
        let mut offered: Vec<u32> = self
            .proposal(room)
            .into_iter()
            .filter(|id| {
                self.pieces
                    .iter()
                    .any(|piece| piece.id == *id && piece.kind == Kind::MysteriousCrate)
            })
            .collect();
        if (offered.len() as u32) < WANDERER_TOLL {
            return;
        }
        offered.truncate(WANDERER_TOLL as usize);
        let Some((berth, x, y)) = first_fit(
            &self.rooms,
            &self.pieces,
            self.next_piece,
            Kind::VeryMysteriousCrate,
        ) else {
            self.cues.push(Cue::Reject { hard: false });
            return;
        };
        for held in &mut self.held {
            if matches!(held, Some(h) if offered.contains(&h.piece)) {
                *held = None;
            }
        }
        self.pieces.retain(|piece| !offered.contains(&piece.id));
        self.pieces.push(Piece {
            id: self.next_piece,
            kind: Kind::VeryMysteriousCrate,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc: Loc::Hold { room: berth, x, y },
        });
        self.next_piece += 1;
        self.cues.push(Cue::Exchange);
    }

    /// The stoker's beat: underway, on the metronome, with nothing
    /// alongside to watch (an open encounter pauses the shovel — which
    /// is also what keeps fresh salvage grabbable), the lowest occupied
    /// `Consume` cell in the burner room's own row-major order goes into
    /// the fire. Its flammability becomes boost; slag pushes nothing and
    /// merely stops existing. This is a conservation ceremony
    /// (`Cue::Burn`), one of the named doors.
    fn feed_burner(&mut self) {
        if self.tick % STOKE_PERIOD != 0 {
            return;
        }
        let watching = self
            .encounters
            .current
            .as_ref()
            .is_some_and(encounter::Encounter::open);
        if watching {
            return;
        }
        let Some(burner) = self.rooms.find(RoomKind::Burner) else {
            return;
        };
        let fed = barter::tiles_of(&self.rooms, burner, Tile::Consume)
            .into_iter()
            .find_map(|(x, y)| {
                self.pieces
                    .iter()
                    .find(|piece| piece.loc == Loc::Hold { room: burner, x, y })
                    .map(|piece| (piece.id, piece.kind))
            });
        let Some((id, kind)) = fed else { return };
        self.pieces.retain(|piece| piece.id != id);
        self.marks.retain(|other| *other != id);
        for held in &mut self.held {
            if matches!(held, Some(h) if h.piece == id) {
                *held = None;
            }
        }
        self.stoke += u64::from(kind.flammable()) * STOKE_PER_FLAM;
        self.cues.push(Cue::Burn {
            intensity: f32::from(kind.flammable()) / 3.0,
        });
    }

    /// The fluff arithmetic: while traveling, each breeding window one
    /// berthed fluff (lowest id — the eldest) buds a copy into an adjacent
    /// free cell of the same room, up to the mercy cap. Deterministic from
    /// the window number, stateless, and honestly a little unnerving.
    fn breed_fluffs(&mut self) {
        if self.tick % FLUFF_WINDOW
            != splitmix(self.seed ^ SALT_FLUFF, self.tick / FLUFF_WINDOW) % FLUFF_WINDOW
        {
            return;
        }
        let fluffs: Vec<(u32, RoomId, u8, u8)> = self
            .pieces
            .iter()
            .filter_map(|piece| match piece.loc {
                Loc::Hold { room, x, y } if piece.kind == Kind::Fluff => {
                    Some((piece.id, room, x, y))
                }
                _ => None,
            })
            .collect();
        if fluffs.is_empty() || fluffs.len() >= FLUFF_CAP {
            return;
        }
        let &(_, room, x, y) = fluffs.iter().min_by_key(|&&(id, _, _, _)| id).unwrap();
        let neighbours = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        for (nx, ny) in neighbours {
            if placement_legal(
                &self.rooms,
                &self.pieces,
                self.next_piece,
                Kind::Fluff,
                room,
                nx,
                ny,
            ) {
                self.pieces.push(Piece {
                    id: self.next_piece,
                    kind: Kind::Fluff,
                    variant: self.rng.u8(..cargo::VARIANTS),
                    gnawed: false,
                    loc: Loc::Hold { room, x: nx, y: ny },
                });
                self.next_piece += 1;
                self.cues.push(Cue::FluffBirth);
                return;
            }
        }
    }

    /// Stow a fresh `kind` at the first legal berth aboard, if any. Free
    /// cargo only: a room's own goods arrive through [`Sim::stock_in`].
    fn spawn_in_hold(&mut self, kind: Kind) -> bool {
        let Some((room, x, y)) = first_fit(&self.rooms, &self.pieces, self.next_piece, kind) else {
            return false;
        };
        self.pieces.push(Piece {
            id: self.next_piece,
            kind,
            variant: self.rng.u8(..cargo::VARIANTS),
            gnawed: false,
            loc: Loc::Hold { room, x, y },
        });
        self.next_piece += 1;
        true
    }

    /// The hangar steal: any suspicious crate aboard is seized the moment
    /// the ship docks at the Guild — in front of the usual trading — and
    /// counted on the delivery tally with a [`Cue::Delivered`]. The
    /// singleton rule caps this at one crate per docking, and a crate held
    /// mid-carry drops first: it is dock time.
    fn steal_crate(&mut self) {
        let Some(index) = self.pieces.iter().position(|piece| {
            matches!(piece.kind.tag(), Some(Tag::Suspicious))
                && matches!(piece.loc, Loc::Hold { room, .. } if self.rooms.riding(room))
        }) else {
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
        if self.deliveries >= PARADE_AT && self.parade_at.is_none() {
            // The counter fills; whatever was being counted, it is enough.
            // The hangar opens and the Grand Parade crosses the sky.
            self.parade_at = Some(self.tick);
            self.cues.push(Cue::ParadeStart);
        }
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

    /// The burner: room 1 on every ship that ever left the yard.
    const BURNER: RoomId = 1;

    /// The room a counterparty brings alongside: room 2, because the
    /// cabin and the burner already hold 0 and 1.
    const TRADE: RoomId = 2;

    /// Seed for the scripted odyssey, found by search: the Guild's first
    /// visit stocks something, and the first leg meets no encounter or
    /// ad drone (either would auto-disengage the scripted warp).
    const ODYSSEY_SEED: u64 = 1;

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

    /// The world point at the middle of one room's net cell.
    fn cell_center(room: RoomId, x: u8, y: u8) -> Vec2 {
        rect_center(layout::cell_rect(room, x, y))
    }

    /// The cabin's own cells, which most tests mean when they say a cell.
    fn cabin(x: u8, y: u8) -> Vec2 {
        cell_center(CABIN, x, y)
    }

    /// Conjure a piece at `loc` for a test board, id from the sim's own
    /// counter so nothing collides.
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

    /// Test scaffolding: berth an extra piece in the cabin.
    fn inject_hold(sim: &mut Sim, kind: Kind, x: u8, y: u8) -> u32 {
        let id = inject_at(sim, kind, Loc::Hold { room: CABIN, x, y });
        assert!(
            placement_legal(&sim.rooms, &sim.pieces, id, kind, CABIN, x, y),
            "test piece berthed illegally at ({x}, {y})"
        );
        id
    }

    /// A docked sim with the cabin emptied: starter cargo swept aside so
    /// tests can lay exact boards.
    fn cleared(seed: u64) -> Sim {
        let mut sim = Sim::new(seed);
        sim.pieces
            .retain(|p| !matches!(p.loc, Loc::Hold { room: CABIN, .. }));
        sim
    }

    /// Carry as three zero-dt frames: press, mid-carry hold, release.
    fn drag(sim: &mut Sim, from: Vec2, to: Vec2) {
        sim.advance(0.0, &press_at(from.x, from.y));
        assert!(sim.held(0).is_some(), "nothing to lift at {from:?}");
        sim.advance(0.0, &held_at(to.x, to.y));
        sim.advance(0.0, &release_at(to.x, to.y));
    }

    /// The same carry appended to a script instead of played live.
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

    /// Test scaffolding: a rat mid-tenure, schedules wound close so the
    /// monkeys meet skitters and nibbles quickly.
    const fn inject_rat(sim: &mut Sim) {
        sim.rats.rat = Some(Rat {
            cell: (5, 5),
            prev_cell: (5, 5),
            moved_at: 0,
            next_move: 60,
            next_nibble: 120,
            chases: 0,
        });
    }

    /// The cells of a class in the room alongside, in tile order.
    fn tiles(sim: &Sim, room: RoomId, class: Tile) -> Vec<(u8, u8)> {
        barter::tiles_of(&sim.rooms, room, class)
    }

    /// The world point of the `n`th offer tile of the room alongside.
    fn offer(sim: &Sim, room: RoomId, n: usize) -> Vec2 {
        let (x, y) = tiles(sim, room, Tile::Offer)[n];
        cell_center(room, x, y)
    }

    /// The world point of the room's handshake fixture.
    fn handshake(sim: &Sim, room: RoomId) -> Vec2 {
        let (x, y) = sim
            .rooms
            .kind(room)
            .and_then(RoomKind::handshake)
            .expect("that room has a handshake");
        cell_center(room, x, y)
    }

    /// Ids of the room's own goods, in tile order.
    fn stock_ids(sim: &Sim, room: RoomId) -> Vec<u32> {
        sim.stock_of(room)
    }

    /// Select `poi` and pull the launch lever; zero-dt frames run no ticks,
    /// so the sim is exactly at departure.
    fn launched_toward(seed: u64, poi: PoiId) -> Sim {
        let mut sim = Sim::new(seed);
        launch(&mut sim, poi);
        sim
    }

    /// Select `poi` on an already-docked sim and pull the lever. The depart
    /// frame may also carry a `RatAboard` on a crowded cabin, so only the
    /// departure itself is asserted exactly.
    fn launch(sim: &mut Sim, poi: PoiId) {
        let target = sim.poi_pos(poi);
        sim.advance(0.0, &press_at(target.x, target.y));
        assert_eq!(sim.cues(), [Cue::Select]);
        assert_eq!(sim.ship().selected, Some(poi));
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert!(
            sim.cues().contains(&Cue::Depart),
            "the lever refused: {:?}",
            sim.cues()
        );
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

    /// The ship as it leaves the yard: cabin, burner bolted to its
    /// starboard door, and the Guild's own room alongside with goods out.
    #[test]
    fn a_new_ship_is_a_cabin_a_burner_and_the_dock_alongside() {
        let sim = Sim::new(7);
        assert_eq!(sim.ship().state, ShipState::Docked(GUILD));
        assert_eq!(sim.rooms().kind(CABIN), Some(RoomKind::Cabin));
        assert_eq!(sim.rooms().kind(BURNER), Some(RoomKind::Burner));
        assert_eq!(sim.rooms().kind(TRADE), Some(RoomKind::Trade));
        assert!(sim.barter().is_some(), "a counterparty means a trade");
        let aboard: Vec<&Piece> = sim
            .pieces()
            .iter()
            .filter(|piece| matches!(piece.loc, Loc::Hold { room: CABIN, .. }))
            .collect();
        assert_eq!(aboard.len(), STARTER_CARGO.len());
        for piece in aboard {
            let Loc::Hold { room, x, y } = piece.loc else {
                unreachable!()
            };
            assert!(
                placement_legal(sim.rooms(), sim.pieces(), piece.id, piece.kind, room, x, y),
                "{:?} berthed illegally at ({x}, {y})",
                piece.kind
            );
        }
        // The station's goods sit on its own stock tiles and belong to it.
        let stock = stock_ids(&sim, TRADE);
        assert!(!stock.is_empty(), "the Guild put nothing out");
        for id in stock {
            let piece = sim.pieces().iter().find(|p| p.id == id).unwrap();
            assert!(!player_owned(sim.rooms(), sim.pieces(), piece.loc));
        }
    }

    #[test]
    fn accumulator_runs_whole_ticks_and_carries_the_remainder() {
        let mut sim = Sim::new(1);
        assert_eq!(sim.advance(TICK_DT / 2.0, &InputFrame::default()), 0);
        assert_eq!(sim.tick(), 0);
        assert_eq!(sim.advance(TICK_DT / 2.0, &InputFrame::default()), 1);
        assert_eq!(sim.tick(), 1);
        assert_eq!(sim.advance(TICK_DT * 3.0, &InputFrame::default()), 3);
        assert_eq!(sim.tick(), 4);
    }

    #[test]
    fn alpha_stays_in_unit_range() {
        let mut sim = Sim::new(2);
        for i in 0..200 {
            sim.advance(
                ((i % 5) as f32).mul_add(0.003, 0.007),
                &InputFrame::default(),
            );
            let alpha = sim.alpha();
            assert!((0.0..1.0).contains(&alpha), "alpha out of range: {alpha}");
        }
    }

    /// How many whole ticks an accumulator of `banked` seconds runs — the
    /// same float arithmetic the sim does, so the expectation cannot drift
    /// from the implementation by one ULP.
    fn whole_ticks(banked: f32) -> u32 {
        let mut acc = banked;
        let mut ticks = 0;
        #[allow(clippy::while_float)]
        while acc >= TICK_DT {
            acc -= TICK_DT;
            ticks += 1;
        }
        ticks
    }

    #[test]
    fn long_frame_dt_is_clamped() {
        let mut sim = Sim::new(3);
        let ticks = sim.advance(10.0, &InputFrame::default());
        assert_eq!(ticks, whole_ticks(MAX_FRAME_DT));
    }

    #[test]
    fn warp_multiplies_time_and_its_clamp() {
        let mut sim = Sim::new(4);
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &warp);
        assert!(sim.is_warp());
        assert_eq!(sim.advance(TICK_DT, &InputFrame::default()), 16);
        let mut sim = Sim::new(4);
        sim.advance(0.0, &warp);
        assert_eq!(
            sim.advance(10.0, &InputFrame::default()),
            whole_ticks(MAX_FRAME_DT * WARP_FACTOR)
        );
    }

    #[test]
    fn pause_freezes_state() {
        let mut sim = launched(5);
        coast(&mut sim, 10);
        let before = sim.save_string();
        let pause = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &pause);
        for _ in 0..50 {
            sim.advance(TICK_DT, &InputFrame::default());
        }
        assert!(sim.is_paused());
        assert_eq!(
            sim.save_string().replace("paused 1", "paused 0"),
            before.replace("paused 1", "paused 0")
        );
    }

    #[test]
    fn pause_warp_and_reseed_announce_themselves() {
        let mut sim = Sim::new(6);
        let pause = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &pause);
        assert_eq!(sim.cues(), [Cue::Pause { paused: true }]);
        sim.advance(0.0, &pause);
        assert_eq!(sim.cues(), [Cue::Pause { paused: false }]);
        let warp = InputFrame {
            toggle_warp: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &warp);
        assert_eq!(sim.cues(), [Cue::Warp { engaged: true }]);
        let reseed = InputFrame {
            reseed: Some(99),
            ..InputFrame::default()
        };
        sim.advance(0.0, &reseed);
        assert_eq!(sim.cues(), [Cue::Reseed]);
        assert_eq!(sim.seed(), 99);
    }

    #[test]
    fn cues_do_not_outlive_the_frame_that_made_them() {
        let mut sim = Sim::new(8);
        let pause = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &pause);
        assert!(!sim.cues().is_empty());
        sim.advance(0.0, &InputFrame::default());
        assert!(sim.cues().is_empty());
    }

    #[test]
    fn launch_without_a_destination_soft_rejects() {
        let mut sim = Sim::new(9);
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
        assert!(matches!(sim.ship().state, ShipState::Docked(_)));
    }

    #[test]
    fn travel_arrives_and_docks() {
        let mut sim = launched_toward(10, URANUS);
        let leg = leg_of(&sim);
        for _ in 0..leg {
            sim.advance(TICK_DT, &InputFrame::default());
        }
        assert_eq!(sim.ship().state, ShipState::Docked(URANUS));
        assert!(sim.barter().is_some());
        assert_eq!(sim.rooms().kind(TRADE), Some(RoomKind::Trade));
    }

    #[test]
    fn drag_places_and_rejects() {
        let mut sim = cleared(11);
        let id = inject_hold(&mut sim, Kind::PerfumeVial, 5, 5);
        drag(&mut sim, cabin(5, 5), cabin(6, 6));
        let moved = sim.pieces().iter().find(|p| p.id == id).unwrap();
        assert_eq!(
            moved.loc,
            Loc::Hold {
                room: CABIN,
                x: 6,
                y: 6
            }
        );
        // A wall cell refuses floor cargo, and names the mount.
        drag(&mut sim, cabin(6, 6), cabin(5, 1));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert_eq!(sim.last_violation(), Some(Violation::Affix(Mount::Floor)));
        // And the doorway refuses everything, by name.
        drag(&mut sim, cabin(6, 6), cabin(11, 3));
        assert_eq!(sim.last_violation(), Some(Violation::Threshold));
    }

    #[test]
    fn save_round_trips() {
        let mut sim = launched(12);
        coast(&mut sim, 90);
        let text = sim.save_string();
        let restored = Sim::from_save(&text).expect("round trip");
        assert_eq!(restored.save_string(), text);
        assert_eq!(restored.pieces(), sim.pieces());
        assert_eq!(restored.rooms().order(), sim.rooms().order());
    }

    #[test]
    fn from_save_rejects_garbage() {
        assert_eq!(Sim::from_save("").err(), Some(SaveError::BadMagic));
        assert_eq!(Sim::from_save("nonsense").err(), Some(SaveError::BadMagic));
        assert_eq!(
            Sim::from_save("STV1\nseed 1\n").err(),
            Some(SaveError::UnsupportedVersion)
        );
        let save = Sim::new(13).save_string();
        let broken = save.replacen("seed", "sxxd", 1);
        assert!(matches!(
            Sim::from_save(&broken),
            Err(SaveError::Parse { .. })
        ));
    }

    /// The full scripted voyage: carry two starter pieces into the
    /// Guild's room, mark a good, shake hands, bump into the launch gate
    /// with the answer still on the station's deck, carry it aboard, and
    /// depart for the outer ring under warp.
    fn odyssey_script(sim: &Sim) -> Vec<(f32, InputFrame)> {
        let mut s = Vec::new();
        drag_frames(&mut s, cabin(3, 3), offer(sim, TRADE, 0));
        drag_frames(&mut s, cabin(6, 3), offer(sim, TRADE, 1));
        // Mark the first good on the station's shelf: I want that one.
        let stock = stock_ids(sim, TRADE);
        let marked = sim
            .pieces()
            .iter()
            .find(|p| p.id == stock[0])
            .expect("a stocked good");
        let at = rect_center(layout::piece_rect(sim.pieces(), marked));
        s.push((0.0, press_at(at.x, at.y)));
        for _ in 0..30 {
            s.push((TICK_DT, InputFrame::default()));
        }
        let shake = handshake(sim, TRADE);
        s.push((0.0, press_at(shake.x, shake.y)));
        // Select the destination and pull too early: the answer is still
        // in the station's room, and the gangway law says no.
        let dest = map::poi_pos(URANUS, 30);
        s.push((0.0, press_at(dest.x, dest.y)));
        let lever = rect_center(layout::LAUNCH_LEVER);
        s.push((0.0, press_at(lever.x, lever.y)));
        s
    }

    #[test]
    fn a_scripted_voyage_trades_through_the_room_and_gates_the_launch() {
        let mut sim = Sim::new(ODYSSEY_SEED);
        let script = odyssey_script(&sim);
        let log = play(&mut sim, &script);
        assert_eq!(count_cues(&log, |c| matches!(c, Cue::Mark { on: true })), 1);
        assert_eq!(count_cues(&log, |c| matches!(c, Cue::Accept { .. })), 1);
        // The lever refused: goods of ours still lie in the calling room.
        assert!(matches!(sim.ship().state, ShipState::Docked(GUILD)));
        assert_eq!(sim.launch_gate(), Err(Refusal::Cargo));
        // Carry the answer aboard and the gate opens.
        let ours: Vec<u32> = sim
            .pieces()
            .iter()
            .filter(|p| {
                matches!(p.loc, Loc::Hold { room: TRADE, .. })
                    && player_owned(sim.rooms(), sim.pieces(), p.loc)
            })
            .map(|p| p.id)
            .collect();
        assert!(!ours.is_empty(), "the station answered with nothing");
        for id in ours {
            let piece = *sim.pieces().iter().find(|p| p.id == id).unwrap();
            let from = rect_center(layout::piece_rect(sim.pieces(), &piece));
            let (room, x, y) =
                first_fit(sim.rooms(), sim.pieces(), id, piece.kind).expect("room aboard");
            drag(&mut sim, from, cell_center(room, x, y));
        }
        assert_eq!(sim.launch_gate(), Ok(()));
        let lever = rect_center(layout::LAUNCH_LEVER);
        sim.advance(0.0, &press_at(lever.x, lever.y));
        assert!(sim.cues().contains(&Cue::Depart));
        // Cast-off took the station's room with it.
        assert_eq!(sim.rooms().kind(TRADE), None);
        assert!(sim.barter().is_none());
    }

    #[test]
    fn save_load_continues_mid_travel() {
        let mut sim = launched(21);
        coast(&mut sim, 300);
        assert_save_continues(sim, 600);
    }

    #[test]
    fn save_load_continues_docked_with_a_proposal_standing() {
        let mut sim = Sim::new(22);
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, cabin(3, 3), to);
        assert_save_continues(sim, 120);
    }

    #[test]
    fn save_load_continues_mid_omen() {
        let mut sim = cleared(0x00C0_FFEE);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 4, 4);
        launch(&mut sim, SATURN);
        let mut fired = false;
        for _ in 0..leg_of(&sim) + 10 {
            sim.advance(TICK_DT, &InputFrame::default());
            if sim.cues().iter().any(|cue| matches!(cue, Cue::OmenStart)) {
                fired = true;
                break;
            }
            if matches!(sim.ship().state, ShipState::Docked(_)) {
                break;
            }
        }
        assert!(fired, "no omen in a whole leg");
        assert_save_continues(sim, 400);
    }

    #[test]
    fn save_load_continues_mid_carry_with_piece_at_origin() {
        let mut sim = cleared(23);
        let id = inject_hold(&mut sim, Kind::PerfumeVial, 5, 5);
        let from = cabin(5, 5);
        sim.advance(0.0, &press_at(from.x, from.y));
        assert!(sim.held(0).is_some());
        let restored = Sim::from_save(&sim.save_string()).expect("mid-carry save parses");
        assert!(restored.held(0).is_none(), "a save drops every carry");
        let piece = restored.pieces().iter().find(|p| p.id == id).unwrap();
        assert_eq!(
            piece.loc,
            Loc::Hold {
                room: CABIN,
                x: 5,
                y: 5
            }
        );
    }

    #[test]
    fn fast_forward_matches_stepwise_across_a_dock() {
        let mut stepwise = launched_toward(24, URANUS);
        let mut jumped = stepwise.clone();
        let leg = leg_of(&stepwise);
        for _ in 0..leg + 60 {
            stepwise.advance(TICK_DT, &InputFrame::default());
        }
        let catch = jumped.fast_forward(leg + 60);
        assert!(catch.arrived);
        assert_eq!(stepwise.save_string(), jumped.save_string());
    }

    #[test]
    fn fast_forward_matches_stepwise_across_an_omen_jump() {
        let mut stepwise = cleared(0x0BAD_F00D);
        inject_hold(&mut stepwise, Kind::SuspiciousCrate, 4, 4);
        launch(&mut stepwise, SATURN);
        let mut jumped = stepwise.clone();
        let leg = leg_of(&stepwise);
        for _ in 0..leg / 2 {
            stepwise.advance(TICK_DT, &InputFrame::default());
        }
        jumped.fast_forward(leg / 2);
        assert_eq!(stepwise.save_string(), jumped.save_string());
    }

    #[test]
    fn fast_forward_while_paused_stays_put() {
        let mut sim = launched(25);
        let pause = InputFrame {
            toggle_pause: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &pause);
        let before = sim.save_string();
        let catch = sim.fast_forward(600);
        assert_eq!(catch.ticks, 0);
        assert_eq!(sim.save_string(), before);
    }

    // ---- The new barter: six beats ----

    #[test]
    fn the_room_answers_a_proposal_with_goods_and_the_handshake_commits() {
        let mut sim = Sim::new(31);
        let before: Vec<u32> = stock_ids(&sim, TRADE);
        assert!(!before.is_empty());
        // Nothing proposed: the handshake has nothing to commit.
        let shake = handshake(&sim, TRADE);
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert_eq!(sim.cues(), [Cue::Refuse]);
        assert!(sim.composed().is_empty(), "an empty proposal buys nothing");

        // Propose the pearls: the room composes an answer out of stock.
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, cabin(6, 3), to);
        let composed = sim.composed();
        assert!(!composed.is_empty(), "a rich proposal must buy something");
        for id in &composed {
            assert!(before.contains(id), "the room offered someone else's goods");
        }
        // Strike the deal: what we proposed is the room's now, what it
        // composed is on its deck and ours to carry.
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert!(matches!(sim.cues().first(), Some(Cue::Accept { .. })));
        assert!(sim.marks().is_empty(), "marks clear on resolution");
        for id in composed {
            let piece = sim.pieces().iter().find(|p| p.id == id).unwrap();
            assert!(
                player_owned(sim.rooms(), sim.pieces(), piece.loc),
                "the answer must cross to the player"
            );
        }
        assert!(sim.proposal(TRADE).is_empty(), "the offer area is clear");
    }

    #[test]
    fn a_mark_steers_the_offer_and_toggles_off_again() {
        let mut sim = Sim::new(32);
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, cabin(6, 3), to);
        let stock = stock_ids(&sim, TRADE);
        assert!(stock.len() >= 2, "this test wants a choice");
        let last = *stock.last().unwrap();
        let piece = *sim.pieces().iter().find(|p| p.id == last).unwrap();
        let at = rect_center(layout::piece_rect(sim.pieces(), &piece));
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.cues(), [Cue::Mark { on: true }]);
        assert_eq!(sim.marks(), [last]);
        assert!(
            sim.composed().contains(&last),
            "a marked kind comes first in the composed pile"
        );
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.cues(), [Cue::Mark { on: false }]);
        assert!(sim.marks().is_empty());
        // Marking never lifts: the room's goods stay on their tiles.
        assert!(sim.held(0).is_none());
        assert_eq!(stock_ids(&sim, TRADE), stock);
    }

    #[test]
    fn a_rooms_own_goods_never_leave_through_a_drop() {
        let mut sim = Sim::new(33);
        let stock = stock_ids(&sim, TRADE);
        let piece = *sim.pieces().iter().find(|p| p.id == stock[0]).unwrap();
        let at = rect_center(layout::piece_rect(sim.pieces(), &piece));
        sim.advance(0.0, &press_at(at.x, at.y));
        assert!(sim.held(0).is_none(), "stock marks, it does not lift");
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == stock[0]).unwrap().loc,
            piece.loc
        );
    }

    #[test]
    fn the_hermitage_remembers_a_gift_forever() {
        let mut sim = cleared(41);
        inject_hold(&mut sim, Kind::BrinePearls, 4, 4);
        wait_for(&mut sim, HERMITAGE);
        travel_to(&mut sim, HERMITAGE);
        assert_eq!(sim.karma(), 0);
        // The hermits stock nothing for strangers, so a proposal buys
        // nothing back and the handshake reads as the gift it is.
        assert!(stock_ids(&sim, TRADE).is_empty());
        let piece = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::BrinePearls)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &piece));
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, from, to);
        let shake = handshake(&sim, TRADE);
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert!(matches!(sim.cues().first(), Some(Cue::Accept { .. })));
        assert_eq!(sim.karma(), 1, "generosity is remembered");
    }

    #[test]
    fn trading_a_kind_teaches_its_value() {
        let mut sim = cleared(42);
        inject_hold(&mut sim, Kind::BrinePearls, 4, 4);
        travel_to(&mut sim, SATURN);
        assert!(!sim.kind_familiar(Kind::BrinePearls), "Saturn is new turf");
        let piece = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::BrinePearls)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &piece));
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, from, to);
        let shake = handshake(&sim, TRADE);
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert!(sim.kind_familiar(Kind::BrinePearls));
    }

    // ---- The gangway law ----

    #[test]
    fn the_launch_gate_reads_bodies_cargo_offers_and_events() {
        let mut sim = Sim::new(51);
        assert_eq!(sim.launch_gate(), Ok(()));
        // A body in the station's room strands that body.
        sim.occupied[0] = TRADE;
        assert_eq!(sim.launch_gate(), Err(Refusal::Aboard));
        sim.occupied[0] = CABIN;
        // A proposal standing on the offer area strands the proposal.
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, cabin(3, 3), to);
        assert_eq!(sim.launch_gate(), Err(Refusal::Cargo));
        // Carrying it back aboard clears both.
        let piece = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::PerfumeVial)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &piece));
        drag(&mut sim, from, cabin(3, 3));
        assert_eq!(sim.launch_gate(), Ok(()));
    }

    #[test]
    fn a_seam_that_could_strand_anything_refuses_to_part() {
        let mut sim = Sim::new(52);
        // The cabin is the root and never parts.
        assert_eq!(sim.part_check(CABIN), Err(Refusal::Root));
        // A body in the room refuses.
        sim.occupied[0] = TRADE;
        assert_eq!(sim.part_check(TRADE), Err(Refusal::Aboard));
        sim.occupied[0] = CABIN;
        // Cargo of ours in the room refuses.
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, cabin(3, 3), to);
        assert_eq!(sim.part_check(TRADE), Err(Refusal::Cargo));
        // Take it back and the seam parts, taking the station's goods.
        let piece = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::PerfumeVial)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &piece));
        drag(&mut sim, from, cabin(3, 3));
        assert_eq!(sim.part_check(TRADE), Ok(()));
        let detach = InputFrame {
            detach: Some(TRADE),
            ..InputFrame::default()
        };
        sim.advance(0.0, &detach);
        assert!(sim.cues().contains(&Cue::Parted));
        assert_eq!(sim.rooms().kind(TRADE), None);
        assert!(sim.barter().is_none());
        assert!(
            sim.pieces()
                .iter()
                .all(|p| !matches!(p.loc, Loc::Hold { room: TRADE, .. })),
            "the station's goods left with the station"
        );
    }

    /// Selling your furnace is legal, foolish, and supported.
    #[test]
    fn the_burner_parts_like_any_other_room() {
        let mut sim = Sim::new(53);
        assert_eq!(sim.part_check(BURNER), Ok(()));
        let detach = InputFrame {
            detach: Some(BURNER),
            ..InputFrame::default()
        };
        sim.advance(0.0, &detach);
        assert_eq!(sim.rooms().kind(BURNER), None);
        // And staged fuel refuses to let it go.
        let mut sim = Sim::new(53);
        drag(&mut sim, cabin(3, 3), cell_center(BURNER, 3, 3));
        assert_eq!(sim.part_check(BURNER), Err(Refusal::Cargo));
    }

    #[test]
    fn attach_and_detach_ride_the_input_schedule() {
        let mut sim = Sim::new(54);
        // The cabin's port-side door to the pump bay's one door: a
        // forecourt declares a single seam and mates through it.
        let attach = InputFrame {
            attach: Some(Attach {
                anchor: CABIN,
                anchor_port: 3,
                kind: RoomKind::Pump,
                port: 0,
            }),
            ..InputFrame::default()
        };
        sim.advance(0.0, &attach);
        assert!(sim.cues().contains(&Cue::Attached));
        let added = sim.rooms().find(RoomKind::Pump).expect("the pump attached");
        // A slot the pump bay does not declare is refused by name, and
        // the ladder it no longer carries is one of them.
        let phantom = InputFrame {
            attach: Some(Attach {
                anchor: CABIN,
                anchor_port: room::LADDER,
                kind: RoomKind::Pump,
                port: room::HATCH,
            }),
            ..InputFrame::default()
        };
        sim.advance(0.0, &phantom);
        assert_eq!(
            sim.cues(),
            [Cue::Refit {
                refusal: Refusal::Absent
            }]
        );
        // The same port twice is refused by name, and nothing changes.
        sim.advance(0.0, &attach);
        assert_eq!(
            sim.cues(),
            [Cue::Refit {
                refusal: Refusal::Mated
            }]
        );
        // An unresolved event room blocks the next takeoff.
        assert_eq!(sim.launch_gate(), Err(Refusal::Pending));
        let detach = InputFrame {
            detach: Some(added),
            ..InputFrame::default()
        };
        sim.advance(0.0, &detach);
        assert!(sim.cues().contains(&Cue::Parted));
        assert_eq!(sim.launch_gate(), Ok(()));
    }

    // ---- Conservation ----

    /// The named doors a piece may leave by. Everything else is theft.
    fn conservation_door(cue: Cue) -> bool {
        matches!(
            cue,
            Cue::Accept { .. }
                | Cue::Burn { .. }
                | Cue::Delivered
                | Cue::Exchange
                | Cue::Parted
                | Cue::Harvest { .. }
                | Cue::FluffBirth
                | Cue::CasinoWin
                | Cue::Arrive
                | Cue::Depart
                | Cue::Reseed
        )
    }

    /// Random input, including carries that cross a doorway, never loses
    /// a piece except through one of the named ceremonies. The interesting
    /// interleaving now is a carry that crosses a seam.
    #[test]
    fn no_input_stream_loses_cargo_without_a_ceremony() {
        let mut rng = fastrand::Rng::with_seed(0x5EA1);
        for run in 0..24 {
            let mut sim = Sim::new(run);
            inject_rat(&mut sim);
            let mut count = sim.pieces().len();
            for _ in 0..900 {
                // Aim mostly at the rooms' own lanes, so carries really do
                // cross seams instead of flapping at empty world.
                let target = match rng.u8(..6) {
                    0 => rect_center(layout::LAUNCH_LEVER),
                    1 => cabin(rng.u8(0..22), rng.u8(0..13)),
                    2 => cell_center(BURNER, rng.u8(0..14), rng.u8(0..9)),
                    3 => cell_center(TRADE, rng.u8(0..16), rng.u8(0..11)),
                    4 => Vec2::new(rng.f32() * 800.0, rng.f32() * 600.0),
                    _ => {
                        let pieces = sim.pieces();
                        if pieces.is_empty() {
                            cabin(5, 5)
                        } else {
                            let piece = pieces[rng.usize(..pieces.len())];
                            rect_center(layout::piece_rect(pieces, &piece))
                        }
                    }
                };
                let input = InputFrame {
                    pointer: target,
                    press: rng.bool(),
                    held: rng.bool(),
                    release: rng.bool(),
                    shift: rng.u8(..8) == 0,
                    detach: (rng.u8(..64) == 0).then(|| rng.u8(..4)),
                    ..InputFrame::default()
                };
                sim.advance(TICK_DT, &input);
                let door = sim.cues().iter().copied().any(conservation_door);
                let now = sim.pieces().len();
                assert!(
                    now >= count || door,
                    "a piece vanished with no ceremony: {:?}",
                    sim.cues()
                );
                count = now;
            }
        }
    }

    /// The same, in lockstep, with six crew grabbing at once.
    #[test]
    fn no_crew_input_stream_loses_cargo_without_a_ceremony() {
        let mut rng = fastrand::Rng::with_seed(0xC5EA);
        for run in 0..8 {
            let mut sim = Sim::new(run);
            let mut count = sim.pieces().len();
            for _ in 0..500 {
                let mut frames = [InputFrame::default(); MAX_CREW];
                for frame in &mut frames {
                    let target = match rng.u8(..4) {
                        0 => cabin(rng.u8(0..22), rng.u8(0..13)),
                        1 => cell_center(BURNER, rng.u8(0..14), rng.u8(0..9)),
                        2 => cell_center(TRADE, rng.u8(0..16), rng.u8(0..11)),
                        _ => {
                            let pieces = sim.pieces();
                            let piece = pieces[rng.usize(..pieces.len())];
                            rect_center(layout::piece_rect(pieces, &piece))
                        }
                    };
                    *frame = InputFrame {
                        pointer: target,
                        press: rng.bool(),
                        held: rng.bool(),
                        release: rng.bool(),
                        ..InputFrame::default()
                    };
                }
                sim.crew_tick(&frames);
                let door = sim.cues().iter().copied().any(conservation_door);
                let now = sim.pieces().len();
                assert!(now >= count || door, "a piece vanished in the crowd");
                count = now;
            }
        }
    }

    // ---- Cabinets and dressings ----

    #[test]
    fn cubby_flow_stows_grabs_back_and_quick_pops() {
        let mut sim = cleared(61);
        let cabinet = inject_hold(&mut sim, Kind::Cabinet, 5, 5);
        let vial = inject_hold(&mut sim, Kind::PerfumeVial, 3, 3);
        let body = layout::piece_rect(
            sim.pieces(),
            sim.pieces().iter().find(|p| p.id == cabinet).unwrap(),
        );
        let cubby = rect_center(layout::cubby_rect(body, 0));
        drag(&mut sim, cabin(3, 3), cubby);
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == vial).unwrap().loc,
            Loc::Stow { cabinet, slot: 0 }
        );
        // An occupied cabinet will not budge.
        let anchor = rect_center(layout::cell_rect(CABIN, 5, 5));
        sim.advance(0.0, &press_at(anchor.x, anchor.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: true }]);
        assert_eq!(sim.last_violation(), Some(Violation::Occupied));
        // Shift pops it back aboard.
        sim.advance(
            0.0,
            &InputFrame {
                pointer: cubby,
                press: true,
                held: true,
                shift: true,
                ..InputFrame::default()
            },
        );
        assert!(matches!(
            sim.pieces().iter().find(|p| p.id == vial).unwrap().loc,
            Loc::Hold { .. }
        ));
    }

    #[test]
    fn dressings_lay_pin_and_peel() {
        let mut sim = cleared(62);
        let rug = inject_at(
            &mut sim,
            Kind::Rug,
            Loc::Laid {
                room: CABIN,
                x: 4,
                y: 7,
            },
        );
        let couch = inject_hold(&mut sim, Kind::Couch, 4, 7);
        assert!(cargo::laid_pinned(
            sim.pieces(),
            sim.pieces().iter().find(|p| p.id == rug).unwrap()
        ));
        // A pinned rug refuses to lift.
        let at = cabin(4, 7);
        sim.advance(0.0, &press_at(at.x, at.y));
        assert!(sim.held(0).is_some(), "the couch lifts, not the rug");
        sim.advance(0.0, &release_at(at.x, at.y));
        // Move the couch, then the rug peels.
        drag(&mut sim, cabin(4, 7), cabin(7, 4));
        assert!(matches!(
            sim.pieces().iter().find(|p| p.id == couch).unwrap().loc,
            Loc::Hold { .. }
        ));
        drag(&mut sim, cabin(4, 7), cabin(4, 9));
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == rug).unwrap().loc,
            Loc::Laid {
                room: CABIN,
                x: 4,
                y: 9
            }
        );
    }

    // ---- The burner room ----

    #[test]
    fn the_hopper_is_reversible_and_rides_into_the_fire() {
        let mut sim = cleared(71);
        let fuel = inject_hold(&mut sim, Kind::PerfumeVial, 5, 5);
        // Staging is an ordinary carry into an ordinary room.
        drag(&mut sim, cabin(5, 5), cell_center(BURNER, 3, 3));
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == fuel).unwrap().loc,
            Loc::Hold {
                room: BURNER,
                x: 3,
                y: 3
            }
        );
        // Snatching it back out is an ordinary carry too.
        drag(&mut sim, cell_center(BURNER, 3, 3), cabin(5, 5));
        assert!(matches!(
            sim.pieces().iter().find(|p| p.id == fuel).unwrap().loc,
            Loc::Hold { room: CABIN, .. }
        ));
        // Cast off with it staged and the stoker takes it on the beat.
        drag(&mut sim, cabin(5, 5), cell_center(BURNER, 3, 3));
        launch(&mut sim, SATURN);
        let mut burned = false;
        for _ in 0..STOKE_PERIOD * 2 {
            sim.advance(TICK_DT, &InputFrame::default());
            if sim.cues().iter().any(|c| matches!(c, Cue::Burn { .. })) {
                burned = true;
                break;
            }
        }
        assert!(burned, "the stoker never shovelled");
        assert!(sim.pieces().iter().all(|p| p.id != fuel));
        assert!(sim.stoked(), "a burn banks way");
    }

    /// Fuel simply stays staged, and the fire is the only thing that ever
    /// takes it: conservation becomes total, and `Cue::Jettison` — the one
    /// ceremony that still discarded — is gone from the vocabulary.
    #[test]
    fn fuel_waits_in_the_furnace_room_and_only_the_fire_takes_it() {
        let mut sim = cleared(72);
        for (i, cell) in [(3_u8, 3_u8), (4, 3), (6, 3)].into_iter().enumerate() {
            inject_hold(&mut sim, Kind::PerfumeVial, 3 + i as u8, 5);
            drag(
                &mut sim,
                cabin(3 + i as u8, 5),
                cell_center(BURNER, cell.0, cell.1),
            );
        }
        launch(&mut sim, SATURN);
        let staged = sim.pieces().len();
        let mut burns = 0;
        for _ in 0..STOKE_PERIOD + 60 {
            sim.advance(TICK_DT, &InputFrame::default());
            burns += count_cues(sim.cues(), |c| matches!(c, Cue::Burn { .. }));
        }
        assert!(burns > 0, "the stoker never shovelled");
        assert_eq!(
            sim.pieces().len(),
            staged - burns,
            "nothing left the ship but through the fire"
        );
        let waiting = sim
            .pieces()
            .iter()
            .filter(|p| matches!(p.loc, Loc::Hold { room: BURNER, .. }))
            .count();
        assert_eq!(waiting, 3 - burns, "unburned fuel waits where it was set");
    }

    #[test]
    fn the_fire_pushes_double_time() {
        let mut sim = cleared(73);
        inject_hold(&mut sim, Kind::Fluff, 5, 5);
        drag(&mut sim, cabin(5, 5), cell_center(BURNER, 3, 3));
        launch(&mut sim, SATURN);
        for _ in 0..STOKE_PERIOD + 5 {
            sim.advance(TICK_DT, &InputFrame::default());
        }
        assert!(sim.stoke() > 0);
        let ShipState::Traveling { progress, .. } = sim.ship().state else {
            panic!("still traveling")
        };
        assert!(
            progress > STOKE_PERIOD,
            "the fire should have made extra way"
        );
    }

    // ---- The vital rule's new exits ----

    #[test]
    fn the_last_vital_instrument_refuses_every_exit() {
        let mut sim = Sim::new(81);
        // The offer area of a calling room is an exit.
        let tank = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::ChartTank)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &tank));
        let to = offer(&sim, TRADE, 0);
        drag(&mut sim, from, to);
        assert_eq!(sim.last_violation(), Some(Violation::Vital));
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == tank.id).unwrap().loc,
            tank.loc
        );
        // So is the incinerator, whose whole net is hazard — a wall
        // instrument burns as readily as a couch, so it goes on the
        // furnace room's wall, and the fire does not care which.
        drag(&mut sim, from, cell_center(BURNER, 5, 0));
        assert_eq!(sim.last_violation(), Some(Violation::Vital));
        // A spare aboard releases the rule.
        inject_hold(&mut sim, Kind::ChartTank, 0, 5);
        drag(&mut sim, from, cell_center(BURNER, 5, 0));
        assert!(matches!(
            sim.pieces().iter().find(|p| p.id == tank.id).unwrap().loc,
            Loc::Hold { room: BURNER, .. }
        ));
    }

    #[test]
    fn the_humming_crate_refuses_the_fire() {
        let mut sim = cleared(82);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 4, 4);
        let from = cabin(4, 4);
        drag(&mut sim, from, cell_center(BURNER, 3, 3));
        assert_eq!(sim.last_violation(), Some(Violation::Suspicious));
    }

    // ---- Rooms and the encounters that bring them ----

    /// Launch on a leg whose encounter is `kind`, searching seeds.
    fn launched_with_encounter(kind: EncounterKind) -> Sim {
        for seed in 0..4000_u64 {
            let mut sim = cleared(seed);
            launch(&mut sim, SATURN);
            if sim
                .encounter()
                .is_some_and(|enc| enc.kind == kind && enc.end > enc.start + 60)
            {
                return sim;
            }
        }
        panic!("no seed produced a {kind:?}");
    }

    /// Run to the encounter's window and stop just inside it.
    fn into_the_window(sim: &mut Sim) {
        let start = sim.encounter().expect("an encounter").start;
        while !sim.encounter().is_some_and(Encounter::open) {
            sim.advance(TICK_DT, &InputFrame::default());
            assert!(
                matches!(sim.ship().state, ShipState::Traveling { .. }),
                "the leg ended before the window at {start}"
            );
        }
    }

    #[test]
    fn a_derelict_brings_its_hold_alongside_and_salvage_is_a_carry() {
        let mut sim = launched_with_encounter(EncounterKind::Derelict);
        into_the_window(&mut sim);
        let wreck = sim.rooms().find(RoomKind::Wreck).expect("a hold attached");
        let salvage = stock_ids(&sim, wreck);
        assert!(!salvage.is_empty(), "a derelict with nothing in it");
        // The salvage is the wreck's until claimed.
        for id in &salvage {
            let piece = sim.pieces().iter().find(|p| p.id == *id).unwrap();
            assert!(!player_owned(sim.rooms(), sim.pieces(), piece.loc));
        }
        // Mark one and work the handshake: it is yours, on the floor.
        let piece = *sim.pieces().iter().find(|p| p.id == salvage[0]).unwrap();
        let at = rect_center(layout::piece_rect(sim.pieces(), &piece));
        sim.advance(0.0, &press_at(at.x, at.y));
        assert_eq!(sim.cues(), [Cue::Mark { on: true }]);
        let shake = handshake(&sim, wreck);
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert!(matches!(sim.cues().first(), Some(Cue::Accept { .. })));
        let claimed = sim.pieces().iter().find(|p| p.id == salvage[0]).unwrap();
        assert!(player_owned(sim.rooms(), sim.pieces(), claimed.loc));
        // An unresolved event blocks the next takeoff; shutting the door
        // is free and always available — after the claim comes aboard.
        let (room, x, y) =
            first_fit(sim.rooms(), sim.pieces(), salvage[0], claimed.kind).expect("a berth");
        let from = rect_center(layout::piece_rect(sim.pieces(), claimed));
        drag(&mut sim, from, cell_center(room, x, y));
        assert_eq!(sim.part_check(wreck), Ok(()));
    }

    #[test]
    fn the_gas_station_tops_up_exactly_once() {
        let mut sim = launched_with_encounter(EncounterKind::GasStation);
        into_the_window(&mut sim);
        let pump = sim.rooms().find(RoomKind::Pump).expect("a pump bay");
        let shake = handshake(&sim, pump);
        let ShipState::Traveling { progress, .. } = sim.ship().state else {
            panic!("traveling")
        };
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert_eq!(sim.cues(), [Cue::GasBoost]);
        let ShipState::Traveling {
            progress: after, ..
        } = sim.ship().state
        else {
            panic!("traveling")
        };
        assert!(after > progress, "the top-up skipped nothing");
        sim.advance(0.0, &press_at(shake.x, shake.y));
        assert_eq!(sim.cues(), [Cue::Reject { hard: false }]);
    }

    #[test]
    fn the_casino_transmutes_losses_and_pays_winners_on_its_own_floor() {
        let mut sim = launched_with_encounter(EncounterKind::Casino);
        inject_hold(&mut sim, Kind::BrinePearls, 4, 4);
        into_the_window(&mut sim);
        let parlor = sim.rooms().find(RoomKind::Parlor).expect("a parlor");
        let wager = *sim
            .pieces()
            .iter()
            .find(|p| p.kind == Kind::BrinePearls)
            .unwrap();
        let from = rect_center(layout::piece_rect(sim.pieces(), &wager));
        let to = offer(&sim, parlor, 0);
        drag(&mut sim, from, to);
        let before = sim.pieces().len();
        let shake = handshake(&sim, parlor);
        sim.advance(0.0, &press_at(shake.x, shake.y));
        let won = sim.cues().contains(&Cue::CasinoWin);
        let lost = sim.cues().contains(&Cue::CasinoLoss);
        assert!(won || lost, "the coin never came down: {:?}", sim.cues());
        if won {
            assert_eq!(sim.pieces().len(), before + 1, "a prize on the house floor");
            assert_eq!(
                sim.pieces().iter().find(|p| p.id == wager.id).unwrap().kind,
                Kind::BrinePearls
            );
        } else {
            assert_eq!(sim.pieces().len(), before, "conservation, not destruction");
            assert_eq!(
                sim.pieces().iter().find(|p| p.id == wager.id).unwrap().kind,
                Kind::CasinoChip
            );
        }
    }

    #[test]
    fn two_swats_knock_the_ad_drone_off() {
        for seed in 0..4000_u64 {
            let mut sim = cleared(seed);
            launch(&mut sim, SATURN);
            let mut attached = false;
            for _ in 0..leg_of(&sim) {
                sim.advance(TICK_DT, &InputFrame::default());
                if sim.advertising() {
                    attached = true;
                    break;
                }
            }
            if !attached {
                continue;
            }
            for _ in 0..AD_SWATS {
                let at = sim.drone_pos().expect("it is right there");
                sim.advance(0.0, &press_at(at.x, at.y));
            }
            assert!(!sim.advertising());
            return;
        }
        panic!("no seed produced an ad drone");
    }

    // ---- The rest of the world, unchanged by the rooms ----

    #[test]
    fn each_guild_docking_steals_the_crate_and_counts_it() {
        let mut sim = cleared(91);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 4, 4);
        travel_to(&mut sim, SATURN);
        travel_to(&mut sim, GUILD);
        assert_eq!(sim.deliveries(), 1);
        assert!(
            sim.pieces().iter().all(|p| p.kind != Kind::SuspiciousCrate),
            "the hangar took it"
        );
    }

    /// Sit at the dock until `poi` can be charted — the comet has to come
    /// round, and the Hermitage has to light its window.
    fn wait_for(sim: &mut Sim, poi: PoiId) {
        for _ in 0..20_000 {
            if sim.poi_chartable(poi) {
                return;
            }
            sim.fast_forward(30);
        }
        panic!("{poi} never became chartable");
    }

    #[test]
    fn the_comet_hands_over_free_ice_and_no_counterparty() {
        let mut sim = cleared(92);
        wait_for(&mut sim, COMET);
        travel_to(&mut sim, COMET);
        assert!(sim.barter().is_none(), "a comet has no counterparty");
        assert_eq!(sim.rooms().find(RoomKind::Trade), None);
        assert!(
            sim.pieces().iter().any(|p| p.kind == Kind::CometIce),
            "no ice was chipped"
        );
    }

    #[test]
    fn three_mysterious_crates_summon_and_feed_the_wanderer() {
        let mut sim = cleared(93);
        for (i, cell) in [(3_u8, 3_u8), (5, 3), (7, 3)].into_iter().enumerate() {
            let id = inject_hold(&mut sim, Kind::MysteriousCrate, cell.0, cell.1);
            assert_eq!(id as usize, sim.next_piece as usize - 1);
            let _ = i;
        }
        assert_eq!(sim.mysterious_aboard(), WANDERER_TOLL);
        assert!(sim.poi_visible(WANDERER));
        travel_to(&mut sim, WANDERER);
        let room = sim.trade_room().expect("??? brings a room");
        for (i, cell) in [(3_u8, 3_u8), (5, 3), (7, 3)].into_iter().enumerate() {
            let to = offer(&sim, room, i);
            drag(&mut sim, cabin(cell.0, cell.1), to);
        }
        assert!(
            sim.pieces()
                .iter()
                .any(|p| p.kind == Kind::VeryMysteriousCrate),
            "??? never took the offering"
        );
        assert_eq!(sim.mysterious_aboard(), 0);
    }

    #[test]
    fn inner_to_inner_courses_need_a_transit_chit() {
        let mut sim = cleared(94);
        travel_to(&mut sim, INNER_RING[0]);
        assert!(sim.inner_ring_locked(INNER_RING[1]));
        inject_hold(&mut sim, Kind::TransitChit, 5, 5);
        assert!(!sim.inner_ring_locked(INNER_RING[1]));
    }

    #[test]
    fn the_umbra_market_only_answers_at_night() {
        let mut sim = Sim::new(95);
        assert!(!sim.poi_visible(UMBRA));
        sim.advance(
            0.0,
            &InputFrame {
                night: true,
                ..InputFrame::default()
            },
        );
        assert!(sim.poi_visible(UMBRA));
    }

    #[test]
    fn fluffs_multiply_in_transit_up_to_the_mercy_cap() {
        let mut sim = cleared(96);
        inject_hold(&mut sim, Kind::Fluff, 5, 5);
        launch(&mut sim, SATURN);
        let mut births = 0;
        for _ in 0..FLUFF_WINDOW * 3 {
            sim.advance(TICK_DT, &InputFrame::default());
            births += count_cues(sim.cues(), |c| matches!(c, Cue::FluffBirth));
            if matches!(sim.ship().state, ShipState::Docked(_)) {
                break;
            }
        }
        assert!(births > 0, "nothing bred in three windows");
    }

    #[test]
    fn omen_fires_exactly_once_at_the_derived_tick() {
        let mut sim = cleared(0x00C0_FFEE);
        inject_hold(&mut sim, Kind::SuspiciousCrate, 4, 4);
        launch(&mut sim, SATURN);
        let leg = leg_of(&sim);
        let mut starts = 0;
        let mut jumps = 0;
        for _ in 0..leg + 10 {
            sim.advance(TICK_DT, &InputFrame::default());
            starts += count_cues(sim.cues(), |c| matches!(c, Cue::OmenStart));
            jumps += count_cues(sim.cues(), |c| matches!(c, Cue::Jump));
        }
        assert_eq!(starts, 1);
        assert_eq!(jumps, 1);
    }

    #[test]
    fn without_a_crate_the_leg_never_jumps() {
        let mut sim = cleared(0x00C0_FFEE);
        launch(&mut sim, SATURN);
        let leg = leg_of(&sim);
        for _ in 0..leg {
            sim.advance(TICK_DT, &InputFrame::default());
            assert!(!sim.cues().iter().any(|c| matches!(c, Cue::Jump)));
        }
    }

    #[test]
    fn creaks_pepper_travel_and_never_the_dock() {
        let mut sim = Sim::new(97);
        for _ in 0..600 {
            sim.advance(TICK_DT, &InputFrame::default());
            assert!(
                !sim.cues().iter().any(|c| matches!(c, Cue::Creak { .. })),
                "a docked ship creaked"
            );
        }
        let mut sim = launched(97);
        let mut creaks = 0;
        for _ in 0..20_000 {
            sim.advance(TICK_DT, &InputFrame::default());
            creaks += count_cues(sim.cues(), |c| matches!(c, Cue::Creak { .. }));
            if matches!(sim.ship().state, ShipState::Docked(_)) {
                break;
            }
        }
        assert!(creaks > 0, "a whole leg without a creak");
    }

    #[test]
    fn reseed_preserves_pause_and_resets_warp() {
        let mut sim = Sim::new(98);
        let toggles = InputFrame {
            toggle_pause: true,
            toggle_warp: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &toggles);
        assert!(sim.is_paused() && sim.is_warp());
        sim.advance(
            0.0,
            &InputFrame {
                reseed: Some(1234),
                ..InputFrame::default()
            },
        );
        assert!(sim.is_paused());
        assert!(!sim.is_warp());
        assert_eq!(sim.seed(), 1234);
    }

    #[test]
    fn arrival_disengages_warp() {
        let mut sim = launched(99);
        sim.advance(
            0.0,
            &InputFrame {
                toggle_warp: true,
                ..InputFrame::default()
            },
        );
        let leg = leg_of(&sim);
        sim.fast_forward(leg + 10);
        assert!(!sim.is_warp());
    }

    // ---- The rat ----

    #[test]
    fn a_press_on_a_perched_rat_chases_and_never_lifts_the_piece() {
        let mut sim = cleared(101);
        let id = inject_hold(&mut sim, Kind::PerfumeVial, 5, 5);
        inject_rat(&mut sim);
        let at = cabin(5, 5);
        sim.advance(0.0, &press_at(at.x, at.y));
        assert!(sim.cues().contains(&Cue::RatChased));
        assert!(sim.held(0).is_none());
        assert_eq!(
            sim.pieces().iter().find(|p| p.id == id).unwrap().loc,
            Loc::Hold {
                room: CABIN,
                x: 5,
                y: 5
            }
        );
    }

    #[test]
    fn three_chases_evict_the_stowaway() {
        let mut sim = cleared(102);
        inject_rat(&mut sim);
        for _ in 0..rats::CHASE_LIMIT {
            let cell = sim.rat().expect("a rat").cell;
            let at = cabin(cell.0, cell.1);
            sim.advance(0.0, &press_at(at.x, at.y));
        }
        assert!(sim.rat().is_none());
    }

    #[test]
    fn saves_continue_mid_rat_tenure_with_the_bite_intact() {
        let mut sim = cleared(103);
        inject_hold(&mut sim, Kind::Couch, 5, 5);
        inject_rat(&mut sim);
        launch(&mut sim, SATURN);
        coast(&mut sim, 400);
        assert_save_continues(sim, 400);
    }

    // ---- Lockstep ----

    fn crew(entries: &[(PlayerId, InputFrame)]) -> CrewFrame {
        let mut frames = [InputFrame::default(); MAX_CREW];
        for &(player, frame) in entries {
            frames[usize::from(player)] = frame;
        }
        frames
    }

    #[test]
    fn crew_tick_equals_advance_for_a_solo_crew() {
        let mut solo = Sim::new(111);
        let mut lockstep = Sim::new(111);
        let script = [
            press_at(cabin(3, 3).x, cabin(3, 3).y),
            held_at(cabin(5, 5).x, cabin(5, 5).y),
            release_at(cabin(5, 5).x, cabin(5, 5).y),
            InputFrame::default(),
        ];
        for input in script {
            solo.advance(TICK_DT, &input);
            lockstep.crew_tick(&crew(&[(0, input)]));
        }
        assert_eq!(solo.save_string(), lockstep.save_string());
    }

    #[test]
    fn same_tick_grab_goes_to_the_lowest_player_in_silence() {
        let mut sim = cleared(112);
        inject_hold(&mut sim, Kind::PerfumeVial, 5, 5);
        let at = cabin(5, 5);
        let press = press_at(at.x, at.y);
        sim.crew_tick(&crew(&[(0, press), (3, press)]));
        assert!(sim.held(0).is_some());
        assert!(sim.held(3).is_none());
        assert_eq!(count_cues(sim.cues(), |c| matches!(c, Cue::Pickup)), 1);
    }

    #[test]
    fn crew_schedules_replay_bit_identically() {
        let mut a = Sim::new(113);
        let mut b = Sim::new(113);
        let mut rng = fastrand::Rng::with_seed(0x5CED);
        let mut schedule = Vec::new();
        for _ in 0..300 {
            let mut frames = [InputFrame::default(); MAX_CREW];
            for frame in &mut frames {
                let at = cabin(rng.u8(0..22), rng.u8(0..13));
                *frame = InputFrame {
                    pointer: at,
                    press: rng.bool(),
                    held: rng.bool(),
                    release: rng.bool(),
                    occupied: rng.u8(..3),
                    ..InputFrame::default()
                };
            }
            schedule.push(frames);
        }
        for frames in &schedule {
            a.crew_tick(frames);
        }
        for frames in &schedule {
            b.crew_tick(frames);
        }
        assert_eq!(a.save_string(), b.save_string());
    }

    #[test]
    fn crew_reseed_last_in_order_wins() {
        let mut sim = Sim::new(114);
        let seeded = |seed| InputFrame {
            reseed: Some(seed),
            ..InputFrame::default()
        };
        sim.crew_tick(&crew(&[(1, seeded(7)), (4, seeded(9))]));
        assert_eq!(sim.seed(), 9);
    }
}
