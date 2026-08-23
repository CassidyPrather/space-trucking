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
}

impl Value {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Str(_) => "a string",
            Self::Triple(_) => "three numbers",
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
    /// whitebox torus. Nothing consumes this field yet and it is here
    /// anyway, because the field arriving after the meshes is how the
    /// torus happened.
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
    fn take_optional_str(
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

    fn take_str(
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

    fn take_triple(
        &mut self,
        key: &str,
        table: &str,
        id: &str,
        path: &Path,
        fallback: [f32; 3],
    ) -> Result<[f32; 3], Complaint> {
        match self.fields.remove(key) {
            None => Ok(fallback),
            Some((Value::Triple(value), _)) => Ok(value),
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

    /// A key nobody asked for is a typo, and a silently ignored typo in a
    /// manifest is a scale override that never applied.
    fn refuse_leftovers(&self, table: &str, id: &str, path: &Path) -> Result<(), Complaint> {
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
    Err(format!(
        "`{text}` is neither a quoted string nor three numbers in brackets"
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
    /// The digest of the SOURCE the converted file came from. The cache
    /// is addressed by it, so this is also where the file is.
    pub sha256: String,
    pub scale: [f32; 3],
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
    pub fill: [f32; 3],
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
         # the SOURCE mesh the glb was converted from, which is also what the cache is\n\
         # addressed by, so a changed pack changes the path and nothing stale is read.\n",
    );
    for entry in resolved {
        let _ = write!(
            text,
            "\n[asset.{}]\nglb = \"{}\"\nsha256 = \"{}\"\nscale = {}\noffset = {}\n\
             rotation = {}\nfill = {}\n",
            entry.id,
            entry.glb,
            entry.sha256,
            triple(entry.scale),
            triple(entry.offset),
            triple(entry.rotation),
            triple(entry.fill),
        );
    }
    text
}

/// Read an index back. Only the guard below and a future art build need
/// this, and both need it to be the same dialect the manifest is.
pub fn read_index(path: &Path, text: &str) -> Result<Vec<Resolved>, Complaint> {
    let mut grouped = group(path, scan(path, text)?, &["asset"])?;
    let assets = grouped.remove("asset").unwrap_or_default();
    let mut resolved = Vec::new();
    for (id, mut draft) in assets {
        let entry = Resolved {
            glb: draft.take_str("glb", "asset", &id, path)?,
            sha256: draft.take_str("sha256", "asset", &id, path)?,
            scale: draft.take_triple("scale", "asset", &id, path, [1.0; 3])?,
            offset: draft.take_triple("offset", "asset", &id, path, [0.0; 3])?,
            rotation: draft.take_triple("rotation", "asset", &id, path, [0.0; 3])?,
            fill: draft.take_triple("fill", "asset", &id, path, [1.0; 3])?,
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
            scale: [0.013_7, 1.0, 2.5],
            offset: [-0.25, 0.0, 0.125],
            rotation: [0.0, -90.0, 0.0],
            fill: [1.0, 0.18, 1.0],
        }];
        let text = render_index(&written);
        let read = read_index(Path::new("index.toml"), &text).expect("its own dialect");
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, written[0].id);
        assert_eq!(read[0].glb, written[0].glb);
        assert_eq!(read[0].sha256, written[0].sha256);
        assert_eq!(read[0].scale, written[0].scale);
        assert_eq!(read[0].offset, written[0].offset);
        assert_eq!(read[0].rotation, written[0].rotation);
        assert_eq!(read[0].fill, written[0].fill);
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
    #[test]
    fn a_per_axis_value_names_all_three_axes() {
        for value in ["[1.0, 1.0]", "[1.0, 1.0, 1.0, 1.0]", "\"1.0\""] {
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
