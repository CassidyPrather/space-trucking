//! Cargo pieces: what they are, where they sit, and the stowage rules.
//!
//! A [`Piece`] is one draggable object. Its [`Loc`] says which room it
//! stands in and where on that room's net, in sixteenths of a cell
//! ([`FINE`]) — every berth in the game is a position on a room's net or
//! a cubby inside a piece standing on one — and [`placement_check`] is
//! the single arbiter of whether a piece may sit there. The renderer and the drag logic both defer to it, so there is
//! exactly one opinion about what fits, and a failure names the
//! [`Violation`] so the frontend can flash the right icon.
//!
//! Ownership is not a list. [`player_owned`] asks the berth's room and
//! tile class and nothing else (docs/ROOMS.md, "The tile-class
//! vocabulary"): the room's own goods sit on `Stock` tiles, and
//! everything else aboard or alongside is the player's.

use super::room::{RoomId, RoomKind, Rooms, Surf, Tile};

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
    /// A slim deck-bolted wardrobe with four cubbies: the first piece
    /// of cargo that *provides* berths (see `Loc::Stow`). Small goods
    /// ride inside — dry, dark, and beyond the reach of rats.
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
    match mount {
        Mount::Floor => matches!(surf, Surf::Floor),
        Mount::Ceiling => matches!(surf, Surf::Ceiling),
        Mount::Wall => matches!(surf, Surf::Aft | Surf::Port | Surf::Starboard | Surf::Front),
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
    /// ([`Kind::plan_on`]) and never the kind's: a wardrobe is one cell
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
            // and nothing three cells wide and two courses tall can
            // stand anywhere on it. A window no station can put out is
            // a window nobody can buy (`barter`, the shelf-fit test).
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
    /// and a berth spins it; the cells it lands on are
    /// [`Kind::plan_on`]'s answer, and on a flank the two are transposes
    /// of one another.
    #[must_use]
    pub const fn upright(self) -> (u8, u8) {
        let (across, _, tall) = self.extent();
        (across, tall)
    }

    /// **The net cells this kind covers on a chart of class `surf`** —
    /// its footprint, stated in the SURFACE's own frame and read back
    /// into the sheet's.
    ///
    /// Two readings, and the chart picks:
    ///
    /// - A chart a body lies **on** — the deck, the deckhead — spends
    ///   the plan: across by deep. What is left over is the height, and
    ///   the height is not the deck's to spend ([`Kind::stature`]).
    /// - A chart a body hangs **against** spends the elevation: across
    ///   by tall. Which way round those land on the sheet depends on
    ///   the wall, because the net is one sheet of paper folded into a
    ///   box and its two side flaps fold out SIDEWAYS: a flank's courses
    ///   climb the sheet's **x** where the aft and front walls' climb
    ///   its **y**. So a flank takes the elevation transposed.
    ///
    /// That transposition is the whole of the old athwart rule, done
    /// properly. A footprint used to be declared in the sheet's frame,
    /// which made the same two cells mean "side by side" on one wall
    /// and "one above the other" on another — so a window carried one
    /// wall over came out a quarter turn from the window that left, and
    /// the arbiter's only answer was to refuse the wall. A body keeps
    /// its shape now and the CELLS turn under it, which is what was
    /// turning all along.
    #[must_use]
    pub const fn plan_on(self, surf: Surf) -> (u8, u8) {
        let (across, deep, tall) = self.extent();
        match surf {
            Surf::Floor | Surf::Ceiling => (across, deep),
            Surf::Aft | Surf::Front => (across, tall),
            Surf::Port | Surf::Starboard => (tall, across),
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
    /// adjacent wall this kind shadows when it stands against one (no
    /// painting behind the wardrobe). The third number of the kind's
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

    /// Whether this kind is one of the ship's instruments — the wall
    /// fittings every hull launches with. Named so the save reader can
    /// hang the missing ones when it loads a document from before they
    /// were cargo.
    ///
    /// The porthole and the bay window are NOT here, and the distinction
    /// is the point: a hull launches with one window, and everything
    /// else with glass in it was bought.
    #[must_use]
    pub const fn instrument(self) -> bool {
        matches!(
            self,
            Self::Window | Self::ChartTank | Self::EtaGauge | Self::DestPreview | Self::LaunchLever
        )
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
/// Every berth in the game is a position on some room's net, or a cubby
/// inside a piece standing on one. Cubbies need no room qualifier — a
/// cabinet knows what room it stands in.
///
/// **A position is not a cell** (docs/BAY.md, "The grid comes out"). `x`
/// and `y` are the footprint's top-left corner in [`FINE`] units of the
/// room's net — net cell `(cx, cy)` spans `cx * FINE .. (cx + 1) * FINE`
/// — so a piece may stand anywhere a sixteenth of a cell can name. The
/// footprint itself is still whole cells in size ([`Foot`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loc {
    /// Standing in a room: the footprint's top-left corner, in fine
    /// units of the room's net.
    Hold { room: RoomId, x: u16, y: u16 },
    /// Inside a cabinet's cubby. The berth exists only while that cabinet
    /// piece does: an occupied cabinet cannot be lifted, so the cubby can
    /// never find itself without a home.
    Stow { cabinet: u32, slot: u8 },
    /// Laid into a room with its top-left corner at `(x, y)`, in fine
    /// units: the dressing layer. Coexists with occupancy over the same
    /// ground (a couch stands on a laid rug); no two dressings overlap
    /// ([`dressing_check`]).
    Laid { room: RoomId, x: u16, y: u16 },
}

impl Loc {
    /// Which room this berth is in, following a cubby to its cabinet.
    #[must_use]
    pub fn room(self, pieces: &[Piece]) -> Option<RoomId> {
        match self {
            Self::Hold { room, .. } | Self::Laid { room, .. } => Some(room),
            Self::Stow { cabinet, .. } => pieces
                .iter()
                .find(|piece| piece.id == cabinet)
                .and_then(|host| host.loc.room(pieces)),
        }
    }
}

/// Cubbies per cabinet: a 2×2 rack of them behind the doors.
pub const CABINET_SLOTS: u8 = 4;

/// Whether `kind` may ride inside a cabinet.
///
/// One cell, and neither the kinds that need the hull's cold (cryo) nor
/// the ones nobody should box up (suspicious — none is 1×1 today, but the
/// rule is written for the day one is). Windows are out too, at every
/// size: a hole in the hull is not a thing you put in a drawer, and a
/// porthole that fit in one would be showing the inside of a wardrobe.
/// Everything else about a stowed piece is ordinary; what *emerges* from
/// a cubby not being a room cell — dark lamps, unbred fluff, rat-proof
/// shelter, invisibility to ??? — is documented in docs/BAY.md, not
/// special-cased anywhere.
#[must_use]
pub const fn stowable(kind: Kind) -> bool {
    matches!(kind.extent(), (1, 1, 1))
        && !kind.window()
        && !matches!(kind.tag(), Some(Tag::Cryo | Tag::Suspicious))
}

/// **The net cells `kind` covers anchored at `(x, y)` of `host`'s net.**
///
/// The one place a berth turns into cells. A footprint is a property of
/// the kind and the CHART it lands on together ([`Kind::plan_on`]) —
/// a plan on the deck, an elevation on a wall, that elevation transposed
/// down a flank — so nothing may answer "which cells" from the kind
/// alone, and everything that used to (the arbiter, the rects the
/// renderer grabs pieces by, the rat's idea of what is underfoot) comes
/// through here.
///
/// `None` where `(x, y)` is not a cell of that net at all: a hole, a
/// fold, a fixture's own socket. A berth nobody can name has no
/// footprint, and the callers that can be handed one say so.
#[must_use]
pub const fn plan(host: RoomKind, kind: Kind, x: u8, y: u8) -> Option<(u8, u8)> {
    match host.surface_of(x, y) {
        Some(surf) => Some(kind.plan_on(surf)),
        None => None,
    }
}

/// **Sub-units per cell**: the quantum a berth is positioned in.
///
/// A sixteenth of a cell — about 34 mm in the cabin, which is also the
/// frontend's finest cut of the world — so a player never sees it, and
/// whole numbers all the way down, so no floating point ever reaches
/// the arbiter and a lockstep crew agrees to the unit.
pub const FINE: u16 = 16;

/// **The sixteenths a sweep tries on each axis** of every whole-cell
/// anchor, when a test means "every berth".
///
/// A berth can be any of 256 positions per cell, and a sweep over all of
/// them is 256 times the runtime for very little more coverage, so the
/// sweeps share one fixed sample instead: on the grid, a sixteenth off
/// it either way, and either side of the half. Deterministic, so a
/// failure names the same berth every run.
pub const FRACTIONS: [u16; 5] = [0, 1, 7, 8, 15];

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

/// **A footprint as ground**: a half-open rect on a room's net, in
/// [`FINE`] units.
///
/// The one place the rect arithmetic lives, so nothing that asks what a
/// piece covers, where its middle is, or how far it stands from another
/// re-derives it. A footprint is still whole cells in size — `plan * FINE`
/// on each axis ([`Kind::plan_on`]) — and only its position is free.
/// Which chart it is planned against is the chart of the cell holding
/// its top-left fine point ([`Foot::chart`]); the arbiter insists it lie
/// wholly in that chart anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foot {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Foot {
    /// The footprint `kind` takes with its top-left corner at fine
    /// `(x, y)` of `host`'s net, or `None` where that corner is on no
    /// chart at all.
    #[must_use]
    pub const fn of(host: RoomKind, kind: Kind, x: u16, y: u16) -> Option<Self> {
        match plan(host, kind, coarse(x), coarse(y)) {
            Some((w, h)) => Some(Self {
                x,
                y,
                w: fine(w),
                h: fine(h),
            }),
            None => None,
        }
    }

    /// Where `piece` stands, if it stands in a room: its room and its
    /// footprint. A cubby has no ground of its own, and a berth off its
    /// room's net covers nothing.
    #[must_use]
    pub fn at(rooms: &Rooms, piece: &Piece) -> Option<(RoomId, Self)> {
        match piece.loc {
            Loc::Hold { room, x, y } | Loc::Laid { room, x, y } => {
                Some((room, Self::of(rooms.kind(room)?, piece.kind, x, y)?))
            }
            Loc::Stow { .. } => None,
        }
    }

    /// Net cell `(x, y)` as a footprint: what a question about one cell
    /// (the rat's, the light's) asks with.
    #[must_use]
    pub const fn cell(x: u8, y: u8) -> Self {
        Self {
            x: fine(x),
            y: fine(y),
            w: FINE,
            h: FINE,
        }
    }

    /// The fine coordinate one past the right edge.
    #[must_use]
    pub const fn right(self) -> u16 {
        self.x + self.w
    }

    /// The fine coordinate one past the bottom edge.
    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y + self.h
    }

    /// The chart this footprint is planned against: the one holding its
    /// top-left fine point.
    #[must_use]
    pub const fn chart(self, host: RoomKind) -> Option<Surf> {
        host.surface_of(coarse(self.x), coarse(self.y))
    }

    /// Every net cell the half-open rect intersects, row-major. A
    /// footprint that sits a sixteenth off the grid covers a row and a
    /// column more than its size, because it is standing on them.
    pub fn cells(self) -> impl Iterator<Item = (u8, u8)> + use<> {
        let (x0, x1) = (coarse(self.x), coarse(self.right() - 1));
        let (y0, y1) = (coarse(self.y), coarse(self.bottom() - 1));
        (y0..=y1).flat_map(move |cy| (x0..=x1).map(move |cx| (cx, cy)))
    }

    /// **The cell under the footprint's centre**: the one every rule
    /// that asks "what tile does this piece stand on" reads.
    ///
    /// Exactly, the cell holding the fine unit just short of the centre
    /// on each axis, so a centre lying on a seam reads the cell above
    /// and to the left of it. That tie-break is chosen for what it
    /// keeps: at any whole-cell anchor, every kind the game has (none is
    /// wider or taller than two cells) reads its ANCHOR cell, which is
    /// the cell the grid's rules always read.
    #[must_use]
    pub const fn centre(self) -> (u8, u8) {
        (
            coarse(self.x + (self.w / 2).saturating_sub(1)),
            coarse(self.y + (self.h / 2).saturating_sub(1)),
        )
    }

    /// Whether the two rects share any ground. Half-open, so two
    /// footprints touching along an edge do not overlap: flush is legal.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Whether `other` lies wholly inside this rect.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    /// The **Chebyshev gap** between two rects, in fine units: the
    /// larger of the clear air between them across and down. Zero when
    /// they touch, corner to corner included, or overlap.
    #[must_use]
    pub const fn gap(self, other: Self) -> u16 {
        let across = if other.x >= self.right() {
            other.x - self.right()
        } else if self.x >= other.right() {
            self.x - other.right()
        } else {
            0
        };
        let down = if other.y >= self.bottom() {
            other.y - self.bottom()
        } else if self.y >= other.bottom() {
            self.y - other.bottom()
        } else {
            0
        };
        if across > down { across } else { down }
    }
}

/// **The tile a berth stands on**: the class of the cell under its
/// footprint's centre ([`Foot::centre`]), or `None` for a cubby or a
/// berth off its room's net.
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
    match loc {
        Loc::Hold { room, x, y } | Loc::Laid { room, x, y } => {
            let (cx, cy) = Foot::of(rooms.kind(room)?, kind, x, y)?.centre();
            rooms.tile(room, cx, cy)
        }
        Loc::Stow { .. } => None,
    }
}

/// Whether any piece rides in `cabinet`'s cubbies.
///
/// An occupied cabinet refuses to be lifted or quick-moved
/// (`Violation::Occupied`): empty it first, piece by piece — which is
/// also why cubby cargo can never be proposed by accident.
#[must_use]
pub fn cabinet_occupied(pieces: &[Piece], cabinet: u32) -> bool {
    pieces
        .iter()
        .any(|piece| matches!(piece.loc, Loc::Stow { cabinet: c, .. } if c == cabinet))
}

/// The first free cubby of `cabinet`, if any.
#[must_use]
pub fn free_cubby(pieces: &[Piece], cabinet: u32) -> Option<u8> {
    (0..CABINET_SLOTS).find(|&slot| {
        !pieces
            .iter()
            .any(|piece| piece.loc == Loc::Stow { cabinet, slot })
    })
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
/// The tile is the one under the footprint's centre ([`berth_tile`]),
/// which is why the kind is asked for: where a piece's middle is depends
/// on how big it is.
#[must_use]
pub fn player_owned(rooms: &Rooms, pieces: &[Piece], kind: Kind, loc: Loc) -> bool {
    match loc {
        // A cubby is inside a piece; whoever owns the furniture owns the
        // shelf, and no room ever stocks its goods inside your wardrobe.
        Loc::Stow { .. } => true,
        Loc::Hold { .. } | Loc::Laid { .. } => {
            let _ = pieces;
            berth_tile(rooms, kind, loc) != Some(Tile::Stock)
        }
    }
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
    /// with a notch, and otherwise a perfectly ordinary piece — it stows,
    /// trades, and resells like anything else.
    pub gnawed: bool,
    pub loc: Loc,
}

/// Which stowage rule refused a placement. One variant per rule, so the
/// renderer can flash the matching icon on a hard reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// The footprint leaves the net, crosses a hole, or bends over a
    /// fold — every placement lies wholly in one chart.
    Bounds,
    /// The footprint overlaps another piece's ground — or its standing
    /// volume: a tall floor piece shadows the wall behind it, and
    /// wall cargo cannot share that space. Touching is not overlapping:
    /// two pieces may stand flush.
    Overlap,
    /// Two volatile pieces closer than half a cell, corners included
    /// (fold seams count: the baseboard is next to the floor in the
    /// room, so it is here).
    Volatile,
    /// A cryo piece whose edge does not lie on the floor's hull edge.
    Cryo,
    /// A second suspicious piece aboard.
    Suspicious,
    /// A placement whose chart does not satisfy the kind's mount.
    Affix(Mount),
    /// A cabinet with goods in its cubbies was asked to move (or to take
    /// more than it has room for): empty it first.
    Occupied,
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

/// Whether `kind` may stand at fine `(x, y)` of `room`'s net.
///
/// `(x, y)` is the footprint's top-left corner, judged against every
/// other piece in `pieces`. The piece with `id` is ignored, so a held
/// piece never collides with its own old footprint.
#[must_use]
pub fn placement_legal(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    room: RoomId,
    x: u16,
    y: u16,
) -> bool {
    placement_check(rooms, pieces, id, kind, room, x, y).is_ok()
}

/// [`placement_legal`], but naming the rule that refused.
///
/// Checks run in a fixed order (bounds/chart, threshold, fixture, mount,
/// cryo, then per-piece overlap-and-shadow / volatile / suspicious in
/// stowage order) so the reported violation is deterministic. Every rung
/// is integer arithmetic over fine rects ([`Foot`]): no floating point
/// reaches the arbiter, so a crew in lockstep agrees on every ruling.
/// Nothing here reasons about where a body may walk: the walker passes
/// through cargo, so a berth is refused for what it collides with, never
/// for what it fences off.
pub fn placement_check(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    room: RoomId,
    x: u16,
    y: u16,
) -> Result<(), Violation> {
    let Some(host) = rooms.kind(room) else {
        return Err(Violation::Bounds);
    };
    // The CORNER's chart first, because the footprint is a function of
    // it: a wardrobe covers one cell of deck and two of wall, and which
    // it is doing here is the chart's answer, not the kind's.
    let Some(foot) = Foot::of(host, kind, x, y) else {
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
    let standing = matches!(surf, Surf::Floor);
    if matches!(kind.tag(), Some(Tag::Cryo)) && !touches_hull(host, foot) {
        return Err(Violation::Cryo);
    }
    // A standing piece's volume: the wall it shadows behind it.
    let my_shadow = if standing {
        shadows(host, foot, kind.stature())
    } else {
        Vec::new()
    };
    let suspicious_here = matches!(kind.tag(), Some(Tag::Suspicious)) && rooms.riding(room);
    for other in pieces {
        if other.id == id {
            continue;
        }
        let Loc::Hold {
            room: oroom,
            x: ox,
            y: oy,
        } = other.loc
        else {
            continue;
        };
        // At most one suspicious piece rides the ship, wherever aboard.
        if suspicious_here
            && matches!(other.kind.tag(), Some(Tag::Suspicious))
            && rooms.riding(oroom)
        {
            return Err(Violation::Suspicious);
        }
        if oroom != room {
            continue;
        }
        let Some(theirs) = Foot::of(host, other.kind, ox, oy) else {
            continue;
        };
        if foot.overlaps(theirs) {
            return Err(Violation::Overlap);
        }
        // Cross-plane volume conflicts are overlaps too: my shadow over
        // standing wall cargo, or — placing onto a wall — some floor
        // piece's shadow over me. No painting behind the wardrobe.
        if my_shadow.iter().any(|shade| shade.overlaps(theirs)) {
            return Err(Violation::Overlap);
        }
        if !standing && theirs.chart(host) == Some(Surf::Floor) {
            let shade = shadows(host, theirs, other.kind.stature());
            if shade.iter().any(|shade| shade.overlaps(foot)) {
                return Err(Violation::Overlap);
            }
        }
        // Two volatile pieces keep half a cell of clear air between
        // them, corners included. Without a buffer a sixteenth of a cell
        // would satisfy "not touching", and the rule would be a formality.
        if matches!(kind.tag(), Some(Tag::Volatile))
            && matches!(other.kind.tag(), Some(Tag::Volatile))
            && foot.gap(theirs) < FINE / 2
        {
            return Err(Violation::Volatile);
        }
    }
    Ok(())
}

/// The one chart a footprint lies wholly inside, if any — a piece bent
/// over a fold, crossing a hole, or leaving the net is nowhere. Every
/// cell the rect so much as touches must be a cell of that chart.
fn footprint_surface(host: RoomKind, foot: Foot) -> Option<Surf> {
    let (cols, rows) = host.grid();
    if foot.right() > fine(cols) || foot.bottom() > fine(rows) {
        return None;
    }
    let anchor = foot.chart(host)?;
    foot.cells()
        .all(|(cx, cy)| host.surface_of(cx, cy) == Some(anchor))
        .then_some(anchor)
}

/// Every tile class a footprint covers, any part of a cell counting.
fn footprint_tiles(host: RoomKind, foot: Foot) -> impl Iterator<Item = Tile> + use<> {
    foot.cells()
        .filter_map(move |(cx, cy)| host.tile_of(cx, cy))
}

/// The floor chart as a fine rect.
const fn floor_foot(host: RoomKind) -> Foot {
    let (fx, fy, fw, fh) = host.floor_rect();
    Foot {
        x: fine(fx),
        y: fine(fy),
        w: fine(fw),
        h: fine(fh),
    }
}

/// Whether a floor footprint's edge lies ON the floor's hull edge (any
/// side of the floor chart — every side of a room is hull). On it, not
/// near it: the cold is in the plating, and a sixteenth of a cell of air
/// is insulation. The drop's edge snap (`Sim::drop_preview`) is what
/// makes flush easy to hit.
const fn touches_hull(host: RoomKind, foot: Foot) -> bool {
    let floor = floor_foot(host);
    foot.x == floor.x
        || foot.y == floor.y
        || foot.right() == floor.right()
        || foot.bottom() == floor.bottom()
}

/// The wall a standing floor footprint shadows: for each floor edge the
/// piece stands within a cell of, the rect of wall directly behind it —
/// as wide as the piece runs along the seam, rising from the baseboard
/// through the piece's stature (three courses at most, which is all the
/// wall there is). A piece a whole cell or more off the wall leaves room
/// to hang something behind it.
fn shadows(host: RoomKind, foot: Foot, stature: u8) -> Vec<Foot> {
    let floor = floor_foot(host);
    let rise = fine(stature.min(3));
    let mut walls = Vec::new();
    if rise == 0 {
        return walls;
    }
    if foot.y.saturating_sub(floor.y) < FINE {
        // The aft wall's baseboard row sits just above the floor's aft edge.
        walls.push(Foot {
            x: foot.x,
            y: floor.y - rise,
            w: foot.w,
            h: rise,
        });
    }
    if floor.bottom().saturating_sub(foot.bottom()) < FINE {
        walls.push(Foot {
            x: foot.x,
            y: floor.bottom(),
            w: foot.w,
            h: rise,
        });
    }
    if foot.x.saturating_sub(floor.x) < FINE {
        walls.push(Foot {
            x: floor.x - rise,
            y: foot.y,
            w: rise,
            h: foot.h,
        });
    }
    if floor.right().saturating_sub(foot.right()) < FINE {
        walls.push(Foot {
            x: floor.right(),
            y: foot.y,
            w: rise,
            h: foot.h,
        });
    }
    walls
}

/// **The anchors a fitting scan tries in `room`**, sorted `(y, x)`.
///
/// Every whole-cell anchor, plus every anchor flush against the right or
/// bottom edge of some other piece already standing or laid in the room.
/// The second set is what finds a snug fit between pieces that do not
/// sit on the grid, without scanning every sixteenth of every cell —
/// which would be 256 times the work to find, nearly always, the same
/// berth. A chart's own far edges need no anchors of their own: a chart
/// is whole cells and so is every footprint, so the anchor flush against
/// one is already a whole-cell anchor.
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
                xs.push(foot.right());
                ys.push(foot.bottom());
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

/// The first berth aboard where `kind` may legally sit, in fine units.
///
/// Rooms in id order, then the [`anchors`] in `(y, x)` order — riding
/// rooms only, because "aboard" means the part of the ship that leaves
/// with you.
///
/// Shared by the shift-click quick-stow, the comet harvest, the ???
/// exchange, the hopper's banking, and every save migration that has to
/// rehome a piece — "first legal spot, even if that is a bad idea" is the
/// contract, so all of them agree on what "first" means. Coverings have
/// no occupancy berth at all ([`dress_fit`] is their scan).
#[must_use]
pub fn first_fit(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
) -> Option<(RoomId, u16, u16)> {
    if kind.covering() {
        return None;
    }
    for (room, host) in rooms.iter() {
        if !host.kind.riding() {
            continue;
        }
        for (x, y) in anchors(rooms, pieces, id, room, host.kind) {
            if placement_legal(rooms, pieces, id, kind, room, x, y) {
                return Some((room, x, y));
            }
        }
    }
    None
}

/// Whether covering `kind` may be laid with its top-left corner at fine
/// `(x, y)` of `room`'s net.
///
/// The dressing layer's own [`placement_check`], reusing the violation
/// ladder whole and consulting every other piece. Checks run in a fixed
/// order (bounds, threshold, fixture, surface, then per-piece dressing
/// overlap / pinned-under-occupancy) so the reported violation is
/// deterministic.
pub fn dressing_check(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    room: RoomId,
    x: u16,
    y: u16,
) -> Result<(), Violation> {
    debug_assert!(kind.covering(), "dressing_check is for coverings only");
    let Some(host) = rooms.kind(room) else {
        return Err(Violation::Bounds);
    };
    let Some(foot) = Foot::of(host, kind, x, y) else {
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
    for other in pieces {
        if other.id == id {
            continue;
        }
        let Some((oroom, theirs)) = Foot::at(rooms, other) else {
            continue;
        };
        if oroom != room || !theirs.overlaps(foot) {
            continue;
        }
        match other.loc {
            // One dressing per point of the room.
            Loc::Laid { .. } => return Err(Violation::Overlap),
            // No sliding a dressing under standing cargo: the pinned
            // rule, symmetric with the lift refusal in `laid_pinned`.
            Loc::Hold { .. } => return Err(Violation::Occupied),
            Loc::Stow { .. } => {}
        }
    }
    Ok(())
}

/// The first anchor (riding rooms in id order, then the [`anchors`] in
/// `(y, x)` order) where covering `kind` may be laid — the dressing
/// layer's [`first_fit`]. Coordinates are fine units.
#[must_use]
pub fn dress_fit(
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
) -> Option<(RoomId, u16, u16)> {
    for (room, host) in rooms.iter() {
        if !host.kind.riding() {
            continue;
        }
        for (x, y) in anchors(rooms, pieces, id, room, host.kind) {
            if dressing_check(rooms, pieces, id, kind, room, x, y).is_ok() {
                return Some((room, x, y));
            }
        }
    }
    None
}

/// Whether occupancy cargo stands on `piece`'s laid footprint.
///
/// A pinned dressing refuses to lift (`Violation::Occupied`) — move the
/// couch, then roll the rug — mirroring [`dressing_check`]'s refusal to
/// lay beneath one.
#[must_use]
pub fn laid_pinned(rooms: &Rooms, pieces: &[Piece], piece: &Piece) -> bool {
    if !matches!(piece.loc, Loc::Laid { .. }) {
        return false;
    }
    let Some((room, foot)) = Foot::at(rooms, piece) else {
        return false;
    };
    pieces.iter().any(|other| {
        other.id != piece.id
            && matches!(other.loc, Loc::Hold { .. })
            && Foot::at(rooms, other)
                .is_some_and(|(at, theirs)| at == room && theirs.overlaps(foot))
    })
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
        && !pieces.iter().any(|other| {
            other.id != piece.id && other.kind == piece.kind && staying(rooms, pieces, other)
        })
}

/// Whether a piece's berth is one it would still hold after a launch.
///
/// The player's own, in a room that rides, and not scheduled for the
/// fire: the possession half of the vital rule. The tile asked is the
/// one under the footprint's centre ([`berth_tile`]).
#[must_use]
pub fn staying(rooms: &Rooms, pieces: &[Piece], piece: &Piece) -> bool {
    if !player_owned(rooms, pieces, piece.kind, piece.loc) {
        return false;
    }
    match piece.loc {
        Loc::Stow { .. } => true,
        Loc::Hold { room, .. } | Loc::Laid { room, .. } => {
            rooms.riding(room) && berth_tile(rooms, piece.kind, piece.loc) != Some(Tile::Consume)
        }
    }
}

/// Whether `kind` is a lamp — one of the three affixed fixtures that cast
/// light while berthed.
#[must_use]
pub const fn lamp(kind: Kind) -> bool {
    matches!(kind, Kind::CeilingLamp | Kind::WallLamp | Kind::FloorLamp)
}

/// Whether `piece` is a lamp, burning.
///
/// Lamps are lit while they stand in a room and nowhere else: boxed in
/// a cabinet cubby they are dark. Everything lighting touches — the rat's
/// fear, the well-lit art bonus, any frontend halo — reads lamp state
/// through this one predicate.
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
/// Lit means within reach of — never wholly inside — some lit lamp's
/// footprint OR some laid luminous coat's: a Chebyshev gap of less than
/// one cell ([`Foot::gap`]). Corners count; under the grid they did not,
/// and a free berth has no corners worth exempting, because a lamp a
/// sixteenth past a crate's corner is not darker than one a sixteenth
/// past its edge. Light does not cross a seam: a lamp lights its own
/// room. Everything light touches — the rat's fear, the seedlings'
/// bloom, the hold painting's spotlight — reads through this one
/// predicate; the well-lit-art price bonus deliberately does not on the
/// offer area (a coat is ambiance, not gallery lighting).
///
/// `host` is the room's own kind, because a lamp's footprint is a
/// question about the chart it stands on (`plan`) and the caller
/// already knows whose room this is.
#[must_use]
pub fn lit_within_reach(host: RoomKind, pieces: &[Piece], room: RoomId, target: Foot) -> bool {
    pieces.iter().any(|piece| {
        let (source, lroom, lx, ly) = match piece.loc {
            Loc::Hold {
                room: r,
                x: px,
                y: py,
            } => (lamp_lit(piece), r, px, py),
            Loc::Laid {
                room: r,
                x: px,
                y: py,
            } => (piece.kind == Kind::LuminousPaint, r, px, py),
            Loc::Stow { .. } => return false,
        };
        source
            && lroom == room
            && Foot::of(host, piece.kind, lx, ly)
                .is_some_and(|light| light.gap(target) < FINE && !light.contains(target))
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

    /// A board of pieces berthed in the cabin at the given cells, ids
    /// counting up from 0.
    fn board(stowed: &[(Kind, u8, u8)]) -> Vec<Piece> {
        stowed
            .iter()
            .enumerate()
            .map(|(i, &(kind, x, y))| Piece {
                id: i as u32,
                kind,
                variant: 0,
                gnawed: false,
                loc: Loc::Hold {
                    room: CABIN,
                    x: fine(x),
                    y: fine(y),
                },
            })
            .collect()
    }

    /// The arbiter asked of a whole-cell anchor, against a [`board`];
    /// the candidate takes the next free id after it.
    fn check(stowed: &[(Kind, u8, u8)], kind: Kind, x: u8, y: u8) -> Result<(), Violation> {
        check_fine(&board(stowed), kind, fine(x), fine(y))
    }

    /// The arbiter asked of a fine anchor, against any board.
    fn check_fine(pieces: &[Piece], kind: Kind, x: u16, y: u16) -> Result<(), Violation> {
        placement_check(&ship(), pieces, pieces.len() as u32, kind, CABIN, x, y)
    }

    #[test]
    fn bounds_rule_accepts_inside_and_names_offgrid() {
        // RationBricks is 2x2: fits on the floor at (4, 3); bent over the
        // starboard fold at (10, 4); off the net entirely at (21, 3).
        assert_eq!(check(&[], Kind::RationBricks, 4, 3), Ok(()));
        assert_eq!(
            check(&[], Kind::RationBricks, 10, 4),
            Err(Violation::Bounds)
        );
        assert_eq!(
            check(&[], Kind::RationBricks, 21, 3),
            Err(Violation::Bounds)
        );
    }

    #[test]
    fn overlap_rule_accepts_beside_and_names_collision() {
        let stowed = [(Kind::PerfumeVial, 5, 5)];
        assert_eq!(check(&stowed, Kind::Seedlings, 6, 5), Ok(()));
        assert_eq!(
            check(&stowed, Kind::Seedlings, 5, 5),
            Err(Violation::Overlap)
        );
        // Multi-cell: ScrapAlloy anchored at (4, 5) covers (5, 5) too.
        assert_eq!(
            check(&stowed, Kind::ScrapAlloy, 4, 5),
            Err(Violation::Overlap)
        );
    }

    /// Nothing berths on a threshold: an aperture's cells belong to two
    /// rooms at once, and a detach would have to pick which grid kept
    /// the cargo. The doorway stays clear for a reason that survives.
    #[test]
    fn the_threshold_rule_keeps_every_aperture_clear() {
        // The cabin's starboard doorway, its aft doorway, its hatch.
        assert_eq!(
            check(&[], Kind::PerfumeVial, 11, 3),
            Err(Violation::Threshold)
        );
        assert_eq!(check(&[], Kind::WallLamp, 3, 1), Err(Violation::Threshold));
        assert_eq!(
            check(&[], Kind::PerfumeVial, 9, 7),
            Err(Violation::Threshold)
        );
        // And a dressing cannot be laid across one either.
        let rooms = ship();
        assert_eq!(
            dressing_check(&rooms, &[], 9, Kind::PaintTin, CABIN, fine(9), fine(7)),
            Err(Violation::Threshold)
        );
    }

    #[test]
    fn heavy_lies_dormant_and_the_walls_refuse_plain_cargo() {
        assert_eq!(check(&[], Kind::GildedIdol, 3, 3), Ok(()));
        assert_eq!(check(&[], Kind::GildedIdol, 5, 5), Ok(()));
        // Lifted onto a wall, the mount law refuses (port chart, clear
        // of the doorway the aperture punches through it).
        assert_eq!(
            check(&[], Kind::GildedIdol, 0, 8),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn volatile_rule_accepts_gapped_and_names_adjacency() {
        let stowed = [(Kind::GasCanister, 3, 7)];
        assert_eq!(check(&stowed, Kind::GasCanister, 3, 5), Ok(()));
        assert_eq!(
            check(&stowed, Kind::GasCanister, 3, 6),
            Err(Violation::Volatile)
        );
        assert_eq!(
            check(&stowed, Kind::GasCanister, 4, 6),
            Err(Violation::Volatile)
        );
        // Corner to corner is too close now: half a cell of clear air,
        // every way.
        assert_eq!(
            check(&stowed, Kind::GasCanister, 5, 6),
            Err(Violation::Volatile)
        );
        assert_eq!(check(&stowed, Kind::GasCanister, 6, 5), Ok(()));
    }

    #[test]
    fn cryo_rule_accepts_edge_and_names_interior() {
        assert_eq!(check(&[], Kind::CryoCore, 3, 4), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, 4, 9), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, 5, 5), Err(Violation::Cryo));
    }

    #[test]
    fn suspicious_rule_accepts_one_and_names_a_second() {
        assert_eq!(check(&[], Kind::SuspiciousCrate, 3, 3), Ok(()));
        let stowed = [(Kind::SuspiciousCrate, 3, 3)];
        assert_eq!(
            check(&stowed, Kind::SuspiciousCrate, 5, 5),
            Err(Violation::Suspicious)
        );
    }

    #[test]
    fn the_floor_takes_cargo_anywhere_a_body_could_stand() {
        // The walker passes through cargo, so the floor keeps no
        // reserved lanes: a wall of cargo may close across the room.
        assert_eq!(check(&[], Kind::PerfumeVial, 10, 3), Ok(()));
        assert_eq!(check(&[], Kind::PerfumeVial, 6, 7), Ok(()));
        let wall: Vec<(Kind, u8, u8)> = (3..9).map(|y| (Kind::PerfumeVial, 4, y)).collect();
        assert_eq!(check(&wall, Kind::PerfumeVial, 4, 9), Ok(()));
        assert_eq!(check(&wall, Kind::PerfumeVial, 6, 5), Ok(()));
    }

    #[test]
    fn tall_floor_cargo_shadows_the_wall_behind_it() {
        // The cabinet (stature 2) against the aft baseboard blocks the
        // two wall rows behind its cell; a painting may not hang there.
        let stowed = [(Kind::Cabinet, 6, 3)];
        assert_eq!(
            check(&stowed, Kind::Painting, 5, 1),
            Err(Violation::Overlap)
        );
        // Two columns over, the wall is clear.
        assert_eq!(check(&stowed, Kind::Painting, 7, 1), Ok(()));
        // And symmetrically.
        let hung = [(Kind::Painting, 7, 1)];
        assert_eq!(check(&hung, Kind::Cabinet, 7, 3), Err(Violation::Overlap));
        assert_eq!(check(&hung, Kind::Cabinet, 5, 3), Ok(()));
    }

    #[test]
    fn affix_rule_accepts_the_mount_surface_and_names_the_miss() {
        assert_eq!(check(&[], Kind::CeilingLamp, 16, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::CeilingLamp, 5, 1),
            Err(Violation::Affix(Mount::Ceiling))
        );
        assert_eq!(check(&[], Kind::WallLamp, 5, 1), Ok(()));
        assert_eq!(check(&[], Kind::WallLamp, 1, 6), Ok(()));
        assert_eq!(
            check(&[], Kind::WallLamp, 5, 5),
            Err(Violation::Affix(Mount::Wall))
        );
        assert_eq!(check(&[], Kind::Couch, 4, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::Couch, 4, 0),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn the_floor_lamp_stands_on_the_floor_and_never_across_a_fold() {
        assert_eq!(check(&[], Kind::FloorLamp, 4, 4), Ok(()));
        assert_eq!(check(&[], Kind::FloorLamp, 4, 8), Ok(()));
        assert_eq!(
            check(&[], Kind::FloorLamp, 5, 0),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(check(&[], Kind::FloorLamp, 5, 2), Err(Violation::Bounds));
    }

    #[test]
    fn the_painting_hangs_on_any_wall_but_never_a_hole() {
        assert_eq!(check(&[], Kind::Painting, 5, 1), Ok(()));
        // A flank too, now that the footprint is stated in the wall's
        // own frame: two cells along the port wall, one course tall.
        assert_eq!(check(&[], Kind::Painting, 0, 6), Ok(()));
        assert_eq!(
            check(&[], Kind::Painting, 4, 4),
            Err(Violation::Affix(Mount::Wall))
        );
        assert_eq!(check(&[], Kind::Painting, 13, 9), Err(Violation::Bounds));
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
    /// A footprint is stated in the wall's own frame now, so the claim
    /// can be made positively: on EVERY wall of EVERY room, a kind
    /// covers its own `across` along the wall and its own `tall` up it.
    /// Which sheet axis is which comes off the net's own fold
    /// ([`down_the_wall`]) and not off the arbiter's table, so the guard
    /// is not the implementation read back.
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
                        let (w, h) = plan(host, kind, x, y).expect("a wall cell has a plan");
                        let sideways = courses_climb_the_sheets_x(host, x, y);
                        let (along, up) = if sideways { (h, w) } else { (w, h) };
                        assert_eq!(
                            (along, up),
                            (across, tall),
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
            assert_eq!(check(&[], kind, 5, 1), Ok(()), "{kind:?} on the aft wall");
            assert_eq!(check(&[], kind, 0, 5), Ok(()), "{kind:?} on the port wall");
        }
        // A square footprint still cannot tell the flanks from the ends.
        assert_eq!(check(&[], Kind::Porthole, 1, 8), Ok(()));
        assert_eq!(check(&[], Kind::ChartTank, 11, 5), Ok(()));
    }

    #[test]
    fn affix_is_checked_before_the_per_piece_scan() {
        let stowed = [(Kind::RationBricks, 4, 4)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 4, 4),
            Err(Violation::Affix(Mount::Wall))
        );
        let stowed = [(Kind::Painting, 5, 1)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 5, 1),
            Err(Violation::Overlap)
        );
    }

    #[test]
    fn lamps_are_lit_only_in_a_room_and_light_their_neighbours() {
        assert!(lamp(Kind::CeilingLamp) && lamp(Kind::WallLamp) && lamp(Kind::FloorLamp));
        assert!(!lamp(Kind::Couch) && !lamp(Kind::Painting) && !lamp(Kind::PerfumeVial));

        let pieces = board(&[(Kind::CeilingLamp, 16, 4)]);
        assert!(lamp_lit(&pieces[0]));
        assert!(lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 15, 4));
        assert!(lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 17, 4));
        assert!(lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 16, 5));
        assert!(!lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 16, 4));
        // Corners count: light reaches a cell's width, every way.
        assert!(lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 15, 3));
        assert!(!lit_adjacent(RoomKind::Cabin, &pieces, CABIN, 18, 4));
        // Light does not cross a seam.
        assert!(!lit_adjacent(RoomKind::Cabin, &pieces, 1, 15, 4));

        // A standing lamp lights from the ONE cell of deck it occupies,
        // however tall it is: a height is spent up the wall behind it
        // (`Kind::stature`) and never across the floor beside it.
        let tall = board(&[(Kind::FloorLamp, 3, 4)]);
        assert!(lit_adjacent(RoomKind::Cabin, &tall, CABIN, 4, 4));
        assert!(lit_adjacent(RoomKind::Cabin, &tall, CABIN, 3, 3));
        assert!(lit_adjacent(RoomKind::Cabin, &tall, CABIN, 3, 5));
        assert!(lit_adjacent(RoomKind::Cabin, &tall, CABIN, 4, 5), "corner");
        assert!(lit_adjacent(RoomKind::Cabin, &tall, CABIN, 4, 3), "corner");
        assert!(
            !lit_adjacent(RoomKind::Cabin, &tall, CABIN, 5, 4),
            "a cell away"
        );

        // Boxed in a cubby a lamp is dark, and non-lamps light nothing.
        let boxed = Piece {
            id: 9,
            kind: Kind::FloorLamp,
            variant: 0,
            gnawed: false,
            loc: Loc::Stow {
                cabinet: 0,
                slot: 0,
            },
        };
        assert!(!lamp_lit(&boxed));
        assert!(!lit_adjacent(RoomKind::Cabin, &[boxed], CABIN, 4, 4));
        let art = board(&[(Kind::Painting, 5, 1)]);
        assert!(!lit_adjacent(RoomKind::Cabin, &art, CABIN, 7, 1));
    }

    #[test]
    fn stowable_is_small_and_neither_cold_nor_suspect() {
        for kind in [
            Kind::PerfumeVial,
            Kind::Seedlings,
            Kind::MysteriousCrate,
            Kind::Fluff,
            Kind::CeilingLamp,
            Kind::WallLamp,
        ] {
            assert!(stowable(kind), "{kind:?} should stow");
        }
        for kind in [
            Kind::CryoCore,
            Kind::CometIce,
            Kind::GildedIdol,
            Kind::Couch,
            Kind::Cabinet,
            Kind::SuspiciousCrate,
            // A hole in the hull is not a thing you put in a drawer,
            // however small the hole is.
            Kind::Porthole,
        ] {
            assert!(!stowable(kind), "{kind:?} should refuse the cubby");
        }
    }

    /// The window family, and what makes it one: every size mounts on a
    /// wall, none of it burns, none of it stows, and every size fits on
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
            // A hull launches with ONE window and buys the rest, which
            // is what keeps an old save from being handed a bay window
            // it never paid the freight on.
            assert_eq!(
                kind.instrument(),
                kind == Kind::Window,
                "{kind:?} disagrees about what a hull comes with"
            );
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
                        ) && placement_check(&ship(), &[], 0, kind, CABIN, fine(x), fine(y)).is_ok()
                    })
                });
                assert!(fits, "{kind:?} fits no wall of a {host:?}");
            }
        }
    }

    #[test]
    fn cubbies_fill_first_free_and_report_occupancy() {
        let cabinet = 7_u32;
        let mut pieces = vec![Piece {
            id: cabinet,
            kind: Kind::Cabinet,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x: fine(4),
                y: fine(4),
            },
        }];
        assert!(!cabinet_occupied(&pieces, cabinet));
        assert_eq!(free_cubby(&pieces, cabinet), Some(0));
        for slot in 0..CABINET_SLOTS {
            pieces.push(Piece {
                id: 100 + u32::from(slot),
                kind: Kind::PerfumeVial,
                variant: 0,
                gnawed: false,
                loc: Loc::Stow { cabinet, slot },
            });
        }
        assert!(cabinet_occupied(&pieces, cabinet));
        assert_eq!(free_cubby(&pieces, cabinet), None);
        assert!(!cabinet_occupied(&pieces, 8));
        assert_eq!(free_cubby(&pieces, 8), Some(0));
    }

    #[test]
    fn dressing_rules_cover_surface_overlap_and_pinning() {
        let rooms = ship();
        let laid = |pieces: &[Piece], kind, x, y| {
            dressing_check(&rooms, pieces, 9, kind, CABIN, fine(x), fine(y))
        };
        assert_eq!(laid(&[], Kind::Rug, 4, 7), Ok(()));
        assert_eq!(
            laid(&[], Kind::Rug, 5, 1),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(laid(&[], Kind::Rug, 10, 7), Err(Violation::Bounds));
        assert_eq!(laid(&[], Kind::PaintTin, 5, 0), Ok(()));
        assert_eq!(laid(&[], Kind::LuminousPaint, 4, 4), Ok(()));
        let mut pieces = vec![Piece {
            id: 0,
            kind: Kind::Rug,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid {
                room: CABIN,
                x: fine(4),
                y: fine(7),
            },
        }];
        assert_eq!(laid(&pieces, Kind::PaintTin, 5, 7), Err(Violation::Overlap));
        assert_eq!(laid(&pieces, Kind::PaintTin, 3, 7), Ok(()));
        pieces.push(Piece {
            id: 1,
            kind: Kind::Couch,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x: fine(3),
                y: fine(7),
            },
        });
        assert_eq!(
            laid(&pieces, Kind::PaintTin, 3, 7),
            Err(Violation::Occupied)
        );
        let rug = Piece {
            id: 2,
            kind: Kind::Rug,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid {
                room: CABIN,
                x: fine(4),
                y: fine(7),
            },
        };
        let couch = Piece {
            id: 3,
            kind: Kind::Couch,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x: fine(5),
                y: fine(7),
            },
        };
        assert!(laid_pinned(&rooms, &[rug, couch], &rug));
        assert!(!laid_pinned(&rooms, &[rug], &rug));
        assert_eq!(first_fit(&rooms, &[], 9, Kind::Rug), None);
        assert_eq!(
            dress_fit(&rooms, &[rug, couch], 9, Kind::Rug),
            Some((CABIN, fine(3), fine(3)))
        );
    }

    #[test]
    fn luminous_coats_light_their_neighbours() {
        let coat = Piece {
            id: 0,
            kind: Kind::LuminousPaint,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid {
                room: CABIN,
                x: fine(5),
                y: fine(1),
            },
        };
        assert!(lit_adjacent(RoomKind::Cabin, &[coat], CABIN, 6, 1));
        assert!(lit_adjacent(RoomKind::Cabin, &[coat], CABIN, 5, 0));
        assert!(
            !lit_adjacent(RoomKind::Cabin, &[coat], CABIN, 5, 1),
            "never inside"
        );
        assert!(
            lit_adjacent(RoomKind::Cabin, &[coat], CABIN, 6, 0),
            "corners count"
        );
        assert!(
            !lit_adjacent(RoomKind::Cabin, &[coat], CABIN, 7, 1),
            "a cell away"
        );
        let tin = Piece {
            id: 1,
            kind: Kind::PaintTin,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid {
                room: CABIN,
                x: fine(7),
                y: fine(1),
            },
        };
        assert!(!lit_adjacent(RoomKind::Cabin, &[tin], CABIN, 6, 1));
    }

    /// Ownership is a function of tile class, and nothing else.
    #[test]
    fn ownership_reads_the_tile_class() {
        let mut rooms = Rooms::new();
        let trade = rooms
            .spawn(RoomKind::Trade, CABIN)
            .expect("a trade room attaches");
        let at = |x, y| Loc::Hold {
            room: trade,
            x: fine(x),
            y: fine(y),
        };
        let owned = |loc| player_owned(&rooms, &[], Kind::PerfumeVial, loc);
        // The trade room's aft floor row is its own stock; its front
        // floor row is the chalked offer square; the deck between is
        // ordinary, and so is everything aboard.
        assert!(!owned(at(5, 3)));
        assert!(owned(at(3, 6)));
        assert!(owned(at(3, 4)));
        // **Except the two cells its own door stands on**, which the
        // doorstep law hands back to the room's ordinary class: a body
        // walking in lands on deck it may use, and the shopfront starts
        // beside the door rather than under it.
        assert!(owned(at(3, 3)));
        assert!(owned(at(4, 3)));
        assert!(owned(Loc::Hold {
            room: CABIN,
            x: fine(4),
            y: fine(4)
        }));
    }

    #[test]
    fn held_piece_ignores_its_own_footprint() {
        let rooms = ship();
        let pieces = board(&[(Kind::RationBricks, 4, 4)]);
        assert_eq!(
            placement_check(
                &rooms,
                &pieces,
                0,
                Kind::RationBricks,
                CABIN,
                fine(4),
                fine(4)
            ),
            Ok(())
        );
        assert_eq!(
            placement_check(
                &rooms,
                &pieces,
                0,
                Kind::RationBricks,
                CABIN,
                fine(5),
                fine(4)
            ),
            Ok(())
        );
    }

    /// A piece at fine `(x, y)` of the cabin, id `id`.
    const fn at(id: u32, kind: Kind, x: u16, y: u16) -> Piece {
        Piece {
            id,
            kind,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold { room: CABIN, x, y },
        }
    }

    /// The footprint's arithmetic, stated once: the cells a rect a
    /// sixteenth off the grid stands on, the cell under its middle, and
    /// the gap between two of them.
    #[test]
    fn a_foot_covers_what_it_touches_and_reads_its_middle() {
        let host = RoomKind::Cabin;
        // A couch is two cells of deck; three sixteenths off the grid it
        // stands on three.
        let couch = Foot::of(host, Kind::Couch, fine(4) + 3, fine(4)).expect("on the deck");
        assert_eq!(couch.cells().collect::<Vec<_>>(), [(4, 4), (5, 4), (6, 4)]);
        assert_eq!(couch.centre(), (5, 4));
        // At a whole-cell anchor the middle lies on a seam and reads the
        // anchor cell, which is the cell the grid's rules always read.
        let square = Foot::of(host, Kind::Couch, fine(4), fine(4)).expect("on the deck");
        assert_eq!(square.centre(), (4, 4));
        for kind in Kind::ALL {
            let foot = Foot::of(host, kind, fine(4), fine(4)).expect("on the deck");
            assert_eq!(foot.centre(), (4, 4), "{kind:?} reads its anchor");
        }
        // Gaps are Chebyshev: the larger of across and down.
        let a = Foot::cell(4, 4);
        assert_eq!(a.gap(Foot::cell(5, 4)), 0, "flush");
        assert_eq!(a.gap(Foot::cell(5, 5)), 0, "corner to corner");
        assert_eq!(a.gap(Foot::cell(6, 5)), FINE);
        assert!(!a.overlaps(Foot::cell(5, 4)), "touching is not overlapping");
        assert!(a.contains(a) && !a.contains(couch));
    }

    #[test]
    fn flush_neighbours_stand_and_a_sixteenth_of_overlap_is_refused() {
        let vial = [at(0, Kind::PerfumeVial, fine(5), fine(5))];
        assert_eq!(check_fine(&vial, Kind::Seedlings, fine(6), fine(5)), Ok(()));
        assert_eq!(check_fine(&vial, Kind::Seedlings, fine(4), fine(5)), Ok(()));
        assert_eq!(
            check_fine(&vial, Kind::Seedlings, fine(6) - 1, fine(5)),
            Err(Violation::Overlap)
        );
        assert_eq!(
            check_fine(&vial, Kind::Seedlings, fine(4) + 1, fine(5) + 15),
            Err(Violation::Overlap)
        );
        // Off the grid entirely, flush on two sides at once.
        let pair = [
            at(0, Kind::PerfumeVial, fine(4) + 5, fine(5)),
            at(1, Kind::PerfumeVial, fine(4) + 5 + FINE, fine(5) + FINE),
        ];
        assert_eq!(
            check_fine(&pair, Kind::Seedlings, fine(4) + 5 + FINE, fine(5)),
            Ok(())
        );
    }

    #[test]
    fn volatile_cargo_keeps_half_a_cell_of_clear_air() {
        // A gas canister is two cells across the deck.
        let can = [at(0, Kind::GasCanister, fine(4), fine(5))];
        let below = fine(6);
        assert_eq!(
            check_fine(&can, Kind::GasCanister, fine(4), below + 7),
            Err(Violation::Volatile)
        );
        assert_eq!(
            check_fine(&can, Kind::GasCanister, fine(4), below + 8),
            Ok(())
        );
        // Corners count: the gap is the larger of across and down.
        let past = fine(6);
        assert_eq!(
            check_fine(&can, Kind::GasCanister, past + 7, below + 7),
            Err(Violation::Volatile)
        );
        assert_eq!(
            check_fine(&can, Kind::GasCanister, past + 8, below + 7),
            Ok(())
        );
    }

    #[test]
    fn cryo_stands_flush_on_the_hull_and_not_a_sixteenth_off() {
        let (fx, fy, fw, _) = RoomKind::Cabin.floor_rect();
        let (left, top, right) = (fine(fx), fine(fy), fine(fx + fw));
        assert_eq!(check_fine(&[], Kind::CryoCore, left, fine(6)), Ok(()));
        assert_eq!(
            check_fine(&[], Kind::CryoCore, left + 1, fine(6)),
            Err(Violation::Cryo)
        );
        assert_eq!(
            check_fine(&[], Kind::CryoCore, right - FINE, fine(4) + 3),
            Ok(())
        );
        assert_eq!(check_fine(&[], Kind::CryoCore, fine(6) + 3, top), Ok(()));
        assert_eq!(
            check_fine(&[], Kind::CryoCore, fine(6) + 3, top + 1),
            Err(Violation::Cryo)
        );
    }

    /// A tall piece shadows the wall behind it while it stands within a
    /// cell of that wall, and a whole cell out it leaves room to hang
    /// something behind it.
    #[test]
    fn a_shadow_reaches_a_cell_off_the_wall_and_no_further() {
        let (_, fy, _, _) = RoomKind::Cabin.floor_rect();
        let wall = fine(fy);
        let near = [at(0, Kind::Cabinet, fine(6), wall + 15)];
        assert_eq!(
            check_fine(&near, Kind::Painting, fine(5), fine(1)),
            Err(Violation::Overlap)
        );
        let clear = [at(0, Kind::Cabinet, fine(6), wall + 16)];
        assert_eq!(check_fine(&clear, Kind::Painting, fine(5), fine(1)), Ok(()));
        // And symmetrically, hanging first.
        let hung = [at(0, Kind::Painting, fine(5), fine(1))];
        assert_eq!(
            check_fine(&hung, Kind::Cabinet, fine(6), wall + 15),
            Err(Violation::Overlap)
        );
        assert_eq!(check_fine(&hung, Kind::Cabinet, fine(6), wall + 16), Ok(()));
        // The shadow is as wide as the piece runs along the seam: a
        // cabinet slid a sixteenth past the painting's end clears it.
        assert_eq!(
            check_fine(&hung, Kind::Cabinet, fine(7), wall),
            Ok(()),
            "flush beside the painting's end"
        );
        assert_eq!(
            check_fine(&hung, Kind::Cabinet, fine(7) - 1, wall),
            Err(Violation::Overlap)
        );
    }

    #[test]
    fn light_reaches_a_cell_and_not_a_sixteenth_more() {
        let lamp = [at(0, Kind::FloorLamp, fine(4), fine(5))];
        let host = RoomKind::Cabin;
        let vial = |x, y| Foot::of(host, Kind::PerfumeVial, x, y).expect("on the deck");
        assert!(lit_within_reach(
            host,
            &lamp,
            CABIN,
            vial(fine(5) + 15, fine(5))
        ));
        assert!(!lit_within_reach(
            host,
            &lamp,
            CABIN,
            vial(fine(6), fine(5))
        ));
        assert!(lit_within_reach(
            host,
            &lamp,
            CABIN,
            vial(fine(5) + 15, fine(6) + 15)
        ));
        assert!(!lit_within_reach(
            host,
            &lamp,
            CABIN,
            vial(fine(5) + 15, fine(7))
        ));
        // Never inside.
        assert!(!lit_within_reach(
            host,
            &lamp,
            CABIN,
            vial(fine(4), fine(5))
        ));
    }

    /// `first_fit` tries the anchors flush against what is already
    /// standing, so a gap that is exactly a crate wide but sits off the
    /// grid is found — before any whole-cell anchor further along.
    #[test]
    fn first_fit_finds_a_snug_gap_off_the_grid() {
        let (fx, fy, _, _) = RoomKind::Cabin.floor_rect();
        let (left, top) = (fine(fx), fine(fy));
        // Two vials on the first row of deck, a vial's width apart, both
        // half a cell off the grid: every whole-cell anchor up to the
        // second one's far side overlaps one of them.
        let board = [
            at(0, Kind::PerfumeVial, left + 8, top),
            at(1, Kind::PerfumeVial, left + 8 + 2 * FINE, top),
        ];
        assert_eq!(
            first_fit(&ship(), &board, 9, Kind::PerfumeVial),
            Some((CABIN, left + 8 + FINE, top))
        );
    }

    /// **Every yes the arbiter says is ground a piece can stand on**, at
    /// every whole-cell anchor and a fixed sample of sixteenths off it
    /// ([`FRACTIONS`]) on each axis: the footprint lies in its net, on
    /// one chart, and over no cell a refusing class keeps.
    #[test]
    fn every_accepted_berth_lies_on_one_chart_clear_of_refusals() {
        for host in super::super::room::ROOM_KINDS {
            let rooms = Rooms::root(host);
            let (cols, rows) = host.grid();
            for kind in Kind::ALL {
                for y in 0..rows {
                    for x in 0..cols {
                        for (dx, dy) in FRACTIONS
                            .into_iter()
                            .flat_map(|dx| FRACTIONS.into_iter().map(move |dy| (dx, dy)))
                        {
                            let (fx, fy) = (fine(x) + dx, fine(y) + dy);
                            let ruling = if kind.covering() {
                                dressing_check(&rooms, &[], 0, kind, 0, fx, fy)
                            } else {
                                placement_check(&rooms, &[], 0, kind, 0, fx, fy)
                            };
                            if ruling.is_err() {
                                continue;
                            }
                            let foot = Foot::of(host, kind, fx, fy).expect("accepted");
                            let chart = foot.chart(host);
                            for (cx, cy) in foot.cells() {
                                assert_eq!(
                                    host.surface_of(cx, cy),
                                    chart,
                                    "{kind:?} at ({fx}, {fy})"
                                );
                                assert!(
                                    !matches!(
                                        host.tile_of(cx, cy),
                                        Some(Tile::Threshold | Tile::Fixture)
                                    ),
                                    "{kind:?} at ({fx}, {fy}) of a {host:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
