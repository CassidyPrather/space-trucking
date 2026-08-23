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
//! window keeps its traditional front-wall punch-out; Saturn's bay
//! window stands square on the cabin's own port flank — the one wall
//! of the four with neither a doorway nor an instrument on it, the
//! furnace being moored to starboard and the market aft; and the
//! porthole rides the trade room's port flank — the one wall of a
//! calling room that is neither its goods nor its counter, and a
//! reminder that a window in a room that is only ALONGSIDE is a window
//! the gangway law will not let you leave with. Every screenshot run
//! therefore sweeps a multi-window frame, a second sky on a second
//! wall, and a pane in another room, without anybody remembering to.
//!
//! **Which of the two rides ashore is not a coin toss.** The bay window
//! is 2×2 and the porthole is one cell, and a pane left in the market
//! has to come home before anything launches. The first wall berth the
//! arbiter will take for a 2×2 is the cabin's aft wall beside its own
//! doorway (`cargo::first_fit`, row-major), and that is the wall the
//! seam's amber latch is bolted to: the bay window walked home stood
//! across 78% of it, and one berth further on, 100%. Nothing was wrong
//! with the latch and nothing was wrong with the berth — a player may
//! hide their own latch with their own cargo and that stays legal
//! (docs/GAUNTLET.md, "A latch and a berth want the same piece of
//! wall") — but a debug board that boots you into a cabin you cannot
//! part a room from is a bad debug board. So the small pane is the one
//! that goes ashore, and the big one is already home.
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
//! rule keeps every aperture clear. And nothing the board stands, here
//! or once it has been carried home, may stand across a seam's amber
//! latch. The tests below re-check all of it.
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
piece 29 30 0 0 hold 2 0 4
piece 30 31 1 0 hold 0 1 7
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

    /// **Nothing the showcase stands hides a seam's amber latch.**
    ///
    /// The latch is the one control that sends a room away, and
    /// `--fixture` is booted to poke at exactly that sort of thing. A
    /// board that hands you a cabin whose seam control is behind glass
    /// is not a board with a defect in the rules; it is a board that
    /// chose a bad berth, which is this file's business to fix.
    ///
    /// **It is asked twice, and the second reading is the one that
    /// caught it.** The board is mid-trade on purpose, so the gangway
    /// law will not let anything launch until the goods standing in the
    /// room that is only alongside have been carried aboard — and the
    /// berth they land in is the sim's own, not this file's
    /// ([`first_fit`], the very berth a shift-press picks). A 2×2 pane
    /// left ashore comes home onto the cabin's aft wall beside its own
    /// doorway, which is where the latch is bolted. Asked only of the
    /// board as it boots, this would have been a green tick over a
    /// cabin nobody could part a room from.
    ///
    /// The reading is the gauntlet's own — [`across`] over
    /// [`worked_faces`], at [`OCCLUDE_BITE`] — so what counts as hiding
    /// a worked face is tuned in one place and this moves with it,
    /// rather than a second number growing up beside the first.
    ///
    /// **This is not a law about berths, and must not become one.**
    /// Every wall cell beside an aperture is a berth and every deck cell
    /// in front of one is a berth, so a player standing their own crate
    /// in front of their own latch is legal by construction and stays
    /// legal (docs/GAUNTLET.md, "A latch and a berth want the same piece
    /// of wall"). What is asserted here is about the board this file
    /// ships and nothing else.
    #[test]
    fn the_showcase_leaves_every_seam_latch_workable() {
        use bevy::prelude::Vec3;
        use space_trucking::sim::cargo::{Piece, first_fit, plan};
        use space_trucking::sim::layout;

        use crate::gauntlet::{Box3, OCCLUDE_BITE, across, worked_faces};
        use crate::room::{self, Placed};

        let sim = Sim::from_save(super::SAVE).expect("the fixture parses");
        let rooms = sim.rooms();
        let placed: Vec<Placed> = rooms
            .iter()
            .map(|(id, room)| room::placed(rooms, id, room))
            .collect();

        // Every amber latch the graph draws. `worked_faces` carries a
        // calling room's handshake too; a latch is the half this is
        // about, and it is named off the part rather than counted off
        // the graph so a second seam arrives on the list by itself.
        let latches: Vec<(String, Box3, Vec3)> = placed
            .iter()
            .flat_map(worked_faces)
            .filter(|(what, _, _)| what.contains("latch"))
            .collect();
        assert!(
            !latches.is_empty(),
            "the showcase grew no seam latch, so this measured nothing at all"
        );

        // The world box one berthed piece fills, posed through the very
        // function the runtime poses a rig with.
        let filled = |piece: &Piece| {
            let Loc::Hold { room, x, y } = piece.loc else {
                return None;
            };
            let host = placed.iter().find(|host| host.id == room)?;
            let (w, h) = plan(host.kind, piece.kind, x, y)?;
            let anchor = layout::cell_rect(room, x, y);
            let rect = layout::Rect::new(
                anchor.x,
                anchor.y,
                f32::from(w) * layout::CELL,
                f32::from(h) * layout::CELL,
            );
            let (lo, hi) = crate::pieces::berth_box(&host.charts, piece.kind, rect)?;
            Some(Box3::spanning(lo, hi))
        };

        let judge = |board: &[Piece], when: &str| {
            for (what, face, inward) in &latches {
                for piece in board {
                    let Some(body) = filled(piece) else { continue };
                    let cover = across(*face, *inward, body);
                    assert!(
                        cover <= OCCLUDE_BITE,
                        "{when}: {:?} #{} stands across {:.0}% of {what}, and a board \
                         whose seam control cannot be worked is a board that cannot \
                         part a room",
                        piece.kind,
                        piece.id,
                        cover * 100.0
                    );
                }
            }
        };

        let aboard: Vec<Piece> = sim.pieces().to_vec();
        judge(&aboard, "as it boots");

        // And once the gangway law has been obeyed, which on this board
        // is not a hypothesis: nothing launches until every piece of the
        // player's standing in the room alongside is carried aboard.
        let mut carried = aboard;
        let mut walked = 0_u32;
        while let Some(nth) = carried.iter().position(|piece| match piece.loc {
            Loc::Hold { room, x, y } => {
                rooms.kind(room).is_some_and(|kind| !kind.riding())
                    && rooms.tile(room, x, y) != Some(Tile::Stock)
            }
            _ => false,
        }) {
            let piece = carried[nth];
            let (room, x, y) =
                first_fit(rooms, &carried, piece.id, piece.kind).unwrap_or_else(|| {
                    panic!(
                        "{:?} #{} has no berth to come home to",
                        piece.kind, piece.id
                    )
                });
            carried[nth].loc = Loc::Hold { room, x, y };
            walked += 1;
        }
        assert!(
            walked > 0,
            "the showcase left nothing ashore, so the carry home tested nothing"
        );
        judge(&carried, "carried home");
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
