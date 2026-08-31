//! The dex: a page of English about a mesh nobody can read the name of.
//!
//! A Synty library is a hundred packs of five thousand files, and every
//! one of those files is called something like `SM_Prop_Crate_01`. `cargo
//! xtask art find crate` answers with forty of those names, which is the
//! whole of what a file name can tell anybody: there are forty crates,
//! and no two of them are distinguishable from here. Choosing one still
//! means opening Unity, or Blender, forty times.
//!
//! So this file is the other half of the search. One table per mesh,
//! carrying the numbers something measured — triangles, meshes,
//! materials, the size it actually is — and one to three sentences of
//! plain English saying what it *looks like*, written by a vision model
//! that was shown a picture of it. `cargo xtask art describe` writes it
//! and `cargo xtask art dex` reads it back.
//!
//! **It is a catalogue and not a manifest, and the difference is what it
//! may be trusted for.** Every line of `art/manifest.toml` is a promise
//! somebody typed and something checks. Every `description` here is a
//! sentence a language model wrote about a picture, and nothing in this
//! repository can check it: it is an index to search, the way a library
//! catalogue is, and the mesh is still the thing that is true.
//!
//! ## Why it is in the repository
//!
//! Nothing derived from a pack may be committed — that is the licence,
//! and it is why `art/cache/` is gitignored. What is here is the same
//! kind of thing `art/manifest.toml` already carries and no more: file
//! names, digests, counts, and English written about them. No geometry,
//! no pixels, nothing that could be turned back into a mesh. It is
//! written where a person can read it and diff it, because it costs a
//! Blender launch and a hosted model call per line and a gitignored copy
//! would be bought again on every clone.
//!
//! The dialect is `art/manifest.toml`'s, read by the same code — see
//! [`crate::manifest`] — plus the bare number that file has no use for.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::manifest::{self, Complaint};

/// One mesh, as the catalogue holds it.
pub struct Entry {
    /// The table name: the file's own stem, in the lowercase-and-
    /// underscores the dialect allows. It is derived from the name and
    /// not from the digest, so the table somebody is reading is the file
    /// they were looking for.
    pub id: String,
    /// The file's stem, spelled the way the pack spells it. This is the
    /// half of the entry the describer is told about, because a
    /// description written without it says "a wooden crate" about a
    /// wooden crate.
    pub name: String,
    /// The pack id `art/manifest.toml` declares, or — for a directory it
    /// does not — the directory's own name under `$SYNTY_STORE`.
    pub pack: String,
    /// Where it is in that pack, written with `/` on every platform: the
    /// same spelling a `source` line in the manifest carries, so an entry
    /// worth using can be pasted straight into one.
    pub source: String,
    pub sha256: String,
    /// The manifest id naming this same file, when one does. It is what
    /// turns the catalogue into an answer to "have I already used this?"
    pub asset: Option<String>,
    /// The atlas the preview was painted with. For an asset the manifest
    /// declares this is that asset's own `texture` line; for anything
    /// else it is the pack atlas this tool picked, and the difference
    /// matters because the second one is a guess.
    pub atlas: Option<String>,
    /// The images the preview scene actually bore, comma separated.
    /// Empty means the mesh rendered untextured, which is worth knowing
    /// before believing anything the description says about colour.
    pub textures: String,
    /// One to three sentences. See [`sentence`] for what is done to it
    /// before it lands here.
    pub description: String,
    /// **What sets this one apart from the others with its name.**
    ///
    /// A description is written about one mesh, and a mesh in a family of
    /// five numbered variants cannot be described in a way that tells it
    /// from the other four — the model has not seen them. Five panels
    /// come back as five sentences that all say "low, elongated, dark
    /// grey housing with a recessed pale panel", differing in adjectives
    /// rather than in fact, which is worse than saying nothing: a reader
    /// cannot tell which of those differences are real.
    ///
    /// So this line comes from a second look, at the whole family at
    /// once. Absent for a mesh with no siblings, and absent — rather than
    /// guessed — whenever the comparison did not happen or could not be
    /// matched back to this member by name.
    pub differs: Option<String>,
    /// What wrote that sentence: a model slug, the program `$ART_DESCRIBER`
    /// named, or `measurements alone` for the entry nothing looked at.
    /// A description is only worth what the thing that wrote it saw, and
    /// this is the field that says which.
    pub described_by: String,
    pub triangles: u64,
    pub meshes: u64,
    pub materials: u64,
    /// How big it is in the file's own units, across each axis — the
    /// converter's measurement doubled, in glTF's axes. Synty's FBX files
    /// import in metres, so this reads as metres for every pack this has
    /// been pointed at.
    pub size: [f32; 3],
}

impl Entry {
    /// The one-line form: what `cargo xtask art dex` prints, and what
    /// makes a catalogue of four hundred meshes skimmable.
    ///
    /// The star is "the manifest already names this one", which is the
    /// first thing to know about a search result and the one fact the
    /// file name cannot carry.
    pub fn line(&self) -> String {
        format!(
            "{:<32} {:>7} tris  {:<18} {}",
            format!(
                "{}{}",
                self.name,
                if self.asset.is_some() { " *" } else { "" }
            ),
            self.triangles,
            format!(
                "{:.2}x{:.2}x{:.2}",
                self.size[0], self.size[1], self.size[2]
            ),
            self.description
        )
    }

    /// Whether this entry answers to `needle` — its name, its path, or
    /// anything the description says. Searching the description is the
    /// entire point of writing one: "the crate with the hazard stripes"
    /// is not a file name anybody has.
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_lowercase();
        [
            self.name.as_str(),
            self.source.as_str(),
            self.description.as_str(),
            self.textures.as_str(),
        ]
        .iter()
        .any(|field| field.to_ascii_lowercase().contains(&needle))
    }
}

/// **One pack's catalogue: the file, and what is in it.**
///
/// One file per pack rather than one for everything, because a pack is
/// the unit a person buys, searches and stops caring about, and a single
/// file would be every pack in the library in one diff.
pub struct Dex {
    pub path: PathBuf,
    pub entries: BTreeMap<String, Entry>,
}

impl Dex {
    /// Read the catalogue for one pack, or start an empty one. A file
    /// that is not there is not a failure — it is the ordinary state
    /// before the first description — but a file that is there and
    /// unreadable is, because the alternative is silently overwriting
    /// somebody's catalogue with one entry.
    pub fn open(dir: &Path, pack: &str) -> Result<Self, String> {
        let path = dir.join(format!("{}.toml", id_of(pack)));
        if !path.is_file() {
            return Ok(Self {
                path,
                entries: BTreeMap::new(),
            });
        }
        let text = crate::fsx::read_to_string(&path)?;
        let entries = read(&path, &text).map_err(|complaint| complaint.to_string())?;
        Ok(Self {
            entries: entries
                .into_iter()
                .map(|entry| (entry.id.clone(), entry))
                .collect(),
            path,
        })
    }

    /// What this file is already described as, if the description was
    /// written against the bytes that are there now. Matched on the
    /// source path as well as the digest, because a pack update that
    /// changes the bytes is the same catalogue line wanting a new
    /// description rather than a second line.
    pub fn described(&self, source: &str, sha256: &str) -> Option<&Entry> {
        self.entries
            .values()
            .find(|entry| entry.source == source && entry.sha256 == sha256)
    }

    /// **A free id for a mesh, which is its name unless its name is
    /// taken.** Two folders in one pack can hold two different meshes
    /// with the same file name, and a catalogue that filed the second
    /// under the first's id would describe one and lose the other.
    pub fn free_id(&self, name: &str, source: &str) -> String {
        let wanted = id_of(name);
        if self
            .entries
            .get(&wanted)
            .is_none_or(|held| held.source == source)
        {
            return wanted;
        }
        for suffix in 2..1000 {
            let candidate = format!("{wanted}_{suffix}");
            if self
                .entries
                .get(&candidate)
                .is_none_or(|held| held.source == source)
            {
                return candidate;
            }
        }
        wanted
    }

    /// File an entry under a free id, which is the id it already had if
    /// it already had one. The id is assigned here rather than by the
    /// caller so that the one rule about collisions lives in the one
    /// place that can see the whole catalogue.
    ///
    /// **A comparison outlives a re-description of the same bytes.**
    /// `differs` is bought by a second look at a whole family, and a
    /// `--force` over one member would otherwise throw it away for every
    /// member of every family it touched. It is carried over only while
    /// the mesh is the same mesh: a pack update changes the digest, and a
    /// comparison of the old geometry is not a fact about the new.
    pub fn insert(&mut self, mut entry: Entry) {
        entry.id = self.free_id(&entry.name, &entry.source);
        if entry.differs.is_none()
            && let Some(had) = self.entries.get(&entry.id)
            && had.sha256 == entry.sha256
        {
            entry.differs.clone_from(&had.differs);
        }
        self.entries.insert(entry.id.clone(), entry);
    }

    pub fn write(&self) -> Result<(), String> {
        let text = render(&self.entries.values().collect::<Vec<_>>());
        crate::fsx::write(&self.path, &text)?;
        // Read back what was just written, for the reason the index is
        // read back: this is a file something else parses later, and a
        // catalogue that will not load is a long way from the run that
        // wrote it.
        read(&self.path, &text).map_err(|complaint| {
            format!("the catalogue this run wrote cannot be read back: {complaint}")
        })?;
        Ok(())
    }
}

/// Every catalogue in the directory, for the reader that was given no
/// pack to look in. Unreadable ones are complained about and skipped:
/// one hand-edited file should not hide the other ninety-nine.
pub fn open_all(dir: &Path) -> (Vec<Dex>, Vec<String>) {
    let mut found = Vec::new();
    let mut trouble = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    for path in paths {
        match crate::fsx::read_to_string(&path)
            .and_then(|text| read(&path, &text).map_err(|complaint| complaint.to_string()))
        {
            Ok(entries) => found.push(Dex {
                entries: entries
                    .into_iter()
                    .map(|entry| (entry.id.clone(), entry))
                    .collect(),
                path,
            }),
            Err(why) => trouble.push(why),
        }
    }
    (found, trouble)
}

/// **The family a mesh belongs to**: its name with the variant number
/// taken off the end.
///
/// `SM_Prop_Light_Panel_01` and `SM_Prop_Light_Panel_05` are two of one
/// thing and `SM_Prop_Light_Panel` is what that thing is called. A single
/// trailing letter goes the same way, because `_A`/`_B` is the other
/// spelling Synty use for the same idea.
///
/// Only the last one comes off. `SM_Bld_Wall_Corner_01` is not in a
/// family with `SM_Bld_Wall_01` — a corner is a different piece, not a
/// variant of a wall — and stripping twice would put them together.
#[must_use]
pub fn family_of(name: &str) -> &str {
    let Some((stem, last)) = name.rsplit_once('_') else {
        return name;
    };
    let variant = !last.is_empty()
        && (last.bytes().all(|byte| byte.is_ascii_digit())
            || (last.len() == 1 && last.as_bytes()[0].is_ascii_alphabetic()));
    if variant && !stem.is_empty() {
        stem
    } else {
        name
    }
}

/// **A name as this dialect spells names**: lowercase letters, digits and
/// underscores. `SM_Prop_Crate_01` becomes `sm_prop_crate_01`, and a
/// directory called `POLYGON - Sci-Fi City` becomes
/// `polygon_sci_fi_city`.
pub fn id_of(name: &str) -> String {
    let mut id = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    let id = id.trim_matches('_').to_owned();
    if id.is_empty() || id.starts_with(|c: char| c.is_ascii_digit()) {
        // A table name has to be a name, and the dialect's names do not
        // begin with a digit. `01_crate` is a file somebody has.
        return format!("mesh_{id}");
    }
    id
}

/// **What a describer's answer is allowed to be**, before it goes in a
/// file people read.
///
/// Three things happen to it, and each one is a thing a model does. It
/// arrives with newlines in it, and this dialect's strings are one line.
/// It arrives with quotation marks in it, and this dialect's strings
/// carry no escapes — a `"` in a value would end the value and the rest
/// of the sentence would be a parse error in somebody's catalogue. And it
/// arrives, now and then, as an essay, when what was asked for was three
/// sentences.
///
/// So: whitespace collapses, quotes and backslashes become apostrophes
/// and slashes, and anything past [`LONGEST`] is cut. Cutting rather than
/// refusing, because a description one sentence too long is still worth
/// having and a run that refused it would have spent the model call
/// anyway.
///
/// **The cut prefers the end of a sentence.** Models overshoot a
/// character budget — they cannot count — and the first real answer this
/// ever got ran forty characters long and ended `giving it a`. A
/// catalogue line that stops mid-clause reads like a bug in the tool
/// rather than a long answer, so a full stop anywhere in the last two
/// fifths of the budget ends the line there, and only a description with
/// no sentence in it at all gets an ellipsis.
pub fn sentence(said: &str) -> String {
    let cleaned: String = said
        .chars()
        .map(|character| match character {
            '"' | '\u{201c}' | '\u{201d}' => '\'',
            '\\' => '/',
            other if other.is_control() => ' ',
            other => other,
        })
        .collect();
    let mut description = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if description.chars().count() > LONGEST {
        let cut: String = description.chars().take(LONGEST).collect();
        let finished = cut
            .rfind(['.', '!', '?'])
            .filter(|end| *end >= cut.len() * 3 / 5);
        description = finished.map_or_else(
            || {
                let end = cut.rfind(' ').unwrap_or(cut.len());
                format!("{}...", cut[..end].trim_end_matches([',', ';']))
            },
            |end| cut[..=end].to_owned(),
        );
    }
    description
}

/// How long a description may be, in characters. Three lines of a
/// terminal, near enough, which is the length that was asked for and the
/// length that stays skimmable in a list of four hundred.
pub const LONGEST: usize = 320;

/// What an entry with no description says, so that "nothing looked at
/// this" is never mistaken for a mesh nobody could say anything about.
pub const UNDESCRIBED: &str = "measurements alone";

/// The catalogue as text, ready to write.
pub fn render(entries: &[&Entry]) -> String {
    use std::fmt::Write as _;
    let mut text = String::from(
        "# Written by `cargo xtask art describe`. One table per mesh in one pack.\n\
         #\n\
         # This is a CATALOGUE, not a manifest. `art/manifest.toml` is promises\n\
         # somebody typed and something checks; every `description` here is a\n\
         # sentence a vision model wrote about a picture of the mesh, and nothing\n\
         # in this repository can check it. Search it, read it, then look at the\n\
         # mesh — `described_by` says what saw it.\n\
         #\n\
         # It carries no art: names, digests, counts and English, which is the\n\
         # same kind of thing the manifest already holds. See docs/ART_PIPELINE.md.\n\
         #\n\
         #   name        the file's own stem, as the pack spells it\n\
         #   pack        the `[pack.*]` id the manifest declares, or the directory\n\
         #   source      where it is in that pack — paste this into a manifest\n\
         #   asset       the manifest id already naming this file, if one does\n\
         #   atlas       what the preview was painted with; a guess unless `asset`\n\
         #   textures    the images the preview scene actually bore\n\
         #   differs     what sets it apart from the others of its name, written\n\
         #               from a second look at the whole family side by side\n\
         #   size        how big it is across each axis, in the file's own units\n",
    );
    for entry in entries {
        let _ = write!(
            text,
            "\n[mesh.{}]\nname = \"{}\"\npack = \"{}\"\nsource = \"{}\"\nsha256 = \"{}\"\n",
            entry.id, entry.name, entry.pack, entry.source, entry.sha256
        );
        if let Some(asset) = &entry.asset {
            let _ = writeln!(text, "asset = \"{asset}\"");
        }
        if let Some(atlas) = &entry.atlas {
            let _ = writeln!(text, "atlas = \"{atlas}\"");
        }
        let _ = write!(
            text,
            "textures = \"{}\"\ndescription = \"{}\"\n",
            entry.textures, entry.description,
        );
        if let Some(differs) = &entry.differs {
            let _ = writeln!(text, "differs = \"{differs}\"");
        }
        let _ = write!(
            text,
            "described_by = \"{}\"\ntriangles = {}\nmeshes = {}\nmaterials = {}\nsize = {}\n",
            entry.described_by,
            entry.triangles,
            entry.meshes,
            entry.materials,
            manifest::triple(entry.size),
        );
    }
    text
}

/// Read a catalogue back, in the dialect the manifest is written in.
pub fn read(path: &Path, text: &str) -> Result<Vec<Entry>, Complaint> {
    let mut grouped = manifest::group(path, manifest::scan(path, text)?, &["mesh"])?;
    let mut entries = Vec::new();
    for (id, mut draft) in grouped.remove("mesh").unwrap_or_default() {
        let count = |draft: &mut manifest::Draft, key: &str| -> Result<u64, Complaint> {
            Ok(draft
                .take_optional_count(key, "mesh", &id, path)?
                .unwrap_or(0))
        };
        let entry = Entry {
            name: draft.take_str("name", "mesh", &id, path)?,
            pack: draft.take_str("pack", "mesh", &id, path)?,
            source: draft.take_str("source", "mesh", &id, path)?,
            sha256: draft.take_str("sha256", "mesh", &id, path)?,
            asset: draft.take_optional_str("asset", "mesh", &id, path)?,
            atlas: draft.take_optional_str("atlas", "mesh", &id, path)?,
            textures: draft
                .take_optional_str("textures", "mesh", &id, path)?
                .unwrap_or_default(),
            description: draft.take_str("description", "mesh", &id, path)?,
            differs: draft.take_optional_str("differs", "mesh", &id, path)?,
            described_by: draft
                .take_optional_str("described_by", "mesh", &id, path)?
                .unwrap_or_else(|| UNDESCRIBED.to_owned()),
            triangles: count(&mut draft, "triangles")?,
            meshes: count(&mut draft, "meshes")?,
            materials: count(&mut draft, "materials")?,
            size: draft.take_triple("size", "mesh", &id, path, [0.0; 3])?,
            id,
        };
        draft.refuse_leftovers("mesh", &entry.id, path)?;
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            id: "sm_prop_crate_01".to_owned(),
            name: "SM_Prop_Crate_01".to_owned(),
            pack: "scifi_space".to_owned(),
            source: "SourceFiles/FBX/SM_Prop_Crate_01.fbx".to_owned(),
            sha256: "a".repeat(64),
            asset: Some("crate_small".to_owned()),
            atlas: Some("PolygonSciFiSpace_Texture_01_A.png".to_owned()),
            textures: "PolygonSciFiSpace_Texture_01_A.png".to_owned(),
            description: "A squat six-panel supply crate with recessed corner braces and a \
                          hazard stripe along one edge."
                .to_owned(),
            differs: Some("The only one of the five with a hazard stripe.".to_owned()),
            described_by: "deepseek/deepseek-v4-flash-vision-exp".to_owned(),
            triangles: 412,
            meshes: 1,
            materials: 1,
            size: [0.727_51, 0.613_194, 0.672_727],
        }
    }

    /// **A catalogue survives being written down and read back.** It is
    /// the only artefact this command produces, the run that produced it
    /// cost a Blender launch and a model call per line, and a file that
    /// cannot be read back is that money spent on nothing.
    #[allow(clippy::float_cmp)]
    #[test]
    fn a_described_mesh_survives_being_written_down_and_read_back() {
        let written = entry();
        let text = render(&[&written]);
        let entries = read(Path::new("dex.toml"), &text).expect("its own dialect");
        assert_eq!(entries.len(), 1);
        let back = &entries[0];
        assert_eq!(back.id, written.id);
        assert_eq!(back.name, written.name);
        assert_eq!(back.source, written.source);
        assert_eq!(back.sha256, written.sha256);
        assert_eq!(back.asset, written.asset);
        assert_eq!(back.atlas, written.atlas);
        assert_eq!(back.textures, written.textures);
        assert_eq!(back.description, written.description);
        assert_eq!(back.described_by, written.described_by);
        assert_eq!(back.triangles, 412);
        assert_eq!(back.meshes, 1);
        assert_eq!(back.materials, 1);
        assert_eq!(back.size, written.size);

        // An entry for a mesh no manifest names yet, which is most of
        // what a catalogue holds: both absences survive as absences.
        let bare = render(&[&Entry {
            asset: None,
            atlas: None,
            ..entry()
        }]);
        let entries = read(Path::new("dex.toml"), &bare).expect("its own dialect");
        assert_eq!(entries[0].asset, None);
        assert_eq!(entries[0].atlas, None);
    }

    /// **A model's answer is made safe for the file before it goes in
    /// one.** The dialect's strings are one line and carry no escapes, so
    /// a newline or a quotation mark in an answer is not a wrong
    /// description — it is a catalogue that no longer parses, and the
    /// next run reads the whole file as broken.
    #[test]
    fn an_answer_with_quotes_and_newlines_in_it_still_makes_a_file_that_parses() {
        let said = "A crate stencilled \"CARGO\".\n\nIt has a \\ scratch across the lid.\r\n";
        let cleaned = sentence(said);
        assert!(!cleaned.contains('"'), "{cleaned}");
        assert!(!cleaned.contains('\\'), "{cleaned}");
        assert!(!cleaned.contains('\n'), "{cleaned}");
        let text = render(&[&Entry {
            description: cleaned.clone(),
            ..entry()
        }]);
        let entries = read(Path::new("dex.toml"), &text).expect("a sentence with quotes in it");
        assert_eq!(entries[0].description, cleaned);
        assert!(entries[0].description.contains("CARGO"), "{cleaned}");
    }

    /// **An answer that ran long finishes at a sentence.** Three lines
    /// was the length asked for; models cannot count characters, and the
    /// first real answer this pipeline got ran over and ended `giving it
    /// a`. A line that stops mid-clause reads as a broken tool, so the
    /// cut lands on a full stop where there is one in reach.
    #[test]
    fn an_answer_longer_than_three_lines_is_cut_where_a_sentence_ends() {
        let overrun = format!(
            "A squat, cube-shaped storage container with a chunky industrial look. \
             The bright yellow-orange lid carries a recessed grey panel, a blue stripe \
             separates it from the dark grey body, and shallow vertical lines break up \
             each face. {}",
            "It also has a small recessed handle on the front face, giving it a ".repeat(2)
        );
        assert!(overrun.chars().count() > LONGEST, "the fixture is not long");
        let cut = sentence(&overrun);
        assert!(cut.chars().count() <= LONGEST, "{}", cut.chars().count());
        assert!(cut.ends_with("break up each face."), "{cut}");
        assert!(
            !cut.contains("..."),
            "there was a sentence to end on: {cut}"
        );

        // And a description with no sentence in it anywhere still gets
        // cut, at a word, and says that it was.
        let essay = "crate ".repeat(200);
        let cut = sentence(&essay);
        assert!(
            cut.chars().count() <= LONGEST + 3,
            "{}",
            cut.chars().count()
        );
        assert!(cut.ends_with("..."), "{cut}");
        assert!(!cut.contains("cra."), "cut in the middle of a word: {cut}");
    }

    /// **Two meshes with one name get two tables.** A pack holds
    /// `Props/SM_Crate.fbx` and `Buildings/SM_Crate.fbx`, and a catalogue
    /// that filed the second under the first's id would describe one mesh
    /// and quietly lose the other.
    #[test]
    fn two_meshes_with_the_same_file_name_do_not_become_one_table() {
        fn filed(dex: &Dex, id: &str) -> Option<String> {
            dex.entries.get(id).map(|entry| entry.source.clone())
        }
        let mut dex = Dex {
            path: PathBuf::from("dex.toml"),
            entries: BTreeMap::new(),
        };
        let one = |source: &str| Entry {
            name: "SM_Crate".to_owned(),
            source: source.to_owned(),
            ..entry()
        };
        dex.insert(one("Props/SM_Crate.fbx"));
        dex.insert(one("Buildings/SM_Crate.fbx"));
        assert_eq!(dex.entries.len(), 2, "two meshes became one table");
        assert_eq!(
            filed(&dex, "sm_crate").as_deref(),
            Some("Props/SM_Crate.fbx")
        );
        assert_eq!(
            filed(&dex, "sm_crate_2").as_deref(),
            Some("Buildings/SM_Crate.fbx")
        );

        // And describing the same file again is an update, not a third
        // table: the id it already has is the id it keeps.
        dex.insert(one("Props/SM_Crate.fbx"));
        assert_eq!(dex.entries.len(), 2);
        assert_eq!(
            filed(&dex, "sm_crate").as_deref(),
            Some("Props/SM_Crate.fbx")
        );
    }

    /// **A file name becomes a name this dialect can carry.** Every id
    /// here is derived from a file name somebody at Synty chose, and the
    /// dialect's names are lowercase letters, digits and underscores —
    /// so the derivation has to answer for spaces, dashes, dots and a
    /// name that starts with a digit.
    #[test]
    fn a_file_name_becomes_a_name_the_dialect_allows() {
        for (name, wanted) in [
            ("SM_Prop_Crate_01", "sm_prop_crate_01"),
            ("POLYGON - Sci-Fi City", "polygon_sci_fi_city"),
            ("SM Bld Wall.001", "sm_bld_wall_001"),
            ("01_crate", "mesh_01_crate"),
            ("!!!", "mesh_"),
        ] {
            let id = id_of(name);
            assert_eq!(id, wanted, "`{name}`");
            assert!(
                id.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "`{id}` is not a name this dialect can hold"
            );
        }
    }

    /// **Numbered variants of one thing are one family, and pieces that
    /// merely share a prefix are not.**
    ///
    /// Six tenths of a real library is in a family, so this rule decides
    /// what most of the catalogue gets compared against. Both mistakes it
    /// could make are bad in the same way — a family that swept in
    /// unrelated meshes would have the comparison spend its line on
    /// differences nobody was choosing between, and a family that split
    /// would compare a panel against nothing.
    #[test]
    fn numbered_variants_of_one_thing_are_one_family() {
        for (name, family) in [
            ("SM_Prop_Light_Panel_01", "SM_Prop_Light_Panel"),
            ("SM_Prop_Light_Panel_05", "SM_Prop_Light_Panel"),
            ("SM_Prop_Crate_A", "SM_Prop_Crate"),
            // Only the last one comes off: a corner is a different piece
            // from a wall, not a variant of one.
            ("SM_Bld_Wall_Corner_01", "SM_Bld_Wall_Corner"),
            // Nothing to take off, so it is its own family of one.
            ("SM_Prop_Barrel", "SM_Prop_Barrel"),
            ("SM_Prop_BarrelStack_01", "SM_Prop_BarrelStack"),
            // `cube` is not a variant number, so this is its own family:
            // `unit_cube` and `unit_pyramid` are two things, not two of
            // one thing.
            ("unit_cube", "unit_cube"),
            ("nounderscores", "nounderscores"),
            ("_01", "_01"),
        ] {
            assert_eq!(family_of(name), family, "`{name}`");
        }
        // And the ones that must not land together.
        assert_ne!(
            family_of("SM_Prop_Barrel_01"),
            family_of("SM_Prop_BarrelStack_01"),
            "a barrel and a stack of barrels are not variants of each other"
        );
    }

    /// **The catalogue is searched by what it says, not only by what
    /// things are called.** A file name is the thing that failed to be
    /// searchable in the first place; "hazard stripe" is the search
    /// somebody actually has.
    #[test]
    fn an_entry_answers_to_a_word_from_its_description() {
        let entry = entry();
        assert!(entry.matches("hazard"));
        assert!(entry.matches("HAZARD"));
        assert!(entry.matches("crate_01"));
        assert!(!entry.matches("pyramid"));
    }
}
