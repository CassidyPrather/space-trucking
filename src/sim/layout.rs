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

/// Hold grid width, in cells.
pub const GRID_COLS: u8 = 6;

/// Hold grid height, in cells.
pub const GRID_ROWS: u8 = 4;

/// Hold cell size, in world units.
pub const CELL: f32 = 34.0;

/// Top-left corner of the hold grid.
pub const GRID_ORIGIN: Vec2 = Vec2::new(30.0, 450.0);

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

/// Where travel-encounter flotsam drifts, bottom-left of the map glass.
/// Only populated mid-leg, when nothing else on the map is clickable.
pub const FLOTSAM_SLOTS: [Rect; 2] = [
    Rect::new(20.0, 380.0, 40.0, 40.0),
    Rect::new(66.0, 380.0, 40.0, 40.0),
];

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

/// Hold cell under `p`, if any.
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
    (x < GRID_COLS && y < GRID_ROWS).then_some((x, y))
}

/// Slot index under `p` within a row of slots, if any.
#[must_use]
pub fn slot_at(slots: &[Rect; 4], p: Vec2) -> Option<u8> {
    slots
        .iter()
        .position(|slot| slot.contains(p))
        .map(|i| i as u8)
}

/// [`slot_at`] for the two-slot flotsam row.
#[must_use]
pub fn slot_at2(slots: &[Rect; 2], p: Vec2) -> Option<u8> {
    slots
        .iter()
        .position(|slot| slot.contains(p))
        .map(|i| i as u8)
}

/// World rect a piece occupies at its current [`Loc`]. Shared by hit-testing
/// and the renderer, so pieces are grabbed exactly where they are drawn.
#[must_use]
pub fn piece_rect(piece: &Piece) -> Rect {
    match piece.loc {
        Loc::Hold { x, y } => {
            let (w, h) = piece.kind.cells();
            let anchor = cell_rect(x, y);
            Rect::new(anchor.x, anchor.y, f32::from(w) * CELL, f32::from(h) * CELL)
        }
        Loc::StationShelf { slot } => SHELF_SLOTS[usize::from(slot)],
        Loc::GivePad { slot } => GIVE_SLOTS[usize::from(slot)],
        Loc::TakePad { slot } => TAKE_SLOTS[usize::from(slot)],
        Loc::ReceivedShelf { slot } => RECEIVED_SLOTS[usize::from(slot)],
        Loc::Flotsam { slot } => FLOTSAM_SLOTS[usize::from(slot)],
    }
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
        for (i, &slot) in FLOTSAM_SLOTS.iter().enumerate() {
            rects.push((&*format!("flotsam[{i}]").leak(), slot));
        }
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
}
