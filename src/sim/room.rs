//! Rooms: the ship attaches what it meets (docs/ROOMS.md).
//!
//! The ship stopped being one box with annexes bolted to it. It is a
//! **graph of rooms** — nodes are rooms, edges are mated ports — laid on
//! one shared integer lattice in units of the room grid's cell. Every
//! room is an axis-aligned box at an integer origin with an integer yaw
//! of 0/90/180/270, one storey tall, and every room declares the same six
//! attachment points: a door on each of the four walls, one ladder in the
//! ceiling, one hatch in the floor.
//!
//! Nothing here touches a float. Attachment is a *geometric* operation
//! validated before it is a topological one: the mate fixes translation
//! and yaw together, so **attachment has zero degrees of freedom** and
//! every replica computes the same pose from the same four small integers.
//! Refusals are named, one variant per rule ([`Refusal`]), and the whole
//! request is validated before anything is written — there is no partial
//! attach and no rollback path, because nothing was written.
//!
//! Every room is also a **room net**: BAY.md's cross of six charts,
//! generalized from a singleton to a family. A room kind declares its
//! floor extent and its ports; the charts, the validity mask, the
//! aperture punch-outs, and the tile classes all derive. Each attached
//! room gets a fixed **net lane** of the sim's logical space indexed by
//! its dense [`RoomId`], so a room's rects are a pure function of its id
//! and no attach ever reflows another room's coordinates.

use super::Vec2;

/// Index of one attached room. Dense and reused: rooms carry no serial
/// identity, and the graph is the only truth about them.
pub type RoomId = u8;

/// Index of one of a room's six attachment points.
pub type PortId = u8;

/// The cabin: the room you start in, and the root of the graph.
pub const CABIN: RoomId = 0;

/// Most rooms the ship will carry at once — the cabin plus the burner
/// plus six crew modules plus two callers, in the spec's arithmetic.
pub const MAX_ROOMS: usize = 10;

/// Attachment points per room: four doors, a ladder, a hatch.
pub const PORTS: usize = 6;

/// The ladder's port index. The vertical pair is mandatory on every
/// room; it is the escape hatch in the literal and engineering senses.
pub const LADDER: PortId = 4;

/// The hatch's port index.
pub const HATCH: PortId = 5;

/// How wide an aperture is, in cells. The law is only that mating
/// apertures be identical; the number itself is tuning (docs/ROOMS.md's
/// open question), and the working shape is the burner doorway's.
pub const APERTURE: u8 = 2;

/// Wall courses. Every room's walls are the cabin's height, which is
/// what makes any door mate any door and a ladder's neighbour sit
/// exactly one storey up.
pub const COURSES: u8 = 3;

/// Which plane of a room a net cell lies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surf {
    Aft,
    Port,
    Floor,
    Starboard,
    Front,
    Ceiling,
}

/// What a cell's colour reads as, and therefore how it behaves.
///
/// The class is declared once by the room kind; the rules and the paint
/// read the same declaration, so a tile that *looks* like an offer area
/// and does not behave like one cannot exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    /// Ordinary deck, wall, or ceiling: an ordinary berth.
    Plain,
    /// A chalked square in the room's own enamel. Cargo berthed here is
    /// *proposed*, not surrendered — it stays the player's until a
    /// resolution says otherwise.
    Offer,
    /// The room's enamel, filled: the room's own goods. Not the
    /// player's, and not carried out until a resolution grants them.
    Stock,
    /// Hazard chevrons. Anything berthed here is scheduled for
    /// destruction on the room's own beat — the burner's hopper.
    Consume,
    /// Doormat striping: an aperture's footprint, belonging to two rooms
    /// at once. Never a berth (`Violation::Threshold`).
    Threshold,
}

/// Everything a room can be. An **appended table**, like `Kind`: new
/// kinds go on the end and old saves keep parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    /// The cabin, room 0: an 8×7 floor and the walls three courses tall.
    Cabin,
    /// The incinerator. Hopper staging is its floor, as `Consume` tiles;
    /// the stoker reads the lowest occupied one on its own beat.
    Burner,
    /// A point of interest's own trade room, attached at the dock: its
    /// `Stock` row is the goods, its `Offer` row is where a proposal is
    /// laid, and its handshake is where a deal is struck.
    Trade,
    /// A derelict's hold, attached when one drifts alongside. Its
    /// salvage is on its own floor and taking it is a carry.
    Wreck,
    /// The casino's parlor, no visible doors. Stake cargo on its offer
    /// area and work the handshake: double or a commemorative chip.
    Parlor,
    /// The gas station's pump bay. Its handshake tops the tanks up.
    Pump,
}

/// Every room kind, in save-token order.
pub const ROOM_KINDS: [RoomKind; 6] = [
    RoomKind::Cabin,
    RoomKind::Burner,
    RoomKind::Trade,
    RoomKind::Wreck,
    RoomKind::Parlor,
    RoomKind::Pump,
];

/// One declared attachment point.
///
/// A port's position is **data, never an assumption**: which wall a door
/// is on and which cells it punches are read from here by the embedder,
/// the validity mask, and every rig.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    /// A door on wall `wall` (0 aft, 1 starboard, 2 front, 3 port),
    /// `offset` cells along that wall from its low end.
    Door { wall: u8, offset: u8 },
    /// The ceiling's ladder, over the floor patch anchored at `(x, y)`.
    Ladder { x: u8, y: u8 },
    /// The floor's hatch, at the floor patch anchored at `(x, y)`.
    Hatch { x: u8, y: u8 },
}

impl RoomKind {
    /// Stable save token. Appended, so old documents keep parsing.
    #[must_use]
    pub const fn token(self) -> u8 {
        self as u8
    }

    /// Inverse of [`RoomKind::token`].
    #[must_use]
    pub const fn from_token(token: u8) -> Option<Self> {
        match token {
            0 => Some(Self::Cabin),
            1 => Some(Self::Burner),
            2 => Some(Self::Trade),
            3 => Some(Self::Wreck),
            4 => Some(Self::Parlor),
            5 => Some(Self::Pump),
            _ => None,
        }
    }

    /// The room's floor extent in cells, `(width, depth)`.
    #[must_use]
    pub const fn floor(self) -> (u8, u8) {
        match self {
            Self::Cabin => (8, 7),
            Self::Burner => (4, 3),
            Self::Trade => (6, 5),
            Self::Wreck => (5, 3),
            Self::Parlor => (4, 4),
            Self::Pump => (3, 3),
        }
    }

    /// The net's bounding grid, `(cols, rows)`: the cross of six charts
    /// laid flat. Three courses of wall on every side of a `w × h`
    /// floor, with the ceiling folded over the starboard cornice.
    #[must_use]
    pub const fn grid(self) -> (u8, u8) {
        let (w, h) = self.floor();
        (6 + 2 * w, 6 + h)
    }

    /// Whether this room **rides**: it travels with the ship. Calling
    /// rooms come alongside and leave, and departure detaches them.
    #[must_use]
    pub const fn riding(self) -> bool {
        matches!(self, Self::Cabin | Self::Burner)
    }

    /// This kind's six attachment points, in port order.
    #[must_use]
    pub const fn ports(self) -> [Port; PORTS] {
        let (doors, ladder, hatch) = match self {
            // The cabin's starboard door is the burner's traditional
            // one; the front door dodges the instrument cluster and the
            // aft and port doors sit at the aft-port corner.
            Self::Cabin => ([0, 0, 3, 0], (6, 4), (6, 4)),
            Self::Burner => ([0, 0, 0, 0], (0, 1), (2, 1)),
            Self::Trade => ([0, 0, 0, 0], (0, 1), (4, 1)),
            Self::Wreck => ([0, 0, 0, 0], (0, 1), (3, 1)),
            Self::Parlor => ([0, 0, 0, 0], (0, 2), (1, 0)),
            Self::Pump => ([0, 0, 0, 0], (1, 1), (0, 1)),
        };
        [
            Port::Door {
                wall: 0,
                offset: doors[0],
            },
            Port::Door {
                wall: 1,
                offset: doors[1],
            },
            Port::Door {
                wall: 2,
                offset: doors[2],
            },
            Port::Door {
                wall: 3,
                offset: doors[3],
            },
            Port::Ladder {
                x: ladder.0,
                y: ladder.1,
            },
            Port::Hatch {
                x: hatch.0,
                y: hatch.1,
            },
        ]
    }

    /// The room's one click-functional fixture, if it has one: the
    /// handshake where a deal is struck (a chit press at the Guild, a
    /// bell at the Hermitage — the form is per-POI, the behavior fixed).
    /// It is set into the room's fabric, so its cell is not a berth.
    #[must_use]
    pub const fn handshake(self) -> Option<(u8, u8)> {
        match self {
            Self::Cabin | Self::Burner => None,
            Self::Trade | Self::Wreck => Some((6, 1)),
            Self::Parlor | Self::Pump => Some((5, 1)),
        }
    }

    /// Which plane a net cell lies in, or `None` where the cross has no
    /// cell — outside the charts, or the fixture's own socket.
    #[must_use]
    #[allow(clippy::many_single_char_names)]
    pub const fn surface_of(self, x: u8, y: u8) -> Option<Surf> {
        if let Some((fx, fy)) = self.handshake() {
            if x == fx && y == fy {
                return None;
            }
        }
        let (w, h) = self.floor();
        let (c, r) = (COURSES, COURSES);
        if y < r {
            if x >= c && x < c + w {
                return Some(Surf::Aft);
            }
            return None;
        }
        if y >= r + h {
            if y < r + h + r && x >= c && x < c + w {
                return Some(Surf::Front);
            }
            return None;
        }
        if x < c {
            return Some(Surf::Port);
        }
        if x < c + w {
            return Some(Surf::Floor);
        }
        if x < c + w + c {
            return Some(Surf::Starboard);
        }
        if x < 2 * c + w + w {
            return Some(Surf::Ceiling);
        }
        None
    }

    /// The floor chart's bounds, `(x0, y0, w, h)`.
    #[must_use]
    pub const fn floor_rect(self) -> (u8, u8, u8, u8) {
        let (w, h) = self.floor();
        (COURSES, COURSES, w, h)
    }

    /// The net cells one port punches: two by two, always.
    #[must_use]
    pub fn aperture_cells(self, port: PortId) -> [(u8, u8); 4] {
        let (w, h) = self.floor();
        let c = COURSES;
        match self.ports()[usize::from(port)] {
            Port::Door { wall, offset } => {
                let mut cells = [(0, 0); 4];
                for course in 0..APERTURE {
                    for along in 0..APERTURE {
                        let cell = match wall {
                            // Aft: baseboard is the row nearest the floor.
                            0 => (c + offset + along, c - 1 - course),
                            // Starboard: baseboard is the column nearest
                            // the floor, courses running outward.
                            1 => (c + w + course, c + offset + along),
                            2 => (c + offset + along, c + h + course),
                            _ => (c - 1 - course, c + offset + along),
                        };
                        cells[usize::from(course * APERTURE + along)] = cell;
                    }
                }
                cells
            }
            Port::Ladder { x, y } => {
                let mut cells = [(0, 0); 4];
                for j in 0..APERTURE {
                    for i in 0..APERTURE {
                        // The ceiling chart folds over the starboard
                        // cornice, so its columns run backwards.
                        cells[usize::from(j * APERTURE + i)] =
                            (2 * c + 2 * w - 1 - (x + i), c + y + j);
                    }
                }
                cells
            }
            Port::Hatch { x, y } => {
                let mut cells = [(0, 0); 4];
                for j in 0..APERTURE {
                    for i in 0..APERTURE {
                        cells[usize::from(j * APERTURE + i)] = (c + x + i, c + y + j);
                    }
                }
                cells
            }
        }
    }

    /// What net cell `(x, y)` is, or `None` where there is no cell.
    ///
    /// This is the single declaration the rules and the paint both read.
    #[must_use]
    pub fn tile_of(self, x: u8, y: u8) -> Option<Tile> {
        let surf = self.surface_of(x, y)?;
        for port in 0..PORTS as PortId {
            if self.aperture_cells(port).contains(&(x, y)) {
                return Some(Tile::Threshold);
            }
        }
        let (_, h) = self.floor();
        // A colored region is a BAND across the room, not a strip of
        // deck: cargo mounts on the floor, on walls, and under ceilings,
        // so an offer area that were floor-only could not hold a
        // painting, and a shelf that were floor-only could not stock one.
        // The aft band is the room's own goods, the front band is where a
        // proposal is laid, and the deck between is ordinary.
        let aft = matches!(surf, Surf::Aft)
            || matches!((surf, y), (Surf::Floor | Surf::Ceiling, _) if y == COURSES);
        let front = matches!(surf, Surf::Front)
            || matches!(surf, Surf::Floor | Surf::Ceiling) && y + 2 >= COURSES + h;
        Some(match self {
            // The whole furnace room is hazard, deck to cornice: anything
            // carried in is fuel, and a wall instrument burns as readily
            // as a couch. Staging is an ordinary berth transition into an
            // ordinary room.
            Self::Burner => Tile::Consume,
            Self::Trade | Self::Wreck if aft => Tile::Stock,
            Self::Trade | Self::Parlor if front => Tile::Offer,
            _ => Tile::Plain,
        })
    }
}

/// A room's placement on the shared lattice. Integers only: no free
/// rotation, no fractional offset, no floating point anywhere in the
/// attach contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    /// Lattice origin of the room's floor box, in cells.
    pub x: i32,
    pub y: i32,
    /// Storey. Every room is one storey tall, so a ladder's neighbour
    /// sits exactly one of these up.
    pub z: i32,
    /// Quarter turns clockwise, `0..4`.
    pub yaw: u8,
}

/// One attached room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    pub kind: RoomKind,
    pub pose: Pose,
    /// Which room and port each of this room's ports is mated to.
    pub mates: [Option<(RoomId, PortId)>; PORTS],
    /// How this room came to be attached: `None` for the root.
    pub anchor: Option<(RoomId, PortId, PortId)>,
}

/// Why an attach or a detach was refused. One variant per rule, in the
/// same shape as `Violation`, so the presentation can flash the matching
/// tell. A refusal is a cue, not an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No such port, or no such room.
    Absent,
    /// The port is already in use.
    Mated,
    /// Ports that cannot mate: a door only ever mates a door, a ladder
    /// only ever a hatch.
    Kinds,
    /// The new room's box intersects placed geometry.
    Blocked,
    /// A seam that will not close identical-or-disjoint.
    Aperture,
    /// The room budget is spent.
    Full,
    /// The cabin is the root; it does not part from itself.
    Root,
    /// A crew body is in the detaching room, or beyond it.
    Aboard,
    /// Cargo of the player's rests in the detaching room, or beyond it.
    Cargo,
    /// The room's business is unfinished: a proposal still on its offer
    /// area.
    Pending,
}

/// One cell of the lattice.
type Cell3 = (i32, i32, i32);

/// One aperture edge: the pair of cells a port's opening joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seam {
    inside: Cell3,
    outside: Cell3,
}

/// Rotate a local floor cell into a room's own box frame.
const fn rot(i: u8, j: u8, w: u8, h: u8, yaw: u8) -> (i32, i32) {
    let (i, j, w, h) = (i as i32, j as i32, w as i32, h as i32);
    match yaw {
        0 => (i, j),
        1 => (h - 1 - j, i),
        2 => (w - 1 - i, h - 1 - j),
        _ => (j, w - 1 - i),
    }
}

/// Rotate a direction by `yaw` quarter turns clockwise.
const fn rot_dir(d: (i32, i32), yaw: u8) -> (i32, i32) {
    match yaw {
        0 => d,
        1 => (-d.1, d.0),
        2 => (-d.0, -d.1),
        _ => (d.1, -d.0),
    }
}

/// A local wall's outward normal.
const fn wall_normal(wall: u8) -> (i32, i32) {
    match wall {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        _ => (-1, 0),
    }
}

impl Room {
    /// The room's box on the lattice, `(x0, y0, x1, y1)` half-open.
    #[must_use]
    pub const fn box_rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = self.kind.floor();
        let (bw, bh) = if self.pose.yaw % 2 == 0 {
            (w as i32, h as i32)
        } else {
            (h as i32, w as i32)
        };
        (self.pose.x, self.pose.y, self.pose.x + bw, self.pose.y + bh)
    }

    /// Whether lattice cell `cell` lies in this room's interior.
    #[must_use]
    pub const fn contains(&self, cell: Cell3) -> bool {
        let (x0, y0, x1, y1) = self.box_rect();
        cell.2 == self.pose.z && cell.0 >= x0 && cell.0 < x1 && cell.1 >= y0 && cell.1 < y1
    }

    /// A local floor cell's lattice cell.
    #[must_use]
    pub const fn cell_of(&self, i: u8, j: u8) -> Cell3 {
        let (w, h) = self.kind.floor();
        let (rx, ry) = rot(i, j, w, h, self.pose.yaw);
        (self.pose.x + rx, self.pose.y + ry, self.pose.z)
    }

    /// The seams one port opens: the cell pairs its aperture joins.
    fn seams(&self, port: PortId) -> Vec<Seam> {
        let (w, h) = self.kind.floor();
        match self.kind.ports()[usize::from(port)] {
            Port::Door { wall, offset } => {
                let normal = rot_dir(wall_normal(wall), self.pose.yaw);
                (0..APERTURE)
                    .map(|along| {
                        let (i, j) = match wall {
                            0 => (offset + along, 0),
                            1 => (w - 1, offset + along),
                            2 => (offset + along, h - 1),
                            _ => (0, offset + along),
                        };
                        let inside = self.cell_of(i, j);
                        Seam {
                            inside,
                            outside: (inside.0 + normal.0, inside.1 + normal.1, inside.2),
                        }
                    })
                    .collect()
            }
            Port::Ladder { x, y } | Port::Hatch { x, y } => {
                let step = if matches!(self.kind.ports()[usize::from(port)], Port::Ladder { .. }) {
                    1
                } else {
                    -1
                };
                let mut seams = Vec::with_capacity(4);
                for j in 0..APERTURE {
                    for i in 0..APERTURE {
                        let inside = self.cell_of(x + i, y + j);
                        seams.push(Seam {
                            inside,
                            outside: (inside.0, inside.1, inside.2 + step),
                        });
                    }
                }
                seams
            }
        }
    }

    /// Whether two ports' openings are the same opening, seen from
    /// either side. Exact, because the lattice is integer: there is no
    /// epsilon and no "close enough to close".
    fn ports_coincide(&self, port: PortId, other: &Self, other_port: PortId) -> bool {
        let mine = self.seams(port);
        let theirs = other.seams(other_port);
        mine.len() == theirs.len()
            && mine.iter().all(|seam| {
                theirs
                    .iter()
                    .any(|t| t.inside == seam.outside && t.outside == seam.inside)
            })
    }
}

/// The room graph and the lattice it lives on.
#[derive(Clone, Debug)]
pub struct Rooms {
    slots: [Option<Room>; MAX_ROOMS],
    /// Attach order — what the save carries, and what a load replays.
    order: Vec<RoomId>,
}

impl Rooms {
    /// The ship as it leaves the yard: the cabin, and the burner bolted
    /// to its starboard door.
    #[must_use]
    pub fn new() -> Self {
        let mut rooms = Self::root(RoomKind::Cabin);
        // The furnace has always hung off the starboard wall; the
        // interface is now the ordinary one.
        let burner = rooms.attach(CABIN, 1, RoomKind::Burner, 3);
        debug_assert!(burner == Ok(1), "the burner must take room 1");
        rooms
    }

    /// A graph holding only its root room, at the lattice origin.
    #[must_use]
    pub fn root(kind: RoomKind) -> Self {
        let mut slots = [None; MAX_ROOMS];
        slots[usize::from(CABIN)] = Some(Room {
            kind,
            pose: Pose {
                x: 0,
                y: 0,
                z: 0,
                yaw: 0,
            },
            mates: [None; PORTS],
            anchor: None,
        });
        Self {
            slots,
            order: vec![CABIN],
        }
    }

    /// The room at `id`, if one is attached.
    #[must_use]
    pub const fn get(&self, id: RoomId) -> Option<&Room> {
        if (id as usize) < MAX_ROOMS {
            self.slots[id as usize].as_ref()
        } else {
            None
        }
    }

    /// What kind of room `id` is, if any.
    #[must_use]
    pub fn kind(&self, id: RoomId) -> Option<RoomKind> {
        self.get(id).map(|room| room.kind)
    }

    /// Every attached room, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (RoomId, &Room)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(id, room)| room.as_ref().map(|room| (id as RoomId, room)))
    }

    /// Attached rooms in attach order — the save's edge list.
    #[must_use]
    pub fn order(&self) -> &[RoomId] {
        &self.order
    }

    /// How many rooms are attached.
    #[must_use]
    pub fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// What net cell `(x, y)` of room `id` is, or `None` for no cell.
    #[must_use]
    pub fn tile(&self, id: RoomId, x: u8, y: u8) -> Option<Tile> {
        self.kind(id)?.tile_of(x, y)
    }

    /// Whether `id` is attached and travels with the ship.
    #[must_use]
    pub fn riding(&self, id: RoomId) -> bool {
        self.kind(id).is_some_and(RoomKind::riding)
    }

    /// The first attached room of `kind`, in id order.
    #[must_use]
    pub fn find(&self, kind: RoomKind) -> Option<RoomId> {
        self.iter()
            .find(|(_, room)| room.kind == kind)
            .map(|(id, _)| id)
    }

    /// Validate and commit an attach: four small integers in, a dense
    /// [`RoomId`] out. The whole request is checked — ports, kinds,
    /// pose, box, and every induced seam — before anything changes.
    pub fn attach(
        &mut self,
        anchor: RoomId,
        anchor_port: PortId,
        kind: RoomKind,
        port: PortId,
    ) -> Result<RoomId, Refusal> {
        let candidate = self.mate_pose(anchor, anchor_port, kind, port)?;
        let mates = self.seams_close(&candidate, None)?;
        let Some(id) = (0..MAX_ROOMS as RoomId).find(|id| self.get(*id).is_none()) else {
            return Err(Refusal::Full);
        };
        let mut room = candidate;
        room.anchor = Some((anchor, anchor_port, port));
        for &(mine, other, theirs) in &mates {
            room.mates[usize::from(mine)] = Some((other, theirs));
            if let Some(neighbour) = self.slots[usize::from(other)].as_mut() {
                neighbour.mates[usize::from(theirs)] = Some((id, mine));
            }
        }
        self.slots[usize::from(id)] = Some(room);
        self.order.push(id);
        Ok(id)
    }

    /// The pose a mate determines — the only pose there is.
    fn mate_pose(
        &self,
        anchor: RoomId,
        anchor_port: PortId,
        kind: RoomKind,
        port: PortId,
    ) -> Result<Room, Refusal> {
        let Some(host) = self.get(anchor) else {
            return Err(Refusal::Absent);
        };
        if usize::from(anchor_port) >= PORTS || usize::from(port) >= PORTS {
            return Err(Refusal::Absent);
        }
        if host.mates[usize::from(anchor_port)].is_some() {
            return Err(Refusal::Mated);
        }
        let host_port = host.kind.ports()[usize::from(anchor_port)];
        let new_port = kind.ports()[usize::from(port)];
        // Kinds mate: door↔door, ladder↔hatch, hatch↔ladder.
        let vertical = match (host_port, new_port) {
            (Port::Door { .. }, Port::Door { .. }) => None,
            (Port::Ladder { .. }, Port::Hatch { .. }) => Some(1),
            (Port::Hatch { .. }, Port::Ladder { .. }) => Some(-1),
            _ => return Err(Refusal::Kinds),
        };
        let yaw = match (host_port, new_port) {
            (Port::Door { wall: hw, .. }, Port::Door { wall: nw, .. }) => {
                let facing = rot_dir(wall_normal(hw), host.pose.yaw);
                let opposed = (-facing.0, -facing.1);
                // The turn that points the new door back at the old one.
                (0..4)
                    .find(|&r| rot_dir(wall_normal(nw), r) == opposed)
                    .ok_or(Refusal::Kinds)?
            }
            // A ladder does not turn you around: the vertical mate keeps
            // the anchor's yaw, which is what keeps its DOF at zero.
            _ => host.pose.yaw,
        };
        let z = host.pose.z + vertical.unwrap_or(0);
        // Place the room at the origin with that yaw, then slide it so
        // its aperture lands exactly on the anchor's.
        let mut room = Room {
            kind,
            pose: Pose { x: 0, y: 0, z, yaw },
            mates: [None; PORTS],
            anchor: None,
        };
        let want = anchor_target(host, anchor_port, vertical.is_some());
        let have = own_aperture(&room, port);
        room.pose.x += want.0 - have.0;
        room.pose.y += want.1 - have.1;
        // The box must be clear: sharing boundary planes is expected,
        // sharing one cell is refusal.
        let (x0, y0, x1, y1) = room.box_rect();
        for (_, other) in self.iter() {
            if other.pose.z != room.pose.z {
                continue;
            }
            let (ox0, oy0, ox1, oy1) = other.box_rect();
            if x0 < ox1 && ox0 < x1 && y0 < oy1 && oy0 < y1 {
                return Err(Refusal::Blocked);
            }
        }
        Ok(room)
    }

    /// Every seam the candidate induces, checked identical-or-disjoint.
    /// Returns the mates that form — the intended one plus whatever
    /// coincidences the placement discovered, which is the only way a
    /// cycle is ever created.
    fn seams_close(
        &self,
        candidate: &Room,
        skip: Option<RoomId>,
    ) -> Result<Vec<(PortId, RoomId, PortId)>, Refusal> {
        let mut mates = Vec::new();
        for port in 0..PORTS as PortId {
            let seams = candidate.seams(port);
            for (id, other) in self.iter() {
                if Some(id) == skip {
                    continue;
                }
                if !seams.iter().any(|seam| other.contains(seam.outside)) {
                    continue;
                }
                // Some of this port's opening leads into that room, so
                // all of it must, through an opening of that room's own.
                let Some(theirs) = (0..PORTS as PortId).find(|&p| {
                    other.mates[usize::from(p)].is_none()
                        && candidate.ports_coincide(port, other, p)
                }) else {
                    return Err(Refusal::Aperture);
                };
                mates.push((port, id, theirs));
            }
        }
        // And symmetrically: nothing already placed may open half into
        // the candidate's interior.
        for (id, other) in self.iter() {
            if Some(id) == skip {
                continue;
            }
            for port in 0..PORTS as PortId {
                if other.mates[usize::from(port)].is_some() {
                    continue;
                }
                if !other
                    .seams(port)
                    .iter()
                    .any(|seam| candidate.contains(seam.outside))
                {
                    continue;
                }
                if !mates
                    .iter()
                    .any(|&(_, mate_id, mate_port)| mate_id == id && mate_port == port)
                {
                    return Err(Refusal::Aperture);
                }
            }
        }
        Ok(mates)
    }

    /// The spawn contract: walk candidate port pairs in a fixed
    /// deterministic order and take the first that validates.
    ///
    /// The order is the anchor's own ports (doors by wall index, then
    /// the ladder, then the hatch), then outward through the graph in id
    /// order, each against the new room's ports in the same order. The
    /// walk terminates in success because the outermost vertical port's
    /// far side has never been reachable by anything (docs/ROOMS.md,
    /// "The escape-hatch guarantee").
    pub fn spawn(&mut self, kind: RoomKind, from: RoomId) -> Result<RoomId, Refusal> {
        if self.count() >= MAX_ROOMS {
            return Err(Refusal::Full);
        }
        let mut anchors: Vec<RoomId> = vec![from];
        anchors.extend(self.iter().map(|(id, _)| id).filter(|&id| id != from));
        let mut last = Refusal::Aperture;
        for anchor in anchors {
            for anchor_port in 0..PORTS as PortId {
                for port in 0..PORTS as PortId {
                    match self.attach(anchor, anchor_port, kind, port) {
                        Ok(id) => return Ok(id),
                        Err(why) => last = why,
                    }
                }
            }
        }
        Err(last)
    }

    /// Cut a room loose. The gates are the caller's (docs/ROOMS.md's
    /// gangway law); this is the graph surgery, and the id is freed for
    /// reuse the moment it is done.
    pub fn detach(&mut self, id: RoomId) -> Result<RoomKind, Refusal> {
        if id == CABIN {
            return Err(Refusal::Root);
        }
        let Some(room) = self.get(id).copied() else {
            return Err(Refusal::Absent);
        };
        for mate in room.mates.into_iter().flatten() {
            if let Some(other) = self.slots[usize::from(mate.0)].as_mut() {
                other.mates[usize::from(mate.1)] = None;
            }
        }
        self.slots[usize::from(id)] = None;
        self.order.retain(|&other| other != id);
        Ok(room.kind)
    }

    /// Every room reachable from the cabin without passing through
    /// `without` — the far side of the gangway law's two gates.
    #[must_use]
    pub fn reachable(&self, without: RoomId) -> [bool; MAX_ROOMS] {
        let mut seen = [false; MAX_ROOMS];
        if without == CABIN || self.get(CABIN).is_none() {
            return seen;
        }
        seen[usize::from(CABIN)] = true;
        let mut frontier = vec![CABIN];
        while let Some(id) = frontier.pop() {
            let Some(room) = self.get(id) else { continue };
            for mate in room.mates.into_iter().flatten() {
                if mate.0 == without || seen[usize::from(mate.0)] {
                    continue;
                }
                seen[usize::from(mate.0)] = true;
                frontier.push(mate.0);
            }
        }
        seen
    }

    /// Whether `room` is `id` itself or sits behind it — the rooms a
    /// detach would take with it.
    #[must_use]
    pub fn beyond(&self, id: RoomId, room: RoomId) -> bool {
        if room == id {
            return true;
        }
        self.get(room).is_some() && !self.reachable(id)[usize::from(room)]
    }

    /// Re-attach a room at a recorded mate, forcing its id — the save's
    /// replay path. The same validation runs; a document that lies about
    /// its own graph fails safe.
    pub fn replay(
        &mut self,
        id: RoomId,
        anchor: RoomId,
        anchor_port: PortId,
        kind: RoomKind,
        port: PortId,
    ) -> Result<(), Refusal> {
        if usize::from(id) >= MAX_ROOMS || self.get(id).is_some() {
            return Err(Refusal::Full);
        }
        let candidate = self.mate_pose(anchor, anchor_port, kind, port)?;
        let mates = self.seams_close(&candidate, None)?;
        let mut room = candidate;
        room.anchor = Some((anchor, anchor_port, port));
        for &(mine, other, theirs) in &mates {
            room.mates[usize::from(mine)] = Some((other, theirs));
            if let Some(neighbour) = self.slots[usize::from(other)].as_mut() {
                neighbour.mates[usize::from(theirs)] = Some((id, mine));
            }
        }
        self.slots[usize::from(id)] = Some(room);
        self.order.push(id);
        Ok(())
    }
}

impl Default for Rooms {
    fn default() -> Self {
        Self::new()
    }
}

/// Where an anchor's aperture wants its neighbour's: the lattice cell
/// the mating room's own aperture must land on, taken as the minimum of
/// the far side's cells so two collinear pairs align exactly.
fn anchor_target(host: &Room, port: PortId, vertical: bool) -> (i32, i32) {
    let seams = host.seams(port);
    let cells: Vec<(i32, i32)> = seams
        .iter()
        .map(|seam| (seam.outside.0, seam.outside.1))
        .collect();
    let _ = vertical;
    min_cell(&cells)
}

/// The same, for a room posed at the origin: where its own aperture sits
/// before the slide.
fn own_aperture(room: &Room, port: PortId) -> (i32, i32) {
    let cells: Vec<(i32, i32)> = room
        .seams(port)
        .iter()
        .map(|seam| (seam.inside.0, seam.inside.1))
        .collect();
    min_cell(&cells)
}

/// Lexicographic minimum of a cell list; `(0, 0)` for an empty one,
/// which cannot happen (every port punches four cells).
fn min_cell(cells: &[(i32, i32)]) -> (i32, i32) {
    cells.iter().copied().min().unwrap_or((0, 0))
}

// ---- Net lanes: the logical space each room's charts are laid into ----

/// The bounding grid of the widest room net, in cells. Every lane is
/// this size, so a room's rects are a pure function of its id.
pub const LANE_COLS: u8 = 22;

/// The bounding grid of the tallest room net, in rows.
pub const LANE_ROWS: u8 = 13;

/// Net cell size, in world units.
pub const CELL: f32 = 34.0;

/// Cells of gutter between lanes, so two rooms' charts never touch.
const LANE_GUTTER: f32 = 1.0;

/// Top-left corner of lane zero — the cabin's, east of the retired
/// console space.
pub const LANE_ORIGIN: Vec2 = Vec2::new(810.0, 16.0);

/// One lane's pitch in world units.
#[must_use]
pub fn lane_pitch() -> f32 {
    (f32::from(LANE_COLS) + LANE_GUTTER) * CELL
}

/// Top-left corner of room `id`'s lane. Lanes are fixed by id, so no
/// attach ever reflows another room's coordinates.
#[must_use]
pub fn lane_origin(id: RoomId) -> Vec2 {
    Vec2::new(
        f32::from(id).mul_add(lane_pitch(), LANE_ORIGIN.x),
        LANE_ORIGIN.y,
    )
}

/// How far east the lanes reach — the world must hold them.
#[must_use]
pub fn lanes_extent() -> f32 {
    f32::from(LANE_COLS).mul_add(CELL, lane_origin((MAX_ROOMS - 1) as RoomId).x)
}

/// Which lane and raw cell `p` falls in, if any. The caller decides
/// whether that room exists and whether the cell is a cell.
#[must_use]
pub fn lane_cell_at(p: Vec2) -> Option<(RoomId, u8, u8)> {
    let dx = p.x - LANE_ORIGIN.x;
    let dy = p.y - LANE_ORIGIN.y;
    if dx < 0.0 || dy < 0.0 {
        return None;
    }
    let pitch = lane_pitch();
    let lane = (dx / pitch) as i32;
    let id = u8::try_from(lane).ok()?;
    if usize::from(id) >= MAX_ROOMS {
        return None;
    }
    let local = f32::from(id).mul_add(-pitch, dx);
    let x = u8::try_from((local / CELL) as i32).ok()?;
    let y = u8::try_from((dy / CELL) as i32).ok()?;
    (x < LANE_COLS && y < LANE_ROWS).then_some((id, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cabin's net is the one BAY.md describes, chart for chart —
    /// the generalization did not move a single cabin cell.
    #[test]
    fn the_cabin_net_is_the_net_it_always_was() {
        let cabin = RoomKind::Cabin;
        assert_eq!(cabin.grid(), (22, 13));
        let mut counts = [0_usize; 6];
        for y in 0..13 {
            for x in 0..22 {
                if let Some(surf) = cabin.surface_of(x, y) {
                    counts[match surf {
                        Surf::Aft => 0,
                        Surf::Port => 1,
                        Surf::Floor => 2,
                        Surf::Starboard => 3,
                        Surf::Front => 4,
                        Surf::Ceiling => 5,
                    }] += 1;
                }
            }
        }
        assert_eq!(counts, [24, 21, 56, 21, 24, 56]);
        // The fold seams that are adjacent in the net are adjacent in
        // the room; a sample from each glued edge.
        for ((ax, ay), a, (bx, by), b) in [
            ((5, 2), Surf::Aft, (5, 3), Surf::Floor),
            ((2, 5), Surf::Port, (3, 5), Surf::Floor),
            ((10, 5), Surf::Floor, (11, 5), Surf::Starboard),
            ((5, 9), Surf::Floor, (5, 10), Surf::Front),
            ((13, 5), Surf::Starboard, (14, 5), Surf::Ceiling),
        ] {
            assert_eq!(cabin.surface_of(ax, ay), Some(a));
            assert_eq!(cabin.surface_of(bx, by), Some(b));
            assert_eq!(ax.abs_diff(bx) + ay.abs_diff(by), 1);
        }
        // The burner doorway keeps its traditional cells, as a
        // threshold rather than a hole.
        for cell in [(11, 3), (12, 3), (11, 4), (12, 4)] {
            assert_eq!(cabin.tile_of(cell.0, cell.1), Some(Tile::Threshold));
        }
    }

    /// Every kind declares six ports, all of them inside its own net,
    /// and no two ports share a cell.
    #[test]
    fn every_room_declares_six_ports_inside_its_own_net() {
        for kind in ROOM_KINDS {
            let (cols, rows) = kind.grid();
            let mut seen: Vec<(u8, u8)> = Vec::new();
            for port in 0..PORTS as PortId {
                for (x, y) in kind.aperture_cells(port) {
                    assert!(x < cols && y < rows, "{kind:?} port {port} leaves the net");
                    assert!(
                        kind.surface_of(x, y).is_some(),
                        "{kind:?} port {port} punches a hole that is not a cell"
                    );
                    assert!(!seen.contains(&(x, y)), "{kind:?} ports share a cell");
                    seen.push((x, y));
                }
            }
            assert_eq!(seen.len(), PORTS * 4);
        }
    }

    /// Attachment has zero degrees of freedom, and the burner lands
    /// where the burner has always landed.
    #[test]
    fn the_burner_mates_the_cabins_starboard_door() {
        let rooms = Rooms::new();
        assert_eq!(rooms.count(), 2);
        let cabin = rooms.get(CABIN).expect("the cabin is room zero");
        let burner = rooms.get(1).expect("the burner is room one");
        assert_eq!(cabin.box_rect(), (0, 0, 8, 7));
        assert_eq!(burner.box_rect(), (8, 0, 12, 3));
        assert_eq!(cabin.mates[1], Some((1, 3)));
        assert_eq!(burner.mates[3], Some((CABIN, 1)));
    }

    /// Overlap is prevented by law, never discovered as clipping: no
    /// two placed rooms ever share a cell, however they were spawned.
    #[test]
    fn no_two_rooms_ever_share_a_cell() {
        let mut rng = fastrand::Rng::with_seed(0x120E);
        for _ in 0..200 {
            let mut rooms = Rooms::new();
            for _ in 0..MAX_ROOMS {
                let kind = ROOM_KINDS[rng.usize(..ROOM_KINDS.len())];
                let from = rng.u8(..MAX_ROOMS as u8);
                let from = if rooms.get(from).is_some() {
                    from
                } else {
                    CABIN
                };
                let _ = rooms.spawn(kind, from);
            }
            let placed: Vec<&Room> = rooms.iter().map(|(_, room)| room).collect();
            for (i, a) in placed.iter().enumerate() {
                for b in &placed[i + 1..] {
                    if a.pose.z != b.pose.z {
                        continue;
                    }
                    let (ax0, ay0, ax1, ay1) = a.box_rect();
                    let (bx0, by0, bx1, by1) = b.box_rect();
                    assert!(
                        !(ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1),
                        "{a:?} and {b:?} interpenetrate"
                    );
                }
            }
        }
    }

    /// The escape-hatch obligation, as an adversarial-order property:
    /// spawn rooms in hostile orders and never see a refusal for want
    /// of space. If this fails the spawn contract is wrong, not the
    /// test (docs/ROOMS.md).
    #[test]
    fn the_spawn_walk_never_starves() {
        let mut rng = fastrand::Rng::with_seed(0x00E5_CA9E);
        for _ in 0..300 {
            let mut rooms = Rooms::new();
            // Detach and re-attach in adversarial orders, so the
            // lattice is left as ragged as the game can leave it.
            while rooms.count() < MAX_ROOMS {
                let kind = ROOM_KINDS[rng.usize(..ROOM_KINDS.len())];
                let anchors: Vec<RoomId> = rooms.iter().map(|(id, _)| id).collect();
                let from = anchors[rng.usize(..anchors.len())];
                let spawned = rooms.spawn(kind, from);
                assert!(
                    spawned.is_ok(),
                    "the vertical frontier starved: {spawned:?}"
                );
                if rng.u8(..4) == 0 {
                    let victims: Vec<RoomId> = rooms
                        .iter()
                        .map(|(id, _)| id)
                        .filter(|&id| id != CABIN)
                        .collect();
                    if !victims.is_empty() {
                        let _ = rooms.detach(victims[rng.usize(..victims.len())]);
                    }
                }
            }
            // Full is the only refusal a full ship may give.
            assert_eq!(rooms.spawn(RoomKind::Trade, CABIN), Err(Refusal::Full));
        }
    }

    /// A half-overlapping aperture is a geometric contradiction, and
    /// the attach that would make it is refused by name.
    #[test]
    fn a_seam_that_will_not_close_is_refused_by_name() {
        let mut rooms = Rooms::root(RoomKind::Cabin);
        // A trade room above through the ladder, then a second one that
        // would have to share the same column: the box refuses first.
        assert!(rooms.attach(CABIN, LADDER, RoomKind::Trade, HATCH).is_ok());
        assert_eq!(
            rooms.attach(CABIN, LADDER, RoomKind::Trade, HATCH),
            Err(Refusal::Mated)
        );
        // Ports that cannot mate are named too.
        assert_eq!(
            rooms.attach(CABIN, HATCH, RoomKind::Trade, 0),
            Err(Refusal::Kinds)
        );
        assert_eq!(rooms.attach(9, 0, RoomKind::Trade, 0), Err(Refusal::Absent));
    }

    /// Lanes are fixed by id: a room's logical rects never move because
    /// another room attached.
    #[test]
    fn lanes_are_a_pure_function_of_the_id() {
        for id in 0..MAX_ROOMS as RoomId {
            let origin = lane_origin(id);
            assert_eq!(lane_cell_at(origin), Some((id, 0, 0)));
            let far = Vec2::new(
                origin.x + f32::from(LANE_COLS - 1) * CELL + 1.0,
                origin.y + f32::from(LANE_ROWS - 1) * CELL + 1.0,
            );
            assert_eq!(lane_cell_at(far), Some((id, LANE_COLS - 1, LANE_ROWS - 1)));
        }
        assert_eq!(lane_cell_at(Vec2::new(0.0, 0.0)), None);
    }
}
