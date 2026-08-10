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
//! one of them marked) and a proposal standing on its offer band. A rat
//! rides at (4, 4). Two rules shape the roster: `VeryMysteriousCrate`
//! stays ashore (at most one suspicious piece aboard), and the fuel
//! hopper arrives EMPTY, so staging is tested by casting off and
//! staging it yourself.
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

/// The fixture save, STV11, hand-authored — the timestamp line is added
/// at boot so no catch-up elapses.
pub const SAVE: &str = "\
STV11
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
piece 16 4 3 0 hold 2 3 3
piece 17 7 1 0 hold 2 3 6
piece 18 1 2 0 hold 2 4 6
piece 19 11 0 0 hold 0 8 3
piece 20 2 1 0 hold 0 7 7
piece 21 5 0 0 hold 2 5 3
piece 22 3 3 1 hold 2 7 3
piece 23 15 2 0 hold 2 5 6
piece 24 26 0 0 hold 0 4 10
piece 25 25 1 0 hold 0 4 12
piece 26 27 2 0 hold 0 9 11
piece 27 28 3 0 hold 0 9 12
piece 28 29 0 0 hold 0 9 10
next_piece 29
";

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
}
