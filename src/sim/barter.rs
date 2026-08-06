//! The no-currency barter minigame: shelves, pads, and an eagerness dial.
//!
//! Each dock generates a fresh visit — a shelf leaning toward what the
//! station produces, a jittered copy of its value table, and a top-three
//! wants list — all derived from `splitmix(seed, station, visit)`, so a save
//! can rebuild the whole thing without storing it. The persistent run RNG is
//! spent only on cosmetic variant rolls, per the determinism rules.

use super::cargo::{KIND_COUNT, Kind, Loc, Piece, Tag, VARIANTS, lamp_lit, lit_adjacent};
use super::map::{GUILD, HERMITAGE, POI_COUNT, PoiId, UMBRA};
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
    /// How much of the composed trade is guesswork, `0..=1`: the fraction
    /// of pad pieces whose kind the player has never traded at this
    /// station. The renderer fogs the needle and withholds the go-lamp by
    /// this — discovery has a cost, and the cost is finding out.
    pub fog: f32,
    /// Refused lever pulls the station will still tolerate this visit.
    /// At zero the shutters come down (see `Sim::conclude`); a gift is
    /// the one thing that reopens them.
    pub patience: u8,
}

/// How much each station values each kind, `0..=6`.
///
/// Rows follow map order (Venus, Earth, Mars, Jupiter, Uranus, Neptune,
/// Guild, Saturn, Umbra Market, Hermitage, comet, `???`), columns follow
/// [`Kind::index`] order. A zero doubles as "local produce": stations
/// shelve the kind they do not value (which is how the Guild comes to
/// broker transit chits, the Umbra Market to bottle midnight — and, since
/// its lamp columns are zeros, to fence seized lamps cheap: light is a
/// rival product there, sold only snuffed). Every row must keep at least
/// three kinds at 2 or above so the wants list survives the ±1 jitter,
/// and both suspicious columns are 4 everywhere but the Guild, which
/// seizes rather than pays. The five fixture columns are lore-directed:
/// Venus buys tack, Earth rations light, Saturn treasures working
/// fixtures, the Hermitage pays best for the couch. The cabinet column
/// (last) follows the same lore: Saturn prizes working furniture, Earth
/// and the Hermitage are practical people, and even the Umbra Market
/// pays fair for a box that keeps light in its place. The comet and
/// `???` never open a barter, so their rows are placeholders kept valid
/// for the invariants above.
// Kept tabular by hand: one row per station is how this table is tuned.
#[rustfmt::skip]
pub const VALUE: [[u8; KIND_COUNT]; POI_COUNT] = [
    [0, 1, 2, 1, 3, 2, 3, 5, 4, 1, 4, 3, 5, 4, 2, 1, 5, 4, 4, 4, 6, 3], // Venus
    [4, 3, 0, 2, 4, 1, 2, 3, 4, 1, 4, 2, 5, 2, 2, 1, 2, 2, 2, 3, 1, 4], // Earth
    [2, 1, 4, 0, 1, 3, 2, 2, 4, 1, 4, 2, 5, 1, 2, 1, 2, 2, 3, 3, 2, 3], // Mars
    [1, 2, 4, 3, 5, 0, 1, 2, 4, 1, 4, 2, 5, 1, 1, 1, 3, 2, 2, 3, 2, 2], // Jupiter
    [2, 3, 3, 2, 4, 4, 0, 1, 4, 1, 4, 2, 5, 1, 1, 1, 2, 3, 2, 2, 3, 2], // Uranus
    [3, 2, 4, 3, 3, 2, 1, 0, 4, 1, 4, 2, 5, 2, 1, 1, 2, 2, 3, 2, 3, 2], // Neptune
    [2, 2, 2, 2, 2, 2, 2, 2, 0, 1, 0, 2, 3, 1, 0, 1, 1, 2, 1, 2, 1, 1], // Guild
    [1, 3, 2, 6, 1, 0, 2, 1, 4, 1, 4, 1, 5, 1, 1, 1, 4, 4, 3, 5, 3, 5], // Saturn
    [3, 2, 1, 1, 2, 0, 5, 4, 4, 3, 4, 3, 0, 2, 2, 1, 0, 0, 0, 3, 4, 3], // Umbra Market
    [1, 1, 3, 1, 4, 1, 1, 2, 4, 2, 4, 2, 3, 3, 1, 1, 2, 3, 2, 6, 3, 4], // Hermitage
    [2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 4, 2, 2, 2, 2, 1, 1, 1, 1, 1, 2, 2], // comet (no barter)
    [2, 2, 2, 2, 2, 2, 2, 2, 4, 2, 4, 2, 2, 2, 2, 1, 1, 1, 1, 1, 3, 2], // ??? (no barter)
];

/// Ceiling for jittered per-visit values.
const VALUE_MAX: u8 = 6;

/// Fewest goods a station shelves.
const SHELF_MIN: usize = 2;

/// Most goods a station shelves; also the number of shelf slots.
const SHELF_MAX: usize = 4;

/// Dial ceiling: the eased eagerness pegs here however lavish the offer.
pub const EAGER_MAX: f32 = 2.0;

/// Refused pulls a station tolerates per visit before shuttering.
pub const PATIENCE: u8 = 3;

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

/// Generate one visit: the barter state plus the station's shelf pieces.
///
/// The shelf holds 2–4 goods, each 70% from the station's produce and 30%
/// uniform over the rest; crates never come from those rolls. A far station
/// shelves a crate one visit in five — never while `aboard` already carries
/// one anywhere — and the Guild never offers one: crates flow outward from
/// the frontier and home to the hangar. `rng` is the persistent run RNG,
/// spent only on variant rolls.
// A visit is genuinely this many ingredients; a params struct would just
// rename the arguments.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate(
    seed: u64,
    station: PoiId,
    visit: u32,
    aboard: &[Piece],
    karma: u32,
    paraded: bool,
    rng: &mut fastrand::Rng,
    next_id: &mut u32,
) -> (Barter, Vec<Piece>) {
    let values = visit_values(seed, station, visit);
    let h = visit_hash(seed, station, visit);

    let crate_aboard = aboard
        .iter()
        .any(|piece| matches!(piece.kind.tag(), Some(Tag::Suspicious)));
    // The Hermitage never deals in crates; its economy is the karma one.
    let offer_crate = station != GUILD
        && station != HERMITAGE
        && !crate_aboard
        && splitmix(h, SALT_CRATE) % CRATE_CHANCE == 0;

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
    // After the Grand Parade, mysterious crates trickle onto ordinary
    // shelves: whatever the hangar was counting, the counting continues.
    if paraded && station != GUILD && splitmix(h, SALT_MYSTERY) % 4 == 0 {
        let slot = goods.len();
        if slot < SHELF_MAX {
            goods.push(shelve(Kind::MysteriousCrate, slot));
        }
    }
    let span = (SHELF_MAX - SHELF_MIN) as u64 + 1;
    let count = if station == HERMITAGE {
        // The gift economy: the hermits shelve nothing for strangers, and
        // one good per two pieces ever gifted to them — generosity comes
        // back, slowly, and never as a transaction.
        (karma as usize / 2).min(SHELF_MAX)
    } else {
        SHELF_MIN + (splitmix(h, SALT_COUNT) % span) as usize
    };
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
        fog: 0.0,
        patience: PATIENCE,
    };
    (barter, goods)
}

/// Rebuild a visit's barter state from a save: same wants as [`generate`]
/// produced, dial snapped to the trade the restored pieces compose. The
/// loader then overwrites the dial with the save's eased value.
#[must_use]
pub(crate) fn rebuild(
    seed: u64,
    station: PoiId,
    visit: u32,
    pieces: &[Piece],
    familiar: u32,
) -> Barter {
    let values = visit_values(seed, station, visit);
    let (target, ready) = eagerness_of(pieces, &values, gnaw_loved(station));
    let eagerness = target.clamp(0.0, EAGER_MAX);
    Barter {
        station,
        visit,
        wants: wants(&values),
        eagerness,
        prev_eagerness: eagerness,
        ready,
        fog: fog_of(pieces, familiar),
        patience: PATIENCE,
    }
}

/// The guesswork fraction of the composed trade: pad pieces whose kind is
/// not in this station's `familiar` bitmask, over all pad pieces. Empty
/// pads read zero — nothing composed, nothing foggy.
#[must_use]
pub(crate) fn fog_of(pieces: &[Piece], familiar: u32) -> f32 {
    let mut on_pads = 0_u32;
    let mut unknown = 0_u32;
    for piece in pieces {
        if matches!(piece.loc, Loc::GivePad { .. } | Loc::TakePad { .. }) {
            on_pads += 1;
            if familiar & (1 << piece.kind.index()) == 0 {
                unknown += 1;
            }
        }
    }
    if on_pads == 0 {
        0.0
    } else {
        unknown as f32 / on_pads as f32
    }
}

/// Gifted value that saturates the accept cue's celebration. The dial pegs
/// for any gift; only the deal's *sound* scales with what was given.
const GIFT_WARMTH: f32 = 8.0;

/// What a rat's bite knocks off a piece's value, floored at zero. The gnaw
/// is permanent, so the discount follows the piece through the economy —
/// on the give pad, on the take pad, and back off the station's shelf.
pub(crate) const GNAW_MALUS: u8 = 2;

/// Whether `station` considers a rat's toothwork artisanal. The Umbra
/// Market does: there, the malus flips into a premium of the same size,
/// and a stowaway becomes a business partner.
#[must_use]
pub(crate) const fn gnaw_loved(station: PoiId) -> bool {
    station == UMBRA
}

/// What lamplight adds to a well-lit painting's price. Only ever added,
/// never subtracted, so the dial's monotone law survives it.
pub(crate) const LIT_BONUS: u8 = 1;

/// Whether `piece` is a painting shown in good light. In the hold that is
/// literal: some cell of its footprint reads [`lit_adjacent`]. On the
/// trade pads — the only places valuation actually prices — the piece is
/// appraised under the hold's lamplight, so any lit lamp aboard counts.
/// Lamp state always comes from the hold ([`lamp_lit`]): a lamp riding a
/// pad or shelf lights nothing, which is exactly what keeps the dial
/// monotone — composing a trade with a lamp never re-prices the art
/// already on the pads.
fn well_lit(piece: &Piece, pieces: &[Piece]) -> bool {
    if piece.kind != Kind::Painting {
        return false;
    }
    match piece.loc {
        Loc::Hold { x, y } => {
            let (w, h) = piece.kind.cells();
            (0..w).any(|dx| (0..h).any(|dy| lit_adjacent(pieces, x + dx, y + dy)))
        }
        Loc::GivePad { .. } | Loc::TakePad { .. } => pieces.iter().any(lamp_lit),
        _ => false,
    }
}

/// One piece's worth under this visit's table: its kind's jittered value,
/// less [`GNAW_MALUS`] if a rat has been at it — or MORE by the same
/// amount where the bite is loved (see [`gnaw_loved`]) — plus
/// [`LIT_BONUS`] for a painting under lamplight (see [`well_lit`]). The
/// single per-piece pricing rule; both pad totals read it, so a gnawed
/// piece is cheaper to buy exactly as it is poorer to sell, and well-lit
/// art is dearer to buy exactly as it is richer to sell.
fn piece_value(piece: &Piece, pieces: &[Piece], values: &[u8; KIND_COUNT], gnaw_love: bool) -> u32 {
    let value = values[piece.kind.index()];
    let value = if !piece.gnawed {
        u32::from(value)
    } else if gnaw_love {
        u32::from(value) + u32::from(GNAW_MALUS)
    } else {
        u32::from(value.saturating_sub(GNAW_MALUS))
    };
    if well_lit(piece, pieces) {
        value + u32::from(LIT_BONUS)
    } else {
        value
    }
}

/// Value given, cost asked, and whether the give pad holds anything at all,
/// priced by this visit's table via [`piece_value`]. The +1 markup per
/// taken item keeps even worthless goods from being free.
fn pad_totals(pieces: &[Piece], values: &[u8; KIND_COUNT], gnaw_love: bool) -> (u32, u32, bool) {
    let mut give = 0_u32;
    let mut giving = false;
    let mut take = 0_u32;
    for piece in pieces {
        let value = piece_value(piece, pieces, values, gnaw_love);
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
pub(crate) fn eagerness_of(
    pieces: &[Piece],
    values: &[u8; KIND_COUNT],
    gnaw_love: bool,
) -> (f32, bool) {
    let (give, take, giving) = pad_totals(pieces, values, gnaw_love);
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
pub(crate) fn deal_value(pieces: &[Piece], values: &[u8; KIND_COUNT], gnaw_love: bool) -> f32 {
    let (give, take, _) = pad_totals(pieces, values, gnaw_love);
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
    // Never on an ordinary shelf: suspicious crates enter through the far
    // stations' special offer, very mysterious crates only through ???,
    // and casino chips only through losing.
    let shelvable = |kind: Kind| {
        !matches!(
            kind,
            Kind::SuspiciousCrate | Kind::VeryMysteriousCrate | Kind::CasinoChip
        )
    };
    let produce = |kind: Kind| shelvable(kind) && VALUE[usize::from(station)][kind.index()] == 0;
    let has_produce = Kind::ALL.iter().any(|&kind| produce(kind));
    let from_produce = has_produce && roll % 10 < PRODUCE_CHANCE;
    let pool: Vec<Kind> = Kind::ALL
        .iter()
        .copied()
        .filter(|&kind| shelvable(kind) && produce(kind) == from_produce)
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
        generate(seed, station, n, aboard, 0, false, &mut rng, &mut next_id)
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
        let values = [
            2, 4, 4, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
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
        assert_eq!(eagerness_of(&[], &values, false), (0.0, false));
    }

    #[test]
    fn a_pure_gift_pegs_the_dial_and_scales_the_celebration() {
        let values = visit_values(1, VENUS, 1);
        let pieces = [piece(Kind::BrinePearls, Loc::GivePad { slot: 0 })];
        let (eagerness, ready) = eagerness_of(&pieces, &values, false);
        assert!(ready, "a gift must always be accepted");
        assert!(
            eagerness >= EAGER_MAX,
            "a gift is the limit of asking nothing: the dial pegs, never a \
             lower reading a take item could jump above"
        );
        // A worthless gift is still a gift: ready even at value zero.
        let zeroed = [0_u8; KIND_COUNT];
        let (pegged, ready) = eagerness_of(&pieces, &zeroed, false);
        assert!(ready);
        assert!(pegged >= EAGER_MAX);
        // The celebration, unlike the dial, scales with what was given.
        assert!(deal_value(&pieces, &zeroed, false) < deal_value(&pieces, &values, false));
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
            eagerness_of(pieces, values, false).0.clamp(0.0, EAGER_MAX)
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
    fn every_new_fixture_is_wanted_somewhere_and_umbra_snubs_lamps() {
        // The wants row must be able to ask for each fixture: for every new
        // kind, some station's visit (base table plus jitter) ranks it
        // top-three.
        for kind in [
            Kind::CeilingLamp,
            Kind::WallLamp,
            Kind::FloorLamp,
            Kind::Couch,
            Kind::Painting,
            Kind::Cabinet,
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
        // The Umbra Market pays zero for every lamp — light is a rival
        // product — which also files lamps under its local produce.
        for lamp in [Kind::CeilingLamp, Kind::WallLamp, Kind::FloorLamp] {
            assert_eq!(VALUE[usize::from(UMBRA)][lamp.index()], 0);
        }
    }

    #[test]
    fn a_well_lit_painting_prices_one_higher_on_either_pad() {
        let mut values = [0_u8; KIND_COUNT];
        values[Kind::Painting.index()] = 3;
        values[Kind::BrinePearls.index()] = 5;
        let ask = piece(Kind::Seedlings, Loc::TakePad { slot: 0 });
        // Sold in the dark: the base value.
        let dark = [piece(Kind::Painting, Loc::GivePad { slot: 0 }), ask];
        assert_eq!(eagerness_of(&dark, &values, false), (3.0, true));
        // Sold under a lit lamp stowed in the hold: one more.
        let lamp = piece(Kind::CeilingLamp, Loc::Hold { x: 2, y: 0 });
        let lit = [piece(Kind::Painting, Loc::GivePad { slot: 0 }), ask, lamp];
        assert_eq!(eagerness_of(&lit, &values, false), (4.0, true));
        // A lamp riding the give pad is dark and lights nothing.
        let dark_lamp = [
            piece(Kind::Painting, Loc::GivePad { slot: 0 }),
            ask,
            piece(Kind::CeilingLamp, Loc::GivePad { slot: 1 }),
        ];
        assert_eq!(eagerness_of(&dark_lamp, &values, false), (3.0, true));
        // Asked for under the same lamplight, the painting costs one more
        // too: dearer to buy exactly as it is richer to sell.
        let give = piece(Kind::BrinePearls, Loc::GivePad { slot: 0 });
        let buy_dark = [give, piece(Kind::Painting, Loc::TakePad { slot: 0 })];
        assert_eq!(eagerness_of(&buy_dark, &values, false), (5.0 / 4.0, true));
        let buy_lit = [give, piece(Kind::Painting, Loc::TakePad { slot: 0 }), lamp];
        assert_eq!(eagerness_of(&buy_lit, &values, false), (5.0 / 5.0, true));
        // In the hold the rule is literal adjacency: a painting beside the
        // lamp reads well lit, one across the room does not, and a shelf
        // painting is never appraised at all.
        let hung = piece(Kind::Painting, Loc::Hold { x: 0, y: 1 });
        let far = piece(Kind::Painting, Loc::Hold { x: 4, y: 3 });
        let lamp_low = piece(Kind::CeilingLamp, Loc::Hold { x: 0, y: 0 });
        assert!(well_lit(&hung, &[hung, lamp_low]));
        assert!(!well_lit(&far, &[far, lamp_low]));
        let shelved = piece(Kind::Painting, Loc::StationShelf { slot: 0 });
        assert!(!well_lit(&shelved, &[shelved, lamp_low]));
    }

    /// The monotone law under lamplight: with a lit lamp stowed in the
    /// hold — every pad painting one dearer — adding to the give pad still
    /// never lowers the dial and adding to the take pad never raises it.
    #[test]
    fn the_dial_stays_monotone_with_the_hold_lamplit() {
        let mut rng = fastrand::Rng::with_seed(0x11A7);
        let dial = |pieces: &[Piece], values: &[u8; KIND_COUNT]| {
            eagerness_of(pieces, values, false).0.clamp(0.0, EAGER_MAX)
        };
        for _ in 0..500 {
            let mut values = [0_u8; KIND_COUNT];
            for v in &mut values {
                *v = rng.u8(0..=6);
            }
            let mut pieces = vec![piece(Kind::WallLamp, Loc::Hold { x: 0, y: 0 })];
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
                    "giving {kind:?} under lamplight lowered the dial: {before} -> {after}"
                );
            } else {
                assert!(
                    after <= before + 1e-6,
                    "asking for {kind:?} under lamplight raised the dial: {before} -> {after}"
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
        assert_eq!(eagerness_of(&pieces, &values, false), (1.0, true));
        // A valued piece costs value + 1: give 1 against cost 3 is short.
        let pieces = [
            piece(Kind::PerfumeVial, Loc::GivePad { slot: 0 }),
            piece(Kind::CryoCore, Loc::TakePad { slot: 0 }),
        ];
        let (eagerness, ready) = eagerness_of(&pieces, &values, false);
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
        assert_eq!(eagerness_of(&fresh, &values, false), (5.0, true));
        assert_eq!(eagerness_of(&bitten, &values, false), (3.0, true));
        // The malus floors at zero rather than going negative: a bitten
        // vial (value 1) gives nothing, and the ratio simply reads short.
        let worthless = [gnawed(Kind::PerfumeVial, Loc::GivePad { slot: 0 }), ask];
        assert_eq!(eagerness_of(&worthless, &values, false), (0.0, false));
        // The take side discounts identically — stations resell the bite —
        // while keeping the +1 markup: cost (5 - 2) + 1 = 4.
        let buying_bitten = [
            piece(Kind::BrinePearls, Loc::GivePad { slot: 0 }),
            gnawed(Kind::BrinePearls, Loc::TakePad { slot: 0 }),
        ];
        assert_eq!(
            eagerness_of(&buying_bitten, &values, false),
            (5.0 / 4.0, true)
        );
        // The celebration reads the same totals: overshoot 5/4 - 1.
        assert!((deal_value(&buying_bitten, &values, false) - 0.25).abs() < 1e-6);
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
        assert_eq!(eagerness_of(&pieces, &values, false), (12.0, true));
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

        // Every far station rolls its own offers; none is crate-dry. The
        // Guild never offers, the Hermitage's economy is karma, and the
        // comet and ??? never open a barter at all.
        for station in 0..POI_COUNT as PoiId {
            if matches!(
                station,
                GUILD | HERMITAGE | super::super::map::COMET | super::super::map::WANDERER
            ) {
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
