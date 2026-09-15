//! **The gauntlet: the checks a screenshot cannot make.**
//!
//! A human walked the station wave and found some fifteen defects that
//! fifteen agents and their screenshots had all signed off on. The
//! post-mortem named four reasons, and every one of them is a hole in the
//! *shape* of the verification rather than in anybody's care:
//!
//! 1. **A still cannot see time.** A lamp that flickers every other frame,
//!    a light that pops on as you approach it, stars that shimmer when the
//!    eye moves near a window — no screenshot ever taken finds one of
//!    those.
//! 2. **Agents shot the framing they designed for**, never the path a
//!    player walks: in through the door, round the room, up to the
//!    counter.
//! 3. **Rooms were photographed empty.** Decor that clips through cargo
//!    needs cargo standing there to clip through, and the containment
//!    test that guards this directory asks whether a fitting stays inside
//!    the room's BOX — never whether it stands where a CRATE stands.
//! 4. **Designers graded their own work.** Nobody ran an adversarial pass.
//!
//! This module is that pass, mechanised. It sweeps **every room the game
//! has** — the twelve stations through [`crate::poi::HOSTS`], the three
//! event rooms, the cabin and the burner — against a **loaded** board, and
//! reports each violation as one line naming the room, the offender, and
//! the rule: everything a fixer needs without re-deriving a number.
//!
//! # The two halves, and why they are split
//!
//! **Pure geometry** ([`sweep`]) runs headless and deterministic, in
//! `cargo test` alongside everything else. It needs no window, no GPU, and
//! no clock: it derives berths from the sim's own arbiter, poses fittings
//! through the very functions the runtime poses them with, and does
//! arithmetic.
//!
//! **Pixels** ([`walk`] and the `--gauntlet-walk` entry point) need a
//! rasteriser, so they live behind an opt-in run under `xvfb`
//! ([`tests::the_filmstrip_holds_still_and_the_light_does_not_pop`], and
//! docs/GAUNTLET.md for the invocation). What *is* pure about them — the
//! path itself, and every geometric assertion made at each waypoint —
//! runs always.
//!
//! # The families, and the defect class each closes
//!
//! | Rule | What it asserts | What it retires |
//! | --- | --- | --- |
//! | [`BERTH_CLEAR`] | no fitting stands in a cell cargo may take, at the height a rig occupies, and no rig is deeper than the band that height is measured over | decor clipping through functional cargo |
//! | [`BERTH_SEEN`] | nothing stands between a wall berth and the room | "the wall plate occludes the window" |
//! | [`BERTH_REACHED`] | every berth is workable from the walk envelope | furniture fencing off a corner of the net |
//! | [`NO_COPLANAR`] | no two drawn faces share a plane and a facing, in a room or in a rig | z-fighting, generally |
//! | [`PROP_POINTS`] | a rig's named features point where their names say | the sconce lighting the wall, the base plate on edge |
//! | [`FACE_FITS`] | a rig draws inside the cells the sim gave it, so its pick face can be its picture | a visible edge that does not answer the crosshair |
//! | [`WALK_CLEAR`] | the walked path stands in air, room to room | furniture in the doorway you enter by |
//! | [`GRID_FITS`] | every shell body stands in its own room's cells and lands on the cargo grid | a wall two centimetres off its own box, and the room next door standing inside this one |
//! | [`PART_SEATED`] | a part that names another part as what holds it up actually meets it | a couch on four stilts of air, a pane of glass floating off its own bezel |
//! | [`RIG_SEATED`] | a rig reaches the chart it is berthed on, on every chart class it may be berthed on | a crate standing on nothing, a canopy that never gets to its own deckhead |
//! | [`FURNITURE_SEATED`] | a hung body meets whatever it says holds it up | a beacon bolted to thin air, a latch floating off its wall |
//! | [`BERTH_FILLED`] | a rig is centred on the ground its plan owns and fills it | a crate half a cell out into the aisle on the one axis nothing measured |
//! | [`BERTH_TURNED`] | a rig stands up and shows the room its face, on every chart class | a pendant looking into the wall it hangs beside |
//! | [`DECK_REACHED`] | every cell of deck a body may set cargo on is walkable to from the door it comes in by | a shopfront running under its own doorway |
//! | [`FIXTURE_REACHED`] | a room's own worked hardware is workable from somewhere a body may stand | a counter you can see and cannot reach |
//! | [`FIXTURE_SEEN`] | nothing a room hangs or stocks stands across its own worked hardware | goods stacked in front of the counter they are sold over |
//!
//! # The four questions, and the map of the ones nobody had asked
//!
//! Twelve of those families were written the same way — a defect was
//! seen, and a family was written that would have caught it — which is a
//! harness that is always one round behind whoever is playing. The last
//! four were written the other way round: the space of things a rule
//! could be ABOUT was enumerated first (a body, a relation, a frame), and
//! the cells of it nobody had asked were filled in. docs/GAUNTLET.md,
//! "The map", is that enumeration, including the triples that are
//! meaningful and cannot be asked yet and the reason for each.
//!
//! # The three layers, and the one that is not described yet
//!
//! The rooms were the whole sweep for as long as everything else could
//! only be BUILT. `pieces::build_kind` composed a cargo kind straight
//! into a live Bevy world and `room::doorways` did the same for a seam's
//! hardware, so nothing pure could enumerate a rig's parts or a
//! doorway's, and no rule could be asked about either — which is how the
//! Guild's transit chit came to cut its card and its stripe to one
//! height and one centre along the whole of a stripe held at arm's
//! length, and how a seam's amber latch came to be entered twice at one
//! transform and photograph two ways.
//!
//! A kind describes its parts now ([`crate::pieces::parts`]) and so does
//! a doorway ([`room::seam_parts`]), and [`scene`] and the rig sweep read
//! those descriptions. **A doorway is read from both of its sides**: a
//! seam's frame is drawn once, by the room with the lower id, and stands
//! on the boundary the two rooms share, so the room that did not draw it
//! is swept with it anyway.
//!
//! Sixty-one findings came out of the cargo and fifty-three out of the
//! doorways, and in both cases the layer was invisible not because a rule
//! was loose but because there was nothing for a rule to be asked about.
//! **The next defect is in the layer nobody has thought to describe
//! yet** — or in the question nobody has thought to ask about a layer
//! that is fully described. [`PART_SEATED`] is the second kind: a rig's
//! parts have been enumerable since `pieces::parts` landed, and for as
//! long as they have been, every rule asked about a part and the WORLD.
//! None asked whether a part meets the other part of the same rig its
//! own name says holds it up, and three of them did not.
//!
//! [`RIG_SEATED`] is the same question asked one body out, and it needed
//! no new description either. Every family measured a rig against a
//! number the world states — the band it is composed in, the cells it
//! draws inside — and none against the one plane its own berth promises
//! it. Twenty-two kinds did not reach theirs. docs/GAUNTLET.md carries
//! that history, what the harness can and cannot see, and everything
//! else an operator needs when a check goes red.
//!
//! # The docket
//!
//! [`docket`] is the itemised list of what the gauntlet catches **today**:
//! it is a work order, not an allowlist. The sweep is asserted equal to it
//! ([`tests::the_gauntlet_finds_exactly_the_docket`]), so a new defect
//! fails the build and a fixed one fails it too, until somebody strikes
//! the line. [`ALLOWED`] is the other thing entirely — pairs the coplanar
//! detector is *wrong* about, each with the reason it is wrong.

use std::collections::BTreeMap;
use std::fmt;

use bevy::prelude::*;
use space_trucking::sim::cargo::{Kind, Loc, Mount, Piece, mount_accepts, placement_check, plan};
use space_trucking::sim::layout;
use space_trucking::sim::room::{CABIN, RoomId, RoomKind, Rooms, Surf, Tile};

use crate::art::Dressings;
use crate::pieces::{Screens, Under};
use crate::poi::{self, Fitting, Frame, Host, Shape};
use crate::rig::{EYE_HEIGHT, PITCH_LIMIT, REACH};
use crate::room::{self, Placed};
use crate::surface::{SimSurface, Station};

// ------------------------------------------------------------ the rules --

/// No fitting may stand where cargo legally stands.
pub const BERTH_CLEAR: &str = "berth-clear";
/// Nothing may stand between a wall berth and the room that reads it.
pub const BERTH_SEEN: &str = "berth-seen";
/// Every berth must be workable from somewhere a body may stand.
pub const BERTH_REACHED: &str = "berth-reached";
/// No two drawn faces may share a plane and a facing.
pub const NO_COPLANAR: &str = "coplanar-faces";
/// A rig's named features must point where their names say.
pub const PROP_POINTS: &str = "prop-points";
/// A rig must draw inside the cells the sim gave it.
pub const FACE_FITS: &str = "face-fits";
/// The walked path must stand in air.
pub const WALK_CLEAR: &str = "walk-clear";
/// The world is built of the cargo grid and aligned to it.
pub const GRID_FITS: &str = "grid-fits";
/// A part that names a seat must meet it.
pub const PART_SEATED: &str = "part-seated";
/// A rig must reach the chart it is berthed on.
pub const RIG_SEATED: &str = "rig-seated";
/// A hung body must meet whatever it says holds it up.
pub const FURNITURE_SEATED: &str = "furniture-seated";
/// A rig must fill the cells its berth spends.
pub const BERTH_FILLED: &str = "berth-filled";
/// A rig must be turned the way its chart and its room say.
pub const BERTH_TURNED: &str = "berth-turned";
/// Every cell of deck a body may set cargo on must be walkable to from
/// the door it comes in by.
pub const DECK_REACHED: &str = "deck-reached";
/// A room's own worked hardware must be reachable from somewhere a body
/// may stand.
pub const FIXTURE_REACHED: &str = "fixture-reached";
/// Nothing a berth may stand may stand between a room's own worked
/// hardware and the room that reads it.
pub const FIXTURE_SEEN: &str = "fixture-seen";

/// The name a finding about cargo is filed under. A rig is not in any
/// one room — the same crate stands in every room the game has — so the
/// thirty-two kinds answer as their own place, the way the twelve
/// stations and the ship's two rooms answer as theirs.
pub const RIGS: &str = "rigs";

/// The name a finding about a room's own net is filed under: the kind,
/// lower-cased. Same argument as [`RIGS`] one layer up — a net is folded
/// the same way in every station that has one, so a defect in how `Trade`
/// lays its deck out is one defect and not twelve.
fn kind_name(kind: RoomKind) -> String {
    format!("{kind:?}").to_lowercase()
}

/// Every rule, for the report's own headings.
pub const RULES: [&str; 16] = [
    BERTH_CLEAR,
    BERTH_SEEN,
    BERTH_REACHED,
    NO_COPLANAR,
    PROP_POINTS,
    FACE_FITS,
    WALK_CLEAR,
    GRID_FITS,
    PART_SEATED,
    RIG_SEATED,
    FURNITURE_SEATED,
    BERTH_FILLED,
    BERTH_TURNED,
    DECK_REACHED,
    FIXTURE_REACHED,
    FIXTURE_SEEN,
];

// ------------------------------------------------------------- the sizes --

/// How far two boxes must genuinely overlap before it is a clip rather
/// than a graze, in metres. A fitting's volume is taken as its AABB, and
/// a sphere's or a torus's AABB has corners the body does not, so a
/// hair of overlap is not evidence of anything.
const CLIP_SLACK: f32 = 0.004;

/// How far a rig's sole may sink into the chart it is berthed on, in
/// metres — the one direction a body is allowed past its own plan.
///
/// A sole flush with the deck shares a plane with it, and a sole above
/// it is furniture floating; burying it is how a foot meets a floor.
/// That argument was made when [`rig_fits`] measured the footprint too
/// and found the cabinet's four feet and the couch's four a third of a
/// centimetre under their own bottom edge, so it is written down here as
/// a number rather than re-argued per family. Deeper than this is a body
/// through the deck, which is not a foot.
///
/// **"Sole" is whichever face meets the chart**, because the argument
/// never mentioned gravity: a floor rig's sole is its underside and a
/// ceiling rig's is its canopy, and a canopy flush with the deckhead
/// shares a plane with it exactly as a foot flush with the deck does.
/// Which face that is comes from the chart the kind may be berthed on
/// ([`chart_joint`]), so the two families that spend this number — this
/// one from above and [`rig_seated`] from below — are asking about one
/// joint from its two sides.
const SOLE_SINK: f32 = 0.010;

/// And how much of the smaller body the overlap must eat. Same argument
/// from the other side: an AABB corner graze is a thin sliver of a round
/// body, a fitting standing in a berth is most of itself.
const CLIP_BITE: f32 = 0.05;

/// The air a wall berth is READ through, past the rig's own depth: about
/// a body's stand-off from the wall it is looking at.
const SIGHT: f32 = 0.55;

/// **How far a part may stand off the seat it names**, in metres: one
/// fight-free step of the decal ladder (`rig::layer::STEP`, 4 mm) plus
/// the thickest paint that could be riding on the seat's own face
/// (`rig::layer::SKIN`, 1.5 mm).
///
/// The step is the floor rather than the ceiling of a joint, and that is
/// the whole reason this is not simply zero: two bodies meeting on one
/// plane is a coin toss in the depth buffer, so a joint that has to READ
/// as a joint stands a step off instead of none (`pieces::GLAZE` spends
/// exactly that). What the rule refuses is the next order of magnitude —
/// a gap you can see daylight through, which on these rigs starts at
/// about a centimetre and is where all three of the family's first
/// findings sat.
const SEAT_GAP: f32 = crate::rig::layer::STEP + crate::rig::layer::SKIN;

/// How much of a wall cell's face a fitting must cover before it is
/// hiding what hangs there rather than standing beside it.
pub const OCCLUDE_BITE: f32 = 0.25;

/// Two faces closer than this are the same plane as far as the depth
/// buffer is concerned. It is [`crate::rig::layer::SKIN`], the thickest a
/// flat paint riding a rung of the decal ladder may be: two faces inside
/// one skin of each other cannot be told apart by depth, and which one
/// draws is then a question of query order, which is not an answer. The
/// ladder's own `STEP` was the other candidate and docs/GAUNTLET.md says
/// why it is the wrong one.
const FIGHT_EPS: f32 = crate::rig::layer::SKIN;

/// How much two coplanar faces must actually overlap before they can
/// fight. Abutting is not overlapping: neighbouring tile fields share a
/// plane and an edge by design, and a shared edge draws nothing twice.
const FIGHT_FOOT: f32 = 0.01;

/// Grid resolution of the standing-point search, per envelope box.
const STANCES: u8 = 8;

/// **The finest the cargo grid is ever cut**, and the unit every length
/// in the world's fabric has to be a whole number of.
///
/// A sixteenth of a cell, because that is what the fabric's own two
/// derived lengths already are: a hull plane is a quarter of the padding
/// cube (four of these) and a chart's trim is a quarter of that (one).
/// Finer than this is not a fraction of the grid any more, it is a
/// number somebody chose — which is the thing the owner's rule exists to
/// stop, and the reason this is a sixteenth rather than whatever divides
/// the offsets that happen to be in the tree today.
const GRID_STEP: f32 = crate::rig::BAY_CELL / 16.0;

/// How far off a grid line a face may land and still be called on it.
/// A millimetre: below what any eye or any depth buffer can tell, and an
/// order under the thinnest paint the decal ladder carries.
const GRID_EPS: f32 = 0.001;

// ------------------------------------------------------------ a finding --

/// One violation, in the one line a fixer can act on without re-deriving
/// anything.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// Whose room: a station's name, an event room's, or the ship's own.
    pub room: String,
    /// Which rule — one of [`RULES`].
    pub rule: &'static str,
    /// The offender, named the way its own source names it.
    pub offender: String,
    /// What is wrong, with the numbers that make it actionable.
    pub detail: String,
}

impl Finding {
    /// The identity of a defect without its numbers — what the docket is
    /// keyed on, so a retune that moves a violation by a millimetre does
    /// not read as a new one.
    #[must_use]
    pub fn key(&self) -> (String, &'static str, String) {
        (self.room.clone(), self.rule, self.offender.clone())
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} {} [{}]",
            self.room, self.offender, self.detail, self.rule
        )
    }
}

/// A world-axis box. Everything the gauntlet measures is one: the lattice
/// only ever turns a room by quarter turns, and a quarter turn carries an
/// axis-aligned body onto an axis-aligned body.
#[derive(Clone, Copy, Debug)]
pub struct Box3 {
    pub lo: Vec3,
    pub hi: Vec3,
}

impl Box3 {
    /// The box a transform gives a unit body, exactly.
    fn of(at: &Transform, unit: Vec3) -> Self {
        let half = at.scale * unit;
        let m = Mat3::from_quat(at.rotation);
        let reach = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
        Self {
            lo: at.translation - reach,
            hi: at.translation + reach,
        }
    }

    /// A box from two corners in any order.
    #[must_use]
    pub const fn spanning(a: Vec3, b: Vec3) -> Self {
        Self {
            lo: Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            hi: Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    /// Grown along one axis-aligned direction.
    fn reaching(self, dir: Vec3, near: f32, far: f32) -> Self {
        let a = self.lo + dir * near;
        let b = self.hi + dir * near;
        let c = self.lo + dir * far;
        let d = self.hi + dir * far;
        Self {
            lo: a.min(b).min(c).min(d),
            hi: a.max(b).max(c).max(d),
        }
    }

    fn span(self) -> Vec3 {
        (self.hi - self.lo).max(Vec3::ZERO)
    }

    fn volume(self) -> f32 {
        let s = self.span();
        s.x * s.y * s.z
    }

    /// How far apart two boxes are: the widest gap on any one axis, and
    /// zero or less where they touch or overlap on all three. A joint is
    /// exactly this reading at zero.
    fn apart(self, other: Self) -> f32 {
        (self.lo - other.hi).max(other.lo - self.hi).max_element()
    }

    /// The shared volume of two boxes, empty where they only touch.
    fn meet(self, other: Self) -> Self {
        Self {
            lo: self.lo.max(other.lo),
            hi: self.hi.min(other.hi),
        }
    }

    /// Whether this box genuinely stands in `other` — [`CLIP_SLACK`] of
    /// overlap on every axis, and [`CLIP_BITE`] of the smaller body eaten.
    fn clips(self, other: Self) -> Option<Vec3> {
        let meet = self.meet(other);
        let span = meet.span();
        if span.min_element() <= CLIP_SLACK {
            return None;
        }
        let smaller = self.volume().min(other.volume()).max(f32::EPSILON);
        (meet.volume() / smaller > CLIP_BITE).then_some(span)
    }
}

// ------------------------------------------------------------ the roster --

/// One room, staged for judgement: the ship it hangs on, the cargo
/// standing in it, and whose room it is.
pub struct Stage {
    /// The name a finding is filed under.
    pub name: String,
    /// The room itself, placed.
    pub placed: Placed,
    /// Every room of the staged ship — the walk envelope and the shells
    /// both need the neighbours.
    pub all: Vec<Placed>,
    /// The graph, for the placement arbiter.
    pub rooms: Rooms,
    /// The loaded board: cargo standing in every room aboard.
    pub cargo: Vec<Piece>,
}

/// The name a host files its findings under.
#[must_use]
pub const fn name_of(host: Host) -> &'static str {
    match host {
        Host::Venus => "Venus",
        Host::Earth => "Earth",
        Host::Mars => "Mars",
        Host::Jupiter => "Jupiter",
        Host::Uranus => "Uranus",
        Host::Neptune => "Neptune",
        Host::Guild => "Guild",
        Host::Saturn => "Saturn",
        Host::Umbra => "Umbra",
        Host::Hermitage => "Hermitage",
        Host::Comet => "Comet",
        Host::Wanderer => "Wanderer",
        Host::Wreck => "Wreck",
        Host::Parlor => "Parlor",
        Host::Pump => "Pump",
    }
}

/// Which room kind a host's character dresses.
const fn serves(host: Host) -> RoomKind {
    match host {
        Host::Wreck => RoomKind::Wreck,
        Host::Parlor => RoomKind::Parlor,
        Host::Pump => RoomKind::Pump,
        _ => RoomKind::Trade,
    }
}

/// **Every room the game has**, staged and loaded.
///
/// The graph is built rather than parsed, and built the way the game
/// builds one: a yard-fresh ship (cabin plus burner) with the caller
/// mated at the first door the spawn walk takes. That gets every station
/// and every event room onto the same deterministic pose without a seed
/// sweep, which matters because a sweep that took a minute would not be
/// run — and the fittings are measured in the room's own frame anyway, so
/// the pose it lands at is not what any of these rules are about.
#[must_use]
pub fn roster() -> Vec<Stage> {
    let mut stages = Vec::new();
    // The ship's own two rooms first: they wear the neutral character, so
    // they are the control arm — a rule that fires on the cabin is a rule
    // with a bug in it, not a cabin with a defect in it.
    let mut ship = Rooms::new();
    let cargo = load(&ship);
    let all: Vec<Placed> = ship
        .iter()
        .map(|(id, room)| room::placed(&ship, id, room))
        .collect();
    for (id, kind, name) in [
        (CABIN, RoomKind::Cabin, "cabin"),
        (1, RoomKind::Burner, "burner"),
    ] {
        let Some(room) = ship.get(id) else { continue };
        debug_assert!(room.kind == kind, "the yard-fresh ship changed shape");
        stages.push(Stage {
            name: name.to_owned(),
            placed: room::placed(&ship, id, room),
            all: all.clone(),
            rooms: ship.clone(),
            cargo: cargo.clone(),
        });
    }
    ship = Rooms::new();
    for host in poi::HOSTS {
        let mut rooms = ship.clone();
        let Ok(id) = rooms.spawn(serves(host), CABIN) else {
            continue;
        };
        let cargo = load(&rooms);
        let all: Vec<Placed> = rooms
            .iter()
            .map(|(other, room)| room::placed(&rooms, other, room))
            .collect();
        let Some(room) = rooms.get(id) else { continue };
        stages.push(Stage {
            name: name_of(host).to_owned(),
            placed: Placed {
                host: Some(host),
                ..room::placed(&rooms, id, room)
            },
            all,
            rooms,
            cargo,
        });
    }
    stages
}

/// **The loaded board**: cargo standing in every legal berth of every
/// room aboard, laid by the sim's own arbiter.
///
/// The showcase fixture was the other candidate and it was the wrong one
/// twice over. It berths cargo in two rooms — the cabin and whatever is
/// alongside — so ten of the twelve stations would still have been swept
/// empty, which is defect reason three all over again; and extending it
/// would have moved every screenshot, gauge run, and save test that reads
/// it. This synthesises instead: same arbiter, same refusals, additive,
/// and it fills a room the fixture has never seen.
///
/// It fills every cell it can rather than composing a tasteful board, and
/// that is the point: the question is not "does this look right", it is
/// "is there a cell where cargo may stand and a station's furniture is
/// already standing".
#[must_use]
pub fn load(rooms: &Rooms) -> Vec<Piece> {
    let mut cargo: Vec<Piece> = Vec::new();
    let mut next: u32 = 0;
    for (id, room) in rooms.iter() {
        let (cols, rows) = room.kind.grid();
        for y in 0..rows {
            for x in 0..cols {
                let Some(kind) = fills(rooms, &cargo, next, id, x, y) else {
                    continue;
                };
                cargo.push(Piece {
                    id: next,
                    kind,
                    variant: 0,
                    gnawed: false,
                    loc: Loc::Hold { room: id, x, y },
                });
                next += 1;
            }
        }
    }
    cargo
}

/// What the load puts on one cell: the first legal kind that carries no
/// lamp, and only then the first legal kind of any sort.
///
/// **Bodies, not lumens.** A plain sweep down `Kind::ALL` hangs a sconce
/// on every wall cell in the room, and sixty point lights in one
/// six-by-five box is not a loaded room — it is a lighting rig. It blows
/// the exposure out of every frame of the filmstrip and it overflows the
/// renderer's own cluster lists, which corrupts the lighting for a few
/// frames and hands the flicker detector a defect it invented itself.
/// The load is about what STANDS in a berth, so it prefers what stands.
///
/// Coverings are skipped outright: they answer a different arbiter and
/// lie flat on their chart, spending no air at all.
fn fills(rooms: &Rooms, cargo: &[Piece], next: u32, id: RoomId, x: u8, y: u8) -> Option<Kind> {
    let legal = |kind: &Kind| {
        !kind.covering() && placement_check(rooms, cargo, next, *kind, id, x, y).is_ok()
    };
    Kind::ALL
        .into_iter()
        .find(|kind| !burns(*kind) && legal(kind))
        .or_else(|| Kind::ALL.into_iter().find(legal))
}

/// Whether a kind lights the room it is berthed in.
const fn burns(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::WallLamp | Kind::FloorLamp | Kind::CeilingLamp | Kind::LuminousPaint
    )
}

/// The same load, applied to a real save so the walk has a world to boot
/// ([`crate::WalkMode`]): every legal berth of every room aboard filled,
/// by the same arbiter, on top of whatever the board already carried.
///
/// `None` where the base does not parse, which is the caller's cue to
/// boot the board it already had rather than invent one.
#[must_use]
pub fn loaded_save(base: &str) -> Option<String> {
    use std::fmt::Write as _;

    let sim = space_trucking::sim::Sim::from_save(base).ok()?;
    let rooms = sim.rooms();
    let mut aboard: Vec<Piece> = sim.pieces().to_vec();
    let mut next = aboard.iter().map(|piece| piece.id + 1).max().unwrap_or(0);
    let mut added: Vec<Piece> = Vec::new();
    for (id, room) in rooms.iter() {
        // The room under judgement is the one that came alongside, and
        // it is the one that gets loaded: the ship keeps whatever board
        // it arrived with, so the filmstrip shows a loaded ROOM rather
        // than a ship packed to the deckhead in every direction.
        if room.kind.riding() {
            continue;
        }
        let (cols, rows) = room.kind.grid();
        for y in 0..rows {
            for x in 0..cols {
                let Some(kind) = fills(rooms, &aboard, next, id, x, y) else {
                    continue;
                };
                let piece = Piece {
                    id: next,
                    kind,
                    variant: 0,
                    gnawed: false,
                    loc: Loc::Hold { room: id, x, y },
                };
                aboard.push(piece);
                added.push(piece);
                next += 1;
            }
        }
    }
    let mut out = String::new();
    for line in base.lines() {
        if line.starts_with("next_piece") {
            for piece in &added {
                let Loc::Hold { room, x, y } = piece.loc else {
                    continue;
                };
                // Writing into a String cannot fail; `save.rs`'s own
                // convention drops the plumbing.
                let _ = writeln!(
                    out,
                    "piece {} {} 0 0 hold {room} {x} {y}",
                    piece.id,
                    piece.kind.index()
                );
            }
            let _ = writeln!(out, "next_piece {next}");
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

// ------------------------------------------------------------- the berths --

/// One berth: a cell of a room's net cargo may legally take, and the air
/// the biggest rig that may take it actually spends.
///
/// The air is the CELL's own column, not the rig's whole body: a standing
/// rig keeps its bas-relief depth and reaches past its own footprint into
/// the aisle in front of it, and a fitting in that aisle is a composition
/// note rather than a clip. What a berth owns is its own cell, floor to
/// the top of the tallest thing that may stand on it.
#[derive(Clone, Copy, Debug)]
pub struct Berth {
    pub cell: (u8, u8),
    pub station: Station,
    /// What the cell reads as. Carried on the berth rather than looked up
    /// again, because two rules turn on it and a rule that re-derived a
    /// class could rule about a different cell than the one it measured.
    pub class: Tile,
    /// The cell's own face on its chart.
    pub face: Box3,
    /// The face plus the air a rig fills, out into the room.
    pub air: Box3,
    /// Which way the room is, from this berth.
    pub inward: Vec3,
    /// The kind whose body sets the depth — the tallest thing that stands
    /// here, or the deepest thing that hangs here.
    pub by: Kind,
}

/// How much air one berthed rig spends off its own chart, in metres —
/// how far its body REACHES into the room, measured from the chart's own
/// plane.
///
/// Derived rather than restated: [`crate::pieces::berth_box`] poses the
/// body through the very function the runtime poses it with, and this is
/// that box measured against the plane it is berthed on. On a deck or
/// under a ceiling it comes out as the rig's HEIGHT (a standing rig
/// keeps its footprint's depth, the bas-relief inheritance from BAY.md);
/// on a wall it comes out as how far into the room the band every kind
/// is drawn within reaches.
///
/// **A reach and not a thickness.** The band begins just BEHIND the
/// berth plane (`pieces::RIG_NEAR`), so a wall rig's box straddles its
/// own chart: the box's thickness laid off the cell face would be the
/// right depth in the wrong place, claiming a rig's near face worth of
/// room no body ever reaches into and forgetting the same depth of wall
/// every body sinks into. A berth's air is what a rig fills OUT INTO THE
/// ROOM ([`Berth::air`]), which is the far reach. On a deck or under a
/// ceiling the two readings agree — a standing rig's box begins on the
/// plane it stands on — which is why only the walls ever carried the
/// error.
fn rig_air(
    charts: &[(Station, SimSurface)],
    kind: Kind,
    rect: layout::Rect,
    plane: Vec3,
    inward: Vec3,
) -> Option<f32> {
    let (lo, hi) = crate::pieces::berth_box(charts, kind, rect)?;
    Some((lo - plane).dot(inward).max((hi - plane).dot(inward)))
}

/// Every berth of one placed room, with the air each one spends.
#[must_use]
pub fn berths(rooms: &Rooms, placed: &Placed) -> Vec<Berth> {
    let (cols, rows) = placed.kind.grid();
    let mut deepest: BTreeMap<(u8, u8), (f32, Kind)> = BTreeMap::new();
    for kind in Kind::ALL {
        if kind.covering() {
            continue;
        }
        for y in 0..rows {
            for x in 0..cols {
                if placement_check(rooms, &[], u32::MAX, kind, placed.id, x, y).is_err() {
                    continue;
                }
                let Some((w, h)) = plan(placed.kind, kind, x, y) else {
                    continue;
                };
                let anchor = layout::cell_rect(placed.id, x, y);
                let rect = layout::Rect::new(
                    anchor.x,
                    anchor.y,
                    f32::from(w) * layout::CELL,
                    f32::from(h) * layout::CELL,
                );
                for j in 0..h {
                    for i in 0..w {
                        let cell = (x + i, y + j);
                        let Some((station, surface)) = chart_of(placed, cell) else {
                            continue;
                        };
                        let Some(air) = rig_air(
                            &placed.charts,
                            kind,
                            rect,
                            surface.center,
                            station.inward(&surface),
                        ) else {
                            continue;
                        };
                        let slot = deepest.entry(cell).or_insert((0.0, kind));
                        if air > slot.0 {
                            *slot = (air, kind);
                        }
                    }
                }
            }
        }
    }
    deepest
        .into_iter()
        .filter_map(|(cell, (air, by))| {
            let (station, surface) = chart_of(placed, cell)?;
            let face = cell_face(placed.id, cell, &surface);
            let inward = station.inward(&surface);
            Some(Berth {
                cell,
                station,
                class: placed.kind.tile_of(cell.0, cell.1)?,
                face,
                air: face.reaching(inward, 0.0, air),
                inward,
                by,
            })
        })
        .collect()
}

/// Which of a room's six charts a net cell reads through, and the chart
/// itself. `None` off the net — a hole, or a fixture's own socket.
fn chart_of(placed: &Placed, (x, y): (u8, u8)) -> Option<(Station, SimSurface)> {
    let (cols, rows) = placed.kind.grid();
    if x >= cols || y >= rows || placed.kind.surface_of(x, y).is_none() {
        return None;
    }
    let cell = layout::cell_rect(placed.id, x, y);
    let mid =
        space_trucking::sim::Vec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
    placed
        .charts
        .iter()
        .find(|(_, surface)| surface.rect.contains(mid))
        .map(|(station, surface)| (*station, *surface))
}

/// One net cell's own face on its chart, in the world.
fn cell_face(id: RoomId, (x, y): (u8, u8), surface: &SimSurface) -> Box3 {
    plan_face(surface, layout::cell_rect(id, x, y))
}

/// **The ground a footprint owns**, in the world: the sim rect a berth
/// spends, read onto the chart it spends it on. A cell's own face is the
/// one-cell case ([`cell_face`]); a two-across kind's is the same box
/// twice as wide, and it is the box the drawing has to land on.
fn plan_face(surface: &SimSurface, rect: layout::Rect) -> Box3 {
    let a = surface.to_world(space_trucking::sim::Vec2::new(rect.x, rect.y));
    let b = surface.to_world(space_trucking::sim::Vec2::new(
        rect.x + rect.w,
        rect.y + rect.h,
    ));
    Box3::spanning(a, b)
}

// ---------------------------------------------------------- the fittings --

/// One thing a room draws, with the box it fills and the name its own
/// source calls it by.
#[derive(Clone, Debug)]
pub struct Drawn {
    pub what: String,
    pub body: Box3,
    /// Which of the box's six sides are faces the renderer draws.
    pub faces: Faces,
    /// **Whether somebody hung it** — a station's fitting, or a room's
    /// own doorway hardware — as against the fabric it hangs on. The
    /// rules that are about furniture ask only about these; the rest is
    /// shells and paint, and paint is the decal ladder's business.
    pub character: bool,
    /// What this body is called where something else is bolted to it —
    /// a station's `Fitting::called`, a doorway part's own `what`.
    /// `None` for the great majority, which nothing hangs off.
    pub name: Option<String>,
    /// **What it claims holds it up**, resolved into the world.
    pub seat: Option<Claim>,
}

/// **One hung body's claim about what holds it up**, in world terms.
///
/// A station writes it as a side of the room's box (`poi::Seat`) and a
/// doorway writes it as a world plane (`room::Seat`), because those are
/// the frames the two are composed in; by the time the sweep sees them
/// they are one thing. What matters is that both were *declared* — the
/// sweep never decides that two bodies near each other were probably
/// meant to touch.
#[derive(Clone, Debug)]
pub enum Claim {
    /// A plane the body has to reach: what the surface is called, a
    /// point on it, and the way the body reaches along to meet it.
    Plane(&'static str, Vec3, Vec3),
    /// Another hung body in the same room, by the name it declares.
    /// Several may answer, and meeting any of them is meeting the seat.
    On(String),
}

/// **Which sides of a body's box are faces**, per world axis and end.
///
/// Everything the gauntlet measures is a box, and for a box that is the
/// body. For everything else the box is a wrapper: a cylinder meets each
/// of the four planes round its flank along a single LINE, a sphere meets
/// all six at a point, and a torus at a curve. A line is not a face and
/// cannot fight one — which is why two bodies cut from one centre, a
/// collar round a pipe or a boss on a post, share a box and nothing else.
/// A detector that could not be told so is a detector somebody loosens
/// the threshold on, and a loosened threshold stops finding the thing it
/// was built for.
///
/// A flat paint is the other case: it is a face on a chart, and its mesh
/// is a box only because the renderer draws boxes. What the decal ladder
/// puts on a rung is the side it shows the room; the other five are the
/// wrapper's, and the one behind it lies inside the ladder's own band,
/// hard against the surface the paint is painted on.
#[derive(Clone, Copy, Debug)]
pub struct Faces {
    /// Whether the low side of each world axis is a face.
    lo: BVec3,
    /// Whether the high side of each world axis is a face.
    hi: BVec3,
}

/// Which world axis a direction lies squarely along, if any. A hair off
/// is not along it: a plane tilted by a degree is a plane the depth
/// buffer can tell apart, and calling it axis-aligned is how a detector
/// invents a fight nobody can see.
fn square(dir: Vec3) -> BVec3 {
    dir.abs().cmpgt(Vec3::splat(0.999))
}

impl Faces {
    /// Six of them: what a box has.
    const ALL: Self = Self {
        lo: BVec3::TRUE,
        hi: BVec3::TRUE,
    };

    /// The faces one silhouette actually presents, in world axes.
    ///
    /// A cylinder and a tapered drum are capped at both ends of their own
    /// `+y` and round everywhere else; a sphere and a torus are round
    /// everywhere.
    ///
    /// **A flat side is a flat side of the BOX only where the body's own
    /// axis lands squarely on a world one.** The lattice and the charts
    /// only ever turn by quarter turns, so for a room every side either
    /// lands or is not there. A rig's own parts do not: the perfume
    /// vial's flask stands on its corner and the gas canister's chevrons
    /// lean, and a leaning box's box has six sides that the box itself
    /// does not draw. Whichever axis a body IS square on — a chevron
    /// leaning about `z` keeps its two `z` ends — is a face; the rest is
    /// wrapper.
    ///
    /// The drum's narrow cap is the one place this reads wide: the top of
    /// a `Cone` is a disc a third of its box across (and a cargo rig's
    /// cone tapers all the way to a point), and the box is what the
    /// footprint is measured over. That errs toward reporting, which is
    /// the direction a detector is allowed to err in.
    fn of(shape: Shape, rot: Quat) -> Self {
        let own = match shape {
            Shape::Slab => Vec3::ONE,
            Shape::Post | Shape::Cone => Vec3::Y,
            Shape::Dome | Shape::Ring => Vec3::ZERO,
        };
        let m = Mat3::from_quat(rot);
        let mut flat = BVec3::FALSE;
        for (axis, mine) in [(m.x_axis, own.x), (m.y_axis, own.y), (m.z_axis, own.z)] {
            if mine > 0.5 {
                flat |= square(axis);
            }
        }
        Self { lo: flat, hi: flat }
    }

    /// The one face a flat paint shows: the way it looks off its chart.
    fn showing(dir: Vec3) -> Self {
        let axis = square(dir);
        Self {
            lo: axis & dir.cmplt(Vec3::ZERO),
            hi: axis & dir.cmpgt(Vec3::ZERO),
        }
    }

    /// Whether one end of one world axis is a drawn face.
    fn has(self, axis: usize, up: bool) -> bool {
        if up { self.hi } else { self.lo }.test(axis)
    }
}

/// One posed fitting, as the sweep measures it: its box, and the sides of
/// that box its own silhouette actually draws.
fn drawn(what: String, frame: &Frame, fitting: &Fitting) -> Drawn {
    Drawn {
        what,
        // What the unit body fills of the frame it is scaled into —
        // half a unit on each axis it fills whole, and the torus's tube
        // on the one it does not. This used to carry that exception as
        // a table of its own, which is half of why five hoops could not
        // be set into a deck: the sweep knew about the tube and the
        // containment law was reading the frame.
        body: Box3::of(&frame.place(fitting), fitting.shape.fill() * 0.5),
        faces: Faces::of(fitting.shape, frame.rot),
        character: true,
        name: fitting.name.map(str::to_owned),
        // A face of the host box is resolved through the very frame the
        // runtime poses the fitting with, so a claim and a body cannot
        // be measured off two different rooms.
        seat: fitting.seat.map(|seat| match seat {
            poi::Seat::Face(face) => {
                let (at, toward) = frame.plane(face);
                Claim::Plane(face.name(), at, toward)
            }
            poi::Seat::On(name) => Claim::On(name.to_owned()),
        }),
    }
}

/// Every fitting a station hangs INSIDE its room, posed through the very
/// frames the runtime poses them with.
///
/// The exterior dressing is deliberately absent: it hangs in the void by
/// law, and `poi::tests::a_character_stays_inside_the_room_it_dresses`
/// already holds it there. Nothing that stands outside a room can stand
/// in one of its berths.
#[must_use]
pub fn fittings(placed: &Placed) -> Vec<Drawn> {
    let character = poi::character_of(placed.host);
    let mut out = Vec::new();
    let frame = Frame::of(placed.lo, placed.hi, placed.yaw);
    for (i, fitting) in character.decor.iter().enumerate() {
        out.push(drawn(
            format!("decor[{i}] {:?}", fitting.shape),
            &frame,
            fitting,
        ));
    }
    if let Some(cell_frame) = handshake_frame(placed) {
        let knob = Fitting::new(
            character.handshake.knob,
            character.handshake.knob_coat,
            character.handshake.knob_at,
            character.handshake.knob_half,
        );
        out.push(drawn(
            format!("handshake knob {:?}", knob.shape),
            &cell_frame,
            &knob,
        ));
        for (i, fitting) in character.handshake.trim.iter().enumerate() {
            out.push(drawn(
                format!("handshake trim[{i}] {:?}", fitting.shape),
                &cell_frame,
                fitting,
            ));
        }
    }
    if !placed.kind.riding() {
        let (at, _) = room::caller_reach(placed);
        let cage = Frame {
            mid: at,
            half: Vec3::splat(room::SHADE_R),
            rot: Quat::IDENTITY,
        };
        for (i, fitting) in character.light.cage.iter().enumerate() {
            out.push(drawn(
                format!("lamp cage[{i}] {:?}", fitting.shape),
                &cage,
                fitting,
            ));
        }
    }
    out
}

/// The handshake's own cell frame: x and y are cell fractions, z is
/// metres out of the wall (`poi::Fitting`). `None` where the kind
/// declares no fixture.
fn handshake_frame(placed: &Placed) -> Option<Frame> {
    let (hx, hy) = placed.kind.handshake()?;
    let (station, surface) = chart_of_raw(placed, (hx, hy))?;
    let cell = layout::cell_rect(placed.id, hx, hy);
    let mid =
        space_trucking::sim::Vec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
    Some(Frame {
        mid: surface.to_world(mid),
        half: Vec3::new(
            cell.w * surface.scale_u() * 0.5,
            cell.h * surface.scale_v() * 0.5,
            1.0,
        ),
        rot: station.face(&surface),
    })
}

/// [`chart_of`] without the net's own veto — the handshake's cell is
/// deliberately a hole in [`RoomKind::surface_of`] (it is the fixture's
/// socket, not a berth), and the fixture still has to be measured off it.
fn chart_of_raw(placed: &Placed, (x, y): (u8, u8)) -> Option<(Station, SimSurface)> {
    let cell = layout::cell_rect(placed.id, x, y);
    let mid =
        space_trucking::sim::Vec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
    placed
        .charts
        .iter()
        .find(|(_, surface)| surface.rect.contains(mid))
        .map(|(station, surface)| (*station, *surface))
}

/// **Everything a doorway is made of**, as the sweep measures it, with
/// the room on the far side of each part's seam where it dresses one.
///
/// The bodies come out of [`room::seam_parts`] — the description the
/// runtime builds a doorway from — so the harness and the world cannot
/// disagree about what a doorway has in it.
fn seam_furniture(placed: &Placed) -> Vec<(Option<RoomId>, Drawn)> {
    room::seam_parts(placed)
        .into_iter()
        .map(|part| {
            (
                part.across,
                Drawn {
                    body: Box3::of(&part.at, Vec3::splat(0.5)),
                    // A tread is paint on a rung of the decal ladder and
                    // shows the one side it turns to the room. Everything
                    // else in a doorway is hardware, and hardware has six
                    // sides.
                    faces: if part.dress.paint() {
                        Faces::showing(part.at.rotation * Vec3::Z)
                    } else {
                        Faces::of(Shape::Slab, part.at.rotation)
                    },
                    character: true,
                    // A doorway's parts answer to their own names, and a
                    // body drawn several times over answers to the name
                    // it shares (`room::Seat::On`).
                    name: Some(part.what.clone()),
                    seat: part.seat.map(|seat| match seat {
                        room::Seat::Plane(what, at, toward) => Claim::Plane(what, at, toward),
                        room::Seat::On(name) => Claim::On(name),
                    }),
                    what: part.what,
                },
            )
        })
        .collect()
}

/// **The scene**: everything one room draws that has a body, as boxes.
///
/// The fabric half — shells, and the flat field a colored class lays over
/// its own cells — is here because the coplanar detector is about what
/// the depth buffer sees, and a fitting standing on the face a painted
/// wall shows the room fights it exactly as hard as two fittings fight
/// each other. A field carries the one face it shows ([`Faces`]); the
/// other five are its mesh's. What is deliberately left out is the rim
/// MARKS: their spacing is the decal ladder's own law and
/// `pieces::tests::the_decal_ladder_never_z_fights` already holds every
/// rung of it.
#[must_use]
pub fn scene(stage: &Stage) -> Vec<Drawn> {
    let placed = &stage.placed;
    let mut out = fittings(placed);
    if placed.id == CABIN {
        for (i, slab) in crate::rig::structure().into_iter().enumerate() {
            out.push(Drawn {
                what: format!("hull slab[{i}]"),
                body: Box3::spanning(slab.center - slab.size * 0.5, slab.center + slab.size * 0.5),
                faces: Faces::ALL,
                character: false,
                name: None,
                seat: None,
            });
        }
    } else {
        for (i, (center, size, _)) in room::shell_boxes(placed, &stage.all)
            .into_iter()
            .enumerate()
        {
            out.push(Drawn {
                what: format!("shell slab[{i}]"),
                body: Box3::spanning(center - size * 0.5, center + size * 0.5),
                faces: Faces::ALL,
                character: false,
                name: None,
                seat: None,
            });
        }
    }
    // **The seam furniture**: the stiles and lintel of a mated doorway,
    // its jamb lamp and its amber latch, the leaf and rivets of a port
    // drawn shut, a hatch's coaming, hinge and pull, and the studded
    // tread each of them lays on the deck it stands on. Posed through the
    // very description the runtime spawns them from
    // ([`room::seam_parts`]), so a doorway cannot be built out of
    // hardware this cannot be asked about.
    //
    // **A doorway is judged from both of its sides.** A seam's frame is
    // drawn once, by the room with the lower id, and it stands on the
    // boundary the two rooms share — so the room that did not draw it
    // still has a jamb standing in its own wall, and a sweep that only
    // asked what a room DREW would have swept every calling room in the
    // game past its own doorway without looking at it.
    out.extend(seam_furniture(placed).into_iter().map(|(_, drawn)| drawn));
    for other in &stage.all {
        if other.id == placed.id {
            continue;
        }
        out.extend(
            seam_furniture(other)
                .into_iter()
                .filter(|(across, _)| *across == Some(placed.id))
                .map(|(_, drawn)| drawn),
        );
    }
    // The pendant, which every calling room hangs and no station may
    // move: stem, shade, glass. Its cage is a character's and came in
    // with the fittings.
    if !placed.kind.riding() {
        let (at, _) = room::caller_reach(placed);
        let stem = (placed.hi.y - at.y) * 0.5;
        for (what, shape, centre, size) in [
            (
                "lamp stem",
                Shape::Slab,
                Vec3::new(at.x, at.y + stem, at.z),
                Vec3::new(room::STEM_T, stem * 2.0, room::STEM_T),
            ),
            (
                "lamp shade",
                poi::character_of(placed.host).light.shade,
                Vec3::new(at.x, room::SHADE_H.mul_add(0.5, at.y), at.z),
                Vec3::new(room::SHADE_R * 2.0, room::SHADE_H, room::SHADE_R * 2.0),
            ),
            (
                "lamp glass",
                Shape::Post,
                Vec3::new(at.x, at.y - 0.01, at.z),
                Vec3::new(
                    room::SHADE_R * room::GLASS_R * 2.0,
                    0.02,
                    room::SHADE_R * room::GLASS_R * 2.0,
                ),
            ),
        ] {
            out.push(Drawn {
                what: what.to_owned(),
                body: Box3::spanning(centre - size * 0.5, centre + size * 0.5),
                faces: Faces::of(shape, Quat::IDENTITY),
                character: false,
                name: None,
                seat: None,
            });
        }
    }
    out.extend(tile_fields(placed));
    out
}

/// **The colored classes' flat fields**, one per cell, exactly where
/// `room::tiles` lays them — the paint a fitting standing on it fights
/// exactly as hard as it fights another fitting.
fn tile_fields(placed: &Placed) -> Vec<Drawn> {
    let mut out = Vec::new();
    let (cols, rows) = placed.kind.grid();
    for y in 0..rows {
        for x in 0..cols {
            let Some(tile) = placed.kind.tile_of(x, y) else {
                continue;
            };
            let Some((station, surface)) = chart_of(placed, (x, y)) else {
                continue;
            };
            // A threshold IS the opening, a struck line is a rim mark,
            // and a fixture's cell is left bare for the room's own
            // hardware: none of the three lays a field over its own
            // cell, so none has a face for anything to fight.
            let lift = match tile {
                Tile::Threshold | Tile::Offer | Tile::Fixture => continue,
                Tile::Plain | Tile::Staging | Tile::Stock | Tile::Consume => {
                    crate::rig::layer::TILE
                }
            };
            let inward = station.inward(&surface);
            let face = cell_face(placed.id, (x, y), &surface);
            let skin = crate::rig::layer::SKIN;
            out.push(Drawn {
                what: format!("{tile:?} field ({x}, {y})"),
                body: face.reaching(inward, skin.mul_add(-0.5, lift), skin.mul_add(0.5, lift)),
                faces: Faces::showing(inward),
                character: false,
                name: None,
                seat: None,
            });
        }
    }
    out
}

// -------------------------------------------------------------- the sweep --

/// **The fabric of one room**: the bodies that ARE the room, split into
/// the two things a room is made of.
///
/// The **shell** is what the room is *constructed of* — its deck, its
/// deckhead, its four walls, and the passages its doorways run through
/// the padding to reach. The **hardware** is what is *bolted into* an
/// opening: a frame, a jamb lamp, a latch, a leaf drawn shut and its
/// rivets, a hatch's coaming, hinge and pull. `true` marks the shell.
///
/// A station's furniture, the cargo, and the pendant are none of it.
/// They are measured off boxes rather than off cells and they are
/// somebody's composition; the other seven families ask about those.
fn fabric(stage: &Stage) -> Vec<(String, Box3, bool)> {
    let placed = &stage.placed;
    let mut out: Vec<(String, Box3, bool)> = if placed.id == CABIN {
        crate::rig::structure()
            .into_iter()
            .enumerate()
            .map(|(i, slab)| {
                (
                    format!("hull slab[{i}]"),
                    Box3::spanning(slab.center - slab.size * 0.5, slab.center + slab.size * 0.5),
                    true,
                )
            })
            .collect()
    } else {
        room::shell_boxes(placed, &stage.all)
            .into_iter()
            .enumerate()
            .map(|(i, (center, size, _))| {
                (
                    format!("shell slab[{i}]"),
                    Box3::spanning(center - size * 0.5, center + size * 0.5),
                    true,
                )
            })
            .collect()
    };
    out.extend(
        room::seam_parts(placed)
            .into_iter()
            // Paint rides the decal ladder, whose rungs are millimetres
            // by law and are held by the ladder's own test. A tread is
            // not architecture; it is a mark on architecture.
            .filter(|part| !part.dress.paint())
            .map(|part| {
                let shell = part.what.contains("passage");
                (
                    part.what.clone(),
                    Box3::of(&part.at, Vec3::splat(0.5)),
                    shell,
                )
            }),
    );
    out
}

/// The box each of a room's declared ports owns: the aperture's own
/// cells, a couple of walls of slack round them for a frame to straddle
/// the edge with, and — where the port is mated — the padding cube the
/// passage runs through.
fn seam_boxes(placed: &Placed) -> Vec<Box3> {
    placed
        .ports
        .iter()
        .filter(|site| site.half_a.length() > 0.0)
        .map(|site| {
            let across = if site.mate.is_some() && site.is_door() {
                room::PAD_M * 0.5
            } else {
                0.0
            };
            let mid = site.leaf + site.out * across;
            let half = site.half_a.abs()
                + site.half_b.abs()
                + Vec3::splat(room::WALL_T * 2.0)
                + site.out.abs() * across;
            Box3::spanning(mid - half, mid + half)
        })
        .collect()
}

/// **The world is built of the cargo grid and aligned to it.**
///
/// The owner's rule, swept. Four passes of this project fixed instances
/// of bad placement and none of them touched the class, and the class
/// was that the fabric was only *mostly* on the lattice: the grid
/// governed a room's plan and stopped at its section, a wall's thickness
/// and a chart's trim were numbers chosen by eye, and the cabin's own
/// hull was measured by hand two centimetres off its own box. None of
/// those is visible in a screenshot. Every one of them is visible to the
/// next thing that has to line up against it, which is how a doorway
/// comes to leave a sliver of daylight and a passage comes to butt
/// against nothing.
///
/// Two clauses, and they are the two questions a player asks:
///
/// - **Does it stand where it belongs?** A shell body stands inside its
///   own room's cells, grown by the one wall its fabric may be proud by;
///   a doorway's hardware stands in a doorway. Nothing is anywhere else.
///   This is the clause the incinerator failed for three reports.
/// - **Does the shell land on the grid?** Every face of every shell body
///   is a whole number of [`GRID_STEP`]s from the lattice — cells across
///   the plan, cells up from the room's own deck.
///
/// **The second clause is spent on the shell and not on the hardware,
/// and that is the exemption, named.** A room is constructed of its
/// deck, its deckhead, its walls and its passages, and those are the
/// bodies everything else has to line up against. A hinge barrel, a
/// rivet head and a twelve-millimetre coaming rim are bolted to the
/// construction rather than part of it; holding those to a thirty-four
/// millimetre notch would be the grid deciding what things look like
/// instead of where they are, which the decree does not ask for and the
/// art direction forbids. They are held by the first clause, by
/// `a_doorway_draws_each_body_once`, and by the coplanar detector.
///
/// The other named exemption is **paint** — treads, sills, tile fields,
/// every rung of `rig::layer` — which is sub-cell by definition and has
/// a law of its own (`pieces::tests::the_decal_ladder_never_z_fights`).
fn grid_fits(stage: &Stage) -> Vec<Finding> {
    let placed = &stage.placed;
    let wall = Vec3::splat(room::WALL_T);
    let own = Box3::spanning(placed.lo - wall, placed.hi + wall);
    let seams = seam_boxes(placed);
    let mut out = Vec::new();
    for (what, body, shell) in fabric(stage) {
        // A shell body may be anywhere in its own room or in one of its
        // own doorways; a doorway's hardware may only be in a doorway.
        let home = if shell { Some(own) } else { None };
        if !home.is_some_and(|home| holds(home, body))
            && !seams.iter().any(|seam| holds(*seam, body))
        {
            let outer = home.unwrap_or(own);
            let outside = (outer.lo - body.lo).max(body.hi - outer.hi).max(Vec3::ZERO);
            out.push(Finding {
                room: stage.name.clone(),
                rule: GRID_FITS,
                offender: what.clone(),
                detail: format!(
                    "stands {:.4}x{:.4}x{:.4} m outside {}",
                    outside.x,
                    outside.y,
                    outside.z,
                    if shell {
                        "the cells its own room was given, and in no doorway"
                    } else {
                        "every doorway this room declares"
                    }
                ),
            });
        }
        if !shell {
            continue;
        }
        // Cells run from the lattice across the plan and from the room's
        // own deck upward, so the deck is the origin a height is counted
        // from.
        let origin = Vec3::new(room::ANCHOR.x, placed.lo.y, room::ANCHOR.z);
        if let Some((axis, miss, face)) = off_grid(body, origin) {
            out.push(Finding {
                room: stage.name.clone(),
                rule: GRID_FITS,
                offender: what,
                detail: format!(
                    "has a face at {face:.4} m on the {} axis, {miss:.4} m off the nearest \
                     sixteenth of a cell",
                    ["x", "y", "z"][axis]
                ),
            });
        }
    }
    out
}

/// Whether `body` lies wholly within `outer`.
fn holds(outer: Box3, body: Box3) -> bool {
    (0..3).all(|axis| {
        body.lo[axis] >= outer.lo[axis] - GRID_EPS && body.hi[axis] <= outer.hi[axis] + GRID_EPS
    })
}

/// The worst face of `body` that does not land on the grid measured from
/// `origin`, as `(axis, miss in metres, where the face is)`.
fn off_grid(body: Box3, origin: Vec3) -> Option<(usize, f32, f32)> {
    let mut worst = (0_usize, 0.0_f32, 0.0_f32);
    for axis in 0..3 {
        for face in [body.lo[axis], body.hi[axis]] {
            let steps = (face - origin[axis]) / GRID_STEP;
            let miss = (steps - steps.round()).abs() * GRID_STEP;
            if miss > worst.1 {
                worst = (axis, miss, face);
            }
        }
    }
    (worst.1 > GRID_EPS).then_some(worst)
}

/// **The whole gauntlet's pure half**: room by room, then kind by kind,
/// rule by rule.
#[must_use]
pub fn sweep() -> Vec<Finding> {
    sweep_dressed(crate::art::Dressings::shipped())
}

/// **The same sweep, told what is dressed in purchased art.**
///
/// [`sweep`] hands it `art/manifest.toml` as it stands in this
/// repository, declared bindings and all — a dressed kind is swept as
/// the body its declaration promises, in place of its whitebox parts.
/// The parameter exists because a family nobody can catch out is
/// a green tick that means nothing: every reading here was factored out
/// of its own loop for exactly that reason (see [`off_plan`] and
/// [`looked_at`]), and a fill declaration is the newest thing the sweep
/// takes at its word.
#[must_use]
pub fn sweep_dressed(declared: &Dressings) -> Vec<Finding> {
    let mut out = Vec::new();
    // The roster is held rather than consumed: `berth_filled` answers
    // about a kind and a chart class rather than about a room, so it
    // reads every room's charts at once and files one finding for a
    // defect all fifteen of them share.
    let stages = roster();
    for stage in &stages {
        out.extend(berth_clear(stage));
        out.extend(berth_seen(stage));
        out.extend(berth_reached(stage));
        out.extend(coplanar(stage));
        out.extend(walk_clear(stage));
        out.extend(grid_fits(stage));
        out.extend(furniture_seated(stage));
        out.extend(fixture_reached(stage));
        out.extend(fixture_seen(stage));
    }
    out.extend(prop_points(declared));
    out.extend(part_seated(declared));
    out.extend(rig_coplanar(declared));
    out.extend(rig_fits(declared));
    out.extend(rig_faces(declared));
    out.extend(rig_seated(declared));
    out.extend(berth_filled(&stages));
    out.extend(berth_turned(&stages));
    out.extend(deck_reached());
    out.sort();
    out.dedup();
    out
}

/// A list of cells, capped so a finding stays one readable line. The
/// count is exact; the enumeration is a sample, because a fixer who has
/// been handed eight cells and a total does not need the other twenty.
fn some_cells(cells: &[(u8, u8)]) -> String {
    const SHOWN: usize = 8;
    let head: Vec<String> = cells
        .iter()
        .take(SHOWN)
        .map(|(x, y)| format!("({x}, {y})"))
        .collect();
    if cells.len() > SHOWN {
        format!("{} and {} more", head.join(", "), cells.len() - SHOWN)
    } else {
        head.join(", ")
    }
}

/// **Which berths a room owes cargo air in**: everything but staging.
///
/// The line the owner drew. A staging cell is the room's own deck lent
/// to the player between one launch and the next — nothing stays there,
/// the launch gate empties it, and a crate that clips a station's bollard
/// while it waits is a clipping incident nobody minds. Every other class
/// is a promise about where cargo *lives*: the cabin's and the burner's
/// plain deck (cargo stays there), a room's `Stock` (its goods stand
/// there), a chalked `Offer` (a proposal stands there and must be read as
/// a proposal), and the hopper's `Consume`.
///
/// `Threshold` and `Fixture` are not berths at all — the arbiter refuses
/// them, so [`berths`] never produces one — and they stay defended by the
/// rule that already defends them.
fn kept(berth: &Berth) -> bool {
    berth.class != Tile::Staging
}

/// What stands in a berth on the loaded board, if anything — so a finding
/// can name the crate a fitting is standing inside of, not merely the
/// cell it could stand in.
fn standing(stage: &Stage, cell: (u8, u8)) -> Option<String> {
    stage
        .cargo
        .iter()
        .find(|piece| {
            matches!(piece.loc, Loc::Hold { room, x, y }
                if room == stage.placed.id && x == cell.0 && y == cell.1)
        })
        .map(|piece| format!("{:?} #{}", piece.kind, piece.id))
}

/// **No decor stands where cargo stays.** The assertion the containment
/// test never made: a fitting inside the room's box is not the same claim
/// as a fitting outside every berth in it.
///
/// **It asks about staying berths and the trade surface, and not about
/// staging** ([`kept`]). A room that leaves owns no volume of its own,
/// which is why every station's furniture used to be a defect somewhere:
/// a trading console clipped the cell it stood on, and there was no cell
/// in the game it could have stood on instead. Now there is a class for
/// the deck a room keeps — cargo may be set down in it, a bollard may
/// stand in it, and if the two clip, the owner's ruling is that this is
/// not a defect. What is left on the list is the honest half: a fitting
/// biting the room's own goods, a proposal's chalk, a doorway, or the
/// counter's own cell still has to move.
fn berth_clear(stage: &Stage) -> Vec<Finding> {
    let berths: Vec<Berth> = berths(&stage.rooms, &stage.placed)
        .into_iter()
        .filter(kept)
        .collect();
    let mut out = Vec::new();
    for fitting in fittings(&stage.placed) {
        // One line per offender, not one per cell: a bar standing across
        // six berths is one thing to move, and six lines that say so are
        // a work order somebody has to summarise before they can start.
        let mut hits: Vec<(&Berth, Vec3)> = berths
            .iter()
            .filter_map(|berth| fitting.body.clips(berth.air).map(|span| (berth, span)))
            .collect();
        if hits.is_empty() {
            continue;
        }
        hits.sort_by(|a, b| b.1.min_element().total_cmp(&a.1.min_element()));
        let cells: Vec<(u8, u8)> = hits.iter().map(|(berth, _)| berth.cell).collect();
        let standing_on = hits
            .iter()
            .filter(|(berth, _)| matches!(berth.station, Station::BayFloor | Station::BayCeiling))
            .count();
        let (worst, span) = hits[0];
        let (x, y) = worst.cell;
        let held = standing(stage, worst.cell).unwrap_or_else(|| format!("{:?}", worst.by));
        out.push(Finding {
            room: stage.name.clone(),
            rule: BERTH_CLEAR,
            offender: fitting.what.clone(),
            detail: format!(
                "occupies {} berth(s) cargo may take ({standing_on} of them deck or \
                 ceiling): {}; worst is ({x}, {y}) on {:?}, where {held} stands, by \
                 {:.3}x{:.3}x{:.3} m",
                hits.len(),
                some_cells(&cells),
                worst.station,
                span.x,
                span.y,
                span.z
            ),
        });
    }
    out
}

/// **Nothing occludes a wall berth that keeps its cargo.** A fitting
/// standing in front of a wall cell hides whatever hangs there, which is
/// a defect the containment law cannot express and a screenshot taken
/// from the side cannot show.
///
/// Narrowed with [`BERTH_CLEAR`] and for the same reason, which on a wall
/// is the *identical* reason: a fitting inside a wall berth's air is by
/// construction standing between that wall and the room, so a rule that
/// allowed the clip and forbade the occlusion would forbid nothing and
/// merely say so twice.
fn berth_seen(stage: &Stage) -> Vec<Finding> {
    let berths: Vec<Berth> = berths(&stage.rooms, &stage.placed)
        .into_iter()
        .filter(kept)
        .collect();
    let mut out = Vec::new();
    for fitting in fittings(&stage.placed) {
        let mut hidden: Vec<((u8, u8), Station, f32)> = Vec::new();
        for berth in &berths {
            if matches!(berth.station, Station::BayFloor | Station::BayCeiling) {
                continue;
            }
            let air = berth.air.span().dot(berth.inward.abs());
            let sight = berth.face.reaching(berth.inward, air, air + SIGHT);
            let meet = fitting.body.meet(sight);
            let span = meet.span();
            if span.min_element() <= CLIP_SLACK {
                continue;
            }
            // How much of the cell's own face the fitting stands across.
            let flat = Vec3::ONE - berth.inward.abs();
            let cell = berth.face.span() * flat + berth.inward.abs();
            let cover = (span * flat + berth.inward.abs()) / cell;
            let cover = cover.x * cover.y * cover.z;
            if cover > OCCLUDE_BITE {
                hidden.push((berth.cell, berth.station, cover));
            }
        }
        if hidden.is_empty() {
            continue;
        }
        hidden.sort_by(|a, b| b.2.total_cmp(&a.2));
        let cells: Vec<(u8, u8)> = hidden.iter().map(|(cell, _, _)| *cell).collect();
        let (worst, station, cover) = hidden[0];
        let (x, y) = worst;
        out.push(Finding {
            room: stage.name.clone(),
            rule: BERTH_SEEN,
            offender: fitting.what.clone(),
            detail: format!(
                "stands between the room and {} wall berth(s): {}; worst hides {:.0}% \
                 of ({x}, {y}) on {station:?}",
                hidden.len(),
                some_cells(&cells),
                cover * 100.0
            ),
        });
    }
    out
}

/// Nearest positive parameter at which a segment enters a box, if any.
/// `dir` need not be normalized; parameters are in units of `dir`.
fn ray_box(origin: Vec3, dir: Vec3, body: Box3) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let (o, d) = (origin[axis], dir[axis]);
        let (a, b) = (body.lo[axis], body.hi[axis]);
        if d.abs() < 1e-9 {
            if o < a || o > b {
                return None;
            }
        } else {
            let (t1, t2) = ((a - o) / d, (b - o) / d);
            near = near.max(t1.min(t2));
            far = far.min(t1.max(t2));
        }
    }
    (far >= near && far > 0.0).then(|| near.max(0.0))
}

/// Every place a body may stand in the staged ship, on a grid.
fn stances(stage: &Stage) -> Vec<Vec3> {
    let envelope = room::walk_boxes(&stage.all);
    let mut out = Vec::new();
    for (lo, hi) in envelope.rooms.iter().chain(&envelope.seams) {
        for a in 0..=STANCES {
            for b in 0..=STANCES {
                out.push(Vec3::new(
                    (f32::from(a) / f32::from(STANCES)).mul_add(hi.x - lo.x, lo.x),
                    EYE_HEIGHT,
                    (f32::from(b) / f32::from(STANCES)).mul_add(hi.z - lo.z, lo.z),
                ));
            }
        }
    }
    out
}

/// **Every berth that keeps its cargo stays reachable.**
/// `rig::tests::every_net_cell_is_workable` is the precedent and the hull
/// was the only thing it could ask about; this asks the same question of
/// every room in the game, with a station's own furniture added to the
/// list of things that may be in the way.
///
/// Narrowed with the other two, and measurement is why. A rule that let a
/// fitting stand in a staging cell and forbade it to hide one forbids
/// standing there at all: a fitting inside a berth's air is between that
/// berth's face and every stance by construction, on a wall and on a
/// deck alike. Dead space that furniture may not be seen to occupy is not
/// dead space.
///
/// **What that would have cost, had it been a soft-lock, is nothing**,
/// and that is measured rather than hoped. The runtime's pointer is cast
/// at mapped surfaces only — the room's charts and standing pieces — and
/// a station's dressing carries none, so a fitting cannot make a berth
/// unpickable, only unlovely (`crate::surface::pick`, and the guard
/// `tests::a_stations_dressing_is_not_in_the_aiming_path`). The other
/// half, that a refusal you cannot see is a refusal you cannot obey, is
/// answered where it arises: the amber frame round detained cargo is
/// drawn with a depth bias and reads through a station's furniture
/// (`room::CLAIM_BIAS`).
fn berth_reached(stage: &Stage) -> Vec<Finding> {
    let scene = scene(stage);
    let stances = stances(stage);
    let mut blamed: BTreeMap<String, Vec<(u8, u8)>> = BTreeMap::new();
    for berth in berths(&stage.rooms, &stage.placed).into_iter().filter(kept) {
        let probe = (berth.face.lo + berth.face.hi) * 0.5 + berth.inward * 0.02;
        // The reading is [`worked`]'s, shared with `fixture-reached`: a
        // berth and a room's own counter are the same question asked of
        // two things, and one arithmetic is what keeps them one question.
        if let Err(blame) = worked(&scene, &stances, probe) {
            blamed.entry(blame).or_default().push(berth.cell);
        }
    }
    blamed
        .into_iter()
        .map(|(blame, cells)| Finding {
            room: stage.name.clone(),
            rule: BERTH_REACHED,
            offender: blame,
            detail: format!(
                "leaves {} berth(s) workable from nowhere a body may stand: {}",
                cells.len(),
                some_cells(&cells)
            ),
        })
        .collect()
}

/// **Every plane two bodies share and both look out of**, described the
/// way a fixer needs it — the detector's one arithmetic, spent by the
/// rooms and by the rigs alike.
///
/// Same-facing is the whole subtlety. Two boxes stacked touch at a plane
/// too, and that plane carries one face UP and one face DOWN — which the
/// depth buffer settles correctly every time. It is two faces looking the
/// same way from the same place that has no answer.
fn shared_faces(a: &Drawn, b: &Drawn) -> Vec<String> {
    let mut out = Vec::new();
    for axis in 0..3 {
        for (up, (pa, pb)) in [
            (false, (a.body.lo[axis], b.body.lo[axis])),
            (true, (a.body.hi[axis], b.body.hi[axis])),
        ] {
            if !a.faces.has(axis, up) || !b.faces.has(axis, up) {
                continue;
            }
            if (pa - pb).abs() >= FIGHT_EPS {
                continue;
            }
            let meet = a.body.meet(b.body).span();
            let flat: Vec<f32> = (0..3).filter(|k| *k != axis).map(|k| meet[k]).collect();
            if flat.iter().any(|span| *span <= FIGHT_FOOT) {
                continue;
            }
            let facing = if up {
                ["+x", "+y", "+z"][axis]
            } else {
                ["-x", "-y", "-z"][axis]
            };
            out.push(format!(
                "a {facing} face at {pa:.4} over {:.3}x{:.3} m",
                flat[0], flat[1]
            ));
        }
    }
    out
}

/// **No two coplanar same-facing surfaces.** The general form of the
/// detector a previous pass built by hand out of a scene dump: for every
/// pair of drawn bodies, every pair of faces that share an axis, a
/// coordinate, and a direction, and whose footprints genuinely overlap.
fn coplanar(stage: &Stage) -> Vec<Finding> {
    let scene = scene(stage);
    let mut shared: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (i, a) in scene.iter().enumerate() {
        for b in &scene[i + 1..] {
            // Fabric against fabric is the decal ladder's business, and
            // the ladder has its own test. What this is for is a
            // station's furniture — against itself, or against the room.
            if !a.character && !b.character {
                continue;
            }
            let pair = format!("{} / {}", a.what, b.what);
            if ALLOWED
                .iter()
                .any(|(room, offender, _)| *room == stage.name && *offender == pair)
            {
                continue;
            }
            let faces = shared_faces(a, b);
            if !faces.is_empty() {
                shared.entry(pair).or_default().extend(faces);
            }
        }
    }
    shared
        .into_iter()
        .map(|(pair, faces)| Finding {
            room: stage.name.clone(),
            rule: NO_COPLANAR,
            offender: pair,
            detail: format!("share {}", faces.join(" and ")),
        })
        .collect()
}

/// **A rig's features point where their names say.** The kind×chart sweep
/// asks whether a rig is turned the way its berth says; this asks whether
/// its own parts are. Both bugs the playtest found in this class — a
/// sconce lighting the wall it is bolted to, a floor lamp's base plate
/// standing on edge — hang perfectly true by the first question.
///
/// It caught them by comparing a hand-authored turn against the claim
/// written beside it, and the cure was to stop writing the turn down:
/// [`crate::pieces::Feature::turn`] derives it from the claim, so the
/// two cannot disagree. What is left for this to find is a claim that
/// points nowhere — an axis or a want somebody left degenerate — and
/// any part that goes back to being turned by hand.
fn prop_points(declared: &Dressings) -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        // A bought mesh points its own features and declares none of
        // them here; the whitebox's claims are about a body this kind no
        // longer draws.
        if dressed(declared, kind) {
            continue;
        }
        for feature in crate::pieces::features(kind) {
            let got = (feature.turn() * feature.axis).normalize_or_zero();
            let want = feature.want.normalize_or_zero();
            if got.dot(want) > 0.99 {
                continue;
            }
            out.push(Finding {
                room: RIGS.to_owned(),
                rule: PROP_POINTS,
                offender: format!("{kind:?} {}", feature.name),
                detail: format!("points {got} in its own rig; its name says {want}"),
            });
        }
    }
    out
}

// --------------------------------------------------------------- the rigs --

/// **What one cargo kind draws**, in metres, as the sweep measures it.
///
/// A rig is composed in its own local frame and berthed by a quarter
/// turn, and a quarter turn carries an axis-aligned body onto an
/// axis-aligned body — so a pair of faces that share a plane and a
/// facing in the rig's own frame share one in the room too, and a pair
/// that does not, does not. **The kind is therefore swept once**, not
/// once per berth: what changes with the berth is where the rig stands,
/// and no rule here is about that. What does not survive the change of
/// frame is the *lengths*, so the local sim units are carried into
/// metres through [`crate::pieces::RIG_UNIT`] before anything is
/// compared to a threshold.
///
/// `showing` is which of a covering's two bodies is up. The laid one and
/// the packed one are never drawn together (`pieces::sync_dressings`
/// shows exactly one), so a plane they happen to share is not a plane
/// anybody sees.
fn rig_scene(declared: &Dressings, kind: Kind, screens: Screens, showing: Under) -> Vec<Drawn> {
    if let Some(drawn) = dressed_scene(declared, kind) {
        return drawn;
    }
    rig_parts(declared, kind, screens, showing)
        .into_iter()
        .filter_map(|part| {
            let body = part.body?;
            let at = rig_pose(&part);
            let faces = if body.sheet() {
                Faces::showing(at.rotation * Vec3::Z)
            } else {
                Faces::of(body.shape(), at.rotation)
            };
            Some(Drawn {
                what: part.label(),
                body: Box3::of(&at, body.half() * crate::pieces::RIG_UNIT),
                faces,
                character: true,
                // A rig's joints are `part-seated`'s, declared on the
                // part and read straight off it; this pair is for the
                // things a room hangs.
                name: None,
                seat: None,
            })
        })
        .collect()
}

/// Whether a purchased mesh stands in for this kind's whitebox parts —
/// the one question the three families that measure a rig against ITSELF
/// have to ask before they ask anything else.
const fn dressed(declared: &Dressings, kind: Kind) -> bool {
    declared.of(kind).is_some()
}

/// **What a kind draws when a purchased mesh draws it**: one body, the
/// size `art/manifest.toml` promises it is, or nothing at all for a kind
/// nothing dresses.
///
/// This is the whole of how a bought asset enters the harness, and the
/// framing is worth being exact about, because it decides what a green
/// sweep means.
///
/// **The declaration is swept, not the mesh.** The mesh is not in this
/// repository, is not on the continuous-integration runner, and never
/// will be — the licence is why. So what CI can check is the *promise*:
/// `fill` says what fraction of its berth box the body occupies, and
/// every rule about where a cargo body stands can be asked of that box
/// exactly as it is asked of the whitebox parts. `cargo xtask art
/// resolve` is where the promise is checked against the mesh, on the
/// machine that has one.
///
/// **It applies whether or not `--features art` is on**, and that is
/// deliberate rather than an oversight. The build that can draw the mesh
/// is the build CI cannot run; if the sweep only looked at declarations
/// in a build nobody tests, the declarations would be swept nowhere. A
/// `dresses` line is a statement about what this kind's body IS, and the
/// harness takes it at its word.
///
/// **It stands in for the parts rather than joining them.** A purchased
/// crate is not a whitebox crate with a mesh next to it; it is the same
/// object drawn another way, and measuring both would report every kind
/// as two bodies in one berth.
fn dressed_scene(declared: &Dressings, kind: Kind) -> Option<Vec<Drawn>> {
    let dressing = declared.of(kind)?;
    let (mid, half) = dressing.fill_box(kind);
    let unit = crate::pieces::RIG_UNIT;
    Some(vec![Drawn {
        what: format!("{} (purchased)", dressing.id),
        body: Box3::spanning((mid - half) * unit, (mid + half) * unit),
        // A box, and the sides of it are sides: `fill` is the tight box
        // round the body, so unlike a `Shape::Ring`'s frame there is no
        // air between the wrapper and the thing.
        faces: Faces::ALL,
        character: true,
        // A bought mesh declares no joints and points no features. What
        // it declares is its size, and that is what is measured.
        name: None,
        seat: None,
    }])
}

/// The parts one kind draws AT ONCE: everything that always draws, plus
/// whichever of a covering's two bodies `showing` names. One place
/// decides what is in a scene, so the families that measure a rig
/// against itself all measure the same scene.
///
/// **A dressed kind draws no parts.** The whitebox description is still
/// there and is still what a default build stamps, but it is not what
/// the manifest says the body IS — see [`dressed_scene`] — so the
/// families that measure a rig against ITSELF have nothing to ask. A
/// bought mesh's own joints and its own named directions are inside a
/// file this repository cannot open.
fn rig_parts(
    declared: &Dressings,
    kind: Kind,
    screens: Screens,
    showing: Under,
) -> Vec<crate::pieces::Part> {
    if dressed(declared, kind) {
        return Vec::new();
    }
    let piece = Piece {
        id: 0,
        kind,
        variant: 0,
        gnawed: false,
        loc: Loc::Hold {
            room: CABIN,
            x: 0,
            y: 0,
        },
    };
    crate::pieces::parts(&piece, screens)
        .into_iter()
        .filter(|part| match part.under {
            Under::Laid | Under::Packed => part.under == showing,
            Under::Rig | Under::Arm | Under::Pivot(_) => true,
        })
        .collect()
}

/// Where one part stands in its rig's own frame, in METRES — the single
/// place a `Part`'s local transform becomes a length.
///
/// The sub-frames' own poses, at rest: the sconce's arm hangs level
/// until a wall column swings it, and the launch handle's pivot sits in
/// its slot until somebody pulls it. A THROWN lever is an animation
/// rather than a defect, and it is measured where it lives.
fn rig_pose(part: &crate::pieces::Part) -> Transform {
    let mut at = part.under.rest() * part.at;
    at.translation *= crate::pieces::RIG_UNIT;
    at
}

/// **A part that names a seat meets it.** [`PART_SEATED`].
///
/// The eight families before it all measure a part against the WORLD —
/// the band it is composed within, the plane it fights, the cells it
/// draws inside, the direction its own name claims. Not one of them
/// measures a part against another part of the SAME rig, and a joint is
/// exactly that: a couch's foot under a couch it does not touch is
/// inside the band, shares no plane, draws well within its cells, and
/// claims no direction to break. It is four stilts of air, and it went
/// green for as long as the harness has existed.
///
/// The claim is declared on the part that makes it
/// (`pieces::Part::seated`) and read back off the rig, exactly as
/// `prop-points` reads a direction. A part that is composition declares
/// no seat and is asked nothing — which is why [`ALLOWED`] needs no
/// entry here: there is nothing to forgive, only things nobody claimed.
///
/// **Several parts may answer to one name.** A pane glazed behind four
/// lips meets whichever lip it reaches, so the finding is the SMALLEST
/// gap to any of them; a seat no part answers to at all is a finding of
/// its own, because a promise about a body the rig does not draw is a
/// promise nobody can keep.
fn part_seated(declared: &Dressings) -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        // A dressed kind draws one body and no joints, so every seat the
        // whitebox declares names a part that is not in the scene — which
        // would be thirty findings saying nothing but "this kind is
        // dressed".
        if dressed(declared, kind) {
            continue;
        }
        let mut worst: BTreeMap<String, String> = BTreeMap::new();
        let scenes: Vec<Vec<crate::pieces::Part>> = Screens::BOTH
            .into_iter()
            .flat_map(|screens| {
                rig_forms(kind)
                    .into_iter()
                    .map(move |showing| rig_parts(declared, kind, screens, showing))
            })
            .collect();
        // **A name nothing answers to**, asked of the claims the rig
        // itself reports (`pieces::seats`) rather than of one scene: a
        // promise about a body that is never drawn at all is broken in
        // every scene at once, and saying so once is enough.
        for seat in crate::pieces::seats(kind) {
            if !scenes
                .iter()
                .any(|scene| scene.iter().any(|part| part.what == seat.on))
            {
                worst.insert(
                    seat.name.to_owned(),
                    format!(
                        "names \"{}\" as the part that holds it, and this rig draws no \
                         such part",
                        seat.on
                    ),
                );
            }
        }
        for scene in &scenes {
            let boxed = |part: &crate::pieces::Part| {
                part.body
                    .map(|body| Box3::of(&rig_pose(part), body.half() * crate::pieces::RIG_UNIT))
            };
            for part in scene {
                let (Some(seat), Some(body)) = (part.seat, boxed(part)) else {
                    continue;
                };
                let gap = scene
                    .iter()
                    .filter(|other| other.what == seat.on)
                    .filter_map(boxed)
                    .map(|held| body.apart(held))
                    .fold(f32::INFINITY, f32::min);
                if gap.is_finite() && gap > SEAT_GAP {
                    worst.insert(
                        part.label(),
                        format!(
                            "stands {gap:.4} m clear of the \"{}\" that holds it, which \
                             is daylight in a joint rather than the step a joint is \
                             drawn with",
                            seat.on
                        ),
                    );
                }
            }
        }
        out.extend(worst.into_iter().map(|(what, detail)| Finding {
            room: RIGS.to_owned(),
            rule: PART_SEATED,
            offender: format!("{kind:?} {what}"),
            detail,
        }));
    }
    out
}

/// Which bodies of a kind are ever drawn at once: one scene for most
/// kinds, and for a covering the two its berth class picks between.
fn rig_forms(kind: Kind) -> Vec<Under> {
    if kind.covering() {
        vec![Under::Laid, Under::Packed]
    } else {
        vec![Under::Rig]
    }
}

/// **No rig draws two faces on one plane.** [`NO_COPLANAR`] again, one
/// crate down: the same detector, the same thresholds, asked of a cargo
/// kind's own parts instead of a room's furniture.
///
/// This is the family the Guild's chit belonged to. Its card and its
/// stripe were cut to the same height and set on the same centre, so the
/// tops were one plane and the bottoms another, along the whole of a
/// stripe somebody holds up to their eye — and nothing could ask, because
/// `build_kind` composed straight into a live world and there was no
/// description to measure. There is one now.
fn rig_coplanar(declared: &Dressings) -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let mut shared: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for screens in Screens::BOTH {
            for showing in rig_forms(kind) {
                let scene = rig_scene(declared, kind, screens, showing);
                for (i, a) in scene.iter().enumerate() {
                    for b in &scene[i + 1..] {
                        let faces = shared_faces(a, b);
                        if faces.is_empty() {
                            continue;
                        }
                        shared
                            .entry(format!("{} / {}", a.what, b.what))
                            .or_insert(faces);
                    }
                }
            }
        }
        out.extend(shared.into_iter().map(|(pair, faces)| Finding {
            room: RIGS.to_owned(),
            rule: NO_COPLANAR,
            offender: format!("{kind:?} {pair}"),
            detail: format!("share {}", faces.join(" and ")),
        }));
    }
    out
}

/// **A rig is no deeper than the band it is drawn in.**
/// [`BERTH_CLEAR`] from the cargo's side.
///
/// `pieces::RIG_NEAR..RIG_FAR` is the depth every kind is composed
/// within, and two things read it: the carry tell's wireframe box, which
/// wraps the body you are holding, and `pieces::berth_box`. A part
/// outside it is a body hanging out of the box the tell draws round it,
/// on every kind there is.
///
/// **On a wall it is also the air**, and that half is narrower than it
/// was written. A wall berth's air IS the band along the chart's normal,
/// so a wall kind deeper than the band is a berth the whole room sweep
/// measures too shallow and a fitting could stand in the part with
/// nothing to say so. A deck or ceiling berth's air is its own cell's
/// column — the footprint across and the rig's height up — and the local
/// `+z` the band is measured on lies flat into the room there, which is
/// the aisle a standing rig is expressly allowed to reach into
/// ([`Berth`]). Both of the two parts this family last carried were
/// floor-mounted, so neither was ever a berth measured wrong; both were a
/// body out of its own carry box, which is defect enough. The finding
/// says which it is rather than claiming the worse one twice.
///
/// **It asks about the depth and not about the footprint**, and that is
/// measured rather than assumed. Sweeping the footprint too found eight
/// things, all one idiom: the cabinet's four feet and the couch's four
/// sink a third of a centimetre below their own bottom edge, which for a
/// rig that STANDS is the deck it stands on. A sole flush with the deck
/// is a plane shared with the deck, and a sole above it is furniture
/// floating; burying it is how a foot meets a floor. What a rig covers
/// across is its plan, and its plan already has a rule —
/// `pieces::tests::every_kind_hangs_true_on_every_legal_berth` asks
/// whether the body a rig draws lands on the four corners its rect owns.
fn rig_fits(declared: &Dressings) -> Vec<Finding> {
    let unit = crate::pieces::RIG_UNIT;
    let (near, far) = (
        crate::pieces::RIG_NEAR * unit,
        crate::pieces::RIG_FAR * unit,
    );
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let mut over: BTreeMap<String, f32> = BTreeMap::new();
        for screens in Screens::BOTH {
            for showing in rig_forms(kind) {
                for drawn in rig_scene(declared, kind, screens, showing) {
                    let spill = (near - drawn.body.lo.z).max(drawn.body.hi.z - far);
                    if spill > CLIP_SLACK {
                        over.insert(drawn.what, spill);
                    }
                }
            }
        }
        out.extend(over.into_iter().map(|(what, spill)| Finding {
            room: RIGS.to_owned(),
            rule: BERTH_CLEAR,
            offender: format!("{kind:?} {what}"),
            detail: format!(
                "reaches {spill:.4} m out of the {near:.3}..{far:.3} m band every rig \
                 is composed within, so it hangs out of the box the carry tell wraps \
                 it in{}",
                if matches!(kind.mount(), Mount::Wall) {
                    " — and this kind hangs on a wall, where that band is also the \
                     air its berth spends, so every berth it may take is measured \
                     too shallow"
                } else {
                    ""
                }
            ),
        }));
    }
    out
}

/// **A rig draws inside the cells the sim gave it.** [`FACE_FITS`].
///
/// A footprint the sim states and a body the rig draws are two claims
/// about one object, and the footprint is the law: it is what placement
/// is checked against, and it is what the sim answers "which piece is at
/// this point" with. The *picture* is what a player aims at, so the pick
/// face is cut from the picture (`pieces::silhouette`) — and held to the
/// cells, because a face reaching past them would read a neighbour's
/// berth.
///
/// This family is the price of that holding. A part drawn outside its
/// own cells is paint the aim cannot follow: the body is out there and
/// the pick stops at the cell edge, so the piece has a visible edge that
/// does not answer. In the world it is a crate you can see the corner of
/// and cannot click, or a lamp whose shade overhangs the berth beside
/// it and grabs the wrong thing when the neighbour is picked.
///
/// **It asks about the plan and not the depth.** The depth is
/// [`rig_fits`]'s question, measured against the band every rig is
/// composed within; this one is measured against the kind's own `w × h`,
/// which is the only extent the sim ever states.
fn rig_faces(declared: &Dressings) -> Vec<Finding> {
    let unit = crate::pieces::RIG_UNIT;
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let (w, h) = kind.upright();
        let cells = Vec2::new(f32::from(w) * layout::CELL, f32::from(h) * layout::CELL) * 0.5;
        let mut over: BTreeMap<String, f32> = BTreeMap::new();
        for screens in Screens::BOTH {
            for showing in rig_forms(kind) {
                for drawn in rig_scene(declared, kind, screens, showing) {
                    // `rig_scene` already measures in metres, so the
                    // cells are carried across rather than the bodies.
                    let (lo, hi) = (drawn.body.lo, drawn.body.hi);
                    let (down, up) = plan_sink(kind);
                    let plan = cells * unit;
                    let worst = [
                        (-plan.x - lo.x, CLIP_SLACK),
                        (hi.x - plan.x, CLIP_SLACK),
                        (-plan.y - lo.y, down),
                        (hi.y - plan.y, up),
                    ]
                    .into_iter()
                    .filter(|&(spill, tol)| spill > tol)
                    .map(|(spill, _)| spill)
                    .fold(f32::NEG_INFINITY, f32::max);
                    if worst.is_finite() {
                        over.insert(drawn.what, worst);
                    }
                }
            }
        }
        out.extend(over.into_iter().map(|(what, spill)| Finding {
            room: RIGS.to_owned(),
            rule: FACE_FITS,
            offender: format!("{kind:?} {what}"),
            detail: format!(
                "reaches {spill:.4} m outside the {:.3} × {:.3} m plan the sim gives \
                 a {w}×{h} kind, so the pick face stops at the cell edge and the \
                 paint does not",
                cells.x * 2.0 * unit,
                cells.y * 2.0 * unit
            ),
        }));
    }
    out
}

/// **Where a rig meets the chart it is berthed on**, in its own frame: a
/// point on that chart and the direction the rig reaches to get there.
///
/// A kind is composed once, in the upright frame no berth turns, and
/// berthed by `pieces::site_on` — which puts a deck berth's
/// rig half its own height above the chart, a deckhead berth's half its
/// height below, and a wall berth's flat on it. So the chart is at a
/// known plane of the rig's own frame, and which plane is a question
/// about the chart's CLASS and nothing else.
///
/// `None` where the kind may not be berthed on that class at all, which
/// is the arbiter's ruling and is asked of the arbiter
/// (`cargo::mount_accepts`) rather than restated here.
fn chart_joint(kind: Kind, surf: Surf) -> Option<Joint> {
    if !mount_accepts(kind.mount(), surf) {
        return None;
    }
    let tall = f32::from(kind.upright().1) * layout::CELL * 0.5 * crate::pieces::RIG_UNIT;
    Some(match surf {
        // A rig STANDS on a deck: its own frame's floor is half its
        // height below the middle, and its sole is what reaches it.
        Surf::Floor => Joint {
            face: "sole",
            chart: "deck",
            at: Vec3::new(0.0, -tall, 0.0),
            toward: Vec3::NEG_Y,
        },
        // A pendant hangs UNDER a deckhead: the same joint upside down,
        // and its canopy is what reaches it.
        Surf::Ceiling => Joint {
            face: "canopy",
            chart: "deckhead",
            at: Vec3::new(0.0, tall, 0.0),
            toward: Vec3::Y,
        },
        // A rig hung on a wall is composed straight onto the berth
        // plane, so the wall is its own frame's `z = 0` and its back is
        // what reaches it. All four walls are one answer, which is what
        // the arbiter says too: nobody hangs a painting on a compass
        // heading.
        Surf::Aft | Surf::Port | Surf::Starboard | Surf::Front => Joint {
            face: "back",
            chart: "wall",
            at: Vec3::ZERO,
            toward: Vec3::NEG_Z,
        },
    })
}

/// One rig-to-chart joint: which face of the rig makes it, what the
/// chart is called, and the plane the face has to reach stated as a
/// point on it and the direction the rig reaches along.
#[derive(Clone, Copy, Debug)]
struct Joint {
    face: &'static str,
    chart: &'static str,
    at: Vec3,
    toward: Vec3,
}

impl Joint {
    /// How far short of the chart a body stops, in metres: positive is
    /// daylight, zero is flush, negative is buried.
    fn short_of(self, body: Box3) -> f32 {
        short_of(self.at, self.toward, body)
    }
}

/// **How far a body stops short of a world plane**, in metres: positive
/// is daylight, zero is flush, negative is buried. The plane is a point
/// on it and the direction the body reaches along to meet it, so the
/// reading is the body's furthest extent that way against the plane's
/// own.
///
/// The one arithmetic three families spend — a rig against the chart it
/// is berthed on, and a station's furniture and a doorway's hardware
/// against whatever they say holds them up. A joint is a joint.
fn short_of(at: Vec3, toward: Vec3, body: Box3) -> f32 {
    let centre = (body.lo + body.hi) * 0.5;
    let half = (body.hi - body.lo) * 0.5;
    at.dot(toward) - (centre.dot(toward) + half.dot(toward.abs()))
}

/// How far a kind's rig may sink past its own plan on the way down and
/// on the way up, in metres — [`SOLE_SINK`] on whichever face meets a
/// chart and [`CLIP_SLACK`] on the one that meets nothing.
///
/// Derived from the joints rather than from the mount, so the day a kind
/// may take two chart classes it is allowed to meet both of them.
fn plan_sink(kind: Kind) -> (f32, f32) {
    let mut sink = (CLIP_SLACK, CLIP_SLACK);
    for surf in Surf::ALL {
        let Some(joint) = chart_joint(kind, surf) else {
            continue;
        };
        if joint.toward.y < 0.0 {
            sink.0 = SOLE_SINK;
        }
        if joint.toward.y > 0.0 {
            sink.1 = SOLE_SINK;
        }
    }
    sink
}

/// **A rig reaches the chart it is berthed on.** [`RIG_SEATED`].
///
/// [`PART_SEATED`]'s sibling, one plane down. That family measures a
/// part against another part of the same rig; this one measures the
/// whole rig against the one thing outside it that a berth actually
/// promises — the deck it stands on, the deckhead it hangs from, the
/// wall it is screwed to.
///
/// **No family caught it.** [`berth_clear`] measures the depth a rig is
/// composed within and says nothing about where inside that depth a body
/// stops. [`rig_faces`] catches a body reaching OUTSIDE its own plan and
/// is deliberately blind to one that fails to reach the plan's own
/// floor — it forgives a centimetre of burial there ([`SOLE_SINK`]) and
/// asks nothing at all about a metre of air. [`part_seated`] is
/// part-against-part. A crate that stops seven centimetres above its own
/// deck cell satisfies every one of the ten and is a crate standing on
/// nothing.
///
/// **It is asked on every chart class the kind may take**, which is the
/// arbiter's list and not this file's ([`chart_joint`]). A kind takes one
/// class today, so it is asked once; the sweep is written the other way
/// round because which plane a body must reach is a property of the
/// chart and never of the body.
///
/// **The tolerance is [`SEAT_GAP`], the same number the seat family
/// spends**, because it is the same joint: a step of the decal ladder
/// plus the thickest paint that could be riding on the seat's own face.
/// A chart's face carries paint too — a tile field, a class's mark — and
/// a rig meets it the way a pane meets a bezel, by going a step into it
/// (`pieces::SOLE_BURY`), so the builder's number sits inside the rule
/// with room to spare. What the rule refuses is the next order of
/// magnitude: a gap you can see the deck through, which on these rigs
/// starts at about a centimetre.
///
/// The other side of the same joint is [`rig_faces`]', which refuses
/// more than [`SOLE_SINK`] of burial. Between them a rig's sole has a
/// band it must land in, and that band is a hair either side of the
/// chart.
fn rig_seated(declared: &Dressings) -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        for surf in Surf::ALL {
            let Some(joint) = chart_joint(kind, surf) else {
                continue;
            };
            // The worst the rig ever looks: a kind whose live and
            // headless bodies differ reaches its chart in one of them
            // and not the other, and the one that misses is the finding.
            let mut worst: Option<(f32, String)> = None;
            for screens in Screens::BOTH {
                for showing in rig_forms(kind) {
                    // A laid covering is not composed onto its chart at
                    // all: `pieces::laid_on` lays it ON the plane and
                    // lifts it a rung of the decal ladder, so its joint
                    // is a derivation and there is nothing a builder
                    // could have got wrong. Only what a berth STANDS is
                    // asked.
                    if showing == Under::Laid {
                        continue;
                    }
                    let scene = rig_scene(declared, kind, screens, showing);
                    let closest = scene
                        .into_iter()
                        .map(|drawn| (joint.short_of(drawn.body), drawn.what))
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some(closest) = closest
                        && worst.as_ref().is_none_or(|(gap, _)| closest.0 > *gap)
                    {
                        worst = Some(closest);
                    }
                }
            }
            let Some((gap, what)) = worst else { continue };
            if gap <= SEAT_GAP {
                continue;
            }
            out.push(Finding {
                room: RIGS.to_owned(),
                rule: RIG_SEATED,
                offender: format!("{kind:?} {}", joint.face),
                detail: format!(
                    "stops {gap:.4} m short of the {} it is berthed on — the nearest \
                     body to it is \"{what}\" — so the rig stands on nothing",
                    joint.chart
                ),
            });
        }
    }
    out
}

/// **How far a berthed body sits off the middle of the ground its plan
/// owns, and how much of that ground it fills**, on one axis of one
/// chart, in metres.
///
/// The one arithmetic [`berth_filled`] spends, factored out for the same
/// reason [`short_of`] is: a family whose reading is buried inside its
/// own loop cannot be shown to answer, and a rule nobody can catch out
/// is a green tick that means nothing.
fn off_plan(spent: Box3, owned: Box3, dir: Vec3) -> (f32, f32) {
    let mid = |body: Box3| (body.lo + body.hi).dot(dir) * 0.5;
    (mid(spent) - mid(owned), spent.span().dot(dir.abs()))
}

/// Which world axis a chart direction runs along. Every chart is square
/// on the world axes — the lattice only ever turns a room by quarter
/// turns — so one of the three always answers.
fn axis_of(dir: Vec3) -> usize {
    let d = dir.abs();
    if d.x >= d.y && d.x >= d.z {
        0
    } else if d.y >= d.z {
        1
    } else {
        2
    }
}

/// One berth, as the two claims [`berth_filled`] compares: the ground
/// the sim's plan owns and the box the drawing spends standing on it.
struct Plan {
    kind: Kind,
    /// The chart class it stands on, which is what a berth's rect means.
    surf: Surf,
    /// Which of the net's six charts it is, so a rule can tell a plane a
    /// body lies ON from one it hangs AGAINST.
    station: Station,
    /// The chart itself, for its own two axes.
    chart: SimSurface,
    /// The cells the sim gave it, in the chart's own frame.
    rect: layout::Rect,
    /// The turn the berth gave it — the half of a pose no box carries.
    rot: Quat,
    owned: Box3,
    spent: Box3,
}

/// **Every berth in the game, with both claims about it.** The sim's
/// arbiter rules which cells a kind may take, `cargo::plan` says how many
/// it then owns, and `pieces::berth_box` poses the body through the very
/// function the runtime poses it with — so a retune of the berth pose
/// moves the question and the answer together.
///
/// Held as a list rather than swept in place because the family and its
/// guard both walk it: a rule that cannot be handed a moved body is a
/// rule nobody can catch out.
fn plans(stages: &[Stage]) -> Vec<Plan> {
    let mut out = Vec::new();
    for stage in stages {
        let (cols, rows) = stage.placed.kind.grid();
        for kind in Kind::ALL {
            // A covering does not stand on its cells, it LIES into them
            // (`pieces::laid_on`), so the ground it owns is the chart
            // itself and there is no band to land anywhere.
            if kind.covering() {
                continue;
            }
            for y in 0..rows {
                for x in 0..cols {
                    if placement_check(&stage.rooms, &[], u32::MAX, kind, stage.placed.id, x, y)
                        .is_err()
                    {
                        continue;
                    }
                    let (Some((w, h)), Some(surf), Some((_, chart))) = (
                        plan(stage.placed.kind, kind, x, y),
                        stage.placed.kind.surface_of(x, y),
                        chart_of(&stage.placed, (x, y)),
                    ) else {
                        continue;
                    };
                    let anchor = layout::cell_rect(stage.placed.id, x, y);
                    let rect = layout::Rect::new(
                        anchor.x,
                        anchor.y,
                        f32::from(w) * layout::CELL,
                        f32::from(h) * layout::CELL,
                    );
                    let (Some((lo, hi)), Some((station, _, _, rot, _))) = (
                        crate::pieces::berth_box(&stage.placed.charts, kind, rect),
                        crate::pieces::berth_pose(&stage.placed.charts, kind, rect),
                    ) else {
                        continue;
                    };
                    out.push(Plan {
                        kind,
                        surf,
                        station,
                        chart,
                        rect,
                        rot,
                        owned: plan_face(&chart, rect),
                        spent: Box3 { lo, hi },
                    });
                }
            }
        }
    }
    out
}

/// **A rig fills the cells its berth spends.** [`BERTH_FILLED`].
///
/// The twelfth family, and the one that closes the last gap between the
/// two claims a berthed piece makes. The sim states a footprint and the
/// cabin draws a body, and every rule before this asked whether the body
/// stayed INSIDE something: [`rig_faces`] holds a rig to its own `w × h`
/// plan and forgives everything short of it, [`rig_fits`] holds it to
/// the depth every rig is composed within and forgives everything short
/// of that, and [`rig_seated`] asks only about the one face that has to
/// touch a chart. **Not one of them asks where inside its berth the body
/// actually is** — so a body could sit hard against one edge of the cells
/// it was given, or half out of them on the axis nothing measured, and
/// stay green in all eleven.
///
/// That is what it did. A rig's own `z = 0` is the BERTH PLANE and its
/// body is composed from just behind it to one cell out into the room —
/// which is the truth on a wall, where the plane is the chart the rig is
/// screwed to. A deck berth has no such plane: its rect is a plan, the
/// cells own the ground on both sides of their own middle, and the band
/// laid off a plane that is not there stood every deck and deckhead
/// berth in the game 0.2329 m out into the aisle. That is 0.42 of a
/// cell, on the one axis the plan spends its depth on and never on the
/// other — which is the shape of the thing the owner reported four times
/// and the harness passed four times. (The bodies inside the band came
/// out 0.117 m to 0.250 m off, kind by kind; the band is where they were
/// composed and the band is what a berth costs.)
///
/// **It asks about the two axes the RECT pays for and not the third.**
/// A berth's rect spends two of a kind's three extents and the chart
/// fixes the other (`cargo::Kind::plan_on`): a deck berth spends across
/// and deep and the deck fixes the height, a wall berth spends across
/// and tall and the wall fixes the depth. What the chart fixes is
/// [`rig_seated`]'s question from one side and [`rig_fits`]'s from the
/// other, and on a wall it is deliberately off centre — the band begins
/// a hair BEHIND the plane, so a rig's back sinks into the paint it is
/// screwed over. What the cells pay for is nobody else's question, and
/// on those axes a body is centred or it is misplaced.
///
/// Two clauses, one reading ([`off_plan`]):
///
/// - **Where.** The box a berth spends is centred on the ground its plan
///   owns, to within [`GRID_EPS`] — the same millimetre `grid-fits`
///   calls a face on its line, because this is the same question one
///   layer up.
/// - **How much.** And it is [`crate::pieces::BAY_FIT`] of that ground,
///   which is the margin a rig wears across and up, said on the axes a
///   plan spends. A body claiming ground it does not fill is a berth
///   measured in the wrong place, which is what `berth-clear` then tells
///   a station's furniture about.
///
/// Filed under [`RIGS`] and keyed by kind and chart class, because the
/// same crate stands in every room in the game and a defect in how a
/// deck berths it is not fifteen defects.
fn berth_filled(stages: &[Stage]) -> Vec<Finding> {
    // Per kind and chart class, on each world axis: the worst offset off
    // the middle, the worst span, the margin that span should have been,
    // and the ground the plan owns there.
    let mut worst: BTreeMap<(String, usize), (f32, f32, f32, f32)> = BTreeMap::new();
    for berth in plans(stages) {
        for dir in [
            berth.chart.half_u.normalize(),
            berth.chart.half_v.normalize(),
        ] {
            let (off, got) = off_plan(berth.spent, berth.owned, dir);
            let ground = berth.owned.span().dot(dir.abs());
            let want = ground * crate::pieces::BAY_FIT;
            let kind = berth.kind;
            let surf = berth.surf;
            let key = (format!("{kind:?} on a {surf:?} berth"), axis_of(dir));
            // The worst berth of the class speaks for it: the same crate
            // on the same chart is one thing to move, and one line that
            // says so is a work order rather than a transcript.
            let seen = worst.entry(key).or_insert((0.0, want, want, ground));
            if off.abs() > seen.0.abs() {
                seen.0 = off;
            }
            if (got - want).abs() > (seen.1 - seen.2).abs() {
                seen.1 = got;
                seen.2 = want;
                seen.3 = ground;
            }
        }
    }
    let mut out = Vec::new();
    for ((offender, axis), (off, got, want, ground)) in worst {
        let name = ["x", "y", "z"][axis];
        if off.abs() > GRID_EPS {
            out.push(Finding {
                room: RIGS.to_owned(),
                rule: BERTH_FILLED,
                offender: offender.clone(),
                detail: format!(
                    "is composed {:.4} m off the middle of the {ground:.3} m of chart its \
                     plan owns on the {name} axis, so the body a berth stands is not on \
                     the lattice that chart is drawn in",
                    off.abs()
                ),
            });
        }
        if (got - want).abs() > GRID_EPS {
            out.push(Finding {
                room: RIGS.to_owned(),
                rule: BERTH_FILLED,
                offender,
                detail: format!(
                    "fills {got:.4} m of the {ground:.3} m of chart its plan owns on the \
                     {name} axis, where a rig wears {want:.4} m, so it does not wear one \
                     margin on all three axes"
                ),
            });
        }
    }
    out
}

/// **The ground a standing rig is looking at**, in its chart's own
/// frame: the middle of the cell one step beyond its own footprint,
/// along the way its face points.
///
/// [`berth_turned`]'s one arithmetic, factored out for the reason
/// [`off_plan`] and [`short_of`] are: a family whose reading is buried
/// inside its own loop cannot be handed a body that has been turned, and
/// a rule nobody can catch out is a green tick that means nothing.
///
/// The look direction lies in the chart's plane for the two charts a rig
/// stands ON, so it resolves to a unit step across the net; on a chart a
/// rig hangs AGAINST it points out of the plane and this says nothing
/// worth reading, which is why the family splits the two.
fn looked_at(chart: &SimSurface, rect: layout::Rect, rot: Quat) -> space_trucking::sim::Vec2 {
    let look = rot * Vec3::Z;
    let du = look.dot(chart.half_u.normalize());
    let dv = look.dot(chart.half_v.normalize());
    let reach = layout::CELL.mul_add(0.5, du.abs().mul_add(rect.w, dv.abs() * rect.h) * 0.5);
    space_trucking::sim::Vec2::new(
        du.mul_add(reach, rect.w.mul_add(0.5, rect.x)),
        dv.mul_add(reach, rect.h.mul_add(0.5, rect.y)),
    )
}

/// **A rig is turned the way its chart and its room say.** [`BERTH_TURNED`].
///
/// The thirteenth family, and the one that closes the half of a berth's
/// pose no box has ever carried. Every rule before it measured a rig as
/// an axis-aligned BOX — the band it is composed within, the ground its
/// plan owns, the plane its sole has to reach — and a box is the same box
/// after a half turn, and the same box after a quarter turn whenever the
/// footprint is square. So the whole of "which way is it looking" fell
/// through eleven families, and the twelfth caught only the quarter turns
/// a non-square plan pays for.
///
/// [`PROP_POINTS`] is the nearest thing there was and it looks one body
/// in: it asks whether a sconce's cup points where the word "cup" says,
/// **inside the rig's own frame**, and a rig hung backwards carries every
/// one of its features faithfully backwards with it. Nothing asked what
/// the rig's own frame was doing.
///
/// Two claims, and between them they pin the turn on every chart the game
/// has:
///
/// - **It stands up.** A rig's own up is the room's up. On a wall that is
///   the upright rule's whole purpose (`pieces::wall_upright` rolls a
///   chart's lie back onto the room's); on a deck and under a deckhead it
///   is what "standing" and "hanging" mean. A quarter turn about the face
///   normal breaks it, and so does an upside-down one — which is what
///   makes this the clause that catches a SQUARE footprint, the one case
///   `berth-filled` is blind to by construction.
/// - **It shows its face to the room.** On a wall, the face a rig turns
///   to the room is the wall's own inward normal. On a deck or under a
///   deckhead there is no such normal, so the claim is the player's
///   instead: **the deck a standing rig is looking at is deck of the same
///   room** ([`looked_at`]). A couch with its face in the front wall is
///   the defect, and it reads as one here without this file ever learning
///   the backing rule's branches — which matters, because a sweep that
///   recomputed `pieces::floor_facing` and compared it with itself would
///   pass every berth in the game and mean nothing.
///
/// What is deliberately NOT asked is the turn of a body whose plan is
/// square and whose cell is nowhere near a wall: a crate in the middle of
/// the deck may face any of four ways and every one of them is a room a
/// player can walk round. Composition is the art direction's business and
/// this file measures shapes.
///
/// Filed under [`RIGS`] and keyed by kind and chart class, for
/// [`berth_filled`]'s reason: the same crate stands in every room in the
/// game, and a defect in how a deck turns it is not fifteen defects.
fn berth_turned(stages: &[Stage]) -> Vec<Finding> {
    // Per kind, chart class and clause: the worst reading of the lot,
    // with the cell it was read at.
    let mut worst: BTreeMap<(String, u8), (f32, String)> = BTreeMap::new();
    for berth in plans(stages) {
        let key = |clause: u8| {
            (
                format!("{:?} on a {:?} berth", berth.kind, berth.surf),
                clause,
            )
        };
        let note = |worst: &mut BTreeMap<(String, u8), (f32, String)>, clause, read, what| {
            let seen = worst
                .entry(key(clause))
                .or_insert((f32::INFINITY, String::new()));
            if read < seen.0 {
                *seen = (read, what);
            }
        };
        let up = (berth.rot * Vec3::Y).dot(Vec3::Y);
        note(
            &mut worst,
            0,
            up,
            format!("its own up points {:?}", berth.rot * Vec3::Y),
        );
        if matches!(berth.station, Station::BayFloor | Station::BayCeiling) {
            let at = looked_at(&berth.chart, berth.rect, berth.rot);
            note(
                &mut worst,
                1,
                if berth.chart.rect.contains(at) {
                    1.0
                } else {
                    0.0
                },
                format!(
                    "it looks at ({:.1}, {:.1}), which is off its own room's {:?} chart",
                    at.x, at.y, berth.surf
                ),
            );
        } else {
            let inward = berth.station.inward(&berth.chart);
            note(
                &mut worst,
                1,
                (berth.rot * Vec3::Z).dot(inward),
                format!("it shows the room {:?}", berth.rot * Vec3::Z),
            );
        }
    }
    let mut out = Vec::new();
    for ((offender, clause), (read, what)) in worst {
        if read > 0.999 {
            continue;
        }
        out.push(Finding {
            room: RIGS.to_owned(),
            rule: BERTH_TURNED,
            offender,
            detail: match clause {
                0 => format!("{what}, not the room's up, so the berth hangs it a turn off true"),
                _ => format!("{what}, so the berth turns its face away from the room"),
            },
        });
    }
    out
}

/// **Every cell of deck a body may set cargo on is walkable to from the
/// door it comes in by.** [`DECK_REACHED`].
///
/// The fourteenth family, and the first one in this file that is about a
/// room's NET rather than about anything drawn in it. Every other rule
/// here measures metres; this one counts cells, and it counts them
/// through the sim's own declaration (`RoomKind::marooned`) rather than
/// through a second one of its own — the cabin may not restate a sim
/// rule, and *which cells the player may use* is as sim a rule as there
/// is.
///
/// It exists because the owner walked a defect the sim's own entry-path
/// law read green. That law clears a chalked band out of the straight
/// run in from a door, and it holds. What it is about is a LANE; what the
/// owner was doing was a JOURNEY, and the journey's first step landed on
/// the shopfront: `Trade` and `Wreck` hang their goods along the wall
/// they present to whatever they came alongside, which is the wall their
/// one door is punched through, so a body walked in and stood on the
/// counter, and the nearest deck a crate of theirs could go on was a step
/// further in.
///
/// Two clauses, and the first is the one that fired:
///
/// - **A door stands on deck a body may use.** Every cell of a declared
///   door's own step takes the player's cargo.
/// - **And nothing is walled off behind it.** From that step, every cell
///   of the room's deck the player may use is reachable across such cells
///   alone.
///
/// Filed under the room KIND ([`kind_name`]) rather than under a station,
/// because a net is folded the same way in every station that has one and
/// a defect in how `Trade` lays its deck out is one defect, not twelve.
fn deck_reached() -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in space_trucking::sim::room::ROOM_KINDS {
        for (slot, _) in kind.declared() {
            let Some(marooned) = kind.marooned(slot) else {
                continue;
            };
            for (x, y) in kind.doorsteps(slot).into_iter().flatten() {
                let tile = kind.tile_of(x, y);
                if tile.is_some_and(Tile::takes_your_cargo) {
                    continue;
                }
                out.push(Finding {
                    room: kind_name(kind),
                    rule: DECK_REACHED,
                    offender: format!("port {slot} doorstep"),
                    detail: format!(
                        "stands on ({x}, {y}), which reads {tile:?} — a body walks in and \
                         lands on ground the room has spoken for"
                    ),
                });
            }
            if marooned.is_empty() {
                continue;
            }
            out.push(Finding {
                room: kind_name(kind),
                rule: DECK_REACHED,
                offender: format!("port {slot}"),
                detail: format!(
                    "leaves {} cell(s) of deck the player may use walled off behind the \
                     room's own: {}",
                    marooned.len(),
                    some_cells(&marooned)
                ),
            });
        }
    }
    out
}

/// **Whether a point can be worked from anywhere a body may stand**, and
/// what is in the way when it cannot.
///
/// [`berth_reached`]'s arithmetic, shared with [`fixture_reached`]: within
/// arm's length, inside the pitch limit, and nothing drawn across the
/// line. `Ok` where some stance works it; otherwise the body blocking the
/// nearest stance a body could take, because an "unreachable" with no
/// culprit is a puzzle rather than a work order.
fn worked(scene: &[Drawn], stances: &[Vec3], probe: Vec3) -> Result<(), String> {
    let clear = |eye: &Vec3, dir: Vec3| {
        !scene
            .iter()
            .any(|drawn| ray_box(*eye, dir, drawn.body).is_some_and(|t| t < 1.0 - 1e-3))
    };
    if stances.iter().any(|eye| {
        let dir = probe - *eye;
        let pitch = (-dir.y).atan2(dir.xz().length()).abs();
        dir.length() <= REACH - 0.05 && pitch <= PITCH_LIMIT - 0.02 && clear(eye, dir)
    }) {
        return Ok(());
    }
    Err(stances
        .iter()
        .min_by(|a, b| {
            a.distance_squared(probe)
                .total_cmp(&b.distance_squared(probe))
        })
        .and_then(|eye| {
            let dir = probe - *eye;
            scene
                .iter()
                .find(|drawn| ray_box(*eye, dir, drawn.body).is_some_and(|t| t < 1.0 - 1e-3))
                .map(|drawn| drawn.what.clone())
        })
        .unwrap_or_else(|| "nothing — simply out of reach".to_owned()))
}

/// **A room's own worked hardware stays reachable.** [`FIXTURE_REACHED`].
///
/// The fifteenth family, and [`berth_reached`]'s missing half. That one
/// asks whether every BERTH is workable from somewhere a body may stand,
/// and a berth is a cell of the net cargo may take. The handshake is not
/// a cell of the net — `RoomKind::surface_of` punches a hole where it
/// stands, so the arbiter never offers it and [`berths`] never produces
/// it — and it is the one thing in a calling room a player has to be able
/// to work. A station could hang a beacon in front of its own counter and
/// every rule in this file would agree it was a fine beacon.
///
/// A fixture does not occlude itself, so its own brasswork is taken off
/// the list of things that could be in the way; everything else the room
/// draws stays on it, which is the same scene [`berth_reached`] is asked
/// against and for the same reason — a station's crates come and go, and
/// its furniture does not.
///
/// The probe is the fixture's own pick face (`room::handshake_face`), the
/// very quad the crosshair meets it on, so this asks about the surface the
/// runtime actually answers through rather than about a plane beside it.
fn fixture_reached(stage: &Stage) -> Vec<Finding> {
    let Some(face) = room::handshake_face(&stage.placed) else {
        return Vec::new();
    };
    let scene: Vec<Drawn> = scene(stage)
        .into_iter()
        .filter(|drawn| !drawn.what.starts_with("handshake"))
        .collect();
    match worked(&scene, &stances(stage), face.center) {
        Ok(()) => Vec::new(),
        Err(blame) => vec![Finding {
            room: stage.name.clone(),
            rule: FIXTURE_REACHED,
            offender: blame,
            detail: "stands between the room's handshake and every stance a body may take, \
                     so the one fixture the room is for cannot be worked"
                .to_owned(),
        }],
    }
}

/// What one room of a staged ship files its findings under: the stage's
/// own name for the room it stages, and the ship's own two names for the
/// two rooms every ship carries. So one cabin seen from fifteen ships is
/// one place, and its findings collapse.
fn room_name(stage: &Stage, placed: &Placed) -> String {
    if placed.id == stage.placed.id {
        return stage.name.clone();
    }
    match placed.kind {
        RoomKind::Burner => "burner".to_owned(),
        _ => "cabin".to_owned(),
    }
}

/// **Every surface of a room's own that a hand actually works**, with the
/// way the room is from it: the counter's brass, and the amber latch on
/// every seam that can be parted.
///
/// These are the two things in a room that are neither fabric nor
/// furniture nor cargo: they answer a press, and a press is the only
/// thing in this game that is not a carry. A room's net does not contain
/// them — `RoomKind::surface_of` punches a hole where the handshake
/// stands and a latch hangs on bare wall beside a jamb — so [`berths`]
/// has never produced one and no rule about berths has ever been about
/// one.
///
/// Which way "into the room" is comes off the room's own middle rather
/// than off a quad's winding, because the two are built by different
/// hands: a handshake's face is spun by `Station::face` and a latch's by
/// the seam's own axes.
#[must_use]
pub fn worked_faces(placed: &Placed) -> Vec<(String, Box3, Vec3)> {
    let middle = (placed.lo + placed.hi) * 0.5;
    let mut out = Vec::new();
    let mut add = |what: String, face: &SimSurface| {
        let half = face.half_u.abs() + face.half_v.abs();
        let n = face.normal();
        let inward = if n.dot(middle - face.center) > 0.0 {
            n
        } else {
            -n
        };
        out.push((
            what,
            Box3::spanning(face.center - half, face.center + half),
            inward,
        ));
    };
    if let Some(face) = room::handshake_face(placed) {
        add("handshake".to_owned(), &face);
    }
    for part in room::seam_parts(placed) {
        if let room::Dress::Grab(_, face) = part.dress {
            add(part.what.clone(), &face);
        }
    }
    out
}

/// **How much of a worked face a body stands across**, seen from the room:
/// zero where it is beside it or behind it, one where it covers it whole.
///
/// [`fixture_seen`]'s one reading, and [`berth_seen`]'s turned round: the
/// face, the air out in front of it as far as a body's own stand-off
/// ([`SIGHT`]), and the fraction of the face the body eats of it. It is
/// factored out for the reason [`off_plan`] and [`short_of`] are: a rule
/// whose reading is buried in its own loop cannot be handed a face that
/// has moved behind something, and a rule nobody can catch out is a green
/// tick that means nothing.
#[must_use]
pub fn across(face: Box3, inward: Vec3, body: Box3) -> f32 {
    let span = body.meet(face.reaching(inward, 0.0, SIGHT)).span();
    if span.min_element() <= CLIP_SLACK {
        return 0.0;
    }
    let flat = Vec3::ONE - inward.abs();
    let cover = (span * flat + inward.abs()) / (face.span() * flat + inward.abs());
    cover.x * cover.y * cover.z
}

/// **Nothing a room hangs, and nothing a room stocks, stands between its
/// own worked hardware and the room.** [`FIXTURE_SEEN`].
///
/// [`berth_seen`] read one way for as long as it existed: it asks whether
/// a station's furniture stands between a wall BERTH and the room, and
/// docs/GAUNTLET.md has carried the other direction as a named structural
/// blind spot ever since the owner reported a latch spawning behind the
/// solar system map. This is that direction, and the two clauses it comes
/// out as are the interesting half.
///
/// - **What the room hangs.** A station's fitting or a doorway's hardware
///   standing across a control it did not draw. This is [`berth_seen`]
///   with the roles swapped and it needs no argument: a beacon over a
///   counter is a beacon over a counter.
/// - **What the room stocks.** A berth of a class the room's own arbiter
///   fills — `Tile::Stock`, the one class a room puts its own goods on —
///   spending its air across a control. The doorstep law's sibling one
///   layer out: a room does not lay its goods where a body has to work.
///
/// **And a third clause was written, measured, and taken out again**,
/// which is the finding worth keeping. Asked of EVERY berth rather than
/// only of the ones a room fills, it reports the cabin's own seam latch
/// crossed by three: (5, 1) on the aft wall beside the jamb, and (5, 3)
/// and (5, 4) on the deck in front of it, the worst standing across 100%
/// of the amber. Nothing is wrong with the latch. Every wall cell beside
/// an aperture is a berth and every deck cell in front of one is a berth,
/// so a control bolted beside a doorway shares air with a berth by
/// construction — the rule would have forbidden the latch rather than
/// moved it, and there is nowhere to move it to. It is the same
/// narrowing [`berth_clear`] spends on a jamb standing in a berth and
/// [`berth_reached`] spends on furniture, made for the same reason and
/// with the numbers written down instead of assumed. What is left of the
/// class — a player standing their own crate in front of their own latch
/// — is a crate they can pick up again, and docs/GAUNTLET.md carries it
/// as a bounded blind spot rather than as a rule nobody could obey.
fn fixture_seen(stage: &Stage) -> Vec<Finding> {
    let mut out = Vec::new();
    for placed in &stage.all {
        out.extend(fixture_seen_in(stage, placed));
    }
    out
}

/// One room of the staged ship, put to the worked-face question.
///
/// **It is asked of every room of the staged ship and not only of the
/// staged room**, which is the same argument [`scene`] makes about a
/// doorway and it has to be made again here. A latch is drawn by the room
/// with the lower id and it hangs on THAT room's side of the wall, so the
/// only latches in the game hang in a cabin — and the roster's own cabin
/// is a yard-fresh one with nothing alongside it, which has no seam to
/// part and therefore no latch at all. Asked of the staged room alone,
/// this family would have swept fifteen stations and never once looked at
/// the control the owner reported. The finding is filed under the room
/// the face stands in, so the same cabin seen from fifteen ships answers
/// once.
fn fixture_seen_in(stage: &Stage, placed: &Placed) -> Vec<Finding> {
    let faces = worked_faces(placed);
    if faces.is_empty() {
        return Vec::new();
    }
    let stocked: Vec<Berth> = berths(&stage.rooms, placed)
        .into_iter()
        .filter(|berth| berth.class == Tile::Stock)
        .collect();
    let mut out = Vec::new();
    for (what, face, inward) in faces {
        // Nothing holds itself up and nothing hides itself: a fixture's
        // own brasswork and a latch's own plate come off the list.
        let stem = what.split_whitespace().next().unwrap_or(&what).to_owned();
        for hung in scene(stage)
            .into_iter()
            .filter(|drawn| drawn.character && !drawn.what.starts_with(&stem))
        {
            let cover = across(face, inward, hung.body);
            if cover <= OCCLUDE_BITE {
                continue;
            }
            out.push(Finding {
                room: room_name(stage, placed),
                rule: FIXTURE_SEEN,
                offender: hung.what.clone(),
                detail: format!(
                    "stands across {:.0}% of {what}, which is a thing a hand has to \
                     find and work",
                    cover * 100.0
                ),
            });
        }
        let mut hidden: Vec<((u8, u8), Station, f32)> = stocked
            .iter()
            .map(|berth| (berth.cell, berth.station, across(face, inward, berth.air)))
            .filter(|(_, _, cover)| *cover > OCCLUDE_BITE)
            .collect();
        if hidden.is_empty() {
            continue;
        }
        hidden.sort_by(|a, b| b.2.total_cmp(&a.2));
        let cells: Vec<(u8, u8)> = hidden.iter().map(|(cell, _, _)| *cell).collect();
        let (worst, station, cover) = hidden[0];
        let (x, y) = worst;
        out.push(Finding {
            room: room_name(stage, placed),
            rule: FIXTURE_SEEN,
            offender: what,
            detail: format!(
                "is read through the air {} of the room's own stock berth(s) spend: {}; \
                 worst is ({x}, {y}) on {station:?}, standing across {:.0}% of it",
                hidden.len(),
                some_cells(&cells),
                cover * 100.0
            ),
        });
    }
    out
}

/// **Everything one room hangs**, with whatever each body says holds it
/// up: the station's own furniture and the hardware bolted into its
/// doorways, posed through the very describers the runtime builds them
/// from.
///
/// A seam's frame is drawn once, by the room with the lower id, and it
/// is asked about there — the joint is a world plane and reads the same
/// from either side, so asking twice would report one defect twice.
fn hung(placed: &Placed) -> Vec<Drawn> {
    let mut out = fittings(placed);
    out.extend(seam_furniture(placed).into_iter().map(|(_, drawn)| drawn));
    out
}

/// **A hung body meets whatever it says holds it up.** [`FURNITURE_SEATED`].
///
/// [`part_seated`]'s question and [`rig_seated`]'s tolerance, asked of
/// the one layer of the world that could not answer it. A rig declares
/// the chart it is berthed on because the sim berths it; a station's
/// `Fitting` is a fraction of a room's box and a doorway's hardware is
/// world units off a site, and neither of them declared anything at all
/// — so a beacon bolted to thin air and a latch floating in front of a
/// wall were invisible to every rule in this file, and the two of them
/// were found by a player looking at the screen.
///
/// **Four defects of one shape have now been found and three of them by
/// eye**, which is the whole argument for this family: the wall lamp's
/// mount pad spanning a band that began where no other wall kind's
/// began, the porthole's whole assembly 32.6 mm out in front of its
/// wall, the Guild's seizure beacon 0.58 m off the aft wall with its
/// hood face down over it, and the seam's detach latch 0.0931 m off the
/// wall it screws to. Only the porthole was catchable, and only because
/// a rig has a chart to be measured against.
///
/// **The claim is declared and then checked, never guessed at**, which
/// is the same reason [`ALLOWED`] needs no entry for [`part_seated`]:
/// there is nothing here to forgive, only things nobody claimed. A sweep
/// that inferred joints would report every bollard that happens to stand
/// near a wall, and it would be *wrong* about the things that are meant
/// to hang on nothing — the Wanderer's fourth collar and its three hum
/// rings say so in their own source, and they stay legal by saying
/// nothing.
///
/// Two readings, one tolerance ([`SEAT_GAP`], because it is the same
/// joint):
///
/// - **Daylight.** How far the body stops short of the plane it names,
///   or of the nearest body answering to the name it names. A claim on a
///   surface is one-sided on purpose: a fitting is held inside its own
///   room's box by the containment law, so it can be flush with a face
///   and never through one, and burial is not a thing it can express.
/// - **A name nothing answers to.** A seat naming a body the room does
///   not draw is its own finding, because a promise about something that
///   is not there is a promise nobody can keep.
fn furniture_seated(stage: &Stage) -> Vec<Finding> {
    let placed = &stage.placed;
    let hung = hung(placed);
    let mut out = Vec::new();
    for (nth, body) in hung.iter().enumerate() {
        let Some(claim) = &body.seat else { continue };
        let (gap, seat) = match claim {
            Claim::Plane(what, at, toward) => {
                (short_of(*at, *toward, body.body), (*what).to_owned())
            }
            Claim::On(name) => {
                // Several bodies may answer to one name — a rivet round
                // a leaf cut in two by the leaf beside it meets whichever
                // half it reaches — so the reading is the smallest gap to
                // any of them.
                let gap = hung
                    .iter()
                    .enumerate()
                    // **Nothing holds itself up.** Several bodies share
                    // one name where a thing is drawn several times over
                    // — a stack of cut plate, a pair of fenced portholes
                    // — and each of them is then looking for the nearest
                    // OTHER one.
                    .filter(|&(i, other)| {
                        i != nth
                            && other
                                .name
                                .as_deref()
                                .is_some_and(|had| had.starts_with(name.as_str()))
                    })
                    .map(|(_, held)| held)
                    .map(|held| body.body.apart(held.body))
                    .fold(f32::INFINITY, f32::min);
                if gap.is_infinite() {
                    out.push(Finding {
                        room: stage.name.clone(),
                        rule: FURNITURE_SEATED,
                        offender: body.what.clone(),
                        detail: format!(
                            "names \"{name}\" as what holds it up, and this room draws no \
                             such body"
                        ),
                    });
                    continue;
                }
                (gap, format!("\"{name}\""))
            }
        };
        if gap <= SEAT_GAP {
            continue;
        }
        out.push(Finding {
            room: stage.name.clone(),
            rule: FURNITURE_SEATED,
            offender: body.what.clone(),
            detail: format!(
                "stands {gap:.4} m clear of the {seat} it says holds it up, which is \
                 daylight in a joint rather than the step a joint is drawn with"
            ),
        });
    }
    out
}

// -------------------------------------------------------------- the walk --

/// One waypoint of the path a player actually takes through a room, with
/// the pose the camera stands in.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub label: &'static str,
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Whether the body is passing an aperture here, and therefore
    /// ducking (`rig::DUCK_HEIGHT`) rather than standing.
    pub stooped: bool,
}

/// **The path, per room**: in through the door, round the room, up to the
/// counter, and back out — the walk the post-mortem says nobody shot.
///
/// The path is the artifact. Judgement of what it shows is a human's or
/// an agent's; what is mechanical is that every waypoint stands somewhere
/// a body legally may, and that nothing is standing in it ([`WALK_CLEAR`]).
#[must_use]
pub fn walk(placed: &Placed) -> Vec<Step> {
    let mid = (placed.lo + placed.hi) * 0.5;
    let look = |from: Vec3, at: Vec3| {
        let d = at - from;
        ((-d.x).atan2(-d.z), d.y.atan2(d.xz().length().max(1e-4)))
    };
    let mut steps = Vec::new();
    let mut step = |label, eye: Vec3, at: Vec3, stooped| {
        let (yaw, pitch) = look(eye, at);
        steps.push(Step {
            label,
            eye,
            yaw,
            pitch,
            stooped,
        });
    };
    // What a walk LOOKS at, arriving: the aft band, where a calling
    // room keeps its goods and sets its counter. Aiming at the room's
    // own centroid pointed the camera at its feet, and a filmstrip of
    // eight pictures of the deck is a filmstrip of nothing.
    let aft = room::wall_out(0, placed.yaw);
    let heart = Vec3::new(mid.x, placed.lo.y + 0.85, mid.z)
        + aft * ((placed.hi - placed.lo) * aft.abs()).length() * 0.5;
    // The door, from the outside: what the room looks like arriving.
    //
    // A MATED door where there is one, because that is the door a body
    // can actually be standing outside of — an unmated one is a plate
    // drawn shut with the void behind it, and a walk that started there
    // would start nowhere.
    let door = placed
        .ports
        .iter()
        .find(|site| site.is_door() && site.mate.is_some())
        .or_else(|| placed.ports.iter().find(|site| site.is_door()));
    if let Some(site) = door {
        // Only the jamb itself is stooped: an aperture is two courses of
        // the cargo grid, so a body bends passing through it and stands
        // up again on either side (`room::Envelope::ducking`).
        let jamb = Vec3::new(site.leaf.x, crate::rig::DUCK_HEIGHT, site.leaf.z);
        let stood = Vec3::new(site.leaf.x, EYE_HEIGHT, site.leaf.z);
        step("approach", stood + site.out * 1.1, heart, false);
        step("stoop", jamb, heart, true);
        step("entry", stood - site.out * 0.7, heart, false);
    }
    let inset = Vec3::new(room::BODY * 1.6, 0.0, room::BODY * 1.6);
    let lo = placed.lo + inset;
    let hi = placed.hi - inset;
    let eye = |x: f32, z: f32| Vec3::new(x, EYE_HEIGHT, z);
    step("middle", eye(mid.x, mid.z), heart, false);
    // Round the room: each flank in turn, looking at the wall it is.
    for (label, at) in [
        ("port-flank", Vec3::new(placed.lo.x, 0.85, mid.z)),
        ("starboard-flank", Vec3::new(placed.hi.x, 0.85, mid.z)),
        ("aft-band", Vec3::new(mid.x, 0.85, placed.hi.z)),
        ("front-band", Vec3::new(mid.x, 0.85, placed.lo.z)),
    ] {
        let toward =
            (Vec3::new(at.x, 0.0, at.z) - Vec3::new(mid.x, 0.0, mid.z)).normalize_or_zero();
        let stand = Vec3::new(mid.x, 0.0, mid.z) + toward * 0.45;
        step(
            label,
            eye(stand.x.clamp(lo.x, hi.x), stand.z.clamp(lo.z, hi.z)),
            at,
            false,
        );
    }
    // And up to the counter, which is where a deal is actually struck.
    if let Some(frame) = handshake_frame(placed) {
        let out = frame.rot * Vec3::Z;
        let stand = frame.mid + out * 0.75;
        step(
            "counter",
            eye(stand.x.clamp(lo.x, hi.x), stand.z.clamp(lo.z, hi.z)),
            frame.mid,
            false,
        );
    }
    if let Some(site) = door {
        let stood = Vec3::new(site.leaf.x, EYE_HEIGHT, site.leaf.z);
        step("leaving", stood - site.out * 0.7, stood + site.out, false);
    }
    steps
}

/// **The walked path stands in air.** Every waypoint must be somewhere a
/// body legally is, and nothing the room draws may be standing in it —
/// which is the eye walking into a fitting, a defect a still shot from
/// that very pose would render as an interesting abstract.
fn walk_clear(stage: &Stage) -> Vec<Finding> {
    let envelope = room::walk_boxes(&stage.all);
    let scene = scene(stage);
    let mut stood: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    let mut out = Vec::new();
    for step in walk(&stage.placed) {
        let head = Box3::spanning(
            step.eye - Vec3::new(room::BODY, 0.18, room::BODY) * 0.5,
            step.eye + Vec3::new(room::BODY, 0.18, room::BODY) * 0.5,
        );
        if !envelope.holds(step.eye) {
            let stance = if step.stooped { "stooping" } else { "standing" };
            out.push(Finding {
                room: stage.name.clone(),
                rule: WALK_CLEAR,
                offender: format!("waypoint '{}'", step.label),
                detail: format!("{stance} at {} stands outside the walk envelope", step.eye),
            });
        }
        for drawn in &scene {
            if drawn.character && head.clips(drawn.body).is_some() {
                stood
                    .entry(drawn.what.clone())
                    .or_default()
                    .push(step.label);
            }
        }
    }
    out.extend(stood.into_iter().map(|(what, labels)| Finding {
        room: stage.name.clone(),
        rule: WALK_CLEAR,
        offender: what,
        detail: format!(
            "stands in the eye's way at waypoint(s) {}",
            labels.join(", ")
        ),
    }));
    out
}

// ------------------------------------------------------------- the report --

/// The whole sweep, as the text `--gauntlet` prints and this module's own
/// tests read back. One line per violation, grouped by rule, and a tally.
#[must_use]
pub fn report(findings: &[Finding]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "gauntlet: {} findings", findings.len());
    for rule in RULES {
        let mine: Vec<&Finding> = findings.iter().filter(|f| f.rule == rule).collect();
        let _ = writeln!(out, "\n-- {rule} ({}) --", mine.len());
        for finding in mine {
            let _ = writeln!(out, "{finding}");
        }
    }
    out
}

/// The sweep as docket lines — `room | rule | offender`, grouped by
/// room with a heading, which is what `--gauntlet-docket` prints and what
/// `src/gauntlet.docket` is made of. Regenerating beats transcribing.
#[must_use]
pub fn as_docket(findings: &[Finding]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut room = String::new();
    let mut seen: Vec<(String, &str, String)> = findings.iter().map(Finding::key).collect();
    seen.sort();
    seen.dedup();
    for (at, rule, offender) in seen {
        if at != room {
            let _ = writeln!(out, "\n# ---- {at} ----");
            room.clone_from(&at);
        }
        let _ = writeln!(out, "{at} | {rule} | {offender}");
    }
    out
}

/// Whatever the sweep found that the docket does not already carry —
/// what makes `--gauntlet` exit non-zero, and the only thing that should.
/// A docketed defect is a defect somebody already wrote down.
#[must_use]
pub fn undocketed(findings: &[Finding]) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|finding| {
            let (room, rule, offender) = finding.key();
            !docket()
                .iter()
                .any(|(a, b, c)| *a == room && *b == rule && *c == offender)
        })
        .collect()
}

// ---------------------------------------------------------- the pixel half --

/// How many frames the flicker detector holds the camera still for. Long
/// enough that an every-other-frame lamp cannot hide in the sampling, and
/// short enough that the whole walk finishes inside a CI step.
pub const FLICKER_FRAMES: usize = 10;

/// How much of the picture may change between two frames from ONE pose
/// before it is a flicker: a fraction of pixels that moved by more than
/// [`FLICKER_STEP`]. Not zero, because bloom and the CRT rasterisers are
/// allowed to breathe; nowhere near a lamp turning off.
pub const FLICKER_TOL: f32 = 0.02;

/// How far one channel must move to count as having moved, 0..=255.
pub const FLICKER_STEP: u8 = 10;

/// How many stand-off samples the light-pop detector takes along the
/// approach to a room's fixture.
pub const APPROACH_STEPS: usize = 12;

/// How much the room's mean brightness may change between two adjacent
/// stand-offs, as a fraction of the brightest sample. A light that fades
/// with distance moves smoothly; a light that is CULLED at a distance
/// steps, and a step is what this is looking for.
pub const POP_TOL: f32 = 0.18;

/// The luma at which a texel counts as **read** rather than as ground,
/// `0..=1`.
///
/// Mean brightness cannot answer "is anything visible here": pure black
/// is banned, so the darkest frame the game can draw still means out at
/// around 0.037 and a room going from unreadable to readable moves it by
/// a fifth. The fraction of the picture standing clear of that ground
/// does answer it, and this is where clear begins — comfortably over the
/// starlight floor an unlit hull settles at, well under anything a lamp
/// or an emissive puts on a surface.
pub const READ_FLOOR: f32 = 0.10;

/// How much of a frame must stand clear of the ground for the room in it
/// to be **legible at all**, as a fraction of the picture.
///
/// The measurement it is argued from, on the furnace with its fire out
/// (`--underway --view burner`): the room read 0.0016 before its tape
/// carried the lights-out floor, and 0.069 after — and the 0.0016 was
/// the version corner and the crosshair, which is to say the room itself
/// contributed nothing. This sits between them nearer the floor than the
/// finding, because it is a *nothing drew* alarm and not a brightness
/// target: 12× of headroom over a black frame, 3× of margin under the
/// dark furnace as it now stands.
///
/// Spent by the dark-room guard in `tests`, which is its whole job — the
/// same arrangement `rig::layer::STEP` has with the ladder test.
#[allow(dead_code)]
pub const ROOM_READS: f32 = 0.02;

/// The poses the flicker and light-pop passes are shot from: the walk's
/// own middle for the still, and a straight line backing off its counter
/// (or its middle, in a room with no fixture) for the approach.
#[must_use]
pub fn approach(placed: &Placed) -> Vec<(Vec3, Vec3)> {
    let steps = walk(placed);
    let anchor = steps
        .iter()
        .find(|step| step.label == "counter")
        .or_else(|| steps.iter().find(|step| step.label == "middle"));
    let Some(anchor) = anchor else {
        return Vec::new();
    };
    let at =
        anchor.eye + Quat::from_euler(EulerRot::YXZ, anchor.yaw, anchor.pitch, 0.0) * Vec3::NEG_Z;
    // Backing off is a WALK, not a lift: the eye keeps its height and
    // stays inside the room. An approach that rose as it retreated left
    // through the ceiling and reported the hull going past as a light
    // switching off — which is the detector finding its own footprints.
    let back = ((anchor.eye - at) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    let inset = Vec3::new(room::BODY, 0.0, room::BODY);
    let (lo, hi) = (placed.lo + inset, placed.hi - inset);
    let span = (hi - lo).max(Vec3::ZERO).length();
    let mut stands: Vec<(Vec3, Vec3)> = (0..APPROACH_STEPS)
        .map(|i| {
            let reach = (i as f32) / (APPROACH_STEPS - 1) as f32 * span;
            let eye = anchor.eye + back * reach;
            (
                Vec3::new(
                    eye.x.clamp(lo.x.min(hi.x), lo.x.max(hi.x)),
                    EYE_HEIGHT,
                    eye.z.clamp(lo.z.min(hi.z), lo.z.max(hi.z)),
                ),
                at,
            )
        })
        .collect();
    // The clamp piles the tail of a short room's approach against its
    // own wall; a stand-off sampled twice is a frame rendered twice and
    // a step of exactly zero, so the duplicates come off.
    stands.dedup_by(|a, b| a.0.distance(b.0) < 1e-3);
    stands
}

// ------------------------------------------------------------ the docket --

/// **The work order.** Every violation the gauntlet catches today, one
/// per line, as `room | rule | offender` — the identity of a defect
/// without the numbers, so a retune that moves one by a millimetre does
/// not read as a new one.
///
/// It lives in a file of its own rather than in a table here because it
/// is a **list somebody is meant to work through**: strike the line, run
/// `cargo test -p cabin`, and the sweep will tell you whether the thing
/// is actually gone. Blank lines and `#` comments are ignored, so the
/// fixer can group and annotate as they go.
///
/// It is not an allowlist. The sweep is asserted EQUAL to it
/// ([`tests::the_gauntlet_finds_exactly_the_docket`]), so a new defect
/// fails the build and a fixed one fails it too.
///
/// **It is empty today, and an empty work order is not a finished
/// harness.** Three layers of the world have been described and swept,
/// and every one of them was invisible until somebody wrote the
/// description down. What that says is that the next defect is in the
/// layer nobody has thought to describe yet, and the file itself says so
/// at more length.
const DOCKET_TEXT: &str = include_str!("gauntlet.docket");

/// The docket, parsed.
#[must_use]
pub fn docket() -> Vec<(String, String, String)> {
    DOCKET_TEXT
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.splitn(3, " | ");
            Some((
                parts.next()?.trim().to_owned(),
                parts.next()?.trim().to_owned(),
                parts.next()?.trim().to_owned(),
            ))
        })
        .collect()
}

/// **Where the coplanar detector is wrong**, pair by pair, with the
/// reason it is wrong — `(room, pair, why)`.
///
/// **It is empty today, and that is a finding of its own.** Every case
/// that has come up has been answered by a truer detector rather than a
/// forgiven pair — the tighter [`FIGHT_EPS`], and then [`Faces`], which
/// knows that a round body has no face round its flank and that a body
/// has to land squarely on a world axis before its box has six.
/// docs/GAUNTLET.md has that argument at length.
///
/// A pair added here needs its reason written beside it, and the reason
/// has to be about the geometry rather than about the number.
pub const ALLOWED: &[(&str, &str, &str)] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    /// **The seat family is asked about real joints, and it answers when
    /// one opens.**
    ///
    /// The guard against a declared-claim family going quietly
    /// toothless. A family nobody declares anything for is a green tick
    /// that means nothing, so this counts the claims; a name pointing at
    /// a body the rig does not draw is a promise nobody can keep, so
    /// this refuses one; and the reading itself has to tell a joint from
    /// daylight, so it is exercised at both ends of its own tolerance.
    ///
    /// Asked in the undressed frame, deliberately: joints are claims the
    /// whitebox description makes about its own parts, the family skips
    /// dressed kinds for exactly that reason, and a manifest dressing
    /// the Couch must not make the Couch's own joints unaskable.
    #[test]
    fn the_seat_family_is_asked_about_real_joints_and_answers_when_one_opens() {
        let mut claims = 0_u32;
        let mut kinds = 0_u32;
        for kind in Kind::ALL {
            let seats = crate::pieces::seats(kind);
            if seats.is_empty() {
                continue;
            }
            kinds += 1;
            for seat in &seats {
                claims += 1;
                let whitebox = Dressings::default();
                let drawn = Screens::BOTH.into_iter().any(|screens| {
                    rig_forms(kind).into_iter().any(|showing| {
                        rig_parts(&whitebox, kind, screens, showing)
                            .iter()
                            .any(|part| part.what == seat.on)
                    })
                });
                assert!(
                    drawn,
                    "{kind:?}: \"{}\" names \"{}\" as what holds it, and no such part \
                     is drawn",
                    seat.name, seat.on,
                );
            }
        }
        assert!(kinds >= 6, "only {kinds} kind(s) claim a joint at all");
        assert!(claims >= 12, "only {claims} joint(s) are claimed at all");
        // And the reading: touching is a joint, a step off is a joint,
        // a body's width off is daylight.
        let seat = Box3 {
            lo: Vec3::ZERO,
            hi: Vec3::splat(0.1),
        };
        let at = |z: f32| Box3 {
            lo: Vec3::new(0.0, 0.0, z),
            hi: Vec3::new(0.1, 0.1, z + 0.1),
        };
        assert!(seat.apart(at(0.05)) <= 0.0, "overlapping is meeting");
        assert!(seat.apart(at(0.1)) <= 0.0, "touching is meeting");
        assert!(
            seat.apart(at(SEAT_GAP.mul_add(0.5, 0.1))) <= SEAT_GAP,
            "a step is a joint"
        );
        assert!(
            seat.apart(at(SEAT_GAP.mul_add(2.0, 0.1))) > SEAT_GAP,
            "daylight is not a joint",
        );
        // Off to one side entirely is not a joint either, however close
        // the two get on the axis they are stacked along.
        let beside = Box3 {
            lo: Vec3::new(1.0, 0.0, 0.1),
            hi: Vec3::new(1.1, 0.1, 0.2),
        };
        assert!(
            seat.apart(beside) > SEAT_GAP,
            "a body across the room is not a seat"
        );
    }

    /// **The furniture family is asked about every room, and it answers
    /// when a body lifts off its seat.**
    ///
    /// The same guard the seat and chart families carry, on the layer
    /// that had no claims at all until now. Three clauses:
    ///
    /// - **It is asked.** Every room in the roster declares seats, and
    ///   both kinds of claim are spent across the game — a face of the
    ///   room's own box, and a body beside it by name. A family nobody
    ///   declares anything for is a green tick that means nothing.
    /// - **Every name answers.** A claim naming a body its room does not
    ///   draw is a promise nobody can keep, and there are none in the
    ///   tree.
    /// - **The reading is live.** This is the clause that matters, and
    ///   it is the one a test written out of the implementation's own
    ///   branch would pass whatever the number said. Every seated body
    ///   in the game is lifted a hand's breadth off the thing it names,
    ///   in the direction its own claim points, and the reading has to
    ///   turn from a joint into daylight — counted, so a family that
    ///   quietly stopped measuring would fail here rather than go green.
    #[test]
    fn the_furniture_family_is_asked_about_every_room_and_answers_when_a_body_lifts_off() {
        /// A hand's breadth: an order over [`SEAT_GAP`], and the size of
        /// gap every defect this family was written for turned out to be.
        const LIFT: f32 = 0.1;
        let (mut planes, mut siblings, mut rooms, mut caught) = (0_u32, 0_u32, 0_u32, 0_u32);
        for stage in roster() {
            let bodies = hung(&stage.placed);
            let mut here = 0_u32;
            for (nth, body) in bodies.iter().enumerate() {
                let Some(claim) = &body.seat else { continue };
                here += 1;
                let lift = |by: Vec3| Box3 {
                    lo: body.body.lo + by,
                    hi: body.body.hi + by,
                };
                match claim {
                    Claim::Plane(_, at, toward) => {
                        planes += 1;
                        let held = short_of(*at, *toward, body.body);
                        let off = short_of(*at, *toward, lift(-*toward * LIFT));
                        assert!(
                            off > LIFT.mul_add(0.9, held),
                            "{}: {} reads the same after being lifted off its own plane",
                            stage.name,
                            body.what
                        );
                        if held <= SEAT_GAP && off > SEAT_GAP {
                            caught += 1;
                        }
                    }
                    Claim::On(name) => {
                        siblings += 1;
                        let seats: Vec<Box3> = bodies
                            .iter()
                            .enumerate()
                            .filter(|&(i, other)| {
                                i != nth
                                    && other
                                        .name
                                        .as_deref()
                                        .is_some_and(|had| had.starts_with(name.as_str()))
                            })
                            .map(|(_, other)| other.body)
                            .collect();
                        assert!(
                            !seats.is_empty(),
                            "{}: {} names \"{name}\" as what holds it up and this room \
                             draws no such body",
                            stage.name,
                            body.what
                        );
                        let gap = |box_: Box3| {
                            seats
                                .iter()
                                .map(|seat| box_.apart(*seat))
                                .fold(f32::INFINITY, f32::min)
                        };
                        let held = gap(body.body);
                        let off = gap(lift(Vec3::Y * LIFT + Vec3::X * LIFT));
                        if held <= SEAT_GAP && off > SEAT_GAP {
                            caught += 1;
                        }
                    }
                }
            }
            if here > 0 {
                rooms += 1;
            }
        }
        assert!(rooms >= 15, "only {rooms} room(s) declare a seat at all");
        assert!(planes >= 100, "only {planes} claim(s) name a surface");
        assert!(
            siblings >= 120,
            "only {siblings} claim(s) name a body beside them"
        );
        assert!(
            caught >= 140,
            "only {caught} seated bodies read as unseated after being lifted a hand's \
             breadth off what holds them"
        );
        // And the reading itself, at both ends of its own tolerance and
        // in both directions, because a joint may be spent by going INTO
        // the thing that holds it.
        let plane = (Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
        let at = |top: f32| Box3 {
            lo: Vec3::new(0.0, top - 0.1, 0.0),
            hi: Vec3::new(0.1, top, 0.1),
        };
        assert!(
            short_of(plane.0, plane.1, at(1.0)) <= SEAT_GAP,
            "flush is seated"
        );
        assert!(
            short_of(plane.0, plane.1, at(1.0 + LIFT)) < 0.0,
            "buried is seated, and reads as buried"
        );
        assert!(
            short_of(plane.0, plane.1, at(SEAT_GAP.mul_add(-0.5, 1.0))) <= SEAT_GAP,
            "a builder's step off is seated"
        );
        assert!(
            short_of(plane.0, plane.1, at(1.0 - LIFT)) > SEAT_GAP,
            "a hand's breadth of daylight is not a joint"
        );
    }

    /// **The chart family is asked about every kind, on every chart it
    /// may be berthed on, and it answers when a rig leaves its plane.**
    ///
    /// The guard against a family that is green because it never asked.
    /// It counts the questions — every kind has a chart and no kind has
    /// none — checks that the chart it is asked about is the arbiter's
    /// answer and not a second copy of the table, and then moves a rig
    /// off its own plane both ways and reads the finding back.
    ///
    /// The last clause is the one that matters. A reading built out of
    /// the implementation's own branch would pass this whatever the
    /// number said, so it is exercised against a body placed by hand at
    /// each end of the tolerance: flush is seated, a builder's step of
    /// burial is seated, and a centimetre of air is not.
    #[test]
    fn every_kind_is_asked_which_chart_it_stands_on_and_answers_when_it_leaves_one() {
        let mut asked = 0_u32;
        for kind in Kind::ALL {
            let charts: Vec<Surf> = Surf::ALL
                .into_iter()
                .filter(|&surf| chart_joint(kind, surf).is_some())
                .collect();
            assert!(
                !charts.is_empty(),
                "{kind:?} is berthed on no chart at all, so nothing asks what it stands on"
            );
            for surf in Surf::ALL {
                assert_eq!(
                    chart_joint(kind, surf).is_some(),
                    mount_accepts(kind.mount(), surf),
                    "{kind:?} on {surf:?}: the family and the arbiter disagree about \
                     whether the berth exists"
                );
            }
            asked += u32::try_from(charts.len()).unwrap_or(0);
        }
        assert!(
            asked >= u32::try_from(Kind::ALL.len()).unwrap_or(0),
            "only {asked} kind-and-chart question(s) are asked of {} kinds",
            Kind::ALL.len()
        );
        // Every mount class is actually represented, so a family that
        // quietly answered `None` for the deckhead would be caught.
        for want in [Mount::Floor, Mount::Ceiling, Mount::Wall] {
            assert!(
                Kind::ALL.into_iter().any(|kind| kind.mount() == want),
                "no kind is berthed on a {want:?} chart, so that clause is vacuous"
            );
        }
        // And the reading, at both ends of its own tolerance. A deck
        // sits at the plane the joint names; a body is walked off it.
        let joint = chart_joint(Kind::CometIce, Surf::Floor).expect("a crate stands on a deck");
        let sole = |y: f32| Box3 {
            lo: Vec3::new(-0.1, y, -0.1),
            hi: Vec3::new(0.1, y + 0.2, 0.1),
        };
        let deck = joint.at.y;
        let bury = crate::pieces::SOLE_BURY * crate::pieces::RIG_UNIT;
        assert!(
            joint.short_of(sole(deck)) <= 0.0,
            "a sole on the plane is seated"
        );
        assert!(
            joint.short_of(sole(deck - bury)) < 0.0,
            "a sole a builder's step into the plane is seated"
        );
        assert!(
            joint.short_of(sole(SEAT_GAP.mul_add(0.5, deck))) <= SEAT_GAP,
            "a sole half a tolerance up is still seated"
        );
        assert!(
            joint.short_of(sole(deck + 0.01)) > SEAT_GAP,
            "a centimetre of daylight is not a joint"
        );
        // The whole family, on a rig walked off its plane: the ceiling
        // lamp's canopy meets its deckhead today, and a millimetre is
        // not what saves it.
        let lamp = chart_joint(Kind::CeilingLamp, Surf::Ceiling).expect("a pendant hangs");
        let canopy = rig_scene(
            Dressings::shipped(),
            Kind::CeilingLamp,
            Screens::LIVE,
            Under::Rig,
        )
        .into_iter()
        .map(|drawn| lamp.short_of(drawn.body))
        .fold(f32::INFINITY, f32::min);
        assert!(
            canopy <= 0.0,
            "the ceiling lamp's canopy stops {canopy} m short of its own deckhead",
        );
        assert!(
            !rig_seated(Dressings::shipped())
                .iter()
                .any(|finding| finding.offender.starts_with("CeilingLamp")),
            "the ceiling lamp is seated, so the family must not be reporting it"
        );
    }

    /// **A wall berth's air is what a rig reaches into the room**, not
    /// how thick its box is.
    ///
    /// The band every kind is composed within begins just behind the
    /// berth plane, so a wall rig's box straddles its own chart. Reading
    /// the box's THICKNESS off the cell face put the berth's air a near
    /// face too far into the room — 31 mm of aisle claimed that no body
    /// reaches into, and 31 mm of wall forgotten that every body sinks
    /// into — and everything measured against that air, the occlusion
    /// window most of all, sat that far out with it.
    #[test]
    fn a_wall_berths_air_is_what_a_rig_reaches_into_the_room() {
        let unit = crate::pieces::RIG_UNIT;
        let reach = crate::pieces::RIG_FAR * unit;
        let thickness = (crate::pieces::RIG_FAR - crate::pieces::RIG_NEAR) * unit;
        assert!(
            (thickness - reach).abs() > 0.02,
            "the band no longer straddles its chart, so this guard is vacuous",
        );
        let stage = roster()
            .into_iter()
            .find(|stage| stage.name == "cabin")
            .expect("the cabin is on the roster");
        let walls: Vec<Berth> = berths(&stage.rooms, &stage.placed)
            .into_iter()
            .filter(|berth| !matches!(berth.station, Station::BayFloor | Station::BayCeiling))
            .collect();
        assert!(!walls.is_empty(), "the cabin has walls to berth on");
        for berth in walls {
            let air = berth.air.span().dot(berth.inward.abs());
            assert!(
                (air - reach).abs() < 1e-4,
                "berth {:?} on {:?} spends {air} m of air where a rig reaches {reach} m",
                berth.cell,
                berth.station,
            );
        }
    }

    /// **The gauntlet finds exactly the docket.** New defect: fails.
    /// Fixed defect: fails, until the line comes out. That is the whole
    /// contract, and it is why the docket is a work order rather than a
    /// baseline nobody ever looks at again.
    #[test]
    fn the_gauntlet_finds_exactly_the_docket() {
        let found = sweep();
        let mut keys: Vec<(String, String, String)> = found
            .iter()
            .map(|finding| {
                let (room, rule, offender) = finding.key();
                (room, rule.to_owned(), offender)
            })
            .collect();
        keys.sort();
        keys.dedup();
        let mut want = docket();
        want.sort();
        want.dedup();
        let fresh: Vec<&(String, String, String)> =
            keys.iter().filter(|key| !want.contains(key)).collect();
        let gone: Vec<&(String, String, String)> =
            want.iter().filter(|key| !keys.contains(key)).collect();
        assert!(
            fresh.is_empty(),
            "the gauntlet caught {} thing(s) the docket does not carry \
             (add them, or fix them):\n{}",
            fresh.len(),
            report(&found)
        );
        assert!(
            gone.is_empty(),
            "the docket carries {} thing(s) the gauntlet no longer catches \
             — strike the lines from src/gauntlet.docket:\n{gone:#?}",
            gone.len()
        );
    }

    /// **Whatever the manifest in this repository dresses is measured as
    /// its declaration.** The other half of the docket contract: the
    /// docket is asserted equal to a sweep, and the sweep reads a file
    /// somebody edits by hand. This guard once pinned that file to
    /// dressing nothing; the first real `dresses` line retired that
    /// premise, and what is worth holding now is the opposite failure —
    /// a declaration that parses, resolves to nothing, and falls back to
    /// the whitebox without anyone noticing, so that "the docket is
    /// empty" quietly stops being a claim about the declared body at all.
    #[test]
    fn every_kind_the_shipped_manifest_dresses_is_swept_as_its_declaration() {
        let shipped = Dressings::shipped();
        for kind in Kind::ALL {
            if shipped.of(kind).is_none() {
                continue;
            }
            let drawn = dressed_scene(shipped, kind)
                .expect("a kind the manifest dresses draws its declaration");
            assert_eq!(
                drawn.len(),
                1,
                "{kind:?} is dressed, so the sweep measures one declared body \
                 in place of its whitebox parts, not {}",
                drawn.len()
            );
        }
    }

    /// **The whitebox stays clean under the dress.** A dressed kind's
    /// whitebox parts leave the shipped sweep — the manifest says the
    /// declared body is what the kind IS — so a whitebox regression in a
    /// dressed kind is a defect the ratchet can no longer see. This
    /// sweeps with nothing dressed and refuses anything the docket does
    /// not carry. One direction only, on purpose: a docket line that
    /// exists only when dressed is legitimately absent from this sweep,
    /// but a fresh finding here is a whitebox defect hiding behind a
    /// purchased mesh.
    #[test]
    fn the_whitebox_stays_clean_under_the_dress() {
        let found = sweep_dressed(&Dressings::default());
        let mut keys: Vec<(String, String, String)> = found
            .iter()
            .map(|finding| {
                let (room, rule, offender) = finding.key();
                (room, rule.to_owned(), offender)
            })
            .collect();
        keys.sort();
        keys.dedup();
        let want = docket();
        let fresh: Vec<&(String, String, String)> =
            keys.iter().filter(|key| !want.contains(key)).collect();
        assert!(
            fresh.is_empty(),
            "the undressed sweep caught {} thing(s) the docket does not carry — \
             whitebox defects the dressed sweep cannot see:\n{}",
            fresh.len(),
            report(&found)
        );
    }

    /// **A declared body that leaves its berth is a finding, and the
    /// same one a whitebox part would be.** The non-vacuity of the
    /// family above.
    ///
    /// The shipped manifest's own declarations are truthful, so the
    /// shipped sweep can only ever be green about this — and a rule that
    /// can only pass is not a rule. So a manifest is written here, with
    /// the two ways a
    /// declaration can put a body somewhere it does not belong, and the
    /// sweep has to say so: a `fill` bigger than the cells the sim gave
    /// the kind (`face-fits`, the paint the aim cannot follow), and an
    /// `offset` that shoves the body out of the one-cell band every rig
    /// is composed within (`berth-clear`, which on a wall kind is the air
    /// its berth spends).
    ///
    /// **And a truthful declaration is silent**, which is the other half:
    /// a family that reports every dressed kind is a family nobody can
    /// use.
    #[test]
    fn a_declared_body_that_leaves_its_berth_is_caught_like_any_other() {
        let declared = |lines: &str| {
            Dressings::read(&format!(
                "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n{lines}"
            ))
            .expect("the dialect")
        };
        let rules = |found: &[Finding]| -> Vec<String> {
            let mut out: Vec<String> = found
                .iter()
                .filter(|finding| finding.offender.contains("crate_small"))
                .map(|finding| finding.rule.to_owned())
                .collect();
            out.sort();
            out.dedup();
            out
        };

        // A body that fills its berth exactly: the identity claim, and
        // the one the manifest's own comment calls the claim most
        // imported meshes turn out to break. **Asked of every kind**,
        // because the sentence this backs is one somebody will act on —
        // that adding a `dresses` line to a truthful asset costs the
        // docket nothing — and it has to be true of the tall kinds and
        // the wall-hung ones as well as of a crate.
        for kind in Kind::ALL {
            let identity = Dressings::read(&format!(
                "[asset.crate_small]\ndresses = \"cargo/{}\"\nfill = [1.0, 1.0, 1.0]\n",
                crate::art::snake(kind)
            ))
            .expect("the dialect");
            let found = sweep_dressed(&identity);
            assert!(
                rules(&found).is_empty(),
                "{kind:?} dressed by a body that fills its own berth was reported: {:?}",
                rules(&found)
            );
        }
        let honest = sweep_dressed(&declared("fill = [1.0, 1.0, 1.0]\n"));

        // Half again as wide as the cells the sim gave it.
        let wide = sweep_dressed(&declared("fill = [1.5, 1.0, 1.0]\n"));
        assert!(
            rules(&wide).contains(&FACE_FITS.to_owned()),
            "a body reaching outside its own cells went unreported: {:?}",
            rules(&wide)
        );

        // The right size, shoved a whole berth deeper than the band.
        let sunk = sweep_dressed(&declared("offset = [0.0, 0.0, 1.6]\n"));
        assert!(
            rules(&sunk).contains(&BERTH_CLEAR.to_owned()),
            "a body shoved out of the band every rig is composed within went \
             unreported: {:?}",
            rules(&sunk)
        );

        // And the whitebox's own parts stop being measured for a kind
        // something else now draws: the two are one object, not two.
        assert!(
            !honest
                .iter()
                .any(|finding| finding.offender.starts_with("SuspiciousCrate ")),
            "a dressed kind was measured as a purchased body AND as its whitebox"
        );
    }

    /// The roster is the whole game: twelve stations, three event rooms,
    /// and the ship's own two. A room nobody sweeps is a room nobody
    /// checks, which is how this pass started.
    #[test]
    fn the_roster_is_every_room_the_game_has() {
        let stages = roster();
        assert_eq!(stages.len(), poi::HOSTS.len() + 2, "a room went unswept");
        for host in poi::HOSTS {
            assert!(
                stages.iter().any(|stage| stage.name == name_of(host)),
                "{host:?} is not on the gauntlet's roster"
            );
        }
        for own in ["cabin", "burner"] {
            assert!(stages.iter().any(|stage| stage.name == own));
        }
    }

    /// **The board is loaded.** Defect reason three, closed: every room
    /// on the roster has cargo standing in it, so a fitting that clips
    /// through a crate has a crate to clip through.
    #[test]
    fn every_room_is_swept_with_cargo_in_it() {
        for stage in roster() {
            let aboard = stage
                .cargo
                .iter()
                .filter(
                    |piece| matches!(piece.loc, Loc::Hold { room, .. } if room == stage.placed.id),
                )
                .count();
            assert!(
                aboard > 0,
                "{} was swept empty, which is the defect this exists to retire",
                stage.name
            );
        }
    }

    /// Berths come out of the sim, never a hand list: every one of them
    /// is a real cell of its own room's net, never a threshold, and never
    /// the handshake's own socket.
    #[test]
    fn berths_are_derived_from_the_sim() {
        for stage in roster() {
            let berths = berths(&stage.rooms, &stage.placed);
            assert!(!berths.is_empty(), "{} has no berths at all", stage.name);
            for berth in &berths {
                let (x, y) = berth.cell;
                let tile = stage.placed.kind.tile_of(x, y);
                assert!(
                    matches!(tile, Some(tile) if tile != Tile::Threshold),
                    "{}: berth ({x}, {y}) is {tile:?}, which holds nothing",
                    stage.name
                );
                assert_ne!(
                    Some((x, y)),
                    stage.placed.kind.handshake(),
                    "{}: the fixture's own socket is not a berth",
                    stage.name
                );
                assert!(
                    berth.air.span().min_element() > 0.0,
                    "{}: berth ({x}, {y}) spends no air",
                    stage.name
                );
            }
        }
    }

    /// The walk is the walk the post-mortem named: in through the door,
    /// round the room, up to the counter. Every waypoint of it stands
    /// where a body may — the geometric half of the filmstrip, which
    /// runs headless whether or not anybody has a rasteriser.
    #[test]
    fn the_walk_enters_by_the_door_and_reaches_the_counter() {
        for stage in roster() {
            let steps = walk(&stage.placed);
            let labels: Vec<&str> = steps.iter().map(|step| step.label).collect();
            assert!(
                labels.contains(&"middle"),
                "{} has no walk at all",
                stage.name
            );
            if stage.placed.ports.iter().any(room::Site::is_door) {
                assert_eq!(
                    labels.first(),
                    Some(&"approach"),
                    "{} is not entered through its door",
                    stage.name
                );
                assert!(labels.contains(&"stoop"), "{} skips its jamb", stage.name);
            }
            assert_eq!(
                stage.placed.kind.handshake().is_some(),
                labels.contains(&"counter"),
                "{}: a room with a fixture is walked up to it",
                stage.name
            );
            let envelope = room::walk_boxes(&stage.all);
            for step in &steps {
                assert!(
                    step.eye.y > 0.0 && step.eye.y < room::CEIL_Y,
                    "{}: waypoint '{}' left the storey",
                    stage.name,
                    step.label
                );
                if step.stooped {
                    assert!(
                        envelope.ducking(step.eye) || envelope.holds(step.eye),
                        "{}: the stoop at '{}' is nowhere",
                        stage.name,
                        step.label
                    );
                }
            }
        }
    }

    /// The clip test is about bodies, not about the corners an AABB
    /// invents: a graze is not a clip, and a fitting standing squarely in
    /// a berth is.
    #[test]
    fn a_graze_is_not_a_clip() {
        let berth = Box3::spanning(Vec3::ZERO, Vec3::splat(0.55));
        let inside = Box3::spanning(Vec3::splat(0.1), Vec3::splat(0.4));
        assert!(inside.clips(berth).is_some(), "a fitting in a berth clips");
        let corner = Box3::spanning(Vec3::splat(0.548), Vec3::splat(0.9));
        assert!(
            corner.clips(berth).is_none(),
            "a corner graze is not a clip"
        );
        let beside = Box3::spanning(Vec3::new(0.55, 0.0, 0.0), Vec3::splat(1.0));
        assert!(beside.clips(berth).is_none(), "abutting is not clipping");
    }

    /// The coplanar detector answers the question it claims to: two faces
    /// looking the same way from one plane fight; two faces looking at
    /// each other are an ordinary stack, and the depth buffer settles
    /// those every time.
    #[test]
    fn only_same_facing_coplanar_pairs_are_a_fight() {
        let a = Drawn {
            what: "a".to_owned(),
            body: Box3::spanning(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.1)),
            faces: Faces::ALL,
            character: true,
            name: None,
            seat: None,
        };
        // Same top, overlapping footprint: a fight.
        let fighting = Drawn {
            what: "b".to_owned(),
            body: Box3::spanning(Vec3::new(0.2, 0.2, 0.0), Vec3::new(0.8, 1.0, 0.2)),
            faces: Faces::ALL,
            character: true,
            name: None,
            seat: None,
        };
        // Stacked: its floor is the other's ceiling, and that is fine.
        let stacked = Drawn {
            what: "c".to_owned(),
            body: Box3::spanning(Vec3::new(0.2, 1.0, 0.0), Vec3::new(0.8, 2.0, 0.2)),
            faces: Faces::ALL,
            character: true,
            name: None,
            seat: None,
        };
        assert!(pairs_fight(&a, &fighting), "coplanar tops must be caught");
        assert!(!pairs_fight(&a, &stacked), "a stack is not a fight");
    }

    /// **Nothing a station hangs stands in a berth that keeps cargo.**
    ///
    /// Three families and one cause: [`BERTH_CLEAR`] finds furniture in
    /// the air a rig fills, [`BERTH_SEEN`] finds it standing between the
    /// room and a wall berth, and [`BERTH_REACHED`] finds it fencing off
    /// the berths behind it. All three used to ask about every cell of
    /// every chart, which no arrangement of furniture could ever satisfy,
    /// because a room owned no volume of its own.
    ///
    /// A room owns its staging now, and lends it out; the three ask about
    /// what is left, which is the surface a trade happens on and the
    /// berths cargo stays in. The docket is a work order and it shrinks;
    /// a law is a thing that does not, so the sentence is kept said here
    /// after the last of its lines comes off the list.
    ///
    /// **A station hangs it**, so it is the rooms this asks about.
    /// [`BERTH_CLEAR`] is spent on the cargo too — a part outside the
    /// band every rig is composed within hangs out of the box the carry
    /// tell wraps it in, and on a wall kind it is a berth measured too
    /// shallow as well — and that half is a work order, whose last two
    /// lines came off when a floor lamp was stood on one axle and a
    /// crate's core was sunk to its own radius.
    #[test]
    fn nothing_a_station_hangs_stands_in_a_berth() {
        let found = sweep();
        let standing: Vec<&Finding> = found
            .iter()
            .filter(|finding| finding.room != RIGS)
            .filter(|finding| matches!(finding.rule, BERTH_CLEAR | BERTH_SEEN | BERTH_REACHED))
            .collect();
        assert!(
            standing.is_empty(),
            "{} thing(s) a station hangs stand in cargo's way:\n{}",
            standing.len(),
            standing
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// **Nothing a station hangs can strand a crate on its own deck.**
    ///
    /// The soft-lock the staging law could have reintroduced, refused in
    /// two clauses that between them leave it nowhere to happen.
    ///
    /// The first is structural and not asserted here because it cannot
    /// be: `room::furnish` spawns a fitting as a mesh, a material and a
    /// transform, and the pointer is cast at entities carrying a
    /// `SimSurface` (`crate::surface::track_pointer`). A station's
    /// dressing is therefore not in the aiming path at all — it cannot
    /// make a berth unpickable, only unlovely, which is exactly the
    /// trade the owner asked for when he ruled a clipping incident
    /// nobody's defect.
    ///
    /// The second is this one: **a room may not fence off its own
    /// staging.** The hull, the shell slabs, the counter and the pendant
    /// are all in the aiming path, and a staging cell no body could work
    /// past *them* would be a crate the launch gate holds forever. Swept
    /// over every room in the game, with a station's own dressing lifted
    /// out of the scene — the one thing that is allowed to be in the way.
    #[test]
    fn a_stations_dressing_is_not_in_the_aiming_path() {
        for stage in roster() {
            let hull: Vec<Drawn> = scene(&stage)
                .into_iter()
                .filter(|drawn| !drawn.character)
                .collect();
            let stances = stances(&stage);
            let mut stranded: Vec<(u8, u8)> = Vec::new();
            for berth in berths(&stage.rooms, &stage.placed) {
                if berth.class != Tile::Staging {
                    continue;
                }
                let probe = (berth.face.lo + berth.face.hi) * 0.5 + berth.inward * 0.02;
                let worked = stances.iter().any(|eye| {
                    let dir = probe - *eye;
                    let pitch = (-dir.y).atan2(dir.xz().length()).abs();
                    if dir.length() > REACH - 0.05 || pitch > PITCH_LIMIT - 0.02 {
                        return false;
                    }
                    !hull
                        .iter()
                        .any(|drawn| ray_box(*eye, dir, drawn.body).is_some_and(|t| t < 1.0 - 1e-3))
                });
                if !worked {
                    stranded.push(berth.cell);
                }
            }
            assert!(
                stranded.is_empty(),
                "{}: {} staging cell(s) its own room fences off, so a crate set \
                 down there would hold the launch forever: {}",
                stage.name,
                stranded.len(),
                some_cells(&stranded)
            );
        }
    }

    /// **No room, no rig and no doorway draws two faces on one plane.**
    /// The docket is a work order and it shrinks; a law is a thing that
    /// does not. This family is empty now — the rooms' half since the
    /// staging law, the rigs' half since thirty-two kinds were swept for
    /// the first time and fifty-one pairs of them came off, and the
    /// doorways' since the seams were described and fifty-three more
    /// came off — and the sentence it closes is worth keeping said after
    /// the last line of its own comes off the list.
    #[test]
    fn no_room_draws_two_faces_on_one_plane() {
        let found = sweep();
        let fights: Vec<&Finding> = found
            .iter()
            .filter(|finding| finding.rule == NO_COPLANAR)
            .collect();
        assert!(
            fights.is_empty(),
            "{} pair(s) of faces share a plane and a facing:\n{}",
            fights.len(),
            fights
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// **Only a box has six faces, and only a square box has any.** A
    /// cylinder meets each of the four planes round its flank along a
    /// single line, a sphere meets all six at a point, and a torus at a
    /// curve — so a boss cut round a post from one centre shares a box
    /// with it and shares no face at all, and the detector must not
    /// report the box as the body.
    ///
    /// A LEANING box is the same argument turned the other way. The
    /// lattice and the charts only ever turn by quarter turns, so a
    /// room's every side either lands squarely on a world axis or is not
    /// there; a rig's own parts lean — the perfume vial's flask stands on
    /// its corner, the gas canister's chevrons are two legs at forty-five
    /// degrees — and a leaning box's box has six sides the box itself
    /// does not draw. It keeps only the axis it leans about.
    #[test]
    fn only_a_box_has_six_faces() {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

        let sides = |faces: Faces| -> usize {
            (0..3)
                .map(|axis| {
                    usize::from(faces.has(axis, false)) + usize::from(faces.has(axis, true))
                })
                .sum()
        };
        // Every turn the lattice and the charts ever apply is a quarter
        // turn, so a cap lands on one world axis whichever way it faces.
        for rot in [
            Quat::IDENTITY,
            Quat::from_rotation_y(FRAC_PI_2),
            Quat::from_rotation_x(FRAC_PI_2),
            Quat::from_rotation_z(-FRAC_PI_2),
        ] {
            for (shape, want) in [
                (Shape::Slab, 6),
                (Shape::Post, 2),
                (Shape::Cone, 2),
                (Shape::Dome, 0),
                (Shape::Ring, 0),
            ] {
                let got = sides(Faces::of(shape, rot));
                assert_eq!(got, want, "a {shape:?} draws {got} flat sides, not {want}");
            }
        }
        // And a body turned off the axes keeps only what it is still
        // square on: a box leaning about `z` shows its two `z` ends and
        // nothing else, and a drum tilted off its own axle shows no cap.
        let lean = Quat::from_rotation_z(FRAC_PI_4);
        assert_eq!(
            sides(Faces::of(Shape::Slab, lean)),
            2,
            "a box on its corner draws two flat sides, not six"
        );
        assert!(
            Faces::of(Shape::Slab, lean).has(2, true) && !Faces::of(Shape::Slab, lean).has(0, true),
            "the two it keeps are the ends of the axis it leans about"
        );
        for shape in [Shape::Post, Shape::Cone] {
            assert_eq!(
                sides(Faces::of(shape, lean)),
                0,
                "a tilted {shape:?} presents no cap to any world axis"
            );
        }
        // And the rule that follows: one centre, two radii, no fight —
        // where two plates on the same plane are one.
        let body = Box3::spanning(Vec3::splat(-0.5), Vec3::splat(0.5));
        let round = |shape| Drawn {
            what: format!("{shape:?}"),
            body,
            faces: Faces::of(shape, Quat::IDENTITY),
            character: true,
            name: None,
            seat: None,
        };
        assert!(
            !pairs_fight(&round(Shape::Post), &round(Shape::Dome)),
            "a boss round a post shares a box, not a face"
        );
        assert!(
            pairs_fight(&round(Shape::Slab), &round(Shape::Slab)),
            "two plates on one plane are still a fight"
        );
    }

    /// **Every doorway is swept, and swept from both of its sides.**
    ///
    /// The blind spot this closes: a room's fabric was described and a
    /// station's fittings were described, and the hardware in between
    /// was built straight into the world, so nothing in a doorway could
    /// be asked about. A kind that grows a new port, or a doorway that
    /// grows a new piece of hardware, arrives on the list rather than
    /// beside it.
    ///
    /// The second clause is the one the arithmetic hides. A seam is
    /// drawn ONCE, by the room with the lower id, and it stands on the
    /// boundary the two share — so a calling room draws no frame at all,
    /// and a sweep that asked each room only what it drew would walk
    /// every station in the game past its own doorway.
    #[test]
    fn every_doorway_is_swept_from_both_sides() {
        for stage in roster() {
            let named: Vec<String> = scene(&stage).into_iter().map(|drawn| drawn.what).collect();
            let has = |mark: &str| named.iter().any(|what| what.contains(mark));
            for site in &stage.placed.ports {
                if site.half_a.length() <= 0.0 {
                    continue;
                }
                // A frame is named for the port of the room that DREW
                // it, and that is this room's port only where this room
                // is the anchor. Either way it stands in this doorway.
                let marks = match site.mate {
                    Some((_, theirs)) => {
                        vec![format!("seam[{}]", site.port), format!("seam[{theirs}]")]
                    }
                    None if site.is_door() => vec![format!("shut[{}]", site.port)],
                    None => vec![format!("hatch[{}]", site.port)],
                };
                assert!(
                    marks.iter().any(|mark| has(mark)),
                    "{}: port {} is dressed and nothing in the sweep can see it",
                    stage.name,
                    site.port
                );
            }
            assert!(
                has("tread ("),
                "{}: a doorway lays a tread and the sweep never measures one",
                stage.name
            );
        }
    }

    /// **The grid family measures something, and it fires when a body
    /// moves.**
    ///
    /// A family that swept an empty list would be green forever, which
    /// is the way every rule in this file could rot and the reason the
    /// last four passes are worth a guard rather than a promise. So
    /// three things are asserted, and none of them is the sweep's own
    /// branch read back: every room has fabric, a mated doorway has a
    /// passage in it, and a body nudged off the grid or into the next
    /// room is caught.
    #[test]
    fn the_grid_family_is_asked_about_every_room_and_answers_when_a_body_moves() {
        let notch = crate::rig::BAY_CELL / 16.0;
        for stage in roster() {
            let fabric = fabric(&stage);
            let shell: Vec<&(String, Box3, bool)> =
                fabric.iter().filter(|(_, _, shell)| *shell).collect();
            assert!(
                shell.len() >= 6,
                "{}: {} shell bodies — a room has six sides",
                stage.name,
                shell.len()
            );
            if stage.placed.ports.iter().any(|site| {
                site.mate.is_some()
                    && site.is_door()
                    && stage.placed.id <= site.mate.expect("mated").0
            }) {
                assert!(
                    fabric.iter().any(|(what, _, _)| what.contains("passage")),
                    "{}: a mated doorway and no passage in the sweep",
                    stage.name
                );
            }
            // The room's own fabric passes; the same fabric shifted by
            // half a notch does not, and shifted by a whole cell into
            // the void does not either.
            let origin = Vec3::new(room::ANCHOR.x, stage.placed.lo.y, room::ANCHOR.z);
            let wall = Vec3::splat(room::WALL_T);
            let own = Box3::spanning(stage.placed.lo - wall, stage.placed.hi + wall);
            for (what, body, _) in &shell {
                assert!(
                    off_grid(*body, origin).is_none(),
                    "{}: {what} is off the grid and the sweep is green",
                    stage.name
                );
                let nudged = Box3::spanning(body.lo + Vec3::X * notch * 0.5, body.hi);
                assert!(
                    off_grid(nudged, origin).is_some(),
                    "{}: {what} moved half a notch and nothing noticed",
                    stage.name
                );
            }
            // And the containment half is not vacuous either: shove the
            // shell a whole cell sideways and something has to leave the
            // room. It is stated over the shell rather than over each
            // slab because a slab is whatever the apertures left of it,
            // and a punched remnant from the middle of a deck can be
            // moved a cell and still be over the deck.
            let shove = |body: Box3| {
                Box3::spanning(
                    body.lo + Vec3::X * crate::rig::BAY_CELL,
                    body.hi + Vec3::X * crate::rig::BAY_CELL,
                )
            };
            assert!(
                shell
                    .iter()
                    .any(|(_, body, _)| holds(own, *body) && !holds(own, shove(*body))),
                "{}: the whole shell moves a cell sideways and is still said to be \
                 standing in its own room",
                stage.name
            );
        }
    }

    /// **The fill family is asked about every berth, and it answers when
    /// a body slides off its cells.**
    ///
    /// Three things, and none of them is the family's own branch read
    /// back. Every chart class a kind may be berthed on is actually
    /// measured, so the sweep cannot go quietly empty. Every berth in the
    /// game reads centred on the ground its plan owns and filling
    /// [`crate::pieces::BAY_FIT`] of it — which is the claim itself, and
    /// which was false on every deck and every deckhead berth until the
    /// band was drawn back onto the cells. And the reading is put to a
    /// body that has moved: the same box slid half a notch along the
    /// chart, and the same box shrunk to half the ground it claims, both
    /// have to stop reading as a berth filled.
    #[test]
    fn the_fill_family_is_asked_about_every_berth_and_answers_when_a_body_slides() {
        let seen = plans(&roster());
        let mut classes: BTreeMap<String, u32> = BTreeMap::new();
        for berth in &seen {
            *classes.entry(format!("{:?}", berth.surf)).or_default() += 1;
        }
        assert_eq!(
            classes.len(),
            Surf::ALL.len(),
            "a chart class went unmeasured: {classes:?}"
        );
        assert!(
            seen.len() > 1000,
            "the sweep should cover every berth in the game: {}",
            seen.len()
        );
        for berth in &seen {
            let (kind, surf) = (berth.kind, berth.surf);
            for dir in [
                berth.chart.half_u.normalize(),
                berth.chart.half_v.normalize(),
            ] {
                let (off, got) = off_plan(berth.spent, berth.owned, dir);
                let want = berth.owned.span().dot(dir.abs()) * crate::pieces::BAY_FIT;
                assert!(
                    off.abs() <= GRID_EPS,
                    "{kind:?} on a {surf:?} berth is composed {off} m off the middle of \
                     the cells its plan spends"
                );
                assert!(
                    (got - want).abs() <= GRID_EPS,
                    "{kind:?} on a {surf:?} berth fills {got} m of the {want} m its plan \
                     spends"
                );
                let step = dir * (GRID_STEP * 0.5);
                let slid = Box3::spanning(berth.spent.lo + step, berth.spent.hi + step);
                assert!(
                    off_plan(slid, berth.owned, dir).0.abs() > GRID_EPS,
                    "{kind:?} on a {surf:?} berth slid half a notch and nothing noticed"
                );
                let shrunk = Box3::spanning(berth.spent.lo, berth.spent.hi - dir.abs() * got * 0.5);
                assert!(
                    (off_plan(shrunk, berth.owned, dir).1 - want).abs() > GRID_EPS,
                    "{kind:?} on a {surf:?} berth halved its body and nothing noticed"
                );
            }
        }
    }

    /// **The turn family is asked about every berth, and it answers when
    /// a body is spun.**
    ///
    /// Three things, and the last is the one that matters. Every chart
    /// class is measured, so the sweep cannot go quietly empty. Every
    /// berth in the game stands up and shows the room its face — which is
    /// the claim itself, and which was false on every deckhead berth
    /// against a seam until the pendant took the backing rule. And the
    /// two readings are put to a body that has been turned: rolled a
    /// quarter turn about the face it shows, a rig has to stop reading as
    /// standing up; turned to look at the wall it is against, a rig on the
    /// edge of its own chart has to stop reading as facing the room.
    ///
    /// The catch-out is deliberately built out of a direction rather than
    /// out of [`crate::pieces::floor_facing`]'s branches. A sweep that
    /// recomputed the backing rule and compared it with itself would pass
    /// two thousand berths and mean nothing — which is the mistake this
    /// file has already made once, and docs/GAUNTLET.md keeps the receipt.
    #[test]
    fn the_turn_family_is_asked_about_every_berth_and_answers_when_a_body_spins() {
        let seen = plans(&roster());
        let mut classes: BTreeMap<String, u32> = BTreeMap::new();
        let mut flat = 0_u32;
        let mut edges = 0_u32;
        for berth in &seen {
            *classes.entry(format!("{:?}", berth.surf)).or_default() += 1;
            let (kind, surf) = (berth.kind, berth.surf);
            let up = berth.rot * Vec3::Y;
            assert!(
                up.dot(Vec3::Y) > 0.999,
                "{kind:?} on a {surf:?} berth rises {up:?}, not the room's up"
            );
            let spun =
                Quat::from_axis_angle(berth.rot * Vec3::Z, std::f32::consts::FRAC_PI_2) * berth.rot;
            assert!(
                (spun * Vec3::Y).dot(Vec3::Y) <= 0.999,
                "{kind:?} on a {surf:?} berth rolled a quarter turn and nothing noticed"
            );
            let (u, v) = (
                berth.chart.half_u.normalize(),
                berth.chart.half_v.normalize(),
            );
            if !matches!(berth.station, Station::BayFloor | Station::BayCeiling) {
                let inward = berth.station.inward(&berth.chart);
                assert!(
                    (berth.rot * Vec3::Z).dot(inward) > 0.999,
                    "{kind:?} on a {surf:?} berth shows the room {:?}, not the wall's own \
                     way in",
                    berth.rot * Vec3::Z
                );
                continue;
            }
            flat += 1;
            let at = looked_at(&berth.chart, berth.rect, berth.rot);
            assert!(
                berth.chart.rect.contains(at),
                "{kind:?} on a {surf:?} berth looks at ({}, {}), off its own chart",
                at.x,
                at.y
            );
            // A berth on the rim of its own chart, turned to look over
            // that rim, has to read as looking off it.
            let rim = berth.chart.rect;
            for (touches, out) in [
                (berth.rect.x <= rim.x + 1e-3, -u),
                (berth.rect.x + berth.rect.w >= rim.x + rim.w - 1e-3, u),
                (berth.rect.y <= rim.y + 1e-3, -v),
                (berth.rect.y + berth.rect.h >= rim.y + rim.h - 1e-3, v),
            ] {
                if !touches {
                    continue;
                }
                edges += 1;
                let outward = Quat::from_rotation_arc(Vec3::Z, out);
                let over = looked_at(&berth.chart, berth.rect, outward);
                assert!(
                    !rim.contains(over),
                    "{kind:?} on a {surf:?} berth turned to face the wall it stands \
                     against still reads as looking into the room"
                );
            }
        }
        assert_eq!(
            classes.len(),
            Surf::ALL.len(),
            "a chart class went unmeasured: {classes:?}"
        );
        assert!(flat > 100, "the flat charts went unswept: {flat}");
        assert!(
            edges > 100,
            "no berth on the rim of a chart was spun: {edges}"
        );
    }

    /// **The approach family is asked about every door the game
    /// declares.**
    ///
    /// The law itself lives in the sim, where the tile classes are, and
    /// so does its catch-out:
    /// `sim::room::tests::a_rooms_own_goods_do_not_stand_between_its_door_and_its_deck`
    /// walls a room off course by course and requires the walk to say so.
    /// The cabin may not restate a sim rule, so what is left here is the
    /// half the cabin owns — that the report actually reaches every room
    /// kind and every door, and that a kind with no door is asked
    /// nothing rather than asked wrongly.
    #[test]
    fn the_approach_family_is_asked_about_every_door_the_game_has() {
        let mut doors = 0_u32;
        let mut kinds = 0_u32;
        for kind in space_trucking::sim::room::ROOM_KINDS {
            let mut mine = 0_u32;
            for (slot, port) in kind.declared() {
                match port {
                    space_trucking::sim::room::Port::Door { .. } => {
                        assert!(
                            kind.marooned(slot).is_some(),
                            "{kind:?} port {slot} is a door the family never asks about"
                        );
                        doors += 1;
                        mine += 1;
                    }
                    _ => assert_eq!(
                        kind.marooned(slot),
                        None,
                        "{kind:?} port {slot} punches the deck; it has no step to start from"
                    ),
                }
            }
            kinds += u32::from(mine > 0);
        }
        assert_eq!(doors, 9, "the game's declared doors: {doors}");
        assert_eq!(
            kinds,
            space_trucking::sim::room::ROOM_KINDS.len() as u32,
            "a room kind has no door at all: {kinds}"
        );
        assert!(deck_reached().is_empty(), "{:?}", deck_reached());
    }

    /// **The fixture family is asked about every counter in the game, and
    /// it answers when one is walled in.**
    ///
    /// Every room that shakes hands is asked — which is every calling
    /// room, and none of the two the ship carries — and every one of them
    /// can be worked from somewhere a body may stand. Then a slab is hung
    /// across one counter's face and the reading has to turn: a rule that
    /// cannot be shown to refuse is a green tick that means nothing.
    #[test]
    fn the_fixture_family_is_asked_about_every_counter_and_answers_when_one_is_walled_in() {
        let mut asked = 0_u32;
        for stage in roster() {
            let Some(face) = room::handshake_face(&stage.placed) else {
                assert!(
                    stage.placed.kind.riding(),
                    "{}: a calling room with no counter",
                    stage.name
                );
                continue;
            };
            asked += 1;
            let found = fixture_reached(&stage);
            assert!(found.is_empty(), "{}: {found:?}", stage.name);
            // A body bolted over the brass: the cell's own face, standing
            // out of the wall far enough to swallow the pick face. That
            // is the shape the defect actually takes — the Guild's
            // seizure beacon hung 0.58 m off its aft wall, and a beacon
            // over a counter is the same fitting one cell along.
            //
            // Two weaker partitions were tried first and both were
            // answered, honestly, by the rule: a plate the size of the
            // cell a hand's breadth out is reached AROUND (a body inside
            // two metres of the brass can stand well to one side and
            // still lean in), and a partition across the whole room a
            // hand's breadth out is stood BEHIND (the walk envelope
            // reaches to within five centimetres of a wall). Neither is
            // a hole in the family. "Workable from anywhere a body may
            // stand" is what it asks, and standing beside a plate is
            // somewhere.
            //
            // Which way "out of the wall" is comes off the room's own
            // middle rather than off the quad's winding, because a guard
            // that got the sign wrong would hang its body inside the wall
            // and prove nothing.
            let n = face.normal();
            let into = (stage.placed.lo + stage.placed.hi) * 0.5 - face.center;
            let normal = if n.dot(into) > 0.0 { n } else { -n };
            let half = face.half_u.abs() + face.half_v.abs() + normal.abs() * 0.2;
            let mut walled: Vec<Drawn> = scene(&stage)
                .into_iter()
                .filter(|drawn| !drawn.what.starts_with("handshake"))
                .collect();
            walled.push(Drawn {
                what: "a beacon bolted over the counter".to_owned(),
                body: Box3::spanning(face.center - half, face.center + half),
                faces: Faces::ALL,
                character: true,
                name: None,
                seat: None,
            });
            assert!(
                worked(&walled, &stances(&stage), face.center).is_err(),
                "{}: a beacon bolted over its counter and the family said nothing",
                stage.name
            );
        }
        assert_eq!(asked, 15, "every calling room shakes hands: {asked}");
    }

    /// **The worked-face family is asked about every control in the game,
    /// and it answers when a body stands across one.**
    ///
    /// The counter of every calling room and the amber latch of every seam
    /// that can be parted are enumerated — a family whose list of subjects
    /// came out empty would be the most expensive kind of green tick — and
    /// nothing the room hangs or stocks stands across one of them. Then the
    /// reading is handed a body a hand's breadth in front of each face and
    /// it has to say so, and handed the same body a hand's breadth BEHIND
    /// it and it has to stay quiet: an occlusion rule that cannot tell
    /// which side of a plate it is standing on is a rule that reports the
    /// wall.
    #[test]
    fn the_worked_face_family_is_asked_about_every_control_and_answers_when_one_is_stood_across() {
        let mut counters = 0_u32;
        let mut latches = 0_u32;
        for stage in roster() {
            for placed in &stage.all {
                let faces = worked_faces(placed);
                for (what, face, inward) in &faces {
                    if what == "handshake" {
                        counters += 1;
                    } else {
                        latches += 1;
                    }
                    let half = face.span() * 0.5 + inward.abs() * 0.02;
                    let mid = (face.lo + face.hi) * 0.5;
                    let front = mid + *inward * 0.1;
                    assert!(
                        across(*face, *inward, Box3::spanning(front - half, front + half))
                            > OCCLUDE_BITE,
                        "{}: a plate over {what} and the family said nothing",
                        room_name(&stage, placed)
                    );
                    let behind = mid - *inward * 0.1;
                    assert!(
                        across(*face, *inward, Box3::spanning(behind - half, behind + half)) <= 0.0,
                        "{}: the wall behind {what} reads as standing across it",
                        room_name(&stage, placed)
                    );
                }
            }
            let found = fixture_seen(&stage);
            assert!(found.is_empty(), "{}: {found:?}", stage.name);
        }
        assert_eq!(counters, 15, "every calling room shakes hands: {counters}");
        assert!(
            latches >= 15,
            "the game's seam latches went unswept: {latches}"
        );
    }

    /// **Every cargo kind is swept, and swept with a body.** The pass
    /// this closes existed because twelve rooms had never been walked;
    /// thirty-two kinds had never been measured at all, and a kind
    /// nobody sweeps is a kind nobody checks. The count is asserted so
    /// that a thirty-third arrives on the list rather than beside it.
    #[test]
    fn the_sweep_reaches_every_cargo_kind() {
        assert_eq!(
            Kind::ALL.len(),
            32,
            "a kind joined the manifest; it joins the sweep with it"
        );
        for kind in Kind::ALL {
            let seen: usize = rig_forms(kind)
                .into_iter()
                .map(|showing| rig_scene(Dressings::shipped(), kind, Screens::LIVE, showing).len())
                .sum();
            assert!(seen > 0, "{kind:?} is swept with no body at all");
        }
    }

    /// **A covering's two bodies are never judged against each other.**
    /// `pieces::sync_dressings` shows exactly one — laid into the room,
    /// or rolled and canned on a counter — so a plane the rolled bolt
    /// happens to share with the laid pile is a plane nobody can see, and
    /// a detector that reported it would be finding its own footprints.
    ///
    /// Asked in the undressed frame: the two bodies are the whitebox
    /// description's, and a dressed covering deliberately wears its one
    /// declared body in both forms — the manifest has no per-form
    /// vocabulary yet, and that gap is docs/GAUNTLET.md's to record, not
    /// this guard's to report as a footprint.
    #[test]
    fn a_coverings_two_bodies_are_never_judged_against_each_other() {
        for kind in Kind::ALL {
            let forms = rig_forms(kind);
            assert_eq!(
                forms.len(),
                if kind.covering() { 2 } else { 1 },
                "{kind:?} is swept in the wrong number of forms"
            );
            if !kind.covering() {
                continue;
            }
            let whitebox = Dressings::default();
            let named = |showing| -> Vec<String> {
                rig_scene(&whitebox, kind, Screens::LIVE, showing)
                    .into_iter()
                    .map(|drawn| drawn.what)
                    .collect()
            };
            let laid = named(Under::Laid);
            let packed = named(Under::Packed);
            assert!(!laid.is_empty() && !packed.is_empty());
            assert!(
                laid.iter().all(|what| !packed.contains(what)),
                "{kind:?} draws one part in both of its bodies"
            );
        }
    }

    /// **A flat paint shows one side.** A class's field is a face on a
    /// chart: the mesh it is drawn from is a box because the renderer
    /// draws boxes, and the side behind it lies inside the decal ladder's
    /// own band, hard against the surface the paint is painted on. Only
    /// the side it turns to the room can be seen, so only that side can
    /// fight.
    #[test]
    fn a_flat_paint_shows_only_the_side_it_turns_to_the_room() {
        for stage in roster() {
            let mid = (stage.placed.lo + stage.placed.hi) * 0.5;
            for drawn in scene(&stage) {
                if !drawn.what.contains(" field (") {
                    continue;
                }
                let mut shown = 0;
                for axis in 0..3 {
                    for up in [false, true] {
                        if !drawn.faces.has(axis, up) {
                            continue;
                        }
                        shown += 1;
                        let (face, back) = if up {
                            (drawn.body.hi[axis], drawn.body.lo[axis])
                        } else {
                            (drawn.body.lo[axis], drawn.body.hi[axis])
                        };
                        assert!(
                            (face - mid[axis]).abs() <= (back - mid[axis]).abs(),
                            "{}: {} shows the side away from the room",
                            stage.name,
                            drawn.what
                        );
                    }
                }
                assert_eq!(
                    shown, 1,
                    "{}: {} shows {shown} sides, and a paint has one",
                    stage.name, drawn.what
                );
            }
        }
    }

    /// Whether the detector's own rule fires on one pair — the test
    /// harness's harness, so the rules above can be stated on two boxes
    /// instead of on a whole room. It asks the detector rather than
    /// restating it, because a restated rule is a rule with a second
    /// place to be wrong.
    fn pairs_fight(a: &Drawn, b: &Drawn) -> bool {
        !shared_faces(a, b).is_empty()
    }

    /// **One rasteriser at a time.** Both pixel harnesses drive a whole
    /// copy of the game, and the test harness would gladly run them at
    /// once: two software rasterisers on one machine is a machine under
    /// load, and a picture taken on a busy machine is measurably not the
    /// picture taken on a quiet one. They take turns.
    static RASTERISER: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The turn, held for as long as the returned guard lives. A panic
    /// in one harness poisons nothing that matters here — the lock
    /// guards a machine, not a value.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        RASTERISER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The built game and the workspace it was built in. Both pixel
    /// harnesses drive the binary itself, because what they are checking
    /// is what the binary does with a window.
    fn built_game() -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the workspace root is above the crate");
        let game = root.join("target/debug/space-trucking");
        assert!(
            game.is_file(),
            "build the game first: cargo build -p cabin (looked in {})",
            game.display()
        );
        (game, root)
    }

    /// **The pixel half is opt-in, and this is the door to it.**
    ///
    /// A screenshot needs a rasteriser, and the ordinary suite has no
    /// window: `.github/workflows/ci-cd.yml` installs neither `xvfb` nor
    /// a Vulkan ICD. So the flicker detector, the light-pop detector and
    /// the filmstrip live in the binary's own `--gauntlet-walk` mode, and
    /// this test runs one room of it. The invocation, what the walk
    /// writes, and where the two detectors' margins actually stand are in
    /// docs/GAUNTLET.md.
    ///
    /// The PURE half of the same walk — that the waypoints are legal,
    /// that the path enters by the door and reaches the counter, and that
    /// nothing is standing in any of them — runs in the ordinary suite
    /// and needs none of that.
    ///
    /// **What keeps it opt-in is the window and nothing else now.** It
    /// used to be that the readings moved; on the counted clock
    /// (`crate::FRAME_STEP`) two walks of one room print the same
    /// twenty-seven lines to the last digit and write twenty-seven
    /// identical PNGs.
    #[test]
    #[ignore = "needs a rasteriser: run it under xvfb, see docs/GAUNTLET.md"]
    fn the_filmstrip_holds_still_and_the_light_does_not_pop() {
        use std::process::Command;

        let _turn = one_at_a_time();
        let (game, root) = built_game();
        let shots = root.join("target/gauntlet-walk");
        std::fs::create_dir_all(&shots).expect("somewhere for the filmstrip to land");
        let ran = Command::new(&game)
            .arg("--gauntlet-walk")
            .arg(&shots)
            .args(["--docked", "6"])
            .status()
            .expect("the game runs");
        assert!(
            ran.success(),
            "the walk found something a still could not: see the \
             gauntlet-walk lines above, and the filmstrip in {}",
            shots.display()
        );
    }

    /// **One view shot twice is one file twice.**
    ///
    /// A screenshot is only evidence if it reproduces, and these did not:
    /// two runs of one `--shot` command disagreed over anywhere from 1%
    /// of the picture to 57% of it. The shot path runs on the counted
    /// clock now (`crate::FRAME_STEP`), in a world that has stopped
    /// reading the wall (`Bridge::steady`), and this is the guard that
    /// keeps it there. What broke, what a loose clock costs, and the one
    /// thing that still reads the OS clock at boot are in
    /// docs/GAUNTLET.md.
    ///
    /// Four views: one from inside the ship, which carries the breathing
    /// emissives, the drifting motes and the tube; one from outside it,
    /// where the star field and the sky clock have the furthest to drift;
    /// and the two that used to come out two ways, whose seam latch was
    /// drawn twice at one transform and is drawn once now
    /// ([`room::seam_parts`]).
    ///
    /// It needs a rasteriser, like everything else in the pixel half:
    ///
    /// ```text
    /// cargo build -p cabin
    /// VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    ///   WGPU_BACKEND=vulkan xvfb-run -a \
    ///   cargo test -p cabin -- --ignored the_same_view
    /// ```
    #[test]
    #[ignore = "needs a rasteriser: run it under xvfb, see docs/GAUNTLET.md"]
    fn the_same_view_shot_twice_is_the_same_bytes() {
        use std::process::Command;

        let _turn = one_at_a_time();
        let (game, root) = built_game();
        let shots = root.join("target/shot-twice");
        std::fs::create_dir_all(&shots).expect("somewhere for the pair to land");
        // The board each view wants, past the fixture: `parlor` is a
        // room nobody keeps, so it has to be brought alongside before
        // there is a room to stand in.
        for (view, board) in [
            ("bay", &[][..]),
            ("drydock", &[]),
            ("starboard", &[]),
            ("parlor", &["--alongside", "parlor"]),
        ] {
            let pair: Vec<std::path::PathBuf> = (1..=2)
                .map(|take| {
                    let path = shots.join(format!("{view}-{take}.png"));
                    let ran = Command::new(&game)
                        .args(["--fixture", "--view", view, "--shot"])
                        .arg(&path)
                        .args(board)
                        .status()
                        .expect("the game runs");
                    assert!(ran.success(), "the {view} shot did not land");
                    path
                })
                .collect();
            let [first, second] = [&pair[0], &pair[1]].map(|path| {
                std::fs::read(path).unwrap_or_else(|why| {
                    panic!("the shot must be on disk: {} ({why})", path.display())
                })
            });
            assert_eq!(
                first.len(),
                second.len(),
                "two shots of {view} are not even the same size"
            );
            let moved = first.iter().zip(&second).filter(|(a, b)| a != b).count();
            assert_eq!(
                moved,
                0,
                "{moved} bytes of {view} moved between two runs of one command: \
                 something in that view is still reading a clock nobody counts \
                 (the pair is in {})",
                shots.display()
            );
        }
    }

    /// **A room with no lamp of its own is still a room you can find.**
    ///
    /// The furnace is the only one there is: every lumen aboard is cargo
    /// (docs/BAY.md), and the fire is the one light the chamber ever
    /// gets — so a ship that has not fed it stands next door to a black
    /// box. The playtest walked into it and reported the hopper, the
    /// cornices and the fire door as *entirely illegible*, and they
    /// were: 0.0016 of that frame stood clear of the ground, all of it
    /// the version corner and the crosshair.
    ///
    /// This asks the picture, because that is the only thing that can be
    /// asked. Nothing pure describes a room's paint, so no rule in this
    /// file can see the tape at all (see docs/GAUNTLET.md, "What the
    /// harness can see"), and a material carrying an emissive is a fact
    /// about a struct rather than about a room. What is asserted is the
    /// observable one: stand in the furnace of a ship whose fire is out
    /// and some of the room comes back.
    ///
    /// `--underway` is the board with no stoke banked — a fresh ship,
    /// cast off and part-way through its first leg — so the fire is
    /// genuinely out rather than turned down.
    #[test]
    #[ignore = "needs a rasteriser: run it under xvfb, see docs/GAUNTLET.md"]
    fn a_furnace_with_its_fire_out_is_still_a_room_you_can_find() {
        use std::process::Command;

        let _turn = one_at_a_time();
        let (game, root) = built_game();
        let shots = root.join("target/dark-room");
        std::fs::create_dir_all(&shots).expect("somewhere for the shot to land");
        let path = shots.join("burner.png");
        let shot = Command::new(&game)
            .args(["--underway", "--view", "burner", "--shot"])
            .arg(&path)
            .output()
            .expect("the game runs");
        assert!(shot.status.success(), "the burner shot did not land");
        let out = String::from_utf8_lossy(&shot.stdout);
        let read: f32 = out
            .lines()
            .find_map(|line| line.strip_prefix("shot ")?.split(" read=").nth(1))
            .and_then(|token| token.trim().parse().ok())
            .unwrap_or_else(|| panic!("the shot must report what it came out as:\n{out}"));
        assert!(
            read >= ROOM_READS,
            "only {read:.5} of the furnace stands clear of the dark, under the \
             {ROOM_READS} a room has to reach to be a room: the chamber owns no \
             lamp, so whatever carries it with the fire out has stopped \
             carrying it (the frame is in {})",
            path.display()
        );
    }

    /// The documented entry point still exists to be invoked. Cheap, and
    /// it is the one part of the paragraph above a compiler can check.
    #[test]
    fn the_pixel_half_has_a_door() {
        assert_eq!(crate::gauntlet_walk_flag(), "--gauntlet-walk");
        const {
            assert!(FLICKER_FRAMES > 2, "one frame cannot disagree with itself");
            assert!(
                APPROACH_STEPS > 2,
                "a jump needs three samples to be a jump"
            );
            assert!(
                READ_FLOOR > 0.0 && ROOM_READS > 0.0,
                "a frame that reads at nothing reads at everything"
            );
        }
    }
}
