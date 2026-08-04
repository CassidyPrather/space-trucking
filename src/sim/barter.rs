//! The no-currency barter minigame: shelves, pads, and an eagerness dial.
//!
//! Each dock generates a fresh visit — shelf goods weighted toward what the
//! station produces, a jittered copy of its value table, and a top-three
//! wants list — all derived from `splitmix(seed, station, visit)`, so a save
//! can rebuild the whole thing without storing it. The persistent run RNG is
//! spent only on cosmetic variant rolls, per the determinism rules.

use super::cargo::{KIND_COUNT, Kind, Loc, Piece, VARIANTS};
use super::map::{POI_COUNT, PoiId};
use super::splitmix;

/// One station visit's trading state. Regenerated per dock, rebuilt on load.
#[derive(Clone, Debug, PartialEq)]
pub struct Barter {
    pub station: PoiId,
    /// How many times this station has been visited, 1-based.
    pub visit: u32,
    /// Top-three kinds the station values this visit, with 1–3 want pips.
    pub wants: [(Kind, u8); 3],
    /// Trade quality as the station sees it: give value over take cost.
    pub eagerness: f32,
    /// Last tick's eagerness, for dial interpolation.
    pub prev_eagerness: f32,
    /// Whether pulling the accept lever would conclude the trade.
    pub ready: bool,
}

/// How much each station values each kind, `0..=6`.
///
/// Rows follow map order (Venus, Earth, Mars, Jupiter, Uranus, Neptune,
/// Guild), columns follow [`Kind::index`] order. Low value doubles as "local
/// produce": stations shelve what they barely value.
pub const VALUE: [[u8; KIND_COUNT]; POI_COUNT] = [
    [0, 1, 2, 1, 3, 2, 3, 5, 4], // Venus
    [4, 3, 0, 2, 4, 1, 2, 3, 4], // Earth
    [2, 1, 4, 0, 1, 3, 2, 2, 4], // Mars
    [1, 2, 4, 3, 5, 0, 1, 2, 4], // Jupiter
    [2, 3, 3, 2, 4, 4, 0, 1, 4], // Uranus
    [3, 2, 4, 3, 3, 2, 1, 0, 4], // Neptune
    [2, 2, 2, 2, 2, 2, 2, 2, 0], // Guild
];

/// Ceiling for jittered per-visit values.
const VALUE_MAX: u8 = 6;

/// Fewest goods a station shelves.
const SHELF_MIN: usize = 2;

/// Most goods a station shelves; also the number of shelf slots.
const SHELF_MAX: usize = 4;

/// This visit's value table: the station row jittered by ±1 per kind,
/// clamped to `0..=6`. Pure, so saves rebuild it instead of storing it.
#[must_use]
pub(crate) fn visit_values(seed: u64, station: PoiId, visit: u32) -> [u8; KIND_COUNT] {
    let h = visit_hash(seed, station, visit);
    let mut values = [0_u8; KIND_COUNT];
    for (k, value) in values.iter_mut().enumerate() {
        let base = VALUE[usize::from(station)][k];
        *value = match splitmix(h, k as u64) % 3 {
            0 => base.saturating_sub(1),
            1 => base,
            _ => (base + 1).min(VALUE_MAX),
        };
    }
    values
}

/// Generate one visit: the barter state plus the station's shelf pieces.
/// `rng` is the persistent run RNG, spent only on variant rolls.
pub(crate) fn generate(
    seed: u64,
    station: PoiId,
    visit: u32,
    rng: &mut fastrand::Rng,
    next_id: &mut u32,
) -> (Barter, Vec<Piece>) {
    let values = visit_values(seed, station, visit);
    let mut throwaway = fastrand::Rng::with_seed(visit_hash(seed, station, visit));
    let count = SHELF_MIN + throwaway.usize(..=SHELF_MAX - SHELF_MIN);
    let shelf = (0..count)
        .map(|slot| {
            let piece = Piece {
                id: *next_id,
                kind: produce_kind(station, &mut throwaway),
                variant: rng.u8(..VARIANTS),
                loc: Loc::StationShelf { slot: slot as u8 },
            };
            *next_id += 1;
            piece
        })
        .collect();
    let barter = Barter {
        station,
        visit,
        wants: wants(&values),
        eagerness: 0.0,
        prev_eagerness: 0.0,
        ready: false,
    };
    (barter, shelf)
}

/// Rebuild a visit's barter state from a save: same wants as [`generate`]
/// produced, eagerness recomputed from the restored pieces.
#[must_use]
pub(crate) fn rebuild(seed: u64, station: PoiId, visit: u32, pieces: &[Piece]) -> Barter {
    let values = visit_values(seed, station, visit);
    let (eagerness, ready) = eagerness_of(pieces, &values);
    Barter {
        station,
        visit,
        wants: wants(&values),
        eagerness,
        prev_eagerness: eagerness,
        ready,
    }
}

/// Trade quality from the pads: value given over cost taken, both priced by
/// this visit's table, with a floor of 1 per taken item so nothing is free.
/// Ready means something is taken and the station at least breaks even.
#[must_use]
pub(crate) fn eagerness_of(pieces: &[Piece], values: &[u8; KIND_COUNT]) -> (f32, bool) {
    let mut give = 0_u32;
    let mut take = 0_u32;
    for piece in pieces {
        let value = u32::from(values[piece.kind.index()]);
        match piece.loc {
            Loc::GivePad { .. } => give += value,
            Loc::TakePad { .. } => take += value.max(1),
            _ => {}
        }
    }
    if take == 0 {
        (0.0, false)
    } else {
        let eagerness = give as f32 / take as f32;
        (eagerness, eagerness >= 1.0)
    }
}

/// One hash per (seed, station, visit) triple, the root of everything a
/// visit derives.
const fn visit_hash(seed: u64, station: PoiId, visit: u32) -> u64 {
    splitmix(splitmix(seed, station as u64), visit as u64)
}

/// Top-three valued kinds this visit, pips `ceil(value / 2)`.
fn wants(values: &[u8; KIND_COUNT]) -> [(Kind, u8); 3] {
    let mut order: Vec<usize> = (0..KIND_COUNT).collect();
    order.sort_by_key(|&k| (std::cmp::Reverse(values[k]), k));
    let want = |k: usize| (Kind::ALL[k], values[k].div_ceil(2));
    [want(order[0]), want(order[1]), want(order[2])]
}

/// Pick a shelf kind weighted toward the station's produce — the less it
/// values a kind, the more of it sits on the dock.
fn produce_kind(station: PoiId, throwaway: &mut fastrand::Rng) -> Kind {
    let weight = |k: &Kind| u32::from(VALUE_MAX + 1 - VALUE[usize::from(station)][k.index()]);
    let total: u32 = Kind::ALL.iter().map(weight).sum();
    let mut roll = throwaway.u32(..total);
    for kind in Kind::ALL {
        let w = weight(&kind);
        if roll < w {
            return kind;
        }
        roll -= w;
    }
    unreachable!("weights sum to total, so the roll always lands")
}
