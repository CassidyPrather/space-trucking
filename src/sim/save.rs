//! Versioned, line-oriented text saves.
//!
//! The format stores only what cannot be recomputed: seed, clock, RNG state,
//! delivery tally, visit counts, ship, leg counter, both event machines (the
//! omen and the rat), eased dial, and the pieces with their gnaw marks.
//! Drags are transient — a held piece serialises at its origin, so
//! a save mid-drag drops every player's drag on load. Everything
//! a visit derives (shelf layout hashes, wants, trade readiness) is rebuilt
//! from the seed on load, which keeps the format small and the determinism
//! honest. Floats that must survive exactly (the eased light, omen, and
//! eagerness) travel as hex bit patterns rather than decimal.
//!
//! Parsing never panics: every malformed line maps to
//! [`SaveError::Parse`] with its 1-based line number (line 0 means the text
//! ended too early).

use std::fmt;
use std::fmt::Write as _;
use std::str::FromStr;

use super::cargo::{self, Kind, Loc, Piece};
use super::encounter::{Drone, Drones, Encounter, EncounterKind, Encounters};
use super::event::{Omen, Phase};
use super::layout::{FLOTSAM_SLOTS, GRID_COLS, GRID_ROWS, SHELF_SLOTS};
use super::map::{POI_COUNT, PoiId, Ship, ShipState};
use super::rats::{CHASE_LIMIT, Rat, Rats};
use super::{KIND_COUNT, MAX_CREW, Sim, barter};

/// Magic-plus-version header of every save this build writes. `STV7`
/// added the banked burner's stoke to the ship line's tail; `STV6`
/// added the `laid` piece location (the dressing layer; docs/BAY.md);
/// `STV5` added `stow` (cabinet cubbies). Each is additive, so the
/// reader accepts the older headers too: a save without those tokens
/// is a valid save with a cold burner and those berths empty. `STV4` widened
/// the visits line for the orbital sky's new POIs (positions themselves are
/// derived from the tick, so none are stored); `STV3` split the leg counter
/// out of the omen line and added the rat state line plus the per-piece gnaw
/// token. Older versions fail safe as unsupported.
const MAGIC: &str = "STV8";

/// Older headers this build still reads. `STV8` moved cargo onto the
/// room net (docs/BAY.md, "The room grid"): berth coordinates from
/// older headers are console-era 6×4 hold cells, which the net embeds
/// at (+3, 0) — the reader translates them and re-berths whatever the
/// room-grid rules no longer accept in place. Everything else stays one
/// additive grammar.
const READABLE: [&str; 5] = [MAGIC, "STV7", "STV6", "STV5", "STV4"];

/// Why a save string was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    /// Not a Space Trucking save at all.
    BadMagic,
    /// A Space Trucking save from a version this build does not read.
    UnsupportedVersion,
    /// Recognised header, malformed body; `line` is 1-based (0 = truncated).
    Parse { line: usize },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a Space Trucking save"),
            Self::UnsupportedVersion => write!(f, "save is from an unsupported version"),
            Self::Parse { line: 0 } => write!(f, "save ends too early"),
            Self::Parse { line } => write!(f, "malformed save at line {line}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Serialise a sim. The inverse of [`parse`].
// One line per field is the format's clarity; splitting the writer into
// halves would only scatter it.
#[allow(clippy::too_many_lines)]
pub(crate) fn serialize(sim: &Sim) -> String {
    let mut out = String::new();
    // Writing into a String cannot fail, so the fmt plumbing is dropped.
    let _ = writeln!(out, "{MAGIC}");
    let _ = writeln!(out, "seed {}", sim.seed);
    let _ = writeln!(out, "tick {}", sim.tick);
    let _ = writeln!(out, "rng {:016x}", sim.rng.get_seed());
    let _ = writeln!(out, "warp {}", u8::from(sim.warp));
    let _ = writeln!(out, "paused {}", u8::from(sim.paused));
    let _ = writeln!(out, "deliveries {}", sim.deliveries);
    let _ = writeln!(out, "karma {}", sim.karma);
    let _ = write!(out, "familiar");
    for mask in sim.familiar {
        let _ = write!(out, " {mask:04x}");
    }
    let _ = writeln!(out);
    let _ = write!(out, "visits");
    for visit in sim.visits {
        let _ = write!(out, " {visit}");
    }
    let _ = writeln!(out);
    match sim.ship.state {
        ShipState::Docked(at) => {
            let _ = write!(out, "ship docked {at}");
        }
        ShipState::Traveling {
            from,
            to,
            progress,
            leg_ticks,
        } => {
            let _ = write!(out, "ship travel {from} {to} {progress} {leg_ticks}");
        }
    }
    let _ = writeln!(out, " {} {}", opt_token(sim.ship.selected), sim.stoke);
    let _ = writeln!(out, "legs {}", sim.legs);
    let omen = &sim.omen;
    let _ = write!(out, "omen {}", opt_token(omen.jump_at));
    match omen.phase {
        Phase::Idle => {
            let _ = write!(out, " idle 0");
        }
        Phase::Omen { elapsed } => {
            let _ = write!(out, " omen {elapsed}");
        }
        Phase::Wake { elapsed } => {
            let _ = write!(out, " wake {elapsed}");
        }
    }
    let _ = writeln!(
        out,
        " {:08x} {:08x}",
        omen.light.to_bits(),
        omen.swell.to_bits()
    );
    match &sim.encounters.current {
        None => {
            let _ = writeln!(out, "enc -");
        }
        Some(enc) => {
            let _ = writeln!(
                out,
                "enc {} {} {} {} {} {}",
                enc.kind.token(),
                enc.start,
                enc.end,
                u8::from(enc.opened),
                u8::from(enc.closed),
                u8::from(enc.used)
            );
        }
    }
    match &sim.drones.drone {
        None => {
            let _ = writeln!(out, "drone -");
        }
        Some(drone) => {
            let _ = writeln!(
                out,
                "drone {} {} {} {} {}",
                drone.start,
                drone.end,
                u8::from(drone.attached),
                u8::from(drone.gone),
                drone.swats
            );
        }
    }
    let _ = writeln!(
        out,
        "parade {} {}",
        opt_token(sim.parade_at),
        opt_token(sim.comet_visit)
    );
    match &sim.rats.rat {
        None => {
            let _ = writeln!(out, "rat -");
        }
        Some(rat) => {
            let _ = writeln!(
                out,
                "rat {} {} {} {} {} {} {} {}",
                rat.cell.0,
                rat.cell.1,
                rat.prev_cell.0,
                rat.prev_cell.1,
                rat.moved_at,
                rat.next_move,
                rat.next_nibble,
                rat.chases
            );
        }
    }
    // The dial is eased state, not derivable from the pieces alone: its bits
    // travel like light and omen do. Zero when traveling. Patience rides
    // beside it: also per-visit, also not derivable.
    let eagerness = sim.barter.as_ref().map_or(0.0, |barter| barter.eagerness);
    let patience = sim
        .barter
        .as_ref()
        .map_or(barter::PATIENCE, |barter| barter.patience);
    let _ = writeln!(out, "eager {:08x} {patience}", eagerness.to_bits());
    for piece in &sim.pieces {
        let _ = write!(
            out,
            "piece {} {} {} {}",
            piece.id,
            piece.kind.index(),
            piece.variant,
            u8::from(piece.gnawed)
        );
        match piece.loc {
            Loc::Hold { x, y } => {
                let _ = writeln!(out, " hold {x} {y}");
            }
            Loc::StationShelf { slot } => {
                let _ = writeln!(out, " shelf {slot}");
            }
            Loc::GivePad { slot } => {
                let _ = writeln!(out, " give {slot}");
            }
            Loc::TakePad { slot } => {
                let _ = writeln!(out, " take {slot}");
            }
            Loc::ReceivedShelf { slot } => {
                let _ = writeln!(out, " recv {slot}");
            }
            Loc::Flotsam { slot } => {
                let _ = writeln!(out, " flot {slot}");
            }
            Loc::Stow { cabinet, slot } => {
                let _ = writeln!(out, " stow {cabinet} {slot}");
            }
            Loc::Laid { x, y } => {
                let _ = writeln!(out, " laid {x} {y}");
            }
        }
    }
    let _ = writeln!(out, "next_piece {}", sim.next_piece);
    out
}

/// Rebuild a sim from [`serialize`] output.
pub(crate) fn parse(s: &str) -> Result<Sim, SaveError> {
    let mut reader = Reader::new(s);
    // Pre-STV8 headers carry console-era 6×4 hold coordinates that the
    // room net embeds at (+3, 0); their berths migrate after reading.
    let legacy = match reader.next_line() {
        Ok(header) if READABLE.contains(&header) => header != MAGIC,
        Ok(other) if other.starts_with("STV") => return Err(SaveError::UnsupportedVersion),
        _ => return Err(SaveError::BadMagic),
    };

    let seed = reader.kv("seed")?;
    let tick = reader.kv("tick")?;
    let rng_state = reader.kv_hex64("rng")?;
    let warp = reader.kv::<u8>("warp")? != 0;
    let paused = reader.kv::<u8>("paused")? != 0;
    let deliveries = reader.kv("deliveries")?;
    let karma = reader.kv("karma")?;
    let familiar = parse_familiar(&mut reader)?;
    let visits = parse_visits(&mut reader)?;
    let (ship, stoke) = parse_ship(&mut reader, tick)?;
    let legs = reader.kv("legs")?;
    let omen = parse_omen(&mut reader)?;
    let encounters = parse_encounter(&mut reader)?;
    let drones = parse_drone(&mut reader)?;
    let (parade_at, comet_visit) = parse_parade(&mut reader)?;
    let mut rats = parse_rat(&mut reader)?;
    if legacy && let Some(rat) = &mut rats.rat {
        rat.cell.0 += 3;
        rat.prev_cell.0 += 3;
    }
    let (eagerness, patience) = parse_eager(&mut reader)?;
    let (pieces, next_piece) = parse_pieces(&mut reader, legacy)?;

    let barter = match ship.state {
        // The comet and ??? dock without a counterparty: no barter opens.
        ShipState::Docked(at) if at != super::map::COMET && at != super::map::WANDERER => {
            let mut barter = barter::rebuild(
                seed,
                at,
                visits[usize::from(at)],
                &pieces,
                familiar[usize::from(at)],
            );
            barter.eagerness = eagerness;
            barter.prev_eagerness = eagerness;
            barter.patience = patience;
            Some(barter)
        }
        _ => None,
    };
    let values = barter.as_ref().map_or([0; KIND_COUNT], |b| {
        barter::visit_values(seed, b.station, b.visit)
    });

    // The outboard rail IS the shelf row (`FLOTSAM_SLOTS == SHELF_SLOTS`),
    // contexts exclusive by construction: docking banks the hopper before
    // any barter opens, so a save claiming staged flotsam AND an open
    // barter at once is lying — clicking "the shelf" would lift the fuel.
    // Refuse it whole rather than let the two contexts share one rect.
    if barter.is_some()
        && pieces
            .iter()
            .any(|piece| matches!(piece.loc, Loc::Flotsam { .. }))
    {
        return Err(SaveError::Parse { line: 0 });
    }

    Ok(Sim {
        seed,
        rng: fastrand::Rng::with_seed(rng_state),
        accumulator: 0.0,
        tick,
        paused,
        warp,
        cues: Vec::new(),
        ship,
        pieces,
        next_piece,
        held: [None; MAX_CREW],
        deliveries,
        barter,
        values,
        visits,
        legs,
        omen,
        rats,
        encounters,
        drones,
        parade_at,
        comet_visit,
        stoke,
        karma,
        familiar,
        night: false,
        last_violation: None,
    })
}

/// The `enc` line: this leg's encounter, if any.
fn parse_encounter(reader: &mut Reader<'_>) -> Result<Encounters, SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("enc") {
        return Err(reader.err());
    }
    match tokens.next() {
        Some("-") => Ok(Encounters { current: None }),
        Some(token) => {
            let kind = token
                .parse::<u8>()
                .ok()
                .and_then(EncounterKind::from_token)
                .ok_or_else(|| reader.err())?;
            let start: u64 = reader.token(tokens.next())?;
            let end: u64 = reader.token(tokens.next())?;
            if end <= start {
                return Err(reader.err());
            }
            let flag = |reader: &Reader<'_>, t: Option<&str>| match t {
                Some("0") => Ok(false),
                Some("1") => Ok(true),
                _ => Err(reader.err()),
            };
            let opened = flag(reader, tokens.next())?;
            let closed = flag(reader, tokens.next())?;
            let used = flag(reader, tokens.next())?;
            Ok(Encounters {
                current: Some(Encounter {
                    kind,
                    start,
                    end,
                    opened,
                    closed,
                    used,
                }),
            })
        }
        None => Err(reader.err()),
    }
}

/// The `drone` line: this leg's ad drone, if any.
fn parse_drone(reader: &mut Reader<'_>) -> Result<Drones, SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("drone") {
        return Err(reader.err());
    }
    match tokens.next() {
        Some("-") => Ok(Drones { drone: None }),
        Some(token) => {
            let start: u64 = token.parse().map_err(|_| reader.err())?;
            let end: u64 = reader.token(tokens.next())?;
            if end <= start {
                return Err(reader.err());
            }
            let flag = |reader: &Reader<'_>, t: Option<&str>| match t {
                Some("0") => Ok(false),
                Some("1") => Ok(true),
                _ => Err(reader.err()),
            };
            let attached = flag(reader, tokens.next())?;
            let gone = flag(reader, tokens.next())?;
            let swats: u8 = reader.token(tokens.next())?;
            if swats > super::encounter::AD_SWATS {
                return Err(reader.err());
            }
            Ok(Drones {
                drone: Some(Drone {
                    start,
                    end,
                    attached,
                    gone,
                    swats,
                }),
            })
        }
        None => Err(reader.err()),
    }
}

/// The `parade` line: the tick the counter filled (or `-`), then the
/// harvested comet apparition (or `-`). The second token is absent in
/// earlier `STV4` saves and defaults to none, so those still load.
fn parse_parade(reader: &mut Reader<'_>) -> Result<(Option<u64>, Option<u64>), SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("parade") {
        return Err(reader.err());
    }
    let opt = |reader: &Reader<'_>, token: Option<&str>| match token {
        Some("-") | None => Ok(None),
        Some(token) => token.parse().map(Some).map_err(|_| reader.err()),
    };
    let parade_at = match tokens.next() {
        None => return Err(reader.err()),
        token => opt(reader, token)?,
    };
    let comet_visit = opt(reader, tokens.next())?;
    Ok((parade_at, comet_visit))
}

/// The `familiar` line: one hex kind bitmask per POI, in map order. Written
/// four digits wide for the old 16-kind masks; reads any width, so saves
/// from before the fixture kinds (bits 16..) still load.
fn parse_familiar(reader: &mut Reader<'_>) -> Result<[u32; POI_COUNT], SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("familiar") {
        return Err(reader.err());
    }
    let mut familiar = [0_u32; POI_COUNT];
    for mask in &mut familiar {
        let token = tokens.next().ok_or_else(|| reader.err())?;
        *mask = u32::from_str_radix(token, 16).map_err(|_| reader.err())?;
    }
    Ok(familiar)
}

/// The `eager` line: the eased dial's bits plus the visit's patience.
fn parse_eager(reader: &mut Reader<'_>) -> Result<(f32, u8), SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("eager") {
        return Err(reader.err());
    }
    let bits = tokens
        .next()
        .and_then(|t| u32::from_str_radix(t, 16).ok())
        .ok_or_else(|| reader.err())?;
    let patience: u8 = reader.token(tokens.next())?;
    if patience > barter::PATIENCE {
        return Err(reader.err());
    }
    Ok((f32::from_bits(bits), patience))
}

/// The `visits` line: one count per POI, in map order.
fn parse_visits(reader: &mut Reader<'_>) -> Result<[u32; POI_COUNT], SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("visits") {
        return Err(reader.err());
    }
    let mut visits = [0_u32; POI_COUNT];
    for visit in &mut visits {
        *visit = reader.token(tokens.next())?;
    }
    Ok(visits)
}

/// The `ship` line, with the selected destination as its last token. The
/// sim's tick is needed to rebuild positions: the sky is a function of time.
fn parse_ship(reader: &mut Reader<'_>, tick: u64) -> Result<(Ship, u64), SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("ship") {
        return Err(reader.err());
    }
    let (pos, state) = match tokens.next() {
        Some("docked") => {
            let at = reader.poi(tokens.next())?;
            (super::map::poi_pos(at, tick), ShipState::Docked(at))
        }
        Some("travel") => {
            let from = reader.poi(tokens.next())?;
            let to = reader.poi(tokens.next())?;
            let progress: u64 = reader.token(tokens.next())?;
            let leg_ticks: u64 = reader.token(tokens.next())?;
            if leg_ticks == 0 || progress > leg_ticks {
                return Err(reader.err());
            }
            (
                super::map::travel_pos(from, to, progress, leg_ticks, tick),
                ShipState::Traveling {
                    from,
                    to,
                    progress,
                    leg_ticks,
                },
            )
        }
        _ => return Err(reader.err()),
    };
    let selected = reader.opt_poi(tokens.next())?;
    // The banked burner rides at the line's tail; absent in saves older
    // than `STV7`, and an unlit fire either way.
    let stoke = match tokens.next() {
        Some(token) => reader.token(Some(token))?,
        None => 0,
    };
    Ok((
        Ship {
            pos,
            prev_pos: pos,
            state,
            selected,
        },
        stoke,
    ))
}

/// The `omen` line: jump schedule, phase, eased floats.
fn parse_omen(reader: &mut Reader<'_>) -> Result<Omen, SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("omen") {
        return Err(reader.err());
    }
    let jump_at = reader.opt_token(tokens.next())?;
    let phase_name = tokens.next();
    let elapsed: u32 = reader.token(tokens.next())?;
    let phase = match phase_name {
        Some("idle") => Phase::Idle,
        Some("omen") => Phase::Omen { elapsed },
        Some("wake") => Phase::Wake { elapsed },
        _ => return Err(reader.err()),
    };
    let light = f32::from_bits(reader.hex32(tokens.next())?);
    let swell = f32::from_bits(reader.hex32(tokens.next())?);
    Ok(Omen {
        jump_at,
        phase,
        light,
        swell,
    })
}

/// The `rat` line: `-` for no stowaway, else its cell, the cell it last
/// hopped from, the hop tick, both schedules, and the chase count — all
/// bounds-checked so a hostile save cannot smuggle a rat off the grid or
/// past the chase limit.
fn parse_rat(reader: &mut Reader<'_>) -> Result<Rats, SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("rat") {
        return Err(reader.err());
    }
    match tokens.next() {
        Some("-") => Ok(Rats { rat: None }),
        first => {
            let x: u8 = reader.token(first)?;
            let y: u8 = reader.token(tokens.next())?;
            let px: u8 = reader.token(tokens.next())?;
            let py: u8 = reader.token(tokens.next())?;
            if x >= GRID_COLS || y >= GRID_ROWS || px >= GRID_COLS || py >= GRID_ROWS {
                return Err(reader.err());
            }
            let moved_at = reader.token(tokens.next())?;
            let next_move = reader.token(tokens.next())?;
            let next_nibble = reader.token(tokens.next())?;
            let chases: u8 = reader.token(tokens.next())?;
            if chases >= CHASE_LIMIT {
                return Err(reader.err());
            }
            Ok(Rats {
                rat: Some(Rat {
                    cell: (x, y),
                    prev_cell: (px, py),
                    moved_at,
                    next_move,
                    next_nibble,
                    chases,
                }),
            })
        }
    }
}

/// The `piece` lines, terminated by the `next_piece` line.
fn parse_pieces(reader: &mut Reader<'_>, legacy: bool) -> Result<(Vec<Piece>, u32), SaveError> {
    let mut pieces = Vec::new();
    loop {
        let line = reader.next_line()?;
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("piece") => {
                let id = reader.token(tokens.next())?;
                let kind_index: usize = reader.token(tokens.next())?;
                let kind = *Kind::ALL.get(kind_index).ok_or_else(|| reader.err())?;
                let variant = reader.token(tokens.next())?;
                let gnawed = match tokens.next() {
                    Some("0") => false,
                    Some("1") => true,
                    _ => return Err(reader.err()),
                };
                let loc = parse_loc(reader, &mut tokens, kind)?;
                pieces.push(Piece {
                    id,
                    kind,
                    variant,
                    gnawed,
                    loc,
                });
            }
            Some("next_piece") => {
                let next_piece = reader.token(tokens.next())?;
                if legacy {
                    migrate_console_grid(reader, &mut pieces)?;
                }
                validate_stows(reader, &pieces)?;
                return Ok((pieces, next_piece));
            }
            _ => return Err(reader.err()),
        }
    }
}

/// Carry a pre-STV8 board onto the room net. The old 6×4 hold embeds at
/// (+3, 0) — its wall band is the aft chart's rows, its deck strip the
/// floor's aft-most row — so every grid berth translates; then whatever
/// the room-grid rules no longer accept where it stands (the bas-relief
/// couch straddled the fold; the old "wall" was the side columns)
/// re-berths at its first legal cell, coverings included. A piece that
/// fits nowhere fails the load whole rather than vanishing quietly —
/// conservation before convenience.
fn migrate_console_grid(reader: &Reader<'_>, pieces: &mut [Piece]) -> Result<(), SaveError> {
    for piece in pieces.iter_mut() {
        if let Loc::Hold { x, .. } | Loc::Laid { x, .. } = &mut piece.loc {
            *x += 3;
        }
    }
    for at in 0..pieces.len() {
        let (id, kind) = (pieces[at].id, pieces[at].kind);
        match pieces[at].loc {
            Loc::Hold { x, y } => {
                if cargo::placement_check(pieces, id, kind, x, y).is_err() {
                    let (nx, ny) =
                        cargo::first_fit(pieces, id, kind).ok_or_else(|| reader.err())?;
                    pieces[at].loc = Loc::Hold { x: nx, y: ny };
                }
            }
            Loc::Laid { x, y } => {
                // Check against the other dressings only: a rug pinned
                // under standing cargo is a legal state and must not be
                // shooed out from under its couch by the move.
                let laid_only: Vec<Piece> = pieces
                    .iter()
                    .filter(|other| matches!(other.loc, Loc::Laid { .. }))
                    .copied()
                    .collect();
                if cargo::dressing_check(&laid_only, id, kind, x, y).is_err() {
                    let (nx, ny) =
                        cargo::dress_fit(pieces, id, kind).ok_or_else(|| reader.err())?;
                    pieces[at].loc = Loc::Laid { x: nx, y: ny };
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Cross-piece stow and dressing validation, after the whole list is
/// read (a cubby may reference a cabinet on a later line). Everything
/// those lines could lie about is checked here, so no later indexing or
/// invariant trips: a stow's host must be a cabinet in the hold, the
/// cargo stowable, no cubby doubled; laid dressings must not overlap
/// one another.
fn validate_stows(reader: &Reader<'_>, pieces: &[Piece]) -> Result<(), SaveError> {
    let mut seen = Vec::new();
    let mut laid: Vec<Piece> = Vec::new();
    for piece in pieces {
        match piece.loc {
            Loc::Stow { cabinet, slot } => {
                let host_ok = pieces.iter().any(|host| {
                    host.id == cabinet
                        && host.kind == Kind::Cabinet
                        && matches!(host.loc, Loc::Hold { .. })
                });
                if !host_ok || !cargo::stowable(piece.kind) || seen.contains(&(cabinet, slot)) {
                    return Err(reader.err());
                }
                seen.push((cabinet, slot));
            }
            Loc::Laid { x, y } => {
                // Re-run the dressing rules against the other dressings
                // only: bounds, surface, and one-per-cell. Occupancy is
                // deliberately absent from the slice — a rug pinned
                // under a couch is a LEGAL state and saves as such.
                if cargo::dressing_check(&laid, piece.id, piece.kind, x, y).is_err() {
                    return Err(reader.err());
                }
                laid.push(*piece);
            }
            _ => {}
        }
    }
    Ok(())
}

/// A piece's location tokens, bounds-checked so later indexing never panics.
fn parse_loc<'a>(
    reader: &Reader<'_>,
    tokens: &mut impl Iterator<Item = &'a str>,
    kind: Kind,
) -> Result<Loc, SaveError> {
    match tokens.next() {
        Some("hold") => {
            let x: u8 = reader.token(tokens.next())?;
            let y: u8 = reader.token(tokens.next())?;
            let (w, h) = kind.cells();
            if x + w > GRID_COLS || y + h > GRID_ROWS {
                return Err(reader.err());
            }
            Ok(Loc::Hold { x, y })
        }
        Some(surface @ ("shelf" | "give" | "take" | "recv")) => {
            let slot: u8 = reader.token(tokens.next())?;
            if usize::from(slot) >= SHELF_SLOTS.len() {
                return Err(reader.err());
            }
            Ok(match surface {
                "shelf" => Loc::StationShelf { slot },
                "give" => Loc::GivePad { slot },
                "take" => Loc::TakePad { slot },
                _ => Loc::ReceivedShelf { slot },
            })
        }
        Some("flot") => {
            let slot: u8 = reader.token(tokens.next())?;
            if usize::from(slot) >= FLOTSAM_SLOTS.len() {
                return Err(reader.err());
            }
            Ok(Loc::Flotsam { slot })
        }
        Some("stow") => {
            let cabinet: u32 = reader.token(tokens.next())?;
            let slot: u8 = reader.token(tokens.next())?;
            if slot >= cargo::CABINET_SLOTS {
                return Err(reader.err());
            }
            // The cabinet reference is checked in `validate_stows`, once
            // the whole piece list exists.
            Ok(Loc::Stow { cabinet, slot })
        }
        Some("laid") => {
            let x: u8 = reader.token(tokens.next())?;
            let y: u8 = reader.token(tokens.next())?;
            let (w, h) = kind.cells();
            if !kind.covering() || x + w > GRID_COLS || y + h > GRID_ROWS {
                return Err(reader.err());
            }
            // Laid-laid overlap is checked in `validate_stows` with the
            // whole list; occupancy overlap is a LEGAL state (a rug
            // pinned under a couch saves and loads pinned).
            Ok(Loc::Laid { x, y })
        }
        _ => Err(reader.err()),
    }
}

/// An optional value as a token: `-` for absent.
fn opt_token<T: fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_owned(), |v| v.to_string())
}

/// Line-by-line reader that remembers where it is, so every failure can name
/// its line.
struct Reader<'a> {
    lines: std::str::Lines<'a>,
    /// 1-based number of the line most recently read; 0 before the first.
    line: usize,
}

impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            lines: s.lines(),
            line: 0,
        }
    }

    /// A parse error naming the current line.
    const fn err(&self) -> SaveError {
        SaveError::Parse { line: self.line }
    }

    fn next_line(&mut self) -> Result<&'a str, SaveError> {
        match self.lines.next() {
            Some(line) => {
                self.line += 1;
                Ok(line)
            }
            None => Err(SaveError::Parse { line: 0 }),
        }
    }

    /// One token parsed to any [`FromStr`] type.
    fn token<T: FromStr>(&self, token: Option<&str>) -> Result<T, SaveError> {
        token
            .and_then(|t| t.parse().ok())
            .ok_or(SaveError::Parse { line: self.line })
    }

    /// One token that is either `-` or a value.
    fn opt_token<T: FromStr>(&self, token: Option<&str>) -> Result<Option<T>, SaveError> {
        match token {
            Some("-") => Ok(None),
            other => self.token(other).map(Some),
        }
    }

    /// A bounds-checked POI id.
    fn poi(&self, token: Option<&str>) -> Result<PoiId, SaveError> {
        let id: PoiId = self.token(token)?;
        if usize::from(id) < POI_COUNT {
            Ok(id)
        } else {
            Err(self.err())
        }
    }

    /// A bounds-checked optional POI id (`-` for none).
    fn opt_poi(&self, token: Option<&str>) -> Result<Option<PoiId>, SaveError> {
        match token {
            Some("-") => Ok(None),
            other => self.poi(other).map(Some),
        }
    }

    /// A whole line of the form `key <value>`.
    fn kv<T: FromStr>(&mut self, key: &str) -> Result<T, SaveError> {
        let line = self.next_line()?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some(key) {
            return Err(self.err());
        }
        self.token(tokens.next())
    }

    /// A `key <16-hex-digits>` line.
    fn kv_hex64(&mut self, key: &str) -> Result<u64, SaveError> {
        let line = self.next_line()?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some(key) {
            return Err(self.err());
        }
        tokens
            .next()
            .and_then(|t| u64::from_str_radix(t, 16).ok())
            .ok_or_else(|| self.err())
    }

    /// One token of 8 hex digits, as raw bits.
    fn hex32(&self, token: Option<&str>) -> Result<u32, SaveError> {
        token
            .and_then(|t| u32::from_str_radix(t, 16).ok())
            .ok_or_else(|| self.err())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{InputFrame, TICK_DT, Vec2, layout};
    use super::*;

    #[test]
    fn errors_display_without_panicking() {
        assert_eq!(SaveError::BadMagic.to_string(), "not a Space Trucking save");
        assert_eq!(
            SaveError::Parse { line: 3 }.to_string(),
            "malformed save at line 3"
        );
        assert_eq!(
            SaveError::Parse { line: 0 }.to_string(),
            "save ends too early"
        );
    }

    /// A worked save with every section populated: docked pieces composed
    /// into a trade, then relaunched mid-leg so ship/event lines are rich —
    /// including a mid-tenure rat and a bitten piece, so the STV3 grammar
    /// is exercised in full.
    fn worked_save() -> String {
        let mut sim = Sim::new(0xFADE);
        let press = |p: Vec2| InputFrame {
            pointer: p,
            press: true,
            held: true,
            ..InputFrame::default()
        };
        sim.advance(0.0, &press(super::super::poi_pos(7, sim.tick())));
        let lever = layout::LAUNCH_LEVER;
        sim.advance(
            0.0,
            &press(Vec2::new(lever.x + lever.w / 2.0, lever.y + lever.h / 2.0)),
        );
        for _ in 0..90 {
            sim.advance(TICK_DT, &InputFrame::default());
        }
        sim.rats.rat = Some(Rat {
            cell: (4, 1),
            prev_cell: (2, 3),
            moved_at: 30,
            next_move: 700,
            next_nibble: 2800,
            chases: 1,
        });
        sim.pieces[0].gnawed = true;
        sim.save_string()
    }

    #[test]
    fn the_rat_line_and_gnaw_token_round_trip_exactly() {
        let save = worked_save();
        let sim = Sim::from_save(&save).expect("the worked save parses");
        assert_eq!(
            sim.rats.rat,
            Some(Rat {
                cell: (4, 1),
                prev_cell: (2, 3),
                moved_at: 30,
                next_move: 700,
                next_nibble: 2800,
                chases: 1,
            })
        );
        assert!(sim.pieces[0].gnawed, "the bite must survive the trip");
        assert!(!sim.pieces[1].gnawed, "and must not spread in transit");
        assert_eq!(sim.save_string(), save);
    }

    #[test]
    fn truncation_at_every_line_boundary_fails_safe() {
        let save = worked_save();
        assert!(Sim::from_save(&save).is_ok(), "the untruncated save parses");
        let lines: Vec<&str> = save.lines().collect();
        for keep in 0..lines.len() {
            let truncated = lines[..keep].join("\n");
            assert!(
                Sim::from_save(&truncated).is_err(),
                "save truncated to {keep}/{} lines parsed anyway",
                lines.len()
            );
        }
    }

    #[test]
    fn mangling_any_line_fails_safe() {
        let save = worked_save();
        let lines: Vec<&str> = save.lines().collect();
        for target in 0..lines.len() {
            for garbage in ["", "!!! ???", "piece x y z", "seed NaN", "\u{1F680}"] {
                let mangled: String = lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| if i == target { garbage } else { line })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    Sim::from_save(&mangled).is_err(),
                    "line {target} replaced by {garbage:?} parsed anyway"
                );
            }
        }
    }

    #[test]
    fn out_of_range_fields_fail_safe() {
        let save = worked_save();
        let rat_line = "rat 4 1 2 3 30 700 2800 1";
        assert!(save.contains(rat_line), "worked save must carry the rat");
        for (needle, bad) in [
            ("ship travel 6 7", "ship travel 6 12"), // POI out of range
            ("tick 90", "tick -90"),
            ("tick 90", "tick 99999999999999999999999"),
            // The rat must sit inside the grid, hop from inside the grid,
            // and stay under the chase limit.
            (rat_line, "rat 18 1 2 3 30 700 2800 1"),
            (rat_line, "rat 4 11 2 3 30 700 2800 1"),
            (rat_line, "rat 4 1 18 3 30 700 2800 1"),
            (rat_line, "rat 4 1 2 11 30 700 2800 1"),
            (rat_line, "rat 4 1 2 3 30 700 2800 3"),
            (rat_line, "rat 4 1 2 3 30 700 2800 -1"),
        ] {
            let mangled = save.replacen(needle, bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
        // Piece fields: an unknown kind, an off-grid hold cell, a bad slot,
        // an unknown surface, a gnaw token that is neither 0 nor 1.
        let docked = Sim::new(3).save_string();
        let piece_line = docked
            .lines()
            .find(|line| line.starts_with("piece"))
            .expect("a fresh save has pieces")
            .to_owned();
        for bad in [
            "piece 0 99 0 0 hold 0 0",
            "piece 0 0 0 0 hold 18 11",
            "piece 0 0 0 0 shelf 7",
            "piece 0 0 0 0 nowhere 0",
            "piece 0 0 0 2 hold 0 0",
            "piece 0 0 0 gnawed hold 0 0",
        ] {
            let mangled = docked.replacen(&piece_line, bad, 1);
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }

    /// A sim with a stocked cabinet, its cubby lines pinned: `(sim, save,
    /// cabinet id, anchor)` with a vial in cubby 0 and fluff in cubby 1.
    fn furnished() -> (Sim, String, u32, (u8, u8)) {
        let mut sim = Sim::new(3);
        let (x, y) =
            cargo::first_fit(&sim.pieces, u32::MAX, Kind::Cabinet).expect("room for a cabinet");
        let cabinet = sim.next_piece;
        for (offset, kind, loc) in [
            (0, Kind::Cabinet, Loc::Hold { x, y }),
            (1, Kind::PerfumeVial, Loc::Stow { cabinet, slot: 0 }),
            (2, Kind::Fluff, Loc::Stow { cabinet, slot: 1 }),
            (3, Kind::Rug, Loc::Laid { x: 3, y: 6 }),
            (4, Kind::LuminousPaint, Loc::Laid { x: 5, y: 0 }),
        ] {
            sim.pieces.push(Piece {
                id: cabinet + offset,
                kind,
                variant: 0,
                gnawed: false,
                loc,
            });
        }
        sim.next_piece += 5;
        let save = sim.save_string();
        (sim, save, cabinet, (x, y))
    }

    #[test]
    fn staged_flotsam_with_an_open_barter_refuses_to_load() {
        // Docked at the Guild, the barter is open; smuggle a staged
        // piece onto the rail and the exclusivity law (the rail IS the
        // shelf row) must refuse the whole save — a fixture taught us
        // what clicking "the shelf" does otherwise.
        let save = Sim::new(7).save_string();
        let forged = save.replace("next_piece", "piece 900 5 0 0 flot 0\nnext_piece");
        assert!(Sim::from_save(&forged).is_err());
        assert!(Sim::from_save(&save).is_ok());
    }

    #[test]
    fn stowed_pieces_round_trip() {
        let (sim, save, cabinet, _) = furnished();
        assert!(save.starts_with("STV8\n"), "the writer stamps STV8");
        let restored = Sim::from_save(&save).expect("furnished save parses");
        assert_eq!(restored.pieces, sim.pieces);
        assert!(
            restored
                .pieces
                .iter()
                .any(|p| p.loc == Loc::Stow { cabinet, slot: 0 }),
            "the cubby survives the trip"
        );
    }

    #[test]
    fn older_headers_still_read() {
        // Each version since STV4 added only a line form or token; a save
        // without them is a valid older document, and the retired console's
        // runs keep walking aboard. Those documents carry console-era 6×4
        // hold coordinates, so the fixture rewrites the hold berths into the
        // old grid — and the reader must migrate every one onto a valid net
        // berth. The stoke token is STV7's, so the pre-STV7 documents drop
        // it from the ship line too.
        let plain = Sim::new(9).save_string();
        let mut old_cells = [(0_u8, 0_u8), (0, 2), (2, 0), (5, 0)].into_iter();
        let legacy_board: String = plain
            .lines()
            .map(|line| {
                if line.starts_with("piece") && line.contains(" hold ") {
                    let head = line.split(" hold ").next().expect("split has a head");
                    let (x, y) = old_cells.next().expect("three starter berths");
                    format!("{head} hold {x} {y}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        assert!(old_cells.next().is_none(), "starter cargo is four pieces");
        for older in ["STV7", "STV6", "STV5", "STV4"] {
            let mut old = legacy_board.replacen("STV8", older, 1);
            if older != "STV7" {
                old = old.replacen("ship docked 6 - 0", "ship docked 6 -", 1);
            }
            assert_ne!(old, plain);
            let sim = Sim::from_save(&old).unwrap_or_else(|e| {
                panic!("{older} must stay readable: {e}");
            });
            // The migration really ran: every console-era berth landed on
            // a legal net berth (translated, or re-fitted when the room
            // grid refuses the translation).
            let held: Vec<&Piece> = sim
                .pieces
                .iter()
                .filter(|piece| matches!(piece.loc, Loc::Hold { .. }))
                .collect();
            assert_eq!(held.len(), 4, "{older}: the starter cargo walks aboard");
            for piece in held {
                let Loc::Hold { x, y } = piece.loc else {
                    unreachable!()
                };
                assert!(
                    cargo::placement_check(&sim.pieces, piece.id, piece.kind, x, y).is_ok(),
                    "{older}: {:?} migrated to an illegal berth ({x}, {y})",
                    piece.kind
                );
            }
        }
    }

    #[test]
    fn lying_stow_lines_fail_safe() {
        let (_, save, cabinet, (x, y)) = furnished();
        let vial_line = format!("piece {} 0 0 0 stow {cabinet} 0", cabinet + 1);
        assert!(save.contains(&vial_line), "vial line changed shape");
        for (needle, bad) in [
            // A cubby in a cabinet that does not exist.
            (
                vial_line.clone(),
                format!("piece {} 0 0 0 stow 4242 0", cabinet + 1),
            ),
            // A slot past the rack.
            (
                vial_line.clone(),
                format!("piece {} 0 0 0 stow {cabinet} 4", cabinet + 1),
            ),
            // Two pieces in one cubby.
            (
                format!("piece {} 13 0 0 stow {cabinet} 1", cabinet + 2),
                format!("piece {} 13 0 0 stow {cabinet} 0", cabinet + 2),
            ),
            // An unstowable kind (the couch, index 19) in a cubby.
            (
                vial_line,
                format!("piece {} 19 0 0 stow {cabinet} 0", cabinet + 1),
            ),
            // A host that is not in the hold.
            (
                format!("piece {cabinet} 21 0 0 hold {x} {y}"),
                format!("piece {cabinet} 21 0 0 give 0"),
            ),
            // A laid non-covering (the couch, index 19).
            (
                format!("piece {} 22 0 0 laid 3 6", cabinet + 3),
                format!("piece {} 19 0 0 laid 3 6", cabinet + 3),
            ),
            // A rug up the wall.
            (
                format!("piece {} 22 0 0 laid 3 6", cabinet + 3),
                format!("piece {} 22 0 0 laid 4 1", cabinet + 3),
            ),
            // Two dressings on one cell.
            (
                format!("piece {} 24 0 0 laid 5 0", cabinet + 4),
                format!("piece {} 24 0 0 laid 3 6", cabinet + 4),
            ),
            // A coat off the grid entirely.
            (
                format!("piece {} 24 0 0 laid 5 0", cabinet + 4),
                format!("piece {} 24 0 0 laid 9 9", cabinet + 4),
            ),
        ] {
            let mangled = save.replacen(&needle, &bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }
}
