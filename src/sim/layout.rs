//! Fixed screen geometry, in world coordinates.
//!
//! The sim hit-tests against these rects and the renderer draws inside them,
//! so the two can never disagree about where a button is. Everything is a
//! constant: the console does not rearrange itself.

use super::Vec2;
use super::cargo::{Loc, Piece};

/// Axis-aligned rectangle in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Construct a rect from its top-left corner and size.
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Whether `p` falls inside (top/left edges in, bottom/right out).
    #[must_use]
    pub const fn contains(self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
}

/// The star map: where POIs live and destinations get picked.
pub const MAP_PANEL: Rect = Rect::new(10.0, 10.0, 500.0, 420.0);

/// The ship console to the map's right.
pub const CONSOLE: Rect = Rect::new(520.0, 10.0, 270.0, 420.0);

/// Preview of the selected destination, top of the console.
pub const DEST_PREVIEW: Rect = Rect::new(560.0, 40.0, 190.0, 190.0);

/// Centre of the ETA arc, between the preview and the launch lever.
pub const ETA_ARC_CENTER: Vec2 = Vec2::new(655.0, 262.0);

/// Radius of the ETA arc.
pub const ETA_ARC_RADIUS: f32 = 24.0;

/// Pull to depart for the selected destination.
pub const LAUNCH_LEVER: Rect = Rect::new(560.0, 300.0, 190.0, 60.0);

/// Pause icon. The frontend turns presses here into `toggle_pause`.
pub const PAUSE_BTN: Rect = Rect::new(530.0, 380.0, 40.0, 40.0);

/// Warp icon. The frontend turns presses here into `toggle_warp`.
pub const WARP_BTN: Rect = Rect::new(580.0, 380.0, 40.0, 40.0);

/// Speaker icon. Mute is frontend state; the sim never hears about it.
pub const SPEAKER: Rect = Rect::new(630.0, 380.0, 40.0, 40.0);

/// The room net's bounding grid width, in cells (see [`surface_of`]).
pub const GRID_COLS: u8 = 18;

/// The room net's bounding grid height, in cells.
pub const GRID_ROWS: u8 = 11;

/// Net cell size, in world units.
pub const CELL: f32 = 34.0;

/// Top-left corner of the room net, east of the retired console space.
///
/// The world grew to hold it (`WORLD_W`); the map and barter rects kept
/// their coordinates, so POI distances and desk muscle memory survive.
pub const GRID_ORIGIN: Vec2 = Vec2::new(810.0, 16.0);

// ---- The room net (docs/BAY.md, "The room grid") ----
//
// The entire room, unfolded into one 2D cross of six charts and laid
// into the logical world — the walkable-bay fold generalized until it
// closes. One unified (x, y) cell space with a validity mask; every
// rule classifies cells through [`surface_of`] and never stores a
// surface. The old 6×4 hold embeds at (+3, 0): its wall band is the
// aft chart's rows, its deck strip is the floor's aft-most row, which
// is what keeps save migration a translation.
//
// Chart bounds (x0..x1, y0..y1), half-open:
//
//         aft  (3..9, 0..3)   cornice y0, baseboard y2
//   port (0..3, 3..8)         cornice x0, baseboard x2
//   floor (3..9, 3..8)        row y3 lies along the aft baseboard
//   starboard (9..12, 3..8)   baseboard x9, cornice x11
//   front (3..9, 8..11)       baseboard y8, cornice y10
//   ceiling (12..18, 3..8)    folded over the starboard cornice
//
// Fold seams that are adjacent in the net are adjacent in the room
// (aft↔floor, port↔floor, starboard↔floor, front↔floor,
// starboard↔ceiling), so plain orthogonal adjacency is 3D-honest
// across them — a floor lamp lights the baseboard behind it. The cut
// seams (wall↔wall corners, the ceiling's other three edges) are NOT
// net-adjacent: light does not turn corners, and neither do rats.

/// Which plane of the room a net cell lies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surf {
    Aft,
    Port,
    Floor,
    Starboard,
    Front,
    Ceiling,
}

/// Classify a net cell, or `None` where the cross has no cell.
///
/// A hole is outside the charts, or one of the two architectural
/// punch-outs: the burner doorway through the starboard chart, and the
/// transit window through the front cornice. Holes are wall that is not
/// there; nothing hangs on them.
#[must_use]
pub const fn surface_of(x: u8, y: u8) -> Option<Surf> {
    // The doorway to the burner annex: starboard rows nearest the aft
    // corner, baseboard and middle heights.
    let doorway = (x == 9 || x == 10) && (y == 3 || y == 4);
    // The transit window: two front-cornice cells left of room center.
    let window = (x == 4 || x == 5) && y == 10;
    if doorway || window {
        return None;
    }
    match (x, y) {
        (3..=8, 0..=2) => Some(Surf::Aft),
        (0..=2, 3..=7) => Some(Surf::Port),
        (3..=8, 3..=7) => Some(Surf::Floor),
        (9..=11, 3..=7) => Some(Surf::Starboard),
        (3..=8, 8..=10) => Some(Surf::Front),
        (12..=17, 3..=7) => Some(Surf::Ceiling),
        _ => None,
    }
}

/// The floor chart's bounds, `(x0, y0, w, h)` — the walkable plane the
/// sealed-floor invariant and the wall-shadow rule reason about.
pub const FLOOR: (u8, u8, u8, u8) = (3, 3, 6, 5);

/// Floor cells kept clear of standing cargo: the threshold in front of
/// the burner doorway. A doorway needs a doormat, and the stoker needs
/// to reach the hopper — coverings may lie here, nothing may stand.
pub const AISLE: [(u8, u8); 2] = [(8, 3), (8, 4)];

/// The barter surface, bottom-right: shelves, pads, dial, and accept lever.
pub const BARTER_PANEL: Rect = Rect::new(260.0, 440.0, 530.0, 150.0);

/// The station's goods on offer, top-left of the barter panel.
pub const SHELF_SLOTS: [Rect; 4] = slot_row(270.0, 448.0);

/// Goods received in a concluded trade, below the shelf.
pub const RECEIVED_SLOTS: [Rect; 4] = slot_row(270.0, 542.0);

/// What the player is offering, top-middle of the barter panel.
pub const GIVE_SLOTS: [Rect; 4] = slot_row(470.0, 448.0);

/// What the player is asking for, below the give pads.
pub const TAKE_SLOTS: [Rect; 4] = slot_row(470.0, 542.0);

/// Pull to conclude the trade on the pads.
pub const ACCEPT_LEVER: Rect = Rect::new(660.0, 530.0, 120.0, 40.0);

/// Where travel-encounter flotsam drifts: the station shelf's own four
/// sockets, doubling as the outboard rail.
///
/// The two meanings are mutually exclusive by construction — shelf goods
/// exist only docked at a trading station (barter open), drift only
/// underway or at barterless berths — so one row of wells serves both
/// without a single ambiguous drop.
pub const FLOTSAM_SLOTS: [Rect; 4] = SHELF_SLOTS;

/// The encounter badge.
///
/// Whatever is alongside shows its sign in the dial housing's corner of
/// the barter panel — which is dormant underway, when encounters happen —
/// and pressing (or dropping cargo on) it is how the player engages.
pub const ENCOUNTER_BADGE: Rect = Rect::new(666.0, 451.0, 68.0, 68.0);

/// Centre of the eagerness dial, right of the pads.
pub const DIAL_CENTER: Vec2 = Vec2::new(700.0, 485.0);

/// Slot edge length. Slots are square.
const SLOT: f32 = 40.0;

/// Horizontal spacing between slot lefts in a row.
const SLOT_STEP: f32 = 46.0;

/// A row of four slots starting at `(x, y)`.
const fn slot_row(x: f32, y: f32) -> [Rect; 4] {
    [
        Rect::new(x, y, SLOT, SLOT),
        Rect::new(x + SLOT_STEP, y, SLOT, SLOT),
        Rect::new(x + 2.0 * SLOT_STEP, y, SLOT, SLOT),
        Rect::new(x + 3.0 * SLOT_STEP, y, SLOT, SLOT),
    ]
}

/// World rect of hold cell `(x, y)`.
#[must_use]
pub fn cell_rect(x: u8, y: u8) -> Rect {
    Rect::new(
        f32::from(x).mul_add(CELL, GRID_ORIGIN.x),
        f32::from(y).mul_add(CELL, GRID_ORIGIN.y),
        CELL,
        CELL,
    )
}

/// Net cell under `p`, if any — a cell of the cross, never a hole or a
/// margin of the bounding box.
#[must_use]
pub fn cell_at(p: Vec2) -> Option<(u8, u8)> {
    let dx = p.x - GRID_ORIGIN.x;
    let dy = p.y - GRID_ORIGIN.y;
    if dx < 0.0 || dy < 0.0 {
        // Truncation rounds toward zero, so just-outside would land in the
        // edge cells without this check.
        return None;
    }
    let x = u8::try_from((dx / CELL) as i32).ok()?;
    let y = u8::try_from((dy / CELL) as i32).ok()?;
    (x < GRID_COLS && y < GRID_ROWS && surface_of(x, y).is_some()).then_some((x, y))
}

/// Slot index under `p` within a row of slots, if any.
#[must_use]
pub fn slot_at(slots: &[Rect; 4], p: Vec2) -> Option<u8> {
    slots
        .iter()
        .position(|slot| slot.contains(p))
        .map(|i| i as u8)
}

/// World rect a piece occupies at its current [`Loc`]. Shared by hit-testing
/// and the renderer, so pieces are grabbed exactly where they are drawn.
///
/// A stowed piece sits in a cubby sub-rect of its cabinet's own footprint,
/// which is why the whole board is a parameter: the cabinet must be looked
/// up. A stow whose cabinet is missing (impossible by the placement and
/// save rules) resolves to a rect parked outside the world, so it can
/// never be grabbed and never collide.
#[must_use]
pub fn piece_rect(pieces: &[Piece], piece: &Piece) -> Rect {
    match piece.loc {
        Loc::Hold { x, y } | Loc::Laid { x, y } => {
            let (w, h) = piece.kind.cells();
            let anchor = cell_rect(x, y);
            Rect::new(anchor.x, anchor.y, f32::from(w) * CELL, f32::from(h) * CELL)
        }
        Loc::StationShelf { slot } => SHELF_SLOTS[usize::from(slot)],
        Loc::GivePad { slot } => GIVE_SLOTS[usize::from(slot)],
        Loc::TakePad { slot } => TAKE_SLOTS[usize::from(slot)],
        Loc::ReceivedShelf { slot } => RECEIVED_SLOTS[usize::from(slot)],
        Loc::Flotsam { slot } => FLOTSAM_SLOTS[usize::from(slot)],
        Loc::Stow { cabinet, slot } => pieces
            .iter()
            .find(|other| other.id == cabinet)
            .map_or(Rect::new(-1000.0, -1000.0, 0.0, 0.0), |host| {
                cubby_rect(piece_rect(pieces, host), slot)
            }),
    }
}

/// The cubby sub-rect for `slot` within a cabinet body rect: a 2×2 rack,
/// row-major from the top-left. Shared by hit-testing and any renderer,
/// so a cubby is grabbed exactly where its contents are drawn.
#[must_use]
pub fn cubby_rect(body: Rect, slot: u8) -> Rect {
    let w = body.w / 2.0;
    let h = body.h / 2.0;
    Rect::new(
        f32::from(slot % 2).mul_add(w, body.x),
        f32::from(slot / 2).mul_add(h, body.y),
        w,
        h,
    )
}

/// The piece under `p`, cubby contents first and dressings last.
///
/// A stowed piece's rect lives inside its cabinet's, so scanning stows
/// before everything else is what lets a click reach into an open cubby
/// instead of always grabbing the furniture around it. Laid dressings
/// scan last for the mirror reason: a rug underlies whatever stands on
/// it, so the couch takes the click and only a bare stretch of rug
/// answers for the rug.
#[must_use]
pub fn piece_at(pieces: &[Piece], p: Vec2) -> Option<&Piece> {
    let stowed = pieces
        .iter()
        .filter(|piece| matches!(piece.loc, Loc::Stow { .. }));
    let rest = pieces
        .iter()
        .filter(|piece| !matches!(piece.loc, Loc::Stow { .. } | Loc::Laid { .. }));
    let laid = pieces
        .iter()
        .filter(|piece| matches!(piece.loc, Loc::Laid { .. }));
    stowed
        .chain(rest)
        .chain(laid)
        .find(|piece| piece_rect(pieces, piece).contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every interactive rect, named, for the pairwise checks below.
    fn interactive() -> Vec<(&'static str, Rect)> {
        let mut rects = vec![
            ("launch", LAUNCH_LEVER),
            ("accept", ACCEPT_LEVER),
            ("pause", PAUSE_BTN),
            ("warp", WARP_BTN),
            ("speaker", SPEAKER),
            (
                "grid",
                Rect::new(
                    GRID_ORIGIN.x,
                    GRID_ORIGIN.y,
                    f32::from(GRID_COLS) * CELL,
                    f32::from(GRID_ROWS) * CELL,
                ),
            ),
        ];
        // FLOTSAM_SLOTS are the shelf rects themselves (exclusive
        // contexts), so listing them would self-collide by design.
        rects.push(("encounter", ENCOUNTER_BADGE));
        for (name, row) in [
            ("shelf", &SHELF_SLOTS),
            ("received", &RECEIVED_SLOTS),
            ("give", &GIVE_SLOTS),
            ("take", &TAKE_SLOTS),
        ] {
            for (i, &slot) in row.iter().enumerate() {
                // The name survives the loop; leaking four tiny strings in a
                // test beats losing which slot collided.
                rects.push((&*format!("{name}[{i}]").leak(), slot));
            }
        }
        rects
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    /// A drop can only mean one thing: no two click/drop targets may share
    /// any area. This is the guard against a hit-test resolving somewhere
    /// the player did not aim.
    #[test]
    fn interactive_rects_never_overlap() {
        let rects = interactive();
        for (i, &(name_a, a)) in rects.iter().enumerate() {
            for &(name_b, b) in &rects[i + 1..] {
                assert!(
                    !overlaps(a, b),
                    "{name_a} and {name_b} overlap: a drop there is ambiguous"
                );
            }
        }
    }

    /// The barter furniture stays inside its panel, so hiding the panel
    /// while traveling also hides every target that needs a station.
    #[test]
    fn barter_furniture_sits_inside_the_panel() {
        let inside = |r: Rect| {
            r.x >= BARTER_PANEL.x
                && r.y >= BARTER_PANEL.y
                && r.x + r.w <= BARTER_PANEL.x + BARTER_PANEL.w
                && r.y + r.h <= BARTER_PANEL.y + BARTER_PANEL.h
        };
        for row in [&SHELF_SLOTS, &RECEIVED_SLOTS, &GIVE_SLOTS, &TAKE_SLOTS] {
            for &slot in row {
                assert!(inside(slot), "slot {slot:?} escapes the barter panel");
            }
        }
        assert!(inside(ACCEPT_LEVER));
        assert!(BARTER_PANEL.contains(DIAL_CENTER));
    }

    /// Everything sits inside the logical world.
    #[test]
    fn everything_fits_the_world() {
        for (name, r) in interactive() {
            assert!(
                r.x >= 0.0
                    && r.y >= 0.0
                    && r.x + r.w <= crate::sim::WORLD_W
                    && r.y + r.h <= crate::sim::WORLD_H,
                "{name} leaves the world"
            );
        }
    }

    /// The net's paper-craft audit: the cross's six charts cover exactly
    /// the advertised cell counts, the holes are holes, and the aisle
    /// lies on the floor.
    #[test]
    fn the_net_folds_from_six_charts() {
        let mut counts = [0usize; 6];
        for y in 0..GRID_ROWS {
            for x in 0..GRID_COLS {
                if let Some(surf) = surface_of(x, y) {
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
        assert_eq!(counts[0], 18, "aft 6x3");
        assert_eq!(counts[1], 15, "port 5x3");
        assert_eq!(counts[2], 30, "floor 6x5");
        assert_eq!(counts[3], 15 - 4, "starboard 5x3 minus the doorway");
        assert_eq!(counts[4], 18 - 2, "front 6x3 minus the window");
        assert_eq!(counts[5], 30, "ceiling 6x5");
        // The architectural holes are not cells.
        for (x, y) in [(9, 3), (10, 3), (9, 4), (10, 4), (4, 10), (5, 10)] {
            assert_eq!(surface_of(x, y), None, "({x},{y}) should be a hole");
        }
        // The doormat is floor.
        for (x, y) in AISLE {
            assert_eq!(surface_of(x, y), Some(Surf::Floor));
        }
    }

    /// The fold seams that are adjacent in the net are the ones that are
    /// adjacent in the room; a sample from each glued edge.
    #[test]
    fn fold_seams_are_watertight() {
        let pairs = [
            ((5, 2), Surf::Aft, (5, 3), Surf::Floor),
            ((2, 5), Surf::Port, (3, 5), Surf::Floor),
            ((8, 5), Surf::Floor, (9, 5), Surf::Starboard),
            ((5, 7), Surf::Floor, (5, 8), Surf::Front),
            ((11, 5), Surf::Starboard, (12, 5), Surf::Ceiling),
        ];
        for ((ax, ay), a_surf, (bx, by), b_surf) in pairs {
            assert_eq!(surface_of(ax, ay), Some(a_surf));
            assert_eq!(surface_of(bx, by), Some(b_surf));
            let adjacent = ax.abs_diff(bx) + ay.abs_diff(by) == 1;
            assert!(adjacent, "seam pair ({ax},{ay})-({bx},{by}) must touch");
        }
    }
}
