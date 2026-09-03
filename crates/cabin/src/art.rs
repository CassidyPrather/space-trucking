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
//! how it sits in that body's box. Those are *promises*, they live in
//! git, and the gauntlet sweeps them in continuous integration
//! (`crate::gauntlet`, and docs/GAUNTLET.md's fill family). This has to
//! work in the default build, because **the build that can draw the mesh
//! is the build continuous integration cannot run**: the payload is not
//! in the repository and never will be. So the promise is what CI can
//! check, and CI checks it whether or not the feature is on.
//!
//! **The loading half is behind `--features art`.** That is where
//! `bevy_gltf` comes in, and where `$ART_CACHE/index.toml`
//! — written by `cargo xtask art resolve` on the machine of somebody who
//! holds the licence — is read at boot. Everything about it fails soft:
//! no cache, no entry, an entry that will not parse, a file that is not
//! there, and the whitebox stands in, because a game that will not start
//! because somebody moved a directory is worse than a game with grey
//! boxes in it. What it never does is put a word on the screen; the
//! zero-text law covers what is drawn, and a complaint belongs on stderr.
//!
//! **And there is a writer**, which is neither half and belongs to both:
//! [`rewritten`] sets some keys in one asset's table and leaves every
//! other byte of the manifest as it found it. It is here rather than
//! with the thing that calls it (`crate::nudge`, the placement bench)
//! because it is the reader above, backwards — one dialect, one file,
//! one place to keep them in step, and one guard that reads back what it
//! wrote through the reader itself. It is compiled in every build for
//! the reason the declaration half is: what it is about is the file, and
//! the file is in the repository whether or not a build can draw what it
//! names.
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
//! here — a body that leaves its berth is a finding, and the gauntlet's
//! families already know how to say so.
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

use std::sync::OnceLock;

use bevy::prelude::*;
use space_trucking::sim::cargo::KIND_COUNT;
use space_trucking::sim::{Kind, layout};

/// The manifest as it stands in the repository, read at compile time.
///
/// Compiled in rather than read off a disk because the thing that needs
/// it most is a sweep that runs in continuous integration on a machine
/// with no art on it, and because a promise that can go missing between
/// the build and the run is not a promise. Editing the manifest rebuilds
/// the cabin, which is correct: the docket depends on it.
const SHIPPED: &str = include_str!("../../../art/manifest.toml");

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
    /// under `--features art`, and the numbers it reads come from a file
    /// only a licence holder can write.
    #[must_use]
    #[cfg_attr(not(feature = "art"), allow(dead_code))]
    pub fn pose(&self, kind: Kind) -> Transform {
        let (mid, half) = Self::berth_box(kind);
        let turn = self.turn();
        let scale = self.scale * half;
        let recentre = self.measured.map_or(Vec3::ZERO, |(measured, _)| measured);
        Transform {
            translation: mid + self.offset * half - turn * (scale * recentre),
            rotation: turn,
            scale,
        }
    }

    /// **The box the harness measures and the aim meets**, as a
    /// `(middle, half)` in the rig's own local sim units.
    ///
    /// The declared body, not the frame it is drawn in — which is the
    /// whole of what `fill` is for. The turn is carried onto it the same
    /// way `pieces::drawn_box` carries a part's: the axis-aligned bound
    /// of the turned box, which is exact on a quarter turn and a hair
    /// generous otherwise.
    #[must_use]
    pub fn fill_box(&self, kind: Kind) -> (Vec3, Vec3) {
        let (mid, half) = Self::berth_box(kind);
        let body = (self.fill * half).abs();
        let m = Mat3::from_quat(self.turn());
        let reach = m.x_axis.abs() * body.x + m.y_axis.abs() * body.y + m.z_axis.abs() * body.z;
        (mid + self.offset * half, reach)
    }
}

/// **Every kind something is declared to dress**, by kind index.
///
/// An array rather than a map: there are thirty-two kinds, the sweep asks
/// about each of them many times over, and `Kind::index` is already the
/// stable number the save format is written in.
///
/// It is a resource as well as a value, and under `--features art` the
/// index's own copy is inserted at boot: it is **what this run believes
/// the numbers are**, which is the manifest's numbers as `resolve` last
/// carried them across. The bench (`crate::nudge`) moves that belief
/// when it writes, so a body let go of after a save does not spring back
/// to what the index said an hour ago.
#[derive(Resource, Debug, Default)]
pub struct Dressings {
    by_kind: [Option<Dressing>; KIND_COUNT],
    /// Bindings that named a body this game does not have, kept rather
    /// than dropped so a guard can be about them. At runtime they are
    /// simply not drawn.
    pub strangers: Vec<String>,
}

impl Dressings {
    /// What the kind is dressed in, if anything.
    #[must_use]
    pub const fn of(&self, kind: Kind) -> Option<&Dressing> {
        self.by_kind[kind.index()].as_ref()
    }

    /// **Move what this run believes**, for the one caller that has
    /// grounds to: the bench, which has just written the same numbers
    /// into the file the belief came from.
    #[cfg_attr(not(feature = "art"), allow(dead_code))]
    pub fn dress(&mut self, kind: Kind, dressing: Dressing) {
        self.by_kind[kind.index()] = Some(dressing);
    }

    /// Whether anything at all is dressed. The answer is no in every
    /// build this repository can make on its own, and the sweep leans on
    /// that: a manifest with no `dresses` line in it changes nothing.
    /// Read by the guards that hold that claim, and by nothing else.
    #[must_use]
    #[allow(dead_code)]
    pub fn any(&self) -> bool {
        self.by_kind.iter().any(Option::is_some)
    }

    /// **What the manifest in this repository declares.** Parsed once.
    ///
    /// A manifest that will not parse answers as an empty set rather than
    /// panicking. The resolver is where a malformed manifest is a
    /// refusal, with the line in it; this is a reader, and a reader that
    /// brings the game down over a file the game does not need is a
    /// worse citizen than one that draws the whitebox.
    #[must_use]
    pub fn shipped() -> &'static Self {
        static ONCE: OnceLock<Dressings> = OnceLock::new();
        ONCE.get_or_init(|| Self::read(SHIPPED).unwrap_or_default())
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
            let Some(name) = binding.strip_prefix("cargo/") else {
                // A namespace this build has no bodies for. The resolver
                // refuses one it has never heard of; one it knows and
                // this does not is a build that is simply older.
                out.strangers.push(binding.to_owned());
                continue;
            };
            let Some(kind) = kind_named(name) else {
                out.strangers.push(binding.to_owned());
                continue;
            };
            let mid = table.triple("measured_mid");
            let half = table.triple("measured_half");
            out.by_kind[kind.index()] = Some(Dressing {
                id: table.id.clone(),
                glb: table.string("glb").map(str::to_owned),
                scale: table.triple("scale").unwrap_or(Vec3::ONE),
                offset: table.triple("offset").unwrap_or(Vec3::ZERO),
                rotation: table.triple("rotation").unwrap_or(Vec3::ZERO),
                fill: table.triple("fill").unwrap_or(Vec3::ONE),
                measured: mid.zip(half),
            });
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
    let mut out = String::new();
    for letter in format!("{kind:?}").chars() {
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

// -------------------------------------------------------- writing back --

/// **Where the manifest is**: `$ART_MANIFEST` if it is set, and
/// `art/manifest.toml` beside wherever the game was started otherwise.
///
/// The same two places `cargo xtask art` looks, said the same way — a
/// bench that wrote numbers into one file while the resolver read
/// another would be a bench that silently did nothing. The relative
/// fallback is [`cache_root`]'s: a running game has no repository root
/// to search from, only a working directory.
#[must_use]
#[cfg_attr(not(feature = "art"), allow(dead_code))]
pub fn manifest_path() -> std::path::PathBuf {
    std::env::var_os("ART_MANIFEST").map_or_else(
        || std::path::PathBuf::from("art").join("manifest.toml"),
        std::path::PathBuf::from,
    )
}

/// **Set some keys in one asset's table, and change nothing else.**
///
/// The manifest is the owner's file. It is nine tenths prose — which
/// pack, which path, why the number is what it is — and a writer that
/// round-tripped it through a parser would hand back a file with the
/// argument deleted and the tables in whatever order a map iterated.
/// So this is a **line edit**: the table's own lines are found, the
/// named keys' values are replaced where the line already exists and a
/// line is added where it does not, and **every other byte of the file
/// comes out the way it went in** — comments, blank lines, spacing
/// round the `=`, the order tables stand in, even the line endings,
/// which are carried through rather than normalised.
///
/// What is *not* preserved is the value that was asked to change, which
/// is the whole point, and a trailing comment on such a line survives
/// the change because the owner wrote it about the key rather than
/// about the number.
///
/// # Errors
/// An id the manifest does not carry, said plainly. Nothing is written
/// in that case, which is why this hands back a string rather than
/// touching the file itself.
#[cfg_attr(not(feature = "art"), allow(dead_code))]
pub fn rewritten(text: &str, id: &str, set: &[(&str, Vec3)]) -> Result<String, String> {
    // Lines with their own terminators still attached, so a file that
    // arrived with CRLF leaves with CRLF and a file with no newline at
    // the end does not grow one.
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    if span(&lines, id).is_none() {
        return Err(format!(
            "no `[asset.{id}]` in the manifest — nothing was written"
        ));
    }
    for (key, value) in set {
        // Re-found per key, because adding a line moves every index
        // after it and an index computed once would be a stale index.
        let Some((head, end)) = span(&lines, id) else {
            unreachable!("the table was there a moment ago")
        };
        let rendered = triple(*value);
        if let Some(at) = (head + 1..end).find(|at| key_of(&lines[*at]) == Some(*key)) {
            lines[at] = set_value(&lines[at], &rendered);
            continue;
        }
        // A key the table never had joins the block of keys rather than
        // landing after whatever prose stands between this table and
        // the next: the last line that sets something is where the
        // owner would have typed it.
        let after = (head + 1..end)
            .rev()
            .find(|at| key_of(&lines[*at]).is_some())
            .unwrap_or(head);
        let ending = terminator(&lines[after]).to_owned();
        if ending.is_empty() {
            // The file ended without a newline. The new line needs one
            // in front of it, and takes the ending nothing else has.
            lines[after].push('\n');
        }
        lines.insert(after + 1, format!("{key} = {rendered}{ending}"));
    }
    Ok(lines.concat())
}

/// **Write one asset's numbers into a manifest on disk.** The read, the
/// edit and the write, with the refusal in the middle: a manifest that
/// does not carry the id is never opened for writing.
///
/// A save that would change nothing writes nothing, so a bench somebody
/// leans on the save key in does not churn a file's timestamp.
///
/// # Errors
/// A file that cannot be read or written, and every refusal
/// [`rewritten`] makes.
#[cfg_attr(not(feature = "art"), allow(dead_code))]
pub fn save_into(path: &std::path::Path, id: &str, set: &[(&str, Vec3)]) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|why| format!("{} cannot be read ({why})", path.display()))?;
    let out = rewritten(&text, id, set)?;
    if out == text {
        return Ok(());
    }
    std::fs::write(path, out).map_err(|why| format!("{} cannot be written ({why})", path.display()))
}

/// Where one `[asset.<id>]` table's lines start and stop: the header's
/// own index, and one past the last line before the next header.
fn span(lines: &[String], id: &str) -> Option<(usize, usize)> {
    let head = lines
        .iter()
        .position(|line| header_of(line) == Some(("asset", id)))?;
    let end = lines[head + 1..]
        .iter()
        .position(|line| header_of(line).is_some())
        .map_or(lines.len(), |at| head + 1 + at);
    Some((head, end))
}

/// The `[table.id]` a line opens, if it opens one. Read the way
/// [`tables`] reads it, so a header this cannot see is a header the
/// game cannot see either.
fn header_of(line: &str) -> Option<(&str, &str)> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (table, id) = inner.split_once('.')?;
    Some((table.trim(), id.trim()))
}

/// The key a line sets, if it sets one.
fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    Some(trimmed.split_once('=')?.0.trim())
}

/// Whatever ends a line, so an edited line ends the same way.
fn terminator(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// One key line with a new value in it. Everything around the value is
/// the owner's — the indentation, the key's own spelling, the spacing on
/// both sides of the `=`, and whatever trails the closing bracket, which
/// is where a comment about the line lives.
fn set_value(line: &str, rendered: &str) -> String {
    let (left, rest) = line.split_once('=').unwrap_or((line, ""));
    let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let tail = rest.find(']').map_or_else(
        || terminator(line).to_owned(),
        |at| rest[at + 1..].to_owned(),
    );
    format!("{left}={}{rendered}{tail}", &rest[..gap])
}

/// Three numbers as the manifest writes them: `[1.0, 1.0, 1.0]`.
fn triple(value: Vec3) -> String {
    format!(
        "[{}, {}, {}]",
        number(value.x),
        number(value.y),
        number(value.z)
    )
}

/// One number, in the style the file is already written in.
///
/// Rust's own `Display` writes `1` for `1.0`, and a per-axis value
/// spelled `[1, 1, 1]` invites the reader to think it is an integer
/// count of something. This is `xtask`'s own rule for the same reason,
/// restated here rather than shared for the reason the reader above is
/// restated: the two crates cannot see each other, and the dialect is
/// cheaper to say twice than to depend on.
fn number(value: f32) -> String {
    // A minus sign in front of a zero somebody nudged back to the middle
    // is a diff nobody wants to read.
    let value = if value == 0.0 { 0.0 } else { value };
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}

// ------------------------------------------------------- the loading half --

#[cfg(feature = "art")]
pub use loading::{Dressed, Worn, cache_root, plugin};

#[cfg(feature = "art")]
mod loading {
    use std::path::PathBuf;

    use bevy::prelude::*;
    use bevy::world_serialization::WorldAsset;
    use space_trucking::sim::Kind;
    use space_trucking::sim::cargo::KIND_COUNT;

    use super::{Dressing, Dressings};
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
    #[derive(Resource, Default)]
    pub struct Dressed {
        scenes: [Option<(Handle<WorldAsset>, Dressing)>; KIND_COUNT],
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
    /// child of a rig. The bench moves these and nothing else.
    #[derive(Component, Clone, Copy, Debug)]
    pub struct Worn(pub Kind);

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
            );
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
        // The numbers, kept where something with no asset server can
        // read them: the bench reads this and never touches a handle,
        // which is what lets a scripted session drive it with no window,
        // no cache and no mesh.
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
    #[cfg(feature = "art")]
    fn unit_cube_glb() -> Vec<u8> {
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
            "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\
             \"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],\
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
    #[cfg(feature = "art")]
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

    /// **A cache holding the cube above, dressing one kind**, written
    /// into a scratch directory of its own. The index is the dialect
    /// `cargo xtask art resolve` writes, because the loading path under
    /// test is the one that reads what the resolver wrote.
    #[cfg(feature = "art")]
    fn a_cache(name: &str, kind: Kind) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("glb")).expect("a scratch cache");
        std::fs::write(dir.join("glb/cube.glb"), unit_cube_glb()).expect("a fixture mesh");
        std::fs::write(
            dir.join("index.toml"),
            format!(
                "[asset.bought]\nglb = \"glb/cube.glb\"\ndresses = \"cargo/{}\"\n\
                 scale = [1.0, 1.0, 1.0]\noffset = [0.0, 0.0, 0.0]\n\
                 fill = [1.0, 1.0, 1.0]\nmeasured_mid = [0.0, 0.0, 0.0]\n\
                 measured_half = [0.5, 0.5, 0.5]\n",
                snake(kind)
            ),
        )
        .expect("a fixture index");
        dir
    }

    /// Pump frames until the cache's one scene has loaded, and hand back
    /// the handle `build_kind` would spawn. Asset loading is
    /// asynchronous, so the frames are counted rather than assumed.
    #[cfg(feature = "art")]
    fn loaded(app: &mut App, kind: Kind) -> Handle<bevy::world_serialization::WorldAsset> {
        use bevy::asset::LoadState;

        app.update();
        let handle = app
            .world()
            .resource::<Dressed>()
            .of(kind)
            .expect("the index dresses the kind it was written for")
            .0
            .clone();
        for _ in 0..10_000 {
            app.update();
            let state = app
                .world()
                .resource::<AssetServer>()
                .get_load_state(&handle)
                .unwrap_or(LoadState::NotLoaded);
            match state {
                LoadState::Loaded => return handle,
                LoadState::Failed(_) => panic!("the cabin could not load a glTF it wrote itself"),
                _ => {}
            }
        }
        panic!("the fixture glTF never finished loading");
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
    #[cfg(feature = "art")]
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
        assert_eq!(dressing.scale, Vec3::splat(2.0));
        assert_eq!(dressing.rotation, Vec3::new(0.0, 90.0, 0.0));
        let pose = dressing.pose(Kind::SuspiciousCrate);
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        assert!(
            (pose.scale - half * 2.0).length() < 1e-4,
            "{:?}",
            pose.scale
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

    /// Every `Mesh3d` standing under one entity, however deep. The rig
    /// root's own child is a scene root, whose child is a node, whose
    /// child is the body — so nothing here may assume a depth.
    ///
    /// A mask copy is not a body, and skipping it is not tidiness:
    /// `outline::paint` hangs each copy off the body it copies, so once
    /// the outline has said anything about this piece the copies are
    /// `Mesh3d` descendants of the same root. Count them as bodies and
    /// this would agree with a propagation that masked them — which is
    /// the propagation that gives every copy a copy of its own, one a
    /// frame, forever.
    #[cfg(feature = "art")]
    fn bodies_under(app: &App, root: Entity) -> Vec<Entity> {
        use crate::outline::MaskProxy;

        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(at) = stack.pop() {
            let entity = app.world().entity(at);
            if let Some(kids) = entity.get::<Children>() {
                stack.extend(kids.iter());
            }
            if at != root && entity.contains::<Mesh3d>() && !entity.contains::<MaskProxy>() {
                out.push(at);
            }
        }
        out.sort_unstable();
        out
    }

    /// **A purchased body wears the mask its whitebox parts would.**
    ///
    /// The defect this is written against was visible the first day a
    /// real Synty mesh stood in the cabin: the crate could be aimed at,
    /// picked up and put down, and no line was ever drawn round it. A
    /// whitebox part is marked as it is spawned, because it IS spawned
    /// there; a dressed kind hands `build_kind` a scene that has not
    /// loaded, and the meshes it becomes turn up frames later with
    /// nothing to tell them whose body they are.
    ///
    /// So: a real converted file, loaded through the real loader, spawned
    /// through the real `WorldAssetRoot`, and the mark has to be on every
    /// body that comes out of it — with the piece's own number, which is
    /// the whole of what `outline` needs to draw everything it can say
    /// about that piece.
    ///
    /// **Two rigs, two numbers, and a third that is nobody's**, because
    /// the failure worth catching is not "no mark" but "the wrong one":
    /// a mark copied from whichever dressed root the query reached first
    /// would outline one crate when the crosshair rested on another.
    #[cfg(feature = "art")]
    #[test]
    fn a_dressed_body_wears_the_mask_its_whitebox_parts_would() {
        use crate::outline::MaskBody;

        let dir = a_cache("space-trucking-art-mask-dressed", Kind::SuspiciousCrate);
        let mut app = stand(&dir);
        let scene = loaded(&mut app, Kind::SuspiciousCrate);

        // Two dressed rigs, exactly as `build_kind` spawns one: the
        // scene, the pose, the handle the bench moves, and the mark.
        let dressed: Vec<(u32, Entity)> = [4_100_u32, 4_101]
            .into_iter()
            .map(|piece| {
                let root = app
                    .world_mut()
                    .spawn((
                        bevy::world_serialization::WorldAssetRoot(scene.clone()),
                        Transform::default(),
                        Visibility::default(),
                        Worn(Kind::SuspiciousCrate),
                        MaskBody::of(piece),
                    ))
                    .id();
                (piece, root)
            })
            .collect();
        // And a rig wearing no purchased body at all: a marked root with
        // a bare mesh under it, which is what a kind the manifest does
        // not dress looks like from here. Nothing may reach it.
        let bare_root = app
            .world_mut()
            .spawn((Transform::default(), MaskBody::of(4_102)))
            .id();
        let bare_body = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::default()),
                Transform::default(),
                ChildOf(bare_root),
            ))
            .id();

        // The scene spawner runs in `SpawnScene`, after this frame's
        // `Update`, so the mark cannot land before the frame after the
        // bodies do. Pumped rather than counted: what is asserted is
        // that it lands, not which frame the engine chose.
        for _ in 0..16 {
            app.update();
        }

        for (piece, root) in dressed {
            let bodies = bodies_under(&app, root);
            assert!(
                !bodies.is_empty(),
                "the fixture scene put no body under piece {piece}, so this guard asks nothing"
            );
            for body in bodies {
                let mark = app
                    .world()
                    .entity(body)
                    .get::<MaskBody>()
                    .unwrap_or_else(|| {
                        panic!(
                            "a body of piece {piece} carries no mask — a dressed kind that \
                             selects and never wears the line"
                        )
                    });
                assert_eq!(
                    mark.piece(),
                    piece,
                    "a body of piece {piece} is marked as somebody else's"
                );
            }
        }
        assert!(
            app.world().entity(bare_body).get::<MaskBody>().is_none(),
            "an undressed rig's body was marked by the dressing pass"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What every mask copy in the world is wearing this frame, by the
    /// body it is a copy of: whether it is drawn, and in which ink.
    #[cfg(feature = "art")]
    fn worn(app: &mut App) -> Vec<(Entity, Visibility, AssetId<crate::outline::MaskInk>)> {
        use crate::outline::{MaskInk, MaskProxy};

        let mut out: Vec<(Entity, Visibility, AssetId<MaskInk>)> = app
            .world_mut()
            .query::<(&MaskProxy, &Visibility, &MeshMaterial3d<MaskInk>)>()
            .iter(app.world())
            .map(|(proxy, shown, ink)| (proxy.part, *shown, ink.0.id()))
            .collect();
        out.sort_by_key(|(part, _, _)| *part);
        out
    }

    /// What the piece's whitebox part is wearing, out of one frame's
    /// copies: the answer every purchased body of that piece has to
    /// match, and the reason nothing here has to name a code.
    #[cfg(feature = "art")]
    fn answer(
        worn: &[(Entity, Visibility, AssetId<crate::outline::MaskInk>)],
        whitebox: Entity,
    ) -> (Entity, Visibility, AssetId<crate::outline::MaskInk>) {
        worn.iter()
            .find(|(part, _, _)| *part == whitebox)
            .copied()
            .expect("the whitebox part of the piece is copied like any other")
    }

    /// **A body on the fixture board the crosshair can rest on, and
    /// that nothing else is saying anything about.**
    ///
    /// Searched rather than written down, for the reason the bench's own
    /// harness searches for its spot: the fixture is re-dressed whenever
    /// the cargo tables change, and a coordinate spelled out here would
    /// quietly stop being the coordinate. The two filters are what make
    /// the reading below exactly one thing — a piece the room has
    /// claimed wears the claim as well, and a piece with a carry handle
    /// answers the aim in two halves depending on where in it the
    /// crosshair lands.
    #[cfg(feature = "art")]
    fn a_plain_body(
        sim: &space_trucking::sim::Sim,
    ) -> (space_trucking::sim::cargo::Piece, space_trucking::sim::Vec2) {
        use space_trucking::sim::layout;

        let lit = crate::room::lit_footprints(sim);
        for piece in sim.pieces() {
            if lit.iter().any(|(id, _)| *id == piece.id)
                || crate::pieces::carry_handle(piece.kind).is_some()
            {
                continue;
            }
            let rect = layout::piece_rect(sim.rooms(), sim.pieces(), piece);
            let at = space_trucking::sim::Vec2::new(
                rect.w.mul_add(0.5, rect.x),
                rect.h.mul_add(0.5, rect.y),
            );
            if layout::piece_at(sim.rooms(), sim.pieces(), at).map(|found| found.id)
                == Some(piece.id)
            {
                return (*piece, at);
            }
        }
        panic!("the fixture board has no plain body the crosshair can rest on");
    }

    /// **The line a purchased body wears is the line its whitebox would
    /// have worn, and it follows the reading as the reading changes.**
    ///
    /// The other half of the mark, and the one that says why the mark is
    /// all there is to carry. What a piece is WEARING lives on no
    /// component: `outline::paint` works the code out afresh every frame
    /// off the sim, the pointer and the carry, and hands it to the ink
    /// each copy is painted with. So a dressed body that carries the
    /// right piece number follows every reading of that piece for
    /// nothing — and the way to prove it is not to name the codes, which
    /// are that module's business, but to stand a whitebox part of the
    /// SAME piece beside the purchased one and demand they wear the same
    /// ink in every frame.
    ///
    /// Three readings, because a guard that only ever saw one would pass
    /// on a mark that had been frozen at the first thing it was told:
    /// the piece flown through, the piece under the crosshair, and the
    /// piece nobody is saying anything about at all.
    ///
    /// And the count is asserted, which is the other law this frame can
    /// break: a copy is a `Mesh3d` child of the body it copies, so a
    /// propagation that marked one would hand every copy a copy of its
    /// own, one a frame, until the world would not fit in memory. One
    /// per body, and no more.
    #[cfg(feature = "art")]
    #[test]
    fn the_line_on_a_dressed_body_is_the_line_its_whitebox_would_wear() {
        use crate::bridge::{Bridge, FrameOutcome};
        use crate::outline::{Ghosts, MaskBody, MaskInk, MaskInks};
        use crate::rig::CameraRig;
        use crate::surface::VirtualPointer;
        use crate::{Phase, Shell};

        let mut bridge = Bridge::boot_fixture(crate::fixture::SAVE);
        bridge.steady();
        let (piece, aimed) = a_plain_body(&bridge.sim);

        let dir = a_cache("space-trucking-art-mask-reading", piece.kind);
        let mut app = stand(&dir);
        let scene = loaded(&mut app, piece.kind);
        app.init_asset::<MaskInk>();
        let inks = {
            let mut assets = app.world_mut().resource_mut::<Assets<MaskInk>>();
            MaskInks::new(&mut assets)
        };
        app.insert_resource(Shell {
            bridge,
            outcome: FrameOutcome::default(),
            muted: false,
        })
        .insert_resource(CameraRig::boot(None))
        .insert_resource(inks)
        .init_resource::<VirtualPointer>()
        .init_resource::<Ghosts>()
        .configure_sets(Update, Phase::View)
        .add_systems(Update, crate::outline::paint.in_set(Phase::View));

        // One piece, drawn twice: the purchased body `build_kind` would
        // spawn under the feature, and a whitebox part of the same piece
        // standing beside it as the answer key.
        let dressed = app
            .world_mut()
            .spawn((
                bevy::world_serialization::WorldAssetRoot(scene),
                Transform::default(),
                Visibility::default(),
                Worn(piece.kind),
                MaskBody::of(piece.id),
            ))
            .id();
        let whitebox = app
            .world_mut()
            .spawn((
                Mesh3d(Handle::default()),
                Transform::default(),
                Visibility::default(),
                MaskBody::of(piece.id),
            ))
            .id();

        // **Flown through.** The x-ray names the piece, nothing else
        // does, and the pointer is parked off the board.
        app.world_mut().resource_mut::<Ghosts>().0 = vec![piece.id];
        for _ in 0..16 {
            app.update();
        }
        let bodies = bodies_under(&app, dressed);
        assert!(
            !bodies.is_empty(),
            "the fixture scene put no body under the dressed rig"
        );
        let ghosting = worn(&mut app);
        assert_eq!(
            ghosting.len(),
            bodies.len() + 1,
            "one copy per body, and no more: {ghosting:?}"
        );
        let key = answer(&ghosting, whitebox);
        assert_eq!(key.1, Visibility::Visible, "the answer key is not drawn");
        for (part, shown, ink) in &ghosting {
            assert_eq!(*shown, key.1, "a copy of {part} is not drawn with the rest");
            assert_eq!(
                *ink, key.2,
                "a copy of {part} wears another piece's reading"
            );
        }

        // **Under the crosshair.** A different reading of the same
        // piece, and every copy has to move with it in the same frame.
        app.world_mut().resource_mut::<Ghosts>().0.clear();
        app.world_mut().resource_mut::<VirtualPointer>().sim = aimed;
        app.update();
        let aiming = worn(&mut app);
        let hovered = answer(&aiming, whitebox);
        assert_ne!(
            hovered.2, key.2,
            "the aim and the x-ray are two readings and must not be one ink"
        );
        for (part, shown, ink) in &aiming {
            assert_eq!(*shown, Visibility::Visible, "a copy of {part} went dark");
            assert_eq!(*ink, hovered.2, "a copy of {part} kept the reading it had");
        }

        // **Nothing said about it at all**, which is the reading the
        // cabin spends most of its frames in: every copy goes away.
        app.world_mut().resource_mut::<VirtualPointer>().sim = crate::bridge::POINTER_PARKED;
        app.update();
        for (part, shown, _) in worn(&mut app) {
            assert_eq!(
                shown,
                Visibility::Hidden,
                "a copy of {part} is still drawn with nothing said about its piece"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The half that masks a purchased body is the gated one.**
    ///
    /// The whitebox build has no cache to read, no scene to spawn, and
    /// therefore no mark to carry down onto one — and "therefore" is a
    /// claim about where the code is, so it is held where the code is.
    /// The declaration half above the gate is compiled into every build
    /// this repository makes; it reads a file in git and answers
    /// questions about promises. The moment a line of it reached the
    /// outline, the whitebox path would have grown a passenger it does
    /// not need and CI could not see.
    ///
    /// A source claim, like `xtask`'s own guard on the feature line, and
    /// for the same reason: what is being asserted is the shape of the
    /// seam, and a build cannot be asked about code it does not have.
    #[test]
    fn only_the_gated_half_of_this_module_reaches_the_outline() {
        const SOURCE: &str = include_str!("art.rs");
        const GATE: &str = "#[cfg(feature = \"art\")]\nmod loading {";

        let (ungated, _) = SOURCE
            .split_once(GATE)
            .expect("the loading half is a module gated on the feature");
        for word in ["outline", "MaskBody", "MaskProxy"] {
            assert!(
                !ungated.contains(word),
                "`{word}` stands in this module OUTSIDE the `art` gate. Everything that \
                 masks a purchased body belongs behind it — a whitebox build has no \
                 purchased body and must have no line of code about one."
            );
        }
    }

    /// **A manifest with prose and odd spacing in it**, for the writer to
    /// be held to. Every shape in it is one the real file has: paragraphs
    /// of argument between the tables, a key whose spacing nobody
    /// normalised, a comment about a value on the value's own line, a
    /// table missing one of the numbers, and a second table after it
    /// whose lines have nothing to do with any of this.
    const FIXTURE: &str = "\
# The manifest is the owner's file, and nine tenths of it is the argument
# for the numbers in it.

[pack.tins]
title = \"Tins\"
dir = \"Tins Pack\"


# ------------------------------------------------------------------
# The crate. It came out of the pack a quarter of the size the berth
# asks for, which is what the scale below is about.
# ------------------------------------------------------------------

[asset.crate_small]
pack = \"tins\"
source   =   \"FBX/SM_Crate.fbx\"
dresses = \"cargo/suspicious_crate\"
scale=[4,4,4]
offset = [0.0, 0.0, 0.0]  # it sits on its own base, so this is a lie
fill = [1.0, 1.0, 1.0]

# And a second one, which no save about the first may touch.

[asset.lamp]
pack = \"tins\"
offset = [0.25, 0.0, 0.0]
";

    /// **Rewriting one table's numbers leaves every other byte alone.**
    ///
    /// The defect this is written against is the obvious implementation:
    /// parse the manifest, change three fields, print it back. That
    /// hands the owner a file with the argument deleted, the tables in
    /// whatever order a map iterated, and the spacing they chose
    /// normalised — a diff nobody can read, over a change worth three
    /// numbers. So the whole file is asserted, byte for byte, and the
    /// only lines that may differ are the ones that were asked to.
    #[test]
    fn rewriting_a_table_leaves_every_other_byte_alone() {
        let out = rewritten(
            FIXTURE,
            "crate_small",
            &[
                ("scale", Vec3::splat(2.0)),
                ("offset", Vec3::new(0.0, -0.5, 0.125)),
                ("rotation", Vec3::new(0.0, 90.0, 0.0)),
            ],
        )
        .expect("the fixture carries the table");
        let want = FIXTURE
            // The spacing round the `=` is the owner's, so it stays; the
            // number style is the manifest's own, so `[4,4,4]` comes back
            // as a triple somebody would have typed.
            .replace("scale=[4,4,4]", "scale=[2.0, 2.0, 2.0]")
            // A comment about the value survives the value.
            .replace(
                "offset = [0.0, 0.0, 0.0]  # it sits",
                "offset = [0.0, -0.5, 0.125]  # it sits",
            )
            // The key the table never had joins the block of keys rather
            // than landing after the paragraph that introduces the next
            // table.
            .replace(
                "fill = [1.0, 1.0, 1.0]\n",
                "fill = [1.0, 1.0, 1.0]\nrotation = [0.0, 90.0, 0.0]\n",
            );
        assert_eq!(out, want);
        // And the two tables are still two tables, with the second one's
        // own numbers where they were.
        let read = Dressings::read(&out).expect("the dialect it was written in");
        let one = read.of(Kind::SuspiciousCrate).expect("the crate");
        assert_eq!(one.scale, Vec3::splat(2.0));
        assert_eq!(one.offset, Vec3::new(0.0, -0.5, 0.125));
        assert_eq!(one.rotation, Vec3::new(0.0, 90.0, 0.0));
        assert!(out.contains("offset = [0.25, 0.0, 0.0]"), "{out}");
    }

    /// **Writing back what was read changes not one byte**, over the
    /// manifest this repository actually ships.
    ///
    /// The number style is not a taste; it is a claim about a file
    /// somebody else wrote. A save that turned `[1.0, 1.0, 1.0]` into
    /// `[1, 1, 1]`, or into `[1.000, 1.000, 1.000]`, would put a diff in
    /// front of the owner every time the bench was opened and closed
    /// without moving anything. This is that claim, asked of the real
    /// file rather than of a fixture — and the real file is only read.
    ///
    /// The numbers written are the numbers READ, not a constant: this
    /// once wrote the identity because the identity was what the file
    /// said, and the owner's first hand-tuned scale turned that constant
    /// into a move. A manifest that dresses nothing asks nothing here;
    /// the fixture round-trips below hold the writer either way.
    #[test]
    fn writing_back_what_the_shipped_manifest_says_changes_nothing() {
        let declared = Dressings::read(SHIPPED).expect("the manifest this repository ships");
        for kind in Kind::ALL {
            let Some(dressing) = declared.of(kind) else {
                continue;
            };
            let out = rewritten(
                SHIPPED,
                &dressing.id,
                &[
                    ("scale", dressing.scale),
                    ("offset", dressing.offset),
                    ("rotation", dressing.rotation),
                ],
            )
            .expect("a table the manifest dresses can be rewritten");
            assert_eq!(out, SHIPPED, "a save that moved nothing moved the file");
        }
    }

    /// **A key the table never had is added to the right table**, and the
    /// tables round it are left alone.
    ///
    /// A manifest is written in the order people write one — a path
    /// first, a digest second, the numbers last — so a table that has
    /// never been nudged carries none of the three. The line has to land
    /// among that table's own keys: after the paragraph above it, before
    /// the paragraph below it, and inside the right pair of headers.
    #[test]
    fn a_key_the_table_never_had_is_added_among_its_own_neighbours() {
        let text = "[asset.one]\npack = \"tins\"\n\n# why the lamp is where it is\n\
                    [asset.two]\npack = \"tins\"\nfill = [1.0, 1.0, 1.0]\n\
                    # and a word after it\n\n[asset.three]\npack = \"tins\"\n";
        let out = rewritten(text, "two", &[("offset", Vec3::new(0.0, 0.25, 0.0))])
            .expect("the table is there");
        assert_eq!(
            out,
            "[asset.one]\npack = \"tins\"\n\n# why the lamp is where it is\n\
             [asset.two]\npack = \"tins\"\nfill = [1.0, 1.0, 1.0]\noffset = [0.0, 0.25, 0.0]\n\
             # and a word after it\n\n[asset.three]\npack = \"tins\"\n"
        );
        // A table whose keys are all missing gains them under its own
        // header and nowhere else.
        let bare = rewritten("[asset.one]\n[asset.two]\n", "one", &[("scale", Vec3::ONE)])
            .expect("a table with nothing in it is still a table");
        assert_eq!(bare, "[asset.one]\nscale = [1.0, 1.0, 1.0]\n[asset.two]\n");
    }

    /// **An id the manifest does not carry is refused, and nothing is
    /// written.**
    ///
    /// The refusal is the whole of the safety here: the bench names a
    /// table from an index that a resolve wrote, and an index that has
    /// drifted from the manifest names ids the manifest does not have.
    /// Appending a table nobody asked for would be worse than doing
    /// nothing, and doing nothing has to mean not touching the file at
    /// all — which is asserted against the bytes on disk, not against
    /// the return value.
    #[test]
    fn an_id_the_manifest_has_no_table_for_is_refused_and_nothing_is_written() {
        assert!(rewritten(FIXTURE, "crate_large", &[("scale", Vec3::ONE)]).is_err());
        let dir = std::env::temp_dir().join("space-trucking-art-refuse");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        let path = dir.join("manifest.toml");
        std::fs::write(&path, FIXTURE).expect("a fixture manifest");
        let why = save_into(&path, "crate_large", &[("scale", Vec3::ONE)])
            .expect_err("an id nothing carries");
        assert!(why.contains("crate_large"), "{why}");
        assert_eq!(
            std::fs::read_to_string(&path).expect("still there"),
            FIXTURE,
            "a refused save wrote to the file anyway"
        );
        // And a save that changes nothing writes nothing either, so a
        // bench somebody leans on the save key in does not churn a file.
        // (What counts as nothing is the BYTES: rewriting `[4,4,4]` with
        // the same numbers is still a change, because the line it lands
        // on comes out in the style the file is written in.)
        save_into(&path, "lamp", &[("offset", Vec3::new(0.25, 0.0, 0.0))])
            .expect("the table is there");
        assert_eq!(
            std::fs::read_to_string(&path).expect("still there"),
            FIXTURE
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A file that arrived with Windows line endings leaves with
    /// them**, and one with no newline at the end does not grow one.
    ///
    /// Byte preservation is not only about the lines somebody can see.
    /// A writer that reassembled a file with `\n` would rewrite every
    /// line of a manifest edited on Windows, and a three-number change
    /// would arrive as a whole-file diff.
    #[test]
    fn the_line_endings_a_manifest_arrived_with_are_the_ones_it_leaves_with() {
        let text = "[asset.one]\r\npack = \"tins\"\r\nscale = [1.0, 1.0, 1.0]\r\n";
        let out = rewritten(text, "one", &[("scale", Vec3::splat(2.0))]).expect("the table");
        assert_eq!(
            out,
            "[asset.one]\r\npack = \"tins\"\r\nscale = [2.0, 2.0, 2.0]\r\n"
        );
        let added = rewritten(text, "one", &[("offset", Vec3::ZERO)]).expect("the table");
        assert_eq!(
            added,
            "[asset.one]\r\npack = \"tins\"\r\nscale = [1.0, 1.0, 1.0]\r\noffset = [0.0, 0.0, 0.0]\r\n"
        );
        // A last line with no ending gains one so that what follows it
        // is a line at all.
        let ragged = rewritten(
            "[asset.one]\npack = \"tins\"",
            "one",
            &[("scale", Vec3::ONE)],
        )
        .expect("the table");
        assert_eq!(
            ragged,
            "[asset.one]\npack = \"tins\"\nscale = [1.0, 1.0, 1.0]"
        );
    }

    /// **Every `dresses` line in the shipped manifest names a body this
    /// game actually has.**
    ///
    /// The resolver checks the shape of a binding and deliberately not
    /// its meaning: `xtask` cannot see a `cargo::Kind` and should not
    /// learn to. So this is the other half of that check, and it lives
    /// here because here is where the bodies are.
    ///
    /// The shipped manifest's first binding made this guard live; what
    /// keeps it honest either way is the pair below it: the same reader,
    /// over a manifest with a name nobody has, has to fail.
    #[test]
    fn every_dressed_name_in_the_manifest_is_a_body_this_game_has() {
        let declared = Dressings::shipped();
        assert!(
            declared.strangers.is_empty(),
            "art/manifest.toml dresses {:?}, and this game has no such body. \
             The names it does have are: {}",
            declared.strangers,
            Kind::ALL
                .into_iter()
                .map(snake)
                .collect::<Vec<_>>()
                .join(", ")
        );
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

        // A namespace the resolver would refuse outright, which a build
        // older than the namespace still has to survive reading.
        assert_eq!(one("fitting/beacon").strangers, vec!["fitting/beacon"]);
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

    /// **The identity declaration is the berth box.** `scale`, `offset`
    /// and `rotation` at their defaults with a mesh that measures one
    /// half-unit each way is the claim the manifest's own comment makes
    /// for the identity line: that the mesh is exactly the box the
    /// description asks for.
    #[test]
    fn the_identity_declaration_is_the_box_the_description_claims() {
        let declared = Dressings::read(
            "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n\
             glb = \"glb/abc.glb\"\nmeasured_mid = [0.0, 0.0, 0.0]\n\
             measured_half = [1.0, 1.0, 1.0]\n",
        )
        .expect("the dialect");
        let one = declared.of(Kind::SuspiciousCrate).expect("a dressing");
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        let (body_mid, body_half) = one.fill_box(Kind::SuspiciousCrate);
        assert!((body_mid - mid).length() < 1e-4, "{body_mid:?} {mid:?}");
        assert!((body_half - half).length() < 1e-4, "{body_half:?} {half:?}");
        // And the pose puts a unit-half mesh over exactly that box.
        let pose = one.pose(Kind::SuspiciousCrate);
        assert!((pose.scale - half).length() < 1e-4, "{:?}", pose.scale);
        assert!(
            (pose.translation - mid).length() < 1e-4,
            "{:?}",
            pose.translation
        );
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
