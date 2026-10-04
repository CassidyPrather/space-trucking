//! **Purchased art: the declaration, and the loading of it.**
//!
//! The game has two graphical implementations of every object planned:
//! the whitebox this repository cuts in code, and a bought asset. This
//! module is the seam. It has two halves and they are gated differently
//! on purpose.
//!
//! **The declaration half is always here.** `art/manifest.toml` is in
//! the repository — it is the one part of the pipeline the licence lets
//! be public — and every asset in it may carry a `dresses` line saying
//! which body of the game it stands in for, plus the four numbers saying
//! how it sits in that body's box. The payload those numbers describe is
//! not in the repository and never will be.
//!
//! **The loading half is in the default build**, and compiled out under
//! `--features whitebox`. That is where
//! `bevy_gltf` comes in, and where `$ART_CACHE/index.toml`
//! — written by `cargo xtask art resolve` on the machine of somebody who
//! holds the licence — is read at boot. Everything about it fails soft:
//! no cache, no entry, an entry that will not parse, a file that is not
//! there, and the whitebox stands in, because a game that will not start
//! because somebody moved a directory is worse than a game with grey
//! boxes in it. What it never does is put a word on the screen; the
//! zero-text law covers what is drawn, and a complaint belongs on stderr.
//!
//! # The placement frame, which is an API
//!
//! A cargo kind's description claims a box — its `Kind::upright` cells
//! across and up, and the one cell of depth every rig is composed within
//! (`pieces::RIG_NEAR..RIG_FAR`). **That box is `[-1, 1]` on every axis
//! of the placement frame**, which is the same normalised frame
//! `poi::Fitting` states a station's hardware in, one box down. In it:
//!
//! | | what it means |
//! | --- | --- |
//! | `scale` | the converted file's own units, carried into berth half-units |
//! | `rotation` | degrees about x, then y, then z, taken in the box's own axes |
//! | `offset` | where the body's middle sits, in berth half-units — `poi::Fitting`'s `at` |
//! | `fill` | what the body then occupies of the box, per axis — `poi::Fitting`'s `half`, and `poi::Shape::fill`'s meaning |
//!
//! In that order: the mesh is centred on its own measured middle, scaled,
//! turned, and set down at the offset. `|offset| + fill <= 1` on an axis
//! is exactly "the body stays inside its berth", and it is not enforced
//! here.
//!
//!
//! **A turn turns the frame, not just the body.** `scale` is per-axis in
//! the MESH's frame and `half` is per-axis in the FRAME's, and the mesh
//! is scaled along its own axes before it is turned — so the half-unit
//! `scale` is counted in is the one belonging to the frame axis that
//! mesh axis LANDS on ([`Dressing::landing`]). While every rotated
//! declaration happened to sit in a frame that was square across the
//! pair its turn swapped, the two readings agreed and nothing needed
//! saying. A trade room is not square, so the first station fitting
//! turned a quarter about `y` asked for a `fill` of twenty-three on one
//! axis — refused, and rightly, because a fraction of the wrong axis is
//! not a fraction of a frame. With the frame turned instead,
//! `scale = 1 / measured_half` and `fill = [1, 1, 1]` mean "fills its
//! frame" under **any** quarter turn, which is what the resolver's own
//! advice always claimed.
//!
//! `scale` and `fill` are deliberately **redundant**, and the redundancy
//! is the mechanism: `fill` is a promise living in the repository, and
//! `scale` times the mesh's own measured size is the fact. `cargo xtask
//! art resolve` is where the two are made to meet.
//!
//! Two consequences worth knowing before writing numbers. A berth box is
//! a cube for every one-cell kind and 1:2:1 for a `1×2` one, so the
//! placement frame is anisotropic on the tall kinds: a rotation that is
//! not a quarter turn shears a body there, and a per-axis `scale` is how
//! to answer it. And the numbers the game draws are the *index's*, not
//! the manifest's — an edit to the manifest reaches the game through
//! `resolve`, which is also the only moment it is checked.

// The whitebox build compiles the loading half out, and with it the only
// reader of most of the declaration half.
#![cfg_attr(feature = "whitebox", allow(dead_code))]
use bevy::prelude::*;
use space_trucking::sim::cargo::KIND_COUNT;
use space_trucking::sim::room::{ROOM_KINDS, RoomKind};
use space_trucking::sim::{Kind, layout};

/// **The parts of a room's shell a purchased module can stand in for.**
///
/// Cargo is dressed one mesh per kind; the shell is dressed one mesh per
/// *role*, and `room::cladding` decides how many of each a room needs
/// and where each one's frame is — a wall five cells long is one panel
/// stretched, a wall eight cells long is two. The roles are the things
/// the whitebox draws separately: a wall slab, the deck pan, the
/// deckhead pan, and what stands in an opening.
///
/// An **appended table**, like `Kind`: the manifest spells these by
/// name (`fabric/wall`) and a build older than a role reads the binding
/// as a stranger rather than as the wrong role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fabric {
    /// A wall panel, stood between deck and deckhead; also stood over a
    /// mated doorway, upside down, as the lintel.
    Wall,
    /// A deck tile.
    Floor,
    /// A deckhead tile.
    Ceiling,
    /// What fills a door drawn shut: a door frame over the aperture with
    /// its leaf in it. The same frame as [`Fabric::Doorway`]'s, in the
    /// same box, so a door that is shut on one dock and open on the next
    /// is one frame whose leaf comes and goes rather than two frames.
    Door,
    /// A mated doorway's mouth: the same frame, drawn open. A module
    /// with a `leaf` line hides that node here (`loading::Open`); one
    /// without is a frame with a hole in it, which is also open.
    Doorway,
    /// What fills a hatch drawn shut: a cover in the deck's aperture.
    Hatch,
}

/// How many fabric roles there are.
pub const FABRIC_COUNT: usize = 6;

/// One slot per room kind and one for "every room": what a `room` line
/// selects among.
const ROOM_SLOTS: usize = ROOM_KINDS.len() + 1;

impl Fabric {
    /// Every role, in table order.
    pub const ALL: [Self; FABRIC_COUNT] = [
        Self::Wall,
        Self::Floor,
        Self::Ceiling,
        Self::Door,
        Self::Doorway,
        Self::Hatch,
    ];

    /// The stable slot.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The name a manifest spells it by, after `fabric/`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Floor => "floor",
            Self::Ceiling => "ceiling",
            Self::Door => "door",
            Self::Doorway => "doorway",
            Self::Hatch => "hatch",
        }
    }
}

/// **Which fabric role a `dresses` name means.**
#[must_use]
pub fn fabric_named(name: &str) -> Option<Fabric> {
    Fabric::ALL.into_iter().find(|role| role.name() == name)
}

/// **Which room kind a `room` line means**, by the kind's own spelling
/// in snake case — the same derivation [`kind_named`] uses for cargo,
/// for the same reason: a second table would have to be kept in step
/// with `ROOM_KINDS` by hand.
#[must_use]
pub fn room_named(name: &str) -> Option<RoomKind> {
    ROOM_KINDS
        .into_iter()
        .find(|kind| snake_case(&format!("{kind:?}")) == name)
}

/// A room kind's manifest spelling.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn room_snake(kind: RoomKind) -> String {
    snake_case(&format!("{kind:?}"))
}

/// **The name a manifest spells one of a station's objects by**, after
/// `fitting/`: the host's own spelling, then the object's.
///
/// Qualified by the station because a binding is `<namespace>/<name>` and
/// has no third part to put the station in — and unqualified names would
/// have fifteen authors racing for the word `counter`. The station half
/// is derived from `Host`'s own `Debug` for the reason [`kind_named`]
/// derives its half: a second table is a table that falls out of step.
#[must_use]
pub fn piece_binding(host: crate::poi::Host, piece: &str) -> String {
    format!("{}_{piece}", snake_case(&format!("{host:?}")))
}

/// **Which station object a `dresses` name means**, or `None` where no
/// station names one.
///
/// Searched rather than parsed on the underscore, because a host's
/// spelling and an object's are both lowercase words joined by
/// underscores and `venus_mirror_frame` could be split two ways. What
/// the stations actually declare is the list, so it is the list that
/// answers.
#[must_use]
pub fn piece_named(name: &str) -> Option<(crate::poi::Host, &'static str)> {
    crate::poi::HOSTS.into_iter().find_map(|host| {
        crate::poi::pieces(host)
            .into_iter()
            .find(|piece| piece_binding(host, piece) == name)
            .map(|piece| (host, piece))
    })
}

/// **One purchased body, as declared**: which kind it dresses, and the
/// four numbers that put it in that kind's box.
#[derive(Clone, Debug, PartialEq)]
pub struct Dressing {
    /// The asset's stable id, for saying which line an answer came from.
    pub id: String,
    /// The converted file, relative to the cache root. Only the index
    /// carries one; a manifest declares no such thing.
    pub glb: Option<String>,
    /// The converted file's own units, in berth half-units.
    pub scale: Vec3,
    /// Where the body's middle sits, in berth half-units.
    pub offset: Vec3,
    /// Degrees about x, then y, then z.
    pub rotation: Vec3,
    /// What fraction of the berth box the body occupies, per axis.
    pub fill: Vec3,
    /// The tight box the converter found round the mesh, in the
    /// converted file's own units. `None` where nothing measured it, in
    /// which case the mesh is placed on its own origin rather than on
    /// its own middle — the only honest thing to do with a body whose
    /// size nobody knows.
    pub measured: Option<(Vec3, Vec3)>,
    /// **The node of the scene that is a door's leaf**, where the module
    /// has one. A kit ships a door as a frame with the leaf a child
    /// object of it, shut over a real opening; naming the node is what
    /// lets one bought frame dress both a shut door and an open doorway.
    /// Read by [`Fabric::Doorway`] alone — a doorway hides it — and
    /// carried, unread, on every other binding.
    pub leaf: Option<String>,
    /// **The node of the scene that is a lamp's glass**, where the
    /// fitting has one. The packs that model a shade hang it as a node
    /// of its own with the lit bulb inside, so naming it is what lets
    /// the cabin draw that node see-through (`pieces::wake_fittings`)
    /// and the light inside it read. Carried, unread, on every binding
    /// that is not a lamp's.
    pub glass: Option<String>,
}

impl Dressing {
    /// **The box a kind's own description claims**, as a `(middle, half)`
    /// in the rig's local sim units: its cells across and up, and the one
    /// cell of depth every rig is composed within.
    ///
    /// This is the same box `pieces::drawn_box` falls back to for a kind
    /// that draws nothing at all, and it is stated once here so a
    /// purchased body and a whitebox one are measured off one claim.
    #[must_use]
    pub fn berth_box(kind: Kind) -> (Vec3, Vec3) {
        let (w, h) = kind.upright();
        (
            Vec3::new(0.0, 0.0, crate::pieces::rig_mid()),
            Vec3::new(
                f32::from(w) * layout::CELL * 0.5,
                f32::from(h) * layout::CELL * 0.5,
                (crate::pieces::RIG_FAR - crate::pieces::RIG_NEAR) * 0.5,
            ),
        )
    }

    /// The turn the declaration asks for.
    #[must_use]
    pub fn turn(&self) -> Quat {
        Quat::from_euler(
            EulerRot::XYZ,
            self.rotation.x.to_radians(),
            self.rotation.y.to_radians(),
            self.rotation.z.to_radians(),
        )
    }

    /// **Where the purchased scene stands**, in the rig's own local sim
    /// units — the transform a `SceneRoot` is spawned under.
    ///
    /// The three overrides, folded into one transform: the mesh is
    /// carried off its own measured middle, scaled out of its own units
    /// into the berth's, turned, and set down at the offset.
    ///
    /// Unused in a whitebox build, which is every build made from this
    /// repository alone: the thing that calls it is `pieces::build_kind`
    /// in the default (art) build, and the numbers it reads come from a file
    /// only a licence holder can write.
    #[must_use]
    #[cfg_attr(feature = "whitebox", allow(dead_code))]
    pub fn pose(&self, kind: Kind) -> Transform {
        let (mid, half) = Self::berth_box(kind);
        self.pose_in(mid, half, Quat::IDENTITY)
    }

    /// **Where the purchased scene stands in an arbitrary frame**: a box
    /// given as its middle and half-extents, turned by `frame` — the
    /// same arithmetic as [`Dressing::pose`], with the berth box handed
    /// in rather than looked up.
    ///
    /// This is what lets one declaration dress many frames. A cargo
    /// kind's frame is its berth box, always the same; a wall panel's
    /// frame is whatever run of wall `room::cladding` hands it, and the
    /// four numbers mean exactly what they mean in a berth: `scale` is
    /// mesh units per frame half-unit, so a panel declared to fill its
    /// frame fills a long wall and a short one alike, stretched to each.
    #[must_use]
    pub fn pose_in(&self, mid: Vec3, half: Vec3, frame: Quat) -> Transform {
        let turn = self.turn();
        let scale = self.scale * self.landing(half);
        let recentre = self.measured.map_or(Vec3::ZERO, |(measured, _)| measured);
        Transform {
            translation: mid + frame * (self.offset * half - turn * (scale * recentre)),
            rotation: frame * turn,
            scale,
        }
    }

    /// **The frame half-extent each of the mesh's own axes lands on**,
    /// once [`Dressing::rotation`] has turned it.
    ///
    /// `scale` is "mesh units per frame half-unit" and the mesh is
    /// scaled along its OWN axes before it is turned, so the half-unit
    /// meant is the one belonging to the frame axis that mesh axis ends
    /// up along. Without the turn those are the same axis and this is
    /// the identity, which is why nothing needed it while every rotated
    /// declaration happened to sit in a frame that was square across the
    /// pair it swapped.
    ///
    /// **A trade room is not square** — 4.4 m across, 3.85 m deep, 2.2 m
    /// high — and a station fitting turned a quarter about `y` therefore
    /// asks for a `fill` on one axis of over twenty, which the resolver
    /// refuses and is right to: `fill` is a fraction of a frame, and a
    /// fraction of the wrong axis is not one. Turning the frame instead
    /// keeps `fill` a fraction of the axis the body actually reaches
    /// along, so `scale = 1 / measured_half` with `fill = [1, 1, 1]`
    /// means "fills its frame" under **any** quarter turn — which is
    /// what the resolver's own advice has always claimed it means.
    ///
    /// Quarter turns permute; anything else blends, which is the same
    /// hair of generosity `fill_box` already takes on a turn that is not
    /// a quarter.
    fn landing(&self, half: Vec3) -> Vec3 {
        (self.turn().inverse() * half).abs()
    }
}

/// **Every kind something is declared to dress**, by kind index.
///
/// An array rather than a map: there are thirty-two kinds, the sweep asks
/// about each of them many times over, and `Kind::index` is already the
/// stable number the save format is written in.
///
/// It is a resource as well as a value, and in the default (art) build the
/// index's own copy is inserted at boot: it is **what this run believes
/// the numbers are**, which is the manifest's numbers as `resolve` last
/// carried them across.
#[derive(Resource, Debug, Default)]
pub struct Dressings {
    by_kind: [Option<Dressing>; KIND_COUNT],
    /// **The shell's dressings**, by room slot and then by role. Slot 0
    /// is the table with no `room` line — every room — and slot `1 + n`
    /// is room kind `n`'s own, which wins where it exists.
    by_fabric: [[Option<Dressing>; FABRIC_COUNT]; ROOM_SLOTS],
    /// **A station's own objects**, by the name a manifest spells them
    /// by ([`piece_binding`]).
    ///
    /// A `Vec` where the other two are arrays, because a piece is named
    /// by a string a station chose rather than by an index the sim
    /// hands out, and because there are a few dozen of them against
    /// thirty-two kinds asked about every frame. The lookup is a scan
    /// and it is done once per room built, not once per body drawn.
    by_piece: Vec<(String, Dressing)>,
    /// Bindings that named a body this game does not have, kept rather
    /// than dropped so a guard can be about them. At runtime they are
    /// simply not drawn.
    pub strangers: Vec<String>,
}

/// The slot a `room` line selects: none is every room's.
const fn room_slot(room: Option<RoomKind>) -> usize {
    match room {
        None => 0,
        Some(kind) => 1 + kind.token() as usize,
    }
}

impl Dressings {
    /// What the kind is dressed in, if anything.
    #[must_use]
    pub const fn of(&self, kind: Kind) -> Option<&Dressing> {
        self.by_kind[kind.index()].as_ref()
    }

    /// **What one role of a room's shell is dressed in**: the room
    /// kind's own table where it has one, the table for every room
    /// otherwise, and nothing where neither was declared.
    ///
    /// The declaration half's answer; the loading half asks its own copy
    /// (`Dressed::of_fabric`) with the scene beside it. Read by the
    /// guards today.
    #[must_use]
    #[allow(dead_code)]
    pub fn of_fabric(&self, room: RoomKind, role: Fabric) -> Option<&Dressing> {
        self.by_fabric[room_slot(Some(room))][role.index()]
            .as_ref()
            .or_else(|| self.by_fabric[room_slot(None)][role.index()].as_ref())
    }

    /// **What one of a station's own objects is dressed in**, by the
    /// name a manifest spells it by ([`piece_binding`]).
    ///
    /// There is no room slot here and there is no fallback: an object
    /// belongs to one station, and a module bought for the Guild's chute
    /// is not a module for anybody else's.
    #[must_use]
    #[allow(dead_code)]
    pub fn of_piece(&self, name: &str) -> Option<&Dressing> {
        self.by_piece
            .iter()
            .find(|(spelled, _)| spelled == name)
            .map(|(_, dressing)| dressing)
    }

    /// Whether anything at all is dressed. The answer is no in every
    /// build this repository can make on its own, and the sweep leans on
    /// that: a manifest with no `dresses` line in it changes nothing.
    /// Read by the guards that hold that claim, and by nothing else.
    #[must_use]
    #[allow(dead_code)]
    pub fn any(&self) -> bool {
        self.by_kind.iter().any(Option::is_some)
            || self
                .by_fabric
                .iter()
                .any(|slot| slot.iter().any(Option::is_some))
            || !self.by_piece.is_empty()
    }

    /// Read a manifest or an index — they are one dialect, and which
    /// keys are present is the only difference between them.
    ///
    /// # Errors
    /// The line that does not read, and why.
    pub fn read(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for table in tables(text)? {
            if table.table != "asset" {
                continue;
            }
            let Some(binding) = table.string("dresses") else {
                continue;
            };
            let mid = table.triple("measured_mid");
            let half = table.triple("measured_half");
            let dressing = Dressing {
                id: table.id.clone(),
                glb: table.string("glb").map(str::to_owned),
                scale: table.triple("scale").unwrap_or(Vec3::ONE),
                offset: table.triple("offset").unwrap_or(Vec3::ZERO),
                rotation: table.triple("rotation").unwrap_or(Vec3::ZERO),
                fill: table.triple("fill").unwrap_or(Vec3::ONE),
                measured: mid.zip(half),
                leaf: table.string("leaf").map(str::to_owned),
                glass: table.string("glass").map(str::to_owned),
            };
            if let Some(name) = binding.strip_prefix("cargo/") {
                let Some(kind) = kind_named(name) else {
                    out.strangers.push(binding.to_owned());
                    continue;
                };
                out.by_kind[kind.index()] = Some(dressing);
            } else if let Some(name) = binding.strip_prefix("fabric/") {
                let Some(role) = fabric_named(name) else {
                    out.strangers.push(binding.to_owned());
                    continue;
                };
                // A `room` line naming a kind this build has not got is
                // a stranger in the same sense: the binding is real and
                // the body it is for is not here.
                let room = if let Some(spelling) = table.string("room") {
                    let Some(kind) = room_named(spelling) else {
                        out.strangers
                            .push(format!("{binding} in room `{spelling}`"));
                        continue;
                    };
                    Some(kind)
                } else {
                    None
                };
                out.by_fabric[room_slot(room)][role.index()] = Some(dressing);
            } else if let Some(name) = binding.strip_prefix("fitting/") {
                // The name is checked against what the stations actually
                // declare, so a `part_of` nobody wrote is a stranger
                // rather than a table that silently never applies.
                if piece_named(name).is_none() {
                    out.strangers.push(binding.to_owned());
                    continue;
                }
                out.by_piece.push((name.to_owned(), dressing));
            } else {
                // A namespace this build has no bodies for. The resolver
                // refuses one it has never heard of; one it knows and
                // this does not is a build that is simply older.
                out.strangers.push(binding.to_owned());
            }
        }
        Ok(out)
    }
}

/// **Which cargo kind a `dresses` name means**, derived from the kind's
/// own spelling rather than looked up in a second table.
///
/// A table would be thirty-two lines that have to be kept in step with
/// `Kind::ALL` by hand, and the day one falls out of step is the day a
/// manifest line silently dresses the wrong crate. `Kind`'s own `Debug`
/// is the spelling, and snake case is the spelling of it a person types.
#[must_use]
pub fn kind_named(name: &str) -> Option<Kind> {
    Kind::ALL.into_iter().find(|kind| snake(*kind) == name)
}

/// One kind's name in a manifest: `VeryMysteriousCrate` as
/// `very_mysterious_crate`.
#[must_use]
pub fn snake(kind: Kind) -> String {
    snake_case(&format!("{kind:?}"))
}

/// A `Debug` spelling in snake case: the one derivation behind every
/// name the manifest spells.
fn snake_case(camel: &str) -> String {
    let mut out = String::new();
    for letter in camel.chars() {
        if letter.is_ascii_uppercase() && !out.is_empty() {
            out.push('_');
        }
        out.push(letter.to_ascii_lowercase());
    }
    out
}

// ------------------------------------------------------------ the dialect --

/// One `[table.id]` and the keys under it.
///
/// The reader is here rather than shared with `xtask` because the two
/// crates cannot see each other — the resolver has no dependencies at all
/// and the cabin has Bevy — and because eighty lines of reader is a
/// smaller thing to carry than a TOML crate in either graph. The dialect
/// is documented once, in `xtask/src/manifest.rs`.
#[allow(clippy::struct_field_names)]
struct Table {
    table: String,
    id: String,
    keys: Vec<(String, Value)>,
}

enum Value {
    Str(String),
    Triple(Vec3),
}

impl Table {
    fn get(&self, key: &str) -> Option<&Value> {
        self.keys
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, v)| v)
    }

    fn string(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(value) => Some(value),
            Value::Triple(_) => None,
        }
    }

    fn triple(&self, key: &str) -> Option<Vec3> {
        match self.get(key)? {
            Value::Triple(value) => Some(*value),
            Value::Str(_) => None,
        }
    }
}

fn tables(text: &str) -> Result<Vec<Table>, String> {
    let mut out: Vec<Table> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('[') {
            let header = header
                .strip_suffix(']')
                .ok_or_else(|| format!("{line}: `{trimmed}` never closes its table"))?;
            let (table, id) = header
                .split_once('.')
                .ok_or_else(|| format!("{line}: `[{header}]` is not `[<table>.<id>]`"))?;
            out.push(Table {
                table: table.to_owned(),
                id: id.to_owned(),
                keys: Vec::new(),
            });
            continue;
        }
        let (key, rest) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("{line}: `{trimmed}` is not a `key = value`"))?;
        let holder = out
            .last_mut()
            .ok_or_else(|| format!("{line}: `{trimmed}` comes before any table"))?;
        let value = read_value(rest.trim()).map_err(|why| format!("{line}: {why}"))?;
        holder.keys.push((key.trim().to_owned(), value));
    }
    Ok(out)
}

fn read_value(text: &str) -> Result<Value, String> {
    if let Some(rest) = text.strip_prefix('"') {
        let end = rest
            .find('"')
            .ok_or_else(|| format!("`{text}` never closes its string"))?;
        return Ok(Value::Str(rest[..end].to_owned()));
    }
    let rest = text
        .strip_prefix('[')
        .ok_or_else(|| format!("`{text}` is neither a string nor three numbers"))?;
    let end = rest
        .find(']')
        .ok_or_else(|| format!("`{text}` never closes its array"))?;
    let numbers: Vec<f32> = rest[..end]
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .map_err(|_| format!("`{}` is not a number", part.trim()))
        })
        .collect::<Result<_, _>>()?;
    let triple: [f32; 3] = numbers
        .try_into()
        .map_err(|got: Vec<f32>| format!("`{text}` has {} numbers, not three", got.len()))?;
    Ok(Value::Triple(Vec3::from(triple)))
}

// ------------------------------------------------------- the loading half --

#[cfg(not(feature = "whitebox"))]
pub use loading::{Clad, Dressed, Fitted, Glazed, Open, Worn, cache_root, plugin};

#[cfg(not(feature = "whitebox"))]
mod loading {
    use std::path::PathBuf;

    use bevy::prelude::*;
    use bevy::world_serialization::WorldAsset;
    use space_trucking::sim::Kind;
    use space_trucking::sim::cargo::KIND_COUNT;

    use space_trucking::sim::room::{ROOM_KINDS, RoomKind};

    use super::{Dressing, Dressings, FABRIC_COUNT, Fabric, ROOM_SLOTS, room_slot};
    use crate::Phase;
    use crate::outline::{MaskBody, MaskProxy};

    /// **Where the resolved art is**: `$ART_CACHE` if it is set, and
    /// `art/cache` beside wherever the game was started otherwise —
    /// exactly the two places `cargo xtask art resolve` writes to.
    ///
    /// The answer comes back absolute, and has to: this path becomes the
    /// asset server's root, and Bevy resolves a *relative* root against
    /// `BEVY_ASSET_ROOT` or `CARGO_MANIFEST_DIR` — under `cargo run`,
    /// this crate's directory — never the working directory. A relative
    /// `art/cache` here and the resolver's `art/cache` beside the repo
    /// root would name two different places while spelled identically.
    #[must_use]
    pub fn cache_root() -> PathBuf {
        let root = std::env::var_os("ART_CACHE")
            .map_or_else(|| PathBuf::from("art").join("cache"), PathBuf::from);
        std::path::absolute(&root).unwrap_or(root)
    }

    /// **Where this run's resolved art is.** A resource rather than a
    /// call to [`cache_root`] at the point of use, because a guard
    /// cannot set an environment variable: this workspace forbids
    /// `unsafe`, `std::env::set_var` is unsafe, and a seam nothing can
    /// point at a fixture is a seam nothing can test.
    #[derive(Resource, Debug, Clone)]
    pub struct Cache(pub PathBuf);

    impl Default for Cache {
        fn default() -> Self {
            Self(cache_root())
        }
    }

    /// **What the game will draw instead of the whitebox**: one loaded
    /// scene per dressed kind, with the declaration that places it.
    ///
    /// The resource is inserted whether or not anything resolved, so the
    /// systems that read it need no `Option` and the "no art on this
    /// machine" case is an empty table rather than an absent one.
    /// A loaded scene and the declaration that places it.
    type Bought = (Handle<WorldAsset>, Dressing);

    #[derive(Resource, Default)]
    pub struct Dressed {
        scenes: [Option<Bought>; KIND_COUNT],
        /// The shell's, by room slot and role — `Dressings::by_fabric`
        /// with the loaded scene beside each declaration.
        fabric: [[Option<Bought>; FABRIC_COUNT]; ROOM_SLOTS],
        /// A station's own objects, by the name a manifest spells them
        /// by — `Dressings::by_piece` with the loaded scene beside each.
        pieces: Vec<(String, Bought)>,
    }

    impl Dressed {
        /// The scene and the numbers for one kind, or nothing — which is
        /// the answer for every kind in a build with no cache under it.
        #[must_use]
        pub fn of(&self, kind: Kind) -> Option<(&Handle<WorldAsset>, &Dressing)> {
            self.scenes[kind.index()]
                .as_ref()
                .map(|(scene, dressing)| (scene, dressing))
        }

        /// The scene and the numbers for one role of one room kind's
        /// shell: the kind's own table where it has one, every room's
        /// otherwise, nothing where neither resolved.
        #[must_use]
        pub fn of_fabric(
            &self,
            room: RoomKind,
            role: Fabric,
        ) -> Option<(&Handle<WorldAsset>, &Dressing)> {
            self.fabric[room_slot(Some(room))][role.index()]
                .as_ref()
                .or_else(|| self.fabric[room_slot(None)][role.index()].as_ref())
                .map(|(scene, dressing)| (scene, dressing))
        }

        /// The scene and the numbers for one of a station's own
        /// objects, by the name a manifest spells it by
        /// ([`super::piece_binding`]).
        #[must_use]
        pub fn of_piece(&self, name: &str) -> Option<(&Handle<WorldAsset>, &Dressing)> {
            self.pieces
                .iter()
                .find(|(spelled, _)| spelled == name)
                .map(|(_, (scene, dressing))| (scene, dressing))
        }

        /// **Every scene this run asked for**, cargo, shell and station
        /// furniture alike — what a screenshot waits on before it fires.
        pub fn scenes(&self) -> impl Iterator<Item = &Handle<WorldAsset>> {
            self.scenes
                .iter()
                .chain(self.fabric.iter().flatten())
                .flatten()
                .chain(self.pieces.iter().map(|(_, bought)| bought))
                .map(|(scene, _)| scene)
        }

        /// **Put one station object's scene and numbers in.**
        pub fn furnish(&mut self, name: String, scene: Handle<WorldAsset>, dressing: Dressing) {
            self.pieces.push((name, (scene, dressing)));
        }

        /// **Put one shell role's scene and numbers in**, for one room
        /// kind or for every room.
        pub fn clad(
            &mut self,
            room: Option<RoomKind>,
            role: Fabric,
            scene: Handle<WorldAsset>,
            dressing: Dressing,
        ) {
            self.fabric[room_slot(room)][role.index()] = Some((scene, dressing));
        }

        /// **Put one kind's scene and numbers in.** [`load_index`] fills
        /// the table this way as it reads, and so does a guard that
        /// needs a dressed kind without a cache on the disk under it —
        /// the same reason [`Cache`] is a resource rather than a call.
        pub fn dress(&mut self, kind: Kind, scene: Handle<WorldAsset>, dressing: Dressing) {
            self.scenes[kind.index()] = Some((scene, dressing));
        }
    }

    /// **A body of a purchased scene that has not been spoken for**: a
    /// mesh, not already marked, and not one of the mask's own copies.
    type Unmarked = (With<Mesh3d>, Without<MaskBody>, Without<MaskProxy>);

    /// **A purchased body, as it stands in the world.** Put on the
    /// entity `pieces::build_kind` spawns the scene under, which is the
    /// only handle anything downstream has on a drawn mesh: a
    /// `WorldAssetRoot` is otherwise indistinguishable from any other
    /// child of a rig.
    #[derive(Component, Clone, Copy, Debug)]
    pub struct Worn(#[allow(dead_code)] pub Kind);

    /// **A purchased panel of a room's shell, as it stands in the
    /// world.** The counterpart of [`Worn`] for the fabric: put on the
    /// entity `room::rebuild` spawns a module's scene under, so what a
    /// room is clad in can be found — and so the outline,
    /// which queries [`Worn`], never mistake a wall for a piece of cargo.
    #[derive(Component, Clone, Copy, Debug)]
    #[allow(dead_code)]
    pub struct Clad(pub Fabric);

    /// **A purchased piece of a station's furniture, as it stands in
    /// the world**, by the name a manifest spells it by. The
    /// counterpart of [`Worn`] and [`Clad`] for `fitting/` — so a
    /// station's bought chute is as findable as a bought crate, and so
    /// the outline, which queries [`Worn`], never mistake
    /// somebody's furniture for cargo.
    #[derive(Component, Clone, Debug)]
    #[allow(dead_code)]
    pub struct Fitted(pub String);

    /// **A bought door frame standing in a mated doorway**, with the name
    /// of the node in its scene that is the leaf. Put beside [`Clad`] on
    /// the root of a `fabric/doorway` module whose declaration carries a
    /// `leaf` line, and read by [`open_doors`], which hides that node
    /// once the scene has landed.
    ///
    /// The state is the role's, not the module's: the same file is a
    /// shut door in a `fabric/door` frame and an open one here, and what
    /// differs is exactly this component being on the root.
    #[derive(Component, Clone, Debug)]
    pub struct Open {
        /// The node to hide, by its glTF name.
        pub leaf: String,
        /// Whether the scene has arrived and been looked through once —
        /// so a doorway with no such node is said on stderr once rather
        /// than every frame.
        looked: bool,
    }

    impl Open {
        /// A doorway to draw open, by the name of its leaf.
        #[must_use]
        pub const fn new(leaf: String) -> Self {
            Self {
                leaf,
                looked: false,
            }
        }
    }

    /// **A bought lamp whose shade is a node of its own**, with the
    /// name of that node. Put beside [`Worn`] on the root by
    /// `pieces::build_kind` when the binding carries a `glass` line, and
    /// read by `pieces::wake_fittings`, which draws that node
    /// see-through once the scene has landed — the same shape as
    /// [`Open`], for the same reason: the scene arrives frames after
    /// its root, and a name can only meet a file that is there.
    #[derive(Component, Clone, Debug)]
    pub struct Glazed {
        /// The asset id, for saying which line named a node that is
        /// not in its file.
        pub what: String,
        /// The node to draw as glass, by its glTF name.
        pub node: String,
        /// Whether the scene has arrived and been looked through once —
        /// so a lamp with no such node is said on stderr once rather
        /// than every frame.
        pub looked: bool,
    }

    impl Glazed {
        /// A lamp whose glass is the node so named, in the asset so
        /// named.
        #[must_use]
        pub const fn new(what: String, node: String) -> Self {
            Self {
                what,
                node,
                looked: false,
            }
        }
    }

    /// Read the index at boot and ask for every scene it names.
    ///
    /// **Every way this can go wrong ends in the whitebox.** No cache
    /// directory, no index, an index that will not parse, an entry
    /// naming a file that is not there: each one leaves the kind
    /// undressed and puts a sentence on stderr. The game is playable
    /// without any of this and always will be — that is the whole point
    /// of cutting the geometry in code — so nothing here is worth a
    /// panic, and nothing here is worth a word on screen either.
    pub fn plugin(app: &mut App) {
        app.init_resource::<Cache>()
            .init_resource::<Dressed>()
            .init_resource::<Dressings>()
            .add_systems(Startup, load_index)
            // **Before the pass that reads the mark, and that is a claim
            // rather than a tidiness.** A body that appeared during this
            // frame's `SpawnScene` is marked here and outlined by
            // `paint` in the same frame; unordered, which of the two
            // frames the line first showed up in would be the
            // scheduler's to pick, and what the mask draws is not a
            // thing this game lets a thread win a race over.
            .add_systems(
                Update,
                mask_dressed
                    .in_set(Phase::View)
                    .before(crate::outline::paint),
            )
            .add_systems(Update, open_doors.in_set(Phase::View));
    }

    pub(super) fn load_index(
        cache: Res<Cache>,
        assets: Res<AssetServer>,
        mut dressed: ResMut<Dressed>,
        mut declared: ResMut<Dressings>,
    ) {
        let index = cache.0.join("index.toml");
        let Ok(text) = std::fs::read_to_string(&index) else {
            eprintln!(
                "art: no {} — drawing the whitebox. `cargo xtask art resolve` writes one.",
                index.display()
            );
            return;
        };
        let read = match Dressings::read(&text) {
            Ok(read) => read,
            Err(why) => {
                eprintln!(
                    "art: {} does not read ({why}) — drawing the whitebox. It is written \
                     by `cargo xtask art resolve` and rewritten by the next one.",
                    index.display()
                );
                return;
            }
        };
        for stranger in &read.strangers {
            eprintln!(
                "art: {} dresses `{stranger}`, which this build has no body for",
                index.display()
            );
        }
        for kind in Kind::ALL {
            let Some(dressing) = read.of(kind) else {
                continue;
            };
            let Some(glb) = &dressing.glb else {
                eprintln!(
                    "art: `{}` dresses {kind:?} and names no converted file — drawing the \
                     whitebox",
                    dressing.id
                );
                continue;
            };
            // The cache root is the asset root under this feature, so a
            // path out of the index is a path the server can take
            // verbatim. `#Scene0` is `GltfAssetLabel::Scene(0)`, glTF's
            // first scene, which is the one a single-object export has.
            dressed.dress(kind, assets.load(format!("{glb}#Scene0")), dressing.clone());
        }
        // The shell's, slot by slot: the table for every room and then
        // each kind's own, each asked directly rather than through the
        // fallback so a kind without a table of its own loads nothing
        // twice.
        for (slot, roles) in read.by_fabric.iter().enumerate() {
            let room = (slot > 0).then(|| ROOM_KINDS[slot - 1]);
            for role in Fabric::ALL {
                let Some(dressing) = &roles[role.index()] else {
                    continue;
                };
                let Some(glb) = &dressing.glb else {
                    eprintln!(
                        "art: `{}` dresses fabric/{} and names no converted file — drawing \
                         the whitebox",
                        dressing.id,
                        role.name()
                    );
                    continue;
                };
                dressed.clad(
                    room,
                    role,
                    assets.load(format!("{glb}#Scene0")),
                    dressing.clone(),
                );
            }
        }
        // The stations' own furniture. One flat list, because a piece
        // is named by a station and not indexed by the sim.
        for (name, dressing) in &read.by_piece {
            let Some(glb) = &dressing.glb else {
                eprintln!(
                    "art: `{}` dresses fitting/{name} and names no converted file —                      drawing the whitebox",
                    dressing.id
                );
                continue;
            };
            dressed.furnish(
                name.clone(),
                assets.load(format!("{glb}#Scene0")),
                dressing.clone(),
            );
        }
        // The numbers, kept where something with no asset server can
        // read them.
        *declared = read;
    }

    /// **Carry a rig's mask onto the bodies a purchased mesh arrives
    /// as**, so the outline follows a bought silhouette the way it
    /// follows a cut one.
    ///
    /// A whitebox part is marked in the breath it is spawned in
    /// (`pieces::RigParts::mask`), because the part is right there. A
    /// dressed kind has nothing to mark at that moment: `build_kind`
    /// spawns a `WorldAssetRoot` and the meshes under it appear frames
    /// later, when the loader has read the file and the spawner has
    /// copied the scene into the world. So the root wears the mark and
    /// this hands it down.
    ///
    /// **Only the identity travels, and that is the whole reason this is
    /// four lines rather than a second selection system.** What a piece
    /// is WEARING is on no component at all — `outline::paint` works the
    /// code out afresh every frame from the sim, the pointer and the
    /// carry, and paints the proxy with it — so a body carrying the
    /// right piece number follows every reading of that piece for
    /// nothing: the aim arriving, the room's claim lighting, the x-ray
    /// ghosting, all of it, with nothing here to keep in step.
    ///
    /// It re-walks rather than waiting on an event, and cheaply: the
    /// only roots it looks at are dressed ones, and a purchased prop is
    /// a handful of nodes. What that buys is a mark that survives
    /// everything the spawner does on its own — a scene that lands late,
    /// a hot reload that despawns the bodies and copies fresh ones in,
    /// a rig respawned by `sync_pieces` under a piece that had already
    /// loaded.
    ///
    /// **The copies are not bodies**, and skipping them is load-bearing
    /// rather than tidy: `outline::paint` cuts each mask proxy as a
    /// CHILD of the body it copies, so a proxy is a `Mesh3d` descendant
    /// of this root like any other. Mark one and it becomes a body with
    /// a copy of its own, every frame, forever.
    pub(super) fn mask_dressed(
        mut commands: Commands,
        dressed: Query<(Entity, &MaskBody), With<Worn>>,
        kin: Query<&Children>,
        bare: Query<(), Unmarked>,
    ) {
        for (root, mark) in &dressed {
            for part in kin.iter_descendants(root) {
                if bare.contains(part) {
                    commands.entity(part).insert(MaskBody::of(mark.piece()));
                }
            }
        }
    }

    /// **Draw every open doorway open**: hide the leaf of each bought
    /// door frame standing in a mated doorway.
    ///
    /// The frame arrives as a scene, frames after its root was spawned,
    /// so this walks the root's descendants rather than acting at spawn
    /// — and re-walks every frame for the reason [`mask_dressed`] does:
    /// a scene that lands late or is copied in fresh by a hot reload
    /// gets its leaf hidden again for nothing. The walk is a handful of
    /// door roots with a couple of nodes each.
    ///
    /// **A doorway with no such node is said once and drawn as it
    /// came.** The manifest names the leaf and the resolver checks the
    /// line's shape and never opens the mesh, so the first place the
    /// name meets the file is here; a frame whose leaf is misspelled is
    /// a doorway drawn shut, which is the one thing a `leaf` line exists
    /// to prevent, and it is worth a sentence on stderr.
    pub(super) fn open_doors(
        mut doorways: Query<(Entity, &mut Open, &Name)>,
        kin: Query<&Children>,
        mut nodes: Query<(&Name, &mut Visibility)>,
    ) {
        for (root, mut open, what) in &mut doorways {
            let mut landed = false;
            let mut hidden = 0;
            for node in kin.iter_descendants(root) {
                landed = true;
                let Ok((name, mut visibility)) = nodes.get_mut(node) else {
                    continue;
                };
                if name.as_str() != open.leaf {
                    continue;
                }
                hidden += 1;
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
            if !landed || open.looked {
                continue;
            }
            open.looked = true;
            if hidden == 0 {
                eprintln!(
                    "art: `{what}` is drawn open, and its scene has no node called `{}` to \
                     hide — the doorway keeps its leaf. The manifest's `leaf` line names a \
                     node of the mesh; `cargo xtask art dex` lists what a file is made of.",
                    open.leaf
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A unit cube as a binary glTF**, written here byte by byte.
    ///
    /// There is no purchased mesh in this repository and there never will
    /// be, so the only way to prove that the loading path loads anything
    /// is to write a file it has to accept. A `.glb` is a twelve-byte
    /// header and two chunks — the JSON that describes the scene, and the
    /// buffer the vertex data lives in — and a cube with positions and
    /// indices is the smallest thing that exercises the whole of it:
    /// header, chunk framing, accessors, a buffer view, a mesh, a node
    /// and a scene.
    ///
    /// Byte by byte rather than through a crate for the reason the zip
    /// fixture in `xtask/tests/pipeline.rs` is: a fixture built by the
    /// same library the code under test uses proves that the library
    /// agrees with itself.
    #[cfg(not(feature = "whitebox"))]
    fn unit_cube_glb() -> Vec<u8> {
        cube_glb("\"scenes\":[{\"nodes\":[0]}],\"nodes\":[{\"mesh\":0}],")
    }

    /// **A door frame as a binary glTF**: one cube named for the frame
    /// with a second cube named for the leaf hung under it, which is the
    /// shape every `SM_Bld_Wall_Doorframe_0N` in the Synty kit arrives
    /// in — a frame with a real opening and the leaf a CHILD object
    /// sitting shut in it.
    ///
    /// The whole of what `art::Open` is about is that shape, so the
    /// fixture has to have it. A single-node cube would let the pass
    /// pass by finding nothing and hiding nothing.
    #[cfg(not(feature = "whitebox"))]
    fn door_frame_glb() -> Vec<u8> {
        cube_glb(
            "\"scenes\":[{\"nodes\":[0]}],\
             \"nodes\":[{\"name\":\"SM_Frame\",\"mesh\":0,\"children\":[1]},\
             {\"name\":\"SM_Leaf\",\"mesh\":0}],",
        )
    }

    /// The cube above, in whatever scene and node graph is handed in.
    #[cfg(not(feature = "whitebox"))]
    fn cube_glb(scene: &str) -> Vec<u8> {
        // Eight corners of a cube half a unit each way, so the tight box
        // round it is exactly `[-0.5, 0.5]` — the number the index
        // fixture below declares it measured.
        // Twelve triangles, two per face, wound so the outside faces
        // out. The corner index is a bit per axis: 1 is +x, 2 is +y,
        // 4 is +z.
        const FACES: [[u16; 4]; 6] = [
            [0, 2, 3, 1], // -z
            [5, 7, 6, 4], // +z
            [4, 6, 2, 0], // -x
            [1, 3, 7, 5], // +x
            [0, 1, 5, 4], // -y
            [2, 6, 7, 3], // +y
        ];
        let mut bin: Vec<u8> = Vec::new();
        for z in [-0.5_f32, 0.5] {
            for y in [-0.5_f32, 0.5] {
                for x in [-0.5_f32, 0.5] {
                    for value in [x, y, z] {
                        bin.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        for [a, b, c, d] in FACES {
            for corner in [a, b, c, a, c, d] {
                bin.extend_from_slice(&corner.to_le_bytes());
            }
        }
        let positions = 8 * 3 * 4;
        let indices = bin.len() - positions;
        let json = format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,{scene}\
             \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\
             \"indices\":1}}]}}],\
             \"accessors\":[\
             {{\"bufferView\":0,\"componentType\":5126,\"count\":8,\"type\":\"VEC3\",\
             \"min\":[-0.5,-0.5,-0.5],\"max\":[0.5,0.5,0.5]}},\
             {{\"bufferView\":1,\"componentType\":5123,\"count\":36,\"type\":\"SCALAR\"}}],\
             \"bufferViews\":[\
             {{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{positions},\"target\":34962}},\
             {{\"buffer\":0,\"byteOffset\":{positions},\"byteLength\":{indices},\
             \"target\":34963}}],\
             \"buffers\":[{{\"byteLength\":{}}}]}}",
            bin.len()
        );
        // Both chunks are padded to four bytes — JSON with spaces and the
        // buffer with zeros, which is what the specification asks for and
        // what every reader checks.
        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2_u32.to_le_bytes());
        let total = 12 + 8 + json.len() + 8 + bin.len();
        out.extend_from_slice(&u32::try_from(total).expect("a small fixture").to_le_bytes());
        out.extend_from_slice(&u32::try_from(json.len()).expect("small").to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&u32::try_from(bin.len()).expect("small").to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    /// **The smallest app that can read a glTF**, pointed at one cache.
    ///
    /// A task pool for the load to run on, the asset server, the two
    /// asset kinds a mesh becomes, and the loader itself. `finish` is
    /// not optional — `GltfPlugin` only registers its loader there, and
    /// an app that never finishes waits for a loader that was never
    /// installed.
    #[cfg(not(feature = "whitebox"))]
    fn stand(root: &std::path::Path) -> App {
        use bevy::asset::AssetPlugin;
        use bevy::world_serialization::WorldSerializationPlugin;

        let mut app = App::new();
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            AssetPlugin {
                file_path: root.display().to_string(),
                ..default()
            },
            WorldSerializationPlugin,
            bevy::mesh::MeshPlugin,
            bevy::gltf::GltfPlugin::default(),
        ))
        .insert_resource(super::loading::Cache(root.to_path_buf()));
        plugin(&mut app);
        app.finish();
        app.cleanup();
        app
    }

    /// **A converted mesh in the cache is loaded, and the whitebox is
    /// what happens when it is not.**
    ///
    /// The end of the loading path, proved against a file this test
    /// wrote. What it establishes is exactly three things and no more:
    /// that the feature's Bevy list can decode a binary glTF at all, that
    /// the index this repository's own resolver writes is read back into
    /// a handle for the kind it names, and that every way the cache can
    /// be missing or wrong leaves the kind undressed instead of bringing
    /// the game down.
    ///
    /// What it does not prove, and cannot: that a real Synty FBX comes
    /// out of Blender looking right. That needs the owner's disk, and
    /// `docs/ART_PIPELINE.md` says which command closes it.
    #[cfg(not(feature = "whitebox"))]
    #[test]
    fn a_converted_mesh_in_the_cache_is_what_a_dressed_kind_draws() {
        use bevy::asset::LoadState;
        use bevy::world_serialization::WorldAsset;

        let dir = std::env::temp_dir().join("space-trucking-art-cabin-load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("glb")).expect("a scratch cache");
        std::fs::write(dir.join("glb/cube.glb"), unit_cube_glb()).expect("a fixture mesh");
        std::fs::write(
            dir.join("index.toml"),
            "[asset.crate_small]\nglb = \"glb/cube.glb\"\n\
             sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
             dresses = \"cargo/suspicious_crate\"\n\
             scale = [2.0, 2.0, 2.0]\noffset = [0.0, -0.5, 0.0]\n\
             rotation = [0.0, 90.0, 0.0]\nfill = [1.0, 1.0, 1.0]\n\
             measured_mid = [0.0, 0.0, 0.0]\nmeasured_half = [0.5, 0.5, 0.5]\n",
        )
        .expect("a fixture index");

        let mut app = stand(&dir);
        app.update();
        let dressed = app.world().resource::<Dressed>();
        let (handle, dressing) = dressed
            .of(Kind::SuspiciousCrate)
            .expect("the index dresses the suspicious crate");
        let handle = handle.clone();
        // The numbers came out of the index and not out of a default,
        // and the pose the scene is spawned under is the one they make:
        // twice the berth box, a quarter turn about its own up, and half
        // a half-box down — which is what `build_kind` hands the
        // `WorldAssetRoot` it spawns in place of the whitebox parts.
        //
        // Twice the berth box READ ALONG THE AXES THE TURNED MESH LANDS
        // ON (`Dressing::landing`): `scale` counts mesh units per frame
        // half-unit along the mesh's own axes, and a quarter turn about
        // up carries mesh x onto frame z and mesh z onto frame x. This
        // berth is two cells across and one deep, so the two readings
        // differ, and the one that fills the berth is the landed one.
        assert_eq!(dressing.scale, Vec3::splat(2.0));
        assert_eq!(dressing.rotation, Vec3::new(0.0, 90.0, 0.0));
        let pose = dressing.pose(Kind::SuspiciousCrate);
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        let landed = Vec3::new(half.z, half.y, half.x);
        assert!(
            (pose.scale - landed * 2.0).length() < 1e-4,
            "{:?} is not twice the berth half-extents the turned mesh lands on, {:?}",
            pose.scale,
            landed * 2.0
        );
        assert!(
            (pose.translation - (mid - Vec3::Y * (half.y * 0.5))).length() < 1e-4,
            "{:?}",
            pose.translation
        );
        assert!(
            (pose.rotation * Vec3::Z - Vec3::X).length() < 1e-4,
            "a quarter turn about up did not carry the body's face onto +x"
        );
        assert!(dressed.of(Kind::Couch).is_none(), "an unnamed kind is bare");

        // Asset loading is asynchronous, so the frames are pumped until
        // the server has an answer either way rather than once.
        let mut state = LoadState::NotLoaded;
        for _ in 0..10_000 {
            app.update();
            state = app
                .world()
                .resource::<AssetServer>()
                .get_load_state(&handle)
                .unwrap_or(LoadState::NotLoaded);
            if matches!(state, LoadState::Loaded | LoadState::Failed(_)) {
                break;
            }
        }
        assert!(
            matches!(state, LoadState::Loaded),
            "the cabin could not load a glTF it wrote itself: {state:?}"
        );
        assert_eq!(
            app.world().resource::<Assets<WorldAsset>>().len(),
            1,
            "the scene the mesh describes did not become an asset"
        );

        // And every way the cache can be absent or wrong leaves the kind
        // undressed rather than bringing the game down.
        for (what, root) in [
            ("a cache that is not there", dir.join("nowhere")),
            ("a cache with no index in it", dir.join("glb")),
        ] {
            let mut app = stand(&root);
            app.update();
            assert!(
                app.world()
                    .resource::<Dressed>()
                    .of(Kind::SuspiciousCrate)
                    .is_none(),
                "{what} dressed something"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every entity under `root`, however deep, that carries this name,
    /// with what it is drawn as. The leaf of a door frame is a node of a
    /// loaded scene, so nothing here may assume a depth.
    #[cfg(not(feature = "whitebox"))]
    fn named_under(app: &App, root: Entity, name: &str) -> Vec<Visibility> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(at) = stack.pop() {
            let entity = app.world().entity(at);
            if let Some(kids) = entity.get::<Children>() {
                stack.extend(kids.iter());
            }
            if at != root
                && entity.get::<Name>().is_some_and(|had| had.as_str() == name)
                && let Some(shown) = entity.get::<Visibility>()
            {
                out.push(*shown);
            }
        }
        out
    }

    /// **A doorway drawn open hides its leaf, and a shut door keeps
    /// it — from one mesh, in one box.**
    ///
    /// This is the whole mechanism, and it is worth saying what it
    /// replaced. A shut door used to be one purchased mesh and a mated
    /// doorway a DIFFERENT one, in a different frame, because this
    /// repository had measured the kit's door panels as one body each
    /// and concluded they were sealed. They are not: the opening is
    /// there and the leaf is a child object hung in it. So the two
    /// states are now one module whose leaf comes and goes, and a door
    /// that is shut on one dock and open on the next no longer swaps the
    /// wall for a different wall.
    ///
    /// What is proved here is the runtime half — that the pass finds a
    /// node by the name the manifest gave it and hides that node and
    /// nothing else. The description half, that both states are one box,
    /// is `room::tests::the_cladding_tiles_every_wall_and_leaves_each_open_doorway_open`,
    /// which runs in the build with no art on it.
    #[cfg(not(feature = "whitebox"))]
    #[test]
    fn a_doorway_drawn_open_hides_its_leaf_and_a_shut_door_keeps_it() {
        use bevy::asset::LoadState;

        let dir = std::env::temp_dir().join("space-trucking-art-open-doors");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("glb")).expect("a scratch cache");
        std::fs::write(dir.join("glb/door.glb"), door_frame_glb()).expect("a fixture mesh");
        // Both roles, one mesh, the same four numbers: the `leaf` line is
        // the only difference between them, exactly as the shipped
        // manifest has it.
        std::fs::write(
            dir.join("index.toml"),
            "[asset.door_frame]\nglb = \"glb/door.glb\"\ndresses = \"fabric/door\"\n\
             scale = [1.0, 1.0, 1.0]\noffset = [0.0, 0.0, 0.0]\nfill = [1.0, 1.0, 1.0]\n\
             [asset.door_frame_open]\nglb = \"glb/door.glb\"\ndresses = \"fabric/doorway\"\n\
             leaf = \"SM_Leaf\"\n\
             scale = [1.0, 1.0, 1.0]\noffset = [0.0, 0.0, 0.0]\nfill = [1.0, 1.0, 1.0]\n",
        )
        .expect("a fixture index");

        let mut app = stand(&dir);
        app.update();
        let (handle, mouth) = app
            .world()
            .resource::<Dressed>()
            .of_fabric(RoomKind::Cabin, Fabric::Doorway)
            .expect("the index dresses a doorway");
        let (handle, leaf) = (handle.clone(), mouth.leaf.clone());
        assert_eq!(
            leaf.as_deref(),
            Some("SM_Leaf"),
            "the leaf did not survive the index"
        );
        assert_eq!(
            app.world()
                .resource::<Dressed>()
                .of_fabric(RoomKind::Cabin, Fabric::Door)
                .and_then(|(_, shut)| shut.leaf.clone()),
            None,
            "a shut door was given a leaf to hide"
        );
        for _ in 0..10_000 {
            app.update();
            match app
                .world()
                .resource::<AssetServer>()
                .get_load_state(&handle)
                .unwrap_or(LoadState::NotLoaded)
            {
                LoadState::Loaded => break,
                LoadState::Failed(_) => panic!("the cabin could not load a glTF it wrote itself"),
                _ => {}
            }
        }

        // One of each, spawned the way `room::clad` spawns them.
        let open = app
            .world_mut()
            .spawn((
                bevy::world_serialization::WorldAssetRoot(handle.clone()),
                Transform::default(),
                Visibility::default(),
                Clad(Fabric::Doorway),
                Name::new("seam[0] mouth"),
                Open::new(leaf.expect("the leaf read above")),
            ))
            .id();
        let shut = app
            .world_mut()
            .spawn((
                bevy::world_serialization::WorldAssetRoot(handle),
                Transform::default(),
                Visibility::default(),
                Clad(Fabric::Door),
                Name::new("shut[1] door"),
            ))
            .id();
        // The scene lands in `SpawnScene`, after this frame's `Update`,
        // so the pass cannot reach it before the frame after. Pumped
        // rather than counted, like the mask's.
        for _ in 0..16 {
            app.update();
        }

        let hidden = named_under(&app, open, "SM_Leaf");
        assert_eq!(
            hidden.len(),
            1,
            "the fixture scene put {} leaves under the open doorway, so this guard asks \
             nothing — the glTF node's name did not reach the world",
            hidden.len()
        );
        assert_eq!(
            hidden[0],
            Visibility::Hidden,
            "a mated doorway is drawing its leaf: the door is open and the door is shut"
        );
        assert!(
            named_under(&app, open, "SM_Frame")
                .iter()
                .all(|shown| *shown != Visibility::Hidden),
            "hiding the leaf took the frame with it, and the doorway is a hole in the wall"
        );
        let kept = named_under(&app, shut, "SM_Leaf");
        assert_eq!(kept.len(), 1, "the shut door lost its leaf node");
        assert!(
            kept[0] != Visibility::Hidden,
            "a shut door is drawing no leaf, which is a doorway nobody can close"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A shell binding is read into its role and its room, and the
    /// room's own table wins over the table for every room.** The
    /// fallback is the whole reason a manifest can dress every wall in
    /// one line and then give the furnace its own; a fallback that did
    /// not fall back, or a kind's table that did not win, would each be
    /// a colour quietly wrong in one room.
    /// **A `glass` line is read where it is named and absent where it
    /// is not**, and a lamp's glass reaches the kind's dressing whole:
    /// the node name is what `pieces::wake_fittings` draws see-through,
    /// and a name that did not survive the read is a shade drawn opaque
    /// over a lit bulb.
    #[test]
    fn a_glass_line_is_read_where_it_is_named() {
        let declared = Dressings::read(
            "[asset.sconce]\ndresses = \"cargo/wall_lamp\"\nglb = \"glb/a.glb\"\n\
             glass = \"SM_Prop_Lighting_Wall_Glass_05\"\n\
             [asset.column]\ndresses = \"cargo/floor_lamp\"\nglb = \"glb/b.glb\"\n",
        )
        .expect("the dialect");
        assert_eq!(
            declared
                .of(Kind::WallLamp)
                .and_then(|one| one.glass.as_deref()),
            Some("SM_Prop_Lighting_Wall_Glass_05")
        );
        assert_eq!(
            declared
                .of(Kind::FloorLamp)
                .and_then(|one| one.glass.as_deref()),
            None,
            "a lamp with no glass line was handed somebody else's node"
        );
    }

    #[test]
    fn a_fabric_binding_is_read_by_role_and_by_room() {
        let declared = Dressings::read(
            "[asset.panel]\ndresses = \"fabric/wall\"\nglb = \"glb/a.glb\"\n\
             [asset.panel_hot]\ndresses = \"fabric/wall\"\nroom = \"burner\"\n\
             glb = \"glb/b.glb\"\n\
             [asset.tile]\ndresses = \"fabric/floor\"\nglb = \"glb/c.glb\"\n\
             [asset.odd]\ndresses = \"fabric/wall\"\nroom = \"attic\"\n\
             [asset.odder]\ndresses = \"fabric/gable\"\n\
             [asset.mouth]\ndresses = \"fabric/doorway\"\nglb = \"glb/d.glb\"\n\
             leaf = \"SM_Bld_Wall_Door_01\"\n",
        )
        .expect("the dialect");
        // A leaf is read where it is named and absent where it is not.
        assert_eq!(
            declared
                .of_fabric(RoomKind::Cabin, Fabric::Doorway)
                .and_then(|one| one.leaf.as_deref()),
            Some("SM_Bld_Wall_Door_01")
        );
        assert_eq!(
            declared
                .of_fabric(RoomKind::Cabin, Fabric::Wall)
                .and_then(|one| one.leaf.as_deref()),
            None
        );
        assert_eq!(
            declared
                .of_fabric(RoomKind::Cabin, Fabric::Wall)
                .map(|one| one.id.as_str()),
            Some("panel")
        );
        assert_eq!(
            declared
                .of_fabric(RoomKind::Burner, Fabric::Wall)
                .map(|one| one.id.as_str()),
            Some("panel_hot")
        );
        assert_eq!(
            declared
                .of_fabric(RoomKind::Burner, Fabric::Floor)
                .map(|one| one.id.as_str()),
            Some("tile")
        );
        assert_eq!(
            declared
                .of_fabric(RoomKind::Trade, Fabric::Door)
                .map(|one| one.id.as_str()),
            None
        );
        assert!(declared.any());
        assert_eq!(
            declared.strangers,
            vec!["fabric/wall in room `attic`", "fabric/gable"]
        );
        // Every role and every room round-trips its own spelling.
        for role in Fabric::ALL {
            assert_eq!(fabric_named(role.name()), Some(role));
        }
        for kind in ROOM_KINDS {
            assert_eq!(room_named(&room_snake(kind)), Some(kind), "{kind:?}");
        }
        assert_eq!(room_named("Burner"), None);
    }

    /// **A frame turns the declaration with it.** The same numbers that
    /// put a body in a berth put a panel in a wall; the wall's frame is
    /// turned to face the room, and a mesh's own offset has to turn with
    /// it or a panel nudged "into the wall" on the aft wall would move
    /// sideways on the starboard one.
    #[test]
    fn a_pose_in_a_turned_frame_carries_the_offset_round_with_it() {
        let declared = Dressings::read(
            "[asset.panel]\ndresses = \"fabric/wall\"\nglb = \"glb/a.glb\"\n\
             measured_mid = [0.0, 0.0, 0.0]\nmeasured_half = [1.0, 1.0, 1.0]\n\
             offset = [0.0, 0.0, 0.5]\n",
        )
        .expect("the dialect");
        let one = declared
            .of_fabric(RoomKind::Cabin, Fabric::Wall)
            .expect("a dressing");
        let mid = Vec3::new(3.0, 1.0, -2.0);
        let half = Vec3::new(2.0, 1.5, 0.25);
        let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let pose = one.pose_in(mid, half, quarter);
        // Half the frame's depth along the frame's own +z, which the
        // quarter turn has pointed down world +x.
        let expected = mid + quarter * Vec3::new(0.0, 0.0, 0.5 * half.z);
        assert!((pose.translation - expected).length() < 1e-5, "{pose:?}");
        assert!((pose.scale - half).length() < 1e-5, "{:?}", pose.scale);
        assert!((pose.rotation.dot(quarter).abs() - 1.0).abs() < 1e-5);
        // And the berth pose is the same arithmetic with the berth box in.
        let cargo = Dressings::read(
            "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n\
             glb = \"glb/abc.glb\"\nmeasured_mid = [0.0, 0.0, 0.0]\n\
             measured_half = [1.0, 1.0, 1.0]\n",
        )
        .expect("the dialect");
        let crate_small = cargo.of(Kind::SuspiciousCrate).expect("a dressing");
        let (bmid, bhalf) = Dressing::berth_box(Kind::SuspiciousCrate);
        let a = crate_small.pose(Kind::SuspiciousCrate);
        let b = crate_small.pose_in(bmid, bhalf, Quat::IDENTITY);
        assert!((a.translation - b.translation).length() < 1e-6);
        assert!((a.scale - b.scale).length() < 1e-6);
    }

    /// **A binding that names nothing is caught, and a binding that
    /// names something is read.** The non-vacuity of the guard above:
    /// the shipped manifest cannot exercise either branch, so a manifest
    /// written here does.
    #[test]
    fn a_binding_naming_no_body_this_game_has_is_a_stranger() {
        let one = |dresses: &str| {
            Dressings::read(&format!(
                "[asset.crate_small]\nsource = \"a.fbx\"\ndresses = \"{dresses}\"\n"
            ))
            .expect("a manifest in the dialect")
        };
        let stranger = one("cargo/crate_of_holding");
        assert_eq!(stranger.strangers, vec!["cargo/crate_of_holding"]);
        assert!(!stranger.any());

        let known = one("cargo/suspicious_crate");
        assert!(known.strangers.is_empty(), "{:?}", known.strangers);
        assert_eq!(
            known.of(Kind::SuspiciousCrate).map(|one| one.id.as_str()),
            Some("crate_small")
        );

        // A real namespace and an object no station declares. `beacon`
        // is short a station: every piece is spelled `<host>_<piece>`,
        // so a bare object name can never be one and this is also the
        // shape a build older than the station that named it reads.
        assert_eq!(one("fitting/beacon").strangers, vec!["fitting/beacon"]);

        // A namespace nothing here has ever had, which a build older
        // than a namespace still has to survive reading.
        assert_eq!(one("rigging/mast").strangers, vec!["rigging/mast"]);
        // And the shell's namespace, which this build does have.
        let clad = one("fabric/wall");
        assert!(clad.strangers.is_empty(), "{:?}", clad.strangers);
        assert!(clad.any());
    }

    /// **A kind's manifest name is its own spelling, in snake case.**
    /// The mapping is derived and not tabled, so this asks the derivation
    /// about the shapes it has to get right: a two-word name, a
    /// three-word one, and a one-word one.
    #[test]
    fn a_kinds_name_is_its_own_spelling() {
        assert_eq!(snake(Kind::PerfumeVial), "perfume_vial");
        assert_eq!(snake(Kind::VeryMysteriousCrate), "very_mysterious_crate");
        assert_eq!(snake(Kind::Couch), "couch");
        assert_eq!(kind_named("bay_window"), Some(Kind::BayWindow));
        assert_eq!(kind_named("Couch"), None);
        // Every kind round-trips, so no two of them can collide either.
        for kind in Kind::ALL {
            assert_eq!(kind_named(&snake(kind)), Some(kind), "{kind:?}");
        }
    }

    /// **A mesh whose origin is not its middle is carried onto its
    /// middle.** A Synty prop often sits on its own base rather than in
    /// the centre of its own bounds, and `offset = [0, 0, 0]` has to
    /// mean "centred in its berth" for the containment arithmetic above
    /// it to mean anything.
    #[test]
    fn a_mesh_is_placed_on_its_own_middle_and_not_on_its_origin() {
        let declared = Dressings::read(
            "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n\
             glb = \"glb/abc.glb\"\nmeasured_mid = [0.0, 0.5, 0.0]\n\
             measured_half = [1.0, 0.5, 1.0]\nfill = [1.0, 0.5, 1.0]\n",
        )
        .expect("the dialect");
        let one = declared.of(Kind::SuspiciousCrate).expect("a dressing");
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        let pose = one.pose(Kind::SuspiciousCrate);
        // The mesh's own middle is half a unit up its own y, so the pose
        // pulls it back down by that much in berth units.
        assert!((pose.translation.y - 0.5f32.mul_add(-half.y, mid.y)).abs() < 1e-4);
        assert!((pose.translation.x - mid.x).abs() < 1e-4);
    }
}
