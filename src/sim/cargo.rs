//! Cargo pieces: what they are, where they sit, and the stowage rules.
//!
//! A [`Piece`] is one draggable object. Its [`Loc`] says which surface it is
//! on — the ship's hold grid or one of the barter panel's shelves and pads —
//! and [`placement_check`] is the single arbiter of whether a piece may sit
//! at a given hold cell. The renderer and the drag logic both defer to it, so
//! there is exactly one opinion about what fits, and a failure names the
//! [`Violation`] so the frontend can flash the right icon.

use super::layout::{GRID_COLS, GRID_ROWS};

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
}

/// Number of cargo kinds.
pub const KIND_COUNT: usize = 22;

/// Cosmetic variant rolls per kind, for the renderer to vary sprites with.
/// The persistent run RNG is spent on these and nothing else.
pub(crate) const VARIANTS: u8 = 4;

/// Special handling a kind demands in the hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// Must be stowed low: anchor row 2 or below.
    Heavy,
    /// No two volatile pieces may sit orthogonally adjacent.
    Volatile,
    /// Must touch the hold's outer edge.
    Cryo,
    /// At most one suspicious piece aboard, and hauling it has consequences.
    Suspicious,
    /// A fixture: its footprint must touch the named room surface.
    Affix(Mount),
}

/// The room surface a fixture mounts to. The hold grid's edges are the
/// room: row 0 is the ceiling, the last row is the floor, and the outer
/// columns are the walls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mount {
    Ceiling,
    Floor,
    Wall,
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
            | Self::WallLamp => (1, 1),
            Self::GildedIdol | Self::BrinePearls | Self::FloorLamp | Self::Cabinet => (1, 2),
            Self::RationBricks | Self::SuspiciousCrate | Self::VeryMysteriousCrate => (2, 2),
            Self::ScrapAlloy | Self::GasCanister | Self::Couch | Self::Painting => (2, 1),
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
            Self::WallLamp | Self::Painting => Some(Tag::Affix(Mount::Wall)),
            _ => None,
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
        Loc::Hold { .. } | Loc::GivePad { .. } | Loc::ReceivedShelf { .. } | Loc::Stow { .. }
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
    /// The footprint runs off the grid.
    Bounds,
    /// The footprint overlaps another stowed piece.
    Overlap,
    /// A heavy piece anchored above row 2.
    Heavy,
    /// Two volatile pieces orthogonally adjacent.
    Volatile,
    /// A cryo piece not touching the hold's outer edge.
    Cryo,
    /// A second suspicious piece aboard.
    Suspicious,
    /// A fixture whose footprint misses its mount surface.
    Affix(Mount),
    /// A cabinet with goods in its cubbies was asked to move (or to take
    /// more than it has room for): empty it first.
    Occupied,
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
/// Checks run in a fixed order (bounds, heavy, cryo, affix, then per-piece
/// overlap / volatile / suspicious in stowage order) so the reported
/// violation is deterministic.
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
    if matches!(kind.tag(), Some(Tag::Heavy)) && y < 2 {
        return Err(Violation::Heavy);
    }
    if matches!(kind.tag(), Some(Tag::Cryo)) && !touches_edge(x, y, w, h) {
        return Err(Violation::Cryo);
    }
    if let Some(Tag::Affix(mount)) = kind.tag() {
        if !touches_mount(mount, x, y, w, h) {
            return Err(Violation::Affix(mount));
        }
    }
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

/// Whether a footprint touches the hold's outer edge.
const fn touches_edge(x: u8, y: u8, w: u8, h: u8) -> bool {
    x == 0 || y == 0 || x + w == GRID_COLS || y + h == GRID_ROWS
}

/// Whether a footprint touches its required mount surface: some covered
/// cell on row 0 (ceiling), the last row (floor), or an outer column
/// (wall).
const fn touches_mount(mount: Mount, x: u8, y: u8, w: u8, h: u8) -> bool {
    match mount {
        Mount::Ceiling => y == 0,
        Mount::Floor => y + h == GRID_ROWS,
        Mount::Wall => x == 0 || x + w == GRID_COLS,
    }
}

/// Cell-rect intersection test.
const fn overlaps(a: (u8, u8, u8, u8), b: (u8, u8, u8, u8)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The first hold cell (row-major scan) where `kind` may legally sit.
///
/// Shared by the shift-click quick-stow, the comet harvest, and the
/// ??? exchange — "first legal spot, even if that is a bad idea" is the
/// contract, so all three agree on what "first" means.
#[must_use]
pub fn first_fit(pieces: &[Piece], id: u32, kind: Kind) -> Option<(u8, u8)> {
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            if placement_legal(pieces, id, kind, x, y) {
                return Some((x, y));
            }
        }
    }
    None
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

/// Whether hold cell `(x, y)` sits in lamplight: orthogonally adjacent to
/// — never inside — some lit lamp's footprint, by the same [`adjacent`]
/// rule the volatile check uses, so corners do not count.
#[must_use]
pub fn lit_adjacent(pieces: &[Piece], x: u8, y: u8) -> bool {
    pieces.iter().any(|piece| {
        let Loc::Hold { x: lx, y: ly } = piece.loc else {
            return false;
        };
        let (w, h) = piece.kind.cells();
        lamp_lit(piece) && adjacent((x, y, 1, 1), (lx, ly, w, h))
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
        // RationBricks is 2x2: fits anchored at (4, 2), runs off at (5, 0).
        assert_eq!(check(&[], Kind::RationBricks, 4, 2), Ok(()));
        assert_eq!(check(&[], Kind::RationBricks, 5, 0), Err(Violation::Bounds));
        assert_eq!(check(&[], Kind::RationBricks, 0, 3), Err(Violation::Bounds));
    }

    #[test]
    fn overlap_rule_accepts_beside_and_names_collision() {
        let stowed = [(Kind::PerfumeVial, 2, 2)];
        assert_eq!(check(&stowed, Kind::Seedlings, 3, 2), Ok(()));
        assert_eq!(
            check(&stowed, Kind::Seedlings, 2, 2),
            Err(Violation::Overlap)
        );
        // Multi-cell: ScrapAlloy anchored at (1, 2) covers (2, 2) too.
        assert_eq!(
            check(&stowed, Kind::ScrapAlloy, 1, 2),
            Err(Violation::Overlap)
        );
    }

    #[test]
    fn heavy_rule_accepts_low_and_names_high() {
        assert_eq!(check(&[], Kind::GildedIdol, 0, 2), Ok(()));
        assert_eq!(check(&[], Kind::GildedIdol, 0, 1), Err(Violation::Heavy));
        assert_eq!(check(&[], Kind::ScrapAlloy, 3, 0), Err(Violation::Heavy));
    }

    #[test]
    fn volatile_rule_accepts_gapped_and_names_adjacency() {
        let stowed = [(Kind::GasCanister, 0, 3)];
        // A full empty row between the two: fine.
        assert_eq!(check(&stowed, Kind::GasCanister, 0, 1), Ok(()));
        // Directly above: orthogonally adjacent.
        assert_eq!(
            check(&stowed, Kind::GasCanister, 0, 2),
            Err(Violation::Volatile)
        );
        // Offset anchors still touch through their non-anchor cells.
        assert_eq!(
            check(&stowed, Kind::GasCanister, 1, 2),
            Err(Violation::Volatile)
        );
        // Corner contact only: not adjacent.
        assert_eq!(check(&stowed, Kind::GasCanister, 2, 2), Ok(()));
    }

    #[test]
    fn cryo_rule_accepts_edge_and_names_interior() {
        assert_eq!(check(&[], Kind::CryoCore, 0, 1), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, 2, 3), Ok(()));
        assert_eq!(check(&[], Kind::CryoCore, 2, 1), Err(Violation::Cryo));
    }

    #[test]
    fn suspicious_rule_accepts_one_and_names_a_second() {
        assert_eq!(check(&[], Kind::SuspiciousCrate, 3, 0), Ok(()));
        let stowed = [(Kind::SuspiciousCrate, 0, 2)];
        assert_eq!(
            check(&stowed, Kind::SuspiciousCrate, 3, 0),
            Err(Violation::Suspicious)
        );
    }

    #[test]
    fn affix_rule_accepts_the_mount_surface_and_names_the_miss() {
        // A ceiling lamp hangs from row 0 and nowhere lower.
        assert_eq!(check(&[], Kind::CeilingLamp, 3, 0), Ok(()));
        assert_eq!(
            check(&[], Kind::CeilingLamp, 3, 1),
            Err(Violation::Affix(Mount::Ceiling))
        );
        // A wall lamp takes either wall, never the middle of the room.
        assert_eq!(check(&[], Kind::WallLamp, 0, 2), Ok(()));
        assert_eq!(check(&[], Kind::WallLamp, 5, 1), Ok(()));
        assert_eq!(
            check(&[], Kind::WallLamp, 2, 2),
            Err(Violation::Affix(Mount::Wall))
        );
        // The couch sits on the floor row, wherever along it.
        assert_eq!(check(&[], Kind::Couch, 2, 3), Ok(()));
        assert_eq!(
            check(&[], Kind::Couch, 2, 2),
            Err(Violation::Affix(Mount::Floor))
        );
    }

    #[test]
    fn the_floor_lamp_fits_only_anchored_at_row_two() {
        // 1x2 with a floor affix: only anchor row 2 reaches the deck.
        for y in 0..GRID_ROWS {
            let expect = match y {
                2 => Ok(()),
                3 => Err(Violation::Bounds),
                _ => Err(Violation::Affix(Mount::Floor)),
            };
            assert_eq!(check(&[], Kind::FloorLamp, 1, y), expect, "anchor y={y}");
        }
    }

    #[test]
    fn the_painting_reaches_a_wall_from_either_end_but_not_the_middle() {
        // 2x1 against the left wall, the right wall, and neither.
        assert_eq!(check(&[], Kind::Painting, 0, 1), Ok(()));
        assert_eq!(check(&[], Kind::Painting, 4, 1), Ok(()));
        assert_eq!(
            check(&[], Kind::Painting, 2, 1),
            Err(Violation::Affix(Mount::Wall))
        );
    }

    #[test]
    fn affix_is_checked_before_the_per_piece_scan() {
        // Off the mount AND overlapping: the affix rule answers first...
        let stowed = [(Kind::RationBricks, 2, 1)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 2, 1),
            Err(Violation::Affix(Mount::Wall))
        );
        // ...while an on-mount collision still names the overlap.
        let stowed = [(Kind::PerfumeVial, 0, 2)];
        assert_eq!(
            check(&stowed, Kind::WallLamp, 0, 2),
            Err(Violation::Overlap)
        );
    }

    #[test]
    fn lamps_are_lit_only_in_the_hold_and_light_their_neighbours() {
        assert!(lamp(Kind::CeilingLamp) && lamp(Kind::WallLamp) && lamp(Kind::FloorLamp));
        assert!(!lamp(Kind::Couch) && !lamp(Kind::Painting) && !lamp(Kind::PerfumeVial));

        let pieces = board(&[(Kind::CeilingLamp, 2, 0)]);
        assert!(lamp_lit(&pieces[0]));
        // Orthogonal neighbours read lit; the lamp's own cell and the
        // corners do not.
        assert!(lit_adjacent(&pieces, 1, 0));
        assert!(lit_adjacent(&pieces, 3, 0));
        assert!(lit_adjacent(&pieces, 2, 1));
        assert!(!lit_adjacent(&pieces, 2, 0));
        assert!(!lit_adjacent(&pieces, 1, 1));
        assert!(!lit_adjacent(&pieces, 4, 0));

        // A floor lamp lights along its whole 1x2 footprint.
        let tall = board(&[(Kind::FloorLamp, 0, 2)]);
        assert!(lit_adjacent(&tall, 1, 2));
        assert!(lit_adjacent(&tall, 1, 3));
        assert!(lit_adjacent(&tall, 0, 1));
        assert!(!lit_adjacent(&tall, 1, 1));

        // Off the hold a lamp is dark, and stowed non-lamps light nothing.
        let shelved = Piece {
            id: 9,
            kind: Kind::FloorLamp,
            variant: 0,
            gnawed: false,
            loc: Loc::StationShelf { slot: 0 },
        };
        assert!(!lamp_lit(&shelved));
        assert!(!lit_adjacent(&[shelved], 0, 1));
        let art = board(&[(Kind::Painting, 0, 1)]);
        assert!(!lit_adjacent(&art, 2, 1));
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
            loc: Loc::Hold { x: 0, y: 2 },
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
    fn held_piece_ignores_its_own_footprint() {
        let pieces = board(&[(Kind::RationBricks, 2, 1)]);
        // Re-dropping piece 0 onto (or near) itself must not self-collide.
        assert_eq!(
            placement_check(&pieces, 0, Kind::RationBricks, 2, 1),
            Ok(())
        );
        assert_eq!(
            placement_check(&pieces, 0, Kind::RationBricks, 3, 1),
            Ok(())
        );
    }
}
