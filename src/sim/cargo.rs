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
    /// lets a course be charted to Venus, Earth, or Mars.
    TransitChit,
    /// What the space casino hands back when the house wins. The house
    /// says it is worth a fortune. Every station disagrees.
    CasinoChip,
}

/// Number of cargo kinds.
pub const KIND_COUNT: usize = 16;

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
            | Self::CasinoChip => (1, 1),
            Self::GildedIdol | Self::BrinePearls => (1, 2),
            Self::RationBricks | Self::SuspiciousCrate | Self::VeryMysteriousCrate => (2, 2),
            Self::ScrapAlloy | Self::GasCanister => (2, 1),
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
        Loc::Hold { .. } | Loc::GivePad { .. } | Loc::ReceivedShelf { .. }
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
/// Checks run in a fixed order (bounds, heavy, cryo, then per-piece overlap
/// / volatile / suspicious in stowage order) so the reported violation is
/// deterministic.
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

/// Cell-rect intersection test.
const fn overlaps(a: (u8, u8, u8, u8), b: (u8, u8, u8, u8)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The first hold cell (row-major scan) where `kind` may legally sit, if
/// any. Shared by the shift-click quick-stow, the comet harvest, and the
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
