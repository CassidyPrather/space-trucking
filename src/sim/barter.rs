//! The economy that survived its interface.
//!
//! The pads, the eagerness dial, patience, the fog, the accept lever and
//! the whole desk-scale counter are gone (docs/ROOMS.md, "The decision").
//! What is here is *economy*, not interface, and it keeps working
//! unchanged behind the new flow: [`VALUE`], the per-visit ±1 jitter, the
//! wants row, [`GNAW_MALUS`], the Umbra Market's gnaw premium, the
//! well-lit-art bonus, and the arithmetic a room does when it answers a
//! proposal with goods.
//!
//! Each dock generates a fresh visit — stock leaning toward what the
//! station produces, a jittered copy of its value table, and a top-three
//! wants list — all derived from `splitmix(seed, station, visit)`, so a
//! save can rebuild the whole thing without storing it. The persistent
//! run RNG is spent only on cosmetic variant rolls.

use super::cargo::{KIND_COUNT, Kind, Loc, Piece, Tag, lamp_lit};
use super::map::{GUILD, HERMITAGE, POI_COUNT, PoiId, UMBRA};
use super::room::{RoomId, Rooms, Tile};
use super::splitmix;

/// One station visit's trading state.
///
/// Regenerated per dock, rebuilt on load — and now nothing but the
/// derived facts, because the offer is goods on tiles rather than a
/// needle with a state machine behind it.
///
/// Deliberately currency-free: nothing here (or anywhere in the public
/// API) is a player-visible balance. Values stay internal and trades
/// settle piece-for-piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Barter {
    pub station: PoiId,
    /// How many times this station has been visited, 1-based.
    pub visit: u32,
    /// Top-three kinds the station values this visit, with 1–3 want pips.
    pub wants: [(Kind, u8); 3],
}

/// How much each station values each kind, `0..=6`.
///
/// Rows follow map order (Venus, Earth, Mars, Jupiter, Uranus, Neptune,
/// Guild, Saturn, Umbra Market, Hermitage, comet, `???`), columns follow
/// [`Kind::index`] order. A zero doubles as "local produce": stations
/// stock the kind they do not value (which is how the Guild comes to
/// broker transit chits, the Umbra Market to bottle midnight — and, since
/// its lamp columns are zeros, to fence seized lamps cheap: light is a
/// rival product there, sold only snuffed). Every row must keep at least
/// three kinds at 2 or above so the wants list survives the ±1 jitter,
/// and both suspicious columns are 4 everywhere but the Guild, which
/// seizes rather than pays. The five fixture columns are lore-directed:
/// Venus buys tack, Earth rations light, Saturn treasures working
/// fixtures, the Hermitage pays best for the couch. The cabinet
/// column follows the same lore: Saturn prizes working furniture, Earth
/// and the Hermitage are practical people, and even the Umbra Market
/// pays fair for a box that keeps light in its place. The covering
/// columns (rug, enamel, luminous — last three): Venus buys tack and
/// the Hermitage warmth, Mars is rust country for enamel, and the
/// Umbra Market prices luminous paint at zero — light is a rival
/// product, so it bottles glow as local produce and shelves it snuffed
/// in blackout tins, exactly as it fences lamps. The comet and `???`
/// never open a trade room, so their rows are placeholders kept valid
/// for the invariants above.
// Kept tabular by hand: one row per station is how this table is tuned.
#[rustfmt::skip]
pub const VALUE: [[u8; KIND_COUNT]; POI_COUNT] = [
    [0, 1, 2, 1, 3, 2, 3, 5, 4, 1, 4, 3, 5, 4, 2, 1, 5, 4, 4, 4, 6, 3, 5, 3, 4, 3, 3, 2, 2, 2], // Venus
    [4, 3, 0, 2, 4, 1, 2, 3, 4, 1, 4, 2, 5, 2, 2, 1, 2, 2, 2, 3, 1, 4, 2, 3, 2, 2, 4, 2, 2, 3], // Earth
    [2, 1, 4, 0, 1, 3, 2, 2, 4, 1, 4, 2, 5, 1, 2, 1, 2, 2, 3, 3, 2, 3, 3, 4, 2, 2, 3, 2, 2, 3], // Mars
    [1, 2, 4, 3, 5, 0, 1, 2, 4, 1, 4, 2, 5, 1, 1, 1, 3, 2, 2, 3, 2, 2, 2, 2, 3, 3, 3, 2, 2, 2], // Jupiter
    [2, 3, 3, 2, 4, 4, 0, 1, 4, 1, 4, 2, 5, 1, 1, 1, 2, 3, 2, 2, 3, 2, 2, 2, 3, 3, 3, 2, 2, 2], // Uranus
    [3, 2, 4, 3, 3, 2, 1, 0, 4, 1, 4, 2, 5, 2, 1, 1, 2, 2, 3, 2, 3, 2, 3, 2, 3, 3, 3, 2, 2, 2], // Neptune
    [2, 2, 2, 2, 2, 2, 2, 2, 0, 1, 0, 2, 3, 1, 0, 1, 1, 2, 1, 2, 1, 1, 1, 2, 1, 1, 2, 1, 1, 2], // Guild
    [1, 3, 2, 6, 1, 0, 2, 1, 4, 1, 4, 1, 5, 1, 1, 1, 4, 4, 3, 5, 3, 5, 3, 4, 3, 4, 3, 3, 3, 3], // Saturn
    [3, 2, 1, 1, 2, 0, 5, 4, 4, 3, 4, 3, 0, 2, 2, 1, 0, 0, 0, 3, 4, 3, 3, 2, 0, 5, 2, 1, 1, 2], // Umbra Market
    [1, 1, 3, 1, 4, 1, 1, 2, 4, 2, 4, 2, 3, 3, 1, 1, 2, 3, 2, 6, 3, 4, 5, 3, 4, 5, 2, 2, 2, 1], // Hermitage
    [2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 4, 2, 2, 2, 2, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2], // comet (no trade)
    [2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 4, 2, 2, 2, 2, 1, 1, 1, 1, 1, 3, 2, 2, 2, 3, 2, 2, 2, 2, 2], // ??? (no trade)
];

/// Ceiling for jittered per-visit values.
const VALUE_MAX: u8 = 6;

/// Fewest goods a station puts out.
const STOCK_MIN: usize = 2;

/// Most goods a station puts out in one visit.
const STOCK_MAX: usize = 4;

/// Chance a stock roll picks from the station's produce, in tenths.
const PRODUCE_CHANCE: u64 = 7;

/// Chance denominator for a far station's crate offer: one visit in five.
const CRATE_CHANCE: u64 = 5;

/// Stream salts under the visit hash, so the crate roll, the stock count,
/// and each stocked kind draw independent values.
const SALT_CRATE: u64 = 1;
const SALT_COUNT: u64 = 2;
const SALT_KIND: u64 = 0x100;
const SALT_MYSTERY: u64 = 0x200;

/// This visit's value table: the station row jittered by ±1 per kind,
/// clamped to `0..=6`. Pure, so saves rebuild it instead of storing it.
/// One pin: the Guild's crate valuation stays zero — it seizes crates at
/// the dock, it never pays for them.
#[must_use]
pub(crate) fn visit_values(seed: u64, station: PoiId, visit: u32) -> [u8; KIND_COUNT] {
    let h = visit_hash(seed, station, visit);
    let mut values = [0_u8; KIND_COUNT];
    for (k, value) in values.iter_mut().enumerate() {
        let base = VALUE[usize::from(station)][k];
        *value = if station == GUILD
            && (k == Kind::SuspiciousCrate.index() || k == Kind::VeryMysteriousCrate.index())
        {
            0
        } else {
            match splitmix(h, k as u64) % 3 {
                0 => base.saturating_sub(1),
                1 => base,
                _ => (base + 1).min(VALUE_MAX),
            }
        };
    }
    values
}

/// This visit's derived state: the wants row, and nothing else.
#[must_use]
pub(crate) fn open(seed: u64, station: PoiId, visit: u32) -> Barter {
    Barter {
        station,
        visit,
        wants: wants(&visit_values(seed, station, visit)),
    }
}

/// What a station puts on its `Stock` tiles this visit, in tile order.
///
/// Two to four goods, each 70% from the station's produce and 30%
/// uniform over the rest; crates never come from those rolls. A far
/// station offers a crate one visit in five — never while `aboard`
/// already carries one anywhere — and the Guild never offers one: crates
/// flow outward from the frontier and home to the hangar. `cap` is the
/// room's own tile count, so the slot count is the room's, not a fixed
/// four.
#[must_use]
pub(crate) fn stock_kinds(
    seed: u64,
    station: PoiId,
    visit: u32,
    aboard: &[Piece],
    karma: u32,
    paraded: bool,
    cap: usize,
) -> Vec<Kind> {
    let h = visit_hash(seed, station, visit);
    let room = cap.min(STOCK_MAX);

    let crate_aboard = aboard
        .iter()
        .any(|piece| matches!(piece.kind.tag(), Some(Tag::Suspicious)));
    // The Hermitage never deals in crates; its economy is the karma one.
    let offer_crate = station != GUILD
        && station != HERMITAGE
        && !crate_aboard
        && splitmix(h, SALT_CRATE) % CRATE_CHANCE == 0;

    let mut goods = Vec::new();
    if offer_crate && goods.len() < room {
        goods.push(Kind::SuspiciousCrate);
    }
    // After the Grand Parade, mysterious crates trickle onto ordinary
    // shelves: whatever the hangar was counting, the counting continues.
    if paraded && station != GUILD && splitmix(h, SALT_MYSTERY) % 4 == 0 && goods.len() < room {
        goods.push(Kind::MysteriousCrate);
    }
    let span = (STOCK_MAX - STOCK_MIN) as u64 + 1;
    let count = if station == HERMITAGE {
        // The gift economy: the hermits put nothing out for strangers,
        // and one good per two pieces ever gifted to them — generosity
        // comes back, slowly, and never as a transaction.
        (karma as usize / 2).min(STOCK_MAX)
    } else {
        STOCK_MIN + (splitmix(h, SALT_COUNT) % span) as usize
    };
    // The crate takes a tile, so ordinary goods yield rather than overflow.
    let count = count.min(room.saturating_sub(goods.len()));
    for i in 0..count {
        goods.push(stock_kind(station, splitmix(h, SALT_KIND + i as u64)));
    }
    goods
}

/// Gifted value that saturates the accept cue's celebration. The deal
/// *sound* scales with what was given.
const GIFT_WARMTH: f32 = 8.0;

/// What a rat's bite knocks off a piece's value, floored at zero. The gnaw
/// is permanent, so the discount follows the piece through the economy —
/// on the offer area, on the room's own tiles, and back again.
pub(crate) const GNAW_MALUS: u8 = 2;

/// Whether `station` considers a rat's toothwork artisanal. The Umbra
/// Market does: there, the malus flips into a premium of the same size,
/// and a stowaway becomes a business partner.
#[must_use]
pub(crate) const fn gnaw_loved(station: PoiId) -> bool {
    station == UMBRA
}

/// What lamplight adds to a well-lit painting's price.
pub(crate) const LIT_BONUS: u8 = 1;

/// The markup a room adds to each piece of its own stock, so even
/// worthless goods are never free.
pub(crate) const STOCK_MARKUP: u32 = 1;

/// Whether `piece` is a painting shown in good light. On an ordinary
/// berth that is literal: some cell of its footprint reads
/// [`super::cargo::lit_adjacent`]. On a calling room's offer area — the
/// only place valuation actually prices — the piece is appraised under
/// the ship's own lamplight, so any lamp lit aboard counts.
fn well_lit(rooms: &Rooms, piece: &Piece, pieces: &[Piece]) -> bool {
    if piece.kind != Kind::Painting {
        return false;
    }
    let Loc::Hold { room, x, y } = piece.loc else {
        return false;
    };
    if rooms.tile(room, x, y) == Some(Tile::Offer) {
        return pieces.iter().any(|other| {
            lamp_lit(other) && matches!(other.loc, Loc::Hold { room, .. } if rooms.riding(room))
        });
    }
    let (w, h) = piece.kind.cells();
    (0..w).any(|dx| (0..h).any(|dy| super::cargo::lit_adjacent(pieces, room, x + dx, y + dy)))
}

/// One piece's worth under this visit's table: its kind's jittered value,
/// less [`GNAW_MALUS`] if a rat has been at it — or MORE by the same
/// amount where the bite is loved (see [`gnaw_loved`]) — plus
/// [`LIT_BONUS`] for a painting under lamplight. The single per-piece
/// pricing rule; both sides of a deal read it, so a gnawed piece is
/// cheaper to buy exactly as it is poorer to sell, and well-lit art is
/// dearer to buy exactly as it is richer to sell.
#[must_use]
pub(crate) fn piece_value(
    rooms: &Rooms,
    piece: &Piece,
    pieces: &[Piece],
    values: &[u8; KIND_COUNT],
    gnaw_love: bool,
) -> u32 {
    let value = values[piece.kind.index()];
    let value = if !piece.gnawed {
        u32::from(value)
    } else if gnaw_love {
        u32::from(value) + u32::from(GNAW_MALUS)
    } else {
        u32::from(value.saturating_sub(GNAW_MALUS))
    };
    if well_lit(rooms, piece, pieces) {
        value + u32::from(LIT_BONUS)
    } else {
        value
    }
}

/// The pile a room composes in answer to a proposal.
///
/// Candidates are `(piece id, asking price, marked)`; the room takes the
/// best pile its own stock offers that the proposal's value covers,
/// **preferring marked kinds** and breaking ties deterministically (dearer
/// first, then by piece id). Greedy on a handful of goods is the whole
/// arithmetic; nothing here is a search.
#[must_use]
pub(crate) fn compose(budget: u32, candidates: &[(u32, u32, bool)]) -> Vec<u32> {
    let mut order: Vec<(u32, u32, bool)> = candidates.to_vec();
    order.sort_by_key(|&(id, cost, marked)| (!marked, std::cmp::Reverse(cost), id));
    let mut spent = 0_u32;
    let mut taken = Vec::new();
    for (id, cost, _) in order {
        if spent + cost <= budget {
            spent += cost;
            taken.push(id);
        }
    }
    taken.sort_unstable();
    taken
}

/// Generosity of the concluded deal in `0..=1`, for the accept cue's
/// gain: the overshoot past what was taken, or the gifted value itself
/// for a pure gift. A pegged offer must not make every token gift sound
/// lavish, so the two scales stay separate.
#[must_use]
pub(crate) fn deal_value(given: u32, taken: u32) -> f32 {
    if taken > 0 {
        (given as f32 / taken as f32 - 1.0).clamp(0.0, 1.0)
    } else {
        (given as f32 / GIFT_WARMTH).clamp(0.0, 1.0)
    }
}

/// One hash per (seed, station, visit) triple, the root of everything a
/// visit derives.
const fn visit_hash(seed: u64, station: PoiId, visit: u32) -> u64 {
    splitmix(splitmix(seed, station as u64), visit as u64)
}

/// Top-three valued kinds this visit, zeros excluded, ties broken by kind
/// index; pips `ceil(value / 2)`, so `1..=3`.
fn wants(values: &[u8; KIND_COUNT]) -> [(Kind, u8); 3] {
    let mut order: Vec<usize> = (0..KIND_COUNT).filter(|&k| values[k] > 0).collect();
    order.sort_by_key(|&k| (std::cmp::Reverse(values[k]), k));
    debug_assert!(order.len() >= 3, "VALUE rows must survive jitter, see doc");
    let want = |k: usize| (Kind::ALL[k], values[k].div_ceil(2));
    [want(order[0]), want(order[1]), want(order[2])]
}

/// One stocked kind from one roll: 70% the station's produce (base value
/// zero), 30% uniform over the others. Crates are excluded on both
/// branches — they enter the world only through the far stations' special
/// offer — which also empties the Guild's produce pool, making it stock a
/// uniform spread.
fn stock_kind(station: PoiId, roll: u64) -> Kind {
    // Never on ordinary stock: suspicious crates enter through the far
    // stations' special offer, very mysterious crates only through ???,
    // and casino chips only through losing.
    let stockable = |kind: Kind| {
        !matches!(
            kind,
            Kind::SuspiciousCrate | Kind::VeryMysteriousCrate | Kind::CasinoChip
        )
    };
    let produce = |kind: Kind| stockable(kind) && VALUE[usize::from(station)][kind.index()] == 0;
    let has_produce = Kind::ALL.iter().any(|&kind| produce(kind));
    let from_produce = has_produce && roll % 10 < PRODUCE_CHANCE;
    let pool: Vec<Kind> = Kind::ALL
        .iter()
        .copied()
        .filter(|&kind| stockable(kind) && produce(kind) == from_produce)
        .collect();
    pool[((roll / 10) % pool.len() as u64) as usize]
}

/// Every cell of `room` whose tile reads `class`, in row-major order —
/// the order a room fills its own tiles, and the order the stoker reads
/// the hopper in.
#[must_use]
pub fn tiles_of(rooms: &Rooms, room: RoomId, class: Tile) -> Vec<(u8, u8)> {
    let Some(kind) = rooms.kind(room) else {
        return Vec::new();
    };
    let (cols, rows) = kind.grid();
    let mut cells = Vec::new();
    for y in 0..rows {
        for x in 0..cols {
            if kind.tile_of(x, y) == Some(class) {
                cells.push((x, y));
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::room::{CABIN, RoomKind};

    /// Venus, whose produce is the perfume at kind index 0.
    const VENUS: PoiId = 0;

    /// A ship with a trade room attached, and that room's id.
    fn stall() -> (Rooms, RoomId) {
        let mut rooms = Rooms::new();
        let trade = rooms
            .spawn(RoomKind::Trade, CABIN)
            .expect("a trade room attaches");
        (rooms, trade)
    }

    /// A piece at `loc`, id and variant immaterial to valuation.
    const fn piece(kind: Kind, loc: Loc) -> Piece {
        Piece {
            id: 0,
            kind,
            variant: 0,
            gnawed: false,
            loc,
        }
    }

    #[test]
    fn same_visit_regenerates_identically_and_the_next_differs() {
        let stock = |n| stock_kinds(42, VENUS, n, &[], 0, false, 5);
        assert_eq!(stock(3), stock(3));
        assert_eq!(visit_values(42, VENUS, 3), visit_values(42, VENUS, 3));
        assert_eq!(open(42, VENUS, 3), open(42, VENUS, 3));
        let differs = (4..10).any(|n| {
            stock(n) != stock(3) || visit_values(42, VENUS, n) != visit_values(42, VENUS, 3)
        });
        assert!(differs, "six later visits identical to visit 3");
    }

    #[test]
    fn values_jitter_within_one_of_base_and_guild_crate_stays_zero() {
        for station in 0..POI_COUNT as PoiId {
            for n in 1..40 {
                let values = visit_values(99, station, n);
                for (k, &value) in values.iter().enumerate() {
                    let base = VALUE[usize::from(station)][k];
                    assert!(value <= VALUE_MAX);
                    assert!(
                        i16::from(value).abs_diff(i16::from(base)) <= 1,
                        "kind {k} at station {station} jittered {base} -> {value}"
                    );
                }
                assert_eq!(
                    visit_values(99, GUILD, n)[Kind::SuspiciousCrate.index()],
                    0,
                    "the Guild never wants its crates back"
                );
            }
        }
    }

    #[test]
    fn wants_skip_zero_valued_kinds_and_break_ties_by_index() {
        let values = [
            2, 4, 4, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0,
        ];
        assert_eq!(
            wants(&values),
            [
                (Kind::SuspiciousCrate, 3), // value 6
                (Kind::GildedIdol, 2),      // value 4, lower index wins the tie
                (Kind::RationBricks, 2),    // value 4
            ]
        );
        for station in 0..POI_COUNT as PoiId {
            for n in 1..30 {
                for (_, pips) in wants(&visit_values(7, station, n)) {
                    assert!((1..=3).contains(&pips), "pips {pips} out of range");
                }
            }
        }
    }

    /// The composition law: the room takes the best pile the proposal
    /// covers, marked kinds first, ties broken deterministically.
    #[test]
    fn a_room_composes_the_best_pile_the_proposal_covers() {
        // Budget 5 against three goods costing 4, 3, and 2.
        let candidates = [(10, 4, false), (11, 3, false), (12, 2, false)];
        assert_eq!(compose(5, &candidates), vec![10]);
        assert_eq!(compose(9, &candidates), vec![10, 11, 12]);
        assert_eq!(compose(0, &candidates), Vec::<u32>::new());
        // A mark jumps the queue: with the cheap one marked, the budget
        // buys it first and the dear one no longer fits.
        let marked = [(10, 4, false), (11, 3, false), (12, 2, true)];
        assert_eq!(compose(5, &marked), vec![11, 12]);
        // Ties break by id, always the same way.
        let tied = [(20, 3, false), (5, 3, false)];
        assert_eq!(compose(3, &tied), vec![5]);
    }

    #[test]
    fn a_well_lit_painting_prices_one_higher_on_the_offer_area() {
        let (rooms, trade) = stall();
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::Painting.index()] = 3;
        let offer = tiles_of(&rooms, trade, Tile::Offer)[0];
        let art = piece(
            Kind::Painting,
            Loc::Hold {
                room: trade,
                x: offer.0,
                y: offer.1,
            },
        );
        assert_eq!(piece_value(&rooms, &art, &[art], &values, false), 3);
        // A lamp lit aboard appraises the proposal one dearer.
        let lamp = piece(
            Kind::CeilingLamp,
            Loc::Hold {
                room: CABIN,
                x: 16,
                y: 4,
            },
        );
        assert_eq!(piece_value(&rooms, &art, &[art, lamp], &values, false), 4);
        // Berthed aboard, the rule is literal adjacency instead: hung on
        // the far aft wall, the same lamp lights nothing.
        let hung = piece(
            Kind::Painting,
            Loc::Hold {
                room: CABIN,
                x: 5,
                y: 1,
            },
        );
        assert_eq!(piece_value(&rooms, &hung, &[hung, lamp], &values, false), 3);
    }

    #[test]
    fn a_gnawed_piece_prices_two_lower_with_a_floor_at_zero() {
        let (rooms, _) = stall();
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::BrinePearls.index()] = 5;
        values[Kind::PerfumeVial.index()] = 1;
        let at = Loc::Hold {
            room: CABIN,
            x: 4,
            y: 4,
        };
        let fresh = piece(Kind::BrinePearls, at);
        let bitten = Piece {
            gnawed: true,
            ..fresh
        };
        assert_eq!(piece_value(&rooms, &fresh, &[fresh], &values, false), 5);
        assert_eq!(piece_value(&rooms, &bitten, &[bitten], &values, false), 3);
        // The Umbra Market calls the bite artisanal and pays a premium.
        assert_eq!(piece_value(&rooms, &bitten, &[bitten], &values, true), 7);
        // The malus floors at zero rather than going negative.
        let cheap = Piece {
            kind: Kind::PerfumeVial,
            gnawed: true,
            ..fresh
        };
        assert_eq!(piece_value(&rooms, &cheap, &[cheap], &values, false), 0);
    }

    #[test]
    fn generosity_scales_the_celebration_but_never_past_one() {
        assert!((deal_value(4, 2) - 1.0).abs() < 1e-6);
        assert!((deal_value(3, 2) - 0.5).abs() < 1e-6);
        assert!(deal_value(2, 4) < 1e-6);
        // A pure gift reads by what was given, not by a ratio.
        assert!(deal_value(2, 0) < deal_value(6, 0));
    }

    #[test]
    fn stock_leans_toward_produce_and_ordinary_rolls_never_yield_crates() {
        let mut perfume = 0_usize;
        let mut total = 0_usize;
        let mut kinds_seen = std::collections::BTreeSet::new();
        // A crate aboard suppresses the special offer, so every good
        // here comes from the ordinary rolls under test.
        let aboard = [piece(
            Kind::SuspiciousCrate,
            Loc::Hold {
                room: CABIN,
                x: 4,
                y: 4,
            },
        )];
        for n in 1..200 {
            let stock = stock_kinds(0xFEED, VENUS, n, &aboard, 0, false, 5);
            assert!((STOCK_MIN..=STOCK_MAX).contains(&stock.len()));
            for kind in stock {
                assert_ne!(kind, Kind::SuspiciousCrate);
                kinds_seen.insert(kind.index());
                perfume += usize::from(kind == Kind::PerfumeVial);
                total += 1;
            }
        }
        assert!(
            perfume * 10 > total * 5,
            "only {perfume}/{total} goods were Venus produce"
        );
        assert!(perfume < total, "the 30% branch never fired");
        assert!(kinds_seen.len() >= 3, "stock lacks variety: {kinds_seen:?}");
    }

    #[test]
    fn far_stations_offer_crates_one_in_five_and_respect_the_singleton() {
        let mut offered = 0_usize;
        for n in 1..=600 {
            let stock = stock_kinds(0xFEED, VENUS, n, &[], 0, false, 5);
            let crates = stock
                .iter()
                .filter(|&&kind| kind == Kind::SuspiciousCrate)
                .count();
            assert!(crates <= 1, "visit {n} stocked {crates} crates");
            offered += crates;
        }
        assert!(
            (70..=180).contains(&offered),
            "{offered}/600 crate offers is far from one in five"
        );
        for station in 0..POI_COUNT as PoiId {
            if matches!(
                station,
                GUILD | HERMITAGE | super::super::map::COMET | super::super::map::WANDERER
            ) {
                continue;
            }
            let some = (1..=60).any(|n| {
                stock_kinds(0xFEED, station, n, &[], 0, false, 5).contains(&Kind::SuspiciousCrate)
            });
            assert!(some, "station {station} never offered a crate");
        }
        // One aboard anywhere suppresses the offer.
        let aboard = [piece(
            Kind::SuspiciousCrate,
            Loc::Hold {
                room: CABIN,
                x: 4,
                y: 4,
            },
        )];
        for n in 1..=100 {
            assert!(
                !stock_kinds(0xFEED, VENUS, n, &aboard, 0, false, 5)
                    .contains(&Kind::SuspiciousCrate),
                "visit {n} offered a second crate"
            );
        }
    }

    #[test]
    fn the_guild_never_offers_crates() {
        for n in 1..=200 {
            assert!(
                !stock_kinds(0xFEED, GUILD, n, &[], 0, false, 5).contains(&Kind::SuspiciousCrate),
                "the Guild stocked a crate on visit {n}"
            );
        }
    }

    #[test]
    fn every_new_fixture_is_wanted_somewhere_and_umbra_snubs_lamps() {
        for kind in [
            Kind::CeilingLamp,
            Kind::WallLamp,
            Kind::FloorLamp,
            Kind::Couch,
            Kind::Painting,
            Kind::Cabinet,
            Kind::Rug,
            Kind::PaintTin,
            Kind::LuminousPaint,
        ] {
            let asked = (0..POI_COUNT as PoiId).any(|station| {
                (1..=300).any(|n| {
                    wants(&visit_values(0xF1C5, station, n))
                        .iter()
                        .any(|&(want, _)| want == kind)
                })
            });
            assert!(asked, "{kind:?} is never in any wants row");
        }
        for lamp in [Kind::CeilingLamp, Kind::WallLamp, Kind::FloorLamp] {
            assert_eq!(VALUE[usize::from(UMBRA)][lamp.index()], 0);
        }
    }
}
