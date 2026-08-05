//! The no-currency barter minigame: shelves, pads, and an eagerness dial.
//!
//! Each dock generates a fresh visit — a shelf leaning toward what the
//! station produces, a jittered copy of its value table, and a top-three
//! wants list — all derived from `splitmix(seed, station, visit)`, so a save
//! can rebuild the whole thing without storing it. The persistent run RNG is
//! spent only on cosmetic variant rolls, per the determinism rules.

use super::cargo::{KIND_COUNT, Kind, Loc, Piece, Tag, VARIANTS};
use super::map::{GUILD, POI_COUNT, PoiId};
use super::splitmix;

/// One station visit's trading state. Regenerated per dock, rebuilt on load.
///
/// Deliberately currency-free: nothing here (or anywhere in the public API)
/// is a player-visible balance. `eagerness` is a ratio the dial displays,
/// values stay internal, and trades settle piece-for-piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Barter {
    pub station: PoiId,
    /// How many times this station has been visited, 1-based.
    pub visit: u32,
    /// Top-three kinds the station values this visit, with 1–3 want pips.
    pub wants: [(Kind, u8); 3],
    /// The dial reading, `0..=EAGER_MAX`: eased toward the trade's true
    /// give-over-take ratio a little every tick.
    pub eagerness: f32,
    /// Last tick's dial reading, for render interpolation.
    pub prev_eagerness: f32,
    /// Whether pulling the accept lever would conclude the trade. Tracks the
    /// true ratio, not the eased dial, so accepting never races an animation.
    pub ready: bool,
}

/// How much each station values each kind, `0..=6`.
///
/// Rows follow map order (Venus, Earth, Mars, Jupiter, Uranus, Neptune,
/// Guild, Saturn, Umbra Market, Hermitage, comet, `???`), columns follow
/// [`Kind::index`] order. A zero doubles as "local produce": stations
/// shelve the kind they do not value. Every row must keep at least three
/// kinds at 2 or above so the wants list survives the ±1 jitter, and the
/// crate column is 4 everywhere but the Guild. The comet and `???` never
/// open a barter, so their rows are placeholders kept valid for the
/// invariants above.
pub const VALUE: [[u8; KIND_COUNT]; POI_COUNT] = [
    [0, 1, 2, 1, 3, 2, 3, 5, 4], // Venus
    [4, 3, 0, 2, 4, 1, 2, 3, 4], // Earth
    [2, 1, 4, 0, 1, 3, 2, 2, 4], // Mars
    [1, 2, 4, 3, 5, 0, 1, 2, 4], // Jupiter
    [2, 3, 3, 2, 4, 4, 0, 1, 4], // Uranus
    [3, 2, 4, 3, 3, 2, 1, 0, 4], // Neptune
    [2, 2, 2, 2, 2, 2, 2, 2, 0], // Guild
    [1, 3, 2, 6, 1, 0, 2, 1, 4], // Saturn: the ring-barons pay dearly for scrap
    [3, 2, 1, 1, 2, 0, 5, 4, 4], // Umbra Market: cold things and sea-things, at night
    [1, 1, 3, 1, 4, 1, 1, 2, 4], // Hermitage: seeds and staples; wealth bores hermits
    [2, 2, 2, 2, 2, 2, 2, 2, 4], // comet (no barter)
    [2, 2, 2, 2, 2, 2, 2, 2, 4], // ??? (no barter)
];

/// Ceiling for jittered per-visit values.
const VALUE_MAX: u8 = 6;

/// Fewest goods a station shelves.
const SHELF_MIN: usize = 2;

/// Most goods a station shelves; also the number of shelf slots.
const SHELF_MAX: usize = 4;

/// Dial ceiling: the eased eagerness pegs here however lavish the offer.
pub const EAGER_MAX: f32 = 2.0;

/// Dial speed in eagerness units per second: the full sweep takes one.
pub(crate) const EAGER_RATE: f32 = 2.0;

/// Chance a shelf roll picks from the station's produce, in tenths.
const PRODUCE_CHANCE: u64 = 7;

/// Chance denominator for a far station's crate offer: one visit in five.
const CRATE_CHANCE: u64 = 5;

/// Stream salts under the visit hash, so the crate roll, the shelf count,
/// and each shelf kind draw independent values.
const SALT_CRATE: u64 = 1;
const SALT_COUNT: u64 = 2;
const SALT_KIND: u64 = 0x100;

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
        *value = if station == GUILD && k == Kind::SuspiciousCrate.index() {
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

/// Generate one visit: the barter state plus the station's shelf pieces.
///
/// The shelf holds 2–4 goods, each 70% from the station's produce and 30%
/// uniform over the rest; crates never come from those rolls. A far station
/// shelves a crate one visit in five — never while `aboard` already carries
/// one anywhere — and the Guild never offers one: crates flow outward from
/// the frontier and home to the hangar. `rng` is the persistent run RNG,
/// spent only on variant rolls.
pub(crate) fn generate(
    seed: u64,
    station: PoiId,
    visit: u32,
    aboard: &[Piece],
    rng: &mut fastrand::Rng,
    next_id: &mut u32,
) -> (Barter, Vec<Piece>) {
    let values = visit_values(seed, station, visit);
    let h = visit_hash(seed, station, visit);

    let crate_aboard = aboard
        .iter()
        .any(|piece| matches!(piece.kind.tag(), Some(Tag::Suspicious)));
    let offer_crate =
        station != GUILD && !crate_aboard && splitmix(h, SALT_CRATE) % CRATE_CHANCE == 0;

    let mut shelve = |kind: Kind, slot: usize| {
        let piece = Piece {
            id: *next_id,
            kind,
            variant: rng.u8(..VARIANTS),
            gnawed: false,
            loc: Loc::StationShelf { slot: slot as u8 },
        };
        *next_id += 1;
        piece
    };

    let mut goods = Vec::new();
    if offer_crate {
        goods.push(shelve(Kind::SuspiciousCrate, 0));
    }
    let span = (SHELF_MAX - SHELF_MIN) as u64 + 1;
    let count = SHELF_MIN + (splitmix(h, SALT_COUNT) % span) as usize;
    // The crate takes a slot, so ordinary goods yield rather than overflow.
    let count = count.min(SHELF_MAX - goods.len());
    for i in 0..count {
        let slot = goods.len();
        let kind = shelf_kind(station, splitmix(h, SALT_KIND + i as u64));
        goods.push(shelve(kind, slot));
    }

    let barter = Barter {
        station,
        visit,
        wants: wants(&values),
        eagerness: 0.0,
        prev_eagerness: 0.0,
        ready: false,
    };
    (barter, goods)
}

/// Rebuild a visit's barter state from a save: same wants as [`generate`]
/// produced, dial snapped to the trade the restored pieces compose. The
/// loader then overwrites the dial with the save's eased value.
#[must_use]
pub(crate) fn rebuild(seed: u64, station: PoiId, visit: u32, pieces: &[Piece]) -> Barter {
    let values = visit_values(seed, station, visit);
    let (target, ready) = eagerness_of(pieces, &values);
    let eagerness = target.clamp(0.0, EAGER_MAX);
    Barter {
        station,
        visit,
        wants: wants(&values),
        eagerness,
        prev_eagerness: eagerness,
        ready,
    }
}

/// Gifted value that saturates the accept cue's celebration. The dial pegs
/// for any gift; only the deal's *sound* scales with what was given.
const GIFT_WARMTH: f32 = 8.0;

/// What a rat's bite knocks off a piece's value, floored at zero. The gnaw
/// is permanent, so the discount follows the piece through the economy —
/// on the give pad, on the take pad, and back off the station's shelf.
pub(crate) const GNAW_MALUS: u8 = 2;

/// One piece's worth under this visit's table: its kind's jittered value,
/// less [`GNAW_MALUS`] if a rat has been at it. The single per-piece
/// pricing rule; both pad totals read it, so a gnawed piece is cheaper to
/// buy exactly as it is poorer to sell.
fn piece_value(piece: &Piece, values: &[u8; KIND_COUNT]) -> u32 {
    let value = values[piece.kind.index()];
    u32::from(if piece.gnawed {
        value.saturating_sub(GNAW_MALUS)
    } else {
        value
    })
}

/// Value given, cost asked, and whether the give pad holds anything at all,
/// priced by this visit's table via [`piece_value`]. The +1 markup per
/// taken item keeps even worthless goods from being free.
fn pad_totals(pieces: &[Piece], values: &[u8; KIND_COUNT]) -> (u32, u32, bool) {
    let mut give = 0_u32;
    let mut giving = false;
    let mut take = 0_u32;
    for piece in pieces {
        let value = piece_value(piece, values);
        match piece.loc {
            Loc::GivePad { .. } => {
                give += value;
                giving = true;
            }
            Loc::TakePad { .. } => take += value + 1,
            _ => {}
        }
    }
    (give, take, giving)
}

/// Trade quality from the pads: value given over cost asked. Ready means
/// the station at least breaks even — or the trade is a pure gift, which
/// every station accepts. Gifting through the lever is the one way to shed
/// cargo, so hold space can always be freed and nothing is ever lost to a
/// stray drop.
///
/// A pure gift reads as the limiting case of "you ask for nothing": the
/// ratio's limit is unbounded eagerness, so the dial pegs. That keeps the
/// gauge monotone — loading the give pad never lowers it, loading the take
/// pad never raises it — which the property test below holds it to. Two
/// scales that disagree at the boundary is how a needle jumps the wrong
/// way when a piece crosses pads.
#[must_use]
pub(crate) fn eagerness_of(pieces: &[Piece], values: &[u8; KIND_COUNT]) -> (f32, bool) {
    let (give, take, giving) = pad_totals(pieces, values);
    if take > 0 {
        let eagerness = give as f32 / take as f32;
        (eagerness, eagerness >= 1.0)
    } else if giving {
        (f32::INFINITY, true)
    } else {
        (0.0, false)
    }
}

/// Generosity of the concluded deal in `0..=1`, for the accept cue's gain:
/// the overshoot past break-even for a trade, the gifted value itself for a
/// pure gift. Separate from the dial on purpose — the gauge answers "would
/// the station take this?", the cue answers "how big a deal was that?", and
/// a pegged needle must not make every token gift sound lavish.
#[must_use]
pub(crate) fn deal_value(pieces: &[Piece], values: &[u8; KIND_COUNT]) -> f32 {
    let (give, take, _) = pad_totals(pieces, values);
    if take > 0 {
        (give as f32 / take as f32 - 1.0).clamp(0.0, 1.0)
    } else {
        (give as f32 / GIFT_WARMTH).clamp(0.0, 1.0)
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

/// One shelf kind from one roll: 70% the station's produce (base value
/// zero), 30% uniform over the others. Crates are excluded on both branches
/// — they enter the world only through the far stations' special offer —
/// which also empties the Guild's produce pool, making it shelve a uniform
/// spread.
fn shelf_kind(station: PoiId, roll: u64) -> Kind {
    let produce = |kind: Kind| {
        kind != Kind::SuspiciousCrate && VALUE[usize::from(station)][kind.index()] == 0
    };
    let has_produce = Kind::ALL.iter().any(|&kind| produce(kind));
    let from_produce = has_produce && roll % 10 < PRODUCE_CHANCE;
    let pool: Vec<Kind> = Kind::ALL
        .iter()
        .copied()
        .filter(|&kind| kind != Kind::SuspiciousCrate && produce(kind) == from_produce)
        .collect();
    pool[((roll / 10) % pool.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Venus, whose produce is the perfume at kind index 0.
    const VENUS: PoiId = 0;

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

    /// The same piece after a rat has been at it.
    const fn gnawed(kind: Kind, loc: Loc) -> Piece {
        Piece {
            id: 0,
            kind,
            variant: 0,
            gnawed: true,
            loc,
        }
    }

    /// Run [`generate`] with fixed cosmetics, returning barter and shelf.
    fn visit(seed: u64, station: PoiId, n: u32, aboard: &[Piece]) -> (Barter, Vec<Piece>) {
        let mut rng = fastrand::Rng::with_seed(0);
        let mut next_id = 0;
        generate(seed, station, n, aboard, &mut rng, &mut next_id)
    }

    #[test]
    fn same_visit_regenerates_identically_and_the_next_differs() {
        let (a, shelf_a) = visit(42, VENUS, 3, &[]);
        let (b, shelf_b) = visit(42, VENUS, 3, &[]);
        assert_eq!(a, b);
        assert_eq!(shelf_a, shelf_b);
        assert_eq!(visit_values(42, VENUS, 3), visit_values(42, VENUS, 3));

        // The next visit re-rolls: over a handful of visits the shelves and
        // value tables cannot all repeat.
        let differs = (4..10).any(|n| {
            let (_, shelf) = visit(42, VENUS, n, &[]);
            let kinds = |s: &[Piece]| s.iter().map(|piece| piece.kind).collect::<Vec<_>>();
            kinds(&shelf) != kinds(&shelf_a)
                || visit_values(42, VENUS, n) != visit_values(42, VENUS, 3)
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
        let values = [2, 4, 4, 0, 1, 0, 0, 0, 6];
        assert_eq!(
            wants(&values),
            [
                (Kind::SuspiciousCrate, 3), // value 6
                (Kind::GildedIdol, 2),      // value 4, lower index wins the tie
                (Kind::RationBricks, 2),    // value 4
            ]
        );
        // Zeros never appear, so pips are always 1..=3.
        for station in 0..POI_COUNT as PoiId {
            for n in 1..30 {
                for (_, pips) in wants(&visit_values(7, station, n)) {
                    assert!((1..=3).contains(&pips), "pips {pips} out of range");
                }
            }
        }
    }

    #[test]
    fn empty_pads_are_zero_and_never_ready() {
        let values = visit_values(1, VENUS, 1);
        assert_eq!(eagerness_of(&[], &values), (0.0, false));
    }

    #[test]
    fn a_pure_gift_pegs_the_dial_and_scales_the_celebration() {
        let values = visit_values(1, VENUS, 1);
        let pieces = [piece(Kind::BrinePearls, Loc::GivePad { slot: 0 })];
        let (eagerness, ready) = eagerness_of(&pieces, &values);
        assert!(ready, "a gift must always be accepted");
        assert!(
            eagerness >= EAGER_MAX,
            "a gift is the limit of asking nothing: the dial pegs, never a \
             lower reading a take item could jump above"
        );
        // A worthless gift is still a gift: ready even at value zero.
        let zeroed = [0_u8; KIND_COUNT];
        let (pegged, ready) = eagerness_of(&pieces, &zeroed);
        assert!(ready);
        assert!(pegged >= EAGER_MAX);
        // The celebration, unlike the dial, scales with what was given.
        assert!(deal_value(&pieces, &zeroed) < deal_value(&pieces, &values));
    }

    /// The gauge's contract, held under fire: whatever already sits on the
    /// pads, adding to the give pad never lowers the reading and adding to
    /// the take pad never raises it. This is the genre guard for "the
    /// needle moved the wrong way" — any future pricing tweak that breaks
    /// gauge monotonicity fails here, not in someone's hands.
    #[test]
    fn dial_reading_is_monotone_under_pad_changes() {
        let mut rng = fastrand::Rng::with_seed(0xD1A1);
        let dial = |pieces: &[Piece], values: &[u8; KIND_COUNT]| {
            eagerness_of(pieces, values).0.clamp(0.0, EAGER_MAX)
        };
        for _ in 0..500 {
            let mut values = [0_u8; KIND_COUNT];
            for v in &mut values {
                *v = rng.u8(0..=6);
            }
            // A random starting spread across both pads, possibly empty.
            let mut pieces = Vec::new();
            for slot in 0..3 {
                if rng.bool() {
                    pieces.push(piece(
                        Kind::ALL[rng.usize(..KIND_COUNT)],
                        Loc::GivePad { slot },
                    ));
                }
                if rng.bool() {
                    pieces.push(piece(
                        Kind::ALL[rng.usize(..KIND_COUNT)],
                        Loc::TakePad { slot },
                    ));
                }
            }
            let before = dial(&pieces, &values);
            let kind = Kind::ALL[rng.usize(..KIND_COUNT)];
            let (loc, raises) = if rng.bool() {
                (Loc::GivePad { slot: 3 }, true)
            } else {
                (Loc::TakePad { slot: 3 }, false)
            };
            pieces.push(piece(kind, loc));
            let after = dial(&pieces, &values);
            if raises {
                assert!(
                    after >= before - 1e-6,
                    "giving {kind:?} lowered the dial: {before} -> {after}"
                );
            } else {
                assert!(
                    after <= before + 1e-6,
                    "asking for {kind:?} raised the dial: {before} -> {after}"
                );
            }
        }
    }

    #[test]
    fn take_cost_marks_every_item_up_by_one() {
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::PerfumeVial.index()] = 1;
        values[Kind::Seedlings.index()] = 0;
        values[Kind::CryoCore.index()] = 2;
        // A worthless taken piece still costs 1: give 1 exactly breaks even.
        let pieces = [
            piece(Kind::PerfumeVial, Loc::GivePad { slot: 0 }),
            piece(Kind::Seedlings, Loc::TakePad { slot: 0 }),
        ];
        assert_eq!(eagerness_of(&pieces, &values), (1.0, true));
        // A valued piece costs value + 1: give 1 against cost 3 is short.
        let pieces = [
            piece(Kind::PerfumeVial, Loc::GivePad { slot: 0 }),
            piece(Kind::CryoCore, Loc::TakePad { slot: 0 }),
        ];
        let (eagerness, ready) = eagerness_of(&pieces, &values);
        assert!((eagerness - 1.0 / 3.0).abs() < 1e-6);
        assert!(!ready);
    }

    #[test]
    fn a_gnawed_piece_prices_two_lower_with_a_floor_at_zero() {
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::BrinePearls.index()] = 5;
        values[Kind::PerfumeVial.index()] = 1;
        // On the give pad: 5 fresh, 3 bitten, against a cost-1 ask.
        let ask = piece(Kind::Seedlings, Loc::TakePad { slot: 0 });
        let fresh = [piece(Kind::BrinePearls, Loc::GivePad { slot: 0 }), ask];
        let bitten = [gnawed(Kind::BrinePearls, Loc::GivePad { slot: 0 }), ask];
        assert_eq!(eagerness_of(&fresh, &values), (5.0, true));
        assert_eq!(eagerness_of(&bitten, &values), (3.0, true));
        // The malus floors at zero rather than going negative: a bitten
        // vial (value 1) gives nothing, and the ratio simply reads short.
        let worthless = [gnawed(Kind::PerfumeVial, Loc::GivePad { slot: 0 }), ask];
        assert_eq!(eagerness_of(&worthless, &values), (0.0, false));
        // The take side discounts identically — stations resell the bite —
        // while keeping the +1 markup: cost (5 - 2) + 1 = 4.
        let buying_bitten = [
            piece(Kind::BrinePearls, Loc::GivePad { slot: 0 }),
            gnawed(Kind::BrinePearls, Loc::TakePad { slot: 0 }),
        ];
        assert_eq!(eagerness_of(&buying_bitten, &values), (5.0 / 4.0, true));
        // The celebration reads the same totals: overshoot 5/4 - 1.
        assert!((deal_value(&buying_bitten, &values) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn generosity_overshoots_past_the_dial() {
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::BrinePearls.index()] = 6;
        values[Kind::Seedlings.index()] = 0;
        let pieces = [
            piece(Kind::BrinePearls, Loc::GivePad { slot: 0 }),
            piece(Kind::BrinePearls, Loc::GivePad { slot: 1 }),
            piece(Kind::Seedlings, Loc::TakePad { slot: 0 }),
        ];
        // Give 12 against cost 1: the raw ratio runs far past EAGER_MAX;
        // the dial cap and the accept-value clamp are applied by the sim.
        assert_eq!(eagerness_of(&pieces, &values), (12.0, true));
    }

    #[test]
    fn shelves_lean_toward_produce_and_ordinary_rolls_never_yield_crates() {
        let mut perfume = 0_usize;
        let mut total = 0_usize;
        let mut kinds_seen = std::collections::BTreeSet::new();
        // A crate aboard suppresses the special offer, so every shelf good
        // here comes from the ordinary rolls under test.
        let aboard = [piece(Kind::SuspiciousCrate, Loc::Hold { x: 0, y: 0 })];
        for n in 1..200 {
            let (_, shelf) = visit(0xFEED, VENUS, n, &aboard);
            assert!((SHELF_MIN..=SHELF_MAX).contains(&shelf.len()));
            for (slot, piece) in shelf.iter().enumerate() {
                assert_eq!(piece.loc, Loc::StationShelf { slot: slot as u8 });
                assert_ne!(piece.kind, Kind::SuspiciousCrate);
                kinds_seen.insert(piece.kind.index());
                perfume += usize::from(piece.kind == Kind::PerfumeVial);
                total += 1;
            }
        }
        // 70% produce with generous statistical slack.
        assert!(
            perfume * 10 > total * 5,
            "only {perfume}/{total} shelf goods were Venus produce"
        );
        assert!(perfume < total, "the 30% branch never fired");
        assert!(
            kinds_seen.len() >= 3,
            "shelves lack variety: {kinds_seen:?}"
        );
    }

    #[test]
    fn far_stations_offer_crates_one_in_five_and_respect_the_singleton() {
        let mut offered = 0_usize;
        for n in 1..=600 {
            let (_, shelf) = visit(0xFEED, VENUS, n, &[]);
            let crates = shelf
                .iter()
                .filter(|piece| piece.kind == Kind::SuspiciousCrate)
                .count();
            assert!(crates <= 1, "visit {n} shelved {crates} crates");
            assert!((SHELF_MIN..=SHELF_MAX).contains(&shelf.len()));
            offered += crates;
        }
        assert!(
            (70..=180).contains(&offered),
            "{offered}/600 crate offers is far from one in five"
        );

        // Every far station rolls its own offers; none is crate-dry.
        for station in 0..POI_COUNT as PoiId {
            if station == GUILD {
                continue;
            }
            let some = (1..=60).any(|n| {
                let (_, shelf) = visit(0xFEED, station, n, &[]);
                shelf
                    .iter()
                    .any(|piece| piece.kind == Kind::SuspiciousCrate)
            });
            assert!(some, "station {station} never offered a crate");
        }

        // One aboard anywhere — hold or a pad — suppresses the offer.
        for loc in [Loc::Hold { x: 0, y: 0 }, Loc::GivePad { slot: 0 }] {
            let aboard = [piece(Kind::SuspiciousCrate, loc)];
            for n in 1..=100 {
                let (_, shelf) = visit(0xFEED, VENUS, n, &aboard);
                assert!(
                    shelf
                        .iter()
                        .all(|piece| piece.kind != Kind::SuspiciousCrate),
                    "visit {n} offered a second crate"
                );
            }
        }
    }

    #[test]
    fn the_guild_never_offers_crates() {
        for n in 1..=200 {
            let (_, shelf) = visit(0xFEED, GUILD, n, &[]);
            assert!(
                shelf
                    .iter()
                    .all(|piece| piece.kind != Kind::SuspiciousCrate),
                "the Guild shelved a crate on visit {n}"
            );
        }
    }
}
