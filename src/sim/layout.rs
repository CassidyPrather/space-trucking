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

/// One fine unit (`cargo::FINE`, a 256th of a cell), in world units.
/// `CELL` is 34, so this is 0.1328125 — 17/128, exact in binary, which
/// is what keeps a whole cell's berth on the very rect its cell has.
const FINE_UNIT: f32 = CELL / cargo::FINE as f32;

/// World rect of a box of net `room` given in fine units.
#[must_use]
pub fn fine_rect(room: RoomId, span: cargo::Aabb) -> Rect {
    let origin = lane_origin(room);
    Rect::new(
        (span.x0 as f32).mul_add(FINE_UNIT, origin.x),
        (span.y0 as f32).mul_add(FINE_UNIT, origin.y),
        span.w() as f32 * FINE_UNIT,
        span.h() as f32 * FINE_UNIT,
    )
}

/// World rect of the box round footprint `foot` in room `room`: the
/// footprint itself at a quarter turn, and its bounds at any other.
#[must_use]
pub fn foot_rect(room: RoomId, foot: cargo::Foot) -> Rect {
    fine_rect(room, foot.aabb())
}

/// **Where `p` falls on room `room`'s net, in `cargo::FINE` units**,
/// which may lie off the net or be negative: the caller clamps.
///
/// The one float-to-integer step a drop takes, so it is stated exactly:
/// the offset from the lane's corner is divided by [`FINE_UNIT`] and
/// rounded to the nearest whole unit, halves away from zero
/// (`f32::round`), then converted once to `i32`. Everything after this
/// is integer arithmetic. A fine unit is exact in binary and so is a
/// cell's centre, so aiming at the middle of a cell names its middle
/// unit on every machine a crew plays on.
#[must_use]
pub fn fine_at(room: RoomId, p: Vec2) -> (i32, i32) {
    let origin = lane_origin(room);
    (
        ((p.x - origin.x) / FINE_UNIT).round() as i32,
        ((p.y - origin.y) / FINE_UNIT).round() as i32,
    )
}

/// **How finely a pointer is read for hit-testing**: sub-units per fine
/// unit. A berth is placed in whole fine units ([`fine_at`]), but a
/// reading taken a hair inside a piece's rim — which is where a frontend
/// reads the edge of a body it struck — must stay inside it, and a
/// whole unit is wider than the hair.
const READING: i64 = 256;

/// How far off a net a reading may lie, in [`READING`] units: far past
/// every lane, and short of anything the frame arithmetic could overflow
/// on. A pointer out there is on nothing either way.
const READING_REACH: f64 = (1_i64 << 40) as f64;

/// **Where `p` falls on room `room`'s net**, in `1 / READING` of a fine
/// unit: the pointer read once, finely, for asking which piece it is on.
/// `None` for a pointer that is not a point at all (a NaN, an infinity),
/// which no piece contains.
///
/// The one rounding a hit-test takes, stated as exactly as the drop's
/// ([`fine_at`]): in `f64`, offset from the lane's corner, over
/// [`FINE_UNIT`], times [`READING`], held within [`READING_REACH`], and
/// rounded halves away from zero. Every step is basic IEEE arithmetic,
/// correctly rounded on every platform, so a crew agrees on which piece a
/// press lands on.
fn net_point(room: RoomId, p: Vec2) -> Option<(i64, i64)> {
    if !(p.x.is_finite() && p.y.is_finite()) {
        return None;
    }
    let origin = lane_origin(room);
    let read = |at: f32, from: f32| {
        ((f64::from(at) - f64::from(from)) / f64::from(FINE_UNIT) * READING as f64)
            .clamp(-READING_REACH, READING_REACH)
            .round() as i64
    };
    Some((read(p.x, origin.x), read(p.y, origin.y)))
}

/// A rect parked outside the world: what a berth nobody can name gets,
/// so it is never drawn anywhere it could be mistaken for a piece.
const NOWHERE: Rect = Rect::new(-1000.0, -1000.0, 0.0, 0.0);

/// **World rect a piece's berth spans** at its current [`Loc`]: the box
/// round it on the net's own axes.
///
/// A drawing's bounds — never an answer to "which piece is at this
/// point", which is [`piece_contains`]'s, because the box round a couch
/// turned an eighth is mostly the air beside it.
///
/// **The graph is a parameter because a footprint is** (`cargo::Foot::of`):
/// a berth's ground is the kind and the chart it lands on together, and
/// the chart is the room's to say. Two berths of one wardrobe are two
/// different rects, and neither of them is a property of the wardrobe.
///
/// The rect is the berth's TRUE one, fractions of a cell and all: a
/// piece standing five units off the grid is drawn five units off the
/// grid. A fine unit is `CELL / 256` exactly, so a berth on a whole cell
/// at a quarter turn lands on the very rect [`cell_rect`] gives that
/// cell.
///
/// A stowed piece spans its cubby: a quarter of its cabinet's own
/// footprint, read in the cabinet's own frame (`cargo::Foot::quarter_box`),
/// which is why the whole board is a parameter too. A stow whose cabinet
/// is missing (impossible by the placement and save rules), or a berth
/// off its room's net, resolves to [`NOWHERE`].
#[must_use]
pub fn piece_rect(rooms: &Rooms, pieces: &[Piece], piece: &Piece) -> Rect {
    match piece.loc {
        Loc::Hold { room, .. } | Loc::Laid { room, .. } => {
            cargo::Foot::at(rooms, piece).map_or(NOWHERE, |(_, foot)| foot_rect(room, foot))
        }
        Loc::Stow { cabinet, slot } => pieces
            .iter()
            .find(|other| other.id == cabinet)
            .and_then(|host| cargo::Foot::at(rooms, host))
            .map_or(NOWHERE, |(room, foot)| {
                fine_rect(room, foot.quarter_box(slot))
            }),
    }
}

/// Where `p` lies in the frame of the footprint `piece` stands on, if it
/// stands in a room: read on that room's net ([`net_point`]) and carried
/// into the piece's own frame (`cargo::Foot::frame`).
fn framed(rooms: &Rooms, piece: &Piece, p: Vec2) -> Option<cargo::Framed> {
    let (room, foot) = cargo::Foot::at(rooms, piece)?;
    Some(foot.frame(net_point(room, p)?, READING))
}

/// **Whether `p` is on `piece`**: on the footprint it stands or lies on,
/// at whatever turn, or — for a stowed piece — in its cubby.
///
/// The footprint is the oriented one, and the test is made in the
/// piece's OWN frame: the pointer is carried into it, never the piece out
/// onto the sheet's axes, so the air beside a turned couch is not the
/// couch. A cubby is a quarter of its cabinet read the same way, row-major
/// from the cabinet's own top left as a person facing it sees it — the
/// rack its rig draws.
#[must_use]
pub fn piece_contains(rooms: &Rooms, pieces: &[Piece], piece: &Piece, p: Vec2) -> bool {
    match piece.loc {
        Loc::Hold { .. } | Loc::Laid { .. } => {
            framed(rooms, piece, p).is_some_and(cargo::Framed::inside)
        }
        Loc::Stow { cabinet, slot } => pieces
            .iter()
            .find(|other| other.id == cabinet)
            .and_then(|host| framed(rooms, host, p))
            .is_some_and(|at| at.inside() && at.quarter() == slot),
    }
}

/// **Where `p` lies on `piece`'s own face**, as fractions.
///
/// `(0, 0)` at its top left and `(1, 1)` at its bottom right as a person
/// facing it sees it, whatever its turn. `None` for a piece with no
/// ground of its own (a cubby's).
///
/// For a frontend measuring a sub-rect it declared in the piece's own
/// units — a carry handle — against the pointer, so the band it draws and
/// the band it routes are one band at every angle.
#[must_use]
pub fn piece_frame(rooms: &Rooms, piece: &Piece, p: Vec2) -> Option<Vec2> {
    framed(rooms, piece, p).map(|at| {
        let (x, y) = at.fractions();
        Vec2::new(x, y)
    })
}

/// The piece under `p`, cubby contents first and dressings last.
///
/// A stowed piece's cubby lives inside its cabinet, so scanning stows
/// before everything else is what lets a click reach into an open cubby
/// instead of always grabbing the furniture around it. Laid dressings
/// scan last for the mirror reason: a rug underlies whatever stands on
/// it, so the couch takes the click and only a bare stretch of rug
/// answers for the rug. Each is asked [`piece_contains`], so a turned
/// piece answers for its own ground and nothing beside it.
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
        .find(|piece| piece_contains(rooms, pieces, piece, p))
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
