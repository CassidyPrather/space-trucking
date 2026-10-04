//! Cargo pieces: what they are, where they sit, and the stowage rules.
//!
//! A [`Piece`] is one draggable object. Its [`Loc`] says which room it
//! stands in, where on that room's net, in 256ths of a cell ([`FINE`]),
//! and how far it is turned on its chart ([`Turn`]) — every berth in the
//! game is a position on a room's net — and [`placement_check`] is the
//! single arbiter of whether a piece may sit there. The renderer and the
//! drag logic both defer to it, so there is exactly one opinion about
//! what fits, and a failure names the [`Violation`] so the frontend can
//! flash the right icon.
//!
//! **What fits is the room's question, not the cargo's** (docs/BAY.md,
//! "Cargo stops colliding"). The arbiter asks about the room — its
//! charts, its doorways, its hardware, a kind's mount and the hull's
//! cold — and about the few special items that still answer to each
//! other (volatile spacing, one suspicious piece aboard). It never asks
//! whether one piece's body meets another's: placement is to taste, and
//! a crate may stand in a wardrobe. What the GAME sets down itself still
//! looks for free space first ([`tidy`]), because a room the game
//! furnished should not arrive in a heap.
//!
//! Ownership is not a list. [`player_owned`] asks the berth's room and
//! tile class and nothing else (docs/ROOMS.md, "The tile-class
//! vocabulary"): the room's own goods sit on `Stock` tiles, and
//! everything else aboard or alongside is the player's.

use super::room::{COURSES, RoomId, RoomKind, Rooms, Surf, Tile};

/// Everything haulable. Declaration order is the stable [`Kind::index`]
/// order that the barter value table is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    PerfumeVial,
    GildedIdol,
    RationBricks,
    ScrapAlloy,
    Seedlings,
    GasCanister,
    CryoCore,
    BrinePearls,
    SuspiciousCrate,
    /// A small humming box. Three aboard and something notices.
    MysteriousCrate,
    /// What ??? trades three mysterious crates for. The Guild counts it
    /// as four deliveries; nobody explains the arithmetic.
    VeryMysteriousCrate,
    /// Chipped off a comet at perihelion. Free, if you can catch it.
    CometIce,
    /// Bottled at the Umbra Market during business hours only.
    BottledMidnight,
    /// A legally distinct ball of fur. It was two balls of fur a
    /// moment ago. (It multiplies in transit; see the fluff event.)
    Fluff,
    /// Inner-ring transit papers, brokered by the Guild. Carrying one
    /// lets a course be charted directly between Venus, Earth, and Mars,
    /// whose factions otherwise refuse each other's traffic.
    TransitChit,
    /// What the space casino hands back when the house wins. The house
    /// says it is worth a fortune. Every station disagrees.
    CasinoChip,
    /// A hanging shade for the hold's gantry. Lit while berthed, like
    /// every lamp — cargo that casts light on its neighbours.
    CeilingLamp,
    /// A sconce off a repossessed liner, wall fittings included.
    WallLamp,
    /// A standing lamp, shade up top, base bolted to the deck.
    FloorLamp,
    /// Somebody's living room, in transit. The rat agrees.
    Couch,
    /// Gilt frame, subject debatable. Shows best under lamplight.
    Painting,
    /// A slim deck-bolted wardrobe in oiled oak. Furniture, and nothing
    /// more: it stores nothing (docs/BAY.md, "Cargo stops colliding"),
    /// and what stands in it stands there because somebody put it there.
    Cabinet,
    /// Somebody's heirloom, woven warm and gnawably soft. Lays on the
    /// deck (see `Loc::Laid`) and cargo stands on it without complaint.
    Rug,
    /// Ship enamel in a battered tin, color by the tin's roll. Coats
    /// one cell of the room; scrapes off mostly usable.
    PaintTin,
    /// Paint that glows — strained, the label implies, from something
    /// that should not be strained. A laid coat lights its neighbours
    /// like a weak lamp; the Umbra Market sells it snuffed, in
    /// blackout tins.
    LuminousPaint,
    /// The exterior window: hangs like a painting, shows space like a
    /// window, asks no further questions. Rehang it on any wall and
    /// the void follows — whimsy dictates the physics defers.
    Window,
    /// The chart tank: the star map in a phosphor aquarium, off the
    /// wall at last. Vital — the last one aboard refuses every exit,
    /// because a ship that cannot chart is a coffin with a rug.
    ChartTank,
    /// The ETA gauge: a passive arc that reads the current leg. Not
    /// vital; flying without one is legal and merely nerve-wracking.
    EtaGauge,
    /// The destination preview: a small glass showing where the
    /// selected course ends. Not vital; surprises build character.
    DestPreview,
    /// The launch handle: the lever that commits a charted course.
    /// Vital — the last one aboard refuses every exit.
    LaunchLever,
    /// A hand's breadth of glass in a bolt ring — the cheapest hole
    /// anybody ever cut in a hull, and the one every hull has. Fits a
    /// wall no wider than itself, which is why the little rooms get
    /// one and the big pane never reaches them.
    Porthole,
    /// Four cells of glass in a frame that arrives in two crates.
    /// Saturn's, and only Saturn's: that ring is somebody else's hull
    /// all the way round, and somebody else's hull is where big flat
    /// glass comes from. Hung, it gives back twice the sky the ship
    /// launched with. The freight on one is why nobody hauls two.
    BayWindow,
}

/// Number of cargo kinds.
///
/// The discovery ledger is a `u32` bitmask per station (`Sim::familiar`),
/// so **32 is the ceiling** and this table is at it. The next kind widens
/// that mask before it widens this number.
pub const KIND_COUNT: usize = 32;

/// Cosmetic variant rolls per kind, for the renderer to vary sprites with.
/// The persistent run RNG is spent on these and nothing else.
pub(crate) const VARIANTS: u8 = 4;

/// Special handling a kind demands in a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// Weighty. Dormant since the room grid put all standing cargo on
    /// the floor (heavy-rides-low had nothing left to refuse); kept as
    /// kind data for the stacking rules to consume (`supports:top`,
    /// BAY.md) — nothing heavy will ride on top of anything.
    Heavy,
    /// No two volatile pieces may stand within half a cell of each other.
    Volatile,
    /// Must touch the room's outer edge.
    Cryo,
    /// At most one suspicious piece aboard, and hauling it has consequences.
    Suspicious,
    /// A fixture: its footprint must touch the named room surface.
    Affix(Mount),
    /// A dressing: it lays *into* the room (`Loc::Laid`) instead of
    /// occupying cells. `Some(mount)` restricts which surface it
    /// covers; `None` coats anywhere.
    Covering(Option<Mount>),
}

/// The plane class a kind stands on.
///
/// Under the room net (`room::RoomKind::surface_of`), every placement
/// lies wholly in one chart and that chart's class must match the kind's
/// mount — the room-grid placement law (BAY.md): floor unless otherwise
/// specified, walls for paintings and instruments, ceilings for hanging
/// lamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    Ceiling,
    Floor,
    Wall,
}

/// Whether chart `surf` satisfies `mount`. Any of the four walls is
/// "the wall"; nobody hangs a painting on a compass heading.
///
/// The arbiter's own clause, and public because the drawing has to ask
/// it too: which charts a kind may be berthed on decides which plane
/// its body has to reach, and a frontend that answered that from the
/// mount itself would be a second copy of this table
/// (`cabin::gauntlet`, `rig-seated`).
#[must_use]
pub const fn mount_accepts(mount: Mount, surf: Surf) -> bool {
    matches!(
        (mount, Mount::of(surf)),
        (Mount::Floor, Mount::Floor)
            | (Mount::Ceiling, Mount::Ceiling)
            | (Mount::Wall, Mount::Wall)
    )
}

impl Mount {
    /// **The class of chart `surf`**: the deck, the deckhead, or a wall,
    /// any of the four. [`mount_accepts`] asks a kind's mount of it, and a
    /// carry's lift is kept from one chart to another of the same class
    /// and starts again from the surface across two (docs/BAY.md, "Lift"):
    /// a height off the deck means nothing on a wall.
    #[must_use]
    pub const fn of(surf: Surf) -> Self {
        match surf {
            Surf::Floor => Self::Floor,
            Surf::Ceiling => Self::Ceiling,
            Surf::Aft | Surf::Port | Surf::Starboard | Surf::Front => Self::Wall,
        }
    }
}

impl Kind {
    /// Every kind, in [`Kind::index`] order. For iteration; the sim itself
    /// never needs to enumerate kinds by number.
    pub const ALL: [Self; KIND_COUNT] = [
        Self::PerfumeVial,
        Self::GildedIdol,
        Self::RationBricks,
        Self::ScrapAlloy,
        Self::Seedlings,
        Self::GasCanister,
        Self::CryoCore,
        Self::BrinePearls,
        Self::SuspiciousCrate,
        Self::MysteriousCrate,
        Self::VeryMysteriousCrate,
        Self::CometIce,
        Self::BottledMidnight,
        Self::Fluff,
        Self::TransitChit,
        Self::CasinoChip,
        Self::CeilingLamp,
        Self::WallLamp,
        Self::FloorLamp,
        Self::Couch,
        Self::Painting,
        Self::Cabinet,
        Self::Rug,
        Self::PaintTin,
        Self::LuminousPaint,
        Self::Window,
        Self::ChartTank,
        Self::EtaGauge,
        Self::DestPreview,
        Self::LaunchLever,
        Self::Porthole,
        Self::BayWindow,
    ];

    /// **What a kind takes up, in cells of its own frame**: `(across,
    /// deep, tall)`.
    ///
    /// A body has one shape and this is it — across the face it shows
    /// the room, deep away from whatever it sits against, tall up from
    /// it. Which two of the three a berth spends is the BERTH's business
    /// ([`Kind::face_on`]) and never the kind's: a wardrobe is one cell
    /// of deck and two courses tall, and it is that whichever chart it
    /// finds itself on.
    ///
    /// **This is the re-authored 3D extent** (docs/BAY.md). The retired
    /// console's glyph `(w, h)` was doing all three jobs before it, and
    /// the second number was an ELEVATION: on a wall it meant courses,
    /// on the deck the same number was read as depth, so a 1×2 wardrobe
    /// claimed 1.06 m of deck for a body that reaches 0.53 m into the
    /// room. Half a metre of bare deck in front of every standing piece
    /// answered for the piece, which is the hitbox the playtest could
    /// not line up with anything it could see.
    ///
    /// Everything is one cell deep, and that is the same sentence
    /// `pieces::RIG_NEAR..RIG_FAR` says in the frontend's own units. The
    /// day something wants two, the number is here to say so.
    #[must_use]
    pub const fn extent(self) -> (u8, u8, u8) {
        match self {
            Self::PerfumeVial
            | Self::Seedlings
            | Self::CryoCore
            | Self::MysteriousCrate
            | Self::CometIce
            | Self::BottledMidnight
            | Self::Fluff
            | Self::TransitChit
            | Self::CasinoChip
            | Self::CeilingLamp
            | Self::WallLamp
            | Self::PaintTin
            | Self::LuminousPaint
            | Self::EtaGauge
            | Self::DestPreview
            | Self::LaunchLever
            | Self::Porthole => (1, 1, 1),
            Self::GildedIdol | Self::BrinePearls | Self::FloorLamp | Self::Cabinet => (1, 1, 2),
            Self::RationBricks
            | Self::SuspiciousCrate
            | Self::VeryMysteriousCrate
            // Square on its face, and the same square the chart tank is:
            // the biggest thing this ship already knows how to hang on a
            // wall. Bigger was tried and refused by the arithmetic —
            // a calling room's shelf is its aft wall with a handshake
            // in the middle of it and a doorway through the corner,
            // and while the walls were three courses tall nothing three
            // cells wide and two courses tall could stand anywhere on
            // it. A window no station can put out is a window nobody
            // can buy (`barter`, the shelf-fit test). The walls are a
            // course taller now, so a wider pane would fit over the
            // counter; whether one is wanted is a design question, and
            // this square is the answer until somebody asks it.
            | Self::BayWindow
            | Self::ChartTank => (2, 1, 2),
            Self::ScrapAlloy
            | Self::GasCanister
            | Self::Couch
            | Self::Painting
            | Self::Rug
            | Self::Window => (2, 1, 1),
        }
    }

    /// **The face a rig is drawn on**, `(across, tall)`: the kind's own
    /// upright frame, which no berth turns. A drawing is composed here
    /// and a berth spins it: the piece's own [`Turn`] about its chart's
    /// normal, laid on the sheet by [`net_angle`].
    #[must_use]
    pub const fn upright(self) -> (u8, u8) {
        let (across, _, tall) = self.extent();
        (across, tall)
    }

    /// **The two of this kind's three extents a berth on a chart of
    /// class `surf` spends**, `(across, span)`, in the BODY's own frame:
    /// its footprint before anything turns it.
    ///
    /// Two readings, and the chart picks:
    ///
    /// - A chart a body lies **on** — the deck, the deckhead — spends
    ///   the plan: across by deep. What is left over is the height, and
    ///   the height is not the deck's to spend ([`Kind::stature`]).
    /// - A chart a body hangs **against** spends the elevation: across
    ///   by tall, and the wall fixes the depth.
    ///
    /// Which way round those land on the sheet is not the kind's to say,
    /// and it is not this function's either. The net is one sheet of
    /// paper folded into a box and its two side flaps fold out SIDEWAYS:
    /// a flank's courses climb the sheet's **x** where the aft and front
    /// walls' climb its **y**. That used to be a transposition written
    /// here, and before that it was the athwart rule, which refused the
    /// wall: a footprint declared in the sheet's frame meant "side by
    /// side" on one wall and "one above the other" on another, so a
    /// window carried one wall over came out a quarter turn from the
    /// window that left. A body keeps its shape, and [`net_angle`] is the
    /// one place that lays it on the sheet, at whatever turn it has
    /// (docs/BAY.md, "Cargo turns").
    #[must_use]
    pub const fn face_on(self, surf: Surf) -> (u8, u8) {
        let (across, deep, tall) = self.extent();
        match surf {
            Surf::Floor | Surf::Ceiling => (across, deep),
            Surf::Aft | Surf::Front | Surf::Port | Surf::Starboard => (across, tall),
        }
    }

    /// **How far a body of this kind reaches off a chart of class
    /// `surf`**, in cells along the chart's normal: the one of its three
    /// extents that [`Kind::face_on`] leaves over. Its height, standing on
    /// the deck or hanging from the deckhead, and its depth out of a wall.
    /// What [`lift_cap`] takes off the room's section, so a lifted body
    /// stops where its far side meets the room's.
    #[must_use]
    pub const fn proud(self, surf: Surf) -> u8 {
        let (_, deep, tall) = self.extent();
        match surf {
            Surf::Floor | Surf::Ceiling => tall,
            Surf::Aft | Surf::Front | Surf::Port | Surf::Starboard => deep,
        }
    }

    /// Stowage constraint, if any.
    #[must_use]
    pub const fn tag(self) -> Option<Tag> {
        match self {
            Self::GildedIdol | Self::ScrapAlloy => Some(Tag::Heavy),
            Self::GasCanister => Some(Tag::Volatile),
            Self::CryoCore | Self::CometIce => Some(Tag::Cryo),
            Self::SuspiciousCrate | Self::VeryMysteriousCrate => Some(Tag::Suspicious),
            Self::CeilingLamp => Some(Tag::Affix(Mount::Ceiling)),
            Self::FloorLamp | Self::Couch | Self::Cabinet => Some(Tag::Affix(Mount::Floor)),
            Self::WallLamp
            | Self::Painting
            | Self::Window
            | Self::Porthole
            | Self::BayWindow
            | Self::ChartTank
            | Self::EtaGauge
            | Self::DestPreview
            | Self::LaunchLever => Some(Tag::Affix(Mount::Wall)),
            Self::Rug => Some(Tag::Covering(Some(Mount::Floor))),
            Self::PaintTin | Self::LuminousPaint => Some(Tag::Covering(None)),
            _ => None,
        }
    }

    /// Where this kind stands — its effective mount. The room-grid law:
    /// unless otherwise specified, cargo goes on the floor; fixtures
    /// keep their affixed surface. Coverings answer to
    /// [`dressing_check`] instead and never consult this.
    #[must_use]
    pub const fn mount(self) -> Mount {
        match self.tag() {
            Some(Tag::Affix(mount)) => mount,
            _ => Mount::Floor,
        }
    }

    /// Standing height on the floor, in wall cells — how far up an
    /// adjacent wall this kind shadows when it stands against one (the
    /// game hangs no painting behind the wardrobe, [`clear`]). The third number of the kind's
    /// own [`Kind::extent`], which is where a height belongs: the deck
    /// spends a plan, and the wall behind the deck is what a height is
    /// spent on.
    #[must_use]
    pub const fn stature(self) -> u8 {
        self.extent().2
    }

    /// Whether this kind is an operational instrument the ship cannot
    /// function without: the LAST one of a vital kind in the player's
    /// possession refuses every exit ceremony — the offer area of a
    /// calling room, the incinerator's hazard tiles, the casino's
    /// wager — with `Violation::Vital`, checked in `resolve_drop`.
    /// Spares trade freely; stations occasionally stock used
    /// instruments, which is its own little economy.
    #[must_use]
    pub const fn vital(self) -> bool {
        matches!(self, Self::ChartTank | Self::LaunchLever)
    }

    /// Whether this kind is a **window**: a hole in the hull with glass
    /// in it, whatever size the hole is.
    ///
    /// The family shares everything that matters — the wall mount, the
    /// frame, the sky pane, the whimsy rule that the void follows
    /// wherever it is rehung — and differs only in how much of the
    /// outside it lets in. Anything that wants "a window" rather than
    /// "the 2×1 one" asks here.
    #[must_use]
    pub const fn window(self) -> bool {
        matches!(self, Self::Window | Self::Porthole | Self::BayWindow)
    }

    /// Whether this kind is a dressing — laid into the room rather than
    /// standing on it. Coverings have no occupancy form at all.
    #[must_use]
    pub const fn covering(self) -> bool {
        matches!(self.tag(), Some(Tag::Covering(_)))
    }

    /// How eagerly the burner takes this kind, `0..=3`: stoke earned per
    /// piece fed to the fire. Upholstery, fur, and fuel go up gloriously;
    /// wood and paper honestly; metal, stone, and ice are slag — the
    /// stoker still shovels them through (disposal is disposal), they
    /// just push nothing. The suspicious kinds never reach the hopper at
    /// all (they refuse the hazard tiles), so their values here are moot.
    #[must_use]
    pub const fn flammable(self) -> u8 {
        match self {
            Self::Fluff | Self::Rug | Self::Couch | Self::GasCanister => 3,
            Self::Seedlings
            | Self::RationBricks
            | Self::Painting
            | Self::Cabinet
            | Self::PerfumeVial
            | Self::LuminousPaint => 2,
            Self::TransitChit
            | Self::CasinoChip
            | Self::BottledMidnight
            | Self::PaintTin
            | Self::CeilingLamp
            | Self::WallLamp
            | Self::FloorLamp
            | Self::MysteriousCrate => 1,
            Self::GildedIdol
            | Self::ScrapAlloy
            | Self::CryoCore
            | Self::BrinePearls
            | Self::CometIce
            | Self::SuspiciousCrate
            | Self::VeryMysteriousCrate
            | Self::Window
            | Self::Porthole
            | Self::BayWindow
            | Self::ChartTank
            | Self::EtaGauge
            | Self::DestPreview
            | Self::LaunchLever => 0,
        }
    }

    /// Stable column index into the barter value table.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Which berth a piece sits in.
///
/// Every berth in the game is a position on some room's net, in one of
/// two layers: standing in the room, or laid into its surface.
///
/// **A position is not a cell, and a body has a turn** (docs/BAY.md,
/// "The grid comes out" and "Cargo turns"). `x` and `y` are the
/// footprint's CENTRE in [`FINE`] units of the room's net — net cell
/// `(cx, cy)` spans `cx * FINE .. (cx + 1) * FINE` — and `turn` is how
/// far the body is turned about its chart's normal ([`Turn`]). The
/// centre is the anchor because it is the one point of a footprint no
/// turn moves: a top-left corner is somewhere else at every angle. The
/// footprint itself is still whole cells in size ([`Foot`]).
///
/// **And a standing body has a height off its surface** (docs/BAY.md,
/// "Lift"). `lift` is how far a berth stands off its chart, in [`FINE`]
/// units along the chart's normal and away from it into the room: up off
/// the deck, down from the deckhead, out from a wall. It is the third
/// coordinate a vase needs to stand on a cabinet's top now that nothing
/// stores anything, and it is a TASTE coordinate: the sim stores it,
/// saves it, sends it and caps it ([`lift_cap`]), and no rule reads it.
/// The arbiter is asked about the plane ([`Loc::spot`]), so light, the
/// volatile spacing and every other ruling stay what they were on the
/// ground under the body. A laid covering has no lift: a coat is flush
/// with what it coats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loc {
    /// Standing in a room: the footprint's centre, in fine units of the
    /// room's net, the body's turn on its chart, and how far it stands
    /// off that chart into the room.
    Hold {
        room: RoomId,
        x: u16,
        y: u16,
        turn: Turn,
        lift: u16,
    },
    /// Laid into a room centred on `(x, y)`, in fine units, at `turn`:
    /// the dressing layer. Coexists with occupancy over the same ground
    /// (a couch stands on a laid rug), and with other dressings too:
    /// nothing in a room is refused for the body it meets.
    Laid {
        room: RoomId,
        x: u16,
        y: u16,
        turn: Turn,
    },
}

impl Loc {
    /// Which room this berth is in.
    #[must_use]
    pub const fn room(self) -> RoomId {
        match self {
            Self::Hold { room, .. } | Self::Laid { room, .. } => room,
        }
    }

    /// The room position this berth names, whichever layer it lies in:
    /// the plane, and nothing of the [`Loc::lift`], because the plane is
    /// all an arbiter is asked about.
    #[must_use]
    pub const fn spot(self) -> Spot {
        match self {
            Self::Hold {
                room, x, y, turn, ..
            }
            | Self::Laid { room, x, y, turn } => Spot { room, x, y, turn },
        }
    }

    /// How far this berth stands off its chart, in [`FINE`] units: a
    /// standing body's own lift, and nothing for a laid covering.
    #[must_use]
    pub const fn lift(self) -> u16 {
        match self {
            Self::Hold { lift, .. } => lift,
            Self::Laid { .. } => 0,
        }
    }
}

/// **A position in a room**: the room, a footprint's centre on its net in
/// [`FINE`] units, and the body's [`Turn`] on its chart.
///
/// Everything a berth in a room says except which layer it lies in — what
/// the two arbiters ([`placement_check`], [`dressing_check`]) are asked
/// about, and what the fitting scans ([`first_fit`], [`dress_fit`]) and
/// the drop answer with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub room: RoomId,
    pub x: u16,
    pub y: u16,
    pub turn: Turn,
}

impl Spot {
    /// This spot as a berth standing on its chart: no lift, which is
    /// where the game sets down everything it places itself.
    #[must_use]
    pub const fn hold(self) -> Loc {
        self.lifted(0)
    }

    /// This spot as a berth standing `lift` [`FINE`] units off its chart
    /// (docs/BAY.md, "Lift"). Only a player's carry ever asks for one
    /// above the chart, and the drop has already held it to the cap.
    #[must_use]
    pub const fn lifted(self, lift: u16) -> Loc {
        Loc::Hold {
            room: self.room,
            x: self.x,
            y: self.y,
            turn: self.turn,
            lift,
        }
    }

    /// This spot as a dressing laid into its room.
    #[must_use]
    pub const fn laid(self) -> Loc {
        Loc::Laid {
            room: self.room,
            x: self.x,
            y: self.y,
            turn: self.turn,
        }
    }

    /// The berth `kind` takes here: laid if it is a covering, standing
    /// otherwise — the one layer each kind has.
    #[must_use]
    pub const fn berth(self, kind: Kind) -> Loc {
        self.berth_at(kind, 0)
    }

    /// The berth `kind` takes here `lift` off its chart: a standing body
    /// [`Spot::lifted`], and a covering laid flush whatever it is asked,
    /// because a coat has no height to stand at.
    #[must_use]
    pub const fn berth_at(self, kind: Kind, lift: u16) -> Loc {
        if kind.covering() {
            self.laid()
        } else {
            self.lifted(lift)
        }
    }
}

/// **Sub-units per cell**: the quantum a berth is positioned in.
///
/// A 256th of a cell, about 2 mm in the cabin. It was a sixteenth, 34 mm,
/// which is a grid a hand can see: cargo is placed by somebody grabbing
/// and moving it, eventually in VR, and the owner wants no snapping a
/// hand can feel (docs/BAY.md, "Cargo turns"). Whole numbers all the way
/// down, so no floating point ever reaches the arbiter and a lockstep
/// crew agrees to the unit. The widest lane is 22 cells, 5,632 units, so
/// a `u16` holds every centre a net has with room to spare.
pub const FINE: u16 = 256;

/// **The fine offsets a sweep tries on each axis** of every whole-cell
/// anchor, when a test means "every berth".
///
/// A berth can be any of 65,536 positions per cell, and a sweep over all
/// of them buys very little a sample does not, so the sweeps share one
/// fixed sample instead: on the grid, the finest unit past it either
/// way, and either side of the half. Deterministic, so a failure names
/// the same berth every run.
pub const FRACTIONS: [u16; 5] = [0, 1, FINE / 2 - 1, FINE / 2, FINE - 1];

/// **The highest a body of `kind` may stand off chart `surf` of `host`**,
/// in [`FINE`] units.
///
/// The room's section off that chart ([`RoomKind::section`]) less the
/// body's own reach off it ([`Kind::proud`]), so a lifted body stays
/// inside the room's box. A
/// wardrobe raised as far as it goes meets the deckhead with its top, a
/// pendant lowered all the way stands on the deck, and a painting carried
/// out from its wall stops with its face against the wall across the
/// room. A covering's is nought: a coat is flush with what it coats.
///
/// **The one statement of the cap** (docs/BAY.md, "Lift"). The drop holds
/// a carry's lift to it (`Sim::drop_preview`, the release's own
/// resolution), a save refuses a berth past it, and a frontend asks it so
/// the lift it keeps never climbs where the drop will not follow. Whole
/// cells, because a room's section and a kind's extent both are.
#[must_use]
pub const fn lift_cap(host: RoomKind, kind: Kind, surf: Surf) -> u16 {
    let room = host.section(surf);
    let body = kind.proud(surf);
    if kind.covering() || room <= body {
        0
    } else {
        (room - body) as u16 * FINE
    }
}

/// The fine coordinate of cell `cell`'s top-left edge.
#[must_use]
pub const fn fine(cell: u8) -> u16 {
    cell as u16 * FINE
}

/// The cell holding fine coordinate `fine`. Saturates past the last
/// cell a `u8` can name, which no net comes near.
#[must_use]
pub const fn coarse(fine: u16) -> u8 {
    let cell = fine / FINE;
    if cell > u8::MAX as u16 {
        u8::MAX
    } else {
        cell as u8
    }
}

/// The cell holding signed fine coordinate `fine`: [`coarse`] for the
/// geometry, whose corners may stand a hair off the net's own sheet.
/// Anything left of the sheet reads the sheet's first cell.
const fn cell_of(fine: i32) -> u8 {
    if fine <= 0 {
        0
    } else {
        let fine = fine.unsigned_abs();
        coarse(if fine > u16::MAX as u32 {
            u16::MAX
        } else {
            fine as u16
        })
    }
}

/// **A turn about a chart's normal**: a binary angle, 65,536 to the full
/// turn (0.0055°), wrapping where a `u16` wraps.
///
/// Positive is counter-clockwise **as seen by a person in the room looking
/// at the surface** — down at the deck, up at the deckhead, straight at a
/// wall. `Turn(0)` is the chart's upright frame: on a wall up is up, and
/// on the deck and the deckhead the body faces the room's front. Which
/// way any of that lies on the net is [`net_angle`]'s to say, and nobody
/// else's.
///
/// Any angle is a berth (docs/BAY.md, "Cargo turns"). The unit is one
/// nobody can see, and nothing in the sim snaps a turn: a convenient
/// angle is a frontend's offer on input, never something a rule depends
/// on. It is a whole number for the reason a position is ([`FINE`]): the
/// arbiter decides with integers, so a crew in lockstep agrees on every
/// ruling at every angle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Turn(pub u16);

impl Turn {
    /// The chart's own upright frame.
    pub const ZERO: Self = Self(0);
    /// A quarter turn, counter-clockwise as seen from the room.
    pub const QUARTER: Self = Self(1 << 14);
    /// Half a turn.
    pub const HALF: Self = Self(1 << 15);

    /// `n` quarter turns, wrapping.
    #[must_use]
    pub const fn quarters(n: u8) -> Self {
        Self((n as u16 % 4) << 14)
    }

    /// Whether this is a whole number of quarter turns: a turn that lays
    /// a footprint along the net's own axes, where the trig is exact and
    /// a footprint is exactly the rectangle the grid always drew.
    #[must_use]
    pub const fn square(self) -> bool {
        self.0 & (Self::QUARTER.0 - 1) == 0
    }

    /// The cosine, in [`TRIG_ONE`] units.
    #[must_use]
    pub const fn cos(self) -> i64 {
        let r = (self.0 & (Self::QUARTER.0 - 1)) as i64;
        match self.0 >> 14 {
            0 => cos_quadrant(r),
            1 => -sin_quadrant(r),
            2 => -cos_quadrant(r),
            _ => sin_quadrant(r),
        }
    }

    /// The sine, in [`TRIG_ONE`] units.
    #[must_use]
    pub const fn sin(self) -> i64 {
        let r = (self.0 & (Self::QUARTER.0 - 1)) as i64;
        match self.0 >> 14 {
            0 => sin_quadrant(r),
            1 => cos_quadrant(r),
            2 => -sin_quadrant(r),
            _ => -cos_quadrant(r),
        }
    }

    /// This turn in radians, counter-clockwise as seen from the room —
    /// for a frontend posing a body, never for a rule. The one
    /// multiplication is basic arithmetic, and no rule reads the answer.
    #[must_use]
    pub fn radians(self) -> f32 {
        f32::from(self.0) * (std::f32::consts::TAU / 65_536.0)
    }
}

impl std::ops::Add for Turn {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0.wrapping_add(other.0))
    }
}

impl std::ops::Sub for Turn {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0.wrapping_sub(other.0))
    }
}

/// **The fixed-point one [`Turn::cos`] and [`Turn::sin`] answer in**:
/// 2^30, so a cosine is good to about a billionth and its product with
/// any coordinate a net has fits an `i64` with room to spare.
pub const TRIG_ONE: i64 = 1 << 30;

/// π in [`TRIG_ONE`] units, rounded: the one transcendental number the
/// trig is built from, written down rather than computed.
const PI_FIX: i64 = 3_373_259_426;

/// A product of two [`TRIG_ONE`]-scaled magnitudes, rounded back to one
/// scale. Both are non-negative wherever it is used.
const fn q30(a: i64, b: i64) -> i64 {
    (a * b + TRIG_ONE / 2) >> 30
}

/// The first octant's sine and cosine, `x` in `0..=8192` sixty-five
/// thousandths of a turn, by their Taylor series to the eleventh and
/// twelfth powers in Horner form — integer multiplies and divides, each
/// rounded the one way, so every machine computes the same bits. On an
/// octant the dropped terms are below a part in a hundred billion.
const fn octant(x: i64) -> (i64, i64) {
    // The angle in radians, TRIG_ONE-scaled: x / 32768 of π.
    let t = (x * PI_FIX + (1 << 14)) >> 15;
    let t2 = q30(t, t);
    let mut sin = TRIG_ONE;
    let mut k = 11;
    while k > 1 {
        sin = TRIG_ONE - (q30(t2, sin) + k * (k - 1) / 2) / (k * (k - 1));
        k -= 2;
    }
    let mut cos = TRIG_ONE;
    let mut k = 12;
    while k > 0 {
        cos = TRIG_ONE - (q30(t2, cos) + k * (k - 1) / 2) / (k * (k - 1));
        k -= 2;
    }
    (q30(t, sin), cos)
}

/// The sine of `r` in `0..=16384` (one quadrant). Past the octant it is
/// the cosine of the complement, so the two halves of a quadrant are one
/// polynomial read from either end and the quarter turns come out exact:
/// zero is zero and a quarter is [`TRIG_ONE`].
const fn sin_quadrant(r: i64) -> i64 {
    if r <= 8192 {
        octant(r).0
    } else {
        octant(16_384 - r).1
    }
}

/// The cosine of `r` in `0..=16384`: the sine of its complement, which
/// is what makes `cos(θ) == sin(quarter − θ)` an identity in the bits and
/// not just in the arithmetic.
const fn cos_quadrant(r: i64) -> i64 {
    sin_quadrant(16_384 - r)
}

/// A product with a [`TRIG_ONE`]-scaled factor, back to whole units:
/// the nearest, halves away from zero, so a footprint turned half a turn
/// rounds to exactly the negation of the unturned one.
const fn fix(v: i64) -> i32 {
    let half = TRIG_ONE / 2;
    let whole = if v >= 0 {
        (v + half) / TRIG_ONE
    } else {
        -((half - v) / TRIG_ONE)
    };
    whole as i32
}

/// **The one mapping from chart to net**: the net angle a body berthed on
/// chart `surf` at `turn` lays its own across axis at.
///
/// The net is the room unfolded and seen from OUTSIDE, so a chart's own
/// axes are not the room's: a chart's +x can read mirrored from inside,
/// the front chart unfolds downward, and the flanks' courses climb the
/// sheet's x. Everything that needs a footprint asks here — [`Foot::of`],
/// and through it the arbiter, the drop, the light and the rat — and the
/// frontend poses bodies with the same answer, so a drawn body and the
/// ground the sim gave it are one claim (`cabin::pieces::site_on`).
///
/// A net angle is measured the sheet's way: from the sheet's +x toward
/// its +y (which runs DOWN the sheet), in [`Turn`] units. The answer is a
/// base angle per chart plus the turn:
///
/// - **The base** is where the across axis of an upright body lies on
///   that chart's sheet — a viewer's right as they face the surface. On
///   the aft wall that is the sheet's −x (the wall is seen from behind
///   on the sheet); on the front wall, which unfolds downward off the
///   deck's far edge, its +x; on the port flank its +y and on the
///   starboard flank its −y, because the flanks fold out sideways. On
///   the deck an upright body faces the front, which on the sheet is +y,
///   so its across axis lies along −x; under the deckhead, which folds on
///   past the starboard cornice, along +x.
/// - **The handedness** is the same on every chart, and that is why the
///   turn is simply added. The net is one sheet unfolded from one box:
///   every chart shows the sheet the same side out, and the room's inside
///   is the other side of every one of them. Turning from a chart's +x
///   toward its +y is therefore counter-clockwise as seen from inside the
///   room on all six alike — which is the way a [`Turn`] counts — and no
///   chart flips it. (The cabin's charts say the same in 3D: all six
///   normals point out of the room together, `cabin::surface::Station::
///   chart_flipped`.)
#[must_use]
pub const fn net_angle(surf: Surf, turn: Turn) -> Turn {
    let base = match surf {
        Surf::Front | Surf::Ceiling => Turn::ZERO,
        Surf::Port => Turn::QUARTER,
        Surf::Aft | Surf::Floor => Turn::HALF,
        Surf::Starboard => Turn::quarters(3),
    };
    Turn(base.0.wrapping_add(turn.0))
}

/// Half of `cells` cells, in fine units: a footprint's half-extent.
const fn half_of(cells: u8) -> i32 {
    cells as i32 * FINE as i32 / 2
}

/// **An axis-aligned box on a net**, half-open, in [`FINE`] units.
///
/// What a footprint spans on the sheet's own axes ([`Foot::aabb`]): the
/// rect a renderer draws over, a drop clamps into a chart, and the cells
/// a band of wall shadows. For a footprint at a quarter turn it is the
/// footprint exactly; at any other angle it is the box round it, and
/// nothing that asks which ground a piece covers asks this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Aabb {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Aabb {
    /// Width, in fine units.
    #[must_use]
    pub const fn w(self) -> i32 {
        self.x1 - self.x0
    }

    /// Height, in fine units.
    #[must_use]
    pub const fn h(self) -> i32 {
        self.y1 - self.y0
    }

    /// Every net cell the half-open box intersects, row-major.
    pub fn cells(self) -> impl Iterator<Item = (u8, u8)> + use<> {
        let (x0, x1) = (cell_of(self.x0), cell_of(self.x1 - 1));
        let (y0, y1) = (cell_of(self.y0), cell_of(self.y1 - 1));
        (y0..=y1).flat_map(move |cy| (x0..=x1).map(move |cx| (cx, cy)))
    }

    /// The box as a footprint at the sheet's own lie, for asking the
    /// footprint questions of a band of wall or a cell. Every box the sim
    /// builds has even sides, so its centre is a whole unit.
    #[must_use]
    pub const fn foot(self) -> Foot {
        Foot::new(
            (self.x0 + self.x1) / 2,
            (self.y0 + self.y1) / 2,
            (self.w() / 2, self.h() / 2),
            Turn::ZERO,
        )
    }
}

/// **A footprint as ground**: an oriented rectangle on a room's net, in
/// [`FINE`] units.
///
/// The one place the geometry lives, so nothing that asks what a piece
/// covers, where its middle is, or how near it stands to another
/// re-derives it. A footprint is a centre, two half-extents in the body's
/// own frame — whole cells in size, [`Kind::face_on`] — and the net angle
/// its across axis lies at ([`net_angle`]).
///
/// Its two half-axis vectors are rounded to whole units ONCE, the one way
/// ([`TRIG_ONE`]), and its corners are the centre plus or minus each of
/// them: everything after that is exact integer geometry on those
/// corners, products in `i64` and `i128` wherever they need it. At a
/// quarter turn there is nothing to round — the trig is exact there — so
/// an axis-aligned footprint is exactly the rectangle the grid always
/// drew, and at any other angle its corners are within a unit of true.
///
/// **Its own frame**: `x` across the body, its right as a person facing
/// it sees it; `y` a quarter turn short of that, which is down its face
/// on a wall, and the same quarter laid flat on a deck or a deckhead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foot {
    /// The centre, in fine units of the net.
    pub x: i32,
    pub y: i32,
    /// Half the footprint across its own frame and down it, fine units.
    pub half: (i32, i32),
    /// The net angle its across axis lies at.
    pub angle: Turn,
    /// The half-axis vectors, rounded once: along `angle` and a quarter
    /// short of it.
    across: (i32, i32),
    down: (i32, i32),
}

/// `a · b`, wide.
const fn dot(a: (i64, i64), b: (i64, i64)) -> i64 {
    a.0 * b.0 + a.1 * b.1
}

/// `a × b`, wide: the signed area the two span.
const fn cross(a: (i64, i64), b: (i64, i64)) -> i64 {
    a.0 * b.1 - a.1 * b.0
}

/// A pair, widened.
const fn wide((x, y): (i32, i32)) -> (i64, i64) {
    (x as i64, y as i64)
}

/// The squared length of the shortest way from `p` to the segment
/// `q0 .. q1`, against `r` squared: whether it is under `r`, exactly.
/// The perpendicular case compares a cross product squared with `r²`
/// times the segment's length squared, so nothing is ever divided.
fn segment_within(point: (i64, i64), from: (i64, i64), to: (i64, i64), reach: i64) -> bool {
    let edge = (to.0 - from.0, to.1 - from.1);
    let off = (point.0 - from.0, point.1 - from.1);
    let along = dot(off, edge);
    let length = dot(edge, edge);
    let limit = i128::from(reach) * i128::from(reach);
    if along <= 0 {
        i128::from(dot(off, off)) < limit
    } else if along >= length {
        let past = (point.0 - to.0, point.1 - to.1);
        i128::from(dot(past, past)) < limit
    } else {
        let side = i128::from(cross(edge, off));
        side * side < limit * i128::from(length)
    }
}

/// **Where a point lies in a footprint's own frame**, exactly
/// ([`Foot::frame`]).
///
/// Its two coordinates along the footprint's own axes, as fractions of
/// the half-extents: `across / whole` and `down / whole`, −1 at the
/// footprint's left edge and its top and +1 at its right and its foot,
/// as a person facing it sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Framed {
    across: i128,
    down: i128,
    whole: i128,
}

impl Framed {
    /// **Whether the point is on the footprint**: half-open, its left
    /// edge and its top in and its right edge and its foot out, so two
    /// footprints standing flush never both answer for the seam between
    /// them.
    #[must_use]
    pub const fn inside(self) -> bool {
        -self.whole <= self.across
            && self.across < self.whole
            && -self.whole <= self.down
            && self.down < self.whole
    }

    /// The point as fractions of the footprint's own face, `(0, 0)` at its
    /// top left and `(1, 1)` at its bottom right — for a frontend
    /// measuring a sub-rect it declared in the piece's own units (a carry
    /// handle) against the point, which no rule reads.
    #[must_use]
    pub fn fractions(self) -> (f32, f32) {
        let whole = self.whole as f64;
        (
            f64::midpoint(self.across as f64 / whole, 1.0) as f32,
            f64::midpoint(self.down as f64 / whole, 1.0) as f32,
        )
    }
}

impl Foot {
    /// The footprint centred on fine `(x, y)` with half-extents `half` in
    /// its own frame and its across axis at net angle `angle`.
    #[must_use]
    pub const fn new(x: i32, y: i32, half: (i32, i32), angle: Turn) -> Self {
        let (cos, sin) = (angle.cos(), angle.sin());
        let (hw, hh) = (half.0 as i64, half.1 as i64);
        Self {
            x,
            y,
            half,
            angle,
            across: (fix(hw * cos), fix(hw * sin)),
            down: (fix(hh * sin), fix(-hh * cos)),
        }
    }

    /// **The footprint `kind` takes centred on fine `(x, y)` of `host`'s
    /// net at `turn`**, or `None` where that centre is on no chart.
    ///
    /// The chart is the one under the centre ([`Foot::centre_cell`]), and
    /// it decides both what the footprint spends ([`Kind::face_on`]) and
    /// which way it lies on the sheet ([`net_angle`]): a wardrobe covers
    /// one cell of deck or two courses of wall, and which it is doing is
    /// the chart's answer, not the kind's.
    #[must_use]
    pub const fn of(host: RoomKind, kind: Kind, x: u16, y: u16, turn: Turn) -> Option<Self> {
        let (cx, cy) = (cell_of(x as i32 - 1), cell_of(y as i32 - 1));
        match host.surface_of(cx, cy) {
            Some(surf) => Some(Self::planned(kind, surf, (x as i32, y as i32), turn)),
            None => None,
        }
    }

    /// The footprint `kind` takes on a chart of class `surf`, centred on
    /// fine `centre` at `turn` — [`Foot::of`] with the chart already
    /// named, for a caller that has one in hand (the drop, which plans a
    /// carry against the chart under the pointer before it knows where
    /// the centre will settle).
    #[must_use]
    pub const fn planned(kind: Kind, surf: Surf, centre: (i32, i32), turn: Turn) -> Self {
        let (across, span) = kind.face_on(surf);
        Self::new(
            centre.0,
            centre.1,
            (half_of(across), half_of(span)),
            net_angle(surf, turn),
        )
    }

    /// Where `piece` stands: its room and its footprint. A berth off its
    /// room's net covers nothing.
    #[must_use]
    pub fn at(rooms: &Rooms, piece: &Piece) -> Option<(RoomId, Self)> {
        let spot = piece.loc.spot();
        let host = rooms.kind(spot.room)?;
        Some((
            spot.room,
            Self::of(host, piece.kind, spot.x, spot.y, spot.turn)?,
        ))
    }

    /// Net cell `(x, y)` as a footprint: what a question about one cell
    /// (the rat's, the light's) asks with.
    #[must_use]
    pub const fn cell(x: u8, y: u8) -> Self {
        let half = FINE as i32 / 2;
        Self::new(
            fine(x) as i32 + half,
            fine(y) as i32 + half,
            (half, half),
            Turn::ZERO,
        )
    }

    /// The four corners, in order round the footprint.
    #[must_use]
    pub const fn corners(self) -> [(i32, i32); 4] {
        let (a, b) = (self.across, self.down);
        [
            (self.x + a.0 + b.0, self.y + a.1 + b.1),
            (self.x - a.0 + b.0, self.y - a.1 + b.1),
            (self.x - a.0 - b.0, self.y - a.1 - b.1),
            (self.x + a.0 - b.0, self.y + a.1 - b.1),
        ]
    }

    /// The box round the footprint on the sheet's own axes. Exactly the
    /// footprint at a quarter turn.
    #[must_use]
    pub const fn aabb(self) -> Aabb {
        let ex = self.across.0.abs() + self.down.0.abs();
        let ey = self.across.1.abs() + self.down.1.abs();
        Aabb {
            x0: self.x - ex,
            y0: self.y - ey,
            x1: self.x + ex,
            y1: self.y + ey,
        }
    }

    /// **The cell under the footprint's centre**: the one every rule
    /// that asks "what tile does this piece stand on" reads.
    ///
    /// Exactly, the cell holding the fine unit just short of the centre on
    /// each axis, so a centre lying on a seam reads the cell above and to
    /// the left of it. That tie-break is chosen for what it keeps: at any
    /// whole-cell berth, every kind the game has (none is wider or taller
    /// than two cells) reads the cell its footprint's top-left corner is
    /// in, which is the cell the grid's rules always read.
    #[must_use]
    pub const fn centre_cell(self) -> (u8, u8) {
        (cell_of(self.x - 1), cell_of(self.y - 1))
    }

    /// The chart this footprint is planned against: the one under its
    /// centre. The arbiter insists it lie wholly in that chart anyway.
    #[must_use]
    pub const fn chart(self, host: RoomKind) -> Option<Surf> {
        let (cx, cy) = self.centre_cell();
        host.surface_of(cx, cy)
    }

    /// Every net cell whose square the footprint's INTERIOR intersects,
    /// row-major. A footprint a unit off the grid covers a row and a
    /// column more than its size, because it is standing on them; a
    /// corner merely touching a cell does not stand on it.
    pub fn cells(self) -> impl Iterator<Item = (u8, u8)> + use<> {
        let exact = self.angle.square();
        self.aabb()
            .cells()
            .filter(move |&(cx, cy)| exact || self.overlaps(Self::cell(cx, cy)))
    }

    /// The footprint's two edge normals: each half-axis turned a quarter.
    const fn normals(self) -> [(i64, i64); 2] {
        [
            (-(self.across.1 as i64), self.across.0 as i64),
            (-(self.down.1 as i64), self.down.0 as i64),
        ]
    }

    /// How far the footprint reaches either side of its centre along
    /// `n`, in units of `n`'s own length.
    const fn reach(self, n: (i64, i64)) -> i64 {
        dot(wide(self.across), n).abs() + dot(wide(self.down), n).abs()
    }

    /// The separating-axis reading both overlap questions share: on each
    /// edge normal of either footprint, how far apart the two centres lie
    /// against how far the two reach. `Less` on every axis is ground the
    /// two share; `Equal` at worst is two footprints touching; `Greater`
    /// on any axis is daylight between them.
    fn worst_axis(self, other: Self) -> std::cmp::Ordering {
        let apart = (i64::from(other.x - self.x), i64::from(other.y - self.y));
        self.normals()
            .into_iter()
            .chain(other.normals())
            .map(|n| dot(apart, n).abs().cmp(&(self.reach(n) + other.reach(n))))
            .max()
            .unwrap_or(std::cmp::Ordering::Less)
    }

    /// **Whether the two footprints share ground.** Touching does not —
    /// two pieces stand flush along an edge or corner to corner at any
    /// angle without sharing any — and any intersection of positive area
    /// does. No rule refuses either now; the game's own tidiness asks it
    /// ([`clear`]).
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self.worst_axis(other) == std::cmp::Ordering::Less
    }

    /// Whether the point `p` lies on the footprint, edges included.
    #[must_use]
    pub const fn contains(self, p: (i32, i32)) -> bool {
        let d = (p.0 as i64 - self.x as i64, p.1 as i64 - self.y as i64);
        let whole = cross(wide(self.across), wide(self.down)).abs();
        cross(d, wide(self.down)).abs() <= whole && cross(wide(self.across), d).abs() <= whole
    }

    /// Whether `other` lies wholly on this footprint, edges included.
    #[must_use]
    pub const fn holds(self, other: Self) -> bool {
        let [a, b, c, d] = other.corners();
        self.contains(a) && self.contains(b) && self.contains(c) && self.contains(d)
    }

    /// **Where the point `p` lies in this footprint's own frame**, exactly.
    ///
    /// `p` is in `1 / per` of a fine unit: a pointer is read finer than a
    /// berth is placed (`layout::net_point`), because a reading taken a
    /// hair inside a piece's rim must stay inside it. The answer is
    /// rational and its parts are integers, so which piece and which part
    /// of it a press lands on is the same on every machine a crew plays on.
    #[must_use]
    pub const fn frame(self, p: (i64, i64), per: i64) -> Framed {
        let d = (p.0 - self.x as i64 * per, p.1 - self.y as i64 * per);
        let (a, b) = (wide(self.across), wide(self.down));
        let area = cross(a, b);
        let sign: i128 = if area < 0 { -1 } else { 1 };
        // d = s·a + t·b, so s = (d × b) / (a × b) and t = (a × d) / (a × b).
        let (ax, ay, bx, by) = (a.0 as i128, a.1 as i128, b.0 as i128, b.1 as i128);
        let (dx, dy) = (d.0 as i128, d.1 as i128);
        Framed {
            across: sign * (dx * by - dy * bx),
            down: sign * (ax * dy - ay * dx),
            whole: sign * area as i128 * per as i128,
        }
    }

    /// **Whether the shortest way between the two footprints is under
    /// `r` fine units**, Euclidean and exact: squares compared with
    /// squares, rationals cross-multiplied, nothing ever rooted. Ground
    /// they share or a seam they touch along is no distance at all.
    ///
    /// Euclidean because the pieces turn. A Chebyshev buffer is a square,
    /// and a rule that is a square depends on which way the room is
    /// turned: two canisters a hand apart corner to corner would be legal
    /// at one angle and refused at the next.
    #[must_use]
    pub fn clearance_below(self, other: Self, r: i32) -> bool {
        if self.worst_axis(other) != std::cmp::Ordering::Greater {
            return true;
        }
        // Daylight between the boxes round them settles it cheaply.
        let (a, b) = (self.aabb(), other.aabb());
        if a.x0 - b.x1 >= r || b.x0 - a.x1 >= r || a.y0 - b.y1 >= r || b.y0 - a.y1 >= r {
            return false;
        }
        // Two convex shapes with daylight between them are nearest at a
        // corner of one and an edge of the other.
        let near = |from: Self, to: Self| {
            let edges = to.corners().map(wide);
            from.corners().into_iter().map(wide).any(|p| {
                (0..4).any(|i| segment_within(p, edges[i], edges[(i + 1) % 4], i64::from(r)))
            })
        };
        near(self, other) || near(other, self)
    }
}

/// **The tile a berth stands on**: the class of the cell under its
/// footprint's centre ([`Foot::centre_cell`]), or `None` for a berth with
/// no footprint.
///
/// Every rule that asks what a piece is standing on — ownership,
/// [`staying`], the offer and the fire, the launch gate — asks it
/// here, once, so a piece half on the chalk and half off it is on
/// whichever its middle is on and nowhere else. The classes that refuse
/// cargo outright do not ask this: they refuse a footprint that covers
/// any cell of theirs (the arbiter's threshold and fixture rungs, and
/// the drop's own gate).
#[must_use]
pub fn berth_tile(rooms: &Rooms, kind: Kind, loc: Loc) -> Option<Tile> {
    let spot = loc.spot();
    let (cx, cy) = Foot::of(rooms.kind(spot.room)?, kind, spot.x, spot.y, spot.turn)?.centre_cell();
    rooms.tile(spot.room, cx, cy)
}

/// Whether a `kind` at `loc` belongs to the player rather than to the
/// room it stands in.
///
/// **This is THE ownership rule** and it is a function of tile class, not
/// of a hand-written list of berths (docs/ROOMS.md): a berth on a `Stock`
/// tile is the room's own goods; every other berth is the player's — a
/// proposal on an `Offer` tile stays the player's until a resolution says
/// otherwise, and a piece staged on the incinerator's `Consume` tiles is
/// still the player's right up until the stoker takes it. The drop
/// matrix, the [`crate::sim::Sim::drop_targets`] affordances, and any
/// renderer hint all derive from this one predicate. Never restate it.
///
/// The tile is the one under the footprint's centre ([`berth_tile`]).
#[must_use]
pub fn player_owned(rooms: &Rooms, kind: Kind, loc: Loc) -> bool {
    berth_tile(rooms, kind, loc) != Some(Tile::Stock)
}

/// One cargo piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Stable identity, never reused within a run.
    pub id: u32,
    pub kind: Kind,
    /// Visual flavour roll, for the renderer to vary sprites with.
    pub variant: u8,
    /// A rat has been at it: permanently bitten (see `rats`), worth a
    /// little less at every station (see `barter::GNAW_MALUS`), rendered
    /// with a notch, and otherwise a perfectly ordinary piece — it stands,
    /// trades, and resells like anything else.
    pub gnawed: bool,
    pub loc: Loc,
}

/// Which placement rule refused a placement. One variant per rule, so the
/// renderer can flash the matching icon on a hard reject.
///
/// Every one of them is about the room or about a special item's own
/// condition. None is about one piece's body meeting another's: that
/// was `Overlap`, with the standing shadow, the one dressing per point
/// and the pinned rule beside it, and they went together when placement
/// became a matter of taste (docs/BAY.md, "Cargo stops colliding").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// The footprint leaves the net, crosses a hole, or bends over a
    /// fold — every placement lies wholly in one chart.
    Bounds,
    /// Two volatile pieces with less than half a cell of clear air between
    /// them, measured straight across (fold seams count: the baseboard is
    /// next to the floor in the room, so it is here). The one rule left
    /// that sets one piece against another, and it stays because it is a
    /// special item's own condition rather than a question of who stands
    /// where: two canisters a hand apart are a hazard, not a clip.
    Volatile,
    /// A cryo piece that does not reach the floor's hull edge.
    Cryo,
    /// A second suspicious piece aboard.
    Suspicious,
    /// A placement whose chart does not satisfy the kind's mount.
    Affix(Mount),
    /// The last vital instrument aboard offered to an exit ceremony —
    /// a calling room's offer area, the incinerator, the casino — a ship
    /// that cannot chart or launch is a soft-lock, so the last of each
    /// stays.
    Vital,
    /// An aperture's footprint was asked to hold cargo. A threshold
    /// belongs to two rooms at once, so nothing berths there: the
    /// doorway stays clear because it is shared space.
    Threshold,
    /// A cell the room's own hardware already fills was asked to hold
    /// cargo. The counter's deck and the pendant's ceiling are the
    /// room's, not the net's (`RoomKind::fixture`).
    Fixture,
}

/// **The turn the game gives a `kind` it places itself, centred on fine
/// `centre` of chart `surf` of `host`'s net.**
///
/// Default facing is procedural and nothing more (docs/BAY.md, "Cargo
/// turns"): the starting board, a station's stock and furniture,
/// [`first_fit`] and [`dress_fit`] — and through them salvage, the comet's
/// ice, the exchange, banking the hopper — fluff budding, and the
/// frontend's fixture boards all take it. A player's drop NEVER does: a
/// carried piece keeps the turn it was carried at, because a rule that
/// re-faced a couch the player had just set down would be the sim
/// overruling a hand.
///
/// It is the backing rule, and it used to be the frontend's, decided
/// where a body was drawn rather than where it was ruled: a body
/// standing within half a cell of a seam of its deck turns its back to
/// that wall — the couch against the wall — and anywhere else faces the
/// front. The aft seam is asked first, then the front, then the flanks,
/// and a flank only turns a body whose plan is one cell across, which is
/// the turn that does not move a footprint off its own cells. "Half a
/// cell or less" is a rule and not a rounding allowance: a body that
/// faces a seam keeps more than half a cell of deck in front of its
/// face. The deckhead takes the same rule,
/// because a pendant hung on the front row looking into the front wall a
/// hand's breadth away is the couch defect stood on its head. A wall
/// takes `Turn(0)`, its upright frame: up is up.
///
/// On both charts a body stands on, `Turn(0)` faces the sheet's +y
/// ([`net_angle`]) and the plan at `Turn(0)` lies along the sheet's own
/// axes, so one reading in sheet terms serves the deck and the deckhead
/// alike: away from the near edge, every time.
#[must_use]
pub fn default_turn(host: RoomKind, kind: Kind, surf: Surf, centre: (u16, u16)) -> Turn {
    if !matches!(surf, Surf::Floor | Surf::Ceiling) {
        return Turn::ZERO;
    }
    let (across, deep) = kind.face_on(surf);
    let (hw, hh) = (half_of(across), half_of(deep));
    let (cx, cy, cw, ch) = host.chart_rect(surf);
    let (left, top) = (i32::from(fine(cx)), i32::from(fine(cy)));
    let (right, bottom) = (left + i32::from(fine(cw)), top + i32::from(fine(ch)));
    let (x, y) = (i32::from(centre.0), i32::from(centre.1));
    let seam = i32::from(FINE / 2);
    let one_column = across == 1;
    if y - hh - top <= seam {
        Turn::ZERO
    } else if bottom - (y + hh) <= seam {
        Turn::HALF
    } else if x - hw - left <= seam && one_column {
        // Facing the sheet's +x: three quarters round from facing +y.
        Turn::quarters(3)
    } else if right - (x + hw) <= seam && one_column {
        Turn::QUARTER
    } else {
        Turn::ZERO
    }
}

/// The half-extents of the box round `kind` on chart `surf` at a quarter
/// turn `turn`, on the sheet's own axes: its own two extents, or the two
/// transposed when the turn lays it across the sheet's other axis.
const fn square_reach(kind: Kind, surf: Surf, turn: Turn) -> (i32, i32) {
    let (across, span) = kind.face_on(surf);
    let (hw, hh) = (half_of(across), half_of(span));
    if net_angle(surf, turn).0 & Turn::QUARTER.0 == 0 {
        (hw, hh)
    } else {
        (hh, hw)
    }
}

/// **The berth the game gives `kind` with its footprint's top-left at
/// fine `(x, y)` of `host`'s net**: the centre that puts it there and the
/// turn [`default_turn`] gives it.
///
/// Whole-cell-aligned berths are how the game sets things out — a
/// shelf's stock, a derelict's salvage, the starting board, a fitting
/// scan — and a centre is not a whole cell, so this is where the one
/// becomes the other. `None` where that corner is on no chart, or where
/// the centre is off the corner's: the footprint is planned against the
/// corner's own chart, and a centre that lands on another chart is no
/// berth of this one — a wall body two courses tall set down by the
/// deck's edge would put its middle on the deck, where it is a different
/// body. The backing rule turns only a body whose plan is one cell
/// across, which no quarter turn moves off its corner.
#[must_use]
pub fn anchored(host: RoomKind, kind: Kind, x: u16, y: u16) -> Option<(u16, u16, Turn)> {
    let surf = host.surface_of(coarse(x), coarse(y))?;
    let (hw, hh) = square_reach(kind, surf, Turn::ZERO);
    let centre = (
        u16::try_from(i32::from(x) + hw).ok()?,
        u16::try_from(i32::from(y) + hh).ok()?,
    );
    centred_on(host, surf, centre)
        .then(|| (centre.0, centre.1, default_turn(host, kind, surf, centre)))
}

/// Whether fine `centre` of `host`'s net lies on chart `surf`, read the
/// way [`Foot::of`] reads the chart a footprint is planned on.
fn centred_on(host: RoomKind, surf: Surf, centre: (u16, u16)) -> bool {
    let middle = (
        cell_of(i32::from(centre.0) - 1),
        cell_of(i32::from(centre.1) - 1),
    );
    host.surface_of(middle.0, middle.1) == Some(surf)
}

/// Whether `kind` may stand at `spot`.
///
/// Judged against the room, and against every other piece in `pieces`
/// only where a special item's own condition names one (volatile
/// spacing, one suspicious piece aboard). The piece with `id` is
/// ignored, so a held piece never answers for its own old berth.
#[must_use]
pub fn placement_legal(rooms: &Rooms, pieces: &[Piece], id: u32, kind: Kind, spot: Spot) -> bool {
    placement_check(rooms, pieces, id, kind, spot).is_ok()
}

/// [`placement_legal`], but naming the rule that refused.
///
/// Checks run in a fixed order (bounds/chart, threshold, fixture, mount,
/// cryo, then per-piece suspicious / volatile in board order) so the
/// reported violation is deterministic. Every rung is integer arithmetic
/// over oriented footprints ([`Foot`]): no floating point reaches the
/// arbiter, so a crew in lockstep agrees on every ruling at every angle.
///
/// **No rung asks whether this body meets another one** (docs/BAY.md,
/// "Cargo stops colliding"). A crate may stand in a wardrobe, a painting
/// may hang behind one, and two crates may share a deck cell: placement
/// is to taste, and the sim keeps no opinion about it. What a rung asks
/// about is the room — its charts, its doorways, its own hardware, the
/// mount a kind needs and the hull a cryo piece needs — and the two
/// special items that answer to each other wherever they stand. Nothing
/// here reasons about where a body may walk either: the walker passes
/// through cargo.
pub fn placement_check(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    spot: Spot,
) -> Result<(), Violation> {
    let Some(host) = rooms.kind(spot.room) else {
        return Err(Violation::Bounds);
    };
    // The CENTRE's chart first, because the footprint is a function of
    // it: a wardrobe covers one cell of deck and two of wall, and which
    // it is doing here is the chart's answer, not the kind's.
    let Some(foot) = Foot::of(host, kind, spot.x, spot.y, spot.turn) else {
        return Err(Violation::Bounds);
    };
    let Some(surf) = footprint_surface(host, foot) else {
        return Err(Violation::Bounds);
    };
    if footprint_tiles(host, foot).any(|tile| tile == Tile::Threshold) {
        return Err(Violation::Threshold);
    }
    if footprint_tiles(host, foot).any(|tile| tile == Tile::Fixture) {
        return Err(Violation::Fixture);
    }
    let mount = kind.mount();
    if !mount_accepts(mount, surf) {
        return Err(Violation::Affix(mount));
    }
    if matches!(kind.tag(), Some(Tag::Cryo)) && !near_hull(host, foot) {
        return Err(Violation::Cryo);
    }
    let suspicious_here = matches!(kind.tag(), Some(Tag::Suspicious)) && rooms.riding(spot.room);
    let volatile = matches!(kind.tag(), Some(Tag::Volatile));
    if !suspicious_here && !volatile {
        return Ok(());
    }
    for other in pieces {
        if other.id == id || !matches!(other.loc, Loc::Hold { .. }) {
            continue;
        }
        let at = other.loc.room();
        // At most one suspicious piece rides the ship, wherever aboard.
        if suspicious_here && matches!(other.kind.tag(), Some(Tag::Suspicious)) && rooms.riding(at)
        {
            return Err(Violation::Suspicious);
        }
        // Two volatile pieces keep half a cell of clear air between them,
        // straight across at whatever angle either stands. Without a
        // buffer a unit of daylight would satisfy "not touching", and the
        // rule would be a formality. **The air is measured in plan**, at
        // whatever lift either stands: a lift is taste and no rule reads
        // it (docs/BAY.md, "Lift"), so a canister raised over another is
        // as near it as the ground under the two says.
        if volatile
            && at == spot.room
            && matches!(other.kind.tag(), Some(Tag::Volatile))
            && Foot::at(rooms, other)
                .is_some_and(|(_, theirs)| foot.clearance_below(theirs, i32::from(FINE / 2)))
        {
            return Err(Violation::Volatile);
        }
    }
    Ok(())
}

/// **The arbiter for the layer `berth` lies in**: [`placement_check`] for
/// a piece standing in a room, [`dressing_check`] for a covering laid
/// into one.
///
/// The drop and [`tidy`] both ask this, so which of the two answers is
/// decided once. A covering stands, packed, wherever a room sets one out
/// on a shelf or an offer, so the layer is the berth's and not the
/// kind's.
pub fn berth_check(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    berth: Loc,
) -> Result<(), Violation> {
    match berth {
        Loc::Hold { .. } => placement_check(rooms, pieces, id, kind, berth.spot()),
        Loc::Laid { .. } => dressing_check(rooms, kind, berth.spot()),
    }
}

/// The chart a footprint lies wholly inside, if any — a piece bent over a
/// fold, crossing a hole, or leaving the net is nowhere. Every corner
/// lies in the chart's rect (edges included: a footprint flush with a
/// fold is on its chart), and no cell its interior covers is a hole.
fn footprint_surface(host: RoomKind, foot: Foot) -> Option<Surf> {
    let chart = foot.chart(host)?;
    let (cx, cy, cw, ch) = host.chart_rect(chart);
    let (left, top) = (i32::from(fine(cx)), i32::from(fine(cy)));
    let (right, bottom) = (left + i32::from(fine(cw)), top + i32::from(fine(ch)));
    let inside = foot
        .corners()
        .iter()
        .all(|&(x, y)| (left..=right).contains(&x) && (top..=bottom).contains(&y));
    (inside
        && foot
            .cells()
            .all(|(x, y)| host.surface_of(x, y) == Some(chart)))
    .then_some(chart)
}

/// Every tile class a footprint covers, any part of a cell's interior
/// counting.
fn footprint_tiles(host: RoomKind, foot: Foot) -> impl Iterator<Item = Tile> + use<> {
    foot.cells()
        .filter_map(move |(cx, cy)| host.tile_of(cx, cy))
}

/// The floor chart as a fine box.
const fn floor_box(host: RoomKind) -> Aabb {
    let (fx, fy, fw, fh) = host.floor_rect();
    Aabb {
        x0: fine(fx) as i32,
        y0: fine(fy) as i32,
        x1: fine(fx + fw) as i32,
        y1: fine(fy + fh) as i32,
    }
}

/// **How near the floor's hull edge a cryo piece must reach**: a
/// sixteenth of a cell (about 3 cm), in fine units.
///
/// The cold is in the plating, so the rule is "on the hull edge", and it
/// used to be exactly that — flush, to the unit. A hand is not exact,
/// and a body turned a degree off square touches a wall at one corner a
/// hair later than its neighbour does, so the rule allows the hair: an
/// edge within a sixteenth of the hull is on it. More would be cheating
/// the plating — the drop's wall snap slides a footprint flush from an
/// eighth of a cell out (`Sim::drop_preview`), so a deliberate placement
/// never needs the allowance at all.
pub const HULL_TOUCH: i32 = FINE as i32 / 16;

/// Whether a floor footprint reaches the floor's hull edge (any side of
/// the floor chart — every side of a room is hull), within
/// [`HULL_TOUCH`]. A convex footprint is nearest a straight edge at one
/// of its corners, so the box round it says exactly how near.
const fn near_hull(host: RoomKind, foot: Foot) -> bool {
    let floor = floor_box(host);
    let span = foot.aabb();
    span.x0 - floor.x0 <= HULL_TOUCH
        || span.y0 - floor.y0 <= HULL_TOUCH
        || floor.x1 - span.x1 <= HULL_TOUCH
        || floor.y1 - span.y1 <= HULL_TOUCH
}

/// The wall a standing floor footprint shadows: for each floor edge the
/// piece comes within a cell of, the band of wall directly behind it — as
/// wide as the piece's shadow on that seam, rising from the baseboard
/// through the piece's stature ([`COURSES`] at most, which is all the
/// wall there is). A piece a whole cell or more off the wall leaves room
/// to hang something behind it.
///
/// No rule reads it any more: a player may hang a painting behind their
/// own wardrobe. It is half of what a standing body occupies, and so
/// half of what the game's own tidiness steers clear of ([`clear`]).
///
/// A footprint is nearest a seam at a corner and its shadow on the seam
/// is the span of its corners along it, so both are read off the box
/// round it — exactly, at any angle.
fn shadows(host: RoomKind, foot: Foot, stature: u8) -> Vec<Foot> {
    let floor = floor_box(host);
    let span = foot.aabb();
    let rise = i32::from(fine(stature.min(COURSES)));
    let cell = i32::from(FINE);
    let mut walls = Vec::new();
    if rise == 0 {
        return Vec::new();
    }
    if span.y0 - floor.y0 < cell {
        // The aft wall's baseboard row sits just above the floor's aft edge.
        walls.push(Aabb {
            x0: span.x0,
            y0: floor.y0 - rise,
            x1: span.x1,
            y1: floor.y0,
        });
    }
    if floor.y1 - span.y1 < cell {
        walls.push(Aabb {
            x0: span.x0,
            y0: floor.y1,
            x1: span.x1,
            y1: floor.y1 + rise,
        });
    }
    if span.x0 - floor.x0 < cell {
        walls.push(Aabb {
            x0: floor.x0 - rise,
            y0: span.y0,
            x1: floor.x0,
            y1: span.y1,
        });
    }
    if floor.x1 - span.x1 < cell {
        walls.push(Aabb {
            x0: floor.x1,
            y0: span.y0,
            x1: floor.x1 + rise,
            y1: span.y1,
        });
    }
    walls.into_iter().map(Aabb::foot).collect()
}

/// **The corners a fitting scan sets footprints down at in `room`**,
/// sorted `(y, x)`.
///
/// Every whole-cell corner, plus every corner flush against the right or
/// bottom edge of the box round some other piece already standing or
/// laid in the room. The second set is what finds a snug fit between
/// pieces that do not sit on the grid, without scanning every unit of
/// every cell — which would be 65,536 times the work to find, nearly
/// always, the same berth. A chart's own far edges need no corners of
/// their own: a chart is whole cells and so is every footprint at a
/// quarter turn, so the corner flush against one is already a whole-cell
/// corner.
fn anchors(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    room: RoomId,
    host: RoomKind,
) -> Vec<(u16, u16)> {
    let (cols, rows) = host.grid();
    let mut xs: Vec<u16> = (0..cols).map(fine).collect();
    let mut ys: Vec<u16> = (0..rows).map(fine).collect();
    for other in pieces {
        if other.id == id {
            continue;
        }
        if let Some((at, foot)) = Foot::at(rooms, other) {
            if at == room {
                let span = foot.aabb();
                xs.extend(u16::try_from(span.x1));
                ys.extend(u16::try_from(span.y1));
            }
        }
    }
    for axis in [&mut xs, &mut ys] {
        axis.sort_unstable();
        axis.dedup();
    }
    xs.retain(|&x| x < fine(cols));
    ys.retain(|&y| y < fine(rows));
    ys.iter()
        .flat_map(|&y| xs.iter().map(move |&x| (x, y)))
        .collect()
}

/// **The spots a fitting scan offers `kind`, in the order it offers
/// them**, for one pass: every [`anchors`] corner of every riding room, in
/// room-id order, with the footprint's top-left set down there.
///
/// Pass 0 is the turn the game gives a body there ([`default_turn`]);
/// pass 1 is a quarter turn on from it. Those are all the passes there
/// are, because the arbiter reads a footprint and never the way a body
/// faces on it: half a turn lays the very same ground down again, so the
/// other two quarters could only be refused for what the first two were.
/// A body square on its chart has one shape at every quarter and gets
/// pass 0 alone.
fn fitting_spots(rooms: &Rooms, pieces: &[Piece], id: u32, kind: Kind, pass: u8) -> Vec<Spot> {
    let mut spots = Vec::new();
    for (room, host) in rooms.iter() {
        if !host.kind.riding() {
            continue;
        }
        for (x, y) in anchors(rooms, pieces, id, room, host.kind) {
            let Some((cx, cy, turn)) = anchored(host.kind, kind, x, y) else {
                continue;
            };
            if pass == 0 {
                spots.push(Spot {
                    room,
                    x: cx,
                    y: cy,
                    turn,
                });
                continue;
            }
            let Some(surf) = host.kind.surface_of(coarse(x), coarse(y)) else {
                continue;
            };
            let (across, span) = kind.face_on(surf);
            if across == span {
                continue;
            }
            let turn = turn + Turn::QUARTER;
            let (hw, hh) = square_reach(kind, surf, turn);
            let (Ok(cx), Ok(cy)) = (
                u16::try_from(i32::from(x) + hw),
                u16::try_from(i32::from(y) + hh),
            ) else {
                continue;
            };
            if !centred_on(host.kind, surf, (cx, cy)) {
                continue;
            }
            spots.push(Spot {
                room,
                x: cx,
                y: cy,
                turn,
            });
        }
    }
    spots
}

/// Every spot a fitting scan offers `kind`, in the order it offers them:
/// all of [`fitting_spots`]' first pass, then all of its second, so a
/// body is turned onto its side only when nothing aboard takes it the way
/// the game would stand it. Built a pass at a time, as it is asked for.
fn fitting_order<'a>(
    rooms: &'a Rooms,
    pieces: &'a [Piece],
    id: u32,
    kind: Kind,
) -> impl Iterator<Item = Spot> + 'a {
    (0..2).flat_map(move |pass| fitting_spots(rooms, pieces, id, kind, pass))
}

/// **The first berth aboard for `kind`, as the game would choose it.**
///
/// Riding rooms only, because "aboard" means the part of the ship that
/// leaves with you; then [`fitting_spots`]' order — every room at the
/// turn the game would give a body there before any room at the turn
/// after it — and [`tidy`]'s preference across all of it: the first spot
/// standing clear of everything already there, and only when there is
/// none, the first spot the arbiter allows at all.
///
/// Shared by the shift-click quick-move, the comet harvest, the ???
/// exchange, and the hopper's banking, so all of them agree on what
/// "first" means. Coverings have no occupancy berth at all ([`dress_fit`]
/// is their scan).
#[must_use]
pub fn first_fit(rooms: &Rooms, pieces: &[Piece], id: u32, kind: Kind) -> Option<Spot> {
    if kind.covering() {
        return None;
    }
    let order = fitting_order(rooms, pieces, id, kind).map(Spot::hold);
    tidy(rooms, pieces, id, kind, order).map(Loc::spot)
}

/// Whether covering `kind` may be laid at `spot`.
///
/// The dressing layer's own [`placement_check`], reusing the violation
/// ladder, in a fixed order (bounds, threshold, fixture, surface) so the
/// reported violation is deterministic. Every rung is about the room: a
/// dressing coexists with whatever stands on it and with any other
/// dressing it is laid over, so no other piece is consulted at all —
/// which is why none is passed.
pub fn dressing_check(rooms: &Rooms, kind: Kind, spot: Spot) -> Result<(), Violation> {
    debug_assert!(kind.covering(), "dressing_check is for coverings only");
    let Some(host) = rooms.kind(spot.room) else {
        return Err(Violation::Bounds);
    };
    let Some(foot) = Foot::of(host, kind, spot.x, spot.y, spot.turn) else {
        return Err(Violation::Bounds);
    };
    // Wholly on one chart — a rug bent over a fold is not a rug anyone
    // respects, and a coat cannot paint across a hole.
    let Some(surf) = footprint_surface(host, foot) else {
        return Err(Violation::Bounds);
    };
    if footprint_tiles(host, foot).any(|tile| tile == Tile::Threshold) {
        return Err(Violation::Threshold);
    }
    if footprint_tiles(host, foot).any(|tile| tile == Tile::Fixture) {
        return Err(Violation::Fixture);
    }
    if let Some(Tag::Covering(Some(mount))) = kind.tag() {
        // A restricted covering names WHICH chart class it covers; the
        // footprint-surface rule above already made it whole.
        if !mount_accepts(mount, surf) {
            return Err(Violation::Affix(mount));
        }
    }
    Ok(())
}

/// The first spot aboard where covering `kind` may be laid — the
/// dressing layer's [`first_fit`], offering the same spots in the same
/// order with the same preference for free ground.
#[must_use]
pub fn dress_fit(rooms: &Rooms, pieces: &[Piece], id: u32, kind: Kind) -> Option<Spot> {
    if !kind.covering() {
        return None;
    }
    let order = fitting_order(rooms, pieces, id, kind).map(Spot::laid);
    tidy(rooms, pieces, id, kind, order).map(Loc::spot)
}

/// **Whether `kind` in `berth` would stand clear of every other body in
/// its room** — the free space the game looks for when it sets something
/// down itself ([`tidy`]), and no rule's.
///
/// A body is its footprint, in either layer, and — for a piece standing
/// on the deck — the wall it shadows behind it ([`shadows`]), because a
/// wardrobe's bulk stands in front of that wall whether or not anything
/// refuses a painting there. Two bodies meet where one's footprint shares
/// ground with the other's footprint or shadow; two shadows on one wall
/// are two bodies one in front of the other, which is not a meeting.
/// Touching is clear, so the game still sets things flush. Both layers
/// count, because free space is space nothing is in: a rug the game lays
/// goes on bare deck, and a crate it sets down goes beside the rug rather
/// than on it.
#[must_use]
pub fn clear(rooms: &Rooms, pieces: &[Piece], id: u32, kind: Kind, berth: Loc) -> bool {
    let spot = berth.spot();
    let Some(host) = rooms.kind(spot.room) else {
        return false;
    };
    let Some(foot) = Foot::of(host, kind, spot.x, spot.y, spot.turn) else {
        return false;
    };
    let mine = cast(host, kind, berth, foot);
    let on_wall = !matches!(foot.chart(host), Some(Surf::Floor | Surf::Ceiling));
    pieces.iter().filter(|other| other.id != id).all(|other| {
        let Some((at, theirs)) = Foot::at(rooms, other) else {
            return true;
        };
        if at != spot.room {
            return true;
        }
        let theirs_cast = || cast(host, other.kind, other.loc, theirs);
        !(foot.overlaps(theirs)
            || mine.iter().any(|shade| shade.overlaps(theirs))
            || (on_wall && theirs_cast().iter().any(|shade| shade.overlaps(foot))))
    })
}

/// The wall a body standing in `loc` with footprint `foot` shadows: its
/// [`shadows`] when it stands on the deck, and nothing for a body on a
/// wall or a deckhead, or for anything laid.
fn cast(host: RoomKind, kind: Kind, loc: Loc, foot: Foot) -> Vec<Foot> {
    if matches!(loc, Loc::Hold { .. }) && foot.chart(host) == Some(Surf::Floor) {
        shadows(host, foot, kind.stature())
    } else {
        Vec::new()
    }
}

/// **The game's own tidiness**: the first of `candidates` that stands
/// clear, or failing that the first that is legal.
///
/// Of the berths offered, in the order given, the first `kind` may take
/// ([`berth_check`]) standing [`clear`] of every other body in its room;
/// failing that, the first it may take at all; `None` only where the
/// arbiter allows none of them.
///
/// **A preference, never a rule** (docs/BAY.md, "Cargo stops
/// colliding"). Bodies may share space, and a player may set a crate in
/// a wardrobe on purpose. What the GAME sets down itself — the fitting
/// scans ([`first_fit`], [`dress_fit`]) and through them the quick-move,
/// salvage, the comet's ice, the exchange and banking the hopper; a
/// station's stock and its restock; a fluff's bud; the frontend's
/// fixture boards — looks for free space first, so a room it furnished
/// does not arrive in a heap. And it takes a crowded spot rather than
/// none, because a piece with somewhere legal to go is never refused for
/// want of elbow room. Stated once, here, so all of them mean the same
/// thing by it; each brings only its own order.
#[must_use]
pub fn tidy(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    candidates: impl IntoIterator<Item = Loc>,
) -> Option<Loc> {
    let mut crowded = None;
    for berth in candidates {
        if berth_check(rooms, pieces, id, kind, berth).is_err() {
            continue;
        }
        if clear(rooms, pieces, id, kind, berth) {
            return Some(berth);
        }
        crowded.get_or_insert(berth);
    }
    crowded
}

/// Whether `piece` is the LAST vital instrument of its kind in the
/// player's possession — the piece every exit ceremony must refuse.
///
/// Only berths that are STAYING count as possession: a spare already
/// staged on a calling room's offer area or on the incinerator's tiles
/// is itself on its way out, and counting it would let both of a pair be
/// staged and both be lost.
#[must_use]
pub fn last_vital_aboard(rooms: &Rooms, pieces: &[Piece], piece: &Piece) -> bool {
    piece.kind.vital()
        && !pieces
            .iter()
            .any(|other| other.id != piece.id && other.kind == piece.kind && staying(rooms, other))
}

/// Whether a piece's berth is one it would still hold after a launch.
///
/// The player's own, in a room that rides, and not scheduled for the
/// fire: the possession half of the vital rule. The tile asked is the
/// one under the footprint's centre ([`berth_tile`]).
#[must_use]
pub fn staying(rooms: &Rooms, piece: &Piece) -> bool {
    player_owned(rooms, piece.kind, piece.loc)
        && rooms.riding(piece.loc.room())
        && berth_tile(rooms, piece.kind, piece.loc) != Some(Tile::Consume)
}

/// Whether `kind` is a lamp — one of the three affixed fixtures that cast
/// light while berthed.
#[must_use]
pub const fn lamp(kind: Kind) -> bool {
    matches!(kind, Kind::CeilingLamp | Kind::WallLamp | Kind::FloorLamp)
}

/// Whether `piece` is a lamp, burning.
///
/// Lamps are lit while they stand in a room, which is everywhere a lamp
/// can be: nothing boxes one up any more. Everything lighting touches —
/// the rat's fear, the well-lit art bonus, any frontend halo — reads lamp
/// state through this one predicate.
#[must_use]
pub const fn lamp_lit(piece: &Piece) -> bool {
    lamp(piece.kind) && matches!(piece.loc, Loc::Hold { .. })
}

/// Which room a lit lamp lights, if it lights one.
#[must_use]
pub const fn lamp_room(piece: &Piece) -> Option<RoomId> {
    match piece.loc {
        Loc::Hold { room, .. } if lamp(piece.kind) => Some(room),
        _ => None,
    }
}

/// Whether cell `(room, x, y)` sits in light: [`lit_within_reach`],
/// asked of one cell. The rat walks its lattice a cell at a time, and
/// this is the shape of question it asks.
#[must_use]
pub fn lit_adjacent(host: RoomKind, pieces: &[Piece], room: RoomId, x: u8, y: u8) -> bool {
    lit_within_reach(host, pieces, room, Foot::cell(x, y))
}

/// Whether `target` — a cell, or a piece's footprint — sits in light.
///
/// Lit means less than one cell away from — and never wholly inside —
/// some lit lamp's footprint OR some laid luminous coat's, measured
/// straight across ([`Foot::clearance_below`]). Corners count, as they
/// have since the grid came out: a lamp a unit past a crate's corner is
/// not darker than one a unit past its edge. Euclidean, because light
/// does not care which way the room is turned, and a rule that did would
/// light a crate at one angle and not the next. Light does not cross a
/// seam: a lamp lights its own room. Everything light touches — the
/// rat's fear, the seedlings' bloom, the hold painting's spotlight —
/// reads through this one predicate; the well-lit-art price bonus
/// deliberately does not on the offer area (a coat is ambiance, not
/// gallery lighting).
///
/// **Light reaches in plan.** A lift is taste and no rule reads it
/// (docs/BAY.md, "Lift"): a lamp lifted onto a cabinet lights what is
/// around it on the ground under it, as it did standing there, and a
/// crate raised to the deckhead over a lamp is as lit as the ground it
/// stands over.
///
/// `host` is the room's own kind, because a lamp's footprint is a
/// question about the chart it stands on ([`Foot::of`]) and the caller
/// already knows whose room this is.
#[must_use]
pub fn lit_within_reach(host: RoomKind, pieces: &[Piece], room: RoomId, target: Foot) -> bool {
    pieces.iter().any(|piece| {
        let source = match piece.loc {
            Loc::Hold { .. } => lamp_lit(piece),
            Loc::Laid { .. } => piece.kind == Kind::LuminousPaint,
        };
        let spot = piece.loc.spot();
        if !source || spot.room != room {
            return false;
        }
        Foot::of(host, piece.kind, spot.x, spot.y, spot.turn).is_some_and(|light| {
            light.clearance_below(target, i32::from(FINE)) && !light.holds(target)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::room::CABIN;

    /// The ship as it leaves the yard, for the placement tests.
    fn ship() -> Rooms {
        Rooms::new()
    }

    /// The cabin's deck cell `(i, j)`, as a cell of its net. The tests
    /// below name every cell by the place in the room it is — a deck
    /// cell, the deckhead over one, a course of a wall counted up from
    /// the deck — so a wall that grows a course moves none of them.
    const fn deck(i: u8, j: u8) -> (u8, u8) {
        RoomKind::Cabin.deck_cell(i, j)
    }

    /// The cabin's deckhead over deck cell `(i, j)`.
    const fn deckhead(i: u8, j: u8) -> (u8, u8) {
        RoomKind::Cabin.deckhead_cell(i, j)
    }

    /// Course `course` of the cabin's aft wall, `along` cells from its
    /// port end.
    const fn aft(along: u8, course: u8) -> (u8, u8) {
        RoomKind::Cabin.wall_cell(0, along, course)
    }

    /// Course `course` of the cabin's starboard wall, `along` cells
    /// from its aft end.
    const fn starboard(along: u8, course: u8) -> (u8, u8) {
        RoomKind::Cabin.wall_cell(1, along, course)
    }

    /// Course `course` of the cabin's port wall, `along` cells from its
    /// aft end.
    const fn port(along: u8, course: u8) -> (u8, u8) {
        RoomKind::Cabin.wall_cell(3, along, course)
    }

    /// **The spot `kind` takes in the cabin with its footprint's top-left
    /// on whole cell `(x, y)`**, at the turn the game gives a body there:
    /// the berth the grid would have called "cell `(x, y)`". `None` where
    /// that corner is on no chart.
    fn cell(kind: Kind, (x, y): (u8, u8)) -> Option<Spot> {
        let (x, y, turn) = anchored(RoomKind::Cabin, kind, fine(x), fine(y))?;
        Some(Spot {
            room: CABIN,
            x,
            y,
            turn,
        })
    }

    /// A board of pieces berthed in the cabin at the given cells, ids
    /// counting up from 0.
    fn board(stowed: &[(Kind, (u8, u8))]) -> Vec<Piece> {
        stowed
            .iter()
            .enumerate()
            .map(|(i, &(kind, at))| Piece {
                id: i as u32,
                kind,
                variant: 0,
                gnawed: false,
                loc: cell(kind, at).expect("a cell of the net").hold(),
            })
            .collect()
    }

    /// The arbiter asked of a whole-cell anchor, against a [`board`];
    /// the candidate takes the next free id after it. A corner on no
    /// chart is out of bounds.
    fn check(stowed: &[(Kind, (u8, u8))], kind: Kind, at: (u8, u8)) -> Result<(), Violation> {
        cell(kind, at).map_or(Err(Violation::Bounds), |spot| {
            check_at(&board(stowed), kind, spot)
        })
    }

    /// Whether anything on `pieces` lights cell `(x, y)` of the cabin's
    /// net, read as if it were room `room`'s.
    fn lit_cell(pieces: &[Piece], room: RoomId, (x, y): (u8, u8)) -> bool {
        lit_adjacent(RoomKind::Cabin, pieces, room, x, y)
    }

    /// The arbiter asked of any spot, against any board.
    fn check_at(pieces: &[Piece], kind: Kind, spot: Spot) -> Result<(), Violation> {
        placement_check(&ship(), pieces, pieces.len() as u32, kind, spot)
    }

    #[test]
    fn bounds_rule_accepts_inside_and_names_offgrid() {
        // RationBricks is 2x2: fits on the deck by its aft wall; bent
        // over the starboard fold from the deck's last column; off the
        // net entirely from the deckhead's far column.
        assert_eq!(check(&[], Kind::RationBricks, deck(1, 0)), Ok(()));
        assert_eq!(
            check(&[], Kind::RationBricks, deck(7, 1)),
            Err(Violation::Bounds)
        );
        assert_eq!(
            check(&[], Kind::RationBricks, deckhead(0, 0)),
            Err(Violation::Bounds)
        );
    }

    /// **Cargo shares ground with cargo**, and the arbiter says so
    /// (docs/BAY.md, "Cargo stops colliding"): beside, on top of, and
    /// half over another piece are all berths, in either layer.
    #[test]
    fn cargo_shares_ground_with_cargo() {
        let stowed = [(Kind::PerfumeVial, deck(2, 2))];
        assert_eq!(check(&stowed, Kind::Seedlings, deck(3, 2)), Ok(()));
        assert_eq!(check(&stowed, Kind::Seedlings, deck(2, 2)), Ok(()));
        // Multi-cell: ScrapAlloy anchored at deck (1, 2) covers deck
        // (2, 2) too.
        assert_eq!(check(&stowed, Kind::ScrapAlloy, deck(1, 2)), Ok(()));
        // A crate in a wardrobe, and a painting behind one.
        let wardrobe = [(Kind::Cabinet, deck(3, 0))];
        assert_eq!(check(&wardrobe, Kind::PerfumeVial, deck(3, 0)), Ok(()));
        assert_eq!(check(&wardrobe, Kind::Painting, aft(2, 1)), Ok(()));
        // And a dressing under a couch, or over another dressing.
        let rooms = ship();
        let rug = cell(Kind::Rug, deck(1, 4)).expect("on the deck");
        assert_eq!(dressing_check(&rooms, Kind::Rug, rug), Ok(()));
        assert_eq!(
            berth_check(
                &rooms,
                &board(&[(Kind::Couch, deck(1, 4))]),
                9,
                Kind::Rug,
                rug.laid()
            ),
            Ok(())
        );
    }

    /// Nothing berths on a threshold: an aperture's cells belong to two
    /// rooms at once, and a detach would have to pick which grid kept
    /// the cargo. The doorway stays clear for a reason that survives.
    #[test]
    fn the_threshold_rule_keeps_every_aperture_clear() {
        // The cabin's starboard doorway, its aft doorway, its hatch.
        assert_eq!(
            check(&[], Kind::PerfumeVial, starboard(0, 0)),
            Err(Violation::Threshold)
        );
        assert_eq!(
            check(&[], Kind::WallLamp, aft(0, 1)),
            Err(Violation::Threshold)
        );
        assert_eq!(
            check(&[], Kind::PerfumeVial, deck(6, 4)),
            Err(Violation::Threshold)
        );
        // And a dressing cannot be laid across one either.
        let rooms = ship();
        let tin = cell(Kind::PaintTin, deck(6, 4)).expect("on the net");
        assert_eq!(
            dressing_check(&rooms, Kind::PaintTin, tin),
            Err(Violation::Threshold)
        );
    }

    #[test]
    fn heavy_lies_dormant_and_the_walls_refuse_plain_cargo() {
        assert_eq!(check(&[], Kind::GildedIdol, deck(0, 0)), Ok(()));
        assert_eq!(check(&[], Kind::GildedIdol, deck(2, 2)), Ok(()));
        // Lifted onto a wall, the mount law refuses (port chart, clear
        // of the doorway the aperture punches through it).
        assert_eq!(
            check(&[], Kind::GildedIdol, port(5, 2)),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn volatile_rule_accepts_gapped_and_names_adjacency() {
        let stowed = [(Kind::GasCanister, deck(0, 4))];
        assert_eq!(check(&stowed, Kind::GasCanister, deck(0, 2)), Ok(()));
        assert_eq!(
            check(&stowed, Kind::GasCanister, deck(0, 3)),
            Err(Violation::Volatile)
        );
        assert_eq!(
            check(&stowed, Kind::GasCanister, deck(1, 3)),
            Err(Violation::Volatile)
        );
        // Corner to corner is too close now: half a cell of clear air,
        // every way.
        assert_eq!(
            check(&stowed, Kind::GasCanister, deck(2, 3)),
            Err(Violation::Volatile)
        );
        assert_eq!(check(&stowed, Kind::GasCanister, deck(3, 2)), Ok(()));
    }

    #[test]
    fn cryo_rule_accepts_edge_and_names_interior() {
        assert_eq!(check(&[], Kind::CryoCore, deck(0, 1)), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, deck(1, 6)), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, deck(2, 2)), Err(Violation::Cryo));
    }

    #[test]
    fn suspicious_rule_accepts_one_and_names_a_second() {
        assert_eq!(check(&[], Kind::SuspiciousCrate, deck(0, 0)), Ok(()));
        let stowed = [(Kind::SuspiciousCrate, deck(0, 0))];
        assert_eq!(
            check(&stowed, Kind::SuspiciousCrate, deck(2, 2)),
            Err(Violation::Suspicious)
        );
    }

    #[test]
    fn the_floor_takes_cargo_anywhere_a_body_could_stand() {
        // The walker passes through cargo, so the floor keeps no
        // reserved lanes: a wall of cargo may close across the room.
        assert_eq!(check(&[], Kind::PerfumeVial, deck(7, 0)), Ok(()));
        assert_eq!(check(&[], Kind::PerfumeVial, deck(3, 4)), Ok(()));
        let wall: Vec<(Kind, (u8, u8))> = (0..6).map(|j| (Kind::PerfumeVial, deck(1, j))).collect();
        assert_eq!(check(&wall, Kind::PerfumeVial, deck(1, 6)), Ok(()));
        assert_eq!(check(&wall, Kind::PerfumeVial, deck(3, 2)), Ok(()));
    }

    /// **The game keeps clear of a standing body's bulk**: the cabinet
    /// (stature 2) against the aft baseboard stands in front of the two
    /// wall rows behind its cell, and the game hangs nothing there itself
    /// — though a player may, and the arbiter lets them.
    #[test]
    fn tall_floor_cargo_shadows_the_wall_behind_it_for_the_game() {
        let tidy_at = |stowed: &[(Kind, (u8, u8))], kind, at| {
            let spot = cell(kind, at).expect("on the net");
            let pieces = board(stowed);
            assert_eq!(check_at(&pieces, kind, spot), Ok(()), "{kind:?} is legal");
            clear(&ship(), &pieces, pieces.len() as u32, kind, spot.hold())
        };
        let stowed = [(Kind::Cabinet, deck(3, 0))];
        assert!(!tidy_at(&stowed, Kind::Painting, aft(2, 1)));
        // Two columns over, the wall is clear.
        assert!(tidy_at(&stowed, Kind::Painting, aft(4, 1)));
        // And symmetrically.
        let hung = [(Kind::Painting, aft(4, 1))];
        assert!(!tidy_at(&hung, Kind::Cabinet, deck(4, 0)));
        assert!(tidy_at(&hung, Kind::Cabinet, deck(2, 0)));
    }

    #[test]
    fn affix_rule_accepts_the_mount_surface_and_names_the_miss() {
        assert_eq!(check(&[], Kind::CeilingLamp, deckhead(5, 1)), Ok(()));
        assert_eq!(
            check(&[], Kind::CeilingLamp, aft(2, 1)),
            Err(Violation::Affix(Mount::Ceiling))
        );
        assert_eq!(check(&[], Kind::WallLamp, aft(2, 1)), Ok(()));
        assert_eq!(check(&[], Kind::WallLamp, port(3, 1)), Ok(()));
        assert_eq!(
            check(&[], Kind::WallLamp, deck(2, 2)),
            Err(Violation::Affix(Mount::Wall))
        );
        assert_eq!(check(&[], Kind::Couch, deck(1, 1)), Ok(()));
        assert_eq!(
            check(&[], Kind::Couch, aft(1, 2)),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn the_floor_lamp_stands_on_the_floor_and_never_across_a_fold() {
        assert_eq!(check(&[], Kind::FloorLamp, deck(1, 1)), Ok(()));
        assert_eq!(check(&[], Kind::FloorLamp, deck(1, 5)), Ok(()));
        assert_eq!(
            check(&[], Kind::FloorLamp, aft(2, 2)),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(
            check(&[], Kind::FloorLamp, aft(2, 0)),
            Err(Violation::Bounds)
        );
    }

    #[test]
    fn the_painting_hangs_on_any_wall_but_never_a_hole() {
        assert_eq!(check(&[], Kind::Painting, aft(2, 1)), Ok(()));
        // A flank too, now that the footprint is stated in the wall's
        // own frame: two cells along the port wall, one course tall.
        assert_eq!(check(&[], Kind::Painting, port(3, 2)), Ok(()));
        assert_eq!(
            check(&[], Kind::Painting, deck(1, 1)),
            Err(Violation::Affix(Mount::Wall))
        );
        assert_eq!(
            check(&[], Kind::Painting, starboard(6, 2)),
            Err(Violation::Bounds)
        );
    }

    /// Whether `(x, y)` is a wall cell one step off the deck — the
    /// baseboard course, where the fold is close enough to point at.
    fn baseboard(host: RoomKind, x: u8, y: u8) -> bool {
        [(0_i8, 1_i8), (0, -1), (1, 0), (-1, 0)]
            .into_iter()
            .any(|step| on_the_deck(host, x, y, step))
    }

    /// Whether one step from `(x, y)` lands on `host`'s deck.
    fn on_the_deck(host: RoomKind, x: u8, y: u8, (dx, dy): (i8, i8)) -> bool {
        let (fx, fy, fw, fh) = host.floor_rect();
        let step = |c: u8, d: i8| u8::try_from(i16::from(c) + i16::from(d)).ok();
        let (Some(cx), Some(cy)) = (step(x, dx), step(y, dy)) else {
            return false;
        };
        (fx..fx + fw).contains(&cx) && (fy..fy + fh).contains(&cy)
    }

    /// **Which way a wall chart's courses climb the sheet**, read off
    /// the net rather than assumed: the sheet direction a wall chart
    /// approaches the floor chart in is DOWN that wall. The aft and
    /// front walls fold off the deck's near and far rows, so their
    /// courses climb the sheet's y; the two flanks fold out sideways, so
    /// theirs climb its x. Derived here so the guard below asks the net
    /// the same question a player asks the room, instead of reading the
    /// arbiter's own table back at it.
    fn courses_climb_the_sheets_x(host: RoomKind, x: u8, y: u8) -> bool {
        // Step one cell each way and see which step lands on the deck:
        // that is the way down this wall, whatever the wall is.
        let vertical = on_the_deck(host, x, y, (0, 1)) || on_the_deck(host, x, y, (0, -1));
        let sideways = on_the_deck(host, x, y, (1, 0)) || on_the_deck(host, x, y, (-1, 0));
        assert!(
            vertical != sideways,
            "({x}, {y}) is not one cell off the deck of a {host:?}",
        );
        sideways
    }

    /// **A footprint keeps its shape on every wall it may take.**
    ///
    /// The net is one sheet folded into a box and the side flaps fold
    /// out sideways, so the two cells that lie level on the aft wall
    /// stood one above the other on a flank. Nothing in the drawing did
    /// that — the cells turned and the body lay on its cells — which is
    /// why it read to a player as the starting window rotating a quarter
    /// turn when it was carried one wall over, and why the arbiter's
    /// only answer used to be to refuse the wall.
    ///
    /// A footprint is stated in the body's own frame and laid on the
    /// sheet by [`net_angle`], so the claim can be made positively: on
    /// EVERY wall of EVERY room, an upright kind covers its own `across`
    /// along the wall and its own `tall` up it. Which sheet axis is which
    /// comes off the net's own fold ([`courses_climb_the_sheets_x`]) and
    /// not off the mapping's table, so the guard is not the
    /// implementation read back.
    #[test]
    fn a_footprint_keeps_its_shape_on_every_wall_it_may_take() {
        let mut seen: Vec<(RoomKind, Surf)> = Vec::new();
        for host in super::super::room::ROOM_KINDS {
            let (cols, rows) = host.grid();
            for kind in Kind::ALL {
                let (across, _, tall) = kind.extent();
                for y in 0..rows {
                    for x in 0..cols {
                        let Some(surf) = host.surface_of(x, y) else {
                            continue;
                        };
                        if matches!(surf, Surf::Floor | Surf::Ceiling) {
                            continue;
                        }
                        // The baseboard course, where the fold itself is
                        // one step away and the sheet can be asked which
                        // way that is.
                        if !baseboard(host, x, y) {
                            continue;
                        }
                        let span = Foot::planned(kind, surf, (0, 0), Turn::ZERO).aabb();
                        let (w, h) = (span.w() / i32::from(FINE), span.h() / i32::from(FINE));
                        let sideways = courses_climb_the_sheets_x(host, x, y);
                        let (along, up) = if sideways { (h, w) } else { (w, h) };
                        assert_eq!(
                            (along, up),
                            (i32::from(across), i32::from(tall)),
                            "{kind:?} on the {surf:?} wall of a {host:?} at ({x}, {y}) \
                             covers {along} along and {up} up",
                        );
                        if !seen.contains(&(host, surf)) {
                            seen.push((host, surf));
                        }
                    }
                }
            }
        }
        // Every wall of every room was actually asked, and both fold
        // directions are in the sample — a sweep that only met the aft
        // wall would pass this whatever the flanks did.
        assert_eq!(seen.len(), super::super::room::ROOM_KINDS.len() * 4);
        // And a non-square kind now hangs on a flank, which is the berth
        // the retired athwart rule existed to refuse.
        for kind in [Kind::Window, Kind::Painting] {
            assert_eq!(
                check(&[], kind, aft(2, 1)),
                Ok(()),
                "{kind:?} on the aft wall"
            );
            assert_eq!(
                check(&[], kind, port(2, 2)),
                Ok(()),
                "{kind:?} on the port wall"
            );
        }
        // A square footprint still cannot tell the flanks from the ends.
        assert_eq!(check(&[], Kind::Porthole, port(5, 1)), Ok(()));
        assert_eq!(check(&[], Kind::ChartTank, starboard(2, 0)), Ok(()));
    }

    #[test]
    fn affix_is_checked_before_the_per_piece_scan() {
        let stowed = [(Kind::RationBricks, deck(1, 1))];
        assert_eq!(
            check(&stowed, Kind::WallLamp, deck(1, 1)),
            Err(Violation::Affix(Mount::Wall))
        );
        // A canister lifted onto the baseboard right behind another one
        // is too close to it AND off the deck, and the mount is named.
        let stowed = [(Kind::GasCanister, deck(3, 0))];
        assert_eq!(
            check(&stowed, Kind::GasCanister, aft(3, 0)),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn lamps_are_lit_only_in_a_room_and_light_their_neighbours() {
        assert!(lamp(Kind::CeilingLamp) && lamp(Kind::WallLamp) && lamp(Kind::FloorLamp));
        assert!(!lamp(Kind::Couch) && !lamp(Kind::Painting) && !lamp(Kind::PerfumeVial));

        let pieces = board(&[(Kind::CeilingLamp, deckhead(5, 1))]);
        assert!(lamp_lit(&pieces[0]));
        assert!(lit_cell(&pieces, CABIN, deckhead(6, 1)));
        assert!(lit_cell(&pieces, CABIN, deckhead(4, 1)));
        assert!(lit_cell(&pieces, CABIN, deckhead(5, 2)));
        assert!(!lit_cell(&pieces, CABIN, deckhead(5, 1)));
        // Corners count: light reaches a cell's width, every way.
        assert!(lit_cell(&pieces, CABIN, deckhead(6, 0)));
        assert!(!lit_cell(&pieces, CABIN, deckhead(3, 1)));
        // Light does not cross a seam.
        assert!(!lit_cell(&pieces, 1, deckhead(6, 1)));

        // A standing lamp lights from the ONE cell of deck it occupies,
        // however tall it is: a height is spent up the wall behind it
        // (`Kind::stature`) and never across the floor beside it.
        let tall = board(&[(Kind::FloorLamp, deck(0, 1))]);
        assert!(lit_cell(&tall, CABIN, deck(1, 1)));
        assert!(lit_cell(&tall, CABIN, deck(0, 0)));
        assert!(lit_cell(&tall, CABIN, deck(0, 2)));
        assert!(lit_cell(&tall, CABIN, deck(1, 2)), "corner");
        assert!(lit_cell(&tall, CABIN, deck(1, 0)), "corner");
        assert!(!lit_cell(&tall, CABIN, deck(2, 1)), "a cell away");

        // Non-lamps light nothing.
        let art = board(&[(Kind::Painting, aft(2, 1))]);
        assert!(!lit_cell(&art, CABIN, aft(4, 1)));
    }

    /// The window family, and what makes it one: every size mounts on a
    /// wall, none of it burns, and every size fits on
    /// a wall of every room in the game. That last one is the aperture
    /// math's whole contract with the cargo table — a window nobody can
    /// hang anywhere is a window that would never show a sky.
    #[test]
    fn every_window_is_a_wall_fitting_that_fits_a_wall() {
        let family: Vec<Kind> = Kind::ALL.into_iter().filter(|kind| kind.window()).collect();
        assert_eq!(
            family,
            vec![Kind::Window, Kind::Porthole, Kind::BayWindow],
            "the family is the family"
        );
        let mut sizes: Vec<(u8, u8, u8)> = family.iter().map(|kind| kind.extent()).collect();
        sizes.sort_unstable();
        sizes.dedup();
        assert_eq!(sizes.len(), family.len(), "no two windows are one window");
        for kind in family {
            assert_eq!(kind.mount(), Mount::Wall, "{kind:?} hangs on a wall");
            assert_eq!(kind.flammable(), 0, "glass and brass do not burn");
            assert!(!kind.vital(), "a ship flies blind, unhappily");
            // Every room kind must be able to take it somewhere. Every
            // wall is "somewhere" now — a footprint is stated in the
            // wall's own frame, so the flanks take a non-square one the
            // same way the ends do — and the arbiter is the one asked,
            // because a hole punched through a wall is still a hole.
            for host in super::super::room::ROOM_KINDS {
                let (cols, rows) = host.grid();
                let fits = (0..rows).any(|y| {
                    (0..cols).any(|x| {
                        matches!(
                            host.surface_of(x, y),
                            Some(Surf::Aft | Surf::Port | Surf::Starboard | Surf::Front)
                        ) && anchored(host, kind, fine(x), fine(y)).is_some_and(|(x, y, turn)| {
                            placement_check(
                                &ship(),
                                &[],
                                0,
                                kind,
                                Spot {
                                    room: CABIN,
                                    x,
                                    y,
                                    turn,
                                },
                            )
                            .is_ok()
                        })
                    })
                });
                assert!(fits, "{kind:?} fits no wall of a {host:?}");
            }
        }
    }

    /// The dressing layer's rules are the room's: the chart a covering
    /// may cover, and whole on it. Whatever else is there is no business
    /// of the arbiter's — and the game's own scan still lays a rug on
    /// bare deck first, settling for a crowded stretch only when no bare
    /// one is left.
    #[test]
    fn dressing_rules_cover_the_surface_and_nothing_else() {
        let rooms = ship();
        let laid = |kind, at| {
            cell(kind, at).map_or(Err(Violation::Bounds), |spot| {
                dressing_check(&rooms, kind, spot)
            })
        };
        assert_eq!(laid(Kind::Rug, deck(1, 4)), Ok(()));
        assert_eq!(
            laid(Kind::Rug, aft(2, 1)),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(laid(Kind::Rug, deck(7, 4)), Err(Violation::Bounds));
        assert_eq!(laid(Kind::PaintTin, aft(2, 2)), Ok(()));
        assert_eq!(laid(Kind::LuminousPaint, deck(1, 1)), Ok(()));
        let rug_at = |at| cell(Kind::Rug, at).expect("on the deck").laid();
        let couch_at = |at| cell(Kind::Couch, at).expect("on the deck").hold();
        let rug = Piece {
            id: 2,
            kind: Kind::Rug,
            variant: 0,
            gnawed: false,
            loc: rug_at(deck(1, 4)),
        };
        let couch = Piece {
            id: 3,
            kind: Kind::Couch,
            variant: 0,
            gnawed: false,
            loc: couch_at(deck(0, 0)),
        };
        assert_eq!(first_fit(&rooms, &[], 9, Kind::Rug), None);
        assert_eq!(dress_fit(&rooms, &[], 9, Kind::Couch), None);
        // The first deck corner is under the couch, so the rug goes beside
        // it: free space first.
        let first = dress_fit(&rooms, &[], 9, Kind::Rug).expect("bare deck");
        assert_eq!(Some(first), cell(Kind::Rug, deck(0, 0)));
        let beside = dress_fit(&rooms, &[rug, couch], 9, Kind::Rug).expect("bare deck");
        assert_ne!(beside, first, "the game lays no rug under a couch");
        assert!(clear(&rooms, &[rug, couch], 9, Kind::Rug, beside.laid()));
    }

    #[test]
    fn luminous_coats_light_their_neighbours() {
        let coat = Piece {
            id: 0,
            kind: Kind::LuminousPaint,
            variant: 0,
            gnawed: false,
            loc: cell(Kind::LuminousPaint, aft(2, 1))
                .expect("on the wall")
                .laid(),
        };
        assert!(lit_cell(&[coat], CABIN, aft(3, 1)));
        assert!(lit_cell(&[coat], CABIN, aft(2, 2)));
        assert!(!lit_cell(&[coat], CABIN, aft(2, 1)), "never inside");
        assert!(lit_cell(&[coat], CABIN, aft(3, 2)), "corners count");
        assert!(!lit_cell(&[coat], CABIN, aft(4, 1)), "a cell away");
        let tin = Piece {
            id: 1,
            kind: Kind::PaintTin,
            variant: 0,
            gnawed: false,
            loc: cell(Kind::PaintTin, aft(4, 1)).expect("on the wall").laid(),
        };
        assert!(!lit_cell(&[tin], CABIN, aft(3, 1)));
    }

    /// Ownership is a function of tile class, and nothing else.
    #[test]
    fn ownership_reads_the_tile_class() {
        let mut rooms = Rooms::new();
        let trade = rooms
            .spawn(RoomKind::Trade, CABIN)
            .expect("a trade room attaches");
        // A vial on the trade room's own deck cell `(i, j)`.
        let at = |i, j| {
            let (x, y) = RoomKind::Trade.deck_cell(i, j);
            let (x, y, turn) =
                anchored(RoomKind::Trade, Kind::PerfumeVial, fine(x), fine(y)).expect("on the net");
            Loc::Hold {
                room: trade,
                x,
                y,
                turn,
                lift: 0,
            }
        };
        let owned = |loc| player_owned(&rooms, Kind::PerfumeVial, loc);
        // The trade room's aft floor row is its own stock; its front
        // floor row is the chalked offer square; the deck between is
        // ordinary, and so is everything aboard.
        assert!(!owned(at(2, 0)));
        assert!(owned(at(0, 3)));
        assert!(owned(at(0, 1)));
        // **Except the two cells its own door stands on**, which the
        // doorstep law hands back to the room's ordinary class: a body
        // walking in lands on deck it may use, and the shopfront starts
        // beside the door rather than under it.
        assert!(owned(at(0, 0)));
        assert!(owned(at(1, 0)));
        assert!(owned(
            cell(Kind::PerfumeVial, deck(1, 1)).expect("aboard").hold()
        ));
    }

    /// **A lift stops where the body meets the far side of the room**
    /// (docs/BAY.md, "Lift"): the room's section off the chart, less the
    /// body's own reach off it. A wardrobe two courses tall is raised two
    /// courses and its top is at the deckhead; a pendant a course deep is
    /// lowered three and stands on the deck; a painting is carried out
    /// from the aft wall to a cell short of the front one, and from a
    /// flank to a cell short of the other. A covering is flush. And for
    /// every kind on every chart of every room, the cap and the body make
    /// the room exactly.
    #[test]
    fn a_lift_stops_where_the_body_meets_the_far_side_of_the_room() {
        let (w, h) = RoomKind::Cabin.floor();
        let cells = |n: u8| u16::from(n) * FINE;
        let cabin = |kind, surf| lift_cap(RoomKind::Cabin, kind, surf);
        assert_eq!(cabin(Kind::Cabinet, Surf::Floor), cells(COURSES - 2));
        assert_eq!(cabin(Kind::CeilingLamp, Surf::Ceiling), cells(COURSES - 1));
        assert_eq!(cabin(Kind::Painting, Surf::Aft), cells(h - 1));
        assert_eq!(cabin(Kind::Painting, Surf::Front), cells(h - 1));
        assert_eq!(cabin(Kind::Painting, Surf::Port), cells(w - 1));
        assert_eq!(cabin(Kind::Painting, Surf::Starboard), cells(w - 1));
        assert_eq!(cabin(Kind::Rug, Surf::Floor), 0);
        assert_eq!(cabin(Kind::LuminousPaint, Surf::Aft), 0);
        for host in crate::sim::room::ROOM_KINDS {
            for kind in Kind::ALL.into_iter().filter(|kind| !kind.covering()) {
                for surf in Surf::ALL {
                    assert_eq!(
                        lift_cap(host, kind, surf) + cells(kind.proud(surf)),
                        cells(host.section(surf)),
                        "{kind:?} on {host:?}'s {surf:?} does not fill the room at its cap"
                    );
                }
            }
        }
    }

    /// A held piece never answers for its own old berth: a canister
    /// carried a cell along is not too near itself, and is too near
    /// another one there.
    #[test]
    fn held_piece_ignores_its_own_footprint() {
        let rooms = ship();
        let pieces = board(&[(Kind::GasCanister, deck(1, 1))]);
        for at in [deck(1, 1), deck(2, 1)] {
            let spot = cell(Kind::GasCanister, at).expect("on the deck");
            assert_eq!(
                placement_check(&rooms, &pieces, 0, Kind::GasCanister, spot),
                Ok(())
            );
            assert_eq!(
                placement_check(&rooms, &pieces, 1, Kind::GasCanister, spot),
                Err(Violation::Volatile)
            );
        }
    }

    /// A piece centred on fine `(x, y)` of the cabin at `turn`, id `id`.
    const fn at(id: u32, kind: Kind, x: u16, y: u16, turn: Turn) -> Piece {
        Piece {
            id,
            kind,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x,
                y,
                turn,
                lift: 0,
            },
        }
    }

    /// The cabin spot centred on fine `(x, y)` at `turn`.
    const fn spot(x: u16, y: u16, turn: Turn) -> Spot {
        Spot {
            room: CABIN,
            x,
            y,
            turn,
        }
    }

    /// One seventh of a turn: an angle no sum of halvings ever lands on,
    /// so nothing about it is square.
    const SEVENTH: Turn = Turn(9362);

    /// An eighth of a turn.
    const EIGHTH: Turn = Turn(1 << 13);

    /// **The trig is exact where it has to be and the same every time it
    /// is asked.** The quarter turns come out as zeroes and ones, so an
    /// axis-aligned footprint is exactly the rectangle the grid drew; the
    /// identities that make a turn's frame a frame hold in the bits, at
    /// every one of the 65,536 turns; and the values are pinned, so a
    /// machine that computed them differently would fail here before it
    /// forked a crew.
    #[test]
    fn the_trig_is_exact_at_the_quarters_and_the_same_everywhere() {
        let one = TRIG_ONE;
        for (turn, cos, sin) in [
            (Turn::ZERO, one, 0),
            (Turn::QUARTER, 0, one),
            (Turn::HALF, -one, 0),
            (Turn::quarters(3), 0, -one),
        ] {
            assert_eq!((turn.cos(), turn.sin()), (cos, sin), "{turn:?}");
            assert!(turn.square());
        }
        assert!(!SEVENTH.square() && !EIGHTH.square() && !Turn(1).square());
        let mut worst = 0.0_f64;
        for raw in 0..=u16::MAX {
            let turn = Turn(raw);
            // A quarter on is the same pair, turned: in the bits.
            let on = turn + Turn::QUARTER;
            assert_eq!((on.cos(), on.sin()), (-turn.sin(), turn.cos()), "{turn:?}");
            // Turning back is the mirror: in the bits.
            let back = Turn::ZERO - turn;
            assert_eq!((back.cos(), back.sin()), (turn.cos(), -turn.sin()));
            // And the pair is a unit vector to within a few billionths.
            let (c, s) = (
                turn.cos() as f64 / one as f64,
                turn.sin() as f64 / one as f64,
            );
            let truth = f64::from(raw) * std::f64::consts::TAU / 65_536.0;
            worst = worst
                .max((c - truth.cos()).abs())
                .max((s - truth.sin()).abs());
        }
        assert!(worst < 1e-8, "the trig strays {worst} from the true value");
        // Asked twice, answered the same; and pinned, so a platform that
        // computed one bit differently is caught here.
        for turn in [Turn(1), SEVENTH, EIGHTH, Turn(12_345)] {
            assert_eq!(
                (turn.cos(), turn.sin()),
                (turn.cos(), turn.sin()),
                "{turn:?}"
            );
        }
        assert_eq!((Turn(1).cos(), Turn(1).sin()), (1_073_741_819, 102_944));
        assert_eq!((SEVENTH.cos(), SEVENTH.sin()), (669_490_072, 839_466_823));
        assert_eq!((EIGHTH.cos(), EIGHTH.sin()), (759_250_125, 759_250_125));
    }

    /// **A footprint's corners land within a unit of true at any angle**,
    /// even four cells across, and at a quarter turn they are exactly the
    /// axis-aligned rectangle — the box round it IS it.
    #[test]
    fn corners_are_within_a_unit_of_true_and_exact_at_the_quarters() {
        let half = (2 * i32::from(FINE), i32::from(FINE) / 2);
        for raw in (0..=u16::MAX).step_by(97).chain([9362, 16_384, 32_768]) {
            let turn = Turn(raw);
            let foot = Foot::new(3000, 2000, half, turn);
            let angle = f64::from(raw) * std::f64::consts::TAU / 65_536.0;
            let (c, s) = (angle.cos(), angle.sin());
            let truth = |sa: f64, sb: f64| {
                let (hw, hh) = (f64::from(half.0), f64::from(half.1));
                (
                    (sb * hh).mul_add(s, (sa * hw).mul_add(c, 3000.0)),
                    (-sb * hh).mul_add(c, (sa * hw).mul_add(s, 2000.0)),
                )
            };
            let want = [
                truth(1.0, 1.0),
                truth(-1.0, 1.0),
                truth(-1.0, -1.0),
                truth(1.0, -1.0),
            ];
            for (got, want) in foot.corners().iter().zip(want) {
                // A unit, and the trig's billionths of one.
                let unit = 1.0 + 1e-6;
                assert!(
                    (f64::from(got.0) - want.0).abs() <= unit
                        && (f64::from(got.1) - want.1).abs() <= unit,
                    "{turn:?}: corner {got:?}, true {want:?}"
                );
            }
        }
        for quarter in 0..4 {
            let foot = Foot::new(3000, 2000, half, Turn::quarters(quarter));
            let span = foot.aabb();
            let (w, h) = if quarter % 2 == 0 {
                (2 * half.0, 2 * half.1)
            } else {
                (2 * half.1, 2 * half.0)
            };
            assert_eq!((span.w(), span.h()), (w, h), "quarter {quarter}");
            let mut corners = foot.corners().to_vec();
            corners.sort_unstable();
            assert_eq!(
                corners,
                [
                    (span.x0, span.y0),
                    (span.x0, span.y1),
                    (span.x1, span.y0),
                    (span.x1, span.y1)
                ],
                "quarter {quarter} is exactly its box"
            );
        }
    }

    /// **Flush is clear at any angle and a unit of overlap is not.** Two
    /// equal footprints side by side along their own across axis share an
    /// edge exactly — corners are rounded once, so the shared edge is the
    /// same two points in both — and the overlap test reads that as
    /// touching. A unit closer, at the square turn, the eighth, and the
    /// seventh alike, and they share ground.
    #[test]
    fn touching_is_clear_and_a_unit_of_overlap_is_not_at_any_angle() {
        let half = (i32::from(FINE), i32::from(FINE) / 2);
        for turn in [Turn::ZERO, EIGHTH, SEVENTH, Turn(1)] {
            let a = Foot::new(2000, 2000, half, turn);
            let [(x0, y0), (x1, y1), ..] = a.corners();
            // One footprint along: the far edge of `a` is the near edge of
            // the next, exactly.
            let step = (x0 - x1, y0 - y1);
            let b = Foot::new(2000 + step.0, 2000 + step.1, half, turn);
            assert!(
                !a.overlaps(b) && !b.overlaps(a),
                "{turn:?}: flush overlapped"
            );
            assert!(a.clearance_below(b, 1), "{turn:?}: flush is no distance");
            // A unit back along the dominant axis of that step is a sliver
            // of shared ground.
            let nudge = if step.0.abs() >= step.1.abs() {
                (step.0.signum(), 0)
            } else {
                (0, step.1.signum())
            };
            let c = Foot::new(2000 + step.0 - nudge.0, 2000 + step.1 - nudge.1, half, turn);
            assert!(a.overlaps(c) && c.overlaps(a), "{turn:?}: a unit in stood");
            // Corner to corner, diagonally across: touching again.
            let [(cx0, cy0), _, (cx2, cy2), _] = a.corners();
            let d = Foot::new(2000 + cx0 - cx2, 2000 + cy0 - cy2, half, turn);
            assert!(!a.overlaps(d), "{turn:?}: corner to corner overlapped");
        }
    }

    /// The footprint's arithmetic, stated once: the cells a footprint a
    /// few units off the grid stands on, the cell under its middle, and
    /// the distance between two of them.
    #[test]
    fn a_foot_covers_what_it_touches_and_reads_its_middle() {
        let host = RoomKind::Cabin;
        // A couch is two cells of deck; three units off the grid it
        // stands on three.
        let couch = Foot::of(host, Kind::Couch, fine(5) + 3, fine(4) + 128, Turn::ZERO)
            .expect("on the deck");
        assert_eq!(couch.cells().collect::<Vec<_>>(), [(4, 4), (5, 4), (6, 4)]);
        assert_eq!(couch.centre_cell(), (5, 4));
        // At a whole-cell berth the middle may lie on a seam, and it reads
        // the cell the footprint's top-left is in, which is the cell the
        // grid's rules always read.
        for kind in Kind::ALL {
            let (x, y, turn) = anchored(host, kind, fine(4), fine(4)).expect("on the deck");
            let foot = Foot::of(host, kind, x, y, turn).expect("on the deck");
            assert_eq!(foot.centre_cell(), (4, 4), "{kind:?} reads its corner");
        }
        // Distance is straight across: flush and corner to corner are no
        // distance, and a cell's daylight is a cell's.
        let a = Foot::cell(4, 4);
        assert!(a.clearance_below(Foot::cell(5, 4), 1), "flush");
        assert!(a.clearance_below(Foot::cell(5, 5), 1), "corner to corner");
        assert!(!a.clearance_below(Foot::cell(6, 5), i32::from(FINE)));
        assert!(a.clearance_below(Foot::cell(6, 5), i32::from(FINE) + 1));
        // Two cells off on each axis is a cell's daylight each way: √2 of
        // a cell, which a Chebyshev reading would have called one.
        assert!(!a.clearance_below(Foot::cell(6, 6), 362));
        assert!(a.clearance_below(Foot::cell(6, 6), 363));
        assert!(!a.overlaps(Foot::cell(5, 4)), "touching is not overlapping");
        assert!(a.holds(a) && !a.holds(couch));
        // Turned an eighth, a couch stands on the cells its body crosses
        // and not on the ones the box round it merely spans.
        let turned = Foot::of(host, Kind::Couch, fine(6), fine(6), EIGHTH).expect("on the deck");
        let cells: Vec<(u8, u8)> = turned.cells().collect();
        let boxed: Vec<(u8, u8)> = turned.aabb().cells().collect();
        assert!(cells.len() < boxed.len(), "{cells:?} against {boxed:?}");
        assert!(cells.contains(&(5, 5)) && cells.contains(&(6, 6)));
        assert!(!cells.contains(&(4, 7)) && !cells.contains(&(7, 4)));
    }

    /// **The game's tidiness reads flush as clear and a unit of overlap as
    /// crowded**, at any angle — and the arbiter allows both, because
    /// crowded is a matter of taste.
    #[test]
    fn flush_neighbours_are_clear_and_a_unit_of_overlap_is_crowded() {
        let vial = [at(
            0,
            Kind::PerfumeVial,
            fine(5) + 128,
            fine(5) + 128,
            Turn::ZERO,
        )];
        let one = |x, y| spot(x, y, Turn::ZERO);
        let tidy_at = |board: &[Piece], spot| {
            assert_eq!(check_at(board, Kind::Seedlings, spot), Ok(()));
            clear(&ship(), board, 9, Kind::Seedlings, spot.hold())
        };
        assert!(tidy_at(&vial, one(fine(6) + 128, fine(5) + 128)));
        assert!(tidy_at(&vial, one(fine(4) + 128, fine(5) + 128)));
        assert!(!tidy_at(&vial, one(fine(6) + 127, fine(5) + 128)));
        assert!(!tidy_at(&vial, one(fine(4) + 129, fine(6) + 127)));
        // A neighbour standing in the corner of the vial's cell: crowded
        // by the square vial, whose corner is there, and not by the vial
        // turned an eighth, whose waist is not.
        let turned = [at(
            0,
            Kind::PerfumeVial,
            fine(5) + 128,
            fine(5) + 128,
            EIGHTH,
        )];
        let corner = one(fine(6) + 92, fine(4) + 164);
        assert!(!tidy_at(&vial, corner));
        assert!(tidy_at(&turned, corner));
    }

    /// **[`tidy`] takes the first free spot, and the first legal one when
    /// nothing is free** — never nothing while the arbiter allows
    /// something.
    #[test]
    fn tidy_prefers_free_ground_and_settles_for_legal() {
        let rooms = ship();
        let crate_at = |at| cell(Kind::PerfumeVial, at).expect("on the deck");
        let board = board(&[(Kind::PerfumeVial, deck(1, 1))]);
        let taken = crate_at(deck(1, 1)).hold();
        let free = crate_at(deck(2, 1)).hold();
        // A wall spot first, which the arbiter refuses a vial outright.
        let wall = cell(Kind::PerfumeVial, aft(2, 1))
            .expect("on the wall")
            .hold();
        assert_eq!(
            tidy(&rooms, &board, 9, Kind::PerfumeVial, [wall, taken, free]),
            Some(free)
        );
        assert_eq!(
            tidy(&rooms, &board, 9, Kind::PerfumeVial, [wall, taken]),
            Some(taken),
            "crowded beats nowhere"
        );
        assert_eq!(tidy(&rooms, &board, 9, Kind::PerfumeVial, [wall]), None);
        // And a full deck still takes one more: the first legal berth.
        let mut full: Vec<Piece> = Vec::new();
        while let Some(spot) = first_fit(&rooms, &full, 999, Kind::PerfumeVial) {
            if !clear(&rooms, &full, 999, Kind::PerfumeVial, spot.hold()) {
                break;
            }
            full.push(Piece {
                id: full.len() as u32,
                kind: Kind::PerfumeVial,
                variant: 0,
                gnawed: false,
                loc: spot.hold(),
            });
        }
        assert!(full.len() > 30, "only {} vials fit aboard", full.len());
        let crowded = first_fit(&rooms, &full, 999, Kind::PerfumeVial).expect("a berth");
        assert!(placement_legal(
            &rooms,
            &full,
            999,
            Kind::PerfumeVial,
            crowded
        ));
        assert_eq!(
            crowded,
            fitting_order(&rooms, &full, 999, Kind::PerfumeVial)
                .find(|&spot| placement_legal(&rooms, &full, 999, Kind::PerfumeVial, spot))
                .expect("a legal spot"),
            "the first legal one"
        );
    }

    /// Two volatile pieces keep half a cell of clear air, **straight
    /// across**: directly below, half a cell is the line; corner to
    /// corner, the same half cell is the diagonal, which a Chebyshev
    /// buffer would have drawn at half a cell on each axis instead.
    #[test]
    fn volatile_cargo_keeps_half_a_cell_of_clear_air() {
        let half = FINE / 2;
        // A gas canister is two cells across the deck.
        let can = [at(0, Kind::GasCanister, fine(5), fine(5) + 128, Turn::ZERO)];
        let below = |gap: u16| spot(fine(5), fine(6) + 128 + gap, Turn::ZERO);
        assert_eq!(
            check_at(&can, Kind::GasCanister, below(half - 1)),
            Err(Violation::Volatile)
        );
        assert_eq!(check_at(&can, Kind::GasCanister, below(half)), Ok(()));
        // Corner to corner, `d` units of daylight each way is d√2 apart:
        // 90 is under half a cell and 91 is not.
        let diagonal = |d: u16| spot(fine(7) + d, fine(6) + 128 + d, Turn::ZERO);
        assert_eq!(
            check_at(&can, Kind::GasCanister, diagonal(90)),
            Err(Violation::Volatile)
        );
        assert_eq!(check_at(&can, Kind::GasCanister, diagonal(91)), Ok(()));
        // And the rule does not care which way either one is turned:
        // spun an eighth, the same canister at the same centre is the same
        // distance from a neighbour straight below its tip.
        let spun = [at(0, Kind::GasCanister, fine(6), fine(6), EIGHTH)];
        let tip = Foot::of(RoomKind::Cabin, Kind::GasCanister, fine(6), fine(6), EIGHTH)
            .expect("on the deck")
            .aabb()
            .y1;
        let under = |gap: i32| {
            let y = u16::try_from(tip + 128 + gap).expect("on the deck");
            spot(fine(6), y, Turn::ZERO)
        };
        assert_eq!(
            check_at(&spun, Kind::GasCanister, under(i32::from(half) - 1)),
            Err(Violation::Volatile)
        );
        assert_eq!(
            check_at(&spun, Kind::GasCanister, under(i32::from(half))),
            Ok(())
        );
    }

    /// Cryo reaches the hull within a sixteenth of a cell ([`HULL_TOUCH`])
    /// and not a unit more — square on, and turned, where it is the
    /// corner that reaches.
    #[test]
    fn cryo_reaches_the_hull_within_a_sixteenth_and_not_a_unit_more() {
        let (fx, fy, fw, _) = RoomKind::Cabin.floor_rect();
        let (left, top, right) = (fine(fx), fine(fy), fine(fx + fw));
        // The deck's middle, three cells in on each axis, and its second
        // row.
        let (mid_x, mid_y, second) = (fine(fx + 3), fine(fy + 3), fine(fy + 1));
        let reach = u16::try_from(HULL_TOUCH).expect("a sixteenth");
        let core = |x, y, turn| check_at(&[], Kind::CryoCore, spot(x, y, turn));
        assert_eq!(core(left + 128, mid_y, Turn::ZERO), Ok(()));
        assert_eq!(core(left + 128 + reach, mid_y, Turn::ZERO), Ok(()));
        assert_eq!(
            core(left + 128 + reach + 1, mid_y, Turn::ZERO),
            Err(Violation::Cryo)
        );
        assert_eq!(core(right - 128, second + 3, Turn::ZERO), Ok(()));
        assert_eq!(core(mid_x + 3, top + 128 + reach, Turn::ZERO), Ok(()));
        assert_eq!(
            core(mid_x + 3, top + 129 + reach, Turn::ZERO),
            Err(Violation::Cryo)
        );
        // Turned an eighth, the core reaches the wall with a corner: its
        // half-diagonal from the centre.
        let corner = Foot::planned(Kind::CryoCore, Surf::Floor, (0, 0), EIGHTH)
            .aabb()
            .x1;
        let out = u16::try_from(corner).expect("a hand's breadth");
        assert_eq!(core(left + out + reach, mid_y, EIGHTH), Ok(()));
        assert_eq!(
            core(left + out + reach + 1, mid_y, EIGHTH),
            Err(Violation::Cryo)
        );
    }

    /// A tall piece shadows the wall behind it while it comes within a
    /// cell of that wall, and a whole cell out it leaves room for the
    /// game to hang something behind it — square on, and turned, where
    /// the corner nearest the wall is what comes within the cell and the
    /// shadow is as wide as the body's own shadow on the seam. The arbiter
    /// allows every one of these; the shadow is the game's tidiness only.
    #[test]
    fn a_shadow_reaches_a_cell_off_the_wall_and_no_further() {
        let check_at = |board: &[Piece], kind, spot| {
            assert_eq!(placement_check(&ship(), board, 9, kind, spot), Ok(()));
            if clear(&ship(), board, 9, kind, spot.hold()) {
                Ok(())
            } else {
                Err("crowded")
            }
        };
        let (fx, fy, _, _) = RoomKind::Cabin.floor_rect();
        let wall = fine(fy);
        // A painting on the aft wall's second course, centred on the seam
        // between its third and fourth cells; a cabinet standing in front
        // of the fourth, and one cell further along.
        let (_, row) = aft(3, 1);
        let (seam, beside) = (fine(fx + 3), fine(fx + 4));
        let painting = spot(seam, fine(row) + 128, Turn::ZERO);
        let cabinet = |x, top: u16| at(0, Kind::Cabinet, x, top + 128, Turn::ZERO);
        let near = [cabinet(seam + 128, wall + 255)];
        assert_eq!(check_at(&near, Kind::Painting, painting), Err("crowded"));
        let clear = [cabinet(seam + 128, wall + 256)];
        assert_eq!(check_at(&clear, Kind::Painting, painting), Ok(()));
        // And symmetrically, hanging first.
        let hung = [at(0, Kind::Painting, seam, fine(row) + 128, Turn::ZERO)];
        let standing = |x, top: u16| spot(x, top + 128, Turn::ZERO);
        assert_eq!(
            check_at(&hung, Kind::Cabinet, standing(seam + 128, wall + 255)),
            Err("crowded")
        );
        assert_eq!(
            check_at(&hung, Kind::Cabinet, standing(seam + 128, wall + 256)),
            Ok(())
        );
        // The shadow is as wide as the piece runs along the seam: a
        // cabinet slid a unit past the painting's end clears it.
        assert_eq!(
            check_at(&hung, Kind::Cabinet, standing(beside + 128, wall)),
            Ok(()),
            "flush beside the painting's end"
        );
        assert_eq!(
            check_at(&hung, Kind::Cabinet, standing(beside + 127, wall)),
            Err("crowded")
        );
        // Turned an eighth, the cabinet's shadow is its corners' span on
        // the seam, and its nearest corner is what comes within the cell.
        let reach = Foot::planned(Kind::Cabinet, Surf::Floor, (0, 0), EIGHTH).aabb();
        let (ex, ey) = (
            u16::try_from(reach.x1).expect("small"),
            u16::try_from(reach.y1).expect("small"),
        );
        let turned = |x: u16, gap: u16| spot(x, wall + gap + ey, EIGHTH);
        // Its shadow begins at its own left corner: flush beside the
        // painting's end on the seam is clear, a unit over is not.
        assert_eq!(
            check_at(&hung, Kind::Cabinet, turned(beside + ex, 0)),
            Ok(())
        );
        assert_eq!(
            check_at(&hung, Kind::Cabinet, turned(beside + ex - 1, 0)),
            Err("crowded")
        );
        // And a cell out from its nearest corner, it shadows nothing.
        assert_eq!(
            check_at(&hung, Kind::Cabinet, turned(beside + ex - 1, 255)),
            Err("crowded")
        );
        assert_eq!(
            check_at(&hung, Kind::Cabinet, turned(beside + ex - 1, 256)),
            Ok(())
        );
    }

    /// Light reaches less than a cell, **straight across**: a vial whose
    /// corner is a cell less a unit off the lamp's on both axes is lit by
    /// a Chebyshev reading and dark by the true one, √2 of that away.
    #[test]
    fn light_reaches_less_than_a_cell_straight_across() {
        let lamp = [at(
            0,
            Kind::FloorLamp,
            fine(4) + 128,
            fine(5) + 128,
            Turn::ZERO,
        )];
        let host = RoomKind::Cabin;
        let vial = |x, y, turn| Foot::of(host, Kind::PerfumeVial, x, y, turn).expect("on the deck");
        let lit = |foot| lit_within_reach(host, &lamp, CABIN, foot);
        // Beside it: a cell less a unit lights, a cell does not.
        assert!(lit(vial(fine(6) + 127, fine(5) + 128, Turn::ZERO)));
        assert!(!lit(vial(fine(6) + 128, fine(5) + 128, Turn::ZERO)));
        // Off its corner: d units of daylight each way is d√2 away.
        assert!(lit(vial(
            fine(5) + 128 + 181,
            fine(6) + 128 + 181,
            Turn::ZERO
        )));
        assert!(!lit(vial(
            fine(5) + 128 + 182,
            fine(6) + 128 + 182,
            Turn::ZERO
        )));
        // Never inside — and a vial turned an eighth on the lamp's own
        // cell pokes its corners out past the lamp, so it is lit.
        assert!(!lit(vial(fine(4) + 128, fine(5) + 128, Turn::ZERO)));
        assert!(lit(vial(fine(4) + 128, fine(5) + 128, EIGHTH)));
        // A lamp turned an eighth lights a cell's width from its corners.
        let spun = [at(0, Kind::FloorLamp, fine(4) + 128, fine(5) + 128, EIGHTH)];
        let corner = Foot::of(host, Kind::FloorLamp, fine(4) + 128, fine(5) + 128, EIGHTH)
            .expect("on the deck")
            .aabb()
            .x1;
        let right = |gap: i32| {
            let x = u16::try_from(corner + gap + 128).expect("on the deck");
            vial(x, fine(5) + 128, Turn::ZERO)
        };
        assert!(lit_within_reach(host, &spun, CABIN, right(255)));
        assert!(!lit_within_reach(host, &spun, CABIN, right(256)));
    }

    /// `first_fit` tries the corners flush against what is already
    /// standing, so a gap that is exactly a crate wide but sits off the
    /// grid is found — before any whole-cell corner further along.
    #[test]
    fn first_fit_finds_a_snug_gap_off_the_grid() {
        let (fx, fy, _, _) = RoomKind::Cabin.floor_rect();
        let (left, top) = (fine(fx), fine(fy));
        // Two vials on the first row of deck, a vial's width apart, both
        // half a cell off the grid: every whole-cell corner up to the
        // second one's far side overlaps one of them.
        let board = [
            at(0, Kind::PerfumeVial, left + 256, top + 128, Turn::ZERO),
            at(
                1,
                Kind::PerfumeVial,
                left + 256 + 2 * FINE,
                top + 128,
                Turn::ZERO,
            ),
        ];
        assert_eq!(
            first_fit(&ship(), &board, 9, Kind::PerfumeVial),
            Some(spot(left + 256 + FINE, top + 128, Turn::ZERO))
        );
    }

    /// **`first_fit` turns a body only when nothing aboard takes it the
    /// way the game would stand it**, and then a quarter turn is a berth
    /// like any other. A deck filled with vials everywhere but one gap
    /// two cells deep and one across has no room for a couch standing
    /// upright, and the gap is a couch turned on its side.
    #[test]
    fn first_fit_turns_a_body_only_when_nothing_takes_it_upright() {
        let rooms = ship();
        let mut pieces: Vec<Piece> = Vec::new();
        for (room, placed) in rooms.iter() {
            let (fx, fy, fw, fh) = placed.kind.floor_rect();
            for y in fy..fy + fh {
                for x in fx..fx + fw {
                    if room == CABIN && x == 6 && (5..7).contains(&y) {
                        continue;
                    }
                    let Some((cx, cy, turn)) =
                        anchored(placed.kind, Kind::PerfumeVial, fine(x), fine(y))
                    else {
                        continue;
                    };
                    let vial = Spot {
                        room,
                        x: cx,
                        y: cy,
                        turn,
                    };
                    let id = pieces.len() as u32;
                    if placement_legal(&rooms, &pieces, id, Kind::PerfumeVial, vial) {
                        pieces.push(Piece {
                            id,
                            kind: Kind::PerfumeVial,
                            variant: 0,
                            gnawed: false,
                            loc: vial.hold(),
                        });
                    }
                }
            }
        }
        let found = first_fit(&rooms, &pieces, 999, Kind::Couch).expect("the gap");
        assert_eq!(
            (found.room, found.x, found.y),
            (CABIN, fine(6) + 128, fine(6)),
            "the couch stands in the gap"
        );
        assert!(
            found.turn == Turn::QUARTER || found.turn == Turn::quarters(3),
            "on its side: {:?}",
            found.turn
        );
        // A square body is never offered a turn it could not use.
        assert!(
            fitting_spots(&rooms, &pieces, 999, Kind::PerfumeVial, 1).is_empty(),
            "a vial has one shape at every quarter"
        );
    }

    /// **The backing rule, as the game's own placements read it**: a
    /// deck body within half a cell of a seam turns its back to it, the
    /// aft seam first; a flank turns only a one-column body; everything
    /// else faces the front; a deckhead reads the same rule off its own
    /// sheet; a wall is upright.
    #[test]
    fn the_game_turns_a_body_against_the_seam_it_stands_by() {
        let host = RoomKind::Cabin;
        let (fx, fy, fw, fh) = host.floor_rect();
        let (left, top) = (fine(fx), fine(fy));
        let (right, bottom) = (fine(fx + fw), fine(fy + fh));
        let deck = |kind, x, y| default_turn(host, kind, Surf::Floor, (x, y));
        let mid = (fine(7), fine(6) + 128);
        assert_eq!(deck(Kind::Cabinet, mid.0, mid.1), Turn::ZERO);
        // Half a cell or less off the aft seam: back to it, facing front.
        assert_eq!(deck(Kind::Cabinet, mid.0, top + 128 + 128), Turn::ZERO);
        // Off the front seam: back to it, which is half a turn.
        assert_eq!(deck(Kind::Cabinet, mid.0, bottom - 128), Turn::HALF);
        assert_eq!(deck(Kind::Cabinet, mid.0, bottom - 128 - 128), Turn::HALF);
        assert_eq!(deck(Kind::Cabinet, mid.0, bottom - 128 - 129), Turn::ZERO);
        // Off the port seam, a one-column body faces starboard: its back
        // to the port wall, three quarters round from facing the front.
        assert_eq!(deck(Kind::Cabinet, left + 128, mid.1), Turn::quarters(3));
        assert_eq!(deck(Kind::Cabinet, right - 128, mid.1), Turn::QUARTER);
        // A couch two cells across stays facing the front there: a
        // quarter turn would stand it off its own cells.
        assert_eq!(deck(Kind::Couch, left + 256, mid.1), Turn::ZERO);
        // The aft seam is asked first: a corner reads as aft.
        assert_eq!(deck(Kind::Cabinet, left + 128, top + 128), Turn::ZERO);
        // The deckhead reads the same rule off its own sheet.
        let (cx, cy, cw, _) = host.chart_rect(Surf::Ceiling);
        let ceiling = |x, y| default_turn(host, Kind::CeilingLamp, Surf::Ceiling, (x, y));
        assert_eq!(
            ceiling(fine(cx) + 128, fine(cy) + 3 * 256 + 128),
            Turn::quarters(3)
        );
        assert_eq!(
            ceiling(fine(cx + cw) - 128, fine(cy) + 3 * 256 + 128),
            Turn::QUARTER
        );
        // A wall is upright wherever it is hung.
        for surf in [Surf::Aft, Surf::Port, Surf::Starboard, Surf::Front] {
            assert_eq!(default_turn(host, Kind::Painting, surf, mid), Turn::ZERO);
        }
    }

    /// **The one mapping from chart to net, pinned against the net's own
    /// folds.** The base angle of every chart is read back off the sheet
    /// the way a player reads the room: on a wall, a body's up points
    /// away from the deck it folds off; on the deck and the deckhead, an
    /// upright body faces the front, which is the sheet's +y on both.
    /// And the handedness is one for all six: the four walls' "right" —
    /// the base itself — runs round the deck's own edge the same way on
    /// every side, which is what a sheet seen all from one side does.
    #[test]
    fn the_chart_to_net_mapping_reads_the_nets_own_folds() {
        let unit = |turn: Turn| {
            (
                (turn.cos() / TRIG_ONE) as i32,
                (turn.sin() / TRIG_ONE) as i32,
            )
        };
        for host in super::super::room::ROOM_KINDS {
            let (fx, fy, fw, fh) = host.floor_rect();
            let walls = [
                (Surf::Aft, (fx, fy - 1), (0, 1)),
                (Surf::Front, (fx, fy + fh), (0, -1)),
                (Surf::Port, (fx - 1, fy), (1, 0)),
                (Surf::Starboard, (fx + fw, fy), (-1, 0)),
            ];
            for (surf, (x, y), toward_deck) in walls {
                assert_eq!(host.surface_of(x, y), Some(surf), "{host:?} {surf:?}");
                // Up is a quarter on from across, and it leads away from
                // the deck: one step down that wall lands on the deck.
                let up = unit(net_angle(surf, Turn::QUARTER));
                assert_eq!(up, (-toward_deck.0, -toward_deck.1), "{host:?} {surf:?} up");
                // Right runs along the seam.
                let right = unit(net_angle(surf, Turn::ZERO));
                assert_eq!(right.0 * toward_deck.0 + right.1 * toward_deck.1, 0);
                // And it runs round the deck one way on every wall: turning
                // from the way to the deck onto the way right leads is the
                // same turn on all four sides, which is a sheet seen from
                // one side and no chart flipped over.
                let turning = toward_deck.0 * right.1 - toward_deck.1 * right.0;
                assert_eq!(turning, 1, "{host:?} {surf:?} is mirrored");
            }
            // On the deck and the deckhead an upright body faces the front,
            // which is the sheet's +y on both: its face a quarter short of
            // its across axis on the deck, standing up out of it, and a
            // quarter past it on the deckhead, hanging down.
            assert_eq!(unit(net_angle(Surf::Floor, Turn::quarters(3))), (0, 1));
            assert_eq!(unit(net_angle(Surf::Ceiling, Turn::QUARTER)), (0, 1));
        }
        // The mapping is the base plus the turn, and a turn wraps.
        for surf in Surf::ALL {
            for turn in [Turn::ZERO, Turn(1), SEVENTH, Turn(u16::MAX)] {
                assert_eq!(
                    net_angle(surf, turn) - net_angle(surf, Turn::ZERO),
                    turn,
                    "{surf:?}"
                );
            }
        }
    }

    /// **Every yes the arbiter says is ground a piece can stand on**, at
    /// every whole-cell centre and a fixed sample of units off it
    /// ([`FRACTIONS`]) on each axis, at the square turns and some that
    /// are not: every corner lies in its net, on one chart, and over no
    /// cell a refusing class keeps.
    #[test]
    fn every_accepted_berth_lies_on_one_chart_clear_of_refusals() {
        for host in super::super::room::ROOM_KINDS {
            let rooms = Rooms::root(host);
            let (cols, rows) = host.grid();
            for kind in Kind::ALL {
                for turn in [Turn::ZERO, Turn::QUARTER, SEVENTH, EIGHTH] {
                    for y in 0..rows {
                        for x in 0..cols {
                            for (dx, dy) in FRACTIONS
                                .into_iter()
                                .flat_map(|dx| FRACTIONS.into_iter().map(move |dy| (dx, dy)))
                            {
                                let (fx, fy) = (fine(x) + dx, fine(y) + dy);
                                let spot = Spot {
                                    room: 0,
                                    x: fx,
                                    y: fy,
                                    turn,
                                };
                                if berth_check(&rooms, &[], 0, kind, spot.berth(kind)).is_err() {
                                    continue;
                                }
                                let foot = Foot::of(host, kind, fx, fy, turn).expect("accepted");
                                let chart = foot.chart(host);
                                let (cx, cy, cw, ch) = host.chart_rect(chart.expect("on a chart"));
                                for (px, py) in foot.corners() {
                                    assert!(
                                        (i32::from(fine(cx))..=i32::from(fine(cx + cw)))
                                            .contains(&px)
                                            && (i32::from(fine(cy))..=i32::from(fine(cy + ch)))
                                                .contains(&py),
                                        "{kind:?} at ({fx}, {fy}) {turn:?} leaves its chart"
                                    );
                                }
                                for (cx, cy) in foot.cells() {
                                    assert_eq!(
                                        host.surface_of(cx, cy),
                                        chart,
                                        "{kind:?} at ({fx}, {fy}) {turn:?}"
                                    );
                                    assert!(
                                        !matches!(
                                            host.tile_of(cx, cy),
                                            Some(Tile::Threshold | Tile::Fixture)
                                        ),
                                        "{kind:?} at ({fx}, {fy}) {turn:?} of a {host:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
