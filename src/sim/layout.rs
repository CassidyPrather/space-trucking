//! Fixed screen geometry, in world coordinates.
//!
//! The sim hit-tests against these rects and the renderer draws inside them,
//! so the two can never disagree about where a button is. Everything is a
//! constant: the console does not rearrange itself.
//!
//! The room grid lives east of the classic rects, in **net lanes**: one
//! reserved rect of logical space per attached room, indexed by its dense
//! `RoomId` (`super::room`). Lanes are fixed by id, so a room's rects are a
//! pure function of that id and no attach ever reflows another room's
//! coordinates.

use super::Vec2;
use super::cargo::{self, Loc, Piece};
use super::room::{self, RoomId, Rooms};

pub use super::room::{CELL, LANE_COLS as GRID_COLS, LANE_ROWS as GRID_ROWS, lane_origin};

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

/// Top-left corner of the cabin's lane — where the room grid used to
/// begin, back when there was only one room.
pub const GRID_ORIGIN: Vec2 = room::LANE_ORIGIN;

/// World rect of net cell `(x, y)` in room `room`.
#[must_use]
pub fn cell_rect(room: RoomId, x: u8, y: u8) -> Rect {
    let origin = lane_origin(room);
    Rect::new(
        f32::from(x).mul_add(CELL, origin.x),
        f32::from(y).mul_add(CELL, origin.y),
        CELL,
        CELL,
    )
}

/// Which room and raw net cell `p` falls in, if any.
///
/// Raw: this answers about lanes, not about the room net's validity
/// mask, because a lane's geometry is fixed and a room's charts are not.
/// `Sim::cell_at` is the arbiter that also asks whether the room exists
/// and whether the cell is a cell.
#[must_use]
pub fn cell_at(p: Vec2) -> Option<(RoomId, u8, u8)> {
    room::lane_cell_at(p)
}

/// A rect parked outside the world: what a berth nobody can name gets,
/// so it is never grabbed and never collides.
const NOWHERE: Rect = Rect::new(-1000.0, -1000.0, 0.0, 0.0);

/// World rect a piece occupies at its current [`Loc`]. Shared by hit-testing
/// and the renderer, so pieces are grabbed exactly where they are drawn.
///
/// **The graph is a parameter because a footprint is** (`cargo::plan`):
/// a berth's cells are the kind and the chart it lands on together, and
/// the chart is the room's to say. Two berths of one wardrobe are two
/// different rects, and neither of them is a property of the wardrobe.
///
/// A stowed piece sits in a cubby sub-rect of its cabinet's own footprint,
/// which is why the whole board is a parameter too: the cabinet must be
/// looked up. A stow whose cabinet is missing (impossible by the placement
/// and save rules), or a berth off its room's net, resolves to
/// [`NOWHERE`].
#[must_use]
pub fn piece_rect(rooms: &Rooms, pieces: &[Piece], piece: &Piece) -> Rect {
    match piece.loc {
        Loc::Hold { room, x, y } | Loc::Laid { room, x, y } => rooms
            .kind(room)
            .and_then(|host| cargo::plan(host, piece.kind, x, y))
            .map_or(NOWHERE, |(w, h)| {
                let anchor = cell_rect(room, x, y);
                Rect::new(anchor.x, anchor.y, f32::from(w) * CELL, f32::from(h) * CELL)
            }),
        Loc::Stow { cabinet, slot } => pieces
            .iter()
            .find(|other| other.id == cabinet)
            .map_or(NOWHERE, |host| {
                cubby_rect(piece_rect(rooms, pieces, host), slot)
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
pub fn piece_at<'a>(rooms: &Rooms, pieces: &'a [Piece], p: Vec2) -> Option<&'a Piece> {
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
        .find(|piece| piece_rect(rooms, pieces, piece).contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::room::MAX_ROOMS;

    /// Every interactive rect, named, for the pairwise checks below.
    fn interactive() -> Vec<(&'static str, Rect)> {
        let mut rects = vec![
            ("launch", LAUNCH_LEVER),
            ("pause", PAUSE_BTN),
            ("warp", WARP_BTN),
            ("speaker", SPEAKER),
        ];
        for id in 0..MAX_ROOMS as RoomId {
            let origin = lane_origin(id);
            rects.push((
                Box::leak(format!("lane[{id}]").into_boxed_str()),
                Rect::new(
                    origin.x,
                    origin.y,
                    f32::from(GRID_COLS) * CELL,
                    f32::from(GRID_ROWS) * CELL,
                ),
            ));
        }
        rects
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    /// A drop can only mean one thing: no two click/drop targets may share
    /// any area. This is the guard against a hit-test resolving somewhere
    /// the player did not aim — and, since every room now has a lane, the
    /// guard that two rooms never share a rect.
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
