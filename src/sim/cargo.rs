//! Cargo pieces: what they are, where they sit, and the stowage rules.
//!
//! A [`Piece`] is one draggable object. Its [`Loc`] says which surface it is
//! on — the ship's hold grid or one of the barter panel's shelves and pads —
//! and [`placement_check`] is the single arbiter of whether a piece may sit
//! at a given hold cell. The renderer and the drag logic both defer to it, so
//! there is exactly one opinion about what fits, and a failure names the
//! [`Violation`] so the frontend can flash the right icon.

use super::layout::{FLOOR, GRID_COLS, GRID_ROWS, Surf, surface_of};

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
    /// A hanging shade for the hold's gantry. Lit while stowed, like
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
}

/// Number of cargo kinds.
pub const KIND_COUNT: usize = 30;

/// Cosmetic variant rolls per kind, for the renderer to vary sprites with.
/// The persistent run RNG is spent on these and nothing else.
pub(crate) const VARIANTS: u8 = 4;

/// Special handling a kind demands in the hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// Weighty. Dormant since the room grid put all standing cargo on
    /// the floor (heavy-rides-low had nothing left to refuse); kept as
    /// kind data for the stacking rules to consume (`supports:top`,
    /// BAY.md) — nothing heavy will ride on top of anything.
    Heavy,
    /// No two volatile pieces may sit orthogonally adjacent.
    Volatile,
    /// Must touch the hold's outer edge.
    Cryo,
    /// At most one suspicious piece aboard, and hauling it has consequences.
    Suspicious,
    /// A fixture: its footprint must touch the named room surface.
    Affix(Mount),
    /// A dressing: aboard, it lays *into* the room (`Loc::Laid`)
    /// instead of occupying cells. `Some(mount)` restricts which
    /// surface it covers; `None` coats anywhere.
    Covering(Option<Mount>),
}

/// The plane class a kind stands on.
///
/// Under the room net (`layout::surface_of`), every placement lies wholly
/// in one chart and that chart's class must match the kind's mount — the
/// room-grid placement law (BAY.md): floor unless otherwise specified,
/// walls for paintings and instruments, ceilings for hanging lamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    Ceiling,
    Floor,
    Wall,
}

/// Whether chart `surf` satisfies `mount`. Any of the four walls is
/// "the wall"; nobody hangs a painting on a compass heading.
const fn mount_accepts(mount: Mount, surf: Surf) -> bool {
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
    ];

    /// Footprint in hold cells, `(w, h)`.
    #[must_use]
    pub const fn cells(self) -> (u8, u8) {
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
            | Self::LaunchLever => (1, 1),
            Self::GildedIdol | Self::BrinePearls | Self::FloorLamp | Self::Cabinet => (1, 2),
            Self::RationBricks
            | Self::SuspiciousCrate
            | Self::VeryMysteriousCrate
            | Self::ChartTank => (2, 2),
            Self::ScrapAlloy
            | Self::GasCanister
            | Self::Couch
            | Self::Painting
            | Self::Rug
            | Self::Window => (2, 1),
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
    /// painting behind the wardrobe). Fully re-authored 3D extents are
    /// deferred (BAY.md); until then a kind stands as tall as its old
    /// bas-relief silhouette was.
    #[must_use]
    pub const fn stature(self) -> u8 {
        self.cells().1
    }

    /// Whether this kind is an operational instrument the ship cannot
    /// function without: the LAST one of a vital kind in the player's
    /// possession refuses every exit ceremony (the give pads, the
    /// burner hopper, the casino's wager) — `Violation::Vital`, checked
    /// in `resolve_drop`. Spares trade freely; stations occasionally
    /// stock used instruments, which is its own little economy.
    #[must_use]
    pub const fn vital(self) -> bool {
        matches!(self, Self::ChartTank | Self::LaunchLever)
    }

    /// Whether this kind is one of the ship's instruments — the wall
    /// fittings every hull launches with. Named so the save reader can
    /// hang the missing ones when it loads a document from before they
    /// were cargo.
    #[must_use]
    pub const fn instrument(self) -> bool {
        matches!(
            self,
            Self::Window | Self::ChartTank | Self::EtaGauge | Self::DestPreview | Self::LaunchLever
        )
    }

    /// Whether this kind is a dressing — laid into the room rather than
    /// stowed on it. Coverings have no hold-occupancy form aboard.
    #[must_use]
    pub const fn covering(self) -> bool {
        matches!(self.tag(), Some(Tag::Covering(_)))
    }

    /// How eagerly the burner takes this kind, `0..=3`: stoke earned per
    /// piece fed to the fire. Upholstery, fur, and fuel go up gloriously;
    /// wood and paper honestly; metal, stone, and ice are slag — the
    /// stoker still shovels them through (disposal is disposal), they
    /// just push nothing. The suspicious kinds never reach the hopper at
    /// all (they refuse the rail), so their values here are moot.
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

/// Which surface a piece sits on. Hold coordinates are grid cells; every
/// other variant is a slot index into the matching layout rect array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loc {
    /// Anchor cell in the ship's hold grid (top-left of the footprint).
    Hold { x: u8, y: u8 },
    /// The station's goods on offer.
    StationShelf { slot: u8 },
    /// What the player is offering.
    GivePad { slot: u8 },
    /// What the player is asking for.
    TakePad { slot: u8 },
    /// Goods just traded for, waiting to be stowed.
    ReceivedShelf { slot: u8 },
    /// Adrift beside the ship during a travel encounter. Not the player's
    /// until stowed; whatever is left drifts away when the encounter ends.
    Flotsam { slot: u8 },
    /// Inside a cabinet's cubby. The berth exists only while that cabinet
    /// piece does: an occupied cabinet cannot be lifted, so the cubby can
    /// never find itself without a home.
    Stow { cabinet: u32, slot: u8 },
    /// Laid into the room at the anchor cell: the dressing layer.
    /// Coexists with occupancy on the same cells (a couch stands on a
    /// laid rug); no two dressings share a cell ([`dressing_check`]).
    Laid { x: u8, y: u8 },
}

/// Cubbies per cabinet: a 2×2 rack of them behind the doors.
pub const CABINET_SLOTS: u8 = 4;

/// Whether `kind` may ride inside a cabinet.
///
/// One cell, and neither the kinds that need the hull's cold (cryo) nor
/// the ones nobody should box up (suspicious — none is 1×1 today, but the
/// rule is written for the day one is). Everything else about a stowed
/// piece is ordinary; what *emerges* from a cubby not being a hold cell —
/// dark lamps, unbred fluff, rat-proof shelter, invisibility to ??? — is
/// documented in docs/BAY.md, not special-cased anywhere.
#[must_use]
pub const fn stowable(kind: Kind) -> bool {
    let (w, h) = kind.cells();
    w == 1 && h == 1 && !matches!(kind.tag(), Some(Tag::Cryo | Tag::Suspicious))
}

/// Whether any piece rides in `cabinet`'s cubbies.
///
/// An occupied cabinet refuses to be lifted or quick-moved
/// (`Violation::Occupied`): empty it first, piece by piece — which is
/// also why cubby cargo can never reach a trade pad by accident.
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

/// Whether a piece at `loc` belongs to the player rather than the station.
///
/// This is THE ownership rule: the drop matrix in `resolve_drop`, the
/// [`crate::sim::Sim::drop_targets`] affordances, and any renderer hint all
/// derive from this one predicate. Never restate it — a hand-mirrored copy
/// is how a highlight ends up inviting a drop the rules refuse.
#[must_use]
pub const fn player_owned(loc: Loc) -> bool {
    matches!(
        loc,
        Loc::Hold { .. }
            | Loc::GivePad { .. }
            | Loc::ReceivedShelf { .. }
            | Loc::Stow { .. }
            | Loc::Laid { .. }
    )
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

/// Which stowage rule refused a hold placement. One variant per rule, so the
/// renderer can flash the matching icon on a hard reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// The footprint leaves the net, crosses a hole, or bends over a
    /// fold — every placement lies wholly in one chart.
    Bounds,
    /// The footprint overlaps another piece's cells — or its standing
    /// volume: a tall floor piece shadows the wall behind it, and
    /// wall cargo cannot share that space.
    Overlap,
    /// Two volatile pieces orthogonally adjacent (fold seams count:
    /// the baseboard is next to the floor in the room, so it is here).
    Volatile,
    /// A cryo piece not touching the floor's hull edge.
    Cryo,
    /// A second suspicious piece aboard.
    Suspicious,
    /// A placement whose chart does not satisfy the kind's mount.
    Affix(Mount),
    /// A cabinet with goods in its cubbies was asked to move (or to take
    /// more than it has room for): empty it first.
    Occupied,
    /// The last vital instrument aboard offered to an exit ceremony
    /// (the rail, a give pad, the casino) — a ship that cannot chart or
    /// launch is a soft-lock, so the last of each stays.
    Vital,
}

/// Whether `kind` may be anchored at hold cell `(x, y)` given every other
/// piece in `pieces`. The piece with `id` is ignored, so a held piece never
/// collides with its own old footprint.
#[must_use]
pub fn placement_legal(pieces: &[Piece], id: u32, kind: Kind, x: u8, y: u8) -> bool {
    placement_check(pieces, id, kind, x, y).is_ok()
}

/// [`placement_legal`], but naming the rule that refused.
///
/// Checks run in a fixed order (bounds/chart, mount, cryo, then
/// per-piece overlap-and-shadow / volatile / suspicious in stowage
/// order) so the reported violation is deterministic. Nothing here
/// reasons about where a body may walk: the walker passes through
/// cargo, so a berth is refused for what it collides with, never for
/// what it fences off.
pub fn placement_check(
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    x: u8,
    y: u8,
) -> Result<(), Violation> {
    let (w, h) = kind.cells();
    if x + w > GRID_COLS || y + h > GRID_ROWS {
        return Err(Violation::Bounds);
    }
    let Some(surf) = footprint_surface(x, y, w, h) else {
        return Err(Violation::Bounds);
    };
    let mount = kind.mount();
    if !mount_accepts(mount, surf) {
        return Err(Violation::Affix(mount));
    }
    let standing = matches!(surf, Surf::Floor);
    if matches!(kind.tag(), Some(Tag::Cryo)) && !touches_hull(x, y, w, h) {
        return Err(Violation::Cryo);
    }
    // A standing piece's volume: the wall cells it shadows behind it.
    let my_shadow = if standing {
        shadow_cells(x, y, w, h, kind.stature())
    } else {
        Vec::new()
    };
    for other in pieces {
        if other.id == id {
            continue;
        }
        let Loc::Hold { x: ox, y: oy } = other.loc else {
            continue;
        };
        let (ow, oh) = other.kind.cells();
        if overlaps((x, y, w, h), (ox, oy, ow, oh)) {
            return Err(Violation::Overlap);
        }
        // Cross-plane volume conflicts are overlaps too: my shadow over
        // standing wall cargo, or — placing onto a wall — some floor
        // piece's shadow over me. No painting behind the wardrobe.
        if my_shadow
            .iter()
            .any(|&(sx, sy)| covers(ox, oy, ow, oh, sx, sy))
        {
            return Err(Violation::Overlap);
        }
        if !standing && matches!(surface_of(ox, oy), Some(Surf::Floor)) {
            let theirs = shadow_cells(ox, oy, ow, oh, other.kind.stature());
            if theirs.iter().any(|&(sx, sy)| covers(x, y, w, h, sx, sy)) {
                return Err(Violation::Overlap);
            }
        }
        if matches!(kind.tag(), Some(Tag::Volatile))
            && matches!(other.kind.tag(), Some(Tag::Volatile))
            && adjacent((x, y, w, h), (ox, oy, ow, oh))
        {
            return Err(Violation::Volatile);
        }
        if matches!(kind.tag(), Some(Tag::Suspicious))
            && matches!(other.kind.tag(), Some(Tag::Suspicious))
        {
            return Err(Violation::Suspicious);
        }
    }
    Ok(())
}

/// The one chart a footprint lies wholly inside, if any — a piece bent
/// over a fold, crossing a hole, or leaving the net is nowhere.
fn footprint_surface(x: u8, y: u8, w: u8, h: u8) -> Option<Surf> {
    let anchor = surface_of(x, y)?;
    for cy in y..y + h {
        for cx in x..x + w {
            if surface_of(cx, cy) != Some(anchor) {
                return None;
            }
        }
    }
    Some(anchor)
}

/// Whether footprint `(x, y, w, h)` covers cell `(cx, cy)`.
const fn covers(x: u8, y: u8, w: u8, h: u8, cx: u8, cy: u8) -> bool {
    cx >= x && cx < x + w && cy >= y && cy < y + h
}

/// Whether a floor footprint touches the floor's hull edge (any side of
/// the floor chart — every side of this room is hull).
const fn touches_hull(x: u8, y: u8, w: u8, h: u8) -> bool {
    x == FLOOR.0 || y == FLOOR.1 || x + w == FLOOR.0 + FLOOR.2 || y + h == FLOOR.1 + FLOOR.3
}

/// The wall cells a standing floor footprint shadows: for each footprint
/// edge lying along a baseboard seam, the wall cells directly behind it,
/// baseboard upward through the piece's stature.
fn shadow_cells(x: u8, y: u8, w: u8, h: u8, stature: u8) -> Vec<(u8, u8)> {
    let (fx, fy, fw, fh) = FLOOR;
    let mut cells = Vec::new();
    for depth in 0..stature.min(3) {
        for cx in x..x + w {
            if y == fy {
                // Aft baseboard row sits just above the floor's aft edge.
                cells.push((cx, fy - 1 - depth));
            }
            if y + h == fy + fh {
                cells.push((cx, fy + fh + depth));
            }
        }
        for cy in y..y + h {
            if x == fx {
                cells.push((fx - 1 - depth, cy));
            }
            if x + w == fx + fw {
                cells.push((fx + fw + depth, cy));
            }
        }
    }
    cells
}

/// Cell-rect intersection test.
const fn overlaps(a: (u8, u8, u8, u8), b: (u8, u8, u8, u8)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The first hold cell (row-major scan) where `kind` may legally sit.
///
/// Shared by the shift-click quick-stow, the comet harvest, and the
/// ??? exchange — "first legal spot, even if that is a bad idea" is the
/// contract, so all three agree on what "first" means. Coverings have
/// no hold berth at all ([`dress_fit`] is their scan).
#[must_use]
pub fn first_fit(pieces: &[Piece], id: u32, kind: Kind) -> Option<(u8, u8)> {
    if kind.covering() {
        return None;
    }
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            if placement_legal(pieces, id, kind, x, y) {
                return Some((x, y));
            }
        }
    }
    None
}

/// Whether covering `kind` may be laid at anchor `(x, y)`.
///
/// The dressing layer's own [`placement_check`], reusing the violation
/// ladder whole and consulting every other piece. Checks run in a fixed
/// order (bounds, surface, then per-piece dressing overlap /
/// pinned-under-occupancy) so the reported violation is deterministic.
pub fn dressing_check(
    pieces: &[Piece],
    id: u32,
    kind: Kind,
    x: u8,
    y: u8,
) -> Result<(), Violation> {
    debug_assert!(kind.covering(), "dressing_check is for coverings only");
    let (w, h) = kind.cells();
    if x + w > GRID_COLS || y + h > GRID_ROWS {
        return Err(Violation::Bounds);
    }
    // Wholly on one chart — a rug bent over a fold is not a rug anyone
    // respects, and a coat cannot paint across a hole.
    let Some(surf) = footprint_surface(x, y, w, h) else {
        return Err(Violation::Bounds);
    };
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
        let (ow, oh) = other.kind.cells();
        match other.loc {
            // One dressing per cell.
            Loc::Laid { x: ox, y: oy } if overlaps((x, y, w, h), (ox, oy, ow, oh)) => {
                return Err(Violation::Overlap);
            }
            // No sliding a dressing under standing cargo: the pinned
            // rule, symmetric with the lift refusal in `laid_pinned`.
            Loc::Hold { x: ox, y: oy } if overlaps((x, y, w, h), (ox, oy, ow, oh)) => {
                return Err(Violation::Occupied);
            }
            _ => {}
        }
    }
    Ok(())
}

/// The first anchor (row-major) where covering `kind` may be laid — the
/// dressing layer's [`first_fit`], for quick-moves off the pads.
#[must_use]
pub fn dress_fit(pieces: &[Piece], id: u32, kind: Kind) -> Option<(u8, u8)> {
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            if dressing_check(pieces, id, kind, x, y).is_ok() {
                return Some((x, y));
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
pub fn laid_pinned(pieces: &[Piece], piece: &Piece) -> bool {
    let Loc::Laid { x, y } = piece.loc else {
        return false;
    };
    let (w, h) = piece.kind.cells();
    pieces.iter().any(|other| {
        let Loc::Hold { x: ox, y: oy } = other.loc else {
            return false;
        };
        let (ow, oh) = other.kind.cells();
        other.id != piece.id && overlaps((x, y, w, h), (ox, oy, ow, oh))
    })
}

/// Whether `piece` is the LAST vital instrument of its kind in the
/// player's possession — the piece every exit ceremony must refuse.
///
/// Only berths that are STAYING count as possession: a spare already
/// staged on a give pad or the rail is itself on its way out, and
/// counting it would let both of a pair be staged and both be lost.
/// (The received shelf counts — departure refuses while it holds
/// goods, so nothing there is ever stranded.)
#[must_use]
pub fn last_vital_aboard(pieces: &[Piece], piece: &Piece) -> bool {
    piece.kind.vital()
        && !pieces.iter().any(|other| {
            other.id != piece.id
                && other.kind == piece.kind
                && matches!(
                    other.loc,
                    Loc::Hold { .. }
                        | Loc::Stow { .. }
                        | Loc::Laid { .. }
                        | Loc::ReceivedShelf { .. }
                )
        })
}

/// Whether two footprints share an orthogonal edge (corners do not count).
const fn adjacent(a: (u8, u8, u8, u8), b: (u8, u8, u8, u8)) -> bool {
    let x_overlap = a.0 < b.0 + b.2 && b.0 < a.0 + a.2;
    let y_overlap = a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
    let x_touch = a.0 + a.2 == b.0 || b.0 + b.2 == a.0;
    let y_touch = a.1 + a.3 == b.1 || b.1 + b.3 == a.1;
    (x_overlap && y_touch) || (y_overlap && x_touch)
}

/// Whether `kind` is a lamp — one of the three affixed fixtures that cast
/// light while stowed.
#[must_use]
pub const fn lamp(kind: Kind) -> bool {
    matches!(kind, Kind::CeilingLamp | Kind::WallLamp | Kind::FloorLamp)
}

/// Whether `piece` is a lamp, burning.
///
/// Lamps are lit while stowed in the hold and nowhere else: on a shelf, a
/// pad, the outboard net, or boxed in a cabinet cubby they are dark. Everything lighting touches —
/// the rat's fear, the well-lit art bonus, any frontend halo — reads lamp
/// state through this one predicate.
#[must_use]
pub const fn lamp_lit(piece: &Piece) -> bool {
    lamp(piece.kind) && matches!(piece.loc, Loc::Hold { .. })
}

/// Whether hold cell `(x, y)` sits in light.
///
/// Lit means orthogonally adjacent to — never inside — some lit lamp's
/// footprint OR some laid luminous coat's, by the same [`adjacent`]
/// rule the volatile check uses, so corners do not count. Everything
/// light touches — the rat's fear, the seedlings' bloom, the hold
/// painting's spotlight — reads through
/// this one predicate; the pad-side well-lit-art bonus deliberately
/// does not (a coat is ambiance, not gallery lighting — see
/// `barter::well_lit`).
#[must_use]
pub fn lit_adjacent(pieces: &[Piece], x: u8, y: u8) -> bool {
    pieces.iter().any(|piece| {
        let (source, lx, ly) = match piece.loc {
            Loc::Hold { x, y } => (lamp_lit(piece), x, y),
            Loc::Laid { x, y } => (piece.kind == Kind::LuminousPaint, x, y),
            _ => return false,
        };
        let (w, h) = piece.kind.cells();
        source && adjacent((x, y, 1, 1), (lx, ly, w, h))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A board of pieces stowed at the given cells, ids counting up from 0.
    fn board(stowed: &[(Kind, u8, u8)]) -> Vec<Piece> {
        stowed
            .iter()
            .enumerate()
            .map(|(i, &(kind, x, y))| Piece {
                id: i as u32,
                kind,
                variant: 0,
                gnawed: false,
                loc: Loc::Hold { x, y },
            })
            .collect()
    }

    /// The next free id after [`board`], for the candidate piece.
    fn check(stowed: &[(Kind, u8, u8)], kind: Kind, x: u8, y: u8) -> Result<(), Violation> {
        let pieces = board(stowed);
        placement_check(&pieces, pieces.len() as u32, kind, x, y)
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

    #[test]
    fn heavy_lies_dormant_and_the_walls_refuse_plain_cargo() {
        // Heavy cargo stands anywhere on the floor now — the room-grid
        // law already keeps it off the walls, which is what riding low
        // used to mean. The tag waits for the stacking rules.
        assert_eq!(check(&[], Kind::GildedIdol, 3, 3), Ok(()));
        assert_eq!(check(&[], Kind::GildedIdol, 5, 5), Ok(()));
        // The old failure mode, restated: lifted onto a wall, the mount
        // law refuses (port chart, floor kind).
        assert_eq!(
            check(&[], Kind::GildedIdol, 0, 4),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn volatile_rule_accepts_gapped_and_names_adjacency() {
        let stowed = [(Kind::GasCanister, 3, 7)];
        // A full empty row between the two: fine.
        assert_eq!(check(&stowed, Kind::GasCanister, 3, 5), Ok(()));
        // Directly above: orthogonally adjacent.
        assert_eq!(
            check(&stowed, Kind::GasCanister, 3, 6),
            Err(Violation::Volatile)
        );
        // Offset anchors still touch through their non-anchor cells.
        assert_eq!(
            check(&stowed, Kind::GasCanister, 4, 6),
            Err(Violation::Volatile)
        );
        // Corner contact only: not adjacent.
        assert_eq!(check(&stowed, Kind::GasCanister, 5, 6), Ok(()));
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
        // The walker passes through cargo now, so the floor keeps no
        // reserved lanes: the burner threshold takes a berth like any
        // other cell, and a wall of cargo may close across the room.
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
        let stowed = [(Kind::Cabinet, 4, 3)];
        assert_eq!(
            check(&stowed, Kind::Painting, 4, 1),
            Err(Violation::Overlap)
        );
        // One column over, the wall is clear.
        assert_eq!(check(&stowed, Kind::Painting, 5, 1), Ok(()));
        // And symmetrically: with the painting hung first, the cabinet
        // may not stand in front of it.
        let hung = [(Kind::Painting, 4, 1)];
        assert_eq!(check(&hung, Kind::Cabinet, 4, 3), Err(Violation::Overlap));
        assert_eq!(check(&hung, Kind::Cabinet, 6, 3), Ok(()));
    }

    #[test]
    fn affix_rule_accepts_the_mount_surface_and_names_the_miss() {
        // A ceiling lamp hangs from the ceiling chart and nowhere else.
        assert_eq!(check(&[], Kind::CeilingLamp, 16, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::CeilingLamp, 4, 1),
            Err(Violation::Affix(Mount::Ceiling))
        );
        // A wall lamp takes any wall, never the middle of the room.
        assert_eq!(check(&[], Kind::WallLamp, 3, 1), Ok(()));
        assert_eq!(check(&[], Kind::WallLamp, 1, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::WallLamp, 5, 5),
            Err(Violation::Affix(Mount::Wall))
        );
        // The couch sits on the floor, wherever across it.
        assert_eq!(check(&[], Kind::Couch, 4, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::Couch, 4, 0),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn the_floor_lamp_stands_on_the_floor_and_never_across_a_fold() {
        // 1x2 with a floor mount: fine mid-floor and against the front
        // edge; refused on the aft wall; nowhere when bent over the fold.
        assert_eq!(check(&[], Kind::FloorLamp, 4, 4), Ok(()));
        assert_eq!(check(&[], Kind::FloorLamp, 4, 8), Ok(()));
        assert_eq!(
            check(&[], Kind::FloorLamp, 4, 0),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(check(&[], Kind::FloorLamp, 4, 2), Err(Violation::Bounds));
    }

    #[test]
    fn the_painting_hangs_on_any_wall_but_never_a_hole() {
        // 2x1 on the aft wall, on the port wall, off in the room, and
        // across the burner doorway's punch-out.
        assert_eq!(check(&[], Kind::Painting, 4, 1), Ok(()));
        assert_eq!(check(&[], Kind::Painting, 0, 4), Ok(()));
        assert_eq!(
            check(&[], Kind::Painting, 4, 4),
            Err(Violation::Affix(Mount::Wall))
        );
        assert_eq!(check(&[], Kind::Painting, 11, 3), Err(Violation::Bounds));
    }

    #[test]
    fn affix_is_checked_before_the_per_piece_scan() {
        // Off the mount AND overlapping: the affix rule answers first...
        let stowed = [(Kind::RationBricks, 4, 4)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 4, 4),
            Err(Violation::Affix(Mount::Wall))
        );
        // ...while an on-mount collision still names the overlap.
        let stowed = [(Kind::Painting, 3, 1)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 3, 1),
            Err(Violation::Overlap)
        );
    }

    #[test]
    fn lamps_are_lit_only_in_the_hold_and_light_their_neighbours() {
        assert!(lamp(Kind::CeilingLamp) && lamp(Kind::WallLamp) && lamp(Kind::FloorLamp));
        assert!(!lamp(Kind::Couch) && !lamp(Kind::Painting) && !lamp(Kind::PerfumeVial));

        let pieces = board(&[(Kind::CeilingLamp, 16, 4)]);
        assert!(lamp_lit(&pieces[0]));
        // Orthogonal neighbours read lit; the lamp's own cell and the
        // corners do not.
        assert!(lit_adjacent(&pieces, 15, 4));
        assert!(lit_adjacent(&pieces, 17, 4));
        assert!(lit_adjacent(&pieces, 16, 5));
        assert!(!lit_adjacent(&pieces, 16, 4));
        assert!(!lit_adjacent(&pieces, 15, 3));
        assert!(!lit_adjacent(&pieces, 18, 4));

        // A floor lamp lights along its whole 1x2 footprint.
        let tall = board(&[(Kind::FloorLamp, 3, 4)]);
        assert!(lit_adjacent(&tall, 4, 4));
        assert!(lit_adjacent(&tall, 4, 5));
        assert!(lit_adjacent(&tall, 3, 3));
        assert!(!lit_adjacent(&tall, 4, 3));

        // Off the hold a lamp is dark, and stowed non-lamps light nothing.
        let shelved = Piece {
            id: 9,
            kind: Kind::FloorLamp,
            variant: 0,
            gnawed: false,
            loc: Loc::StationShelf { slot: 0 },
        };
        assert!(!lamp_lit(&shelved));
        assert!(!lit_adjacent(&[shelved], 4, 4));
        let art = board(&[(Kind::Painting, 3, 1)]);
        assert!(!lit_adjacent(&art, 5, 1));
    }

    #[test]
    fn stowable_is_small_and_neither_cold_nor_suspect() {
        // One cell and unencumbered: rides in a cabinet.
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
        // Too big, or a rule says no.
        for kind in [
            Kind::CryoCore,   // 1x1 but needs the hull
            Kind::CometIce,   // ditto
            Kind::GildedIdol, // 1x2
            Kind::Couch,      // 2x1
            Kind::Cabinet,    // no cabinets in cabinets
            Kind::SuspiciousCrate,
        ] {
            assert!(!stowable(kind), "{kind:?} should refuse the cubby");
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
            loc: Loc::Hold { x: 4, y: 4 },
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
        // A different cabinet's cubbies are its own business.
        assert!(!cabinet_occupied(&pieces, 8));
        assert_eq!(free_cubby(&pieces, 8), Some(0));
    }

    #[test]
    fn dressing_rules_cover_surface_overlap_and_pinning() {
        // A rug lies wholly on the floor and nowhere else — not on a
        // wall, and never bent over a fold.
        assert_eq!(dressing_check(&[], 9, Kind::Rug, 4, 7), Ok(()));
        assert_eq!(
            dressing_check(&[], 9, Kind::Rug, 4, 1),
            Err(Violation::Affix(Mount::Floor))
        );
        assert_eq!(
            dressing_check(&[], 9, Kind::Rug, 10, 7),
            Err(Violation::Bounds)
        );
        // Paint coats any chart's cells.
        assert_eq!(dressing_check(&[], 9, Kind::PaintTin, 3, 0), Ok(()));
        assert_eq!(dressing_check(&[], 9, Kind::LuminousPaint, 4, 4), Ok(()));
        // One dressing per cell: a coat may not land on a laid rug.
        let mut pieces = vec![Piece {
            id: 0,
            kind: Kind::Rug,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid { x: 4, y: 7 },
        }];
        assert_eq!(
            dressing_check(&pieces, 9, Kind::PaintTin, 5, 7),
            Err(Violation::Overlap)
        );
        assert_eq!(dressing_check(&pieces, 9, Kind::PaintTin, 3, 7), Ok(()));
        // No sliding a dressing under standing cargo...
        pieces.push(Piece {
            id: 1,
            kind: Kind::Couch,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold { x: 3, y: 7 },
        });
        assert_eq!(
            dressing_check(&pieces, 9, Kind::PaintTin, 3, 7),
            Err(Violation::Occupied)
        );
        // ...and none lifts from under it: lay a rug, stand a couch on
        // half of it, and the rug is pinned.
        let rug = Piece {
            id: 2,
            kind: Kind::Rug,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid { x: 4, y: 7 },
        };
        let couch = Piece {
            id: 3,
            kind: Kind::Couch,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold { x: 5, y: 7 },
        };
        assert!(laid_pinned(&[rug, couch], &rug));
        assert!(!laid_pinned(&[rug], &rug));
        // Coverings have no hold berth for first_fit to find, and the
        // dressing scan starts clear of both the rug and the couch.
        assert_eq!(first_fit(&[], 9, Kind::Rug), None);
        assert_eq!(dress_fit(&[rug, couch], 9, Kind::Rug), Some((3, 3)));
    }

    #[test]
    fn luminous_coats_light_their_neighbours() {
        let coat = Piece {
            id: 0,
            kind: Kind::LuminousPaint,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid { x: 4, y: 1 },
        };
        assert!(lit_adjacent(&[coat], 3, 1));
        assert!(lit_adjacent(&[coat], 4, 0));
        assert!(!lit_adjacent(&[coat], 4, 1), "never inside, only beside");
        assert!(!lit_adjacent(&[coat], 3, 0), "corners do not count");
        // Plain enamel sheds no light, and an unlaid tin is just a tin.
        let tin = Piece {
            id: 1,
            kind: Kind::PaintTin,
            variant: 0,
            gnawed: false,
            loc: Loc::Laid { x: 6, y: 1 },
        };
        assert!(!lit_adjacent(&[tin], 5, 1));
        let canned = Piece {
            id: 2,
            kind: Kind::LuminousPaint,
            variant: 0,
            gnawed: false,
            loc: Loc::StationShelf { slot: 0 },
        };
        assert!(!lit_adjacent(&[canned], 1, 1));
    }

    #[test]
    fn held_piece_ignores_its_own_footprint() {
        let pieces = board(&[(Kind::RationBricks, 4, 4)]);
        // Re-dropping piece 0 onto (or near) itself must not self-collide.
        assert_eq!(
            placement_check(&pieces, 0, Kind::RationBricks, 4, 4),
            Ok(())
        );
        assert_eq!(
            placement_check(&pieces, 0, Kind::RationBricks, 5, 4),
            Ok(())
        );
    }
}
