//! Rooms: the ship attaches what it meets (docs/ROOMS.md).
//!
//! The ship stopped being one box with annexes bolted to it. It is a
//! **graph of rooms** — nodes are rooms, edges are mated ports — laid on
//! one shared integer lattice in units of the room grid's cell. Every
//! room is an axis-aligned box at an integer origin with an integer yaw
//! of 0/90/180/270, one storey tall, and every room **declares the ports
//! it needs** out of six slots: a door on each of the four walls, a
//! ladder in the ceiling, a hatch in the floor. The cabin fills all six
//! because the cabin is the extensible piece; a furnace or a market
//! states its one seam and is a dead end, which is what a dead end
//! should look like.
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

/// Index of one of a room's six port slots.
///
/// The slot **is** the position: wall `w`'s door is port `w`, the ladder
/// is [`LADDER`], the hatch is [`HATCH`]. A kind fills the slots it needs
/// and leaves the rest empty; naming an empty one is [`Refusal::Absent`].
pub type PortId = u8;

/// The cabin: the room you start in, and the root of the graph.
pub const CABIN: RoomId = 0;

/// Most rooms the ship will carry at once — the cabin plus the burner
/// plus six crew modules plus two callers, in the spec's arithmetic.
pub const MAX_ROOMS: usize = 10;

/// Port **slots** per room — the bound, not a quota.
///
/// One door per wall, one ladder, one hatch. Because the slot index is
/// the position, "at most one door per wall" stopped being a rule anyone
/// can break and became arithmetic, and a kind's declaration is just
/// which slots it fills ([`RoomKind::ports`]).
pub const PORTS: usize = 6;

/// The ladder's port slot.
///
/// The vertical pair is the **cabin's** — it is the escape hatch in the
/// literal and the engineering senses, and the cabin is the room that
/// carries it (docs/ROOMS.md, "The escape-hatch guarantee").
pub const LADDER: PortId = 4;

/// The hatch's port slot.
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
    /// Bare fabric a room's **own** hardware already fills: the deck its
    /// handshake is worked over, and the patch of ceiling its pendant
    /// hangs from. Never a berth (`Violation::Fixture`) — see
    /// [`RoomKind::fixture`].
    Fixture,
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

    /// The ports this kind declares, by slot — `None` where the room has
    /// no such opening.
    ///
    /// **A room declares only the ports it needs.** The owner's decree,
    /// verbatim in docs/ROOMS.md: the incinerator and the Guild's room do
    /// not need four doors and a vertical pair, they can be dead ends,
    /// and six openings punched through a four-metre box read as busy
    /// rather than built. Only the **cabin** has to be extensible —
    /// cabins are the linkin' logs that meld into whacky amalgamations —
    /// so the cabin keeps the full complement and every other kind states
    /// its business and stops.
    ///
    /// Each declaration below is argued where it stands, because a port
    /// is architecture: adding one is a promise that something may arrive
    /// through it, and removing one is a promise that nothing will.
    #[must_use]
    pub const fn ports(self) -> [Option<Port>; PORTS] {
        const fn door(wall: u8, offset: u8) -> Port {
            Port::Door { wall, offset }
        }
        match self {
            // Six, and the only kind with six. The cabin is the piece the
            // crew melds — four ways out so any cabin can mate any cabin
            // on any side, and the vertical pair because the ladder
            // column and the hatch column are the frontier the spawn
            // walk falls back on and nothing else declares them. The
            // starboard door is the burner's traditional one; the front
            // door dodges the instrument cluster; the aft and port doors
            // sit at the aft-port corner.
            Self::Cabin => [
                Some(door(0, 0)),
                Some(door(1, 0)),
                Some(door(2, 3)),
                Some(door(3, 0)),
                Some(Port::Ladder { x: 6, y: 4 }),
                Some(Port::Hatch { x: 6, y: 4 }),
            ],
            // The incinerator, named in the decree. One door, on the wall
            // it has hung from since the first slice — the cabin's
            // starboard side — and nothing else. No hatch: a furnace
            // mouth in the deck is a second way the fire can travel, and
            // a hopper you can fall into is not a hopper. A furnace is a
            // dead end, and now it looks like one.
            Self::Burner => [None, None, None, Some(door(3, 0)), None, None],
            // The leaf rooms are one shape, and it is the decree's shape:
            // **one door, aft** — the wall a room presents to whatever it
            // came alongside — and nothing else at all.
            //
            // - `Trade` is the Guild's room, named in the decree with the
            //   furnace. A second door was considered, so two cabins
            //   could share one market; it buys nothing, because the
            //   graph already lets both crews walk to it through their
            //   own ship, and it costs the exact clutter the decree
            //   objects to. A market is a place you visit, not a
            //   corridor.
            // - `Wreck`: a derelict has exactly one seam worth trusting,
            //   the one you just mated. The rest of the hull is vacuum
            //   with edges.
            // - `Parlor`: the casino's whole conceit is that it has no
            //   visible doors. It gets the one it cannot do without.
            // - `Pump`: a forecourt. Come alongside, top up, cast off —
            //   and the fuel reaches the furnace in a crewman's arms,
            //   the way everything else in this game travels.
            Self::Trade | Self::Wreck | Self::Parlor | Self::Pump => {
                [Some(door(0, 0)), None, None, None, None, None]
            }
        }
    }

    /// The port in slot `port`, if this kind declares one. Every rule
    /// reads the declaration through here, so an absent port is absent
    /// everywhere at once.
    #[must_use]
    pub const fn port(self, port: PortId) -> Option<Port> {
        if (port as usize) < PORTS {
            self.ports()[port as usize]
        } else {
            None
        }
    }

    /// Every port this kind declares, in slot order.
    pub fn declared(self) -> impl Iterator<Item = (PortId, Port)> {
        self.ports()
            .into_iter()
            .enumerate()
            .filter_map(|(slot, port)| port.map(|port| (slot as PortId, port)))
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

    /// The net cells one port punches: two by two, always. `None` where
    /// the kind declares no such port — an undeclared slot punches
    /// nothing, so the wall stays a wall.
    #[must_use]
    pub fn aperture_cells(self, port: PortId) -> Option<[(u8, u8); 4]> {
        let (w, h) = self.floor();
        let c = COURSES;
        Some(match self.port(port)? {
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
        })
    }

    /// Whether net cell `(x, y)` is **deck a declared door stands on** —
    /// the two floor cells a body's first step lands in, in this room's
    /// own net.
    ///
    /// Not a tile class: those cells are ordinary berths and the player
    /// may stand cargo on them exactly as `cabin::room::treads` says
    /// (the studded plate is a reading of traffic, not a refusal). It is
    /// a rule about who may CHOOSE them, and the answer is nobody but
    /// the player — see [`super::Sim::free_berth_in`].
    #[must_use]
    pub fn doorstep(self, x: u8, y: u8) -> bool {
        let (width, depth) = self.floor();
        self.declared().any(|(_, port)| {
            let Port::Door { wall, offset } = port else {
                return false;
            };
            (0..APERTURE).any(|along| {
                let step = match wall % 4 {
                    0 => (offset + along, 0),
                    1 => (width - 1, offset + along),
                    2 => (offset + along, depth - 1),
                    _ => (0, offset + along),
                };
                (COURSES + step.0, COURSES + step.1) == (x, y)
            })
        })
    }

    /// **The entry path of a declared door**: the straight run in from
    /// that doorway — the deck lane its aperture opens onto, carried
    /// clean across the room, and the wall and ceiling cells standing
    /// over that lane.
    ///
    /// A door on the aft or front wall opens a lane of COLUMNS and a
    /// body crossing it walks fore-and-aft; a door on a side wall opens
    /// a lane of ROWS and the body walks athwart. Either way the lane is
    /// read off the port declaration and nothing else, so it moves when
    /// a door moves and exists only where a door does.
    #[must_use]
    pub fn entry_path(self, x: u8, y: u8) -> bool {
        let Some(surf) = self.surface_of(x, y) else {
            return false;
        };
        let (width, _) = self.floor();
        self.declared().any(|(_, port)| {
            let Port::Door { wall, offset } = port else {
                return false;
            };
            let lane = |n: u8| n >= offset && n < offset + APERTURE;
            let across = wall % 2 == 0;
            match surf {
                Surf::Floor => {
                    if across {
                        lane(x - COURSES)
                    } else {
                        lane(y - COURSES)
                    }
                }
                // The ceiling chart folds over the starboard cornice, so
                // its columns run backwards.
                Surf::Ceiling => {
                    if across {
                        lane(2 * COURSES + 2 * width - 1 - x)
                    } else {
                        lane(y - COURSES)
                    }
                }
                Surf::Aft | Surf::Front => across && lane(x - COURSES),
                Surf::Port | Surf::Starboard => !across && lane(y - COURSES),
            }
        })
    }

    /// **The cells a room's own hardware already fills.**
    ///
    /// A room is not only a net: it is a room, and it carries two pieces
    /// of hardware nobody chose to put there. The handshake is set into
    /// the aft wall and juts over the deck in front of it; the pendant
    /// hangs from the middle of the ceiling. Both are the ROOM's — the
    /// kind declares them, a station only dresses them — and both stand
    /// in air a rig would otherwise stand in.
    ///
    /// The socket the handshake is set into was already a hole
    /// ([`RoomKind::surface_of`]); this is the rest of the same argument,
    /// and it is stated in cells so the sim can rule on it without ever
    /// learning what the hardware looks like.
    ///
    /// Riding rooms declare neither: a cabin has no handshake and lights
    /// itself, so nothing here narrows the ship's own frontier.
    #[must_use]
    pub fn fixture(self, x: u8, y: u8) -> bool {
        // Whichever of a run of `n` cells the middle of the room falls
        // in: one where the count is odd, two where it is even and the
        // middle lands on the seam between them.
        const fn middle(n: u8, i: u8) -> bool {
            i + 1 >= n.div_ceil(2) && i <= n / 2
        }
        let Some(surf) = self.surface_of(x, y) else {
            return false;
        };
        let (w, h) = self.floor();
        match surf {
            // The counter's own deck: a wardrobe berthed here would
            // swallow the fixture a deal is struck at.
            Surf::Floor => self
                .handshake()
                .is_some_and(|(hx, _)| (x, y) == (hx, COURSES)),
            // A counter set in a corner column turns onto the wall
            // beside it, at its own course — which is the whole of a
            // three-wide bay's problem: the pump's cell IS the corner,
            // so its brass is the side wall's business too.
            Surf::Port | Surf::Starboard => self.handshake().is_some_and(|(hx, hy)| {
                // Aft courses count outward from the deck, so the
                // handshake's row names the course its brass is on.
                let course = COURSES - 1 - hy;
                y == COURSES
                    && ((hx == COURSES && x + 1 + course == COURSES)
                        || (hx + 1 == COURSES + w && x == COURSES + w + course))
            }),
            // The pendant's patch. The ceiling chart folds over the
            // starboard cornice, so its columns run backwards.
            Surf::Ceiling => {
                !self.riding() && middle(w, 2 * COURSES + 2 * w - 1 - x) && middle(h, y - COURSES)
            }
            _ => false,
        }
    }

    /// What net cell `(x, y)` is, or `None` where there is no cell.
    ///
    /// This is the single declaration the rules and the paint both read.
    #[must_use]
    pub fn tile_of(self, x: u8, y: u8) -> Option<Tile> {
        let surf = self.surface_of(x, y)?;
        for (port, _) in self.declared() {
            if self
                .aperture_cells(port)
                .is_some_and(|cells| cells.contains(&(x, y)))
            {
                return Some(Tile::Threshold);
            }
        }
        // The room's own hardware before its own bands: a fixture stands
        // where it stands, and no colour a kind paints moves it.
        if self.fixture(x, y) {
            return Some(Tile::Fixture);
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
        let tile = match self {
            // The whole furnace room is hazard, deck to cornice: anything
            // carried in is fuel, and a wall instrument burns as readily
            // as a couch. Staging is an ordinary berth transition into an
            // ordinary room.
            Self::Burner => Tile::Consume,
            Self::Trade | Self::Wreck if aft => Tile::Stock,
            Self::Trade | Self::Parlor if front => Tile::Offer,
            _ => Tile::Plain,
        };
        // **The entry-path law: an offer area may not sit in the way in.**
        //
        // The playtest walked through a station's chalk on the way to
        // every wall it had, at every station, because a band across the
        // room is a band across the door's own lane as well. Proposing is
        // a thing you do deliberately, at a place you go to; ground you
        // cross on your way in is not that place, and chalk you tread on
        // without meaning to says the opposite of what the class means.
        //
        // Stated relative to the DOORS rather than to a wall, so it is
        // one law and not twelve tunings: whatever a kind's bands would
        // have said, a cell in a declared door's [`entry_path`] is
        // ordinary deck. It therefore holds for every kind the game has
        // and every kind it grows, it moves when a door moves, and a
        // station cannot opt out of it, because a station never had a
        // say in it — the room kind declares bands and ports, and this
        // reads both.
        //
        // `Stock` is deliberately untouched. A shopfront either side of
        // the door you came in by is a shopfront; goods are the room's
        // to place where it likes, and the tell that they are not yours
        // is that pressing one marks it instead of lifting it.
        Some(if tile == Tile::Offer && self.entry_path(x, y) {
            Tile::Plain
        } else {
            tile
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

    /// The seams one port opens: the cell pairs its aperture joins. An
    /// undeclared slot opens none, which is how a dead end reads to every
    /// rule at once — nothing to mate, nothing to close, nothing to draw.
    fn seams(&self, port: PortId) -> Vec<Seam> {
        let (w, h) = self.kind.floor();
        let Some(declared) = self.kind.port(port) else {
            return Vec::new();
        };
        match declared {
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
                let step = if matches!(declared, Port::Ladder { .. }) {
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
    /// epsilon and no "close enough to close". Two walls that declare
    /// nothing are not an opening they share; they are two walls.
    fn ports_coincide(&self, port: PortId, other: &Self, other_port: PortId) -> bool {
        let mine = self.seams(port);
        let theirs = other.seams(other_port);
        !mine.is_empty()
            && mine.len() == theirs.len()
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
        // A slot the kind does not declare is not a port: the wall is a
        // wall, and naming it is `Absent` — the same refusal a door
        // carried off its wall will one day give (the stretch goal).
        let (Some(host_port), Some(new_port)) = (host.kind.port(anchor_port), kind.port(port))
        else {
            return Err(Refusal::Absent);
        };
        if host.mates[usize::from(anchor_port)].is_some() {
            return Err(Refusal::Mated);
        }
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

    /// Whether one attach request would validate, without writing
    /// anything. The walk asks this; `attach` asks it again on the way
    /// in, because validation is cheap and a second opinion is free.
    fn would_mate(
        &self,
        anchor: RoomId,
        anchor_port: PortId,
        kind: RoomKind,
        port: PortId,
    ) -> Result<(), Refusal> {
        let candidate = self.mate_pose(anchor, anchor_port, kind, port)?;
        self.seams_close(&candidate, None).map(|_| ())
    }

    /// The spawn contract's walk: candidate port pairs in a fixed
    /// deterministic order, the first that validates.
    ///
    /// The order is the anchor's own ports (doors by wall index, then
    /// the ladder, then the hatch), then outward through the graph in id
    /// order, each against the new room's ports in the same order. Slots
    /// a kind does not declare are tried and refused `Absent`, which
    /// costs one lookup and keeps the order stated in one place.
    ///
    /// For a **cabin** the walk cannot come back empty: every occupied
    /// storey holds a cabin (a storey is only ever reached by a hatch
    /// mating a ladder, and only cabins declare those), so the topmost
    /// cabin's ladder is free and the storey above it has never been
    /// reachable by anything. For everything else see docs/ROOMS.md,
    /// "The escape-hatch guarantee" — a caller mates a door, and doors
    /// are finite.
    fn berth(&self, kind: RoomKind, from: RoomId) -> Result<(RoomId, PortId, PortId), Refusal> {
        let mut anchors: Vec<RoomId> = vec![from];
        anchors.extend(self.iter().map(|(id, _)| id).filter(|&id| id != from));
        let mut deepest = Refusal::Absent;
        for anchor in anchors {
            for anchor_port in 0..PORTS as PortId {
                for port in 0..PORTS as PortId {
                    match self.would_mate(anchor, anchor_port, kind, port) {
                        Ok(()) => return Ok((anchor, anchor_port, port)),
                        // Report the deepest reason the walk reached, so
                        // a ship with no room left says so instead of
                        // blaming the last empty slot it happened to try.
                        Err(why) if reached(why) > reached(deepest) => deepest = why,
                        Err(_) => {}
                    }
                }
            }
        }
        Err(deepest)
    }

    /// Attach a room the game itself asked for, wherever it fits: the
    /// spawn contract's walk, committed.
    pub fn spawn(&mut self, kind: RoomKind, from: RoomId) -> Result<RoomId, Refusal> {
        if self.count() >= MAX_ROOMS {
            return Err(Refusal::Full);
        }
        let (anchor, anchor_port, port) = self.berth(kind, from)?;
        self.attach(anchor, anchor_port, kind, port)
    }

    /// Re-seat a room whose recorded mate no longer exists, keeping its
    /// id: the migration path for a document written when its kind
    /// declared ports it no longer does (docs/ROOMS.md's port law, and
    /// `save`'s pre-STV12 chain). Conservation before convenience — the
    /// room's cargo is berthed by id, so the id is what must survive.
    pub fn reseat(&mut self, id: RoomId, kind: RoomKind, from: RoomId) -> Result<(), Refusal> {
        if usize::from(id) >= MAX_ROOMS || self.get(id).is_some() {
            return Err(Refusal::Full);
        }
        let from = if self.get(from).is_some() {
            from
        } else {
            CABIN
        };
        let (anchor, anchor_port, port) = self.berth(kind, from)?;
        self.replay(id, anchor, anchor_port, kind, port)
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

/// How far into validation a refusal got. The spawn walk keeps the
/// deepest one it saw: "no space" is a truer answer than "no port" when
/// the walk tried a hundred slots that were never there.
const fn reached(refusal: Refusal) -> u8 {
    match refusal {
        Refusal::Absent => 0,
        Refusal::Kinds => 1,
        Refusal::Mated => 2,
        Refusal::Blocked => 3,
        Refusal::Aperture => 4,
        _ => 5,
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

    /// The invariants that survived the six: whatever a kind declares
    /// lands wholly inside its own net, no two ports share a cell, and a
    /// door sits on the wall its slot is named for — which is how "at
    /// most one door per wall" became arithmetic instead of a rule.
    #[test]
    fn declared_ports_land_in_their_own_net_and_never_overlap() {
        for kind in ROOM_KINDS {
            let (cols, rows) = kind.grid();
            let mut seen: Vec<(u8, u8)> = Vec::new();
            for (port, declared) in kind.declared() {
                if let Port::Door { wall, .. } = declared {
                    assert_eq!(wall, port, "{kind:?}'s door {port} hangs on wall {wall}");
                }
                assert!(
                    matches!(declared, Port::Ladder { .. }) == (port == LADDER),
                    "{kind:?} put a ladder in slot {port}"
                );
                let cells = kind
                    .aperture_cells(port)
                    .expect("a declared port punches cells");
                for (x, y) in cells {
                    assert!(x < cols && y < rows, "{kind:?} port {port} leaves the net");
                    assert!(
                        kind.surface_of(x, y).is_some(),
                        "{kind:?} port {port} punches a hole that is not a cell"
                    );
                    assert!(!seen.contains(&(x, y)), "{kind:?} ports share a cell");
                    seen.push((x, y));
                }
            }
            // An undeclared slot punches nothing: the wall stays a wall.
            for port in 0..PORTS as PortId {
                assert_eq!(
                    kind.port(port).is_none(),
                    kind.aperture_cells(port).is_none(),
                    "{kind:?} slot {port} disagrees with itself"
                );
            }
            assert!(kind.port(PORTS as PortId).is_none());
        }
    }

    /// The roster, kind by kind: the cabin is extensible and everything
    /// else states its business (docs/ROOMS.md's port law, and the
    /// owner's decree behind it). Changing a number here is a design
    /// decision, so it costs an edit to a test that says why.
    #[test]
    fn only_the_cabin_is_extensible() {
        for (kind, doors, vertical) in [
            (RoomKind::Cabin, 4, true),
            (RoomKind::Burner, 1, false),
            (RoomKind::Trade, 1, false),
            (RoomKind::Wreck, 1, false),
            (RoomKind::Parlor, 1, false),
            (RoomKind::Pump, 1, false),
        ] {
            let declared: Vec<(PortId, Port)> = kind.declared().collect();
            assert_eq!(
                declared
                    .iter()
                    .filter(|(_, port)| matches!(port, Port::Door { .. }))
                    .count(),
                doors,
                "{kind:?} does not declare {doors} doors"
            );
            assert_eq!(
                kind.port(LADDER).is_some() && kind.port(HATCH).is_some(),
                vertical,
                "{kind:?} disagrees with the decree about its vertical pair"
            );
            assert_eq!(
                declared.len(),
                doors + usize::from(vertical) * 2,
                "{kind:?} declares a port nobody argued for"
            );
        }
        // The furnace hangs off the cabin's starboard wall, as it has
        // since the first slice, and that is the only seam it has.
        assert!(matches!(
            RoomKind::Burner.port(3),
            Some(Port::Door { wall: 3, .. })
        ));
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

    /// `Sim::part`'s graph surgery, which is the only detach the game
    /// performs: a room parts with everything standing behind it, so the
    /// graph the ship holds is never disconnected. The escape-hatch
    /// guarantee leans on that, and this is where the test says so.
    fn part(rooms: &mut Rooms, id: RoomId) {
        let doomed: Vec<RoomId> = rooms
            .iter()
            .map(|(other, _)| other)
            .filter(|&other| rooms.beyond(id, other))
            .collect();
        for room in doomed {
            let _ = rooms.detach(room);
        }
    }

    /// The escape-hatch obligation, restated for a ship whose leaf rooms
    /// are dead ends, as an adversarial-order property (docs/ROOMS.md,
    /// "The escape-hatch guarantee"):
    ///
    /// 1. **A cabin always finds a berth.** Only cabins declare the
    ///    vertical pair, so only a cabin can reach a new storey — and
    ///    therefore every occupied storey holds one, the topmost one's
    ///    ladder is free, and the storey above it has never been
    ///    reachable by anything.
    /// 2. **A caller always fits the cabin that just arrived.** Four
    ///    unmated doors come with it, which is how a ship whose doors
    ///    are spent makes another one.
    ///
    /// If this fails the spawn contract is wrong, not the test.
    #[test]
    fn the_cabin_frontier_never_starves() {
        let mut rng = fastrand::Rng::with_seed(0x00E5_CA9E);
        for _ in 0..300 {
            let mut rooms = Rooms::new();
            // Spawn and part in adversarial orders, so the lattice is
            // left as ragged as the game can leave it.
            while rooms.count() < MAX_ROOMS {
                let anchors: Vec<RoomId> = rooms.iter().map(|(id, _)| id).collect();
                let from = anchors[rng.usize(..anchors.len())];
                // Clause one, on every ship this walk can build.
                let mut ahead = rooms.clone();
                let berthed = ahead.spawn(RoomKind::Cabin, from);
                assert!(berthed.is_ok(), "the cabin frontier starved: {berthed:?}");
                // Clause two: whatever the trip has done to the ship, the
                // cabin that just landed can host any caller there is.
                if let Ok(fresh) = berthed {
                    if ahead.count() < MAX_ROOMS {
                        for kind in ROOM_KINDS {
                            let mut alongside = ahead.clone();
                            let called = alongside.spawn(kind, fresh);
                            assert!(
                                called.is_ok(),
                                "{kind:?} starved on a fresh cabin: {called:?}"
                            );
                        }
                    }
                }
                let kind = ROOM_KINDS[rng.usize(..ROOM_KINDS.len())];
                let _ = rooms.spawn(kind, from);
                if rng.u8(..4) == 0 {
                    let victims: Vec<RoomId> = rooms
                        .iter()
                        .map(|(id, _)| id)
                        .filter(|&id| id != CABIN)
                        .collect();
                    if !victims.is_empty() {
                        part(&mut rooms, victims[rng.usize(..victims.len())]);
                    }
                }
                if rooms.count() == 1 {
                    // A ship parted back to its cabin still grows.
                    assert!(rooms.spawn(RoomKind::Burner, CABIN).is_ok());
                }
            }
            // Full is the only refusal a full ship may give.
            assert_eq!(rooms.spawn(RoomKind::Cabin, CABIN), Err(Refusal::Full));
        }
    }

    /// What the ship actually asks for: a dock's room and an unresolved
    /// event alongside at once. The yard-fresh ship seats **three**
    /// callers — the cabin's doors, less the furnace's — and the fourth
    /// is refused by name, which is the honest half of the guarantee.
    #[test]
    fn the_yard_fresh_ship_seats_every_caller_the_game_asks_for() {
        let callers = [
            RoomKind::Trade,
            RoomKind::Wreck,
            RoomKind::Parlor,
            RoomKind::Pump,
        ];
        for first in callers {
            for second in callers {
                for third in callers {
                    let mut rooms = Rooms::new();
                    for kind in [first, second, third] {
                        let called = rooms.spawn(kind, CABIN);
                        assert!(called.is_ok(), "{kind:?} found no door: {called:?}");
                    }
                    // Spent doors refuse, and the answer is another
                    // cabin — which brings four of its own.
                    assert!(rooms.spawn(RoomKind::Trade, CABIN).is_err());
                    let crew = rooms.spawn(RoomKind::Cabin, CABIN);
                    assert!(crew.is_ok(), "the crew cabin starved: {crew:?}");
                    assert!(rooms.spawn(RoomKind::Trade, crew.unwrap()).is_ok());
                }
            }
        }
    }

    /// Every refusal the attach contract can give, by name — including
    /// the new one the port law leans on: a slot a kind does not declare
    /// is `Absent`, exactly as a door carried off its wall will be.
    #[test]
    fn a_seam_that_will_not_close_is_refused_by_name() {
        let mut rooms = Rooms::root(RoomKind::Cabin);
        // A cabin above through the ladder, then a second one that would
        // have to share the same column.
        assert!(rooms.attach(CABIN, LADDER, RoomKind::Cabin, HATCH).is_ok());
        assert_eq!(
            rooms.attach(CABIN, LADDER, RoomKind::Cabin, HATCH),
            Err(Refusal::Mated)
        );
        // Ports that cannot mate are named too.
        assert_eq!(
            rooms.attach(CABIN, HATCH, RoomKind::Cabin, 0),
            Err(Refusal::Kinds)
        );
        // The market declares one door, aft. Its other three walls and
        // its ceiling are walls and a ceiling.
        assert_eq!(
            rooms.attach(CABIN, 0, RoomKind::Trade, 2),
            Err(Refusal::Absent)
        );
        assert_eq!(
            rooms.attach(CABIN, LADDER, RoomKind::Trade, HATCH),
            Err(Refusal::Absent)
        );
        // And a furnace mates through the one wall it hangs from.
        assert_eq!(
            rooms.attach(CABIN, 3, RoomKind::Burner, 1),
            Err(Refusal::Absent)
        );
        assert!(rooms.attach(CABIN, 3, RoomKind::Burner, 3).is_ok());
        assert_eq!(rooms.attach(9, 0, RoomKind::Trade, 0), Err(Refusal::Absent));
    }

    /// **The offer area is never in the way in.** The owner's rule
    /// change, pinned over every room kind and every declared port —
    /// which is the whole point of stating it against the doors instead
    /// of against a wall: one law, twelve stations, and nothing for a
    /// station to opt out of.
    ///
    /// The playtest walked through a station's chalk on the way in, at
    /// every station, because a band across the room is a band across
    /// the door's own lane as well. Now the lane is ordinary deck from
    /// the sill to the far wall, and the chalk sits either side of it.
    #[test]
    fn no_offer_tile_ever_lies_in_a_doors_entry_path() {
        for kind in ROOM_KINDS {
            let (cols, rows) = kind.grid();
            let mut offers = 0;
            let mut lane = 0;
            for y in 0..rows {
                for x in 0..cols {
                    if kind.entry_path(x, y) {
                        lane += 1;
                        assert_ne!(
                            kind.tile_of(x, y),
                            Some(Tile::Offer),
                            "{kind:?} chalks ({x}, {y}), which is the way in"
                        );
                    }
                    offers += usize::from(kind.tile_of(x, y) == Some(Tile::Offer));
                }
            }
            // The law may not empty the thing it constrains: a kind that
            // offers must still have somewhere to lay a proposal, on the
            // deck AND off it (a painting is proposed too).
            if matches!(kind, RoomKind::Trade | RoomKind::Parlor) {
                assert!(offers > 0, "{kind:?} has no offer area left at all");
                for want in [Surf::Floor, Surf::Front, Surf::Ceiling] {
                    assert!(
                        (0..rows).any(|y| (0..cols).any(|x| kind.surface_of(x, y) == Some(want)
                            && kind.tile_of(x, y) == Some(Tile::Offer))),
                        "{kind:?} lost its whole {want:?} offer band"
                    );
                }
            }
            // And a path exists wherever a door does, so the law is not
            // quietly vacuous on some kind.
            let doors = kind
                .declared()
                .filter(|(_, port)| matches!(port, Port::Door { .. }))
                .count();
            assert_eq!(lane > 0, doors > 0, "{kind:?} disagrees about its doors");
            // Every cell of the lane traces back to a door, floor to
            // ceiling: the run in is a volume, not a stripe of deck.
            for (port, declared) in kind.declared() {
                let Port::Door { .. } = declared else {
                    continue;
                };
                let step = kind
                    .aperture_cells(port)
                    .expect("a declared port punches cells");
                for (x, y) in step {
                    assert!(
                        kind.entry_path(x, y),
                        "{kind:?} port {port} punches ({x}, {y}) outside its own entry path"
                    );
                }
            }
        }
    }

    /// **A room keeps the cells its own hardware stands in.**
    ///
    /// The net used to say that every cell of every chart was a berth,
    /// which meant a room's handshake stood in one and its pendant hung
    /// in another and both were clipping cargo the moment anybody put
    /// cargo there. The socket the handshake is set into was already a
    /// hole; the deck it is worked over and the ceiling the lamp hangs
    /// from are the rest of the same argument, and they are stated here
    /// over every kind so a kind that grows a fixture cannot forget to
    /// keep the room for it.
    ///
    /// What the law may NOT do is take the room away from the cargo: it
    /// reserves the counter's own cell and the middle of a ceiling, and
    /// it is checked to reserve nothing else.
    #[test]
    fn a_room_keeps_the_cells_its_own_hardware_stands_in() {
        for kind in ROOM_KINDS {
            let (cols, rows) = kind.grid();
            let (w, h) = kind.floor();
            let (mut counter, mut corner, mut pendant, mut berths) =
                (0_usize, 0_usize, 0_usize, 0_usize);
            for y in 0..rows {
                for x in 0..cols {
                    let Some(tile) = kind.tile_of(x, y) else {
                        continue;
                    };
                    if tile != Tile::Fixture {
                        berths += usize::from(tile != Tile::Threshold);
                        continue;
                    }
                    assert!(kind.fixture(x, y), "{kind:?} paints ({x}, {y}) by accident");
                    match kind.surface_of(x, y) {
                        // A fixture on the DECK is furniture: a body has
                        // to walk round it, so it may not stand in the
                        // way in — the same argument the entry-path law
                        // makes about chalk. A fixture on the ceiling is
                        // hung over, and a lamp over the door's own lane
                        // is a lamp lighting the door.
                        Some(Surf::Floor) => {
                            counter += 1;
                            assert!(
                                !kind.entry_path(x, y) && !kind.doorstep(x, y),
                                "{kind:?} sets its counter at ({x}, {y}), the way in"
                            );
                        }
                        Some(Surf::Port | Surf::Starboard) => corner += 1,
                        Some(Surf::Ceiling) => pendant += 1,
                        surf => panic!("{kind:?} reserves ({x}, {y}), which is {surf:?}"),
                    }
                }
            }
            assert_eq!(
                counter,
                usize::from(kind.handshake().is_some()),
                "{kind:?} does not keep exactly the deck its counter is worked over"
            );
            // And the wall it turns onto, in a room narrow enough that
            // its counter's cell is the corner. A market is six wide and
            // has none; a pump bay is three and is all corner.
            let cornered = kind
                .handshake()
                .is_some_and(|(hx, _)| hx == COURSES || hx + 1 == COURSES + w);
            assert_eq!(
                corner,
                usize::from(cornered),
                "{kind:?} does not keep the wall its counter turns onto"
            );
            // One ceiling cell where the room's middle lands mid-cell,
            // two where it lands on a seam, four where both do.
            let middles = usize::from(2 - w % 2) * usize::from(2 - h % 2);
            assert_eq!(
                pendant,
                if kind.riding() { 0 } else { middles },
                "{kind:?} does not keep the ceiling its own pendant hangs from"
            );
            assert!(berths > 0, "{kind:?} reserved its whole net");
        }
    }

    /// **The doorstep is deck, and it is exactly the deck a door stands
    /// on.** One cell per aperture course along the wall, in the room's
    /// own net, on the floor chart and never on a wall — a rule that
    /// named a wall cell would be a rule about the aperture, which is
    /// what `Tile::Threshold` already is.
    #[test]
    fn every_declared_door_has_a_doorstep_on_its_own_deck() {
        for kind in ROOM_KINDS {
            let (cols, rows) = kind.grid();
            let mut steps = 0;
            for y in 0..rows {
                for x in 0..cols {
                    if !kind.doorstep(x, y) {
                        continue;
                    }
                    steps += 1;
                    assert_eq!(
                        kind.surface_of(x, y),
                        Some(Surf::Floor),
                        "{kind:?}'s doorstep at ({x}, {y}) is not deck"
                    );
                    assert_ne!(
                        kind.tile_of(x, y),
                        Some(Tile::Threshold),
                        "{kind:?}'s doorstep at ({x}, {y}) is the aperture, not the deck"
                    );
                }
            }
            // One per aperture course per door, less whatever two doors
            // share: the cabin's aft and port doors both stand on the
            // aft-port corner cell, and a corner is one cell.
            let (w, h) = kind.floor();
            let mut want: Vec<(u8, u8)> = Vec::new();
            for (_, port) in kind.declared() {
                let Port::Door { wall, offset } = port else {
                    continue;
                };
                for along in 0..APERTURE {
                    let cell = match wall % 4 {
                        0 => (offset + along, 0),
                        1 => (w - 1, offset + along),
                        2 => (offset + along, h - 1),
                        _ => (0, offset + along),
                    };
                    if !want.contains(&cell) {
                        want.push(cell);
                    }
                }
            }
            assert_eq!(
                steps,
                want.len(),
                "{kind:?} has {steps} doorstep cells for {} the declaration names",
                want.len()
            );
        }
        // And a room with no door has no doorstep anywhere.
        assert!(!(0..RoomKind::Cabin.grid().0).any(|x| {
            (0..RoomKind::Cabin.grid().1).any(|y| {
                RoomKind::Cabin.doorstep(x, y)
                    && RoomKind::Cabin.surface_of(x, y) != Some(Surf::Floor)
            })
        }),);
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
