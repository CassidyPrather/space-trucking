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
//! Two fixtures bend the rat's routine (see `docs/FIXTURES.md`). Rats fear
//! light: no hop lands in, no boarding starts in, and no nibble happens in
//! a cell beside a lit lamp (`cargo::lit_adjacent`) — when every candidate
//! reads lit, the rat simply skips that beat and re-arms its schedule, so
//! light is deterrence, never damage. And the couch tempts it: with one
//! stowed, hops become single steps toward the nearest couch cell
//! (clambering over cargo if it must — the couch itself is covered ground),
//! and once aboard it naps — no nibbling, hop cadence stretched by
//! [`NAP_LAZE`] — until an ordinary hop rolls it back off. Both behaviours
//! derive from the pieces each beat; the rat carries no new state.
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

use super::cargo::{Kind, Loc, Piece, lit_adjacent};
use super::layout;
use super::room::{CABIN, RoomKind, Surf};
use super::{Cue, Vec2, splitmix};

/// The rat is a cabin animal: it stows away in the room you live in and
/// never crosses a seam. Every cell it reasons about is one of the
/// cabin's, so its whole world is that room's net.
const RATS_ROOM: RoomKind = RoomKind::Cabin;

/// The cabin net's bounding grid, the rat's whole country.
const GRID_COLS: u8 = RATS_ROOM.grid().0;
const GRID_ROWS: u8 = RATS_ROOM.grid().1;

/// The floor's cell count — the crowding gates' yardstick. The net has
/// far more cells than that, but food density is a floor phenomenon;
/// walls of paintings never fed anybody.
const FLOOR_CELLS: u32 = RATS_ROOM.floor_rect().2 as u32 * RATS_ROOM.floor_rect().3 as u32;

/// Boarding gate: a rat only stows away when at least this many cells
/// are under cargo — half the floor.
pub const CROWDED_CELLS: u32 = FLOOR_CELLS / 2;

/// Walk-off gate: docking with at most this many cells under cargo —
/// a third of the floor — sends the rat ashore. Nothing to eat.
pub const SPARSE_CELLS: u32 = FLOOR_CELLS / 3;

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

/// Hop cadence multiplier while napping on the couch: three times lazier.
pub const NAP_LAZE: u64 = 3;

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
    /// both schedules wound from the departure tick. Lamplit cells are
    /// never boarded; a hold lit wall to wall boards nothing at all.
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
        let Some(cell) = choose_cell(h, pieces, None) else {
            return;
        };
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
    ///
    /// The fixtures cut in here: a stowed couch turns free hops into
    /// single steps toward it ([`couch_step`]), a rat on the couch naps —
    /// no gnawing, `next_move` re-armed [`NAP_LAZE`] times slower — and
    /// lamplight vetoes hop targets and nibbles alike. A beat with nowhere
    /// legal to go, or nothing napless to do, is skipped: the schedule
    /// re-arms and nothing else happens, so neither light nor comfort ever
    /// destroys anything.
    pub fn on_tick(&mut self, seed: u64, tick: u64, pieces: &mut [Piece], cues: &mut Vec<Cue>) {
        let Some(rat) = &mut self.rat else {
            return;
        };
        if tick >= rat.next_move {
            let h = splitmix(seed ^ SALT_MOVE, tick);
            let couch = couch_cells(pieces);
            let target = if couch.is_empty() || couch.contains(&rat.cell) {
                choose_cell(h, pieces, Some(rat.cell))
            } else {
                couch_step(h, pieces, &couch, rat.cell)
            };
            if let Some(cell) = target {
                rat.prev_cell = rat.cell;
                rat.cell = cell;
                rat.moved_at = tick;
                let intensity = ((h >> 40) % 350) as f32 / 1000.0 + 0.2;
                cues.push(Cue::RatSkitter { intensity });
            }
            // The laze reads the cell the rat will spend the beat in.
            let laze = if couch.contains(&rat.cell) {
                NAP_LAZE
            } else {
                1
            };
            rat.next_move = tick + laze * (MOVE_BASE + (h >> 16) % MOVE_JITTER);
        }
        if tick >= rat.next_nibble {
            let h = splitmix(seed ^ SALT_NIBBLE, tick);
            rat.next_nibble = tick + NIBBLE_BASE + h % NIBBLE_JITTER;
            let napping = couch_cells(pieces).contains(&rat.cell);
            if !napping && !lit_adjacent(pieces, CABIN, rat.cell.0, rat.cell.1) {
                if let Some(index) = nearest_hold_piece(pieces, rat.cell) {
                    pieces[index].gnawed = true;
                    cues.push(Cue::RatNibble);
                }
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
        if !layout::cell_rect(CABIN, rat.cell.0, rat.cell.1).contains(p) {
            return false;
        }
        rat.chases += 1;
        cues.push(Cue::RatChased);
        if rat.chases >= CHASE_LIMIT {
            self.rat = None;
            cues.push(Cue::RatLeft);
        } else {
            let h = splitmix(seed ^ SALT_CHASE, tick);
            // A chased rat bolts anywhere unlit — off the couch too. Only
            // a hold lit wall to wall leaves it cowering in place (still
            // chased: the count stands).
            if let Some(cell) = choose_cell(h, pieces, Some(rat.cell)) {
                rat.prev_cell = rat.cell;
                rat.cell = cell;
                rat.moved_at = tick;
            }
            rat.next_move = tick + MOVE_BASE + (h >> 16) % MOVE_JITTER;
        }
        true
    }
}

/// FLOOR cells covered by stowed pieces — the "cargo distribution"
/// number both gates read. Wall and ceiling berths never count: food
/// density is a floor phenomenon (see [`FLOOR_CELLS`]), and a ship
/// whose walls fill with instruments is not thereby a buffet.
pub fn occupied_cells(pieces: &[Piece]) -> u32 {
    pieces
        .iter()
        .filter_map(|piece| match piece.loc {
            Loc::Hold { room: CABIN, x, y } => {
                let (w, h) = piece.kind.cells();
                let on_floor = RATS_ROOM
                    .surface_of(x, y)
                    .is_some_and(|surf| matches!(surf, Surf::Floor));
                on_floor.then_some(u32::from(w) * u32::from(h))
            }
            _ => None,
        })
        .sum()
}

/// Whether hold cell `(cx, cy)` sits under a stowed piece's footprint.
fn covered(pieces: &[Piece], cx: u8, cy: u8) -> bool {
    pieces.iter().any(|piece| {
        let Loc::Hold { room: CABIN, x, y } = piece.loc else {
            return false;
        };
        let (w, h) = piece.kind.cells();
        cx >= x && cx < x + w && cy >= y && cy < y + h
    })
}

/// Deterministically pick the rat's cell from hash `h`: empty cells
/// preferred (the rat likes bare floor), lamplit cells refused outright
/// (see [`lit_adjacent`] — rats fear light), the cell it is leaving
/// excluded whenever any other choice exists, candidates in row-major
/// order indexed by the hash. Only a full hold makes it perch on a piece,
/// and `None` — every candidate lit — makes it skip the beat entirely.
fn choose_cell(h: u64, pieces: &[Piece], avoid: Option<(u8, u8)>) -> Option<(u8, u8)> {
    let mut empty = Vec::new();
    let mut any = Vec::new();
    for y in 0..GRID_ROWS {
        for x in 0..GRID_COLS {
            // The whole net is rat country — floor, walls, ceiling; a
            // rat does not care which way is down. Holes are holes.
            if RATS_ROOM.surface_of(x, y).is_none() {
                continue;
            }
            if avoid == Some((x, y)) || lit_adjacent(pieces, CABIN, x, y) {
                continue;
            }
            any.push((x, y));
            if !covered(pieces, x, y) {
                empty.push((x, y));
            }
        }
    }
    let pool = if empty.is_empty() { &any } else { &empty };
    if pool.is_empty() {
        return None;
    }
    Some(pool[(h % pool.len() as u64) as usize])
}

/// Hold cells under stowed couches, in stowage order. Empty when no couch
/// is aboard. Derived fresh each beat: the couch is the pieces' state, not
/// the rat's.
fn couch_cells(pieces: &[Piece]) -> Vec<(u8, u8)> {
    let mut cells = Vec::new();
    for piece in pieces {
        if piece.kind != Kind::Couch {
            continue;
        }
        let Loc::Hold { room: CABIN, x, y } = piece.loc else {
            continue;
        };
        let (w, h) = piece.kind.cells();
        for dy in 0..h {
            for dx in 0..w {
                cells.push((x + dx, y + dy));
            }
        }
    }
    cells
}

/// Manhattan distance from `cell` to the nearest couch cell.
fn couch_gap(couch: &[(u8, u8)], cell: (u8, u8)) -> u32 {
    couch
        .iter()
        .map(|&(x, y)| u32::from(cell.0.abs_diff(x)) + u32::from(cell.1.abs_diff(y)))
        .min()
        .unwrap_or(u32::MAX)
}

/// The couch drift: one orthogonal step that shrinks the Manhattan
/// distance to the nearest couch cell, the hash picking among the legal
/// candidates (fixed up/down/left/right order, so ties break on the same
/// splitmix stream as every other draw). Light fear still applies; cargo
/// underfoot does not — the couch itself is covered ground, so a drift
/// that refused footprints could never arrive. `None` when lamplight or
/// geometry blocks every step: the rat waits out the beat where it is.
fn couch_step(
    h: u64,
    pieces: &[Piece],
    couch: &[(u8, u8)],
    (cx, cy): (u8, u8),
) -> Option<(u8, u8)> {
    let here = couch_gap(couch, (cx, cy));
    let mut pool = Vec::new();
    let mut consider = |cell: (u8, u8)| {
        if couch_gap(couch, cell) < here && !lit_adjacent(pieces, CABIN, cell.0, cell.1) {
            pool.push(cell);
        }
    };
    if cy > 0 {
        consider((cx, cy - 1));
    }
    if cy + 1 < GRID_ROWS {
        consider((cx, cy + 1));
    }
    if cx > 0 {
        consider((cx - 1, cy));
    }
    if cx + 1 < GRID_COLS {
        consider((cx + 1, cy));
    }
    if pool.is_empty() {
        return None;
    }
    Some(pool[(h % pool.len() as u64) as usize])
}

/// THE nibble target rule: the stowed or laid piece nearest the rat's
/// cell by Manhattan distance to the closest cell of its footprint (zero
/// when the rat perches on it), ties broken by the lower piece id. Laid
/// dressings count — a rug is famously gnawable — but cubby cargo never
/// does (`Loc::Stow` has no cell here at all). Returns an index into
/// `pieces`; `None` when nothing is reachable (a bare hold at a dock),
/// which skips the nibble.
fn nearest_hold_piece(pieces: &[Piece], (cx, cy): (u8, u8)) -> Option<usize> {
    pieces
        .iter()
        .enumerate()
        .filter_map(|(index, piece)| {
            let (Loc::Hold { room: CABIN, x, y } | Loc::Laid { room: CABIN, x, y }) = piece.loc
            else {
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
            loc: Loc::Hold { room: CABIN, x, y },
        }
    }

    #[test]
    fn occupied_cells_sums_floor_footprints_and_ignores_the_rest() {
        let pieces = [
            hold_piece(0, Kind::RationBricks, 4, 4), // 2x2 = 4 floor cells
            hold_piece(1, Kind::PerfumeVial, 3, 3),  // 1 floor cell
            // Wall and ceiling berths are not food density.
            hold_piece(2, Kind::ChartTank, 4, 0),
            hold_piece(3, Kind::CeilingLamp, 14, 5),
            Piece {
                id: 4,
                kind: Kind::RationBricks,
                variant: 0,
                gnawed: false,
                loc: Loc::Stow {
                    cabinet: 0,
                    slot: 0,
                },
            },
        ];
        assert_eq!(occupied_cells(&pieces), 5);
        assert_eq!(occupied_cells(&[]), 0);
    }

    #[test]
    fn cell_choice_prefers_empty_cells_and_avoids_the_current_one() {
        // Every net cell covered except (5, 5): a vial on each of them.
        let mut pieces = Vec::new();
        for y in 0..GRID_ROWS {
            for x in 0..GRID_COLS {
                if RATS_ROOM.surface_of(x, y).is_none() || (x, y) == (5, 5) {
                    continue;
                }
                pieces.push(hold_piece(pieces.len() as u32, Kind::PerfumeVial, x, y));
            }
        }
        // One free cell: every hash lands on it.
        for h in 0..50 {
            assert_eq!(choose_cell(h, &pieces, None), Some((5, 5)));
        }
        // With that cell the one to avoid, the rat perches on cargo instead
        // of staying put, and the draw stays deterministic.
        let perch = choose_cell(7, &pieces, Some((5, 5)));
        assert_ne!(perch, Some((5, 5)));
        assert!(perch.is_some(), "an unlit hold always has somewhere to go");
        assert_eq!(perch, choose_cell(7, &pieces, Some((5, 5))));
    }

    /// A rat mid-tenure with both schedules due almost immediately.
    const fn wound_rat(cell: (u8, u8)) -> Rats {
        Rats {
            rat: Some(Rat {
                cell,
                prev_cell: cell,
                moved_at: 0,
                next_move: 1,
                next_nibble: 2,
                chases: 0,
            }),
        }
    }

    #[test]
    fn the_rat_never_hops_or_nibbles_in_lamplight() {
        // Two lamps light a swath of the hold; cargo on the dark side
        // keeps the nibbles coming. Over a long deterministic run the rat
        // must never occupy a lit cell nor gnaw from one.
        let mut pieces = vec![
            hold_piece(0, Kind::CeilingLamp, 1, 0),
            hold_piece(1, Kind::FloorLamp, 0, 2),
            hold_piece(2, Kind::RationBricks, 4, 0),
            hold_piece(3, Kind::Seedlings, 3, 3),
        ];
        let mut rats = wound_rat((3, 1));
        let mut cues = Vec::new();
        let mut skitters = 0_u32;
        for tick in 1..300_000_u64 {
            cues.clear();
            rats.on_tick(0xBEEF, tick, &mut pieces, &mut cues);
            let rat = rats.rat.expect("nothing evicts it here");
            assert!(
                !lit_adjacent(&pieces, CABIN, rat.cell.0, rat.cell.1),
                "tick {tick}: the rat sat in lamplight at {:?}",
                rat.cell
            );
            skitters += cues
                .iter()
                .filter(|cue| matches!(cue, Cue::RatSkitter { .. }))
                .count() as u32;
        }
        assert!(skitters > 100, "the rat barely moved: {skitters} skitters");
        assert!(
            pieces.iter().any(|piece| piece.gnawed),
            "dark cargo still gets gnawed"
        );
    }

    #[test]
    fn a_hold_lit_wall_to_wall_pins_the_rat_and_boards_no_new_one() {
        // Lamps down every even column of the net: every odd-column cell
        // reads lit_adjacent from a horizontal neighbour, every lamp from
        // its column-mates.
        let mut pieces: Vec<Piece> = Vec::new();
        for y in 0..GRID_ROWS {
            for x in 0..GRID_COLS {
                if RATS_ROOM.surface_of(x, y).is_some() && x % 2 == 0 {
                    pieces.push(hold_piece(pieces.len() as u32, Kind::CeilingLamp, x, y));
                }
            }
        }
        for y in 0..GRID_ROWS {
            for x in 0..GRID_COLS {
                if RATS_ROOM.surface_of(x, y).is_some() {
                    assert!(lit_adjacent(&pieces, CABIN, x, y), "({x}, {y}) reads dark");
                }
            }
        }
        // A rat already aboard (lamps stowed around it) skips every beat:
        // no hop, no nibble, schedules still re-armed.
        let mut rats = wound_rat((5, 4));
        let mut cues = Vec::new();
        for tick in 1..30_000_u64 {
            rats.on_tick(7, tick, &mut pieces, &mut cues);
        }
        let rat = rats.rat.expect("light deters, it never evicts");
        assert_eq!(rat.cell, (5, 4), "nowhere unlit to go");
        assert!(rat.next_move > 29_999, "skipped beats must re-arm");
        assert!(cues.is_empty(), "a skipped beat makes no sound: {cues:?}");
        assert!(pieces.iter().all(|piece| !piece.gnawed));
        // And no stowaway boards a lit hold, even on a winning roll.
        let mut fresh = Rats::new();
        let legs = (0..500_u64)
            .find(|&legs| splitmix(9 ^ SALT_BOARD, legs) % BOARD_CHANCE == 0)
            .expect("a boarding roll within 500 legs");
        fresh.on_depart(9, legs, 100, &pieces, false, &mut cues);
        assert!(fresh.rat.is_none(), "it boarded a lit hold");
        assert!(cues.is_empty());
    }

    #[test]
    fn the_couch_tempts_the_rat_into_a_nap() {
        let mut pieces = vec![
            hold_piece(0, Kind::Couch, 4, 3),
            hold_piece(1, Kind::Seedlings, 3, 7),
            hold_piece(2, Kind::RationBricks, 6, 6),
        ];
        let couch = couch_cells(&pieces);
        assert_eq!(couch, [(4, 3), (5, 3)]);
        let mut rats = wound_rat((0, 3));
        let mut cues = Vec::new();
        let mut napped_beats = 0_u32;
        let mut woke = false;
        for tick in 1..600_000_u64 {
            cues.clear();
            let before = rats.rat.expect("aboard").cell;
            let was_napping = couch.contains(&before);
            rats.on_tick(0xC0C4, tick, &mut pieces, &mut cues);
            let rat = rats.rat.expect("aboard");
            let hopped = cues.iter().any(|cue| matches!(cue, Cue::RatSkitter { .. }));
            if hopped && !was_napping {
                // Off the couch every hop is one step straight toward it.
                assert_eq!(
                    couch_gap(&couch, rat.cell),
                    couch_gap(&couch, before) - 1,
                    "tick {tick}: a drift hop failed to close on the couch"
                );
            }
            if hopped && was_napping {
                woke = true;
            }
            if couch.contains(&rat.cell) {
                napped_beats += 1;
                assert!(
                    !cues.contains(&Cue::RatNibble),
                    "tick {tick}: it nibbled mid-nap"
                );
                // The nap cadence: re-armed NAP_LAZE times slower.
                if hopped || rat.next_move > tick {
                    assert!(
                        rat.next_move - tick.min(rat.next_move)
                            <= NAP_LAZE * (MOVE_BASE + MOVE_JITTER),
                        "nap re-arm overshot"
                    );
                }
            }
        }
        assert!(napped_beats > 10_000, "it never settled in: {napped_beats}");
        assert!(woke, "a nap still ends: the lazy hop must eventually fire");
        assert!(
            pieces.iter().any(|piece| piece.gnawed),
            "en route it still bites"
        );
        // The nap cadence itself: a rat on the couch re-arms its hop at
        // least NAP_LAZE * MOVE_BASE out.
        let mut napping = wound_rat((4, 3));
        cues.clear();
        napping.on_tick(0xC0C4, 1, &mut pieces, &mut cues);
        let rat = napping.rat.expect("aboard");
        if couch.contains(&rat.cell) {
            assert!(rat.next_move > NAP_LAZE * MOVE_BASE, "no laze applied");
        }
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
