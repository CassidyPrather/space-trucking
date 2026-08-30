//! The manifest: what the repository is allowed to know about art it is
//! not allowed to carry.
//!
//! Synty's licence lets their meshes ship inside a built game and forbids
//! redistributing them as source, so the payload cannot live here — not
//! in git, not in LFS on a public remote. What can live here is a
//! *reference*: a stable id somebody types in code, the pack it came out
//! of, the path inside that pack, the hash of the bytes that path held
//! when the line was written, and the handful of numbers that say how the
//! mesh sits in the box the game's own description claims for it.
//!
//! ## The dialect
//!
//! The file is a strict subset of TOML: `# comments`, `[table.id]`
//! headers, and `key = value` where a value is a quoted string or an
//! array of exactly three numbers. Real TOML is a much larger language
//! and none of the rest of it is wanted here, so anything outside the
//! subset is an error naming the line rather than a silently-ignored key.
//! Staying a subset is what buys the comments: the file is a manifest
//! people edit by hand, editors already highlight it, and a bespoke
//! format would have neither property.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// A complaint about a specific line of a specific file.
pub struct Complaint {
    pub file: PathBuf,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Complaint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file.display(), self.line, self.message)
    }
}

/// `Debug` says the same thing `Display` does, so a guard that unwraps a
/// complaint prints the sentence a person would have been shown rather
/// than the struct behind it.
impl fmt::Debug for Complaint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// One `key = value` in one `[table.id]`, with the line it was read from
/// so every later complaint can point at it.
pub struct Entry {
    table: String,
    id: String,
    /// The line the `[table.id]` header is on, which is what a complaint
    /// about the table as a whole should point at.
    header: usize,
    key: String,
    value: Value,
    line: usize,
}

pub enum Value {
    Str(String),
    Triple([f32; 3]),
    /// One number on its own. The manifest asks for this nowhere — its
    /// values are paths, ids and per-axis triples — and it is in the
    /// dialect for the file next door: `art/dex/*.toml` counts triangles
    /// and meshes, and a count written `[412, 412, 412]` to fit a shape
    /// that was never about counts is a worse file than one number.
    ///
    /// Adding it loosens nothing here. Every key in a manifest table asks
    /// for a specific kind, so `scale = 2.0` is still the same refusal it
    /// always was, now naming a number instead of naming nothing — see
    /// the guard at the bottom of this file.
    Number(f64),
}

impl Value {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Str(_) => "a string",
            Self::Triple(_) => "three numbers",
            Self::Number(_) => "one number",
        }
    }
}

/// A pack as downloaded: the directory the owner unzipped it into, and
/// enough English to put in the error when it is not there.
pub struct Pack {
    pub id: String,
    /// What the store calls it, verbatim, so the error names the thing to
    /// click on.
    pub title: String,
    /// The directory under `$SYNTY_STORE` holding it. This is the owner's
    /// own choice, not a guess at Synty's zip naming — the line IS the
    /// contract, so nothing here has to predict what a download unzips to.
    pub dir: String,
    /// Which download of that pack, in the store's own words.
    pub download: String,
    pub line: usize,
}

/// One referenced asset.
pub struct Asset {
    /// The stable id. Code names this, not a path, so re-cutting a mesh
    /// out of a different pack is a manifest edit and not a code edit.
    pub id: String,
    pub pack: String,
    /// Where it lives in the pack's Source Files download, relative to the
    /// pack directory, written with `/` on every platform.
    pub source: String,
    /// Where the same asset lives inside the pack's `.unitypackage`, as
    /// the project-relative path Unity stores (`Assets/...`). Only needed
    /// for a pack whose Source Files download does not carry it.
    pub unity: Option<String>,
    /// A texture to stage beside the mesh before conversion. Synty packs
    /// paint a whole pack from one shared atlas and the FBX names it by a
    /// relative path that only resolves inside the original tree, so a
    /// mesh copied somewhere else converts untextured unless the atlas is
    /// carried along with it.
    pub texture: Option<String>,
    /// The digest the line was written against. Empty means "not recorded
    /// yet"; see [`Asset::NO_DIGEST_YET`].
    pub sha256: String,
    /// **What body of the game this mesh stands in for**, as
    /// `<namespace>/<name>` — `cargo/brine_pearls` today. Absent means
    /// the asset is resolved and indexed and nothing draws it yet, which
    /// is a perfectly good state for a line somebody is still measuring.
    ///
    /// The namespace is here from the first line for the reason the
    /// overrides were: cargo is not the only thing in this game with a
    /// description a mesh could be swapped into, and a flat name would
    /// have to be re-spelled everywhere the day a station fitting gets
    /// one. See [`binding_trouble`] for which namespaces exist.
    pub dresses: Option<String>,
    /// Multiplied onto the size the game's own description asks for.
    pub scale: [f32; 3],
    /// Shifted, in the description's frame, after scaling.
    pub offset: [f32; 3],
    /// Degrees about x, y, z.
    pub rotation: [f32; 3],
    /// **What fraction of its drawing frame the imported body actually
    /// fills**, per axis — the same number, meaning the same thing, as
    /// `poi::Shape::fill` in the cabin. A description claims a box; a
    /// purchased mesh occupies some unknown part of it, and until that
    /// part is written down every rule that measures a body is measuring
    /// the frame instead. Five stations paid for learning that with the
    /// whitebox torus.
    ///
    /// **It is a promise, and [`fill_trouble`] is where it is kept.** The
    /// cabin's gauntlet sweeps it out of the manifest in continuous
    /// integration, on a machine with no art on it; the converter
    /// measures the mesh here, where there is one. Both readings are in
    /// the box's own axes, before `rotation` turns anything.
    pub fill: [f32; 3],
    pub line: usize,
}

impl Asset {
    /// What an unrecorded `sha256` looks like, so the resolver can offer
    /// to fill it in instead of refusing. A manifest line is written
    /// before its pack is on the machine as often as after.
    pub const NO_DIGEST_YET: &'static str = "";
}

pub struct Manifest {
    pub path: PathBuf,
    pub packs: BTreeMap<String, Pack>,
    pub assets: BTreeMap<String, Asset>,
}

impl Manifest {
    pub fn read(path: &Path) -> Result<Self, Complaint> {
        let text = std::fs::read_to_string(path).map_err(|err| Complaint {
            file: path.to_path_buf(),
            line: 0,
            message: format!("cannot be read ({err})"),
        })?;
        Self::parse(path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self, Complaint> {
        let mut grouped = group(path, scan(path, text)?, &["pack", "asset"])?;
        let mut manifest = Self {
            path: path.to_path_buf(),
            packs: BTreeMap::new(),
            assets: BTreeMap::new(),
        };
        manifest.read_packs(path, grouped.remove("pack").unwrap_or_default())?;
        manifest.read_assets(path, grouped.remove("asset").unwrap_or_default())?;
        Ok(manifest)
    }

    fn read_packs(&mut self, path: &Path, packs: BTreeMap<String, Draft>) -> Result<(), Complaint> {
        for (id, mut draft) in packs {
            let title = draft.take_str("title", "pack", &id, path)?;
            let dir = draft.take_str("dir", "pack", &id, path)?;
            let download = draft
                .take_optional_str("download", "pack", &id, path)?
                .unwrap_or_else(|| title.clone());
            draft.refuse_leftovers("pack", &id, path)?;
            self.packs.insert(
                id.clone(),
                Pack {
                    id,
                    title,
                    dir,
                    download,
                    line: draft.line,
                },
            );
        }
        Ok(())
    }

    fn read_assets(
        &mut self,
        path: &Path,
        assets: BTreeMap<String, Draft>,
    ) -> Result<(), Complaint> {
        let complain = |line: usize, message: String| Complaint {
            file: path.to_path_buf(),
            line,
            message,
        };
        for (id, mut draft) in assets {
            let pack = draft.take_str("pack", "asset", &id, path)?;
            let source = draft.take_str("source", "asset", &id, path)?;
            let unity = draft.take_optional_str("unity", "asset", &id, path)?;
            let texture = draft.take_optional_str("texture", "asset", &id, path)?;
            let sha256 = draft
                .take_optional_str("sha256", "asset", &id, path)?
                .unwrap_or_default();
            let dresses = draft.take_optional_str("dresses", "asset", &id, path)?;
            let scale = draft.take_triple("scale", "asset", &id, path, [1.0; 3])?;
            let offset = draft.take_triple("offset", "asset", &id, path, [0.0; 3])?;
            let rotation = draft.take_triple("rotation", "asset", &id, path, [0.0; 3])?;
            let fill = draft.take_triple("fill", "asset", &id, path, [1.0; 3])?;
            draft.refuse_leftovers("asset", &id, path)?;
            let line = draft.line;
            if !self.packs.contains_key(&pack) {
                return Err(complain(
                    line,
                    format!(
                        "`{id}` comes out of pack `{pack}`, and no `[pack.{pack}]` says where \
                         that is or what to call it when it is missing"
                    ),
                ));
            }
            for (part, value) in [
                ("source", Some(&source)),
                ("unity", unity.as_ref()),
                ("texture", texture.as_ref()),
            ] {
                if let Some(value) = value
                    && let Some(reason) = unusable_path(value)
                {
                    return Err(complain(line, format!("`{part}` {reason}")));
                }
            }
            if let Some(binding) = &dresses
                && let Some(reason) = binding_trouble(binding)
            {
                return Err(complain(line, format!("`dresses` {reason}")));
            }
            if !sha256.is_empty() && !is_digest(&sha256) {
                return Err(complain(
                    line,
                    format!(
                        "`sha256` is `{sha256}`, which is not 64 hex digits; leave it empty \
                         and `cargo xtask art hash {id}` will write one"
                    ),
                ));
            }
            for (name, triple) in [("scale", scale), ("fill", fill)] {
                if let Some(bad) = triple.iter().position(|v| !v.is_finite() || *v <= 0.0) {
                    return Err(complain(
                        line,
                        format!(
                            "`{name}` is {triple:?}, and component {bad} is not a positive \
                             number; a body with no extent on an axis cannot be measured on it"
                        ),
                    ));
                }
            }
            if let Some(bad) = fill.iter().position(|v| *v > 1.0) {
                return Err(complain(
                    line,
                    format!(
                        "`fill` is {fill:?}, and component {bad} is over 1.0; fill is the \
                         fraction of its own frame the mesh occupies, so a mesh that \
                         outgrows its frame wants a bigger frame, not a fill over one"
                    ),
                ));
            }
            self.assets.insert(
                id.clone(),
                Asset {
                    id,
                    pack,
                    source,
                    unity,
                    texture,
                    sha256,
                    dresses,
                    scale,
                    offset,
                    rotation,
                    fill,
                    line,
                },
            );
        }
        Ok(())
    }

    pub fn pack_of(&self, asset: &Asset) -> &Pack {
        &self.packs[&asset.pack]
    }
}

/// Fields collected under one `[table.id]` before they are checked
/// against what that table is allowed to hold.
pub struct Draft {
    line: usize,
    fields: BTreeMap<String, (Value, usize)>,
}

impl Draft {
    pub fn take_optional_str(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
    ) -> Result<Option<String>, Complaint> {
        match self.fields.remove(key) {
            None => Ok(None),
            Some((Value::Str(value), _)) => Ok(Some(value)),
            Some((other, line)) => Err(Complaint {
                file: path.to_path_buf(),
                line,
                message: format!(
                    "`{key}` in `[{table}.{id}]` is {}, and it has to be a quoted string",
                    other.kind()
                ),
            }),
        }
    }

    pub fn take_str(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
    ) -> Result<String, Complaint> {
        self.take_optional_str(key, table, id, path)?
            .ok_or_else(|| Complaint {
                file: path.to_path_buf(),
                line: self.line,
                message: format!("`[{table}.{id}]` says no `{key}`, and it has to"),
            })
    }

    pub fn take_triple(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
        fallback: [f32; 3],
    ) -> Result<[f32; 3], Complaint> {
        Ok(self
            .take_optional_triple(key, table, id, path)?
            .unwrap_or(fallback))
    }

    pub fn take_optional_triple(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
    ) -> Result<Option<[f32; 3]>, Complaint> {
        match self.fields.remove(key) {
            None => Ok(None),
            Some((Value::Triple(value), _)) => Ok(Some(value)),
            Some((other, line)) => Err(Complaint {
                file: path.to_path_buf(),
                line,
                message: format!(
                    "`{key}` in `[{table}.{id}]` is {}, and it has to be three numbers \
                     like `[1.0, 1.0, 1.0]`",
                    other.kind()
                ),
            }),
        }
    }

    /// One whole number, for the counts the dex carries. Read as a count
    /// rather than as a number because every number in that file is one —
    /// triangles, meshes, materials — and `412.5 triangles` is a file
    /// somebody has edited into meaninglessness rather than a value to
    /// round.
    pub fn take_optional_count(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
    ) -> Result<Option<u64>, Complaint> {
        let complain = |line, message| Complaint {
            file: path.to_path_buf(),
            line,
            message,
        };
        match self.fields.remove(key) {
            None => Ok(None),
            Some((Value::Number(value), line)) => {
                if value < 0.0 || value.fract() != 0.0 || value > 2f64.powi(53) {
                    return Err(complain(
                        line,
                        format!(
                            "`{key}` in `[{table}.{id}]` is {value}, and it counts things, \
                             so it has to be a whole number that is not negative"
                        ),
                    ));
                }
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                Ok(Some(value as u64))
            }
            Some((other, line)) => Err(complain(
                line,
                format!(
                    "`{key}` in `[{table}.{id}]` is {}, and it has to be one whole number",
                    other.kind()
                ),
            )),
        }
    }

    /// A key nobody asked for is a typo, and a silently ignored typo in a
    /// manifest is a scale override that never applied.
    pub fn refuse_leftovers(&self, table: &str, id: &str, path: &Path) -> Result<(), Complaint> {
        if let Some((key, (_, line))) = self.fields.iter().next() {
            return Err(Complaint {
                file: path.to_path_buf(),
                line: *line,
                message: format!("`[{table}.{id}]` has no key called `{key}`"),
            });
        }
        Ok(())
    }
}

/// Why a path in the manifest cannot be used, or `None` if it can.
fn unusable_path(value: &str) -> Option<String> {
    if value.is_empty() {
        return Some("is empty".to_owned());
    }
    if value.contains('\\') {
        return Some(format!(
            "is `{value}`, and paths here are written with `/` on every platform \
             so one manifest works on all three"
        ));
    }
    if value.starts_with('/') || value.split('/').any(|part| part == ".." || part == ".") {
        return Some(format!(
            "is `{value}`, and it has to be a plain relative path inside the pack"
        ));
    }
    None
}

/// **Which namespaces a `dresses` line may name.** One today, and the
/// list is here rather than the check being "anything with a slash in
/// it", because this file's whole personality is that a typo is a
/// refusal: `carg/brine_pearls` is a binding that would silently never
/// apply, and a mesh that never applies looks exactly like a mesh that
/// converted wrong.
///
/// `fitting` is the one everybody can see coming — a station's own
/// hardware is described the same way cargo is (`poi::Fitting`) — and it
/// is not here, because a namespace nothing reads is a promise this file
/// cannot keep. Adding it is a word here and a match arm in the cabin.
const NAMESPACES: [&str; 1] = ["cargo"];

/// Why a `dresses` value cannot be used, or `None` if it can.
///
/// The name half is checked for SHAPE and not for meaning. What bodies
/// exist is the game's question, not the resolver's — this package
/// cannot see a `cargo::Kind` and should not learn to — so a name that
/// is well-formed and names nothing is caught on the other side of the
/// wall, by the cabin's own guard over this very file.
fn binding_trouble(value: &str) -> Option<String> {
    let mut parts = value.split('/');
    let (Some(namespace), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return Some(format!(
            "is `{value}`, and a binding is `<namespace>/<name>`, like `cargo/crate_small`"
        ));
    };
    if !NAMESPACES.contains(&namespace) {
        return Some(format!(
            "names `{namespace}`, and the namespaces this file has are {}",
            NAMESPACES
                .iter()
                .map(|one| format!("`{one}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Some(format!(
            "is `{value}`, and `{name}` is not a name; names here are lowercase letters, \
             digits and underscores"
        ));
    }
    None
}

fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The dialect reader. Everything above this line is about meaning;
/// everything below it is about lines of text.
pub fn scan(path: &Path, text: &str) -> Result<Vec<Entry>, Complaint> {
    let complain = |line: usize, message: String| Complaint {
        file: path.to_path_buf(),
        line,
        message,
    };
    let mut entries = Vec::new();
    let mut table = String::new();
    let mut id = String::new();
    let mut header_line = 0;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('[') {
            let header = header.strip_suffix(']').ok_or_else(|| {
                complain(
                    line,
                    format!("`{trimmed}` opens a table and never closes it"),
                )
            })?;
            let mut parts = header.split('.');
            let (Some(first), Some(second), None) = (parts.next(), parts.next(), parts.next())
            else {
                return Err(complain(
                    line,
                    format!("`[{header}]` is not `[pack.<id>]` or `[asset.<id>]`"),
                ));
            };
            for part in [first, second] {
                if part.is_empty()
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                {
                    return Err(complain(
                        line,
                        format!(
                            "`{part}` in `[{header}]` is not a name; names here are \
                             lowercase letters, digits and underscores"
                        ),
                    ));
                }
            }
            first.clone_into(&mut table);
            second.clone_into(&mut id);
            header_line = line;
            continue;
        }
        if table.is_empty() {
            return Err(complain(
                line,
                format!("`{trimmed}` comes before any `[pack.<id>]` or `[asset.<id>]`"),
            ));
        }
        let (key, rest) = trimmed.split_once('=').ok_or_else(|| {
            complain(
                line,
                format!("`{trimmed}` is neither a table header nor a `key = value`"),
            )
        })?;
        let key = key.trim().to_owned();
        let value = read_value(rest.trim()).map_err(|why| complain(line, why))?;
        entries.push(Entry {
            table: table.clone(),
            id: id.clone(),
            header: header_line,
            key,
            value,
            line,
        });
    }
    Ok(entries)
}

fn read_value(text: &str) -> Result<Value, String> {
    if let Some(rest) = text.strip_prefix('"') {
        let end = rest
            .find('"')
            .ok_or_else(|| format!("`{text}` opens a string and never closes it"))?;
        let value = &rest[..end];
        if value.contains('\\') {
            return Err(format!(
                "`{value}` has a backslash, and strings here carry no escapes"
            ));
        }
        refuse_trailing(&rest[end + 1..])?;
        return Ok(Value::Str(value.to_owned()));
    }
    if let Some(rest) = text.strip_prefix('[') {
        let end = rest
            .find(']')
            .ok_or_else(|| format!("`{text}` opens an array and never closes it"))?;
        refuse_trailing(&rest[end + 1..])?;
        let mut numbers = Vec::new();
        for part in rest[..end].split(',') {
            let part = part.trim();
            numbers.push(
                part.parse::<f32>()
                    .map_err(|_| format!("`{part}` is not a number"))?,
            );
        }
        let triple: [f32; 3] = numbers.try_into().map_err(|got: Vec<f32>| {
            format!(
                "`[{}]` has {} numbers in it, and this is a per-axis value, so it wants three",
                rest[..end].trim(),
                got.len()
            )
        })?;
        return Ok(Value::Triple(triple));
    }
    // A bare number, which only `art/dex/*.toml` writes: a count of
    // triangles or meshes. Split at the comment the same way the two
    // above are, so `triangles = 412 # after decimation` is 412.
    let number = text.split('#').next().unwrap_or(text).trim();
    if let Ok(value) = number.parse::<f64>()
        && value.is_finite()
    {
        return Ok(Value::Number(value));
    }
    Err(format!(
        "`{text}` is not a quoted string, three numbers in brackets, or one number"
    ))
}

fn refuse_trailing(rest: &str) -> Result<(), String> {
    let rest = rest.trim();
    if rest.is_empty() || rest.starts_with('#') {
        return Ok(());
    }
    Err(format!(
        "`{rest}` trails the value, and only a `#` comment may"
    ))
}

/// Print an `f32` so the file it lands in reads like a manifest somebody
/// typed. Rust's own `Display` writes `1` for `1.0`, and a per-axis value
/// spelled `[1, 1, 1]` invites the reader to think it is an integer count
/// of something.
pub fn number(value: f32) -> String {
    let text = format!("{value}");
    if text.contains(['.', 'e', 'E']) {
        text
    } else {
        format!("{text}.0")
    }
}

pub fn triple(value: [f32; 3]) -> String {
    format!(
        "[{}, {}, {}]",
        number(value[0]),
        number(value[1]),
        number(value[2])
    )
}

/// Sort scanned lines into one [`Draft`] per `[table.id]`, refusing a
/// table this file has no meaning for and a key set twice.
pub fn group(
    path: &Path,
    entries: Vec<Entry>,
    tables: &[&str],
) -> Result<BTreeMap<String, BTreeMap<String, Draft>>, Complaint> {
    let mut grouped: BTreeMap<String, BTreeMap<String, Draft>> = BTreeMap::new();
    for entry in entries {
        if !tables.contains(&entry.table.as_str()) {
            return Err(Complaint {
                file: path.to_path_buf(),
                line: entry.line,
                message: format!(
                    "`[{}.{}]` is not a table this file has; it holds {}",
                    entry.table,
                    entry.id,
                    tables
                        .iter()
                        .map(|table| format!("`[{table}.<id>]`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            });
        }
        let draft = grouped
            .entry(entry.table.clone())
            .or_default()
            .entry(entry.id.clone())
            .or_insert_with(|| Draft {
                line: entry.header,
                fields: BTreeMap::new(),
            });
        if draft.fields.contains_key(&entry.key) {
            return Err(Complaint {
                file: path.to_path_buf(),
                line: entry.line,
                message: format!(
                    "`{}` is set twice in `[{}.{}]`",
                    entry.key, entry.table, entry.id
                ),
            });
        }
        draft.fields.insert(entry.key, (entry.value, entry.line));
    }
    Ok(grouped)
}

/// One line of the resolved index: an id, the file that answers it, and
/// the numbers the manifest attached to it.
///
/// The index is written in the same dialect as the manifest for one
/// reason: the thing that eventually reads it is the game's build, in a
/// crate that cannot call into this one, and a reader for this dialect is
/// the eighty lines above rather than a dependency.
pub struct Resolved {
    pub id: String,
    /// The converted file, relative to the cache root, with `/`
    /// separators so the index reads the same on every platform.
    pub glb: String,
    /// The digest of the SOURCE the converted file came from, and the
    /// front of the name the cache files that file under — the rest of
    /// that name is what else the conversion read, so `glb` above is the
    /// path and this is the provenance.
    pub sha256: String,
    /// Which body of the game draws this, carried through from the
    /// manifest so the thing that reads the index never has to read the
    /// manifest.
    pub dresses: Option<String>,
    pub scale: [f32; 3],
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
    pub fill: [f32; 3],
    /// **What the converter actually measured**, in the converted file's
    /// own units: the tight box round the mesh, as a middle and a half.
    /// `None` where the converter did not say — see [`Bounds`].
    pub measured: Option<Bounds>,
}

/// **The tight box round a converted mesh**, in that file's own units.
///
/// This is the FACT half of the fill declaration. `fill` in the manifest
/// is a promise about how much of its berth a body occupies, and until
/// something measured the mesh the promise was the only statement in the
/// system — which is the whole shape of the defect the field exists to
/// stop, one level up.
///
/// It is optional because the converter contract is deliberately open.
/// `$ART_CONVERTER` is documented as any program taking a source and a
/// destination, and `FBX2glTF` has never heard of this repository; the
/// Blender script this package ships reports its bounds, and a converter
/// that says nothing leaves the promise unchecked rather than refused.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bounds {
    pub mid: [f32; 3],
    pub half: [f32; 3],
}

/// The index as text, ready to write.
pub fn render_index(resolved: &[Resolved]) -> String {
    use std::fmt::Write as _;
    let mut text = String::from(
        "# Written by `cargo xtask art resolve`. Not in git, and not editable by hand:\n\
         # every line here is derived from art/manifest.toml and the packs on this\n\
         # machine, and the next resolve overwrites the file.\n\
         #\n\
         # `glb` is relative to this file's own directory. `sha256` is the digest of\n\
         # the SOURCE mesh the glb was converted from, and the front of the name the\n\
         # cache files it under; the rest of that name covers what else the conversion\n\
         # read, so a changed pack OR a changed atlas changes the path and nothing\n\
         # stale is read.\n\
         #\n\
         # `dresses` is which body of the game draws this one. `measured_mid` and\n\
         # `measured_half` are the tight box the converter reported round the mesh, in\n\
         # the glb's own units — the fact the `fill` promise beside them was checked\n\
         # against. Both are absent where the converter reported nothing.\n",
    );
    for entry in resolved {
        let _ = write!(
            text,
            "\n[asset.{}]\nglb = \"{}\"\nsha256 = \"{}\"\n",
            entry.id, entry.glb, entry.sha256,
        );
        if let Some(dresses) = &entry.dresses {
            let _ = writeln!(text, "dresses = \"{dresses}\"");
        }
        let _ = write!(
            text,
            "scale = {}\noffset = {}\nrotation = {}\nfill = {}\n",
            triple(entry.scale),
            triple(entry.offset),
            triple(entry.rotation),
            triple(entry.fill),
        );
        if let Some(measured) = entry.measured {
            let _ = write!(
                text,
                "measured_mid = {}\nmeasured_half = {}\n",
                triple(measured.mid),
                triple(measured.half),
            );
        }
    }
    text
}

/// **How far a measured body may sit off the `fill` beside it**, in the
/// berth box's own half-units.
///
/// A fiftieth of the half-box: about 5 mm on a one-cell cargo kind,
/// which is the same order as the gauntlet's own clip slack and coarser
/// than the two decimals a `fill` is written with by hand. Tighter than
/// this and a correctly-rounded `0.18` is a refusal; looser and a mesh
/// can be a centimetre bigger than the box every containment rule reads
/// for it, which is the whole defect the field exists to stop.
pub const FILL_SLACK: f32 = 0.02;

/// **Where the fact meets the promise**: the converter measured the
/// mesh, the manifest declared what fraction of its berth that mesh
/// occupies, and this is the one place the two are made to agree.
///
/// The units. `scale` carries the converted file's own units into the
/// berth box's half-units — the box is `[-1, 1]` on every axis there, the
/// same normalised frame `poi::Fitting` states a station's hardware in —
/// so the body's half-extent in that frame is its measured half times
/// `scale`, and `fill` is the claim about exactly that number. Both are
/// stated in the box's own axes, before `rotation` turns anything, so the
/// comparison is exact rather than a bound.
///
/// **Only an asset that dresses something is asked.** A line somebody is
/// still measuring carries the identity `fill = [1.0, 1.0, 1.0]` because
/// that is the default, and refusing it before anything draws it would
/// make the manifest impossible to write in the order people write it.
/// The promise starts mattering the moment a `dresses` line makes
/// something read it.
#[must_use]
pub fn fill_trouble(asset: &Asset, measured: Bounds) -> Option<String> {
    asset.dresses.as_ref()?;
    let occupied = [0, 1, 2].map(|axis| measured.half[axis] * asset.scale[axis]);
    let off = [0, 1, 2].map(|axis| occupied[axis] - asset.fill[axis]);
    // The first of the worst, not the last: three axes equally wrong is
    // one mesh at the wrong size, and naming `z` for it reads like a
    // fact about the depth.
    let worst = (0..3).fold(0, |best: usize, axis| {
        if off[axis].abs() > off[best].abs() {
            axis
        } else {
            best
        }
    });
    if !off[worst].abs().is_finite() || off[worst].abs() <= FILL_SLACK {
        return None;
    }
    let fits = [0, 1, 2].map(|axis| {
        if measured.half[axis].abs() > f32::EPSILON {
            1.0 / measured.half[axis]
        } else {
            asset.scale[axis]
        }
    });
    Some(format!(
        "{} is not the size {} says it is.\n\n  \
         axis      {}\n  \
         declared  fill {}\n  \
         measured  {} of its berth box\n  \
         from      a mesh {} half-units across, at scale {}\n  \
         off by    {:+.4}, and {FILL_SLACK} is the slack\n\n  \
         fix       Either the mesh moved under the line or the line was a guess. If the\n            \
                   mesh is the one you want, paste this and the promise is true again:\n\n              \
         fill = {}\n\n            \
                   If it is the SIZE that is wrong rather than the claim, this scale puts\n            \
                   the mesh exactly in its berth, and `fill = [1.0, 1.0, 1.0]` with it:\n\n              \
         scale = {}\n",
        asset.id,
        asset
            .dresses
            .as_deref()
            .unwrap_or("the body it dresses")
            .to_owned(),
        ["x", "y", "z"][worst],
        triple(asset.fill),
        triple(occupied),
        triple(measured.half),
        triple(asset.scale),
        off[worst],
        triple(occupied),
        triple(fits),
    ))
}

/// Read an index back. Only the guard below and a future art build need
/// this, and both need it to be the same dialect the manifest is.
pub fn read_index(path: &Path, text: &str) -> Result<Vec<Resolved>, Complaint> {
    let mut grouped = group(path, scan(path, text)?, &["asset"])?;
    let assets = grouped.remove("asset").unwrap_or_default();
    let mut resolved = Vec::new();
    for (id, mut draft) in assets {
        let glb = draft.take_str("glb", "asset", &id, path)?;
        let sha256 = draft.take_str("sha256", "asset", &id, path)?;
        let dresses = draft.take_optional_str("dresses", "asset", &id, path)?;
        let scale = draft.take_triple("scale", "asset", &id, path, [1.0; 3])?;
        let offset = draft.take_triple("offset", "asset", &id, path, [0.0; 3])?;
        let rotation = draft.take_triple("rotation", "asset", &id, path, [0.0; 3])?;
        let fill = draft.take_triple("fill", "asset", &id, path, [1.0; 3])?;
        let mid = draft.take_optional_triple("measured_mid", "asset", &id, path)?;
        let half = draft.take_optional_triple("measured_half", "asset", &id, path)?;
        let entry = Resolved {
            glb,
            sha256,
            dresses,
            scale,
            offset,
            rotation,
            fill,
            measured: mid.zip(half).map(|(mid, half)| Bounds { mid, half }),
            id,
        };
        draft.refuse_leftovers("asset", &entry.id, path)?;
        resolved.push(entry);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Manifest, Complaint> {
        Manifest::parse(Path::new("manifest.toml"), text)
    }

    const PACK: &str = "[pack.demo]\ntitle = \"A Pack\"\ndir = \"a-pack\"\n";

    /// **The manifest that ships in this repository is one the resolver
    /// can read.** Read, and nothing beyond read: this asks the parser
    /// about the file and never asks a disk about the art. The art is
    /// the one thing a machine running these guards cannot have — the
    /// licence is why the payload is not in the repository, so continuous
    /// integration is the place it is guaranteed absent — and a guard
    /// here that went looking for it would go red on the day the manifest
    /// first named something, which is the day the tool started being
    /// used for its purpose.
    ///
    /// Reading is not the weak half of the claim. It is the whole of what
    /// this file can get wrong on its own: a key nobody asked for, an
    /// asset out of a pack nobody declared, a digest that is not a
    /// digest, a path that leaves its pack or is spelled for one
    /// platform, an override missing an axis. Whether the file it names
    /// is on this machine is what `cargo xtask art check` answers, at the
    /// keyboard of somebody who holds the licence.
    #[test]
    fn the_manifest_in_the_repository_is_one_the_resolver_can_read() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask sits in the workspace root")
            .join("art")
            .join("manifest.toml");
        let manifest = Manifest::read(&path).unwrap_or_else(|complaint| panic!("{complaint}"));
        assert!(
            !manifest.packs.is_empty(),
            "{} declares no packs, and it is the file every reader copies a table out of",
            path.display()
        );
    }

    /// **A key nobody asked for is a refusal, not a shrug.** The whole
    /// value of the override fields is that somebody can write one down
    /// and have it apply; a manifest that ignores `scal = [2,2,2]` is a
    /// manifest where the override silently never happened, and the
    /// symptom is a mesh the wrong size two slices later.
    #[test]
    fn a_key_this_file_has_no_meaning_for_is_refused() {
        let complaint = parse(&format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nscal = [2.0, 2.0, 2.0]\n"
        ))
        .err()
        .expect("a manifest with a typo in it is not a manifest");
        assert!(complaint.message.contains("scal"), "{complaint}");
        assert_eq!(complaint.line, 8, "{complaint}");
    }

    /// **An asset names a pack that says where it is and what to call
    /// it.** The pack table is the only place the download instruction
    /// lives, so an asset pointing at a pack nobody declared is an asset
    /// whose missing-file message would have nothing useful in it.
    #[test]
    fn an_asset_out_of_an_undeclared_pack_is_refused() {
        let complaint = parse("[asset.crate]\npack = \"nope\"\nsource = \"a.fbx\"\n")
            .err()
            .expect("a pack that does not exist cannot be downloaded");
        assert!(complaint.message.contains("nope"), "{complaint}");
    }

    /// **Fill is a fraction of a frame, so it cannot be more than the
    /// frame.** `poi::Shape::fill` is per-axis and at most one by
    /// construction — a torus fills 0.18 of its box on the axis it lies
    /// about — and a mesh that genuinely outgrows its frame wants a
    /// bigger frame, which is `scale`. Letting `fill` exceed one would
    /// make every containment rule that reads it read a body larger than
    /// the room it was checked against.
    #[test]
    fn a_fill_larger_than_its_own_frame_is_refused() {
        let bad = parse(&format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nfill = [1.0, 1.2, 1.0]\n"
        ));
        assert!(bad.is_err(), "fill over one was accepted");
        let zero = parse(&format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nfill = [1.0, 0.0, 1.0]\n"
        ));
        assert!(
            zero.is_err(),
            "a body with no extent on an axis was accepted"
        );
        assert!(parse(&format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nfill = [1.0, 0.18, 1.0]\n"
        ))
        .is_ok());
    }

    /// **A path in the manifest is a relative path inside its pack,
    /// spelled with `/`.** One manifest is read on three platforms, and
    /// the two ways to break that are a backslash, which is not a
    /// separator on two of them, and a `..`, which walks out of the pack
    /// the manifest said the asset was in.
    #[test]
    fn a_path_in_the_manifest_stays_inside_its_pack() {
        for path in [
            "../elsewhere/a.fbx",
            "/etc/passwd",
            "SourceFiles\\a.fbx",
            "",
        ] {
            assert!(
                parse(&format!(
                    "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"{path}\"\n"
                ))
                .is_err(),
                "`{path}` was accepted as a source"
            );
        }
    }

    /// **The numbers written into the index are the numbers the manifest
    /// gave.** The index is the one artefact something else parses
    /// later, and the four overrides are the whole reason it carries
    /// more than a path. A float that does not survive being printed and
    /// read back is a mesh that arrives at the wrong size on the machine
    /// that reads the index rather than on the one that wrote it.
    // Exact equality is the law here, not an approximation of it: the
    // point of the round trip is that the number that comes back IS the
    // number that went in.
    #[allow(clippy::float_cmp)]
    #[test]
    fn the_overrides_survive_being_written_down_and_read_back() {
        let written = vec![Resolved {
            id: "crate_small".to_owned(),
            glb: "glb/abc.glb".to_owned(),
            sha256: "a".repeat(64),
            dresses: Some("cargo/suspicious_crate".to_owned()),
            scale: [0.013_7, 1.0, 2.5],
            offset: [-0.25, 0.0, 0.125],
            rotation: [0.0, -90.0, 0.0],
            fill: [1.0, 0.18, 1.0],
            measured: Some(Bounds {
                mid: [0.0, 0.5, -0.125],
                half: [36.5, 0.18, 0.4],
            }),
        }];
        let text = render_index(&written);
        let read = read_index(Path::new("index.toml"), &text).expect("its own dialect");
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, written[0].id);
        assert_eq!(read[0].glb, written[0].glb);
        assert_eq!(read[0].sha256, written[0].sha256);
        assert_eq!(read[0].dresses, written[0].dresses);
        assert_eq!(read[0].scale, written[0].scale);
        assert_eq!(read[0].offset, written[0].offset);
        assert_eq!(read[0].rotation, written[0].rotation);
        assert_eq!(read[0].fill, written[0].fill);
        assert_eq!(read[0].measured, written[0].measured);

        // An asset nothing draws yet, and a converter that measured
        // nothing: both absences survive the trip as absences, rather
        // than coming back as an empty string or a box of zero size.
        let bare = render_index(&[Resolved {
            dresses: None,
            measured: None,
            ..written.into_iter().next().expect("the one written above")
        }]);
        // Asked of the lines and not of the file, because the file's own
        // header explains both keys and would answer for them.
        let keys = |text: &str| -> Vec<String> {
            text.lines()
                .filter_map(|line| line.split_once(" = "))
                .map(|(key, _)| key.to_owned())
                .collect()
        };
        assert!(!keys(&bare).iter().any(|key| key == "dresses"), "{bare}");
        assert!(
            !keys(&bare).iter().any(|key| key.starts_with("measured")),
            "{bare}"
        );
        let read = read_index(Path::new("index.toml"), &bare).expect("its own dialect");
        assert_eq!(read[0].dresses, None);
        assert_eq!(read[0].measured, None);
    }

    /// **A binding names a namespace this file has and a name that could
    /// be one.** The `dresses` key is the whole of how a mesh finds the
    /// body it stands in for, and a binding that never applies is
    /// indistinguishable, on screen, from a mesh that converted wrong:
    /// the whitebox is what you see either way.
    ///
    /// What is NOT checked here is whether the name is a body the game
    /// has. This package cannot see a `cargo::Kind` and should not learn
    /// to; that half is the cabin's own guard over this very file.
    #[test]
    fn a_binding_that_names_nothing_this_file_has_is_refused() {
        let one = |dresses: &str| {
            parse(&format!(
                "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\n\
                 dresses = \"{dresses}\"\n"
            ))
        };
        for bad in ["carg/crate", "crate_small", "cargo/", "cargo/Crate", "/x"] {
            assert!(one(bad).is_err(), "`{bad}` was accepted as a binding");
        }
        let complaint = one("carg/crate")
            .err()
            .expect("a namespace nobody has is not a namespace");
        assert!(complaint.message.contains("cargo"), "{complaint}");
        assert_eq!(
            one("cargo/suspicious_crate").expect("a binding").assets["crate"].dresses,
            Some("cargo/suspicious_crate".to_owned())
        );
    }

    /// **A mesh that is not the size its `fill` says it is stops the
    /// run.** The promise is the only thing in the system that says how
    /// much of its berth a purchased body occupies; the converter is the
    /// only thing that can see the mesh. If the two are never made to
    /// meet, `fill` is a number somebody typed once and every containment
    /// rule downstream reads it as a fact.
    ///
    /// **And a line nothing draws yet is not asked.** The identity
    /// `fill = [1.0, 1.0, 1.0]` is the default, so refusing it before a
    /// `dresses` line makes something read it would make the manifest
    /// impossible to write in the order people write it — a path first, a
    /// digest second, the numbers last.
    #[test]
    fn a_mesh_that_is_not_the_size_its_fill_claims_is_refused() {
        let asset = |dresses: &str, scale: &str, fill: &str| {
            let binding = if dresses.is_empty() {
                String::new()
            } else {
                format!("dresses = \"{dresses}\"\n")
            };
            parse(&format!(
                "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\n\
                 {binding}scale = {scale}\nfill = {fill}\n"
            ))
            .expect("a manifest")
        };
        // A mesh a quarter of a metre across each way, at unit scale: it
        // occupies a quarter of its berth box and the line says so.
        let measured = Bounds {
            mid: [0.0; 3],
            half: [0.25; 3],
        };
        let truthful = asset(
            "cargo/suspicious_crate",
            "[1.0, 1.0, 1.0]",
            "[0.25, 0.25, 0.25]",
        );
        assert!(fill_trouble(&truthful.assets["crate"], measured).is_none());

        let boastful = asset(
            "cargo/suspicious_crate",
            "[1.0, 1.0, 1.0]",
            "[0.25, 1.0, 0.25]",
        );
        let trouble = fill_trouble(&boastful.assets["crate"], measured)
            .expect("a body a quarter of the height it claims");
        for wanted in [
            "crate",
            "cargo/suspicious_crate",
            "fill = ",
            "scale = ",
            "y",
        ] {
            assert!(trouble.contains(wanted), "no `{wanted}` in:\n{trouble}");
        }

        // The same wrong numbers, on a line nothing draws yet.
        let undrawn = asset("", "[1.0, 1.0, 1.0]", "[0.25, 1.0, 0.25]");
        assert!(fill_trouble(&undrawn.assets["crate"], measured).is_none());

        // Inside the slack, which is where a hand-written two-decimal
        // fill against a measured mesh actually lands.
        let rounded = asset(
            "cargo/suspicious_crate",
            "[1.0, 1.0, 1.0]",
            "[0.26, 0.24, 0.25]",
        );
        assert!(fill_trouble(&rounded.assets["crate"], measured).is_none());
    }

    /// **A manifest is a commented file, so a comment is never part of a
    /// value.** The whole file is explanation; a `#` that ended up
    /// inside a path would make the explanation change the meaning.
    #[allow(clippy::float_cmp)]
    #[test]
    fn a_comment_is_not_part_of_the_value_beside_it() {
        let manifest = parse(&format!(
            "{PACK}\n# why this one\n[asset.crate]  \n\
             pack = \"demo\" # the sci-fi one\n\
             source = \"a#b.fbx\"   # a hash is legal in a file name\n\
             scale = [1.0, 2.0, 3.0] # twice as tall\n"
        ))
        .expect("comments are not values");
        let asset = &manifest.assets["crate"];
        assert_eq!(asset.pack, "demo");
        assert_eq!(asset.source, "a#b.fbx");
        assert_eq!(asset.scale, [1.0, 2.0, 3.0]);
    }

    /// **A per-axis value has three axes.** Two numbers in a `scale`
    /// would otherwise silently become something, and the something
    /// would be wrong on whichever axis was left out.
    ///
    /// `2.0` is in the list because the dialect learned to read a bare
    /// number for the dex next door, and a shared reader is only safe
    /// while every key in a manifest table still asks for the kind it
    /// wants: a scale written as one number is a mesh at the wrong size
    /// on two axes, and it stays a refusal.
    #[test]
    fn a_per_axis_value_names_all_three_axes() {
        for value in ["[1.0, 1.0]", "[1.0, 1.0, 1.0, 1.0]", "\"1.0\"", "2.0"] {
            assert!(
                parse(&format!(
                    "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nscale = {value}\n"
                ))
                .is_err(),
                "`{value}` was accepted as a scale"
            );
        }
    }

    /// **A digest is either sixty-four hex digits or absent.** The
    /// halfway state — a truncated or pasted-wrong digest — would make
    /// every resolve fail with a mismatch that looks like a corrupted
    /// download.
    #[test]
    fn a_digest_is_a_whole_digest_or_none_at_all() {
        let short = format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nsha256 = \"abc123\"\n"
        );
        assert!(parse(&short).is_err());
        let upper = format!(
            "{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\nsha256 = \"{}\"\n",
            "A".repeat(64)
        );
        assert!(
            parse(&upper).is_err(),
            "an uppercase digest never matches one we print"
        );
        let absent = format!("{PACK}\n[asset.crate]\npack = \"demo\"\nsource = \"a.fbx\"\n");
        assert_eq!(
            parse(&absent).expect("no digest yet").assets["crate"].sha256,
            ""
        );
    }
}
