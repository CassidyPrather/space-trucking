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
//! rasteriser, so they live behind an opt-in run under `xvfb` and are
//! documented at [`tests::the_pixel_half_is_opt_in`]. What *is* pure about
//! them — the path itself, and every geometric assertion made at each
//! waypoint — runs always.
//!
//! # The families, and the defect class each closes
//!
//! | Rule | What it asserts | What it retires |
//! | --- | --- | --- |
//! | [`BERTH_CLEAR`] | no fitting stands in a cell cargo may take, at the height a rig occupies | decor clipping through functional cargo |
//! | [`BERTH_SEEN`] | nothing stands between a wall berth and the room | "the wall plate occludes the window" |
//! | [`BERTH_REACHED`] | every berth is workable from the walk envelope | furniture fencing off a corner of the net |
//! | [`NO_COPLANAR`] | no two drawn faces share a plane and a facing | z-fighting, generally |
//! | [`PROP_POINTS`] | a rig's named features point where their names say | the sconce lighting the wall, the base plate on edge |
//! | [`WALK_CLEAR`] | the walked path stands in air, room to room | furniture in the doorway you enter by |
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
use space_trucking::sim::cargo::{Kind, Loc, Piece, placement_check};
use space_trucking::sim::layout;
use space_trucking::sim::room::{CABIN, RoomId, RoomKind, Rooms, Tile};

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
/// The walked path must stand in air.
pub const WALK_CLEAR: &str = "walk-clear";

/// Every rule, for the report's own headings.
pub const RULES: [&str; 6] = [
    BERTH_CLEAR,
    BERTH_SEEN,
    BERTH_REACHED,
    NO_COPLANAR,
    PROP_POINTS,
    WALK_CLEAR,
];

// ------------------------------------------------------------- the sizes --

/// How far two boxes must genuinely overlap before it is a clip rather
/// than a graze, in metres. A fitting's volume is taken as its AABB, and
/// a sphere's or a torus's AABB has corners the body does not, so a
/// hair of overlap is not evidence of anything.
const CLIP_SLACK: f32 = 0.004;

/// And how much of the smaller body the overlap must eat. Same argument
/// from the other side: an AABB corner graze is a thin sliver of a round
/// body, a fitting standing in a berth is most of itself.
const CLIP_BITE: f32 = 0.05;

/// The air a wall berth is READ through, past the rig's own depth: about
/// a body's stand-off from the wall it is looking at.
const SIGHT: f32 = 0.55;

/// How much of a wall cell's face a fitting must cover before it is
/// hiding what hangs there rather than standing beside it.
const OCCLUDE_BITE: f32 = 0.25;

/// Two faces closer than this are the same plane as far as the depth
/// buffer is concerned. It is [`crate::rig::layer::SKIN`], the thickest a
/// flat paint riding a rung of the decal ladder may be: two faces inside
/// one skin of each other cannot be told apart by depth, and which one
/// draws is then a question of query order, which is not an answer.
///
/// The ladder's own `STEP` (0.004) was the other candidate and it is the
/// wrong one here — it is the minimum step the ladder GUARANTEES to be
/// safe, deliberately conservative because a dozen rungs share five
/// centimetres. Using a safety margin as a detection threshold is how a
/// detector ends up reporting four millimetres of daylight as a fight.
const FIGHT_EPS: f32 = crate::rig::layer::SKIN;

/// How much two coplanar faces must actually overlap before they can
/// fight. Abutting is not overlapping: neighbouring tile fields share a
/// plane and an edge by design, and a shared edge draws nothing twice.
const FIGHT_FOOT: f32 = 0.01;

/// Grid resolution of the standing-point search, per envelope box.
const STANCES: u8 = 8;

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
    const fn spanning(a: Vec3, b: Vec3) -> Self {
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
        .map(|(id, room)| room::placed(id, room))
        .collect();
    for (id, kind, name) in [
        (CABIN, RoomKind::Cabin, "cabin"),
        (1, RoomKind::Burner, "burner"),
    ] {
        let Some(room) = ship.get(id) else { continue };
        debug_assert!(room.kind == kind, "the yard-fresh ship changed shape");
        stages.push(Stage {
            name: name.to_owned(),
            placed: room::placed(id, room),
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
            .map(|(other, room)| room::placed(other, room))
            .collect();
        let Some(room) = rooms.get(id) else { continue };
        stages.push(Stage {
            name: name_of(host).to_owned(),
            placed: Placed {
                host: Some(host),
                ..room::placed(id, room)
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

/// How much air one berthed rig spends off its own chart, in metres.
///
/// Derived rather than restated: [`crate::pieces::berth_box`] poses the
/// body through the very function the runtime poses it with, and this is
/// that box measured along the chart's own normal. On a deck or under a
/// ceiling it comes out as the rig's HEIGHT (a standing rig keeps its
/// footprint's depth, the bas-relief inheritance from BAY.md); on a wall
/// it comes out as the common rig depth every kind is drawn within.
fn rig_air(charts: &[(Station, SimSurface)], rect: layout::Rect, inward: Vec3) -> Option<f32> {
    let (lo, hi) = crate::pieces::berth_box(charts, rect)?;
    Some((hi - lo).dot(inward.abs()))
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
        let (w, h) = kind.cells();
        for y in 0..rows {
            for x in 0..cols {
                if placement_check(rooms, &[], u32::MAX, kind, placed.id, x, y).is_err() {
                    continue;
                }
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
                        let Some(air) = rig_air(&placed.charts, rect, station.inward(&surface))
                        else {
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
    let cell = layout::cell_rect(id, x, y);
    let a = surface.to_world(space_trucking::sim::Vec2::new(cell.x, cell.y));
    let b = surface.to_world(space_trucking::sim::Vec2::new(
        cell.x + cell.w,
        cell.y + cell.h,
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
    /// Whether a character authored it. The rules that are about a
    /// station's furniture ask only about these; the rest is fabric.
    pub character: bool,
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
    /// everywhere. `rot` carries the body's own axes onto the world's,
    /// which is a quarter turn every time the lattice or a chart is
    /// involved, so a cap lands squarely on one world axis.
    fn of(shape: Shape, rot: Quat) -> Self {
        let own = match shape {
            Shape::Slab => Vec3::ONE,
            Shape::Post | Shape::Cone => Vec3::Y,
            Shape::Dome | Shape::Ring => Vec3::ZERO,
        };
        let m = Mat3::from_quat(rot);
        let world = m.x_axis.abs() * own.x + m.y_axis.abs() * own.y + m.z_axis.abs() * own.z;
        let flat = world.cmpgt(Vec3::splat(0.5));
        Self { lo: flat, hi: flat }
    }

    /// The one face a flat paint shows: the way it looks off its chart.
    fn showing(dir: Vec3) -> Self {
        let axis = dir.abs().cmpgt(Vec3::splat(0.5));
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

/// The world half-extents of one unit body, per silhouette. Everything
/// but the ring is one unit across in all three axes; a torus lies flat,
/// and its tube is what it is thick.
const fn unit_half(shape: Shape) -> Vec3 {
    match shape {
        Shape::Ring => Vec3::new(0.5, 0.09, 0.5),
        Shape::Slab | Shape::Post | Shape::Dome | Shape::Cone => Vec3::splat(0.5),
    }
}

/// One posed fitting, as the sweep measures it: its box, and the sides of
/// that box its own silhouette actually draws.
fn drawn(what: String, frame: &Frame, fitting: &Fitting) -> Drawn {
    Drawn {
        what,
        body: Box3::of(&frame.place(fitting), unit_half(fitting.shape)),
        faces: Faces::of(fitting.shape, frame.rot),
        character: true,
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
            });
        }
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
            });
        }
    }
    // The colored classes' flat fields and the berth wells, one per cell,
    // exactly where `room::tiles` lays them.
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
                Tile::Plain | Tile::Stock | Tile::Consume => crate::rig::layer::TILE,
            };
            let inward = station.inward(&surface);
            let face = cell_face(placed.id, (x, y), &surface);
            let skin = crate::rig::layer::SKIN;
            out.push(Drawn {
                what: format!("{tile:?} field ({x}, {y})"),
                body: face.reaching(inward, skin.mul_add(-0.5, lift), skin.mul_add(0.5, lift)),
                faces: Faces::showing(inward),
                character: false,
            });
        }
    }
    out
}

// -------------------------------------------------------------- the sweep --

/// **The whole gauntlet's pure half**, room by room, rule by rule.
#[must_use]
pub fn sweep() -> Vec<Finding> {
    let mut out = Vec::new();
    for stage in roster() {
        out.extend(berth_clear(&stage));
        out.extend(berth_seen(&stage));
        out.extend(berth_reached(&stage));
        out.extend(coplanar(&stage));
        out.extend(walk_clear(&stage));
    }
    out.extend(prop_points());
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

/// **No decor stands where cargo stands.** The assertion the containment
/// test never made: a fitting inside the room's box is not the same claim
/// as a fitting outside every berth in it.
fn berth_clear(stage: &Stage) -> Vec<Finding> {
    let berths = berths(&stage.rooms, &stage.placed);
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

/// **Nothing occludes a wall berth from its room.** A fitting standing in
/// front of a wall cell hides whatever hangs there, which is a defect the
/// containment law cannot express and a screenshot taken from the side
/// cannot show.
fn berth_seen(stage: &Stage) -> Vec<Finding> {
    let berths = berths(&stage.rooms, &stage.placed);
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

/// **Every berth stays reachable.** `rig::tests::every_net_cell_is_workable`
/// is the precedent and the hull was the only thing it could ask about;
/// this asks the same question of every room in the game, with a
/// station's own furniture added to the list of things that may be in the
/// way.
fn berth_reached(stage: &Stage) -> Vec<Finding> {
    let scene = scene(stage);
    let stances = stances(stage);
    let mut blamed: BTreeMap<String, Vec<(u8, u8)>> = BTreeMap::new();
    for berth in berths(&stage.rooms, &stage.placed) {
        let probe = (berth.face.lo + berth.face.hi) * 0.5 + berth.inward * 0.02;
        let worked = stances.iter().any(|eye| {
            let dir = probe - *eye;
            let pitch = (-dir.y).atan2(dir.xz().length()).abs();
            if dir.length() > REACH - 0.05 || pitch > PITCH_LIMIT - 0.02 {
                return false;
            }
            !scene
                .iter()
                .any(|drawn| ray_box(*eye, dir, drawn.body).is_some_and(|t| t < 1.0 - 1e-3))
        });
        if worked {
            continue;
        }
        // Name what is in the way from the nearest stance a body could
        // take: an "unreachable" with no culprit is a puzzle, not a work
        // order.
        let blame = stances
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
            .unwrap_or_else(|| "nothing — simply out of reach".to_owned());
        blamed.entry(blame).or_default().push(berth.cell);
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

/// **No two coplanar same-facing surfaces.** The general form of the
/// detector a previous pass built by hand out of a scene dump: for every
/// pair of drawn bodies, every pair of faces that share an axis, a
/// coordinate, and a direction, and whose footprints genuinely overlap.
///
/// Same-facing is the whole subtlety. Two boxes stacked touch at a plane
/// too, and that plane carries one face UP and one face DOWN — which the
/// depth buffer settles correctly every time. It is two faces looking the
/// same way from the same place that has no answer.
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
                    shared.entry(pair.clone()).or_default().push(format!(
                        "a {facing} face at {pa:.4} over {:.3}x{:.3} m",
                        flat[0], flat[1]
                    ));
                }
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
fn prop_points() -> Vec<Finding> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        for feature in crate::pieces::features(kind) {
            let got = (feature.turn() * feature.axis).normalize_or_zero();
            let want = feature.want.normalize_or_zero();
            if got.dot(want) > 0.99 {
                continue;
            }
            out.push(Finding {
                room: "rigs".to_owned(),
                rule: PROP_POINTS,
                offender: format!("{kind:?} {}", feature.name),
                detail: format!("points {got} in its own rig; its name says {want}"),
            });
        }
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
/// fails the build and a fixed one fails it too. Emptying it is the next
/// agent's whole job.
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
/// Concentric bodies were the honest case the owner named: a firebox
/// glass inside a firebox drum, two cylinders cut from one centre, a
/// collar round a pipe. They are not on this list either, because the
/// answer to them was a truer detector rather than a forgiven pair —
/// [`Faces`] knows which sides of a body's box are faces, and a round
/// body has none round its flank.
///
/// **It is empty today, and that is a finding of its own**: the tighter
/// [`FIGHT_EPS`], and then [`Faces`], answered every case the loose
/// detector produced, so nothing has yet needed forgiving. A pair added
/// here needs its reason written beside it, and the reason has to be
/// about the geometry rather than about the number.
pub const ALLOWED: &[(&str, &str, &str)] = &[];

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        // Same top, overlapping footprint: a fight.
        let fighting = Drawn {
            what: "b".to_owned(),
            body: Box3::spanning(Vec3::new(0.2, 0.2, 0.0), Vec3::new(0.8, 1.0, 0.2)),
            faces: Faces::ALL,
            character: true,
        };
        // Stacked: its floor is the other's ceiling, and that is fine.
        let stacked = Drawn {
            what: "c".to_owned(),
            body: Box3::spanning(Vec3::new(0.2, 1.0, 0.0), Vec3::new(0.8, 2.0, 0.2)),
            faces: Faces::ALL,
            character: true,
        };
        assert!(pairs_fight(&a, &fighting), "coplanar tops must be caught");
        assert!(!pairs_fight(&a, &stacked), "a stack is not a fight");
    }

    /// **Only a box has six faces.** A cylinder meets each of the four
    /// planes round its flank along a single line, a sphere meets all six
    /// at a point, and a torus at a curve — so a boss cut round a post
    /// from one centre shares a box with it and shares no face at all,
    /// and the detector must not report the box as the body.
    #[test]
    fn only_a_box_has_six_faces() {
        use std::f32::consts::FRAC_PI_2;

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
        // And the rule that follows: one centre, two radii, no fight —
        // where two plates on the same plane are one.
        let body = Box3::spanning(Vec3::splat(-0.5), Vec3::splat(0.5));
        let round = |shape| Drawn {
            what: format!("{shape:?}"),
            body,
            faces: Faces::of(shape, Quat::IDENTITY),
            character: true,
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
    /// harness's harness, so the rule above can be stated on two boxes
    /// instead of on a whole room.
    fn pairs_fight(a: &Drawn, b: &Drawn) -> bool {
        (0..3).any(|axis| {
            [
                (false, (a.body.lo[axis], b.body.lo[axis])),
                (true, (a.body.hi[axis], b.body.hi[axis])),
            ]
            .into_iter()
            .any(|(up, (pa, pb))| {
                if !a.faces.has(axis, up) || !b.faces.has(axis, up) {
                    return false;
                }
                if (pa - pb).abs() >= FIGHT_EPS {
                    return false;
                }
                let meet = a.body.meet(b.body).span();
                (0..3).filter(|k| *k != axis).all(|k| meet[k] > FIGHT_FOOT)
            })
        })
    }

    /// **The pixel half is opt-in, and here is how to run it.**
    ///
    /// A screenshot needs a rasteriser, and CI has one only under `xvfb`
    /// with a software Vulkan device. So the flicker detector, the
    /// light-pop detector, and the filmstrip live in the binary's own
    /// `--gauntlet-walk` mode, and this test is the door to them:
    ///
    /// ```text
    /// cargo build -p cabin
    /// VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    ///   WGPU_BACKEND=vulkan xvfb-run -a \
    ///   cargo test -p cabin -- --ignored the_filmstrip
    /// ```
    ///
    /// or, for one named room and the PNGs kept where you want them:
    ///
    /// ```text
    /// VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    ///   WGPU_BACKEND=vulkan xvfb-run -a \
    ///   ./target/debug/space-trucking --gauntlet-walk <dir> --docked 7
    /// ```
    ///
    /// `--docked n` picks a station's room and `--alongside wreck|parlor|pump`
    /// an event room, exactly as they do for a screenshot run. It writes
    /// one PNG per waypoint plus the still and approach strips, prints a
    /// `gauntlet-walk` line per sample carrying the pose, the mean
    /// luminance, and the fraction of the picture that moved, and exits
    /// non-zero if a frame moved while the camera did not or the room's
    /// brightness stepped along the approach.
    ///
    /// The PURE half of the same walk — that the waypoints are legal,
    /// that the path enters by the door and reaches the counter, and
    /// that nothing is standing in any of them — runs in the ordinary
    /// suite, and needs none of the above.
    #[test]
    #[ignore = "needs a rasteriser: run it under xvfb, see the doc comment"]
    fn the_filmstrip_holds_still_and_the_light_does_not_pop() {
        use std::path::PathBuf;
        use std::process::Command;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the workspace root is above the crate");
        let game = root.join("target/debug/space-trucking");
        assert!(
            game.is_file(),
            "build the game first: cargo build -p cabin (looked in {})",
            game.display()
        );
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
        }
    }
}
