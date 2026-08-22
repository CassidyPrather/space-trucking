//! The developer fixture: `--fixture` boots this save instead of
//! `cabin.data`, for sweeping the whole attachment surface in one run
//! (owner's ask: "one of everything, things generally actuated away
//! from their defaults"). Boots sandboxed — it NEVER writes over the
//! real save.
//!
//! What it stages: docked at the Guild mid-run (deliveries, karma,
//! legs, partial familiarity, a destination selected, banked burner
//! stoke so the firebox glows), the ship as its graph of rooms — cabin,
//! burner, and the Guild's own trade room alongside — and every cargo
//! kind aboard or alongside through every berth class: floor cargo,
//! wall painting and sconce, ceiling lamp, an occupied cabinet (vial,
//! fluff, chit, bottled midnight in the cubbies), a gnawed rug pinned
//! under the couch, enamel and luminous coats on the walls, the trade
//! room's own goods on its stock band (seedlings, gas, gnawed scrap,
//! one of them marked) and a three-piece proposal standing on its offer
//! band — which sits clear of the door's own lane now, so the showcase
//! also shows the entry-path law holding (docs/ROOMS.md). A rat
//! rides at (4, 4). Two rules shape the roster: `VeryMysteriousCrate`
//! stays ashore (at most one suspicious piece aboard), and the fuel
//! hopper arrives EMPTY, so staging is tested by casting off and
//! staging it yourself.
//!
//! **Three windows, three sizes, three walls, two rooms**, because a
//! crew that owns several is the case the exterior was rebuilt for
//! (`docs/ART_DIRECTION_3D.md`, "One wall, one sky") and a showcase that
//! only ever hangs one would sweep the easy half of it. The transit
//! window keeps its traditional front-wall punch-out; the porthole
//! hangs mid-course on the port flank; and Saturn's bay window stands
//! square on the trade room's port flank — the one wall of a calling
//! room that is neither its goods nor its counter, and a reminder that
//! a window in a room that is only ALONGSIDE is a window the gangway
//! law will not let you leave with. Every screenshot run therefore sweeps a
//! multi-window frame, a second sky on a second wall, and a pane in
//! another room, without anybody remembering to.
//!
//! The board is deliberately mid-trade, which means the launch gate
//! refuses: goods of the player's stand in a room that is only
//! alongside, and the gangway law will not strand them. Carry them home
//! and the lever lights.
//!
//! Keep the board legal when editing: standing cargo shadows the wall
//! cells behind it (the cabinet and floor lamp against the port seam
//! own its baseboard rows, which is why the wall sconce hangs at the
//! cornice AND why the chart tank hangs on the front wall here rather
//! than at its traditional port berth), the gnawed rug lies PINNED
//! under the couch, and no berth may sit on a doorway — the threshold
//! rule keeps every aperture clear. The test below re-checks all of it.
//!
//! One more courtesy, which is not a rule: the deck cells a doorway
//! stands on are kept clear. Nothing forbids berthing there — the aisle
//! rule died with the collision it guarded — but the doorways are drawn
//! now, and a showcase that parks a cabinet in the one place the trade
//! room can be seen through is a showcase of a cabinet.
//!
//! It was a courtesy the board did not keep, and the doorstep law is
//! what found it out. Two of the trade room's three goods stood on the
//! two cells its own door lands in, because the stock band ran under the
//! doorway and nothing said it should not. They stand along the band
//! now, and the way in is deck a body may walk onto and set a crate
//! down on.

use space_trucking::sim::room::RoomKind;

/// The fixture save, hand-authored at the version this build writes —
/// the timestamp line is added at boot so no catch-up elapses.
///
/// It used to be authored at STV11 and walk the migration chain on every
/// boot, which read as free coverage and was not: the chain's job is to
/// carry a board from a net that no longer exists onto one that does,
/// and a showcase whose proposal ends up wherever the arithmetic leaves
/// it is a showcase of the arithmetic. The market grew to 8×7 at STV16
/// and the front rows it gained are rows no older document can name, so
/// the board is written in the net it is meant to show.
/// Every migration keeps its own test in `sim::save`.
pub const SAVE: &str = "\
STV16
seed 7
tick 12000
rng 3c76e098a8f74c8a
warp 0
paused 0
deliveries 3
karma 2
familiar 0000 0000 0000 0000 0000 0000 00ff 000f 0000 0000 0000 0000
visits 0 0 0 0 0 0 2 1 0 0 0 0
ship docked 6 7 5400
legs 4
omen - idle 0 3f800000 00000000
enc -
drone -
parade - -
rat 4 4 4 4 11800 12300 12600 1
rooms 3
room 0 0 - - -
room 1 1 0 1 3
room 2 2 0 0 0
marks 1 16
piece 0 21 0 0 hold 0 6 4
piece 1 9 0 0 hold 0 7 5
piece 2 8 0 0 hold 0 4 7
piece 3 19 1 0 hold 0 4 6
piece 4 18 2 0 hold 0 3 6
piece 5 12 0 0 stow 0 3
piece 6 6 1 0 hold 0 3 9
piece 7 20 3 0 hold 0 7 1
piece 8 17 1 0 hold 0 0 5
piece 9 16 2 0 hold 0 15 4
piece 10 22 0 1 laid 0 4 6
piece 11 23 1 0 laid 0 6 1
piece 12 24 0 0 laid 0 2 6
piece 13 0 1 0 stow 0 0
piece 14 13 2 0 stow 0 1
piece 15 14 0 0 stow 0 2
piece 16 4 3 0 hold 2 5 3
piece 17 7 1 0 hold 2 6 8
piece 18 1 2 0 hold 2 7 8
piece 19 11 0 0 hold 0 8 3
piece 20 2 1 0 hold 0 7 7
piece 21 5 0 0 hold 2 9 3
piece 22 3 3 1 hold 2 7 3
piece 23 15 2 0 hold 2 5 8
piece 24 26 0 0 hold 0 4 10
piece 25 25 1 0 hold 0 4 12
piece 26 27 2 0 hold 0 9 11
piece 27 28 3 0 hold 0 9 12
piece 28 29 0 0 hold 0 9 10
piece 29 30 0 0 hold 0 1 8
piece 30 31 1 0 hold 2 0 4
next_piece 31
";

/// The same board, re-berthed at another station (`--docked n`).
///
/// **The tool the per-station design agents work with.** Every run boots
/// docked at the Guild, so before this there was no way to *look* at
/// eleven of the twelve rooms a character can be written for — which
/// would have made "judge your screenshots" advice nobody could take.
///
/// It cheats at nothing it does not have to: the graph, the cargo, and
/// every berth are the fixture's own, and only the mooring moves. The
/// destination is cleared (a station cannot be charted from itself, and
/// the inner ring wants a transit chit aboard before it will let you
/// plot a neighbour), and the visit counter is nudged to at least one,
/// because a station whose room is alongside has been called on.
#[must_use]
pub fn docked_at(poi: space_trucking::sim::map::PoiId) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    for line in SAVE.lines() {
        if line.starts_with("ship docked ") {
            let stoke = line.split_whitespace().nth(4).unwrap_or("0");
            let _ = writeln!(out, "ship docked {poi} - {stoke}");
            continue;
        }
        if let Some(counts) = line.strip_prefix("visits ") {
            let mut visits: Vec<u32> = counts
                .split_whitespace()
                .map(|token| token.parse().unwrap_or(0))
                .collect();
            if let Some(count) = visits.get_mut(usize::from(poi)) {
                *count = (*count).max(1);
            }
            let _ = write!(out, "visits");
            for count in visits {
                let _ = write!(out, " {count}");
            }
            let _ = writeln!(out);
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A world with an event room already alongside (`--alongside
/// wreck|parlor|pump`), as a save string.
///
/// **The tool the three event rooms need, for the reason `--docked n`
/// was not enough.** A station's room is alongside whenever you are
/// moored at it, so re-berthing the board shows any of the twelve. The
/// event rooms are not anybody's: a derelict, a parlor, and a pump bay
/// attach *mid-leg*, when an encounter's window opens, and they are gone
/// by the next dock. Without this there is no way to stand in one, and
/// "judge your own screenshots" would be advice nobody could take.
///
/// It cheats at nothing, exactly as `cast_off` does not: it searches
/// seeds for a first leg whose encounter is the one asked for, charts
/// and launches through the sim's own `InputFrame` interface the way a
/// player would, and runs the leg on until the window opens and the sim
/// itself attaches the room. What comes back is a board the sim built.
/// `None` means no seed in range met that thing, which is a fact about
/// the encounter roll and not a fallback worth inventing a board over.
#[must_use]
pub fn alongside(kind: RoomKind) -> Option<String> {
    use space_trucking::sim::{EncounterKind, InputFrame, ShipState, Sim, TICK_DT, layout};

    let want = match kind {
        RoomKind::Wreck => EncounterKind::Derelict,
        RoomKind::Parlor => EncounterKind::Casino,
        RoomKind::Pump => EncounterKind::GasStation,
        // The ship's own rooms and the trade room are not met; they are
        // owned or docked at, and `--docked n` already berths those.
        _ => return None,
    };
    let press = |at| InputFrame {
        pointer: at,
        press: true,
        held: true,
        ..InputFrame::default()
    };
    for seed in 0..SEED_SWEEP {
        for there in 0..12_u8 {
            let mut sim = Sim::new(seed);
            let ShipState::Docked(here) = sim.ship().state else {
                continue;
            };
            if there == here || !sim.poi_chartable(there) {
                continue;
            }
            sim.advance(0.0, &press(sim.poi_pos(there)));
            sim.advance(
                0.0,
                &press(crate::canvas::rect_center(layout::LAUNCH_LEVER)),
            );
            sim.advance(TICK_DT, &InputFrame::default());
            let Some(window) = sim
                .encounter()
                .filter(|enc| enc.kind == want)
                .map(|enc| enc.start)
            else {
                continue;
            };
            // Straight to the window's own edge, then one ordinary tick
            // over it: the room is attached by the tick that opens the
            // encounter, and no tick here is different from any other.
            let ShipState::Traveling { progress, .. } = sim.ship().state else {
                continue;
            };
            sim.fast_forward(window.saturating_sub(progress));
            while sim.rooms().find(kind).is_none()
                && matches!(sim.ship().state, ShipState::Traveling { .. })
            {
                sim.advance(TICK_DT, &InputFrame::default());
            }
            if sim.rooms().find(kind).is_some() {
                return Some(sim.save_string());
            }
        }
    }
    None
}

/// How many seeds [`alongside`] will try before it admits defeat. One
/// leg in three carries an encounter and one encounter in five is any
/// given kind, so a handful of seeds is plenty and this is only a bound
/// on a dev tool's patience.
const SEED_SWEEP: u64 = 2000;

/// A dev board carrying exactly `n` windows and nothing else unusual:
/// the starter ship with every pane stripped off it, then `n` hung on
/// the cabin's aft wall at the first berths the sim's own arbiter will
/// take (`--panes n`).
///
/// This exists to be MEASURED. The exterior's whole claim is that a
/// wall of glass costs about what one window costs
/// (`docs/ART_DIRECTION_3D.md`, "One wall, one sky"), and a claim about
/// scaling is worth exactly the curve behind it — so the curve is
/// something anyone can re-run: `--panes 1|2|4|8 --gauge 240`, with
/// `--grouping pane` for the control arm. `--panes 0` is the other end
/// of the same tool: a ship that sold its window, whose hull had better
/// be solid.
///
/// It cheats at nothing. Every berth goes through `placement_check`, so
/// a board this returns is a board a player could have built, and a
/// refit that made these cells illegal would hand back a shorter board
/// rather than a lie.
#[must_use]
pub fn panes_board(seed: u64, n: usize) -> String {
    use std::fmt::Write as _;

    use space_trucking::sim::cargo::{Loc, Piece, placement_check};
    use space_trucking::sim::room::{CABIN, RoomKind, Surf};
    use space_trucking::sim::{Kind, Sim};

    let sim = Sim::new(seed);
    let rooms = sim.rooms();
    let mut aboard: Vec<Piece> = sim
        .pieces()
        .iter()
        .copied()
        .filter(|piece| !piece.kind.window())
        .collect();
    let mut next = aboard.iter().map(|piece| piece.id + 1).max().unwrap_or(0);

    // Row-major over the cabin's aft chart: one wall, so one sky, which
    // is the arrangement the measurement is actually about.
    let (cols, rows) = RoomKind::Cabin.grid();
    let mut hung: Vec<Piece> = Vec::new();
    'search: for y in 0..rows {
        for x in 0..cols {
            if hung.len() >= n {
                break 'search;
            }
            if RoomKind::Cabin.surface_of(x, y) != Some(Surf::Aft) {
                continue;
            }
            let piece = Piece {
                id: next,
                kind: Kind::Window,
                variant: 0,
                gnawed: false,
                loc: Loc::Hold { room: CABIN, x, y },
            };
            if placement_check(rooms, &aboard, piece.id, piece.kind, CABIN, x, y).is_ok() {
                aboard.push(piece);
                hung.push(piece);
                next += 1;
            }
        }
    }

    let mut out = String::new();
    for line in sim.save_string().lines() {
        if line.starts_with("next_piece") {
            for piece in &hung {
                let Loc::Hold { room, x, y } = piece.loc else {
                    unreachable!("the search only berths in holds");
                };
                // Writing into a String cannot fail, so the fmt
                // plumbing is dropped — `save.rs`'s own convention.
                let _ = writeln!(
                    out,
                    "piece {} {} 0 0 hold {room} {x} {y}",
                    piece.id,
                    piece.kind.index()
                );
            }
            let _ = writeln!(out, "next_piece {next}");
            continue;
        }
        // The stripped panes take their own lines with them, at every
        // size the family has.
        if line.starts_with("piece ")
            && line.split_whitespace().nth(2).is_some_and(|token| {
                Kind::ALL
                    .iter()
                    .filter(|kind| kind.window())
                    .any(|kind| token == kind.index().to_string())
            })
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use space_trucking::sim::Sim;
    use space_trucking::sim::cargo::{Kind, Loc, dressing_check, placement_check};
    use space_trucking::sim::room::{RoomKind, Tile};

    /// The fixture is a real save and an honest board: it parses, its
    /// graph is the ship it claims, every berth passes the sim's own
    /// arbiter, and every kind is aboard except the one the rules forbid.
    #[test]
    fn the_fixture_is_legal_and_nearly_complete() {
        let sim = Sim::from_save(super::SAVE).expect("fixture parses");
        let rooms = sim.rooms();
        assert_eq!(rooms.kind(0), Some(RoomKind::Cabin));
        assert_eq!(rooms.kind(1), Some(RoomKind::Burner));
        assert_eq!(rooms.kind(2), Some(RoomKind::Trade));
        let pieces = sim.pieces();
        for piece in pieces {
            match piece.loc {
                Loc::Hold { room, x, y } => assert_eq!(
                    placement_check(rooms, pieces, piece.id, piece.kind, room, x, y),
                    Ok(()),
                    "{:?} berthed illegally at room {room} ({x}, {y})",
                    piece.kind
                ),
                Loc::Laid { room, x, y } => {
                    let laid: Vec<_> = pieces
                        .iter()
                        .filter(|other| matches!(other.loc, Loc::Laid { .. }))
                        .copied()
                        .collect();
                    assert_eq!(
                        dressing_check(rooms, &laid, piece.id, piece.kind, room, x, y),
                        Ok(()),
                        "{:?} laid illegally at room {room} ({x}, {y})",
                        piece.kind
                    );
                }
                Loc::Stow { .. } => {}
            }
        }
        let mut kinds: Vec<Kind> = pieces.iter().map(|piece| piece.kind).collect();
        kinds.sort_by_key(|kind| kind.index());
        kinds.dedup();
        assert_eq!(
            kinds.len(),
            Kind::ALL.len() - 1,
            "every kind but the rule-forbidden VeryMysteriousCrate"
        );
        assert!(
            !kinds.contains(&Kind::VeryMysteriousCrate),
            "the fixture must not cheat the one-suspicious rule"
        );
        // Three windows, three sizes, and no two of them on one wall:
        // the multi-sky path is swept by every screenshot run.
        let glass: Vec<_> = pieces.iter().filter(|p| p.kind.window()).collect();
        assert_eq!(glass.len(), 3, "the fixture flies three windows");
        let mut sizes: Vec<(u8, u8, u8)> = glass.iter().map(|p| p.kind.extent()).collect();
        sizes.sort_unstable();
        sizes.dedup();
        assert_eq!(sizes.len(), 3, "three sizes, not three of one");
        let mut walls: Vec<(Option<u8>, Option<space_trucking::sim::room::Surf>)> = glass
            .iter()
            .map(|p| match p.loc {
                Loc::Hold { room, x, y } => (
                    Some(room),
                    rooms.kind(room).and_then(|kind| kind.surface_of(x, y)),
                ),
                _ => (None, None),
            })
            .collect();
        walls.sort_by_key(|wall| format!("{wall:?}"));
        walls.dedup();
        assert_eq!(walls.len(), 3, "three walls, so three skies: {walls:?}");
        assert!(
            glass
                .iter()
                .any(|p| !matches!(p.loc, Loc::Hold { room: 0, .. })),
            "one pane rides a room that is only alongside"
        );
        assert!(sim.rat().is_some(), "the stowaway rides the fixture");
        assert!(sim.stoked(), "the firebox arrives banked");
        // Mid-trade, on purpose: the room's own goods on its stock band,
        // a proposal standing on its offer band, and one good marked.
        let tile = |piece: &space_trucking::sim::Piece| match piece.loc {
            Loc::Hold { room, x, y } => rooms.tile(room, x, y),
            _ => None,
        };
        assert!(pieces.iter().any(|p| tile(p) == Some(Tile::Stock)));
        assert!(pieces.iter().any(|p| tile(p) == Some(Tile::Offer)));
        assert_eq!(sim.marks().len(), 1, "one good is spoken for");
        assert!(!sim.composed().is_empty(), "the room answers the proposal");
    }

    /// **Every station's room can be looked at.** `--docked n` re-berths
    /// the same legal board at any place on the chart, so a per-station
    /// design agent can screenshot the room it is writing a character
    /// for. A board that stopped parsing at station seven would be a
    /// fleet that could not see its own work.
    #[test]
    fn the_board_moors_at_every_station() {
        use space_trucking::sim::ShipState;
        use space_trucking::sim::map::{POI_COUNT, PoiId};

        for poi in 0..POI_COUNT as PoiId {
            let sim = Sim::from_save(&super::docked_at(poi))
                .unwrap_or_else(|_| panic!("the board must moor at POI {poi}"));
            assert_eq!(sim.ship().state, ShipState::Docked(poi));
            assert_eq!(sim.ship().selected, None, "a mooring charts nothing");
            assert_eq!(
                sim.rooms().find(RoomKind::Trade),
                Some(2),
                "POI {poi} lost the room the board came with"
            );
        }
    }

    /// **Every event room can be stood in.** The three rooms nobody
    /// keeps only exist mid-leg, so `--alongside` is the only way to
    /// look at one — and a design agent who cannot look at the room it
    /// is dressing is writing numbers into the dark.
    #[test]
    fn every_event_room_can_be_berthed_for_a_look() {
        use space_trucking::sim::ShipState;

        for kind in [RoomKind::Wreck, RoomKind::Parlor, RoomKind::Pump] {
            let save =
                super::alongside(kind).unwrap_or_else(|| panic!("no seed in range met a {kind:?}"));
            let sim = Sim::from_save(&save).unwrap_or_else(|_| panic!("{kind:?} board parses"));
            assert!(
                matches!(sim.ship().state, ShipState::Traveling { .. }),
                "an event room is met underway or not at all"
            );
            assert!(
                sim.rooms().find(kind).is_some(),
                "{kind:?} did not come alongside"
            );
        }
        // A room that is never *met* has no board here, which is how the
        // tool says "use `--docked n` for that one" without guessing.
        assert!(super::alongside(RoomKind::Trade).is_none());
    }
}
