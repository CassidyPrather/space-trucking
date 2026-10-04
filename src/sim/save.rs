//! Versioned, line-oriented text saves.
//!
//! The format stores only what cannot be recomputed: seed, clock, RNG state,
//! delivery tally, visit counts, ship, leg counter, both event machines (the
//! omen and the rat), the room graph as its **edge list in attach order**,
//! the interest marks, and the pieces with their gnaw marks. Carries are
//! transient — a held piece serialises at its origin, so a save mid-carry
//! drops every player's carry on load. Everything a visit derives (stock
//! rolls, wants) is rebuilt from the seed on load, and every room's pose is
//! re-derived from its mate, which keeps the format small and the
//! determinism honest: **a save cannot disagree with the lattice, because
//! it does not store the lattice.** Floats that must survive exactly (the
//! eased light and omen) travel as hex bit patterns rather than decimal.
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
use super::map::{POI_COUNT, PoiId, Ship, ShipState};
use super::rats::{CHASE_LIMIT, Rat, Rats};
use super::room::{CABIN, MAX_ROOMS, PORTS, PortId, RoomId, RoomKind, Rooms, Tile};
use super::{KIND_COUNT, MAX_CREW, Sim, barter};

/// Magic-plus-version header of every save this build writes, and the
/// only header it reads.
///
/// A save from any other version is refused as
/// [`SaveError::UnsupportedVersion`], which the frontend treats like any
/// unreadable save: the game starts a new run. Nothing here migrates,
/// because this is a prototype and a format still finding its shape is
/// cheaper to bump than to carry (`docs/DESIGN_REVIEW.md`, "An old save
/// or tape starts a new run"). Bump it whenever what a line MEANS
/// changes, not only its grammar: a save read under the wrong rules
/// loads a board the player never built.
const MAGIC: &str = "STV22";

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
    // The graph, as its edge list in attach order. Every pose is a pure
    // function of these four small integers, so the lattice is re-derived
    // on load rather than stored.
    let _ = writeln!(out, "rooms {}", sim.rooms.order().len());
    for &id in sim.rooms.order() {
        let Some(room) = sim.rooms.get(id) else {
            continue;
        };
        match room.anchor {
            None => {
                let _ = writeln!(out, "room {id} {} - - -", room.kind.token());
            }
            Some((anchor, anchor_port, port)) => {
                let _ = writeln!(
                    out,
                    "room {id} {} {anchor} {anchor_port} {port}",
                    room.kind.token()
                );
            }
        }
    }
    let _ = write!(out, "marks {}", sim.marks.len());
    for id in &sim.marks {
        let _ = write!(out, " {id}");
    }
    let _ = writeln!(out);
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
            Loc::Hold { room, x, y, turn } => {
                let _ = writeln!(out, " hold {room} {x} {y} {}", turn.0);
            }
            Loc::Laid { room, x, y, turn } => {
                let _ = writeln!(out, " laid {room} {x} {y} {}", turn.0);
            }
        }
    }
    let _ = writeln!(out, "next_piece {}", sim.next_piece);
    out
}

/// Rebuild a sim from [`serialize`] output.
pub(crate) fn parse(s: &str) -> Result<Sim, SaveError> {
    let mut reader = Reader::new(s);
    match reader.next_line() {
        Ok(MAGIC) => {}
        Ok(other) if other.starts_with("STV") => return Err(SaveError::UnsupportedVersion),
        _ => return Err(SaveError::BadMagic),
    }

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
    let rats = parse_rat(&mut reader)?;
    let rooms = parse_rooms(&mut reader)?;
    let marks = parse_marks(&mut reader)?;
    let (pieces, next_piece) = parse_pieces(&mut reader, &rooms)?;
    let marks = marks
        .into_iter()
        .filter(|id| {
            pieces.iter().any(|piece| {
                piece.id == *id
                    && matches!(piece.loc, Loc::Hold { .. })
                    && cargo::berth_tile(&rooms, piece.kind, piece.loc) == Some(Tile::Stock)
            })
        })
        .collect();

    // A counterparty is alongside exactly when its room is: the trade is
    // derived from the graph, never stored beside it.
    let barter = match ship.state {
        ShipState::Docked(at)
            if at != super::map::COMET
                && at != super::map::WANDERER
                && rooms.find(RoomKind::Trade).is_some() =>
        {
            Some(barter::open(seed, at, visits[usize::from(at)]))
        }
        _ => None,
    };
    let values = barter.as_ref().map_or([0; KIND_COUNT], |b| {
        barter::visit_values(seed, b.station, b.visit)
    });

    Ok(Sim {
        seed,
        rng: fastrand::Rng::with_seed(rng_state),
        accumulator: 0.0,
        tick,
        paused,
        warp,
        cues: Vec::new(),
        ship,
        rooms,
        pieces,
        next_piece,
        held: [None; MAX_CREW],
        marks,
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
        occupied: [CABIN; MAX_CREW],
        last_violation: None,
    })
}

/// The `rooms` block: a count, then that many `room` lines in attach
/// order. Each is replayed through the same validated attach the game
/// uses, so a document that lies about its own graph fails safe.
fn parse_rooms(reader: &mut Reader<'_>) -> Result<Rooms, SaveError> {
    let count: usize = reader.kv("rooms")?;
    if count == 0 || count > MAX_ROOMS {
        return Err(reader.err());
    }
    let mut rooms: Option<Rooms> = None;
    for _ in 0..count {
        let line = reader.next_line()?;
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("room") {
            return Err(reader.err());
        }
        let id: RoomId = reader.token(tokens.next())?;
        if usize::from(id) >= MAX_ROOMS {
            return Err(reader.err());
        }
        let kind = reader
            .token::<u8>(tokens.next())
            .ok()
            .and_then(RoomKind::from_token)
            .ok_or_else(|| reader.err())?;
        let anchor = reader.opt_token::<RoomId>(tokens.next())?;
        let anchor_port = reader.opt_token::<PortId>(tokens.next())?;
        let port = reader.opt_token::<PortId>(tokens.next())?;
        match (&mut rooms, anchor, anchor_port, port) {
            (None, None, None, None) if id == CABIN => rooms = Some(Rooms::root(kind)),
            (Some(rooms), Some(anchor), Some(anchor_port), Some(port)) => {
                if usize::from(anchor_port) >= PORTS || usize::from(port) >= PORTS {
                    return Err(reader.err());
                }
                rooms
                    .replay(id, anchor, anchor_port, kind, port)
                    .map_err(|_| reader.err())?;
            }
            _ => return Err(reader.err()),
        }
    }
    rooms.ok_or_else(|| reader.err())
}

/// The `marks` line: a count, then that many piece ids.
fn parse_marks(reader: &mut Reader<'_>) -> Result<Vec<u32>, SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("marks") {
        return Err(reader.err());
    }
    let count: usize = reader.token(tokens.next())?;
    if count > 4096 {
        return Err(reader.err());
    }
    let mut marks = Vec::with_capacity(count);
    for _ in 0..count {
        marks.push(reader.token(tokens.next())?);
    }
    marks.sort_unstable();
    marks.dedup();
    Ok(marks)
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
/// harvested comet apparition (or `-`).
fn parse_parade(reader: &mut Reader<'_>) -> Result<(Option<u64>, Option<u64>), SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("parade") {
        return Err(reader.err());
    }
    let parade_at = reader.opt_token(tokens.next())?;
    let comet_visit = reader.opt_token(tokens.next())?;
    Ok((parade_at, comet_visit))
}

/// The `familiar` line: one hex kind bitmask per POI, in map order,
/// written at least four digits wide — a mask past bit 15 simply runs
/// longer, so the reader takes any width.
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

/// The `ship` line, with the selected destination and the burner's stoke
/// as its last two tokens. The sim's tick is needed to rebuild positions:
/// the sky is a function of time.
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
    // The banked burner rides at the line's tail.
    let stoke = reader.token(tokens.next())?;
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
    let (cols, rows) = RoomKind::Cabin.grid();
    match tokens.next() {
        Some("-") => Ok(Rats { rat: None }),
        first => {
            let x: u8 = reader.token(first)?;
            let y: u8 = reader.token(tokens.next())?;
            let px: u8 = reader.token(tokens.next())?;
            let py: u8 = reader.token(tokens.next())?;
            if x >= cols || y >= rows || px >= cols || py >= rows {
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
fn parse_pieces(reader: &mut Reader<'_>, rooms: &Rooms) -> Result<(Vec<Piece>, u32), SaveError> {
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
                let loc = parse_loc(reader, &mut tokens, rooms, kind)?;
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
                return Ok((pieces, next_piece));
            }
            _ => return Err(reader.err()),
        }
    }
}

/// A piece's location tokens, bounds-checked so later indexing never
/// panics: a room, the footprint's centre in fine units, and its turn.
///
/// Each line is checked on its own, because nothing a line says is about
/// another line any more: there are no cubbies to point at a cabinet
/// further down, and no layer refuses a piece for what else is there
/// (docs/BAY.md, "Cargo stops colliding"). A laid line must pass the
/// dressing rules, which are the room's, and no standing berth may sit on
/// a threshold — a save that lies about either is refused whole.
fn parse_loc<'a>(
    reader: &Reader<'_>,
    tokens: &mut impl Iterator<Item = &'a str>,
    rooms: &Rooms,
    kind: Kind,
) -> Result<Loc, SaveError> {
    let spot = |reader: &Reader<'_>,
                tokens: &mut dyn Iterator<Item = &'a str>|
     -> Result<cargo::Spot, SaveError> {
        let room: RoomId = reader.token(tokens.next())?;
        let x: u16 = reader.token(tokens.next())?;
        let y: u16 = reader.token(tokens.next())?;
        let turn = cargo::Turn(reader.token(tokens.next())?);
        let host = rooms.kind(room).ok_or_else(|| reader.err())?;
        let (cols, rows) = host.grid();
        // Bounded before any arithmetic, so a lying document cannot
        // overflow the footprint's far edge.
        if x >= cargo::fine(cols) || y >= cargo::fine(rows) {
            return Err(reader.err());
        }
        let span = cargo::Foot::of(host, kind, x, y, turn)
            .ok_or_else(|| reader.err())?
            .aabb();
        if span.x0 < 0
            || span.y0 < 0
            || span.x1 > i32::from(cargo::fine(cols))
            || span.y1 > i32::from(cargo::fine(rows))
        {
            return Err(reader.err());
        }
        Ok(cargo::Spot { room, x, y, turn })
    };
    match tokens.next() {
        Some("hold") => {
            let berth = spot(reader, tokens)?.hold();
            if cargo::berth_tile(rooms, kind, berth).is_none_or(|tile| tile == Tile::Threshold) {
                return Err(reader.err());
            }
            Ok(berth)
        }
        Some("laid") => {
            let spot = spot(reader, tokens)?;
            if !kind.covering() || cargo::dressing_check(rooms, kind, spot).is_err() {
                return Err(reader.err());
            }
            Ok(spot.laid())
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
    use crate::sim::cargo::fine;

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

    /// A worked save with every section populated: launched mid-leg so
    /// ship/event lines are rich — including a mid-tenure rat and a
    /// bitten piece, so the whole grammar is exercised.
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

    /// The graph rides the save as its edge list, and the lattice is
    /// re-derived: identical poses, identical mates, identical order.
    #[test]
    fn the_room_graph_round_trips_through_its_edge_list() {
        let sim = Sim::new(0x120E);
        let save = sim.save_string();
        assert!(save.contains("\nrooms 3\n"), "cabin, burner, and the dock");
        let restored = Sim::from_save(&save).expect("the graph parses");
        assert_eq!(restored.rooms().order(), sim.rooms().order());
        for (id, room) in sim.rooms().iter() {
            let mirror = restored.rooms().get(id).expect("every room comes back");
            assert_eq!(mirror.pose, room.pose, "room {id} landed elsewhere");
            assert_eq!(mirror.mates, room.mates, "room {id} mated differently");
        }
        assert_eq!(restored.save_string(), save);
    }

    /// A document that lies about its own graph fails safe into a fresh
    /// run rather than constructing a ship that cannot exist.
    #[test]
    fn lying_room_lines_fail_safe() {
        let save = Sim::new(5).save_string();
        for (needle, bad) in [
            // The burner mated to a port that is already in use.
            ("room 1 1 0 1 3", "room 1 1 0 0 3"),
            // A door mated to a hatch.
            ("room 1 1 0 1 3", "room 1 1 0 1 5"),
            // An anchor that does not exist yet.
            ("room 1 1 0 1 3", "room 1 1 7 1 3"),
            // A room kind off the end of the table.
            ("room 1 1 0 1 3", "room 1 9 0 1 3"),
            // A port index off the end of the six.
            ("room 1 1 0 1 3", "room 1 1 0 9 3"),
            // Two rooms claiming the same id.
            ("room 2 2 0 0 0", "room 1 2 0 0 0"),
            // The market under the cabin's ladder, by a hatch no market
            // declares.
            ("room 2 2 0 0 0", "room 2 2 0 4 5"),
        ] {
            let mangled = save.replacen(needle, bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
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
            (rat_line, "rat 22 1 2 3 30 700 2800 1"),
            (rat_line, "rat 4 13 2 3 30 700 2800 1"),
            (rat_line, "rat 4 1 22 3 30 700 2800 1"),
            (rat_line, "rat 4 1 2 13 30 700 2800 1"),
            (rat_line, "rat 4 1 2 3 30 700 2800 3"),
            (rat_line, "rat 4 1 2 3 30 700 2800 -1"),
        ] {
            let mangled = save.replacen(needle, bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
        // Piece fields: an unknown kind, an off-net cell, a room that is
        // not attached, a doorway berth, an unknown surface, a gnaw token
        // that is neither 0 nor 1.
        let docked = Sim::new(3).save_string();
        let piece_line = docked
            .lines()
            .find(|line| line.starts_with("piece"))
            .expect("a fresh save has pieces")
            .to_owned();
        // Piece fields, too: a berth that runs off the net, a turn past a
        // whole one or below none, and a berth with no turn at all.
        for bad in [
            "piece 0 99 0 0 hold 0 1152 1152 0",
            "piece 0 0 0 0 hold 0 5632 3328 0",
            "piece 0 0 0 0 hold 9 1152 1152 0",
            "piece 0 0 0 0 hold 0 2944 896 0",
            "piece 0 0 0 0 nowhere 0",
            "piece 0 0 0 2 hold 0 1152 1152 0",
            "piece 0 0 0 gnawed hold 0 1152 1152 0",
            "piece 0 0 0 0 hold 0 1152 1152 65536",
            "piece 0 0 0 0 hold 0 1152 1152 -1",
            "piece 0 0 0 0 hold 0 1152 1152",
            "piece 0 0 0 0 hold 0 5600 1152 0",
        ] {
            let mangled = docked.replacen(&piece_line, bad, 1);
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }

    /// The berth `kind` takes in the cabin with its footprint's top-left
    /// on whole cell `(x, y)`, at the turn the game gives a body there,
    /// and that berth's tokens on a save line: room, centre, turn.
    fn tokens(kind: Kind, x: u8, y: u8) -> (cargo::Spot, String) {
        let (x, y, turn) =
            cargo::anchored(RoomKind::Cabin, kind, fine(x), fine(y)).expect("on the net");
        (
            cargo::Spot {
                room: CABIN,
                x,
                y,
                turn,
            },
            format!("{CABIN} {x} {y} {}", turn.0),
        )
    }

    /// A sim with a furnished corner — a cabinet with a vial and a fluff
    /// standing in it, a rug, and two coats on one patch of wall — and the
    /// save tokens of the cabinet's berth.
    fn furnished() -> (Sim, String, u32, String) {
        let mut sim = Sim::new(3);
        let spot = cargo::first_fit(sim.rooms(), sim.pieces(), u32::MAX, Kind::Cabinet)
            .expect("room for a cabinet");
        let cabinet = sim.next_piece;
        let coat = tokens(Kind::LuminousPaint, 5, 0).0.laid();
        for (offset, kind, loc) in [
            (0, Kind::Cabinet, spot.hold()),
            (1, Kind::PerfumeVial, spot.hold()),
            (2, Kind::Fluff, spot.hold()),
            (3, Kind::Rug, tokens(Kind::Rug, 3, 6).0.laid()),
            (4, Kind::LuminousPaint, coat),
            (5, Kind::PaintTin, coat),
        ] {
            sim.pieces.push(Piece {
                id: cabinet + offset,
                kind,
                variant: 0,
                gnawed: false,
                loc,
            });
        }
        sim.next_piece += 6;
        let save = sim.save_string();
        let at = format!("{} {} {} {}", spot.room, spot.x, spot.y, spot.turn.0);
        (sim, save, cabinet, at)
    }

    /// **Pieces sharing ground survive the trip**: three in one wardrobe's
    /// cell and two coats on one patch of wall are a board somebody built,
    /// and it comes back as they built it.
    #[test]
    fn a_board_sharing_ground_round_trips() {
        let (sim, save, cabinet, at) = furnished();
        assert!(
            save.starts_with(&format!("{MAGIC}\n")),
            "the writer stamps the current version"
        );
        assert!(save.contains(&format!("piece {} 0 0 0 hold {at}\n", cabinet + 1)));
        let restored = Sim::from_save(&save).expect("furnished save parses");
        assert_eq!(restored.pieces, sim.pieces);
    }

    /// **A save from any other version is refused, whole.** Nothing here
    /// reads an older document; the frontend starts a new run instead
    /// (`docs/DESIGN_REVIEW.md`). So this build's own save, re-dated, is
    /// refused as a version even though every line in it would parse.
    /// The defaults that let an older line run short went with the
    /// versions that wrote them: a ship line without its stoke and a
    /// parade line without its comet are malformed, not old.
    #[test]
    fn a_save_from_any_other_version_is_refused() {
        let save = Sim::new(9).save_string();
        for header in ["STV4", "STV20", "STV21", "STV23"] {
            assert_eq!(
                Sim::from_save(&save.replacen(MAGIC, header, 1)).err(),
                Some(SaveError::UnsupportedVersion),
                "{header} was read"
            );
        }
        for (needle, short) in [
            ("\nship docked 6 - 0\n", "\nship docked 6 -\n"),
            ("\nparade - -\n", "\nparade -\n"),
        ] {
            let mangled = save.replacen(needle, short, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(
                matches!(Sim::from_save(&mangled), Err(SaveError::Parse { .. })),
                "{short:?} parsed anyway"
            );
        }
    }

    /// **A berth off the grid and off square survives the trip to the
    /// unit**: the document carries the fine centre and the turn, and the
    /// reader takes them as written — a seventh of a turn comes back a
    /// seventh of a turn, not the nearest anything.
    #[test]
    fn an_odd_berth_round_trips() {
        let mut sim = Sim::new(3);
        let vial = sim
            .pieces
            .iter()
            .position(|piece| piece.kind == Kind::PerfumeVial)
            .expect("the starter vial");
        let id = sim.pieces[vial].id;
        let spot = cargo::Spot {
            room: CABIN,
            x: fine(6) + 5,
            y: fine(7) + 11,
            turn: cargo::Turn(9362),
        };
        assert!(
            super::super::placement_legal(sim.rooms(), sim.pieces(), id, Kind::PerfumeVial, spot),
            "the fixture's berth is a legal one"
        );
        sim.pieces[vial].loc = spot.hold();
        let save = sim.save_string();
        assert!(
            save.contains(&format!("piece {id} 0 "))
                && save.contains(&format!(" hold 0 {} {} 9362\n", spot.x, spot.y)),
            "the line carries the fine berth and its turn"
        );
        let restored = Sim::from_save(&save).expect("an odd board loads");
        assert_eq!(restored.pieces, sim.pieces);
        assert_eq!(restored.save_string(), save);
    }

    #[test]
    fn lying_berth_lines_fail_safe() {
        let (_, save, cabinet, at) = furnished();
        let rug = tokens(Kind::Rug, 3, 6).1;
        let coat = tokens(Kind::LuminousPaint, 5, 0).1;
        let vial_line = format!("piece {} 0 0 0 hold {at}", cabinet + 1);
        assert!(save.contains(&vial_line), "vial line changed shape");
        for (needle, bad) in [
            // A cubby, which no cabinet has any more: the line is not a
            // berth at all.
            (
                vial_line.clone(),
                format!("piece {} 0 0 0 stow {cabinet} 0", cabinet + 1),
            ),
            // A standing berth on a doorway.
            (
                vial_line,
                format!(
                    "piece {} 0 0 0 hold {CABIN} {} {} 0",
                    cabinet + 1,
                    fine(11) + 128,
                    fine(3) + 128
                ),
            ),
            // A laid non-covering (the couch, index 19).
            (
                format!("piece {} 22 0 0 laid {rug}", cabinet + 3),
                format!("piece {} 19 0 0 laid {rug}", cabinet + 3),
            ),
            // A rug up the wall.
            (
                format!("piece {} 22 0 0 laid {rug}", cabinet + 3),
                format!(
                    "piece {} 22 0 0 laid {}",
                    cabinet + 3,
                    tokens(Kind::Rug, 5, 1).1
                ),
            ),
            // A coat off the grid entirely: no chart under its centre.
            (
                format!("piece {} 24 0 0 laid {coat}", cabinet + 4),
                format!("piece {} 24 0 0 laid 0 5504 3200 0", cabinet + 4),
            ),
        ] {
            let mangled = save.replacen(&needle, &bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }
}
