//! The ship rat: the game's second event, sibling to the omen machine.
//!
//! DESIGN.md's Events list asks for "rats on the ship which will damage the
//! cargo ... unless dealt with", triggered by cargo distribution and — like
//! every event — feasible to ignore. This module is that, whole: a crowded
//! hold (at least half the grid stowed) rolls one-in-four at each departure
//! for a stowaway, the rat lives in one hold cell and skitters between them
//! every ten seconds or so, and every ~45 s it gnaws the piece nearest its
//! cell, which is then worth a little less at every station forever (see
//! `barter::GNAW_MALUS`). The ignore path works exactly as the design doc's
//! "cargo distribution" trigger suggests: haul lean — a third of the grid or
//! less at any dock — and the rat walks off on its own, nothing to eat. The
//! engaged path is pressing its cell: a press chases it (it hops instantly),
//! and the third chase drives it off the ship for good.
//!
//! Rats never board while a suspicious crate hums in the hold — the hum
//! unnerves them at the gangway — so crate legs are a rat-free blessing and
//! the ship keeps to one weirdness at a time. (A rat already aboard merely
//! hides in the walls: acquiring a crate later does not evict it.)
//!
//! A rat never destroys, moves, or blocks anything: cargo conservation and
//! every stowage invariant hold with one aboard. Its single mark on the
//! world is the permanent `gnawed` flag on a piece.
//!
//! **Repair is deliberately deferred.** The design doc's "requiring repair"
//! reading would let a gnawed piece be mended; this pass keeps the bite
//! permanent, a scar the cargo carries through the economy (stations resell
//! gnawed goods gnawed). When repair arrives it belongs here, next to the
//! teeth that made it necessary.
//!
//! Structurally this is one of the two event siblings: its own state
//! struct, deterministic schedules hashed off the seed, `on_depart` /
//! `on_dock` / `on_tick` / `on_press` hooks called from `Sim`, its own save
//! line, its own cues. `event::Omen` keeps the same shape; a third event
//! should copy the convention rather than grow a framework.

use super::cargo::{Loc, Piece};
use super::layout::{self, GRID_COLS, GRID_ROWS};
use super::{Cue, Vec2, splitmix};

/// Total hold cells.
const GRID_CELLS: u32 = GRID_COLS as u32 * GRID_ROWS as u32;

/// Boarding gate: a rat only stows away when at least this many hold cells
/// are under cargo — half the grid.
pub const CROWDED_CELLS: u32 = GRID_CELLS / 2;

/// Walk-off gate: docking with at most this many hold cells under cargo —
/// a third of the grid — sends the rat ashore. Nothing to eat.
pub const SPARSE_CELLS: u32 = GRID_CELLS / 3;

/// One crowded departure in this many rolls a stowaway.
pub const BOARD_CHANCE: u64 = 4;

/// Chases before the rat abandons ship entirely.
pub const CHASE_LIMIT: u8 = 3;

/// Relocation cadence: 8 s base plus up to 4 s of jitter (~10 s mean).
const MOVE_BASE: u64 = 480;
const MOVE_JITTER: u64 = 240;

/// Nibble cadence: 40 s base plus up to 10 s of jitter (~45 s mean).
const NIBBLE_BASE: u64 = 2400;
const NIBBLE_JITTER: u64 = 600;

/// Stream salts, so the boarding roll, the hop draws, and the chase draws
/// never collide with each other or with any other derived stream.
pub const SALT_BOARD: u64 = 0x2A75_B04D;
const SALT_MOVE: u64 = 0x2A75_3C17;
const SALT_NIBBLE: u64 = 0x2A75_4B1E;
const SALT_CHASE: u64 = 0x2A75_C4A5;

/// One stowaway. At most one exists at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rat {
    /// The hold cell it currently occupies.
    pub cell: (u8, u8),
    /// The cell it last hopped from, for the renderer's hop tween.
    pub prev_cell: (u8, u8),
    /// Tick of that hop, for the same tween. Cosmetic to read, exact to save.
    pub moved_at: u64,
    /// Tick of the next scheduled relocation.
    pub next_move: u64,
    /// Tick of the next gnaw.
    pub next_nibble: u64,
    /// Times chased so far, `0..CHASE_LIMIT`.
    pub chases: u8,
}

/// The rat event's whole state: the stowaway, if one is aboard.
#[derive(Clone, Debug)]
pub struct Rats {
    pub rat: Option<Rat>,
}

impl Rats {
    pub const fn new() -> Self {
        Self { rat: None }
    }

    /// Called once per departure, after the hold is all that remains: a
    /// crowded, crate-free, rat-free hold rolls `1 / BOARD_CHANCE` on the
    /// `(seed, legs)` stream for a stowaway. It boards an empty cell when
    /// one exists (perching on cargo only in a completely full hold) with
    /// both schedules wound from the departure tick.
    pub fn on_depart(
        &mut self,
        seed: u64,
        legs: u64,
        tick: u64,
        pieces: &[Piece],
        suspicious: bool,
        cues: &mut Vec<Cue>,
    ) {
        if self.rat.is_some() || suspicious || occupied_cells(pieces) < CROWDED_CELLS {
            return;
        }
        if splitmix(seed ^ SALT_BOARD, legs) % BOARD_CHANCE != 0 {
            return;
        }
        let h = splitmix(seed ^ SALT_MOVE, tick);
        let cell = choose_cell(h, pieces, None);
        self.rat = Some(Rat {
            cell,
            prev_cell: cell,
            moved_at: tick,
            next_move: tick + MOVE_BASE + (h >> 16) % MOVE_JITTER,
            next_nibble: tick + NIBBLE_BASE + (h >> 32) % NIBBLE_JITTER,
            chases: 0,
        });
        cues.push(Cue::RatAboard);
    }

    /// Called once per arrival: a hold at [`SPARSE_CELLS`] or fewer stowed
    /// cells sends the rat ashore — the ignore path. It otherwise persists
    /// across dockings; it lives in the hold, not the station.
    pub fn on_dock(&mut self, pieces: &[Piece], cues: &mut Vec<Cue>) {
        if self.rat.is_some() && occupied_cells(pieces) <= SPARSE_CELLS {
            self.rat = None;
            cues.push(Cue::RatLeft);
        }
    }

    /// Every unpaused tick: run the relocation and nibble schedules. A hop
    /// re-arms `next_move` and skitters quietly; a nibble re-arms
    /// `next_nibble` and gnaws the nearest stowed piece (see
    /// [`nearest_hold_piece`] for the rule) — permanently, though a re-gnaw
    /// of an already-bitten piece changes nothing but the sound.
    pub fn on_tick(&mut self, seed: u64, tick: u64, pieces: &mut [Piece], cues: &mut Vec<Cue>) {
        let Some(rat) = &mut self.rat else {
            return;
        };
        if tick >= rat.next_move {
            let h = splitmix(seed ^ SALT_MOVE, tick);
            let from = rat.cell;
            rat.cell = choose_cell(h, pieces, Some(from));
            rat.prev_cell = from;
            rat.moved_at = tick;
            rat.next_move = tick + MOVE_BASE + (h >> 16) % MOVE_JITTER;
            let intensity = ((h >> 40) % 350) as f32 / 1000.0 + 0.2;
            cues.push(Cue::RatSkitter { intensity });
        }
        if tick >= rat.next_nibble {
            let h = splitmix(seed ^ SALT_NIBBLE, tick);
            rat.next_nibble = tick + NIBBLE_BASE + h % NIBBLE_JITTER;
            if let Some(index) = nearest_hold_piece(pieces, rat.cell) {
                pieces[index].gnawed = true;
                cues.push(Cue::RatNibble);
            }
        }
    }

    /// Press precedence, called before any piece grab: a press inside the
    /// rat's cell chases it — it hops instantly, and the piece under it (if
    /// any) is NOT lifted. The third chase drives it off the ship. Returns
    /// whether the press was consumed; a `false` leaves the press to the
    /// ordinary pointer paths, which are unchanged.
    pub fn on_press(
        &mut self,
        seed: u64,
        tick: u64,
        p: Vec2,
        pieces: &[Piece],
        cues: &mut Vec<Cue>,
    ) -> bool {
        let Some(rat) = &mut self.rat else {
            return false;
        };
        if !layout::cell_rect(rat.cell.0, rat.cell.1).contains(p) {
            return false;
        }
        rat.chases += 1;
        cues.push(Cue::RatChased);
        if rat.chases >= CHASE_LIMIT {
            self.rat = None;
            cues.push(Cue::RatLeft);
        } else {
            let h = splitmix(seed ^ SALT_CHASE, tick);
            let from = rat.cell;
            rat.cell = choose_cell(h, pieces, Some(from));
            rat.prev_cell = from;
            rat.moved_at = tick;
            rat.next_move = tick + MOVE_BASE + (h >> 16) % MOVE_JITTER;
        }
        true
    }
}

/// Hold cells covered by stowed pieces — the "cargo distribution" number
/// both gates read.
pub fn occupied_cells(pieces: &[Piece]) -> u32 {
    pieces
        .iter()
        .filter_map(|piece| match piece.loc {
            Loc::Hold { .. } => {
                let (w, h) = piece.kind.cells();
                Some(u32::from(w) * u32::from(h))
            }
            _ => None,
        })
        .sum()
}

/// Whether hold cell `(cx, cy)` sits under a stowed piece's footprint.
fn covered(pieces: &[Piece], cx: u8, cy: u8) -> bool {
    pieces.iter().any(|piece| {
        let Loc::Hold { x, y } = piece.loc else {
            return false;
        };
        let (w, h) = piece.kind.cells();
        cx >= x && cx < x + w && cy >= y && cy < y + h
    })
}

/// Deterministically pick the rat's cell from hash `h`: empty cells
/// preferred (the rat likes bare floor), the cell it is leaving excluded
/// whenever any other choice exists, candidates in row-major order indexed
/// by the hash. Only a completely full hold makes it perch on a piece.
fn choose_cell(h: u64, pieces: &[Piece], avoid: Option<(u8, u8)>) -> (u8, u8) {
    let mut empty = Vec::new();
    let mut any = Vec::new();
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            if avoid == Some((x, y)) {
                continue;
            }
            any.push((x, y));
            if !covered(pieces, x, y) {
                empty.push((x, y));
            }
        }
    }
    let pool = if empty.is_empty() { &any } else { &empty };
    pool[(h % pool.len() as u64) as usize]
}

/// THE nibble target rule: the stowed piece nearest the rat's cell by
/// Manhattan distance to the closest cell of its footprint (zero when the
/// rat perches on it), ties broken by the lower piece id. Returns an index
/// into `pieces`; `None` when nothing is stowed at all (a bare hold at a
/// dock), which skips the nibble.
fn nearest_hold_piece(pieces: &[Piece], (cx, cy): (u8, u8)) -> Option<usize> {
    pieces
        .iter()
        .enumerate()
        .filter_map(|(index, piece)| {
            let Loc::Hold { x, y } = piece.loc else {
                return None;
            };
            let (w, h) = piece.kind.cells();
            let distance = u32::from(axis_gap(cx, x, w)) + u32::from(axis_gap(cy, y, h));
            Some((distance, piece.id, index))
        })
        .min_by_key(|&(distance, id, _)| (distance, id))
        .map(|(_, _, index)| index)
}

/// Cells between `c` and the span `[start, start + len)` on one axis.
const fn axis_gap(c: u8, start: u8, len: u8) -> u8 {
    if c < start {
        start - c
    } else if c >= start + len {
        c - (start + len - 1)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::cargo::Kind;
    use super::*;

    /// A stowed piece for the pure helpers below.
    const fn hold_piece(id: u32, kind: Kind, x: u8, y: u8) -> Piece {
        Piece {
            id,
            kind,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold { x, y },
        }
    }

    #[test]
    fn occupied_cells_sums_footprints_and_ignores_the_shelf() {
        let pieces = [
            hold_piece(0, Kind::RationBricks, 0, 0), // 2x2 = 4 cells
            hold_piece(1, Kind::PerfumeVial, 3, 3),  // 1 cell
            Piece {
                id: 2,
                kind: Kind::RationBricks,
                variant: 0,
                gnawed: false,
                loc: Loc::StationShelf { slot: 0 },
            },
        ];
        assert_eq!(occupied_cells(&pieces), 5);
        assert_eq!(occupied_cells(&[]), 0);
    }

    #[test]
    fn cell_choice_prefers_empty_cells_and_avoids_the_current_one() {
        // Everything covered except (0, 0).
        let pieces = [
            hold_piece(0, Kind::RationBricks, 2, 0),
            hold_piece(1, Kind::RationBricks, 4, 0),
            hold_piece(2, Kind::RationBricks, 0, 2),
            hold_piece(3, Kind::RationBricks, 2, 2),
            hold_piece(4, Kind::RationBricks, 4, 2),
            hold_piece(5, Kind::PerfumeVial, 1, 0),
            hold_piece(6, Kind::PerfumeVial, 0, 1),
            hold_piece(7, Kind::PerfumeVial, 1, 1),
        ];
        // One free cell: every hash lands on it.
        for h in 0..50 {
            assert_eq!(choose_cell(h, &pieces, None), (0, 0));
        }
        // With that cell the one to avoid, the rat perches on cargo instead
        // of staying put, and the draw stays deterministic.
        let perch = choose_cell(7, &pieces, Some((0, 0)));
        assert_ne!(perch, (0, 0));
        assert_eq!(perch, choose_cell(7, &pieces, Some((0, 0))));
    }

    #[test]
    fn the_nearest_piece_rule_measures_to_the_footprint_and_breaks_ties_low() {
        let pieces = [
            hold_piece(4, Kind::RationBricks, 0, 0), // footprint out to (1, 1)
            hold_piece(2, Kind::PerfumeVial, 5, 0),
        ];
        // (2, 0): the bricks' edge is 1 away, the vial 3: bricks.
        assert_eq!(nearest_hold_piece(&pieces, (2, 0)), Some(0));
        // (3, 0): both 2 away — the lower id wins, which is the vial.
        assert_eq!(nearest_hold_piece(&pieces, (3, 0)), Some(1));
        // Perched on a footprint is distance zero.
        assert_eq!(nearest_hold_piece(&pieces, (1, 1)), Some(0));
        // Nothing stowed, nothing to gnaw.
        assert_eq!(nearest_hold_piece(&[], (0, 0)), None);
    }
}
