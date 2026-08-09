//! The developer fixture: `--fixture` boots this save instead of
//! `cabin.data`, for sweeping the whole attachment surface in one run
//! (owner's ask: "one of everything, things generally actuated away
//! from their defaults"). Boots sandboxed — it NEVER writes over the
//! real save.
//!
//! What it stages: docked at the Guild mid-run (deliveries, karma,
//! legs, partial familiarity, a destination selected, patience one
//! pull down, banked burner stoke so the firebox glows), and every
//! cargo kind aboard through every berth class — floor cargo, wall
//! painting and sconce, ceiling lamp, an occupied cabinet (vial,
//! fluff, chit, bottled midnight in the cubbies), a gnawed rug pinned
//! under the couch, enamel and luminous coats on the walls, give pads
//! and received shelf dressed mid-trade, station stock on the shelf
//! (seedlings, gas, gnawed scrap). A rat rides at (4, 4). Two rules
//! shape the roster: `VeryMysteriousCrate` stays ashore (at most one
//! suspicious piece aboard), and the burner hopper arrives EMPTY —
//! docking banks the hopper before any barter opens (the rail IS the
//! shelf row; the save parser now refuses the combination outright),
//! so fuel staging is tested by casting off and staging it yourself.
//!
//! Keep the board legal when editing: standing cargo shadows the wall
//! cells behind it (the cabinet and floor lamp against the port seam
//! own its baseboard rows, which is why the wall sconce hangs at the
//! cornice AND why the chart tank hangs on the front wall here rather
//! than at its traditional port berth), the gnawed rug lies PINNED
//! under the couch, and the free floor must stay one connected region,
//! aisle included (`Violation::Sealed`) — the test below re-checks the
//! legality half of all of it. The five instruments spread across the
//! front wall: the tank and the window (at its old punch-out cornice)
//! left of the console face, the gauges and launch handle stacked
//! along the starboard edge, clear of the console's plate.

/// The fixture save, STV9, hand-authored — the timestamp line is added
/// at boot so no catch-up elapses.
pub const SAVE: &str = "\
STV9
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
eager 3f800000 2
piece 0 21 0 0 hold 3 3
piece 1 9 0 0 hold 7 5
piece 2 8 0 0 hold 4 3
piece 3 19 1 0 hold 4 6
piece 4 18 2 0 hold 3 6
piece 5 12 0 0 stow 0 3
piece 6 6 1 0 recv 2
piece 7 20 3 0 hold 7 1
piece 8 17 1 0 hold 0 5
piece 9 16 2 0 hold 13 4
piece 10 22 0 1 laid 4 6
piece 11 23 1 0 laid 6 1
piece 12 24 0 0 laid 2 6
piece 13 0 1 0 stow 0 0
piece 14 13 2 0 stow 0 1
piece 15 14 0 0 stow 0 2
piece 16 4 3 0 shelf 1
piece 17 7 1 0 give 0
piece 18 1 2 0 give 1
piece 19 11 0 0 recv 0
piece 20 2 1 0 recv 1
piece 21 5 0 0 shelf 2
piece 22 3 3 1 shelf 3
piece 23 15 2 0 give 2
piece 24 26 0 0 hold 4 8
piece 25 25 1 0 hold 4 10
piece 26 27 2 0 hold 8 9
piece 27 28 3 0 hold 8 10
piece 28 29 0 0 hold 8 8
next_piece 29
";

#[cfg(test)]
mod tests {
    use space_trucking::sim::Sim;
    use space_trucking::sim::cargo::{Kind, Loc, dressing_check, placement_check};

    /// The fixture is a real save and an honest board: it parses, every
    /// hold berth passes the sim's own placement arbiter, every laid
    /// coat its dressing arbiter, and every kind is aboard except the
    /// one the rules forbid.
    #[test]
    fn the_fixture_is_legal_and_nearly_complete() {
        let sim = Sim::from_save(super::SAVE).expect("fixture parses");
        let pieces = sim.pieces();
        for piece in pieces {
            match piece.loc {
                Loc::Hold { x, y } => assert_eq!(
                    placement_check(pieces, piece.id, piece.kind, x, y),
                    Ok(()),
                    "{:?} berthed illegally at ({x}, {y})",
                    piece.kind
                ),
                Loc::Laid { x, y } => {
                    let laid: Vec<_> = pieces
                        .iter()
                        .filter(|other| matches!(other.loc, Loc::Laid { .. }))
                        .copied()
                        .collect();
                    assert_eq!(
                        dressing_check(&laid, piece.id, piece.kind, x, y),
                        Ok(()),
                        "{:?} laid illegally at ({x}, {y})",
                        piece.kind
                    );
                }
                _ => {}
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
    }
}
