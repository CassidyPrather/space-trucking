//! The rooms the ship attaches, drawn (docs/ROOMS.md, stage two).
//!
//! Stage one made the ship a **graph of rooms on an integer lattice**; the
//! sim hands over every room's kind, its pose, its mated ports, and a net
//! lane of logical space per dense `RoomId`. This module is the other half
//! of that bargain: it turns those integers into a place you can look at
//! and walk into, and it is the ONLY module allowed to know where a room
//! stands.
//!
//! Three ideas carry the whole thing:
//!
//! - **One anchor, everything else derived.** The cabin is the root at the
//!   lattice origin, so [`ANCHOR`] — where lattice cell `(0, 0, 0)`'s
//!   aft-port corner lands in the room — is the single position written
//!   down anywhere in the presentation. Every other room's box, chart,
//!   doorway, and painted tile is arithmetic on its pose.
//! - **A room is its net, folded onto its box.** BAY.md's cross of six
//!   charts generalizes from the cabin to the family: each room's charts
//!   bind its OWN lane rects onto its OWN physical box, so `SimSurface`
//!   keeps making every ruling and the sim never learns it grew a third
//!   dimension.
//! - **The hull follows the ports, never the other way round.** Which wall
//!   a door is on is data (the port law's first clause), so every wall,
//!   deck, and ceiling slab is *punched* by the aperture cells its own
//!   room declares. A mated port is an open doorway; an unmated one is a
//!   plate, drawn shut.
//!
//! The cabin keeps one privilege, and it is declared rather than assumed:
//! its hull was hand-built before the lattice existed and stands a working
//! gutter forward of its own floor box, so [`chart_inset`] carries that
//! authored trim as data. Its floor, its aft wall, its walk envelope, and
//! every one of its apertures derive like everybody else's.

use bevy::prelude::*;

use space_trucking::sim::layout;
use space_trucking::sim::room::{
    APERTURE, CABIN, COURSES, PORTS, Port, PortId, Pose, Room, RoomId, RoomKind, Rooms, Tile,
};
use space_trucking::sim::{Cue, Loc};

use crate::rig::{BAY_CELL, BAY_WALL_Z, EYE_HEIGHT, REACH, Skin, TileFade, WALK_MAX, WALK_MIN};
use crate::surface::{SimSurface, Station};
use crate::{Phase, Shell, glow, palette};

/// Where lattice cell `(0, 0, 0)`'s aft-port corner stands in the world.
///
/// The cabin is the graph's root at the lattice origin (`Rooms::root`), so
/// pinning its own aft-port corner pins the lattice for every room the
/// trip ever attaches. This is the presentation's one written-down
/// position, and it is the cabin's rather than any room's.
pub const ANCHOR: Vec3 = Vec3::new(
    -(RoomKind::Cabin.floor().0 as f32) * BAY_CELL * 0.5,
    0.0,
    BAY_WALL_Z,
);

/// Wall band height: three courses, the same on every room. Uniform
/// height is what lets any door mate any door (docs/ROOMS.md, "One storey,
/// everywhere").
pub const WALL_H: f32 = COURSES as f32 * BAY_CELL;

/// The deck's decal plane, a hair above the floor slab.
pub const FLOOR_Y: f32 = 0.012;

/// The ceiling chart's plane. The band between the wall cornices and here
/// is headroom trim; the net's fold seam glues logically, not physically.
pub const CEIL_Y: f32 = 2.26;

/// Centre height of a room's ceiling slab.
const CEIL_SLAB_Y: f32 = 2.32;

/// Structural thickness of every hull plane.
const WALL_T: f32 = 0.1;

/// One storey's pitch on the lattice: a room's ceiling slab plus a seam.
/// A ladder's neighbour sits exactly one of these up.
const STOREY: f32 = CEIL_SLAB_Y + WALL_T;

/// How far a wall chart stands inside its own box face, by default: proud
/// of the hull's junk, so every chart cell stays workable in front of the
/// ribs rather than behind them.
const CHART_INSET: f32 = 0.03;

/// The cabin's authored trim, by wall (aft, starboard, front, port). Its
/// aft wall IS its box face; its side walls take the ordinary inset; and
/// its front wall stands a declared working gutter BEYOND the floor box —
/// the hull was built before the lattice and the instruments hang on it.
const CABIN_TRIM: [f32; 4] = [0.0, CHART_INSET, -0.47, CHART_INSET];

/// Half-width of a walking body, for the derived envelopes. The cabin's
/// own envelope is authored (`rig::WALK_*`) because its hull is; every
/// attached room insets its lattice box by this.
const BODY: f32 = 0.30;

/// A sealed leaf's thickness — a door drawn shut is a real plate.
const PLATE_T: f32 = 0.05;

/// Doorjamb girth.
const JAMB: f32 = 0.06;

/// The detach latch's plate: a hand-sized amber lever beside the jamb.
const LATCH_W: f32 = 0.07;
const LATCH_H: f32 = 0.16;

/// How long a seam cue burns, seconds — feedback, inside the half-second
/// law (`docs/ART_DIRECTION_3D.md`).
const SEAM_LEN: f32 = 0.45;

/// Which room a spawned entity belongs to, and what kind of room that is.
/// Everything this module builds carries one, so a parted room takes its
/// whole presentation with it and a chart hit can ask its own room's net
/// whether the cell it landed on is a cell.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InRoom {
    pub room: RoomId,
    pub kind: RoomKind,
}

impl InRoom {
    /// Whether this room's net would let a piece berth on `(x, y)` — the
    /// question the pointer asks before it believes a chart hit. Holes and
    /// thresholds are misses, and the ray carries on past them.
    #[must_use]
    pub fn berthable(self, x: u8, y: u8) -> bool {
        !matches!(self.kind.tile_of(x, y), None | Some(Tile::Threshold))
    }
}

/// The amber latch beside a calling room's doorway: clicking it asks the
/// input schedule to part that seam. The sim's gates answer, and a refusal
/// arrives as a cue like any other.
#[derive(Component, Clone, Copy, Debug)]
pub struct Latch {
    pub room: RoomId,
    /// The quad the crosshair must land on.
    pub face: SimSurface,
}

/// A lamp this module drives from cues: the latch's own amber, and the
/// jamb lamp that answers a seam mating or parting.
#[derive(Component, Clone, Debug)]
pub struct SeamLamp {
    pub mat: Handle<StandardMaterial>,
    /// Whether this lamp belongs to a latch (amber, standing) rather than
    /// a jamb (dark until a cue).
    pub latch: bool,
}

/// One bar of the composed offer's claim frame — the lit footprint that
/// reads "this pile is what's on offer for yours". A pool, aimed each
/// frame at whatever `Sim::composed` names; the pieces themselves never
/// move, because stage one resolved the spec's tension that way.
#[derive(Component, Clone, Copy, Debug)]
pub struct ClaimBar(u8);

/// The handshake fixture's lamp: lit while the room has something to
/// commit, dark while the throw would find nothing.
#[derive(Component, Clone, Debug)]
pub struct HandshakeLamp {
    pub room: RoomId,
    pub mat: Handle<StandardMaterial>,
}

/// The handshake's brass plunger, which visibly throws when it is worked.
#[derive(Component, Clone, Copy, Debug)]
pub struct HandshakeThrow {
    pub rest: Vec3,
    pub travel: Vec3,
}

/// One placed room, as the presentation needs it: the same room the sim
/// has, in world units.
#[derive(Clone, Debug)]
pub struct Placed {
    pub id: RoomId,
    pub kind: RoomKind,
    /// Quarter turns clockwise, straight off the pose.
    pub yaw: u8,
    /// The room's interior box: deck corner to ceiling corner.
    pub lo: Vec3,
    pub hi: Vec3,
    /// The net's six charts, bound to this room's own lane rects.
    pub charts: [(Station, SimSurface); 6],
    /// Every declared attachment point, mated or not.
    pub ports: [Site; PORTS],
}

impl Placed {
    /// This room's chart for one plane of its net.
    #[must_use]
    pub fn chart(&self, want: Station) -> Option<&SimSurface> {
        self.charts
            .iter()
            .find(|(station, _)| *station == want)
            .map(|(_, surface)| surface)
    }
}

/// One attachment point, sited on the hull.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub port: PortId,
    /// What the room declares in this slot — `None` where the kind
    /// declares nothing, and a wall that declares nothing is a wall.
    pub declared: Option<Port>,
    /// Which room and port this one is mated to, if any.
    pub mate: Option<(RoomId, PortId)>,
    /// The hole this port punches through the hull, as a world box.
    pub lo: Vec3,
    pub hi: Vec3,
    /// The plane's outward direction — where a neighbour would be.
    pub out: Vec3,
    /// Centre of the opening on the DRAWN wall, where a leaf hangs.
    pub leaf: Vec3,
    /// The opening's two in-plane half-extents.
    pub half_a: Vec3,
    pub half_b: Vec3,
}

impl Site {
    /// Whether this port is a door — the only kind the body walks through
    /// this stage (see the module note on verticality).
    #[must_use]
    pub const fn is_door(&self) -> bool {
        matches!(self.declared, Some(Port::Door { .. }))
    }
}

/// The whole ship as the presentation sees it, rebuilt whenever the sim's
/// graph changes shape. Every consumer — the hull, the charts, the tiles,
/// the walk envelope, the occupied-room field — reads this and nothing
/// else, so no two of them can disagree about where a room is.
#[derive(Resource, Default)]
pub struct Plan {
    pub rooms: Vec<Placed>,
    /// The graph's shape last time this was rebuilt.
    signature: Vec<Shape>,
}

/// The cheap fingerprint of one room's placement, for change detection.
type Shape = (RoomId, u8, i32, i32, i32, u8, u32);

impl Plan {
    /// The placed room with this id, if it is attached.
    #[must_use]
    pub fn get(&self, id: RoomId) -> Option<&Placed> {
        self.rooms.iter().find(|room| room.id == id)
    }

    /// Which room's interior box holds `p`, if any.
    ///
    /// **This is the whole derivation of the occupied-room field.** The
    /// sim learns rooms, not positions: the body is in the room whose box
    /// it stands in, and one dense id is what crosses the interface.
    #[must_use]
    pub fn room_at(&self, p: Vec3) -> Option<RoomId> {
        self.rooms
            .iter()
            .find(|room| {
                p.x >= room.lo.x
                    && p.x <= room.hi.x
                    && p.z >= room.lo.z
                    && p.z <= room.hi.z
                    && p.y >= room.lo.y - WALL_T
                    && p.y <= room.hi.y + WALL_T
            })
            .map(|room| room.id)
    }

    /// The chart a sim point reads through, with the room it belongs to.
    /// Lanes are disjoint by law, so the rect alone answers.
    #[must_use]
    pub fn chart_at(
        &self,
        sim: space_trucking::sim::Vec2,
    ) -> Option<(RoomId, Station, SimSurface)> {
        self.rooms.iter().find_map(|room| {
            room.charts
                .iter()
                .find(|(_, surface)| surface.rect.contains(sim))
                .map(|(station, surface)| (room.id, *station, *surface))
        })
    }
}

/// Where the body may stand: one box per attached room, joined at every
/// mated doorway. Hull collision and nothing else — cargo has no body to
/// bump (docs/BAY.md), so a stack of crates can never fence a corner off.
#[derive(Resource, Default)]
pub struct Envelope {
    /// One box per attached room: its own floor, standable end to end.
    pub rooms: Vec<(Vec3, Vec3)>,
    /// One connector per mated doorway, reaching a body's width into the
    /// rooms on both sides so the two envelopes actually meet.
    pub seams: Vec<(Vec3, Vec3)>,
}

/// Whether `p` falls inside a box, in plan.
fn inside(p: Vec3, (lo, hi): (Vec3, Vec3)) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.z >= lo.z && p.z <= hi.z
}

impl Envelope {
    /// Whether the eye may stand at `p`.
    #[must_use]
    pub fn holds(&self, p: Vec3) -> bool {
        self.rooms.iter().chain(&self.seams).any(|b| inside(p, *b))
    }

    /// Whether `p` is in a doorway rather than in a room.
    ///
    /// An aperture is two courses of the CARGO grid, which makes it a
    /// hatch rather than a hall: a body passes through it the way anybody
    /// boards a freighter, by ducking. The sim's aperture stays exactly
    /// the two cells it declares, and the eye bends instead.
    #[must_use]
    pub fn ducking(&self, p: Vec3) -> bool {
        self.seams.iter().any(|b| inside(p, *b)) && !self.rooms.iter().any(|b| inside(p, *b))
    }

    /// The nearest legal standing point to `p` — for the frame a room
    /// parts out from under the body.
    #[must_use]
    pub fn nearest(&self, p: Vec3) -> Vec3 {
        self.rooms
            .iter()
            .chain(&self.seams)
            .map(|(lo, hi)| Vec3::new(p.x.clamp(lo.x, hi.x), p.y, p.z.clamp(lo.z, hi.z)))
            .min_by(|a, b| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
            .unwrap_or(p)
    }
}

/// Which room the crew body occupies, derived from the camera each frame
/// and handed to the sim as the one new input field (docs/ROOMS.md, "The
/// one new input field").
#[derive(Resource)]
pub struct Occupancy(pub RoomId);

impl Default for Occupancy {
    fn default() -> Self {
        Self(CABIN)
    }
}

/// The detach latch the crosshair rests on this frame, if any.
#[derive(Resource, Default)]
pub struct AimedLatch(pub Option<RoomId>);

/// The seam cues' latched clocks: a refusal strobe and a mate/part pulse.
#[derive(Resource, Default)]
pub struct SeamFx {
    refit: f32,
    seam: f32,
    /// The handshake's own throw, so the plunger visibly works.
    throw: f32,
}

// ---- The lattice, in world units ----

/// The world point at lattice corner `(x, y, z)`. Lattice +x runs to
/// starboard, lattice +y runs forward (world -Z), lattice +z is a storey.
#[must_use]
pub fn lattice_corner(cell: (i32, i32, i32)) -> Vec3 {
    Vec3::new(
        (cell.0 as f32).mul_add(BAY_CELL, ANCHOR.x),
        (cell.2 as f32).mul_add(STOREY, ANCHOR.y),
        (-(cell.1 as f32)).mul_add(BAY_CELL, ANCHOR.z),
    )
}

/// World direction of one step along a room's local +i (net x) axis.
#[must_use]
pub const fn axis_i(yaw: u8) -> Vec3 {
    match yaw % 4 {
        0 => Vec3::X,
        1 => Vec3::NEG_Z,
        2 => Vec3::NEG_X,
        _ => Vec3::Z,
    }
}

/// World direction of one step along a room's local +j (net y) axis.
#[must_use]
pub const fn axis_j(yaw: u8) -> Vec3 {
    match yaw % 4 {
        0 => Vec3::NEG_Z,
        1 => Vec3::NEG_X,
        2 => Vec3::Z,
        _ => Vec3::X,
    }
}

/// A local wall's outward direction: wall 0 is aft (-j), 1 starboard
/// (+i), 2 front (+j), 3 port (-i) — the sim's own numbering.
#[must_use]
pub fn wall_out(wall: u8, yaw: u8) -> Vec3 {
    match wall % 4 {
        0 => -axis_j(yaw),
        1 => axis_i(yaw),
        2 => axis_j(yaw),
        _ => -axis_i(yaw),
    }
}

/// How far a room's wall chart stands inside its own box face. Data, not
/// an assumption: the cabin declares an authored trim, everything else
/// takes the ordinary inset.
#[must_use]
pub fn chart_inset(kind: RoomKind, wall: u8) -> f32 {
    match kind {
        RoomKind::Cabin => CABIN_TRIM[usize::from(wall % 4)],
        _ => CHART_INSET,
    }
}

/// A room's interior box in world units, from its pose alone.
#[must_use]
pub fn room_box(room: &Room) -> (Vec3, Vec3) {
    let (x0, y0, x1, y1) = room.box_rect();
    let a = lattice_corner((x0, y0, room.pose.z));
    let b = lattice_corner((x1, y1, room.pose.z));
    (
        Vec3::new(a.x.min(b.x), a.y, a.z.min(b.z)),
        Vec3::new(a.x.max(b.x), a.y + CEIL_Y, a.z.max(b.z)),
    )
}

/// The cabin as the graph always has it: the root, at the lattice origin.
/// Lets the hull derive its own apertures without asking the sim, since
/// the root's pose is fixed by `Rooms::root` and nothing can move it.
#[must_use]
pub const fn cabin_room() -> Room {
    Room {
        kind: RoomKind::Cabin,
        pose: Pose {
            x: 0,
            y: 0,
            z: 0,
            yaw: 0,
        },
        mates: [None; PORTS],
        anchor: None,
    }
}

/// One placed room, derived from the graph's own record of it. Every
/// consumer builds its `Placed` through here, so nothing can derive a
/// room's geometry two different ways.
#[must_use]
pub fn placed(id: RoomId, room: &Room) -> Placed {
    let (lo, hi) = room_box(room);
    Placed {
        id,
        kind: room.kind,
        yaw: room.pose.yaw,
        lo,
        hi,
        charts: charts(id, room),
        ports: sites(room),
    }
}

/// One net chart's logical rect in room `id`'s own lane.
fn chart_rect(id: RoomId, cx: u8, cy: u8, cw: u8, ch: u8) -> layout::Rect {
    let origin = layout::cell_rect(id, cx, cy);
    layout::Rect::new(
        origin.x,
        origin.y,
        f32::from(cw) * layout::CELL,
        f32::from(ch) * layout::CELL,
    )
}

/// The room net's six charts, folded onto a placed room's own box.
///
/// The box unfolds exactly as BAY.md unfolds the cabin's — rows 0–2 stand
/// on the aft wall, the middle rows fold onto the deck and the side walls,
/// the last three stand on the front wall, and the ceiling chart folds on
/// past the starboard cornice — but every plane and every axis now comes
/// out of the pose, so a room at any yaw folds the same way.
#[must_use]
pub fn charts(id: RoomId, room: &Room) -> [(Station, SimSurface); 6] {
    let kind = room.kind;
    let yaw = room.pose.yaw;
    let (w, h) = kind.floor();
    let (lo, hi) = room_box(room);
    let mid = (lo + hi) * 0.5;
    let (i, j) = (axis_i(yaw), axis_j(yaw));
    let half_i = f32::from(w) * BAY_CELL * 0.5;
    let half_j = f32::from(h) * BAY_CELL * 0.5;
    let wall_mid = WALL_H * 0.5;
    // A wall chart's plane, measured inward from its own box face.
    let plane = |wall: u8, half: f32| wall_out(wall, yaw) * (half - chart_inset(kind, wall));
    let level = |y: f32| Vec3::new(mid.x, lo.y + y, mid.z);
    [
        (
            Station::BayWall,
            SimSurface {
                center: level(wall_mid) + plane(0, half_j),
                half_u: i * half_i,
                half_v: Vec3::NEG_Y * wall_mid,
                rect: chart_rect(id, COURSES, 0, w, COURSES),
            },
        ),
        (
            Station::BayFloor,
            SimSurface {
                center: level(FLOOR_Y),
                half_u: i * half_i,
                half_v: j * half_j,
                rect: chart_rect(id, COURSES, COURSES, w, h),
            },
        ),
        (
            Station::BayPort,
            SimSurface {
                center: level(wall_mid) + plane(3, half_i),
                half_u: Vec3::NEG_Y * wall_mid,
                half_v: j * half_j,
                rect: chart_rect(id, 0, COURSES, COURSES, h),
            },
        ),
        (
            Station::BayStarboard,
            SimSurface {
                center: level(wall_mid) + plane(1, half_i),
                half_u: Vec3::Y * wall_mid,
                half_v: j * half_j,
                rect: chart_rect(id, COURSES + w, COURSES, COURSES, h),
            },
        ),
        (
            Station::BayFront,
            SimSurface {
                center: level(wall_mid) + plane(2, half_j),
                half_u: i * half_i,
                half_v: Vec3::Y * wall_mid,
                rect: chart_rect(id, COURSES, COURSES + h, w, COURSES),
            },
        ),
        (
            Station::BayCeiling,
            SimSurface {
                center: level(CEIL_Y),
                half_u: -i * half_i,
                half_v: j * half_j,
                rect: chart_rect(id, 2 * COURSES + w, COURSES, w, h),
            },
        ),
    ]
}

/// The local floor cells a door's aperture stands on — the port law's own
/// declaration, read rather than assumed.
fn door_cells(kind: RoomKind, wall: u8, offset: u8) -> [(u8, u8); APERTURE as usize] {
    let (w, h) = kind.floor();
    let mut cells = [(0, 0); APERTURE as usize];
    for (step, cell) in cells.iter_mut().enumerate() {
        let along = offset + step as u8;
        *cell = match wall % 4 {
            0 => (along, 0),
            1 => (w - 1, along),
            2 => (along, h - 1),
            _ => (0, along),
        };
    }
    cells
}

/// The world box one lattice cell's column occupies between two heights
/// above its own deck.
fn cell_column(room: &Room, i: u8, j: u8, y0: f32, y1: f32) -> (Vec3, Vec3) {
    let cell = room.cell_of(i, j);
    let a = lattice_corner(cell);
    let b = lattice_corner((cell.0 + 1, cell.1 + 1, cell.2));
    (
        Vec3::new(a.x.min(b.x), a.y + y0, a.z.min(b.z)),
        Vec3::new(a.x.max(b.x), a.y + y1, a.z.max(b.z)),
    )
}

/// Grow a box to hold another.
fn union(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> (Vec3, Vec3) {
    (a.0.min(b.0), a.1.max(b.1))
}

/// The horizontal axis a wall runs along, given its outward normal.
fn axis_along(out: Vec3) -> Vec3 {
    if out.x.abs() > out.z.abs() {
        Vec3::Z
    } else {
        Vec3::X
    }
}

/// Every attachment point of a placed room, sited: the hole it punches,
/// the plane its leaf hangs in, and who (if anyone) is on the far side.
#[must_use]
pub fn sites(room: &Room) -> [Site; PORTS] {
    let kind = room.kind;
    let yaw = room.pose.yaw;
    let ports = kind.ports();
    let blank = Site {
        port: 0,
        declared: None,
        mate: None,
        lo: Vec3::ZERO,
        hi: Vec3::ZERO,
        out: Vec3::Y,
        leaf: Vec3::ZERO,
        half_a: Vec3::ZERO,
        half_b: Vec3::ZERO,
    };
    let mut sites = [blank; PORTS];
    for (index, site) in sites.iter_mut().enumerate() {
        site.port = index as PortId;
        site.declared = ports[index];
        site.mate = room.mates[index];
        // A slot the kind does not declare is a wall: nothing punched,
        // nothing hung, nothing drawn (docs/ROOMS.md, "The port law").
        let Some(declared) = ports[index] else {
            continue;
        };
        match declared {
            Port::Door { wall, offset } => {
                let cells = door_cells(kind, wall, offset);
                let clear = f32::from(APERTURE) * BAY_CELL;
                let mut hole = cell_column(room, cells[0].0, cells[0].1, 0.0, clear);
                for &(i, j) in &cells[1..] {
                    hole = union(hole, cell_column(room, i, j, 0.0, clear));
                }
                let out = wall_out(wall, yaw);
                let inset = chart_inset(kind, wall);
                // The DRAWN wall may stand beyond the box face (the
                // cabin's authored gutter), so the cut reaches as far out
                // as that room's own trim says the plane is — with half a
                // wall to spare, because a doorway that leaves a
                // paper-thin sliver of hull in it is not a doorway.
                let reach = WALL_T.mul_add(1.5, (-inset).max(0.0));
                let sign = out.x + out.z;
                let far = if sign > 0.0 { hole.1 } else { hole.0 };
                let face = far * out.abs() + (hole.0 + hole.1) * 0.5 * (Vec3::ONE - out.abs());
                let outward = face + out * reach;
                let grown = union(hole, (outward, outward));
                let along = axis_along(out);
                site.lo = grown.0;
                site.hi = grown.1;
                site.out = out;
                site.half_a = along * (clear * 0.5);
                site.half_b = Vec3::Y * (clear * 0.5);
                site.leaf = Vec3::new(face.x, clear.mul_add(0.5, hole.0.y), face.z) - out * inset;
            }
            Port::Ladder { x, y } | Port::Hatch { x, y } => {
                let up = matches!(declared, Port::Ladder { .. });
                let (y0, y1) = if up {
                    (CEIL_SLAB_Y - WALL_T, CEIL_SLAB_Y + WALL_T)
                } else {
                    (-WALL_T * 1.5, WALL_T * 0.2)
                };
                let mut hole = cell_column(room, x, y, y0, y1);
                for j in 0..APERTURE {
                    for i in 0..APERTURE {
                        hole = union(hole, cell_column(room, x + i, y + j, y0, y1));
                    }
                }
                let centre = (hole.0 + hole.1) * 0.5;
                let deck = lattice_corner((0, 0, room.pose.z)).y;
                site.lo = hole.0;
                site.hi = hole.1;
                site.out = if up { Vec3::Y } else { Vec3::NEG_Y };
                site.leaf = Vec3::new(centre.x, deck + if up { CEIL_Y } else { FLOOR_Y }, centre.z);
                site.half_a = Vec3::X * ((hole.1.x - hole.0.x) * 0.5);
                site.half_b = Vec3::Z * ((hole.1.z - hole.0.z) * 0.5);
            }
        }
    }
    sites
}

/// The cabin's six apertures as world boxes, for the hull that was built
/// before the lattice was. `rig::structure` punches every one of them,
/// which is why no slab in this game says where a door is.
#[must_use]
pub fn cabin_holes() -> [(Vec3, Vec3); PORTS] {
    let room = cabin_room();
    sites(&room).map(|site| (site.lo, site.hi))
}

/// Subtract a box from a slab, as up to six axis-aligned remainders. A
/// hole that misses leaves the slab whole; a hole that swallows it leaves
/// nothing. This is how every doorway, hatch, and ladder well is cut.
#[must_use]
pub fn punch(center: Vec3, size: Vec3, hole_lo: Vec3, hole_hi: Vec3) -> Vec<(Vec3, Vec3)> {
    let mut lo = center - size * 0.5;
    let mut hi = center + size * 0.5;
    if (hole_lo.cmpge(hi) | hole_hi.cmple(lo)).any() {
        return vec![(center, size)];
    }
    let mut parts = Vec::new();
    for axis in 0..3 {
        if hole_lo[axis] > lo[axis] {
            let mut cut = hi;
            cut[axis] = hole_lo[axis];
            keep(lo, cut, &mut parts);
            lo[axis] = hole_lo[axis];
        }
        if hole_hi[axis] < hi[axis] {
            let mut cut = lo;
            cut[axis] = hole_hi[axis];
            keep(cut, hi, &mut parts);
            hi[axis] = hole_hi[axis];
        }
    }
    parts
}

/// Push a remainder, unless the cut left a sliver too thin to be a wall.
fn keep(lo: Vec3, hi: Vec3, parts: &mut Vec<(Vec3, Vec3)>) {
    let span = hi - lo;
    if span.min_element() > 1e-4 {
        parts.push(((lo + hi) * 0.5, span));
    }
}

/// A `--view` preset parked in an attached room, as `(eye, yaw, pitch)`.
///
/// Dev tooling, and derived like everything else: it asks the graph where
/// the room is rather than remembering where it was last time. Attach the
/// trade room somewhere else and the preset follows it.
///
/// - `trade` / `wreck` / `parlor` / `pump`: inside that room, facing its
///   own aft band — where the stock stands and the handshake is set.
/// - `offer`: inside the trade room, facing its offer band.
/// - `burner`: inside the furnace, facing the firebox.
/// - `door`: in the cabin, facing the seam a calling room came in by.
#[must_use]
pub fn preset(rooms: &Rooms, name: &str) -> Option<(Vec3, f32, f32)> {
    let look = |from: Vec3, at: Vec3, pitch: f32| {
        let d = at - from;
        (from, (-d.x).atan2(-d.z), pitch)
    };
    let inside = |kind: RoomKind, wall: u8, pitch: f32| {
        let id = rooms.find(kind)?;
        let room = rooms.get(id)?;
        let placed = placed(id, room);
        let mid = (placed.lo + placed.hi) * 0.5;
        let eye = Vec3::new(mid.x, EYE_HEIGHT, mid.z);
        let face = wall_out(wall, placed.yaw);
        Some(look(
            eye - face * ((placed.hi - placed.lo) * face.abs()).length() * 0.22,
            eye + face,
            pitch,
        ))
    };
    match name {
        // The cabin's own floor hatch, from a body's length away and
        // looking down at it — the reading the playtest could not make.
        "hatch" => {
            let cabin = placed(CABIN, rooms.get(CABIN)?);
            let site = cabin
                .ports
                .iter()
                .find(|site| matches!(site.declared, Some(Port::Hatch { .. })))?;
            let eye = Vec3::new(site.leaf.x, EYE_HEIGHT, site.leaf.z + 1.15);
            let down = site.leaf - eye;
            Some(look(eye, site.leaf, down.y.atan2(down.xz().length())))
        }
        "trade" => inside(RoomKind::Trade, 0, -0.30),
        "offer" => inside(RoomKind::Trade, 2, -0.18),
        "wreck" => inside(RoomKind::Wreck, 0, -0.10),
        "parlor" => inside(RoomKind::Parlor, 0, -0.10),
        "pump" => inside(RoomKind::Pump, 0, -0.10),
        // The furnace is entered through its own door, so the eye stands
        // by the doorway and looks at the fire in the far wall.
        "burner" => inside(RoomKind::Burner, 1, -0.06),
        // The cabin side of whatever seam is open: the doorway a calling
        // room came in by, or the furnace's if nothing is alongside.
        // On the stoop of whatever seam is open, and standing back from
        // it. `seam` is where a ducking body's eye actually is while it
        // passes through: an aperture is two courses of the cargo grid,
        // which is a hatch, not a hall.
        "seam" | "door" => {
            let cabin = placed(CABIN, rooms.get(CABIN)?);
            let site = cabin
                .ports
                .iter()
                .filter(|site| site.is_door() && site.mate.is_some())
                .max_by_key(|site| site.mate.map(|(other, _)| other))?;
            let stoop = name == "seam";
            // Stand off the jamb's inboard flank, so the doorway is seen
            // rather than filled by whatever the room keeps beside it.
            let flank = site.half_a.normalize_or_zero();
            let toward = Vec3::new(
                f32::midpoint(cabin.lo.x, cabin.hi.x),
                0.0,
                f32::midpoint(cabin.lo.z, cabin.hi.z),
            ) - Vec3::new(site.leaf.x, 0.0, site.leaf.z);
            let flank = if flank.dot(toward) < 0.0 {
                -flank
            } else {
                flank
            };
            let (back, aside, high, pitch) = if stoop {
                (0.10, 0.0, crate::rig::DUCK_HEIGHT, 0.0)
            } else {
                (1.9, 0.9, EYE_HEIGHT, -0.16)
            };
            let eye = Vec3::new(site.leaf.x, high, site.leaf.z) - site.out * back + flank * aside;
            Some(look(eye, Vec3::new(site.leaf.x, 0.55, site.leaf.z), pitch))
        }
        _ => None,
    }
}

// ---- The plan, rebuilt from the graph ----

/// Read the sim's graph and, when its shape changed, re-derive the plan
/// and the walk envelope. Runs before anything that reads either.
pub fn survey(shell: Res<Shell>, mut plan: ResMut<Plan>, mut envelope: ResMut<Envelope>) {
    let rooms = shell.bridge.sim.rooms();
    let signature: Vec<Shape> = rooms
        .iter()
        .map(|(id, room)| {
            let mates = room
                .mates
                .iter()
                .enumerate()
                .fold(0_u32, |acc, (port, at)| {
                    acc | (u32::from(at.is_some()) << port)
                });
            (
                id,
                room.kind.token(),
                room.pose.x,
                room.pose.y,
                room.pose.z,
                room.pose.yaw,
                mates,
            )
        })
        .collect();
    if signature == plan.signature {
        return;
    }
    plan.signature = signature;
    plan.rooms = rooms.iter().map(|(id, room)| placed(id, room)).collect();
    *envelope = walk_boxes(&plan.rooms);
}

/// The walk envelope: a box per room, joined at every mated doorway.
///
/// The cabin's box is authored, because its hull is — it stands a gutter
/// of desk furniture forward of its own floor box that the lattice knows
/// nothing about. Every attached room insets its lattice box by a body's
/// half-width, and each mated door adds a connector reaching a body's
/// width into the rooms on both sides. Hull collision, per-room boxes
/// joined at apertures, and nothing else.
///
/// The vertical pair is deliberately not joined: a ladder is not walkable
/// until the camera can climb, so hatches and ladders stay sealed plates
/// this stage.
#[must_use]
pub fn walk_boxes(rooms: &[Placed]) -> Envelope {
    let mut envelope = Envelope::default();
    for placed in rooms {
        if placed.id == CABIN {
            envelope.rooms.push((WALK_MIN, WALK_MAX));
        } else {
            let lo = placed.lo + Vec3::new(BODY, 0.0, BODY);
            let hi = placed.hi - Vec3::new(BODY, 0.0, BODY);
            if hi.x > lo.x && hi.z > lo.z {
                envelope.rooms.push((
                    Vec3::new(lo.x, EYE_HEIGHT, lo.z),
                    Vec3::new(hi.x, EYE_HEIGHT, hi.z),
                ));
            }
        }
        for site in &placed.ports {
            let Some((other, _)) = site.mate else {
                continue;
            };
            if !site.is_door() || other < placed.id {
                // One connector per seam, drawn by the lower id — the
                // shared-partition convention, one scale up.
                continue;
            }
            let reach = BODY + WALL_T;
            let along = site.half_a.abs() - site.half_a.normalize_or_zero().abs() * BODY;
            let half = along.max(Vec3::ZERO) + site.out.abs() * reach;
            let centre = Vec3::new(site.leaf.x, EYE_HEIGHT, site.leaf.z);
            envelope.seams.push((centre - half, centre + half));
        }
    }
    envelope
}

// ---- Building the rooms ----

/// What has been built, so a graph change rebuilds exactly what changed.
#[derive(Resource, Default)]
struct Built(Vec<Shape>);

pub struct RoomsPlugin;

impl Plugin for RoomsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Plan>()
            .init_resource::<Envelope>()
            .init_resource::<Occupancy>()
            .init_resource::<AimedLatch>()
            .init_resource::<SeamFx>()
            .init_resource::<Built>()
            .add_systems(PostStartup, spawn_claim_pool)
            .add_systems(
                Update,
                (survey, rebuild, occupy, aim_latch)
                    .chain()
                    .in_set(Phase::Input)
                    .before(crate::rig::steer),
            )
            .add_systems(Update, (seam_fx, claim_frames).in_set(Phase::View));
    }
}

/// Spawn what the plan says and despawn what it no longer says. Rooms are
/// rebuilt wholesale when the graph changes: a room is a handful of slabs
/// and a few dozen decals, and a diff finer than "this room" would be
/// machinery guarding nothing.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    plan: Res<Plan>,
    skin: Option<Res<Skin>>,
    fade: Option<Res<TileFade>>,
    shared: Option<Res<crate::pieces::SharedBits>>,
    mut built: ResMut<Built>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    standing: Query<(Entity, &InRoom)>,
) {
    let (Some(skin), Some(fade), Some(shared)) = (skin, fade, shared) else {
        return;
    };
    if built.0 == plan.signature {
        return;
    }
    built.0.clone_from(&plan.signature);
    for (entity, _) in &standing {
        commands.entity(entity).despawn();
    }
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for placed in &plan.rooms {
        let tag = InRoom {
            room: placed.id,
            kind: placed.kind,
        };
        for (station, surface) in placed.charts {
            commands.spawn((station, surface, tag));
        }
        if placed.id != CABIN {
            // The cabin's own hull is `rig::structure`'s, authored and
            // standing since before the lattice; everything else grows
            // its shell from its box.
            shell_slabs(&mut commands, &cube, &skin, placed, &plan.rooms, tag);
        }
        if !placed.kind.riding() {
            let mid = (placed.lo + placed.hi) * 0.5;
            commands.spawn((
                PointLight {
                    color: palette::GLINT,
                    intensity: CALLER_LUMENS,
                    range: CALLER_RANGE,
                    // No shadow maps anywhere, on purpose: the art
                    // direction is light VOLUMES, not simulated occlusion.
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(Vec3::new(mid.x, placed.hi.y - 0.75, mid.z)),
                crate::rig::Dimmable {
                    intensity: CALLER_LUMENS,
                },
                tag,
            ));
        }
        tiles(
            &mut commands,
            &cube,
            &mut materials,
            &skin,
            &fade,
            placed,
            tag,
        );
        doorways(&mut commands, &cube, &mut materials, &skin, placed, tag);
        handshake(&mut commands, &cube, &mut materials, &skin, placed, tag);
        crate::pieces::hint_cells(&mut commands, &cube, &mut materials, &shared, placed);
        crate::airlock::fittings(
            &mut commands,
            &cube,
            &mut meshes,
            &mut materials,
            &skin,
            placed,
            tag,
        );
    }
}

/// A **calling** room lights its own premises.
///
/// The ship owns not one lumen — every light aboard is cargo, and
/// lights-out is a legal state (docs/BAY.md) — but a room that came
/// alongside is not the ship's, and a station that let you trade in the
/// dark would be a station with something to hide. So the callers arrive
/// lit and take their light with them when they part, while the cabin and
/// the burner stay exactly as dark as the crew's own lamps leave them.
/// It dims with everything else when the omen leans on the ship.
const CALLER_LUMENS: f32 = 260_000.0;
const CALLER_RANGE: f32 = 7.5;

/// A room's hull: deck, ceiling, and four walls, each punched by whatever
/// apertures its own ports declare and by whatever a lower-id room's box
/// already fills. Two rooms may share a plane and may never share a slab
/// (docs/ROOMS.md, "Walls have no thickness on the lattice").
fn shell_slabs(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    skin: &Skin,
    placed: &Placed,
    all: &[Placed],
    tag: InRoom,
) {
    for (center, size, hull) in shell_boxes(placed, all) {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(if hull {
                skin.hull.clone()
            } else {
                skin.desk.clone()
            }),
            Transform::from_translation(center).with_scale(size),
            tag,
        ));
    }
}

/// [`shell_slabs`]' geometry, as `(centre, size, is_hull)`. Pure, so the
/// no-clipping law can be asserted against exactly what gets drawn.
#[must_use]
fn shell_boxes(placed: &Placed, all: &[Placed]) -> Vec<(Vec3, Vec3, bool)> {
    let (lo, hi) = (placed.lo, placed.hi);
    let span = hi - lo;
    let plan_centre = Vec3::new(f32::midpoint(lo.x, hi.x), 0.0, f32::midpoint(lo.z, hi.z));
    let mut slabs: Vec<(Vec3, Vec3, bool)> = vec![
        (
            Vec3::new(plan_centre.x, WALL_T.mul_add(-0.5, lo.y), plan_centre.z),
            Vec3::new(span.x + WALL_T, WALL_T, span.z + WALL_T),
            false,
        ),
        (
            Vec3::new(plan_centre.x, lo.y + CEIL_SLAB_Y, plan_centre.z),
            Vec3::new(span.x + WALL_T, WALL_T, span.z + WALL_T),
            true,
        ),
    ];
    let y0 = lo.y - WALL_T;
    let y1 = WALL_T.mul_add(0.8, lo.y + CEIL_SLAB_Y);
    for wall in 0..4_u8 {
        let out = wall_out(wall, placed.yaw);
        let along = axis_along(out);
        // The slab stands wholly OUTSIDE the box: a room occupies its
        // interior, and the partition between two rooms is a boundary,
        // not a volume (docs/ROOMS.md). A wall centred on the face would
        // swallow its own chart — and with it every tile painted on it.
        let face = plan_centre + out * (span * out.abs()).length().mul_add(0.5, WALL_T * 0.5);
        let length = WALL_T.mul_add(2.0, (span * along.abs()).length());
        slabs.push((
            Vec3::new(face.x, f32::midpoint(y0, y1), face.z),
            out.abs() * WALL_T + along.abs() * length + Vec3::Y * (y1 - y0),
            true,
        ));
    }
    let mut out = Vec::new();
    for (center, size, hull) in slabs {
        let mut parts = vec![(center, size)];
        for site in &placed.ports {
            parts = parts
                .into_iter()
                .flat_map(|(c, s)| punch(c, s, site.lo, site.hi))
                .collect();
        }
        // Whatever ANOTHER room's box already fills is that room's to
        // draw; grown by a wall so the two hulls meet rather than gape.
        // Every other room, not merely the older ones: a shell stands in
        // the partition, and a partition has two sides — cutting only
        // against the lower ids left the newer neighbour's half of it
        // standing inside a room that never asked for it.
        for other in all.iter().filter(|other| other.id != placed.id) {
            let grow = Vec3::new(WALL_T, 0.0, WALL_T);
            parts = parts
                .into_iter()
                .flat_map(|(c, s)| {
                    punch(
                        c,
                        s,
                        other.lo - grow - Vec3::Y * WALL_T,
                        other.hi + grow + Vec3::Y * WALL_T,
                    )
                })
                .collect();
        }
        out.extend(parts.into_iter().map(|(c, s)| (c, s, hull)));
    }
    out
}

// ---- Colored tiles ----

/// The colored tiles, painted (docs/ROOMS.md, "The tile-class
/// vocabulary"). The class is declared once by the room kind; the rules
/// and the paint read the same declaration, so a tile that *looks* like an
/// offer area and does not behave like one cannot exist.
///
/// **No signal on hue alone.** Every class carries a border form as well
/// as a hue: the offer's chalk outline and chevrons, the stock's hatched
/// field, the burner's ember-edged hazard, the doorway's doormat stripes.
#[allow(clippy::too_many_lines)]
fn tiles(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    fade: &TileFade,
    placed: &Placed,
    tag: InRoom,
) {
    let (cols, rows) = placed.kind.grid();
    // One material instance per class per room: paint is shared, and
    // nothing here changes brightness per frame.
    let offer_field = glow::enamel(materials, palette::TRIM_GIVE);
    let chalk = glow::etched(materials, palette::GLINT);
    let stock_field = glow::enamel(materials, palette::TRIM_SHELF);
    let hatch = glow::enamel(materials, palette::PLATE_SHADE);
    let ember = glow::phosphor(materials, palette::EMBER, 0.9);
    for (station, surface) in placed.charts {
        let normal = station.inward(&surface);
        let rot = station.face(&surface);
        let (su, sv) = (surface.scale_u(), surface.scale_v());
        for y in 0..rows {
            for x in 0..cols {
                let cell = layout::cell_rect(placed.id, x, y);
                let mid = space_trucking::sim::Vec2::new(
                    cell.w.mul_add(0.5, cell.x),
                    cell.h.mul_add(0.5, cell.y),
                );
                if !surface.rect.contains(mid) {
                    continue;
                }
                let Some(tile) = placed.kind.tile_of(x, y) else {
                    continue;
                };
                let at = surface.to_world(mid);
                let mut paint = |mat: &Handle<StandardMaterial>,
                                 lift: f32,
                                 offset: Vec2,
                                 scale: Vec2,
                                 spin: f32| {
                    commands.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_translation(
                            at + normal * lift
                                + rot
                                    * Vec3::new(
                                        offset.x * cell.w * su,
                                        offset.y * cell.h * sv,
                                        0.0,
                                    ),
                        )
                        .with_rotation(rot * Quat::from_rotation_z(spin))
                        .with_scale(Vec3::new(
                            scale.x * cell.w * su,
                            scale.y * cell.h * sv,
                            crate::rig::layer::SKIN,
                        )),
                        tag,
                    ));
                };
                match tile {
                    // Ordinary deck: the contextual berth well, raised by
                    // `rig::fade_tiles` only while a carry asks "where can
                    // this go?".
                    Tile::Plain => {
                        commands.spawn((
                            Mesh3d(cube.clone()),
                            MeshMaterial3d(fade.mat.clone()),
                            Transform::from_translation(at + normal * crate::rig::layer::TILE)
                                .with_rotation(rot)
                                .with_scale(Vec3::new(
                                    (cell.w - 4.0) * su,
                                    (cell.h - 4.0) * sv,
                                    crate::rig::layer::SKIN,
                                )),
                            crate::rig::BerthTile,
                            tag,
                        ));
                    }
                    // A chalked square in the room's own enamel, chevroned
                    // the way a proposal travels: yours until a resolution
                    // says otherwise.
                    Tile::Offer => {
                        paint(
                            &offer_field,
                            crate::rig::layer::TILE,
                            Vec2::ZERO,
                            Vec2::new(0.92, 0.92),
                            0.0,
                        );
                        for edge in [-0.44_f32, 0.44] {
                            paint(
                                &chalk,
                                crate::rig::layer::DOORMAT,
                                Vec2::new(edge, 0.0),
                                Vec2::new(0.07, 0.94),
                                0.0,
                            );
                            paint(
                                &chalk,
                                crate::rig::layer::DOORMAT,
                                Vec2::new(0.0, edge),
                                Vec2::new(0.94, 0.07),
                                0.0,
                            );
                        }
                        for row in [-0.16_f32, 0.16] {
                            for side in [-1.0_f32, 1.0] {
                                paint(
                                    &chalk,
                                    crate::rig::layer::DOORMAT,
                                    Vec2::new(side * 0.15, row),
                                    Vec2::new(0.34, 0.08),
                                    side * 0.6,
                                );
                            }
                        }
                    }
                    // The room's enamel, filled and hatched like a ledger
                    // column: these goods are the room's own.
                    Tile::Stock => {
                        paint(
                            &stock_field,
                            crate::rig::layer::TILE,
                            Vec2::ZERO,
                            Vec2::new(0.96, 0.96),
                            0.0,
                        );
                        for step in [-0.3_f32, 0.0, 0.3] {
                            paint(
                                &hatch,
                                crate::rig::layer::DOORMAT,
                                Vec2::new(step, 0.0),
                                Vec2::new(0.10, 1.15),
                                0.7,
                            );
                        }
                    }
                    // Hazard field, ember-edged: what lands here is
                    // scheduled for destruction on the room's own beat.
                    Tile::Consume => {
                        paint(
                            &skin.hazard,
                            crate::rig::layer::TILE,
                            Vec2::ZERO,
                            Vec2::new(0.94, 0.94),
                            0.0,
                        );
                        for edge in [-0.42_f32, 0.42] {
                            paint(
                                &ember,
                                crate::rig::layer::DOORMAT,
                                Vec2::new(0.0, edge),
                                Vec2::new(0.92, 0.09),
                                0.0,
                            );
                        }
                    }
                    // An aperture's own cells are the OPENING: a leaf
                    // hangs there when the port is shut and there is
                    // nothing but air when it is open, so painting them
                    // would stripe a doorway you can walk through. The
                    // doormat that reads the threshold is laid on the
                    // deck the door stands on instead ([`doormats`]) —
                    // derived from the very same declaration.
                    Tile::Threshold => {}
                }
            }
        }
    }
    doormats(commands, cube, skin, placed, tag);
}

/// The doormat: hazard stripes on the deck cells each of a room's doors
/// stands on. The threshold rule's own reading, laid where a body can see
/// it — the doorway stays clear because it is shared space, and the
/// stripes say so from either side.
///
/// Paint only: those cells are ordinary berths, and cargo may stand on the
/// stripes like anywhere else. But a doorway that reads as a doorway is
/// worth the paint, and the way out to the fire is still where the
/// stripes say it is. The cells come from the port declaration, so the
/// mat can never be somewhere the door is not.
fn doormats(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    skin: &Skin,
    placed: &Placed,
    tag: InRoom,
) {
    let Some(floor) = placed.chart(Station::BayFloor).copied() else {
        return;
    };
    let normal = Station::BayFloor.inward(&floor);
    let rot = Station::BayFloor.face(&floor);
    let (su, sv) = (floor.scale_u(), floor.scale_v());
    for site in &placed.ports {
        let Some(Port::Door { wall, offset }) = site.declared else {
            continue;
        };
        for (i, j) in door_cells(placed.kind, wall, offset) {
            let cell = layout::cell_rect(placed.id, COURSES + i, COURSES + j);
            let mid = space_trucking::sim::Vec2::new(
                cell.w.mul_add(0.5, cell.x),
                cell.h.mul_add(0.5, cell.y),
            );
            let at = floor.to_world(mid) + normal * crate::rig::layer::DOORMAT;
            for stripe in [-0.28_f32, 0.0, 0.28] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(skin.hazard.clone()),
                    Transform::from_translation(
                        at + rot * Vec3::new(0.0, stripe * cell.h * sv, 0.0),
                    )
                    .with_rotation(rot)
                    .with_scale(Vec3::new(
                        0.86 * cell.w * su,
                        0.17 * cell.h * sv,
                        crate::rig::layer::SKIN,
                    )),
                    tag,
                ));
            }
        }
    }
}

// ---- Doorways ----

/// **The seam is drawn once, by the room with the lower id, and it is
/// drawn in the seam.** Two rooms share a partition, and a partition is a
/// boundary rather than a volume (docs/ROOMS.md) — so a mated aperture's
/// hardware may not be built twice, once per side, at two planes a trim
/// apart. That is what put a jamb of the furnace inside the cabin and a
/// jamb of the cabin inside the furnace, interpenetrating, with the
/// station's own hatching showing through the sliver between them.
///
/// Whether this room dresses this seam: only for a mated door, and only
/// from the lower id. The other side gets the same frame, because there
/// is only one frame and it stands on the boundary they share.
const fn dresses(placed: &Placed, site: &Site) -> bool {
    match site.mate {
        Some((other, _)) => site.is_door() && placed.id <= other,
        None => false,
    }
}

/// The point at the middle of a door's opening ON THE SHARED PARTITION —
/// the room's own box face, not the plane its chart was trimmed to. Seam
/// hardware is centred here so one frame reads from both rooms.
fn seam_centre(placed: &Placed, site: &Site) -> Vec3 {
    let face = if site.out.x + site.out.z > 0.0 {
        placed.hi
    } else {
        placed.lo
    };
    Vec3::new(
        if site.out.x.abs() > 0.5 {
            face.x
        } else {
            site.leaf.x
        },
        site.leaf.y,
        if site.out.z.abs() > 0.5 {
            face.z
        } else {
            site.leaf.z
        },
    )
}

/// Every box a mated door's frame occupies, as `(centre, size)`: two
/// stiles, a lintel, the jamb lamp, and (where the room beyond can be
/// sent away) the latch's plate and its amber. Pure, because the
/// no-clipping law is asserted against exactly this list
/// (`tests::no_rooms_geometry_reaches_into_another_room`).
#[must_use]
fn seam_boxes(placed: &Placed, site: &Site) -> Vec<(Vec3, Vec3)> {
    let (a, b) = (site.half_a.length(), site.half_b.length());
    let dir_a = site.half_a.normalize_or_zero().abs();
    let dir_b = site.half_b.normalize_or_zero().abs();
    let mid = seam_centre(placed, site);
    // The frame stands in the partition and a jamb's girth either side of
    // it, so both rooms see the same doorway and neither is reached into
    // any further than the opening they share.
    let girth = site.out.abs() * (JAMB * 2.0);
    let mut boxes = vec![
        (
            mid + site.half_b.normalize_or_zero() * JAMB.mul_add(0.5, b),
            girth + dir_a * JAMB.mul_add(2.0, a * 2.0) + dir_b * JAMB,
        ),
        (
            mid + site.half_b.normalize_or_zero() * (b + JAMB),
            girth * 0.4 + dir_a * (a * 0.5) + dir_b * 0.035,
        ),
    ];
    for side in [-1.0_f32, 1.0] {
        boxes.push((
            mid + site.half_a.normalize_or_zero() * side * JAMB.mul_add(0.5, a),
            girth + dir_a * JAMB + dir_b * JAMB.mul_add(2.0, b * 2.0),
        ));
    }
    if latch_at(placed, site).is_some() {
        let at = latch_at(placed, site).unwrap_or(mid);
        boxes.push((
            at,
            dir_a * (LATCH_W * 1.6) + Vec3::Y * (LATCH_H * 1.3) + site.out.abs() * 0.02,
        ));
        boxes.push((
            at - site.out * 0.02,
            dir_a * LATCH_W + Vec3::Y * LATCH_H + site.out.abs() * 0.02,
        ));
    }
    boxes
}

/// Where a seam's amber latch hangs, if this seam gets one: on the
/// ANCHOR's side — the lower id, which is the room that stays — and on
/// whichever flank of the jamb stands inside it. The hand that parts a
/// room is never the hand standing in it, and the gangway law would
/// refuse it if it were. The cabin does not part from itself, and a
/// riding room's seam is not asked to.
fn latch_at(placed: &Placed, site: &Site) -> Option<Vec3> {
    let (other, _) = site.mate?;
    if !site.is_door() || placed.id > other {
        return None;
    }
    let (a, b) = (site.half_a.length(), site.half_b.length());
    let mid = seam_centre(placed, site);
    let flank = site.half_a.normalize_or_zero() * (a + JAMB + LATCH_W);
    let toward = Vec3::new(
        f32::midpoint(placed.lo.x, placed.hi.x),
        0.0,
        f32::midpoint(placed.lo.z, placed.hi.z),
    ) - Vec3::new(mid.x, 0.0, mid.z);
    let flank = if flank.dot(toward) < 0.0 {
        -flank
    } else {
        flank
    };
    Some(mid - site.out * (JAMB + LATCH_W) + flank + Vec3::Y * (b * 0.25))
}

/// Every declared port, dressed. A mated door is an open aperture with a
/// jamb, a lamp that answers the seam's cues, and — where the room beyond
/// is one that can be sent away — the amber latch that asks it to part.
/// An unmated door is a leaf, drawn shut: a door facing blank wall is a
/// door that will not open. The vertical pair is neither: it is a hatch
/// in the deck and a ladder port overhead, and it is dressed to say so
/// ([`port_plate`]).
fn doorways(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    placed: &Placed,
    tag: InRoom,
) {
    for site in &placed.ports {
        let (a, b) = (site.half_a.length(), site.half_b.length());
        if a <= 0.0 || b <= 0.0 {
            continue;
        }
        if !dresses(placed, site) {
            if site.mate.is_none() {
                port_plate(commands, cube, skin, site, tag);
            }
            continue;
        }
        for (centre, size) in seam_boxes(placed, site) {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(skin.plate_shade.clone()),
                Transform::from_translation(centre).with_scale(size),
                tag,
            ));
        }
        // The jamb lamp: dark glass that answers a seam mating or parting.
        let mid = seam_centre(placed, site);
        let lamp = glow::phosphor(materials, palette::LAMP_OK, 0.0);
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(lamp.clone()),
            Transform::from_translation(mid + site.half_b.normalize_or_zero() * (b + JAMB))
                .with_scale(
                    site.out.abs() * (JAMB * 0.9)
                        + site.half_a.normalize_or_zero().abs() * (a * 0.5)
                        + site.half_b.normalize_or_zero().abs() * 0.035,
                ),
            SeamLamp {
                mat: lamp,
                latch: false,
            },
            tag,
        ));
        let (Some(at), Some((other, _))) = (latch_at(placed, site), site.mate) else {
            continue;
        };
        let dir_a = site.half_a.normalize_or_zero().abs();
        let grab = glow::phosphor(materials, palette::AMBER, 1.2);
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(grab.clone()),
            Transform::from_translation(at - site.out * 0.02)
                .with_scale(dir_a * LATCH_W + Vec3::Y * LATCH_H + site.out.abs() * 0.02),
            SeamLamp {
                mat: grab,
                latch: true,
            },
            Latch {
                room: other,
                face: SimSurface {
                    center: at - site.out * 0.04,
                    half_u: site.half_a.normalize_or_zero() * (LATCH_W * 1.2),
                    half_v: Vec3::NEG_Y * (LATCH_H * 1.2),
                    rect: layout::cell_rect(placed.id, 0, 0),
                },
            },
            tag,
        ));
    }
}

/// How far a shut vertical port's leaf sits below the deck it is set
/// into — a hatch is a thing you step ON, so it is recessed, never a
/// block you trip over. The playtest could not tell what the lump in the
/// bow-starboard corner was; this is the answer, and it is a shape
/// rather than a label.
const SINK: f32 = 0.022;

/// The coaming's width and how far it stands proud of the deck: a rim
/// you can see from across the room and still walk over.
const RIM_W: f32 = 0.07;
const RIM_H: f32 = 0.012;

/// A shut port, dressed as the thing it is.
///
/// A **door** drawn shut is a riveted plate, as it always was. The
/// **vertical pair** is not a door and must not read as one: a hatch in
/// the deck and a ladder port in the ceiling get a recessed leaf, a
/// coaming rim around the opening, a hinge barrel down one edge, and a
/// recessed pull opposite it — so the eye reads *hinged, lifts that way*
/// without a word of text.
///
/// The pull is BRASS, not amber. Amber is the handle rule's, and it
/// promises a carry: the day apertures become cargo (docs/ROOMS.md's
/// stretch goal) this hardware grows an amber grab and means it. Until
/// then the radium brass says "hardware, findable in the dark", which is
/// the truth.
fn port_plate(commands: &mut Commands, cube: &Handle<Mesh>, skin: &Skin, site: &Site, tag: InRoom) {
    let leaf = site.out.abs() * PLATE_T + site.half_a.abs() * 2.0 + site.half_b.abs() * 2.0;
    if site.is_door() {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(skin.plate.clone()),
            Transform::from_translation(site.leaf).with_scale(leaf.max(Vec3::splat(0.02))),
            tag,
        ));
        for corner in [-0.66_f32, 0.66] {
            for course in [-0.66_f32, 0.66] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(skin.rivet.clone()),
                    Transform::from_translation(
                        site.leaf
                            + site.half_a * corner
                            + site.half_b * course
                            + site.out * PLATE_T,
                    )
                    .with_scale(Vec3::splat(0.045)),
                    tag,
                ));
            }
        }
        return;
    }
    // The vertical pair. `out` points out of the room (down through a
    // hatch, up through a ladder port), so `-out` is always "into the
    // room" and the whole fitting is written once for both.
    let (a, b) = (site.half_a.length(), site.half_b.length());
    let dir_a = site.half_a.normalize_or_zero().abs();
    let dir_b = site.half_b.normalize_or_zero().abs();
    let flat = site.out.abs();
    // The leaf, sunk into the opening: nothing of it stands proud.
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.plate.clone()),
        Transform::from_translation(site.leaf + site.out * PLATE_T.mul_add(0.5, SINK))
            .with_scale(dir_a * (a * 2.0) + dir_b * (b * 2.0) + flat * PLATE_T),
        tag,
    ));
    // The coaming: four bars around the opening, a rim's height proud.
    for (along, across, half) in [
        (dir_a, dir_b, b),
        (dir_a, dir_b, -b),
        (dir_b, dir_a, a),
        (dir_b, dir_a, -a),
    ] {
        let axis = if along == dir_a {
            site.half_b.normalize_or_zero()
        } else {
            site.half_a.normalize_or_zero()
        };
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(skin.plate_shade.clone()),
            Transform::from_translation(
                site.leaf + axis * (half.signum() * RIM_W.mul_add(0.5, half.abs()))
                    - site.out * (RIM_H * 0.5),
            )
            .with_scale(
                along * RIM_W.mul_add(2.0, if along == dir_a { a * 2.0 } else { b * 2.0 })
                    + across * RIM_W
                    + flat * RIM_H,
            ),
            tag,
        ));
    }
    // The hinge: two brass barrels down one edge of the sunk leaf.
    for step in [-0.45_f32, 0.45] {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(skin.brass.clone()),
            Transform::from_translation(
                site.leaf + site.half_a * -0.86 + site.half_b * step + site.out * (SINK * 0.5)
                    - site.out * 0.002,
            )
            .with_scale(dir_a * (a * 0.18) + dir_b * (b * 0.34) + flat * (SINK * 0.7)),
            tag,
        ));
    }
    // And the pull, opposite the hinge: a brass bar lying in a shadowed
    // well, which is what a flush deck fitting looks like — the well
    // reaches the deck plane so the shadow reads, the bar sits inside it
    // so the hand knows where to reach.
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.socket.clone()),
        Transform::from_translation(site.leaf + site.half_a * 0.62 + site.out * (SINK * 0.5))
            .with_scale(dir_a * (a * 0.5) + dir_b * (b * 0.42) + flat * SINK),
        tag,
    ));
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.brass.clone()),
        Transform::from_translation(site.leaf + site.half_a * 0.62 + site.out * (SINK * 0.06))
            .with_scale(dir_a * (a * 0.30) + dir_b * (b * 0.09) + flat * (SINK * 0.30)),
        tag,
    ));
}

// ---- The handshake ----

/// The room's one click-functional fixture: the handshake where a deal is
/// struck (docs/ROOMS.md, "The new barter: six beats", step five). One
/// honest core form for every room that has one — brass plate, brass
/// plunger, one lamp — because per-POI differentiation is a later stage.
/// The handle rule reads a body click as *function* (BAY.md), and on a
/// fixture set into the room's fabric the whole body IS the function: it
/// wears no grab, because it is not cargo and cannot be carried.
///
/// The fixture's own face is a mapped surface bound to its declared cell,
/// standing proud of the wall so the crosshair meets the brass rather than
/// the chart behind it. The sim does the rest: `Sim::handshake_at` reads
/// the cell, `work_handshake` commits, and the cue says how it went.
fn handshake(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    placed: &Placed,
    tag: InRoom,
) {
    let Some((hx, hy)) = placed.kind.handshake() else {
        return;
    };
    let cell = layout::cell_rect(placed.id, hx, hy);
    let mid =
        space_trucking::sim::Vec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
    let Some((station, surface)) = placed
        .charts
        .iter()
        .find(|(_, surface)| surface.rect.contains(mid))
    else {
        return;
    };
    let normal = station.inward(surface);
    let rot = station.face(surface);
    let at = surface.to_world(mid);
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    let plate = Vec3::new(cell.w * su * 0.82, cell.h * sv * 0.82, 0.03);
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.brass.clone()),
        Transform::from_translation(at + normal * 0.015)
            .with_rotation(rot)
            .with_scale(plate),
        tag,
    ));
    // The plunger: a brass slug that visibly throws when it is worked.
    let rest = at + normal * 0.075;
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.brass.clone()),
        Transform::from_translation(rest)
            .with_rotation(rot)
            .with_scale(Vec3::new(plate.x * 0.42, plate.y * 0.42, 0.09)),
        HandshakeThrow {
            rest,
            travel: -normal * 0.045,
        },
        tag,
    ));
    // Its one lamp: lit while there is something to commit.
    let lamp = glow::phosphor(materials, palette::AMBER, 0.0);
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(lamp.clone()),
        Transform::from_translation(
            at + normal * 0.05 + rot * Vec3::new(0.0, -plate.y * 0.62, 0.0),
        )
        .with_rotation(rot)
        .with_scale(Vec3::new(plate.x * 0.5, plate.y * 0.16, 0.02)),
        HandshakeLamp {
            room: placed.id,
            mat: lamp,
        },
        tag,
    ));
    // The pick face: the fixture's own body, bound to its own cell and
    // standing proud of the chart it is set into, so the aim meets the
    // brass the player is looking at.
    commands.spawn((
        Station::Handshake,
        SimSurface {
            center: at + normal * 0.10,
            half_u: rot * (Vec3::X * (cell.w * su * 0.5)),
            half_v: rot * (Vec3::NEG_Y * (cell.h * sv * 0.5)),
            rect: cell,
        },
        tag,
    ));
}

// ---- The body, and the cues ----

/// Which room the eye stands in, fed to the sim as the occupied-room
/// field. A body in a doorway keeps the room it came from until it is
/// wholly inside another; a body whose room parted falls back to the
/// cabin, which is the only room that cannot leave.
pub fn occupy(
    plan: Res<Plan>,
    camera: Single<&Transform, With<crate::rig::CabinCamera>>,
    mut occupancy: ResMut<Occupancy>,
) {
    if let Some(room) = plan.room_at(camera.translation) {
        occupancy.0 = room;
    } else if plan.get(occupancy.0).is_none() {
        occupancy.0 = CABIN;
    }
}

/// Which detach latch the crosshair rests on. Roam only, within reach:
/// the latch is hardware you walk up to, exactly like a berth.
pub fn aim_latch(
    rig: Res<crate::rig::CameraRig>,
    camera: Single<&Transform, With<crate::rig::CabinCamera>>,
    latches: Query<&Latch>,
    mut aimed: ResMut<AimedLatch>,
) {
    aimed.0 = None;
    if !rig.roaming() {
        return;
    }
    let Ok(dir) = Dir3::new(camera.forward().into()) else {
        return;
    };
    let ray = Ray3d::new(camera.translation, dir);
    let mut nearest = REACH;
    for latch in &latches {
        if let Some((t, _, _)) = latch.face.project(ray)
            && t < nearest
        {
            nearest = t;
            aimed.0 = Some(latch.room);
        }
    }
}

/// The seam's own feedback: a mate or a part pulses every jamb lamp green,
/// a refused refit strobes them red the way a violation flashes, the
/// latches breathe amber while a seam could be worked, and the handshake's
/// plunger throws when it is used.
#[allow(clippy::too_many_arguments)]
fn seam_fx(
    time: Res<Time>,
    shell: Res<Shell>,
    aimed: Res<AimedLatch>,
    mut fx: ResMut<SeamFx>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lamps: Query<&SeamLamp>,
    hands: Query<&HandshakeLamp>,
    mut throws: Query<(&HandshakeThrow, &mut Transform)>,
) {
    let dt = time.delta_secs();
    fx.refit = (fx.refit - dt).max(0.0);
    fx.seam = (fx.seam - dt).max(0.0);
    fx.throw = (fx.throw - dt).max(0.0);
    for cue in shell.bridge.sim.cues() {
        match cue {
            Cue::Attached | Cue::Parted => fx.seam = SEAM_LEN,
            Cue::Refit { .. } => fx.refit = SEAM_LEN,
            Cue::Accept { .. } | Cue::Refuse => fx.throw = SEAM_LEN * 0.5,
            _ => {}
        }
    }
    let t = time.elapsed_secs();
    for lamp in &lamps {
        let Some(mut mat) = materials.get_mut(&lamp.mat) else {
            continue;
        };
        if fx.refit > 0.0 {
            // Hard on/off at strobe cadence: motion and brightness carry
            // the refusal, never hue alone.
            let on = (t * 22.0).sin() > 0.0;
            glow::set_lamp(&mut mat, palette::LAMP_NO, if on { 1.0 } else { 0.1 });
        } else if fx.seam > 0.0 {
            glow::set_lamp(&mut mat, palette::LAMP_OK, fx.seam / SEAM_LEN);
        } else if lamp.latch {
            let aim = aimed.0.is_some();
            let level = glow::breathe(t, 2.0, 0.0).mul_add(0.2, if aim { 0.75 } else { 0.4 });
            glow::set_lamp(&mut mat, palette::AMBER, level);
        } else {
            glow::set_lamp(&mut mat, palette::LAMP_OK, 0.0);
        }
    }
    // The handshake's lamp reads the sim's own answer: something to
    // commit, or a throw that would find nothing.
    let sim = &shell.bridge.sim;
    for hand in &hands {
        let Some(mut mat) = materials.get_mut(&hand.mat) else {
            continue;
        };
        let ready = sim.pieces().iter().any(|piece| {
            matches!(piece.loc, Loc::Hold { room, x, y }
                if room == hand.room && sim.rooms().tile(room, x, y) == Some(Tile::Offer))
        }) || !sim.marks().is_empty();
        let level = if ready {
            glow::breathe(t, 1.8, 0.0).mul_add(0.25, 0.6)
        } else {
            0.08
        };
        glow::set_lamp(&mut mat, palette::AMBER, level);
    }
    for (throw, mut transform) in &mut throws {
        let press = (fx.throw / (SEAM_LEN * 0.5)).clamp(0.0, 1.0);
        transform.translation = throw.rest + throw.travel * press;
    }
}

// ---- The composed offer, lit ----

/// How many claim bars stand ready: four per piece, for a pile of eight.
const CLAIM_BARS: usize = 32;

/// The claim frame's rung on the decal ladder — over everything else on
/// the cell, because it is a standing reading rather than a flash.
const CLAIM_LIFT: f32 = crate::rig::layer::CLAIM;

/// Pre-spawn the claim bars, dark. The composed offer is derived by the
/// sim every frame and never stored, so the presentation keeps a pool and
/// aims it — the pieces themselves never move.
fn spawn_claim_pool(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat = glow::phosphor(&mut materials, palette::AMBER, 2.0);
    for i in 0..CLAIM_BARS {
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            ClaimBar(i as u8),
        ));
    }
}

/// **The composed offer is LIT, not moved.** `Sim::composed` names the
/// pile the room would hand over if the handshake were worked right now;
/// this frames each of those pieces where it already stands on the room's
/// stock, so the reading is "this pile is what's on offer for yours"
/// without a single piece changing berth.
fn claim_frames(
    shell: Res<Shell>,
    plan: Res<Plan>,
    mut bars: Query<(&ClaimBar, &mut Transform, &mut Visibility)>,
) {
    let sim = &shell.bridge.sim;
    let composed = sim.composed();
    // Each piece's footprint, on the chart it stands on.
    let mut frames: Vec<(Vec3, Quat, f32, f32)> = Vec::new();
    for id in composed {
        let Some(piece) = sim.pieces().iter().find(|piece| piece.id == id) else {
            continue;
        };
        let rect = layout::piece_rect(sim.pieces(), piece);
        let centre = space_trucking::sim::Vec2::new(
            rect.w.mul_add(0.5, rect.x),
            rect.h.mul_add(0.5, rect.y),
        );
        let Some((_, station, surface)) = plan.chart_at(centre) else {
            continue;
        };
        let at = surface.to_world(centre) + station.inward(&surface) * CLAIM_LIFT;
        frames.push((
            at,
            station.face(&surface),
            rect.w * surface.scale_u(),
            rect.h * surface.scale_v(),
        ));
    }
    for (bar, mut transform, mut visibility) in &mut bars {
        let slot = usize::from(bar.0);
        let Some(&(at, rot, w, h)) = frames.get(slot / 4) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        let girth = 0.012_f32;
        let (offset, scale) = match slot % 4 {
            0 => (Vec3::new(0.0, h * 0.5, 0.0), Vec3::new(w, girth, girth)),
            1 => (Vec3::new(0.0, -h * 0.5, 0.0), Vec3::new(w, girth, girth)),
            2 => (Vec3::new(w * 0.5, 0.0, 0.0), Vec3::new(girth, h, girth)),
            _ => (Vec3::new(-w * 0.5, 0.0, 0.0), Vec3::new(girth, h, girth)),
        };
        *transform = Transform::from_translation(at + rot * offset)
            .with_rotation(rot)
            .with_scale(scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The anchor is honest: the cabin's own lattice box maps onto the
    /// floor chart the bay was authored with, cell for cell. Everything
    /// else in this module is arithmetic on this.
    #[test]
    fn the_cabin_box_is_the_bay_it_always_was() {
        let cabin = cabin_room();
        let (lo, hi) = room_box(&cabin);
        assert!((lo.x - -2.2).abs() < 1e-4, "port wall at {}", lo.x);
        assert!((hi.x - 2.2).abs() < 1e-4, "starboard wall at {}", hi.x);
        assert!((hi.z - BAY_WALL_Z).abs() < 1e-4, "aft wall at {}", hi.z);
        assert!((lo.z - -1.44).abs() < 1e-3, "front floor edge at {}", lo.z);
    }

    /// The derived charts ARE the authored bay: the generalization moved
    /// no cabin cell, and the trim it needed to do that is declared.
    #[test]
    fn the_derived_cabin_charts_match_the_authored_bay() {
        let cabin = cabin_room();
        let derived = charts(CABIN, &cabin);
        let authored = crate::rig::bay_authored();
        for ((sa, a), (sb, b)) in derived.iter().zip(authored.iter()) {
            assert_eq!(sa, sb);
            assert!(
                (a.center - b.center).length() < 1e-3,
                "{sa:?} centre {} vs {}",
                a.center,
                b.center
            );
            assert!((a.half_u - b.half_u).length() < 1e-3, "{sa:?} u axis");
            assert!((a.half_v - b.half_v).length() < 1e-3, "{sa:?} v axis");
            assert_eq!(a.rect, b.rect, "{sa:?} rect");
        }
    }

    /// Every room's charts stay inside its own box (bar the cabin's
    /// declared gutter), and every chart's axes stay perpendicular — the
    /// fold is a fold, at every yaw the lattice can produce.
    #[test]
    fn every_room_folds_its_own_box() {
        let mut rooms = Rooms::new();
        // Reach every yaw: doors on all four walls of the cabin.
        for kind in [RoomKind::Trade, RoomKind::Wreck, RoomKind::Parlor] {
            assert!(rooms.spawn(kind, CABIN).is_ok());
        }
        for (id, room) in rooms.iter() {
            let (lo, hi) = room_box(&room.clone());
            for (station, surface) in charts(id, room) {
                let n = surface.normal();
                assert!(
                    n.length_squared() > 0.9,
                    "room {id} {station:?} has no normal"
                );
                assert!(
                    surface.half_u.dot(surface.half_v).abs() < 1e-3,
                    "room {id} {station:?} axes are not perpendicular"
                );
                let slack = 0.6;
                assert!(
                    surface.center.x >= lo.x - slack
                        && surface.center.x <= hi.x + slack
                        && surface.center.z >= lo.z - slack
                        && surface.center.z <= hi.z + slack,
                    "room {id} {station:?} centre {} escapes its box",
                    surface.center
                );
            }
        }
    }

    /// A punch takes a hole out of a slab and leaves the rest: the volume
    /// arithmetic that keeps every doorway open and every wall standing.
    #[test]
    fn a_punch_removes_exactly_the_hole() {
        let center = Vec3::new(0.0, 1.0, 0.0);
        let size = Vec3::new(4.0, 2.0, 0.2);
        let parts = punch(
            center,
            size,
            Vec3::new(-0.5, 0.0, -1.0),
            Vec3::new(0.5, 1.0, 1.0),
        );
        let volume: f32 = parts.iter().map(|(_, s)| s.x * s.y * s.z).sum();
        let expected = 4.0f32.mul_add(2.0, -1.0) * 0.2;
        assert!(
            (volume - expected).abs() < 1e-3,
            "punched volume {volume} should be {expected}"
        );
        // Nothing left standing inside the hole.
        for (c, s) in &parts {
            let lo = *c - *s * 0.5;
            let hi = *c + *s * 0.5;
            assert!(
                lo.x >= 0.5 - 1e-4 || hi.x <= -0.5 + 1e-4 || lo.y >= 1.0 - 1e-4,
                "a remainder at {c} sits in the doorway"
            );
        }
        // A miss leaves the slab whole.
        assert_eq!(
            punch(center, size, Vec3::splat(9.0), Vec3::splat(10.0)).len(),
            1
        );
    }

    /// The cabin's six apertures come out of its declared ports, and the
    /// starboard one lands where the burner's doorway has always been.
    #[test]
    fn the_cabin_punches_its_declared_ports() {
        let holes = cabin_holes();
        assert_eq!(holes.len(), PORTS);
        let (lo, hi) = holes[1];
        assert!((hi.x - lo.x) > 0.2, "the starboard doorway has no depth");
        assert!(
            2.0f32.mul_add(-BAY_CELL, hi.z - lo.z).abs() < 1e-3,
            "the starboard doorway spans {} not two cells",
            hi.z - lo.z
        );
        assert!(
            f32::from(APERTURE).mul_add(-BAY_CELL, hi.y - lo.y).abs() < 1e-3,
            "the starboard doorway stands {} tall",
            hi.y - lo.y
        );
        // It reaches through the authored hull it is cut into.
        assert!(
            hi.x >= 2.33,
            "the doorway stops inside the wall at {}",
            hi.x
        );
    }

    /// The envelope joins: from anywhere in the cabin a body can reach the
    /// burner through the mated door, and the two boxes overlap at the
    /// connector rather than leaving a step of nothing between them.
    #[test]
    fn the_walk_envelope_reaches_through_a_mated_door() {
        let rooms = Rooms::new();
        let placed: Vec<Placed> = rooms.iter().map(|(id, room)| placed(id, room)).collect();
        let envelope = walk_boxes(&placed);
        // Mid-cabin, mid-doorway, and mid-burner are all legal, and the
        // walk from one to the other never leaves the envelope.
        let burner = &placed[1];
        let inside = Vec3::new(
            f32::midpoint(burner.lo.x, burner.hi.x),
            EYE_HEIGHT,
            f32::midpoint(burner.lo.z, burner.hi.z),
        );
        assert!(envelope.holds(Vec3::new(0.0, EYE_HEIGHT, 0.5)), "the cabin");
        assert!(envelope.holds(inside), "the burner at {inside}");
        let from = Vec3::new(1.5, EYE_HEIGHT, 1.9);
        for step in 0..=40u8 {
            let t = f32::from(step) / 40.0;
            let p = from.lerp(inside, t);
            assert!(envelope.holds(p), "the walk breaks at {p}");
        }
        // And nothing outside the hull is walkable.
        assert!(!envelope.holds(Vec3::new(0.0, EYE_HEIGHT, 6.0)));
        assert!(!envelope.holds(Vec3::new(6.0, EYE_HEIGHT, 0.0)));
    }

    /// The same, for a room that came alongside at a yaw of its own: the
    /// station's trade room mates the cabin's AFT door and lands turned
    /// half around, and the walk into it is still continuous.
    #[test]
    fn the_walk_envelope_reaches_a_room_that_arrived_turned_around() {
        let mut rooms = Rooms::new();
        let trade = rooms
            .spawn(RoomKind::Trade, CABIN)
            .expect("the dock attaches its room");
        let placed: Vec<Placed> = rooms.iter().map(|(id, room)| placed(id, room)).collect();
        let envelope = walk_boxes(&placed);
        let shop = placed
            .iter()
            .find(|room| room.id == trade)
            .expect("the trade room is placed");
        assert_ne!(shop.yaw, 0, "this test wants a turned room");
        let inside = Vec3::new(
            f32::midpoint(shop.lo.x, shop.hi.x),
            EYE_HEIGHT,
            f32::midpoint(shop.lo.z, shop.hi.z),
        );
        assert!(envelope.holds(inside), "the trade room at {inside}");
        // Through the doorway, not across its corner: a mated aperture is
        // two cells wide and a body is not thin, so the way in is the way
        // a body actually walks it — up to the seam, then through.
        let from = Vec3::new(-1.65, EYE_HEIGHT, 1.9);
        let seam = Vec3::new(-1.65, EYE_HEIGHT, 2.41);
        let plan = plan_of(&placed);
        for (a, b) in [(from, seam), (seam, inside)] {
            for step in 0..=40u8 {
                let t = f32::from(step) / 40.0;
                let p = a.lerp(b, t);
                assert!(envelope.holds(p), "the walk breaks at {p}");
                // And the body is either standing in a room or ducking
                // through a doorway; there is no third place to be.
                assert!(
                    plan.room_at(p).is_some() || envelope.ducking(p),
                    "at {p} the body is in no room and no doorway"
                );
            }
        }
    }

    /// A plan wrapping a placed list, for the tests.
    fn plan_of(rooms: &[Placed]) -> Plan {
        Plan {
            rooms: rooms.to_vec(),
            signature: Vec::new(),
        }
    }

    /// Every mapped quad the running game stands up, from a sim alone:
    /// each room's six charts (tagged with whose they are), the
    /// handshake faces, the stations and pick faces that ride the cargo
    /// (`pieces::ride_pieces`), and the hull's own console panel. The
    /// carry is driven through exactly this list, because the carry is
    /// driven through exactly this list at runtime.
    fn world_surfaces(sim: &space_trucking::sim::Sim) -> Vec<crate::surface::Aimable> {
        use crate::surface::Aimable;
        let plan: Vec<Placed> = sim
            .rooms()
            .iter()
            .map(|(id, room)| placed(id, room))
            .collect();
        let mut aims: Vec<Aimable> = Vec::new();
        for room in &plan {
            let tag = InRoom {
                room: room.id,
                kind: room.kind,
            };
            for (station, surface) in room.charts {
                aims.push(Aimable {
                    station,
                    surface,
                    riding: false,
                    in_room: Some(tag),
                });
            }
        }
        let charts: Vec<(Station, SimSurface)> =
            aims.iter().map(|aim| (aim.station, aim.surface)).collect();
        let in_hand = sim.held(0).map(|held| held.piece);
        for piece in sim.pieces() {
            // A piece in hand rides the crosshair, not a berth, so it
            // carries no surface — `ride_pieces`' own rule.
            if !matches!(piece.loc, Loc::Hold { .. }) || in_hand == Some(piece.id) {
                continue;
            }
            let rect = layout::piece_rect(sim.pieces(), piece);
            if let Some((station, surface)) =
                crate::pieces::instrument_surface(&charts, piece.kind, rect)
            {
                aims.push(Aimable {
                    station,
                    surface,
                    riding: true,
                    in_room: None,
                });
            }
            if let Some(surface) = crate::pieces::standing_surface(&charts, piece.kind, rect) {
                aims.push(Aimable {
                    station: Station::Standing,
                    surface,
                    riding: true,
                    in_room: None,
                });
            }
        }
        for (station, surface) in crate::rig::panels() {
            aims.push(Aimable {
                station,
                surface,
                riding: false,
                in_room: None,
            });
        }
        aims
    }

    /// **The carry begins, end to end, without a window.** Walk up to
    /// every piece berthed in the cabin, put the crosshair on it, press,
    /// and the sim must report it in hand.
    ///
    /// This is the whole grab path and nothing else: the pointer is
    /// [`crate::surface::pick`]'s (the same call `track_pointer` makes),
    /// the click routing is [`crate::rig::handle_route`]'s (the same
    /// call `steer` makes), and the frame is the one `advance`
    /// synthesizes for a roam grab. A press that reaches the sim and
    /// lifts nothing is the playtest's dead left click, and it fails
    /// here first.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn a_press_on_a_berthed_piece_lifts_it() {
        use crate::bridge::{Bridge, FrameInput};
        use space_trucking::sim::Sim;

        let mut bridge = Bridge::boot_fixture(&Sim::new(4).save_string());
        let berthed: Vec<(u32, space_trucking::sim::Kind)> = bridge
            .sim
            .pieces()
            .iter()
            .filter(|piece| matches!(piece.loc, Loc::Hold { room, .. } if room == CABIN))
            .map(|piece| (piece.id, piece.kind))
            .collect();
        assert!(berthed.len() >= 4, "the starter cabin should be furnished");
        let mut lifted = 0;
        for (id, kind) in berthed {
            let aims = world_surfaces(&bridge.sim);
            let plan: Vec<Placed> = bridge
                .sim
                .rooms()
                .iter()
                .map(|(room, r)| placed(room, r))
                .collect();
            let envelope = walk_boxes(&plan);
            let charts: Vec<(Station, SimSurface)> =
                aims.iter().map(|aim| (aim.station, aim.surface)).collect();
            let piece = bridge
                .sim
                .pieces()
                .iter()
                .find(|piece| piece.id == id)
                .expect("the piece is still aboard");
            let rect = layout::piece_rect(bridge.sim.pieces(), piece);
            let mid = space_trucking::sim::Vec2::new(
                rect.w.mul_add(0.5, rect.x),
                rect.h.mul_add(0.5, rect.y),
            );
            let Some((station, surface)) = charts
                .iter()
                .find(|(station, surface)| station.chart_flipped() && surface.rect.contains(mid))
                .copied()
            else {
                continue;
            };
            // Where the piece is drawn, and a body's step back from it.
            let inward = station.inward(&surface);
            let at = surface.to_world(mid) + inward * 0.20;
            let flat = Vec3::new(inward.x, 0.0, inward.z).normalize_or(Vec3::Z);
            let mut found = None;
            for back in [0.75_f32, 0.9, 1.1, 0.55] {
                let eye = envelope.nearest(Vec3::new(at.x, EYE_HEIGHT, at.z) + flat * back);
                let Ok(dir) = Dir3::new(at - eye) else {
                    continue;
                };
                let pointer = crate::surface::pick(
                    Ray3d::new(eye, dir),
                    true,
                    crate::rig::REACH,
                    aims.iter().copied(),
                );
                if layout::piece_at(bridge.sim.pieces(), pointer.sim).map(|hit| hit.id) == Some(id)
                {
                    found = Some(pointer);
                    break;
                }
            }
            // The aft-most row of the deck is reached from inside its own
            // cell; the sweep does not insist on aiming at the player's
            // feet, only that whatever the crosshair DOES rest on lifts.
            let Some(pointer) = found else { continue };
            assert!(
                pointer.station.is_some(),
                "{kind:?} resolves no station, so the carry never asks the sim"
            );
            // The handle rule must not eat this press: passive cargo has
            // no function to guard.
            if crate::rig::handle_route(bridge.sim.pieces(), pointer.sim).is_some() {
                continue;
            }
            // `advance`'s roam grab, verbatim.
            bridge.frame(
                0.016,
                &FrameInput {
                    pointer: pointer.sim,
                    press: true,
                    held: true,
                    occupied: CABIN,
                    ..FrameInput::default()
                },
            );
            assert_eq!(
                bridge.sim.held(0).map(|held| held.piece),
                Some(id),
                "a press on {kind:?} at {:?} lifted nothing",
                pointer.sim
            );
            lifted += 1;
            // And set it down again, through the CHART this time: the
            // piece is in hand, so its own pick face has come down and
            // the ray meets the room's net where the berth is. That is
            // the other half of the carry, and it is the half that
            // proves the chart still answers for a cell a piece could
            // berth on (`surface::chart_cell`).
            let aims = world_surfaces(&bridge.sim);
            let mut placed_back = None;
            for back in [0.75_f32, 0.9, 1.1, 0.55] {
                let eye = envelope.nearest(Vec3::new(at.x, EYE_HEIGHT, at.z) + flat * back);
                let Ok(dir) = Dir3::new(surface.to_world(mid) - eye) else {
                    continue;
                };
                let aim = crate::surface::pick(
                    Ray3d::new(eye, dir),
                    true,
                    crate::rig::REACH,
                    aims.iter().copied(),
                );
                if layout::cell_at(aim.sim).is_some() {
                    placed_back = Some(aim);
                    break;
                }
            }
            let aim = placed_back.expect("the emptied berth still answers the crosshair");
            bridge.frame(
                0.016,
                &FrameInput {
                    pointer: aim.sim,
                    release: true,
                    occupied: CABIN,
                    ..FrameInput::default()
                },
            );
            assert!(
                bridge.sim.held(0).is_none(),
                "the hand never emptied over {:?}",
                aim.sim
            );
        }
        assert!(lifted >= 3, "only {lifted} pieces were actually carried");
    }

    /// A ship with a room on every wall of the cabin, for the geometry
    /// laws below: the widest spread of seams the lattice can hand us.
    fn crowded_ship() -> Vec<Placed> {
        let mut rooms = Rooms::new();
        for kind in [
            RoomKind::Trade,
            RoomKind::Wreck,
            RoomKind::Parlor,
            RoomKind::Pump,
        ] {
            let _ = rooms.spawn(kind, CABIN);
        }
        rooms.iter().map(|(id, room)| placed(id, room)).collect()
    }

    /// Whether two boxes share a volume worth seeing.
    fn intersects((alo, ahi): (Vec3, Vec3), (blo, bhi): (Vec3, Vec3), slack: f32) -> bool {
        (0..3).all(|axis| ahi[axis].min(bhi[axis]) - alo[axis].max(blo[axis]) > slack)
    }

    /// **No room reaches into another room.** The partition is a
    /// boundary, not a volume: everything a room draws stands inside its
    /// own box, and the only thing allowed across the line is the
    /// hardware that dresses a mated aperture — one frame, drawn once,
    /// standing in the opening the two rooms share.
    ///
    /// This is the playtest's sliver of another room's hatching, made
    /// geometrically impossible instead of merely absent.
    #[test]
    fn no_rooms_geometry_reaches_into_another_room() {
        let plan = crowded_ship();
        assert!(plan.len() >= 3, "this law wants a ship with neighbours");
        for room in &plan {
            // The aperture columns this room is allowed to stand in: its
            // own mated openings, a jamb's girth either side.
            let seams: Vec<(Vec3, Vec3)> = room
                .ports
                .iter()
                .filter(|site| site.mate.is_some() && site.is_door())
                .map(|site| {
                    let mid = seam_centre(room, site);
                    let half = site.half_a.abs()
                        + site.half_b.abs()
                        + Vec3::splat(JAMB + PLATE_T)
                        + site.out.abs() * JAMB;
                    (mid - half, mid + half)
                })
                .collect();
            let mut drawn: Vec<(Vec3, Vec3)> = shell_boxes(room, &plan)
                .into_iter()
                .map(|(c, s, _)| (c - s * 0.5, c + s * 0.5))
                .collect();
            for site in &room.ports {
                if dresses(room, site) {
                    drawn.extend(
                        seam_boxes(room, site)
                            .into_iter()
                            .map(|(c, s)| (c - s * 0.5, c + s * 0.5)),
                    );
                }
            }
            for other in plan.iter().filter(|other| other.id != room.id) {
                // The neighbour's interior, less a hair so a shared
                // boundary plane is contact rather than trespass.
                let inside = (other.lo + Vec3::splat(1e-3), other.hi - Vec3::splat(1e-3));
                for box_ in &drawn {
                    if !intersects(*box_, inside, 1e-3) {
                        continue;
                    }
                    assert!(
                        seams.iter().any(|seam| {
                            (0..3).all(|axis| {
                                box_.0[axis] >= seam.0[axis] - 1e-3
                                    && box_.1[axis] <= seam.1[axis] + 1e-3
                            })
                        }),
                        "room {} ({:?}) draws {:?}..{:?} inside room {} ({:?})",
                        room.id,
                        room.kind,
                        box_.0,
                        box_.1,
                        other.id,
                        other.kind
                    );
                }
            }
        }
    }

    /// Every chart a room paints on — and therefore every colored tile,
    /// every doormat, and every decal that rides one — stays inside that
    /// room's own box. The cabin is the one exception, and it is a
    /// DECLARED one: its front wall stands an authored gutter beyond its
    /// floor box (`chart_inset`), because its hull was built before the
    /// lattice was.
    #[test]
    fn every_attached_rooms_paint_stays_in_its_own_box() {
        for room in crowded_ship().iter().filter(|room| room.id != CABIN) {
            for (station, surface) in room.charts {
                // The deepest a decal ever rides, plus its own skin.
                let lift =
                    station.inward(&surface) * (crate::rig::layer::CLAIM + crate::rig::layer::SKIN);
                for corner in [-1.0_f32, 1.0] {
                    for course in [-1.0_f32, 1.0] {
                        let at = surface.center
                            + surface.half_u * corner
                            + surface.half_v * course
                            + lift;
                        assert!(
                            at.x >= room.lo.x - 1e-3
                                && at.x <= room.hi.x + 1e-3
                                && at.z >= room.lo.z - 1e-3
                                && at.z <= room.hi.z + 1e-3,
                            "room {} {station:?} paints at {at} outside {}..{}",
                            room.id,
                            room.lo,
                            room.hi
                        );
                    }
                }
            }
        }
    }

    /// The cabin's floor hatch reads as a hatch: nothing of its leaf
    /// stands proud of the deck, and what does stand proud is a rim thin
    /// enough to walk over. The playtest could not tell what the lump in
    /// the bow-starboard corner was, and a lump is what it was.
    #[test]
    fn the_cabins_floor_hatch_is_flush_with_its_deck() {
        let cabin = placed(CABIN, &cabin_room());
        let hatch = cabin
            .ports
            .iter()
            .find(|site| matches!(site.declared, Some(Port::Hatch { .. })))
            .expect("the cabin declares a hatch");
        assert!(hatch.mate.is_none(), "this test wants the hatch drawn shut");
        // The leaf: sunk, its whole body below the deck plane.
        let sink = PLATE_T.mul_add(0.5, SINK);
        let leaf_top = PLATE_T.mul_add(0.5, hatch.out.y.mul_add(sink, hatch.leaf.y));
        assert!(
            leaf_top <= SINK.mul_add(-0.5, FLOOR_Y),
            "the hatch leaf stands {leaf_top} proud of a deck at {FLOOR_Y}"
        );
        // The rim: proud, but no more than a rim.
        let rim_top = RIM_H.mul_add(0.5, hatch.out.y.mul_add(-(RIM_H * 0.5), hatch.leaf.y));
        assert!(
            rim_top - FLOOR_Y <= RIM_H,
            "the hatch coaming stands {} proud, which is a step not a rim",
            rim_top - FLOOR_Y
        );
        const {
            assert!(
                RIM_H < SINK,
                "a rim that outstands its own recess is a lump"
            );
        }
    }

    /// The occupied-room field derives from the body's position and
    /// nothing else, and it answers with the room the body is actually in.
    #[test]
    fn the_occupied_room_is_the_box_the_body_stands_in() {
        let rooms = Rooms::new();
        let plan = Plan {
            rooms: rooms.iter().map(|(id, room)| placed(id, room)).collect(),
            signature: Vec::new(),
        };
        assert_eq!(plan.room_at(Vec3::new(0.0, EYE_HEIGHT, 0.5)), Some(CABIN));
        assert_eq!(plan.room_at(Vec3::new(3.3, EYE_HEIGHT, 1.6)), Some(1));
        assert_eq!(plan.room_at(Vec3::new(0.0, EYE_HEIGHT, 9.0)), None);
    }
}
