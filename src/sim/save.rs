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
use super::room::{CABIN, COURSES, MAX_ROOMS, PORTS, PortId, RoomId, RoomKind, Rooms, Surf, Tile};
use super::{KIND_COUNT, KNOWN_ALL, MAX_CREW, Sim, barter};

/// Magic-plus-version header of every save this build writes.
///
/// `STV16` is the calling rooms' growth (docs/ROOMS.md): three of the
/// four grew so a station's own furniture and the crew's staged cargo
/// would both fit on one deck — a market to 8×7, a parlor to 5×4, a pump
/// bay to 4×3, with the pump's counter moving into its own starboard
/// corner. Nothing about the GRAMMAR moved
/// — a berth is still a room and two cells — but the cell those two
/// numbers name is on a wider net now, so the starboard and ceiling
/// charts start further along and the front wall starts further down.
///
/// `STV15` is the fixture law (docs/ROOMS.md): a room's own hardware
/// stands in cells, so the deck its handshake is worked over and the
/// patch of ceiling its pendant hangs from stopped being berths and
/// became `Fixture`. Nothing about the GRAMMAR moved — a berth is still
/// a room and two cells — but a document written before it may stand
/// cargo inside the counter or the lamp, which is the clip this build
/// exists to refuse.
///
/// `STV14` is the entry-path law (docs/ROOMS.md): an `Offer` band may
/// not fall in a declared door's own lane, so the cells a body crosses
/// walking into a station are ordinary deck and the chalk sits either
/// side of them. Nothing about the GRAMMAR moved — a berth is still a
/// room and two cells — but some of those cells stopped being a
/// proposal and started being ordinary floor, which is a change in what
/// a berth MEANS, and a document written before it is mid-trade in a
/// shape this build would read as "nothing offered".
///
/// `STV13` is the window family (docs/BAY.md): the cargo table grew a
/// porthole and a bay window on its end, so the discovery ledger's masks
/// grew two bits. Nothing about the GRAMMAR moved — appended kinds keep
/// every old token — but a document written before the family existed
/// cannot say whether a station that knew "everything" knew about glass,
/// and guessing wrong either way is a lie about what the crew has seen.
/// The migration below answers it the one honest way (see [`READABLE`]).
///
/// `STV12` is the port law's second reading (docs/ROOMS.md): a room kind
/// declares only the ports it needs, so the incinerator, the market, the
/// hold, the parlor and the pump bay carry one door apiece and the cabin
/// alone keeps the full complement. The slot numbering did not move — a
/// door is still its wall's index, the ladder still 4, the hatch still
/// 5 — so the edge list's grammar is untouched; what changed is that a
/// document can name a slot its kind no longer fills, which is why the
/// migration below re-seats rather than refuses.
///
/// `STV11` is the rooms slice (docs/ROOMS.md): the ship became a graph of
/// rooms, so the document grew a `rooms` block (the edge list in attach
/// order — poses are re-derived, never stored) and a `marks` line, every
/// berth gained its room qualifier, and the whole barter counter — pads,
/// shelves, the eased dial, patience — left the format with the interface
/// it belonged to. `STV10` widened the cabin from a 6×5 floor to an 8×7
/// one; `STV7` added the banked burner's stoke to the ship line's tail;
/// `STV6` added the `laid` piece location; `STV5` added `stow`.
const MAGIC: &str = "STV16";

/// Older headers this build still reads, each with its own migration,
/// applied oldest-first so a `STV4` document walks the whole chain.
///
/// `STV16` grew three of the calling rooms. A pre-STV16 berth in one of
/// them names a cell of a narrower net, so it is **translated per chart**
/// exactly as the cabin's own widening was: the aft wall, the port wall
/// and the floor held still, the starboard wall and the ceiling start
/// one or two columns further along, and the front wall as many rows
/// further down. Whatever the arbiter no longer accepts once that has
/// run walks to the first berth its own room still has. Conservation
/// before convenience: a piece comes out standing somewhere else, never
/// gone.
///
/// `STV15` gave a room's own hardware its cells. A pre-STV15 document
/// may stand cargo on the counter's deck or under the pendant, and both
/// are berths this build refuses — so those pieces WALK, to the first
/// berth of their own room the arbiter still accepts. Conservation
/// before convenience: a piece comes out standing somewhere else, never
/// gone. If its room has no berth left it stays where it is, which is
/// where the player left it.
///
/// `STV14` moved the offer areas out of the doorways. A pre-STV14
/// document may hold a proposal on cells that are plain deck now, and a
/// proposal that quietly became loose cargo is a trade the room would
/// stop answering — so those pieces WALK, onto the same room's offer
/// area, first free berth. Conservation before convenience again: the
/// proposal comes out standing somewhere else, never withdrawn. If the
/// room has no berth left for one it stays where it is, which is a legal
/// berth and the player's either way.
///
/// `STV13` grew the window family. A pre-STV13 document's ledger masks
/// are two bits short, and the reader widens exactly the ones that were
/// FULL when they were written: a station a crew had seen every kind at
/// is a station they have seen everything at, and the Guild — home turf,
/// which boots knowing all of it — stays home turf. A partial mask keeps
/// its bits and simply does not know about glass yet, which is true: a
/// crew that has never been offered a porthole has never been offered
/// one. Berth tokens, piece tokens, and the barter columns are all
/// append-safe by construction, so nothing else in the document moves.
///
/// `STV12` spared the leaf rooms their unused ports. A pre-STV12 document
/// may therefore hang a room off a slot that is no longer there — a pump
/// bay under the cabin's ladder, a market entered through its front wall
/// — and such a room is **re-seated** through the spawn walk, keeping its
/// id and everything berthed in it. Conservation before convenience: the
/// ship comes out a different shape, never a shorter one.
///
/// `STV11` put the cargo into rooms. A pre-STV11 document knows one room,
/// so its berths become the cabin's; its mid-trade state resolves on load
/// (every piece of the player's on a pad or the received shelf walks back
/// aboard to its first legal berth, and the station's own stock is dropped,
/// because the station it belonged to no longer exists as a place); and
/// its staged flotsam lands in the incinerator room's own grid, which is
/// what the hopper is now. `STV10` widened the cabin: a pre-STV10 berth is
/// translated per chart (aft, port, and floor held still; starboard and the
/// ceiling slid +2 columns, the front wall +2 rows). `STV9` made the ship's
/// instruments cargo: older saves carry none, so the reader hangs the
/// missing ones at their traditional berths. `STV8` moved cargo onto the
/// room net: berth coordinates from pre-STV8 headers are console-era 6×4
/// hold cells, which the net embeds at (+3, 0). Whatever the room-grid
/// rules no longer accept once the translations have run re-berths at its
/// first legal cell. Everything else stays one additive grammar.
const READABLE: [&str; 13] = [
    MAGIC, "STV15", "STV14", "STV13", "STV12", "STV11", "STV10", "STV9", "STV8", "STV7", "STV6",
    "STV5", "STV4",
];

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
            Loc::Hold { room, x, y } => {
                let _ = writeln!(out, " hold {room} {x} {y}");
            }
            Loc::Stow { cabinet, slot } => {
                let _ = writeln!(out, " stow {cabinet} {slot}");
            }
            Loc::Laid { room, x, y } => {
                let _ = writeln!(out, " laid {room} {x} {y}");
            }
        }
    }
    let _ = writeln!(out, "next_piece {}", sim.next_piece);
    out
}

/// Where a pre-rooms document said a piece was. Only the reader speaks
/// this dialect; every one of these resolves into a room berth on load.
#[derive(Clone, Copy, Debug)]
enum Berth {
    /// A modern, room-qualified berth.
    Settled(Loc),
    /// A one-room net cell (occupancy).
    Cell(u8, u8),
    /// A one-room net cell (dressing).
    Dress(u8, u8),
    /// The station's own goods: shelf or take pad. The station it
    /// belonged to no longer exists as a place, so these are dropped.
    Theirs,
    /// The player's, mid-trade: a give pad or the received shelf. These
    /// walk back aboard — conservation before convenience.
    Ours,
    /// Staged on the outboard rail: fuel, which lands in the incinerator
    /// room's grid, because that is what the hopper is now.
    Fuel,
}

/// Rebuild a sim from [`serialize`] output.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse(s: &str) -> Result<Sim, SaveError> {
    let mut reader = Reader::new(s);
    let header = match reader.next_line() {
        Ok(header) if READABLE.contains(&header) => header,
        Ok(other) if other.starts_with("STV") => return Err(SaveError::UnsupportedVersion),
        _ => return Err(SaveError::BadMagic),
    };
    // Pre-STV8 headers carry console-era 6×4 hold coordinates that the
    // room net embeds at (+3, 0); their berths migrate after reading.
    let legacy = !matches!(
        header,
        MAGIC | "STV15" | "STV14" | "STV13" | "STV12" | "STV11" | "STV10" | "STV9" | "STV8"
    );
    // Pre-STV9 headers predate the instruments being cargo; the missing
    // ones are hung at their traditional berths after reading.
    let uninstrumented = !matches!(
        header,
        MAGIC | "STV15" | "STV14" | "STV13" | "STV12" | "STV11" | "STV10" | "STV9"
    );
    // Pre-STV10 headers carry narrow-net coordinates: the cabin was a
    // 6×5 floor then, and the charts past the growth have moved.
    let narrow = !matches!(
        header,
        MAGIC | "STV15" | "STV14" | "STV13" | "STV12" | "STV11" | "STV10"
    );
    // Pre-STV11 headers know one room and a barter counter.
    let roomless = !matches!(
        header,
        MAGIC | "STV15" | "STV14" | "STV13" | "STV12" | "STV11"
    );
    // Pre-STV12 headers were written when every room declared six ports;
    // an edge through a slot its kind no longer fills is re-seated.
    let six_ported = !matches!(header, MAGIC | "STV15" | "STV14" | "STV13" | "STV12");
    // Pre-STV13 headers were written before the window family, so their
    // ledger masks are narrower than the table is now.
    let unglazed = !matches!(header, MAGIC | "STV15" | "STV14" | "STV13");
    // Pre-STV14 headers were written when an offer band ran clean across
    // the room, doorway and all; a proposal left on what is deck now
    // walks onto the offer area this build actually has.
    let doorway_offers = !matches!(header, MAGIC | "STV15" | "STV14");
    // Pre-STV15 headers were written when a room's own hardware stood in
    // cells anybody could berth in; cargo left inside the counter or the
    // pendant walks off them.
    let open_fixtures = !matches!(header, MAGIC | "STV15");
    // Pre-STV16 headers were written when three of the calling rooms
    // were narrower, so a berth in one names a cell of a smaller net.
    let cramped_callers = header != MAGIC;

    let seed = reader.kv("seed")?;
    let tick = reader.kv("tick")?;
    let rng_state = reader.kv_hex64("rng")?;
    let warp = reader.kv::<u8>("warp")? != 0;
    let paused = reader.kv::<u8>("paused")? != 0;
    let deliveries = reader.kv("deliveries")?;
    let karma = reader.kv("karma")?;
    let mut familiar = parse_familiar(&mut reader)?;
    if unglazed {
        widen_familiar(&mut familiar);
    }
    let visits = parse_visits(&mut reader)?;
    let (ship, stoke) = parse_ship(&mut reader, tick)?;
    let legs = reader.kv("legs")?;
    let omen = parse_omen(&mut reader)?;
    let encounters = parse_encounter(&mut reader)?;
    let drones = parse_drone(&mut reader)?;
    let (parade_at, comet_visit) = parse_parade(&mut reader)?;
    let mut rats = parse_rat(&mut reader)?;
    // The stowaway walks the same chain its floor does: the console-era
    // embed first, then the widening. Both are chart-preserving, so a
    // rat that was on the deck is still on the deck.
    if let Some(rat) = &mut rats.rat {
        if legacy {
            rat.cell.0 += 3;
            rat.prev_cell.0 += 3;
        }
        if narrow {
            rat.cell = widen(rat.cell.0, rat.cell.1);
            rat.prev_cell = widen(rat.prev_cell.0, rat.prev_cell.1);
        }
    }

    let (rooms, marks) = if roomless {
        // A pre-rooms document knows exactly one room. The eased dial and
        // the visit's patience were interface, and leave with it.
        parse_eager(&mut reader)?;
        (Rooms::new(), Vec::new())
    } else {
        let rooms = parse_rooms(&mut reader, six_ported)?;
        let marks = parse_marks(&mut reader)?;
        (rooms, marks)
    };

    let (mut pieces, berths, mut next_piece) = parse_pieces(&mut reader, &rooms, roomless)?;
    if roomless {
        resettle(&reader, &rooms, &mut pieces, &berths, legacy, narrow)?;
    }
    if uninstrumented {
        inject_instruments(&reader, &rooms, &mut pieces, &mut next_piece)?;
    }
    validate_stows(&reader, &rooms, &pieces)?;
    // Coordinates before classes: a translation puts a berth on the
    // chart the document meant, and only then can a rule about what a
    // cell MEANS be asked of it.
    if cramped_callers {
        regrow_the_calling_rooms(&rooms, &mut pieces);
    }
    if doorway_offers {
        walk_proposals_off_the_doorway(&rooms, &mut pieces);
    }
    if open_fixtures {
        walk_cargo_out_from_under_the_fixtures(&rooms, &mut pieces);
    }
    let marks = marks
        .into_iter()
        .filter(|id| {
            pieces.iter().any(|piece| {
                piece.id == *id
                    && matches!(piece.loc, Loc::Hold { room, x, y }
                        if rooms.tile(room, x, y) == Some(Tile::Stock))
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

    let mut sim = Sim {
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
    };
    if roomless {
        // A pre-rooms document knew a counterparty as a panel, not a
        // place. Docked at a trading POI, the dock brings its room
        // alongside on load and this visit's goods go out as they would
        // have at the arrival that never wrote them down.
        if let ShipState::Docked(at) = sim.ship.state {
            if at != super::map::COMET && at != super::map::WANDERER {
                let visit = sim.visits[usize::from(at)].max(1);
                sim.open_trade(at, visit);
            }
        }
        sim.cues.clear();
    }
    Ok(sim)
}

/// The `rooms` block: a count, then that many `room` lines in attach
/// order. Each is replayed through the same validated attach the game
/// uses, so a document that lies about its own graph fails safe.
///
/// `six_ported` is the pre-STV12 migration: a document from when every
/// kind declared all six slots may name one its kind has since given up,
/// and rather than lose the room (and everything berthed in it) the
/// reader re-seats it through the spawn walk. Only old documents get
/// that mercy — a current save that disagrees with the lattice is a save
/// that lies, and it fails safe as it always has.
fn parse_rooms(reader: &mut Reader<'_>, six_ported: bool) -> Result<Rooms, SaveError> {
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
                let replayed = rooms.replay(id, anchor, anchor_port, kind, port);
                if replayed.is_err() && six_ported {
                    rooms.reseat(id, kind, anchor).map_err(|_| reader.err())?;
                } else {
                    replayed.map_err(|_| reader.err())?;
                }
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

/// The STV13 migration: a pre-window-family ledger, brought up to the
/// table's current width.
///
/// A mask that was FULL for its era becomes full for this one — a crew
/// who had seen every kind a station traded has seen everything it
/// trades, and the Guild boots that way by definition. A partial mask is
/// left alone: not knowing about glass yet is the truth about a crew
/// nobody has offered any.
fn widen_familiar(familiar: &mut [u32; POI_COUNT]) {
    // What "all of them" meant when the document was written: every bit
    // below the family, which appended on the table's end.
    let was_all = KNOWN_ALL & !window_mask();
    for mask in familiar.iter_mut() {
        if *mask == was_all {
            *mask = KNOWN_ALL;
        }
    }
}

/// The window family's own bits in a ledger mask.
fn window_mask() -> u32 {
    Kind::ALL
        .iter()
        .filter(|kind| kind.window())
        .fold(0, |mask, kind| mask | (1 << kind.index()))
}

/// The retired `eager` line: the eased dial's bits plus the visit's
/// patience. Read and discarded — both were interface, and the interface
/// is gone.
fn parse_eager(reader: &mut Reader<'_>) -> Result<(), SaveError> {
    let line = reader.next_line()?;
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("eager") {
        return Err(reader.err());
    }
    if tokens
        .next()
        .and_then(|t| u32::from_str_radix(t, 16).ok())
        .is_none()
    {
        return Err(reader.err());
    }
    let _: u8 = reader.token(tokens.next())?;
    Ok(())
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

/// The `piece` lines, terminated by the `next_piece` line. Pre-rooms
/// documents park their berths in [`Berth`] and [`resettle`] answers for
/// them; modern ones land settled.
fn parse_pieces(
    reader: &mut Reader<'_>,
    rooms: &Rooms,
    roomless: bool,
) -> Result<(Vec<Piece>, Vec<Berth>, u32), SaveError> {
    let mut pieces = Vec::new();
    let mut berths = Vec::new();
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
                let berth = parse_loc(reader, &mut tokens, rooms, kind, roomless)?;
                berths.push(berth);
                pieces.push(Piece {
                    id,
                    kind,
                    variant,
                    gnawed,
                    // Pre-rooms dialect berths carry no room; `resettle`
                    // answers for every one of them before anyone looks.
                    loc: match berth {
                        Berth::Settled(loc) => loc,
                        Berth::Cell(x, y) => Loc::Hold { room: CABIN, x, y },
                        Berth::Dress(x, y) => Loc::Laid { room: CABIN, x, y },
                        _ => Loc::Stow {
                            cabinet: u32::MAX,
                            slot: 0,
                        },
                    },
                });
            }
            Some("next_piece") => {
                let next_piece = reader.token(tokens.next())?;
                return Ok((pieces, berths, next_piece));
            }
            _ => return Err(reader.err()),
        }
    }
}

/// A narrow-net cell's berth on the widened net — the STV10 half of
/// the chain, and a pure per-chart translation.
///
/// The cabin grew starboard and forward from a fixed aft-port corner,
/// so the charts that corner anchors — aft, port, and the floor
/// itself — kept every coordinate they had. What lay beyond the growth
/// simply moved: the starboard wall slid two columns out, the ceiling
/// folded past its cornice with it, and the front wall slid two rows
/// down the net. A cell that was on no old chart is left where it is;
/// the re-fit is the one that answers for it.
const fn widen(x: u8, y: u8) -> (u8, u8) {
    match (x, y) {
        (9..=17, 3..=7) => (x + 2, y),
        (3..=8, 8..=10) => (x, y + 2),
        _ => (x, y),
    }
}

/// Carry a pre-rooms board into the room graph, oldest migration first.
///
/// A pre-STV8 document's 6×4 console hold embeds at (+3, 0); every
/// pre-STV10 berth [`widen`]s onto the 8×7 cabin. Both steps are plain
/// translations, which is the whole point of the format: a save is
/// re-read, never re-solved. Then the mid-trade state resolves — the
/// station's own goods are dropped, the player's walk back aboard, and
/// staged fuel lands in the incinerator's grid — and whatever the room
/// rules no longer accept where it stands (a berth the threshold rule
/// now keeps clear, a bas-relief couch across a fold) re-berths at its
/// first legal cell. A piece that fits nowhere fails the load whole
/// rather than vanishing quietly: conservation before convenience.
fn resettle(
    reader: &Reader<'_>,
    rooms: &Rooms,
    pieces: &mut Vec<Piece>,
    berths: &[Berth],
    legacy: bool,
    narrow: bool,
) -> Result<(), SaveError> {
    // The station's own stock has nowhere to be: the station it belonged
    // to no longer exists as a place.
    let mut order: Vec<(usize, Piece, Berth)> = pieces
        .iter()
        .copied()
        .zip(berths.iter().copied())
        .enumerate()
        .filter(|(_, (_, berth))| !matches!(berth, Berth::Theirs))
        .map(|(at, (piece, berth))| (at, piece, berth))
        .collect();
    for (_, piece, _) in &mut order {
        if let Loc::Hold { x, y, .. } | Loc::Laid { x, y, .. } = &mut piece.loc {
            if legacy {
                *x += 3;
            }
            if narrow {
                (*x, *y) = widen(*x, *y);
            }
        }
    }
    // Settled berths go down first, so the pieces that must find a home
    // are fitted around a board that already exists.
    let mut board: Vec<Piece> = order
        .iter()
        .filter(|(_, _, berth)| !matches!(berth, Berth::Ours | Berth::Fuel))
        .map(|(_, piece, _)| *piece)
        .collect();
    // Whatever the translations left illegal re-berths at its first legal
    // cell — a berth the threshold rule now keeps clear, a bas-relief
    // couch across a fold.
    for at in 0..board.len() {
        let (id, kind) = (board[at].id, board[at].kind);
        match board[at].loc {
            Loc::Hold { room, x, y } => {
                if cargo::placement_check(rooms, &board, id, kind, room, x, y).is_err() {
                    board[at].loc = berth_aboard(reader, rooms, &board, id, kind)?;
                }
            }
            Loc::Laid { room, x, y } => {
                // Check against the other dressings only: a rug pinned
                // under standing cargo is a legal state and must not be
                // shooed out from under its couch by the move.
                let laid_only: Vec<Piece> = board
                    .iter()
                    .filter(|other| matches!(other.loc, Loc::Laid { .. }))
                    .copied()
                    .collect();
                if cargo::dressing_check(rooms, &laid_only, id, kind, room, x, y).is_err() {
                    let (room, x, y) =
                        cargo::dress_fit(rooms, &board, id, kind).ok_or_else(|| reader.err())?;
                    board[at].loc = Loc::Laid { room, x, y };
                }
            }
            Loc::Stow { .. } => {}
        }
    }
    // Fuel first, so the hopper's own tiles go to what was on the rail
    // rather than to whatever walks in from the pads.
    let burner = rooms.find(RoomKind::Burner);
    for pass in [Berth::Fuel, Berth::Ours] {
        for (_, piece, berth) in &mut order {
            if std::mem::discriminant(berth) != std::mem::discriminant(&pass) {
                continue;
            }
            let (id, kind) = (piece.id, piece.kind);
            let hopper = matches!(pass, Berth::Fuel)
                .then(|| {
                    burner.and_then(|room| {
                        barter::tiles_of(rooms, room, Tile::Consume)
                            .into_iter()
                            .find(|&(x, y)| {
                                cargo::placement_legal(rooms, &board, id, kind, room, x, y)
                            })
                            .map(|(x, y)| Loc::Hold { room, x, y })
                    })
                })
                .flatten();
            piece.loc = match hopper {
                Some(loc) => loc,
                None => berth_aboard(reader, rooms, &board, id, kind)?,
            };
            board.push(*piece);
        }
    }
    // Back into save order, so the document round-trips as it was read.
    let settled: Vec<Piece> = order
        .iter()
        .map(|(at, piece, berth)| {
            let resolved = if matches!(berth, Berth::Ours | Berth::Fuel) {
                *piece
            } else {
                board
                    .iter()
                    .find(|other| other.id == piece.id)
                    .copied()
                    .unwrap_or(*piece)
            };
            (*at, resolved)
        })
        .map(|(_, piece)| piece)
        .collect();
    *pieces = settled;
    Ok(())
}

/// The first legal berth aboard for the piece at `at`, or a refusal.
fn berth_aboard(
    reader: &Reader<'_>,
    rooms: &Rooms,
    pieces: &[Piece],
    id: u32,
    kind: Kind,
) -> Result<Loc, SaveError> {
    if kind.covering() {
        let (room, x, y) = cargo::dress_fit(rooms, pieces, id, kind).ok_or_else(|| reader.err())?;
        return Ok(Loc::Laid { room, x, y });
    }
    let (room, x, y) = cargo::first_fit(rooms, pieces, id, kind).ok_or_else(|| reader.err())?;
    Ok(Loc::Hold { room, x, y })
}

/// The STV14 migration: a proposal left standing in a doorway's lane.
///
/// Before the entry-path law an offer band ran clean across the room,
/// the door's own lane included, so a mid-trade document can carry a
/// proposal on cells this build calls plain deck. Left alone those
/// pieces are still the player's and still legal — but the room has
/// stopped being offered them, which is a trade quietly withdrawn while
/// nobody was looking. So they walk: same room, first free berth on its
/// offer area, in the document's own piece order so the walk is as
/// deterministic as everything else here. A room with no berth left for
/// one leaves it where it stands, because a legal berth of the player's
/// is never worse than the alternative.
fn walk_proposals_off_the_doorway(rooms: &Rooms, pieces: &mut [Piece]) {
    // The offer band as it stood before the law: right across the room's
    // front, doorway and all. Written out here rather than reached for,
    // because a migration is the one place a retired rule still has to
    // be said out loud — and because the cells that changed are exactly
    // the ones this covers and the living derivation no longer chalks.
    let retired = |kind: RoomKind, x: u8, y: u8| {
        if !matches!(kind, RoomKind::Trade | RoomKind::Parlor) {
            return false;
        }
        let Some(surf) = kind.surface_of(x, y) else {
            return false;
        };
        let (_, depth) = kind.floor();
        (matches!(surf, Surf::Front)
            || matches!(surf, Surf::Floor | Surf::Ceiling) && y + 2 >= COURSES + depth)
            && kind.tile_of(x, y) != Some(Tile::Offer)
    };
    let stranded: Vec<usize> = pieces
        .iter()
        .enumerate()
        .filter(|(_, piece)| match piece.loc {
            Loc::Hold { room, x, y } => rooms.kind(room).is_some_and(|kind| retired(kind, x, y)),
            _ => false,
        })
        .map(|(at, _)| at)
        .collect();
    for at in stranded {
        let Loc::Hold { room, .. } = pieces[at].loc else {
            continue;
        };
        let (id, kind) = (pieces[at].id, pieces[at].kind);
        let settled: Vec<Piece> = pieces.to_vec();
        let Some((x, y)) = barter::tiles_of(rooms, room, Tile::Offer)
            .into_iter()
            .find(|&(x, y)| super::placement_legal(rooms, &settled, id, kind, room, x, y))
        else {
            continue;
        };
        pieces[at].loc = Loc::Hold { room, x, y };
    }
}

/// The STV15 migration: cargo left standing inside a room's own hardware.
///
/// Before the fixture law the deck a handshake is worked over and the
/// ceiling a pendant hangs from were ordinary berths, so a document can
/// carry a crate inside the counter or a lamp inside the lamp. This
/// build refuses those cells outright, and a piece the arbiter refuses
/// is a piece no later rule can reason about — so they walk: same room,
/// first cell of its net the arbiter still accepts, in the document's
/// own piece order so the walk is as deterministic as everything else
/// here. A room with nowhere left for one leaves it standing, because a
/// piece kept is worth more than a rule tidied.
fn walk_cargo_out_from_under_the_fixtures(rooms: &Rooms, pieces: &mut [Piece]) {
    // Asked of the arbiter rather than of the anchor cell: a two-cell
    // piece standing beside the counter reaches over it, and a piece
    // that only half stands in the fixture is still standing in it.
    let refused = |piece: &Piece| match piece.loc {
        Loc::Hold { room, x, y } => {
            super::cargo::placement_check(rooms, &[], piece.id, piece.kind, room, x, y)
                == Err(super::cargo::Violation::Fixture)
        }
        Loc::Laid { room, x, y } => {
            super::cargo::dressing_check(rooms, &[], piece.id, piece.kind, room, x, y)
                == Err(super::cargo::Violation::Fixture)
        }
        Loc::Stow { .. } => false,
    };
    let stranded: Vec<usize> = pieces
        .iter()
        .enumerate()
        .filter(|(_, piece)| refused(piece))
        .map(|(at, _)| at)
        .collect();
    walk_the_refused(rooms, pieces, &stranded);
}

/// The STV16 migration: a calling room grew under the cargo standing in
/// it.
///
/// The market, the parlor and the pump bay each took another cell or two
/// so a station's furniture and the crew's staged cargo would both fit
/// on one deck (docs/ROOMS.md). A room net is a cross of six
/// charts laid flat, so a wider floor pushes the starboard wall and the
/// ceiling along it and a deeper one pushes the front wall down — which
/// means a berth written against the old extent names a cell on a
/// different chart now. Every one of them is **translated per chart**,
/// exactly as the cabin's own widening was ([`widen`]): the aft wall,
/// the port wall and the floor held still.
///
/// A translation can only be wrong about where a cell is, never about
/// what it is for, so whatever the arbiter refuses once it has run — the
/// counter's own deck in a bay whose counter moved into the corner, a
/// footprint that now straddles a fold — walks to the first berth its
/// own room still has. Conservation before convenience: a piece comes
/// out standing somewhere else, never gone.
fn regrow_the_calling_rooms(rooms: &Rooms, pieces: &mut [Piece]) {
    for piece in pieces.iter_mut() {
        let (room, x, y) = match piece.loc {
            Loc::Hold { room, x, y } | Loc::Laid { room, x, y } => (room, x, y),
            Loc::Stow { .. } => continue,
        };
        let Some(host) = rooms.kind(room) else {
            continue;
        };
        let (x, y) = regrown(host, x, y);
        piece.loc = match piece.loc {
            Loc::Laid { .. } => Loc::Laid { room, x, y },
            _ => Loc::Hold { room, x, y },
        };
    }
    // Only the rooms that actually grew are asked to walk anything: a
    // berth this migration did not touch is a berth some other rule's
    // business, and a migration that tidied those would be answering a
    // question nobody asked it.
    let stranded: Vec<usize> = pieces
        .iter()
        .enumerate()
        .filter(|(_, piece)| match piece.loc {
            Loc::Hold { room, x, y } => {
                regrew(rooms.kind(room))
                    && cargo::placement_check(rooms, &[], piece.id, piece.kind, room, x, y).is_err()
            }
            Loc::Laid { room, x, y } => {
                regrew(rooms.kind(room))
                    && cargo::dressing_check(rooms, &[], piece.id, piece.kind, room, x, y).is_err()
            }
            Loc::Stow { .. } => false,
        })
        .map(|(at, _)| at)
        .collect();
    walk_the_refused(rooms, pieces, &stranded);
}

/// Whether a room kind's floor is wider or deeper than it was before
/// STV16.
const fn regrew(kind: Option<RoomKind>) -> bool {
    matches!(
        kind,
        Some(RoomKind::Trade | RoomKind::Parlor | RoomKind::Pump)
    )
}

/// Where a cell of a pre-STV16 net lands on the net its kind has now.
///
/// The three that grew are written out here rather than reached for,
/// because a migration is the one place a retired number still has to be
/// said out loud.
const fn regrown(kind: RoomKind, x: u8, y: u8) -> (u8, u8) {
    let (was_w, was_h) = match kind {
        RoomKind::Trade => (6, 5),
        RoomKind::Parlor => (4, 4),
        RoomKind::Pump => (3, 3),
        _ => return (x, y),
    };
    let (w, h) = kind.floor();
    // The starboard wall and the ceiling both begin a floor's width
    // along the net, so both slide by exactly what the floor gained.
    if x >= COURSES + was_w {
        return (x + w - was_w, y);
    }
    // And the front wall begins a floor's depth down it.
    if y >= COURSES + was_h {
        return (x, y + h - was_h);
    }
    (x, y)
}

/// Walk each named piece to the first berth of its own room the arbiter
/// still accepts, in the document's own piece order so the walk is as
/// deterministic as everything else here. A room with nowhere left for
/// one leaves it standing, because a piece kept is worth more than a
/// rule tidied.
fn walk_the_refused(rooms: &Rooms, pieces: &mut [Piece], stranded: &[usize]) {
    for &at in stranded {
        let (id, kind) = (pieces[at].id, pieces[at].kind);
        let (room, laid) = match pieces[at].loc {
            Loc::Hold { room, .. } => (room, false),
            Loc::Laid { room, .. } => (room, true),
            Loc::Stow { .. } => continue,
        };
        let Some(host) = rooms.kind(room) else {
            continue;
        };
        let settled: Vec<Piece> = pieces.to_vec();
        let (cols, rows) = host.grid();
        let free = (0..rows)
            .flat_map(|y| (0..cols).map(move |x| (x, y)))
            .find(|&(x, y)| {
                if laid {
                    cargo::dressing_check(rooms, &settled, id, kind, room, x, y).is_ok()
                } else {
                    super::placement_legal(rooms, &settled, id, kind, room, x, y)
                }
            });
        let Some((x, y)) = free else { continue };
        pieces[at].loc = if laid {
            Loc::Laid { room, x, y }
        } else {
            Loc::Hold { room, x, y }
        };
    }
}

/// Hang the instruments a pre-STV9 save predates: each missing
/// instrument kind goes to its traditional berth (the instrument tail
/// of `STARTER_CARGO`), or to its first legal cell when the board has
/// claimed that wall. A board with room for none fails the load whole —
/// a ship without its chart tank or launch lever is the soft-lock the
/// vital rule exists to prevent, so the reader will not construct one.
fn inject_instruments(
    reader: &Reader<'_>,
    rooms: &Rooms,
    pieces: &mut Vec<Piece>,
    next_piece: &mut u32,
) -> Result<(), SaveError> {
    for (kind, x, y) in super::STARTER_CARGO {
        if !kind.instrument() || pieces.iter().any(|piece| piece.kind == kind) {
            continue;
        }
        let id = *next_piece;
        let loc = if cargo::placement_check(rooms, pieces, id, kind, CABIN, x, y).is_ok() {
            Loc::Hold { room: CABIN, x, y }
        } else {
            let (room, x, y) =
                cargo::first_fit(rooms, pieces, id, kind).ok_or_else(|| reader.err())?;
            Loc::Hold { room, x, y }
        };
        pieces.push(Piece {
            id,
            kind,
            variant: 0,
            gnawed: false,
            loc,
        });
        *next_piece += 1;
    }
    Ok(())
}

/// Cross-piece stow and dressing validation, after the whole list is
/// read (a cubby may reference a cabinet on a later line). Everything
/// those lines could lie about is checked here, so no later indexing or
/// invariant trips: a stow's host must be a cabinet standing in a room,
/// the cargo stowable, no cubby doubled; laid dressings must not overlap
/// one another, and no berth may sit on a threshold.
fn validate_stows(reader: &Reader<'_>, rooms: &Rooms, pieces: &[Piece]) -> Result<(), SaveError> {
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
            Loc::Laid { room, x, y } => {
                // Re-run the dressing rules against the other dressings
                // only: bounds, surface, and one-per-cell. Occupancy is
                // deliberately absent from the slice — a rug pinned
                // under a couch is a LEGAL state and saves as such.
                if cargo::dressing_check(rooms, &laid, piece.id, piece.kind, room, x, y).is_err() {
                    return Err(reader.err());
                }
                laid.push(*piece);
            }
            Loc::Hold { room, x, y } => {
                if rooms
                    .tile(room, x, y)
                    .is_none_or(|tile| tile == Tile::Threshold)
                {
                    return Err(reader.err());
                }
            }
        }
    }
    Ok(())
}

/// A piece's location tokens, bounds-checked so later indexing never panics.
fn parse_loc<'a>(
    reader: &Reader<'_>,
    tokens: &mut impl Iterator<Item = &'a str>,
    rooms: &Rooms,
    kind: Kind,
    roomless: bool,
) -> Result<Berth, SaveError> {
    let cell = |reader: &Reader<'_>,
                tokens: &mut dyn Iterator<Item = &'a str>|
     -> Result<(RoomId, u8, u8), SaveError> {
        let room: RoomId = reader.token(tokens.next())?;
        let x: u8 = reader.token(tokens.next())?;
        let y: u8 = reader.token(tokens.next())?;
        let host = rooms.kind(room).ok_or_else(|| reader.err())?;
        let (w, h) = kind.cells();
        let (cols, rows) = host.grid();
        if x + w > cols || y + h > rows {
            return Err(reader.err());
        }
        Ok((room, x, y))
    };
    match tokens.next() {
        Some("hold") if roomless => {
            let x: u8 = reader.token(tokens.next())?;
            let y: u8 = reader.token(tokens.next())?;
            Ok(Berth::Cell(x, y))
        }
        Some("hold") => {
            let (room, x, y) = cell(reader, tokens)?;
            Ok(Berth::Settled(Loc::Hold { room, x, y }))
        }
        Some("laid") if roomless => {
            let x: u8 = reader.token(tokens.next())?;
            let y: u8 = reader.token(tokens.next())?;
            if !kind.covering() {
                return Err(reader.err());
            }
            Ok(Berth::Dress(x, y))
        }
        Some("laid") => {
            let (room, x, y) = cell(reader, tokens)?;
            if !kind.covering() {
                return Err(reader.err());
            }
            Ok(Berth::Settled(Loc::Laid { room, x, y }))
        }
        Some("stow") => {
            let cabinet: u32 = reader.token(tokens.next())?;
            let slot: u8 = reader.token(tokens.next())?;
            if slot >= cargo::CABINET_SLOTS {
                return Err(reader.err());
            }
            // The cabinet reference is checked in `validate_stows`, once
            // the whole piece list exists.
            Ok(Berth::Settled(Loc::Stow { cabinet, slot }))
        }
        // The retired barter counter's berths, read only from pre-rooms
        // documents and resolved by `resettle`.
        Some(surface @ ("shelf" | "give" | "take" | "recv" | "flot")) if roomless => {
            let slot: u8 = reader.token(tokens.next())?;
            if slot >= 4 {
                return Err(reader.err());
            }
            Ok(match surface {
                "shelf" | "take" => Berth::Theirs,
                "flot" => Berth::Fuel,
                _ => Berth::Ours,
            })
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
        for bad in [
            "piece 0 99 0 0 hold 0 4 4",
            "piece 0 0 0 0 hold 0 22 13",
            "piece 0 0 0 0 hold 9 4 4",
            "piece 0 0 0 0 hold 0 11 3",
            "piece 0 0 0 0 nowhere 0",
            "piece 0 0 0 2 hold 0 4 4",
            "piece 0 0 0 gnawed hold 0 4 4",
        ] {
            let mangled = docked.replacen(&piece_line, bad, 1);
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }

    /// A sim with a stocked cabinet, its cubby lines pinned.
    fn furnished() -> (Sim, String, u32, (u8, u8)) {
        let mut sim = Sim::new(3);
        let (_, x, y) = cargo::first_fit(sim.rooms(), sim.pieces(), u32::MAX, Kind::Cabinet)
            .expect("room for a cabinet");
        let cabinet = sim.next_piece;
        for (offset, kind, loc) in [
            (0, Kind::Cabinet, Loc::Hold { room: CABIN, x, y }),
            (1, Kind::PerfumeVial, Loc::Stow { cabinet, slot: 0 }),
            (2, Kind::Fluff, Loc::Stow { cabinet, slot: 1 }),
            (
                3,
                Kind::Rug,
                Loc::Laid {
                    room: CABIN,
                    x: 3,
                    y: 6,
                },
            ),
            (
                4,
                Kind::LuminousPaint,
                Loc::Laid {
                    room: CABIN,
                    x: 5,
                    y: 0,
                },
            ),
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
    fn stowed_pieces_round_trip() {
        let (sim, save, cabinet, _) = furnished();
        assert!(
            save.starts_with(&format!("{MAGIC}\n")),
            "the writer stamps the current version"
        );
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

    /// Whether a save line is an instrument piece — pre-STV9 documents
    /// have none, so forging one starts by stripping them.
    /// Whether a save line is a window piece that is not an instrument
    /// — a porthole or a bay pane, both of which a console-era document
    /// predates and neither of which the reader hangs for free.
    fn window_line(line: &str) -> bool {
        line_kind(line).is_some_and(|kind| kind.window() && !kind.instrument())
    }

    /// The kind a `piece` line names, if it names one.
    fn line_kind(line: &str) -> Option<Kind> {
        line.starts_with("piece")
            .then(|| {
                line.split_whitespace()
                    .nth(2)
                    .and_then(|token| token.parse::<usize>().ok())
                    .and_then(|index| Kind::ALL.get(index).copied())
            })
            .flatten()
    }

    fn instrument_line(line: &str) -> bool {
        line_kind(line).is_some_and(Kind::instrument)
    }

    /// Turn a modern save into a pre-rooms one: strip the room and mark
    /// lines, put the eased dial back, and drop the room qualifier from
    /// every berth.
    fn deroom(save: &str, header: &str) -> String {
        let mut out = String::new();
        for line in save.lines() {
            if line.starts_with("rooms ") || line.starts_with("room ") {
                continue;
            }
            if line.starts_with("marks") {
                out.push_str("eager 3f800000 3\n");
                continue;
            }
            if line.starts_with("piece") {
                let mut tokens: Vec<&str> = line.split_whitespace().collect();
                if matches!(tokens.get(5), Some(&"hold" | &"laid")) {
                    // A pre-rooms document knew one room, so anything the
                    // dock brought alongside simply was not in it.
                    if tokens[6] != "0" {
                        continue;
                    }
                    tokens.remove(6);
                }
                out.push_str(&tokens.join(" "));
                out.push('\n');
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out.replacen(MAGIC, header, 1)
    }

    /// A pre-rooms document loads into the graph: every berth becomes a
    /// cabin berth, and the ship is a cabin with a burner bolted on.
    #[test]
    fn pre_rooms_saves_land_in_the_cabin() {
        let plain = Sim::new(17).save_string();
        let old = deroom(&plain, "STV10");
        assert!(!old.contains("rooms "), "the room block is gone");
        let sim = Sim::from_save(&old).expect("STV10 must stay readable");
        assert_eq!(sim.rooms().count(), 3, "cabin, burner, and the dock");
        assert!(
            sim.pieces().iter().all(|piece| match piece.loc {
                Loc::Hold { room, .. } | Loc::Laid { room, .. } =>
                    room == CABIN || sim.rooms().kind(room) == Some(RoomKind::Trade),
                Loc::Stow { .. } => true,
            }),
            "every migrated berth is the cabin's"
        );
    }

    /// A legacy mid-trade save resolves on load: the player's pieces walk
    /// back aboard, the station's stock is dropped, and staged fuel lands
    /// in the incinerator's grid.
    #[test]
    fn a_mid_trade_legacy_save_resolves_on_load() {
        let plain = Sim::new(19).save_string();
        let old = deroom(&plain, "STV10");
        let forged = old.replacen(
            "next_piece",
            "piece 900 7 0 0 give 0\npiece 901 4 0 0 shelf 1\npiece 902 3 0 0 recv 2\n\
             piece 903 13 0 0 flot 0\nnext_piece",
            1,
        );
        let sim = Sim::from_save(&forged).expect("a mid-trade legacy save must load");
        let find = |id: u32| sim.pieces().iter().find(|piece| piece.id == id).copied();
        // Ours walked aboard.
        for id in [900, 902] {
            let piece = find(id).expect("the player's pieces walk aboard");
            assert!(
                matches!(piece.loc, Loc::Hold { room, .. } | Loc::Laid { room, .. }
                    if sim.rooms().riding(room)),
                "piece {id} did not come aboard"
            );
        }
        // Theirs was dropped: the station no longer exists as a place.
        assert!(find(901).is_none(), "the station's stock stayed behind");
        // And the fuel is in the furnace room, on its hazard tiles.
        let fuel = find(903).expect("staged fuel survives");
        let Loc::Hold { room, x, y } = fuel.loc else {
            panic!("fuel must occupy a cell")
        };
        assert_eq!(sim.rooms().kind(room), Some(RoomKind::Burner));
        assert_eq!(sim.rooms().tile(room, x, y), Some(Tile::Consume));
    }

    /// The entry-path law's migration: a pre-STV14 document's proposal
    /// may be standing on cells this build calls plain deck, and a trade
    /// quietly withdrawn on load is a worse answer than a trade that
    /// moved. So it moves — same room, onto the offer area the law
    /// actually leaves.
    #[test]
    fn a_proposal_left_in_the_doorway_walks_onto_the_offer_area() {
        use super::super::room::RoomKind;

        let sim = Sim::new(19);
        let trade = sim
            .rooms()
            .find(RoomKind::Trade)
            .expect("docked, so a market is alongside");
        // A cell the old band chalked and this one does not: the door's
        // own lane, on the front wall. A document this old was written
        // when a market measured six by five, so it names the cell in
        // the net of the day and the growth translates it into this one.
        let (x, y) = (COURSES, COURSES + 5);
        let (_, depth) = RoomKind::Trade.floor();
        let (nx, ny) = (COURSES, COURSES + depth);
        assert!(RoomKind::Trade.entry_path(nx, ny), "that is the way in");
        assert_eq!(RoomKind::Trade.tile_of(nx, ny), Some(Tile::Staging));
        let lamp = Kind::WallLamp.index();
        let forged = sim.save_string().replacen(MAGIC, "STV13", 1).replacen(
            "next_piece",
            &format!("piece 900 {lamp} 0 0 hold {trade} {x} {y}\nnext_piece"),
            1,
        );
        let loaded = Sim::from_save(&forged).expect("an STV13 document must still load");
        let piece = loaded
            .pieces()
            .iter()
            .find(|piece| piece.id == 900)
            .copied()
            .expect("the proposal survives the load");
        let Loc::Hold { room, x, y } = piece.loc else {
            panic!("a proposal occupies a cell")
        };
        assert_eq!(
            room, trade,
            "the proposal stayed in the room it was made to"
        );
        assert_eq!(
            loaded.rooms().tile(room, x, y),
            Some(Tile::Offer),
            "the proposal is standing on deck rather than on the offer area"
        );
        // And a document this build wrote is left exactly alone.
        let fresh = Sim::from_save(&loaded.save_string()).expect("round trip");
        assert_eq!(
            fresh.pieces().iter().find(|p| p.id == 900).map(|p| p.loc),
            Some(piece.loc),
            "an STV14 document's berths are not migrated"
        );
    }

    /// The fixture law's migration: a pre-STV15 document may stand cargo
    /// on the deck a counter is worked over, and this build refuses that
    /// cell. A piece the arbiter refuses is a piece no later rule can
    /// reason about, so it walks — same room, first cell that still
    /// takes it — and nothing is ever lost on the way.
    #[test]
    fn cargo_left_inside_a_rooms_own_hardware_walks_off_it() {
        use super::super::room::RoomKind;

        let sim = Sim::new(19);
        let trade = sim
            .rooms()
            .find(RoomKind::Trade)
            .expect("docked, so a market is alongside");
        let (hx, _) = RoomKind::Trade
            .handshake()
            .expect("a market has a handshake");
        let (x, y) = (hx, COURSES);
        assert_eq!(
            RoomKind::Trade.tile_of(x, y),
            Some(Tile::Fixture),
            "that deck is the counter's"
        );
        let forged = sim.save_string().replacen(MAGIC, "STV14", 1).replacen(
            "next_piece",
            &format!("piece 901 7 0 0 hold {trade} {x} {y}\nnext_piece"),
            1,
        );
        let loaded = Sim::from_save(&forged).expect("an STV14 document must still load");
        let piece = loaded
            .pieces()
            .iter()
            .find(|piece| piece.id == 901)
            .copied()
            .expect("the crate survives the load");
        let Loc::Hold { room, x, y } = piece.loc else {
            panic!("a crate occupies a cell")
        };
        assert_eq!(room, trade, "the crate stayed in the room it was left in");
        assert_ne!(
            loaded.rooms().tile(room, x, y),
            Some(Tile::Fixture),
            "the crate is still standing inside the counter"
        );
        // And a document this build wrote is left exactly alone.
        let fresh = Sim::from_save(&loaded.save_string()).expect("round trip");
        assert_eq!(
            fresh.pieces().iter().find(|p| p.id == 901).map(|p| p.loc),
            Some(piece.loc),
            "an STV15 document's berths are not migrated"
        );
    }

    /// The growth's migration: three of the calling rooms grew, so
    /// a berth in one names a cell of a narrower net. The translation is
    /// **chart-preserving** — a lamp left on the starboard wall comes
    /// back on the starboard wall and one left on the front wall comes
    /// back on the front wall — and conservation is total: a document
    /// that went in with two lamps comes out with two lamps.
    #[test]
    fn cargo_in_a_room_that_grew_comes_back_on_the_chart_it_was_left_on() {
        use super::super::room::RoomKind;

        let sim = Sim::new(23);
        let trade = sim
            .rooms()
            .find(RoomKind::Trade)
            .expect("docked, so a market is alongside");
        // The market measured six by five when a document like this was
        // written: its starboard wall began at column nine and its front
        // wall at row eight.
        let (was_w, was_h) = (6_u8, 5_u8);
        let (starboard, front) = (
            (COURSES + was_w, COURSES + 1),
            (COURSES + 1, COURSES + was_h + 1),
        );
        let lamp = Kind::WallLamp.index();
        let forged = sim.save_string().replacen(MAGIC, "STV15", 1).replacen(
            "next_piece",
            &format!(
                "piece 910 {lamp} 0 0 hold {trade} {} {}\n\
                 piece 911 {lamp} 0 0 hold {trade} {} {}\nnext_piece",
                starboard.0, starboard.1, front.0, front.1
            ),
            1,
        );
        let loaded = Sim::from_save(&forged).expect("an STV15 document must still load");
        let charted = |id: u32| {
            let piece = loaded
                .pieces()
                .iter()
                .find(|piece| piece.id == id)
                .unwrap_or_else(|| panic!("piece {id} survives the load"));
            let Loc::Hold { room, x, y } = piece.loc else {
                panic!("a lamp occupies a cell")
            };
            assert_eq!(room, trade, "piece {id} left the room it was hung in");
            RoomKind::Trade.surface_of(x, y)
        };
        assert_eq!(charted(910), Some(Surf::Starboard));
        assert_eq!(charted(911), Some(Surf::Front));
        // And a document this build wrote is left exactly alone.
        let written = loaded.save_string();
        let fresh = Sim::from_save(&written).expect("round trip");
        for id in [910, 911] {
            assert_eq!(
                fresh.pieces().iter().find(|p| p.id == id).map(|p| p.loc),
                loaded.pieces().iter().find(|p| p.id == id).map(|p| p.loc),
                "an STV16 document's berths are not migrated"
            );
        }
    }

    /// The window family's migration: a pre-STV13 document's discovery
    /// ledger is two bits short of the table, and the widening is the
    /// one that is TRUE. A station the crew had exhausted becomes a
    /// station they know everything about — the Guild boots that way, so
    /// a Guild that came back from an old document not knowing what a
    /// porthole is would be a Guild that forgot its own shelves. A
    /// partial ledger keeps its bits: nobody has offered that crew any
    /// glass yet, which is what a missing bit means.
    #[test]
    fn pre_stv13_saves_widen_the_ledger_they_were_written_with() {
        let plain = Sim::new(31).save_string();
        let ledger = plain
            .lines()
            .find(|line| line.starts_with("familiar"))
            .expect("every document carries a ledger");
        // The same document, written before the family: the masks are
        // as wide as the table was, and one station is part-explored.
        let narrow = KNOWN_ALL & !window_mask();
        let old = format!(
            "familiar 0000 00ff {narrow:04x} 0000 0000 0000 {narrow:04x} 0000 0000 0000 0000 0000"
        );
        let forged = plain.replacen(MAGIC, "STV12", 1).replacen(ledger, &old, 1);
        let sim = Sim::from_save(&forged).expect("an STV12 document must still load");
        assert_eq!(
            sim.familiar[usize::from(super::super::map::GUILD)],
            KNOWN_ALL,
            "home turf knows the whole table, glass included"
        );
        assert_eq!(sim.familiar[2], KNOWN_ALL, "so does an exhausted station");
        assert_eq!(
            sim.familiar[1], 0x00ff,
            "a part-explored station has simply not been shown any glass"
        );
        // And every window kind now reads familiar at the Guild, which
        // is the fact the widening exists to keep true.
        for kind in Kind::ALL.iter().filter(|kind| kind.window()) {
            assert!(
                sim.familiar[usize::from(super::super::map::GUILD)] & (1 << kind.index()) != 0,
                "{kind:?} must be home-turf familiar"
            );
        }
        // A current document means what it says: no widening at all.
        let honest = plain.replacen(ledger, &old, 1);
        let sim = Sim::from_save(&honest).expect("a current document loads");
        assert_eq!(sim.familiar[2], narrow, "an STV13 ledger is read verbatim");
    }

    /// The port law's migration: a pre-STV12 document may hang a room
    /// off a slot its kind has since given up — here the market under the
    /// cabin's ladder, which no market has any more. The room is
    /// re-seated through the spawn walk rather than lost, it keeps its
    /// id, and everything berthed in it comes with it.
    #[test]
    fn pre_stv12_saves_reseat_rooms_that_lost_their_port() {
        let plain = Sim::new(23).save_string();
        let dock = plain
            .lines()
            .find(|line| line.starts_with("room 2 "))
            .expect("the dock's room is alongside")
            .to_owned();
        assert_eq!(
            dock, "room 2 2 0 0 0",
            "the market mates the cabin's aft door"
        );
        let stocked = |sim: &Sim| {
            sim.pieces()
                .iter()
                .filter(|piece| matches!(piece.loc, Loc::Hold { room: 2, .. }))
                .count()
        };
        // The same document, written when a market carried a hatch.
        let forged = plain
            .replacen(MAGIC, "STV11", 1)
            .replacen(&dock, "room 2 2 0 4 5", 1);
        let sim = Sim::from_save(&forged).expect("an STV11 document must still load");
        assert_eq!(
            sim.rooms().kind(2),
            Some(RoomKind::Trade),
            "the dock survived"
        );
        assert_eq!(
            sim.rooms().get(2).expect("room two is placed").pose.z,
            0,
            "re-seated through a door, not a hatch that is not there"
        );
        assert_eq!(
            stocked(&sim),
            stocked(&Sim::from_save(&plain).expect("the current document loads")),
            "the market's goods came with it"
        );
        // A current document gets no such mercy: it must mean what it says.
        let lying = plain.replacen(&dock, "room 2 2 0 4 5", 1);
        assert!(
            Sim::from_save(&lying).is_err(),
            "a save that lies fails safe"
        );
    }

    #[test]
    fn older_headers_still_read() {
        // Each version since STV4 added only a line form or token; a save
        // without them is a valid older document, and the retired
        // console's runs keep walking aboard. Those documents carry
        // console-era 6×4 hold coordinates, no instrument pieces, and no
        // window that was not one (the porthole is bought, not launched
        // with), so the fixture strips both and rewrites the berths into
        // the old grid — and the reader must walk the whole chain (the
        // console embed, then the widening, then the re-fit) AND hang the
        // five missing instruments, the porthole staying unbought.
        let plain = Sim::new(9).save_string();
        let deroomed = deroom(&plain, "STV10");
        let mut old_cells = [(0_u8, 0_u8), (0, 2), (2, 0), (5, 0)].into_iter();
        let legacy_board: String = deroomed
            .lines()
            .filter(|line| !instrument_line(line) && !window_line(line))
            .map(|line| {
                if line.starts_with("piece") && line.contains(" hold ") {
                    let head = line.split(" hold ").next().expect("split has a head");
                    let (x, y) = old_cells.next().expect("four console-era berths");
                    format!("{head} hold {x} {y}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        assert!(
            old_cells.next().is_none(),
            "console-era cargo is four pieces"
        );
        for older in ["STV7", "STV6", "STV5", "STV4"] {
            let mut old = legacy_board.replacen("STV10", older, 1);
            if older != "STV7" {
                old = old.replacen("ship docked 6 - 0", "ship docked 6 -", 1);
            }
            assert_ne!(old, plain);
            let sim = Sim::from_save(&old).unwrap_or_else(|e| {
                panic!("{older} must stay readable: {e}");
            });
            let held: Vec<&Piece> = sim
                .pieces
                .iter()
                .filter(|piece| matches!(piece.loc, Loc::Hold { room: CABIN, .. }))
                .collect();
            assert_eq!(
                held.len(),
                9,
                "{older}: four migrated berths plus five hung instruments"
            );
            for piece in held {
                let Loc::Hold { room, x, y } = piece.loc else {
                    unreachable!()
                };
                assert!(
                    cargo::placement_check(
                        sim.rooms(),
                        &sim.pieces,
                        piece.id,
                        piece.kind,
                        room,
                        x,
                        y
                    )
                    .is_ok(),
                    "{older}: {:?} migrated to an illegal berth ({x}, {y})",
                    piece.kind
                );
            }
            for kind in [Kind::ChartTank, Kind::LaunchLever] {
                assert!(
                    sim.pieces.iter().any(|piece| piece.kind == kind),
                    "{older}: the vital {kind:?} must come aboard"
                );
            }
        }
    }

    /// The widening, chart by chart: a pre-STV10 board carries narrow-net
    /// coordinates, and each of the six charts translates by its own
    /// declared offset — nothing is re-solved, and the stowaway rides
    /// the same arithmetic its floor does.
    #[test]
    fn pre_stv10_boards_widen_chart_by_chart() {
        let plain = deroom(&Sim::new(5).save_string(), "STV9");
        let head = plain.split("piece ").next().expect("a save has a head");
        let narrow = format!(
            "{head}\
piece 0 20 0 0 hold 5 1
piece 1 17 0 0 hold 1 6
piece 2 0 0 0 hold 7 6
piece 3 26 0 0 hold 10 5
piece 4 29 0 0 hold 5 8
piece 5 16 0 0 hold 14 5
piece 6 22 0 0 laid 3 7
next_piece 7
"
        )
        .replacen("rat -", "rat 14 4 9 3 0 60 120 0", 1);
        let sim = Sim::from_save(&narrow).expect("STV9 must stay readable");
        for (id, want) in [
            (
                0,
                Loc::Hold {
                    room: CABIN,
                    x: 5,
                    y: 1,
                },
            ), // aft: held still
            (
                1,
                Loc::Hold {
                    room: CABIN,
                    x: 1,
                    y: 6,
                },
            ), // port: held still
            (
                2,
                Loc::Hold {
                    room: CABIN,
                    x: 7,
                    y: 6,
                },
            ), // floor: held still
            (
                3,
                Loc::Hold {
                    room: CABIN,
                    x: 12,
                    y: 5,
                },
            ), // starboard: +2 columns
            (
                4,
                Loc::Hold {
                    room: CABIN,
                    x: 5,
                    y: 10,
                },
            ), // front: +2 rows
            (
                5,
                Loc::Hold {
                    room: CABIN,
                    x: 16,
                    y: 5,
                },
            ), // ceiling: +2 columns
            (
                6,
                Loc::Laid {
                    room: CABIN,
                    x: 3,
                    y: 7,
                },
            ),
        ] {
            let piece = sim
                .pieces
                .iter()
                .find(|piece| piece.id == id)
                .expect("every berth survives the trip");
            assert_eq!(piece.loc, want, "piece {id} landed wrong");
        }
        let rat = sim.rats.rat.expect("the stowaway survives the trip");
        assert_eq!(rat.cell, (16, 4));
        assert_eq!(rat.prev_cell, (11, 3));
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
            // A host that is not standing in a room.
            (
                format!("piece {cabinet} 21 0 0 hold 0 {x} {y}"),
                format!("piece {cabinet} 21 0 0 stow {cabinet} 3"),
            ),
            // A laid non-covering (the couch, index 19).
            (
                format!("piece {} 22 0 0 laid 0 3 6", cabinet + 3),
                format!("piece {} 19 0 0 laid 0 3 6", cabinet + 3),
            ),
            // A rug up the wall.
            (
                format!("piece {} 22 0 0 laid 0 3 6", cabinet + 3),
                format!("piece {} 22 0 0 laid 0 5 1", cabinet + 3),
            ),
            // Two dressings on one cell.
            (
                format!("piece {} 24 0 0 laid 0 5 0", cabinet + 4),
                format!("piece {} 24 0 0 laid 0 3 6", cabinet + 4),
            ),
            // A coat off the grid entirely.
            (
                format!("piece {} 24 0 0 laid 0 5 0", cabinet + 4),
                format!("piece {} 24 0 0 laid 0 21 12", cabinet + 4),
            ),
        ] {
            let mangled = save.replacen(&needle, &bad, 1);
            assert_ne!(mangled, save, "needle {needle:?} not found in save");
            assert!(Sim::from_save(&mangled).is_err(), "{bad:?} parsed anyway");
        }
    }
}
