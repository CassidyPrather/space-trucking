//! The workspace's errand runner. One errand so far: the art resolver.
//!
//! The problem it solves is a licence. Synty's terms let their meshes
//! ship inside a built game and forbid redistributing them as source, so
//! the payload cannot be in this repository — not in git, not in LFS on a
//! public remote. What can be in the repository is a reference, and this
//! is the thing that turns a reference into a file the game can load:
//!
//! ```text
//! art/manifest.toml   ids, packs, paths, digests, per-asset overrides
//! $SYNTY_STORE        the packs exactly as downloaded, on the owner's disk
//! art/cache/          the few files a manifest named, converted, gitignored
//! art/dex/            what the meshes in those packs look like, in English
//! ```
//!
//! The last of those is the second errand, and it is about the other end
//! of the same problem: a manifest line has to name a file, and a Synty
//! library is five thousand files per pack called `SM_Prop_Crate_04`.
//! `cargo xtask art describe` renders each mesh, measures it, shows the
//! picture to a vision model and writes down what it looks like; `cargo
//! xtask art dex` searches that. See [`dex`].
//!
//! Continuous integration never runs any of this and never will: the
//! payload is not in the repository, so there is nothing for it to
//! resolve. CI keeps building and testing the whitebox, which is where
//! every gauntlet family and every determinism guard lives. What CI does
//! run is the guards in this package, which are about the resolver's own
//! rules and need no art at all.
//!
//! See `docs/ART_PIPELINE.md`.

mod archive;
mod cache;
mod convert;
mod describe;
mod dex;
mod fsx;
mod json;
mod manifest;
mod preview;
mod sha256;
mod store;
mod unitypackage;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use cache::{Cache, Converted};
use convert::Converter;
use describe::{Describer, Sibling, Subject};
use manifest::{Asset, Bounds, Manifest, Pack, Resolved};
use preview::Previewer;
use store::{Found, Hit, Store, count, slashed};

fn usage() -> String {
    format!(
        "\
cargo xtask art <command>

  check            find every asset the manifest names, hash it, and report; converts nothing
  resolve          check, then convert anything not already in the cache, then write the index
  hash [id ...]    print the `sha256` lines to paste into the manifest
  unpack <pack>    rebuild the asset trees inside one pack's .unitypackage files
  find <text>      search the packs — inside their archives too — and print the manifest lines
  describe [text]  render each mesh, measure it, and write down what it looks like
  dex [text]       read that catalogue back, searching what it says as well as what things
                   are called

Options for `describe`:
  --pack <id>      one whole pack, by the id art/manifest.toml gives it or by its own
                   directory name; reads that pack instead of searching the store
  --limit <n>      how many found meshes to describe (default {limit}); a named pack
                   and the manifest's own assets are not capped
  --jobs <n>       how many to look at at once (default {jobs})
  --model <slug>   a vision model other than {model}
  --offline        render and measure, ask nothing, and say so in every entry
  --force          describe again what has already been described

Environment:
  SYNTY_STORE      where the packs are, one directory per pack, as downloaded (required)
  ART_MANIFEST     a manifest other than art/manifest.toml
  ART_CACHE        a cache other than art/cache
  ART_DEX          a catalogue directory other than art/dex
  BLENDER          the Blender executable, if it is not on PATH
  ART_CONVERTER    a program run as `<program> <source> <destination.glb> [texture]`,
                   instead of Blender
  ART_PREVIEW      a program run as `<program> <jobs file>`, instead of Blender, for
                   the pictures `describe` renders; one `source|picture.png|texture`
                   per line in, one `look <n>` block per job out
  ART_PREVIEW_SIZE how many pixels square one preview sheet of four views is (256)
  OPENROUTER_API_KEY  the key `describe` reaches a hosted vision model with
  ART_DESCRIBER_MODEL the same thing `--model` says
  ART_DESCRIBER    a program run as `<program> <prompt.txt> <picture.png>...` printing
                   the answer, instead of a hosted model — one picture for a mesh, one
                   per member when a family is being told apart
",
        limit = DESCRIBE_LIMIT,
        jobs = DESCRIBE_JOBS,
        model = describe::MODEL,
    )
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let outcome = match words.as_slice() {
        ["art", "check"] => report(false),
        ["art", "resolve"] => report(true),
        ["art", "hash", ids @ ..] => hashes(ids),
        ["art", "unpack", pack] => unpack(pack),
        ["art", "find", needle @ ..] if !needle.is_empty() => find(&needle.join(" ")),
        ["art", "describe", rest @ ..] => describe(rest),
        ["art", "dex", rest @ ..] => catalogue(rest),
        _ => {
            eprint!("{}", usage());
            std::process::exit(2);
        }
    };
    if let Err(complaint) = outcome {
        eprintln!("\nart: {complaint}");
        std::process::exit(1);
    }
}

/// The repository this binary was built out of. An xtask is run from the
/// tree it lives in, so this is exact rather than a search upwards.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root")
        .to_path_buf()
}

fn manifest_path() -> PathBuf {
    std::env::var_os("ART_MANIFEST")
        .map_or_else(|| repo().join("art").join("manifest.toml"), PathBuf::from)
}

fn read_manifest() -> Result<Manifest, String> {
    Manifest::read(&manifest_path()).map_err(|complaint| complaint.to_string())
}

/// What one asset turned into on this machine.
struct Sourced {
    digest: String,
    found: Found,
    /// The atlas the manifest declared for it, if it declared one.
    atlas: Option<Atlas>,
    /// How this asset's conversion is addressed in the cache: the mesh,
    /// the atlas above, and the script that will read them.
    converted: Converted,
    /// Set when the manifest carried no digest for it yet.
    unrecorded: bool,
}

/// **The atlas one asset declared, found on this machine and hashed.**
///
/// Hashed here rather than at conversion time because the digest is half
/// of what the converted file is named after, and the name is what a run
/// consults before it decides there is nothing to do. An atlas found only
/// when a conversion happens is an atlas whose change nothing notices.
struct Atlas {
    from: PathBuf,
    digest: String,
}

/// Find every asset, hash it, and — when `converting` — convert and index
/// it. Every failure is collected rather than returned, because trawling
/// a pack by hand is exactly what this tool exists to stop, and being
/// told about one missing asset at a time is that same trawl.
fn report(converting: bool) -> Result<(), String> {
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    println!(
        "art: {} over {}, from {}",
        count(manifest.assets.len(), "asset"),
        count(manifest.packs.len(), "pack"),
        store.root.display()
    );
    if manifest.assets.is_empty() {
        println!(
            "art: {} names no assets yet. See docs/ART_PIPELINE.md for how to add one.",
            manifest.path.display()
        );
        return Ok(());
    }

    let mut sourced = Vec::new();
    let mut troubles: Vec<String> = Vec::new();
    for asset in manifest.assets.values() {
        match source(&store, &cache, &manifest, asset) {
            Ok(one) => sourced.push((asset, one)),
            Err(trouble) => troubles.push(trouble),
        }
    }
    for (asset, one) in &sourced {
        println!(
            "  {:<24} {:<14} {}  {}",
            asset.id,
            one.found.via(),
            &one.digest[..12],
            one.found.path().display()
        );
    }
    let unrecorded: Vec<&str> = sourced
        .iter()
        .filter(|(_, one)| one.unrecorded)
        .map(|(asset, _)| asset.id.as_str())
        .collect();
    if !unrecorded.is_empty() {
        println!(
            "art: no digest written down yet for {}; `cargo xtask art hash` prints the lines",
            unrecorded.join(", ")
        );
    }
    if !troubles.is_empty() {
        for trouble in &troubles {
            println!("\n{trouble}");
        }
        return Err(format!(
            "{} of {} assets {} not on this machine",
            troubles.len(),
            manifest.assets.len(),
            if troubles.len() == 1 { "is" } else { "are" }
        ));
    }
    if !converting {
        println!("art: every asset found. `cargo xtask art resolve` converts them.");
        return Ok(());
    }
    convert_all(&cache, &sourced)
}

/// Find one asset's bytes and its atlas, and check the bytes against the
/// line that named them.
fn source(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    asset: &Asset,
) -> Result<Sourced, String> {
    let found = store::locate(store, cache, manifest, asset, true)
        .map_err(|missing| missing.to_string())?;
    let digest = sha256::of_file(found.path())
        .map_err(|err| format!("cannot hash {}: {err}", found.path().display()))?;
    if asset.sha256 != Asset::NO_DIGEST_YET && asset.sha256 != digest {
        return Err(format!(
            "{} is here and is not the file the manifest was written against.\n\n  \
             file      {}\n  \
             recorded  {}\n  \
             on disk   {}\n  \
             declared  {}:{}\n\n  \
             fix       Either the pack was updated under you, or this is a different pack\n            \
             version. Look at it, and when it is the file you want, replace the\n            \
             `sha256` line with the one `cargo xtask art hash {}` prints.",
            asset.id,
            found.path().display(),
            asset.sha256,
            digest,
            manifest.path.display(),
            asset.line,
            asset.id,
        ));
    }
    let atlas = atlas(store, cache, manifest, asset)?;
    let converted = Converted::of(&digest, atlas.as_ref().map(|one| one.digest.as_str()));
    Ok(Sourced {
        unrecorded: asset.sha256 == Asset::NO_DIGEST_YET,
        digest,
        found,
        atlas,
        converted,
    })
}

/// **The atlas an asset's `texture` line declared, found and hashed.**
///
/// A declaration and not a hope: the file named here is the one the
/// converter is handed, whatever the mesh file believes about where its
/// own texture lives. Looked for on every run and not only on the runs
/// that convert, because its digest is part of what the converted file is
/// named after — see [`Atlas`].
fn atlas(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    asset: &Asset,
) -> Result<Option<Atlas>, String> {
    let Some(texture) = &asset.texture else {
        return Ok(None);
    };
    let pack = manifest.pack_of(asset);
    let from = store::find_relative(store, cache, pack, texture).ok_or_else(|| {
        format!(
            "{} names texture `{texture}`, and it is not in {}",
            asset.id,
            store.pack_dir(pack).display()
        )
    })?;
    let digest =
        sha256::of_file(&from).map_err(|err| format!("cannot hash {}: {err}", from.display()))?;
    Ok(Some(Atlas { from, digest }))
}

fn convert_all(cache: &Cache, sourced: &[(&Asset, Sourced)]) -> Result<(), String> {
    let wanted: Vec<&(&Asset, Sourced)> = sourced
        .iter()
        .filter(|(_, one)| !cache.glb(&one.converted).is_file())
        .collect();
    let converter = if wanted.is_empty() {
        None
    } else {
        Some(convert::find()?)
    };
    if let Some(converter) = &converter {
        println!(
            "art: converting {} of {} with {}",
            wanted.len(),
            sourced.len(),
            converter.describe()
        );
    }
    for (asset, one) in &wanted {
        let converter = converter.as_ref().expect("a converter when there is work");
        let stage = cache.stage(&one.digest);
        fsx::remove_dir_all(&stage)?;
        let name = one
            .found
            .path()
            .file_name()
            .ok_or_else(|| format!("{} resolved to something with no file name", asset.id))?;
        let staged = stage.join(name);
        fsx::copy(one.found.path(), &staged)?;
        let painted = match &one.atlas {
            None => None,
            Some(atlas) => {
                let leaf = atlas.from.file_name().expect("a texture file has a name");
                // Two copies, because an FBX that names its own texture
                // names it by a relative path, and the two spellings
                // Synty's exporters use are the file beside the mesh and
                // the file in a Textures folder. An FBX that names one
                // still wins; the third argument below is for the FBX
                // that names nothing, which is the common Synty case.
                let beside = stage.join(leaf);
                fsx::copy(&atlas.from, &beside)?;
                fsx::copy(&atlas.from, &stage.join("Textures").join(leaf))?;
                Some(beside)
            }
        };
        let measured = converter.run(
            cache,
            &staged,
            &cache.glb(&one.converted),
            painted.as_deref(),
        )?;
        write_bounds(cache, &one.converted, measured)?;
        println!(
            "  converted {:<24} -> {}{}",
            asset.id,
            one.converted.relative(),
            measured.map_or_else(
                || String::from("  (unmeasured)"),
                |bounds| format!("  {} half-units", manifest::triple(bounds.half))
            )
        );
    }
    index_all(cache, sourced, converter.as_ref())
}

/// **Where the promise meets the fact, and the index gets written.**
///
/// Split from the conversion above because it is a different job asked of
/// the same list: conversion is about files this run had to make, and this
/// is about every asset the manifest names, cached or converted a moment
/// ago. A `fill` line has to be true on the run that converted nothing as
/// much as on the one that converted everything.
fn index_all(
    cache: &Cache,
    sourced: &[(&Asset, Sourced)],
    converter: Option<&Converter>,
) -> Result<(), String> {
    // **Every promise is checked, and the ones that fail are reported
    // together.** The same rule the missing-asset report follows: being
    // told about one wrong `fill`, fixing it, and being told about the
    // next is the trawl this tool exists to stop.
    let mut resolved = Vec::new();
    let mut broken: Vec<String> = Vec::new();
    let mut unmeasured: Vec<&str> = Vec::new();
    for (asset, one) in sourced {
        let measured = read_bounds(cache, &one.converted);
        if let Some(measured) = measured {
            if let Some(trouble) = manifest::fill_trouble(asset, measured) {
                broken.push(trouble);
            }
        } else if asset.dresses.is_some() {
            unmeasured.push(asset.id.as_str());
        }
        resolved.push(Resolved {
            id: asset.id.clone(),
            glb: one.converted.relative(),
            sha256: one.digest.clone(),
            dresses: asset.dresses.clone(),
            scale: asset.scale,
            offset: asset.offset,
            rotation: asset.rotation,
            fill: asset.fill,
            measured,
        });
    }
    if !unmeasured.is_empty() {
        println!(
            "art: {} said nothing about the size of {}, so their `fill` lines went\n     \
             unchecked. The Blender script this package ships reports its bounds; a\n     \
             converter of your own can print `aabb <min x y z> <max x y z>` and be checked\n     \
             the same way.",
            converter.map_or_else(|| "the converter".to_owned(), Converter::describe),
            unmeasured.join(", ")
        );
    }
    if !broken.is_empty() {
        for trouble in &broken {
            println!("\n{trouble}");
        }
        return Err(format!(
            "{} {} the size the manifest says {} is",
            count(broken.len(), "asset"),
            if broken.len() == 1 {
                "is not"
            } else {
                "are not"
            },
            if broken.len() == 1 { "it" } else { "they" },
        ));
    }
    let text = manifest::render_index(&resolved);
    fsx::write(&cache.index(), &text)?;
    // Read back what was just written. The index is the one file here
    // that something else will parse later, and a build that finds it
    // malformed will be a long way from the code that wrote it.
    manifest::read_index(&cache.index(), &text).map_err(|complaint| {
        format!("the index this run wrote cannot be read back: {complaint}")
    })?;
    println!("art: wrote {}", cache.index().display());
    Ok(())
}

/// Keep what the converter measured, beside what it wrote. Six numbers
/// in the order the converter prints them, because a file this small
/// wants no dialect of its own.
fn write_bounds(
    cache: &Cache,
    converted: &Converted,
    measured: Option<Bounds>,
) -> Result<(), String> {
    let Some(measured) = measured else {
        // A converter that measured nothing leaves no file, so a later
        // run cannot mistake silence for a measurement of zero.
        return Ok(());
    };
    let numbers: Vec<String> = (0..3)
        .map(|axis| measured.mid[axis] - measured.half[axis])
        .chain((0..3).map(|axis| measured.mid[axis] + measured.half[axis]))
        .map(|value| format!("{value}"))
        .collect();
    fsx::write(&cache.bounds(converted), &numbers.join(" "))
}

/// What a previous run measured, if it measured anything. A file that
/// cannot be read or does not hold six numbers is the same answer as no
/// file: unmeasured, and said so.
fn read_bounds(cache: &Cache, converted: &Converted) -> Option<Bounds> {
    let text = std::fs::read_to_string(cache.bounds(converted)).ok()?;
    let numbers: Vec<f32> = text
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    let box3 = <[f32; 6]>::try_from(numbers.as_slice()).ok()?;
    Some(Bounds {
        mid: [0, 1, 2].map(|axis| f32::midpoint(box3[axis], box3[axis + 3])),
        half: [0, 1, 2].map(|axis| (box3[axis + 3] - box3[axis]) * 0.5),
    })
}

/// The `sha256` lines, ready to paste. Printed rather than written into
/// the manifest, because the manifest is a file somebody has commented
/// and this tool has no business rewriting it.
fn hashes(ids: &[&str]) -> Result<(), String> {
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let mut trouble = 0;
    for asset in manifest.assets.values() {
        if !ids.is_empty() && !ids.contains(&asset.id.as_str()) {
            continue;
        }
        match store::locate(&store, &cache, &manifest, asset, true) {
            Ok(found) => {
                let digest = sha256::of_file(found.path())
                    .map_err(|err| format!("cannot hash {}: {err}", found.path().display()))?;
                println!(
                    "# [asset.{}], {}:{}",
                    asset.id,
                    manifest.path.display(),
                    asset.line
                );
                println!("sha256 = \"{digest}\"");
            }
            Err(missing) => {
                println!("\n{missing}");
                trouble += 1;
            }
        }
    }
    if trouble > 0 {
        return Err(format!(
            "{} {} not on this machine",
            count(trouble, "asset"),
            if trouble == 1 { "is" } else { "are" }
        ));
    }
    Ok(())
}

fn unpack(pack_id: &str) -> Result<(), String> {
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let pack = manifest.packs.get(pack_id).ok_or_else(|| {
        format!(
            "no `[pack.{pack_id}]` in {}; it names {}",
            manifest.path.display(),
            manifest
                .packs
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let dir = store.pack_dir(pack);
    let packages = store::find_packages(&dir);
    if packages.is_empty() {
        return Err(format!(
            "no .unitypackage anywhere under {}\n\n  \
             That is normal for a pack downloaded as Source Files, and that download is\n  \
             the better one to resolve from anyway. This command is for the packs that\n  \
             only ship a Unity build.",
            dir.display()
        ));
    }
    for package in packages {
        let into = cache.unpacked(&pack.id).join(
            package
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package"),
        );
        let report = unitypackage::unpack(&package, &into)?;
        println!(
            "art: {}, {}  {} -> {}",
            count(report.files, "file"),
            count(report.folders, "folder"),
            package.display(),
            into.display()
        );
    }
    Ok(())
}

/// Search the packs, and print each hit as the manifest line that would
/// name it. A Synty pack is thousands of files with names nobody guesses,
/// it arrives zipped, and reading names off a store page is the part that
/// does not scale. So this looks inside the archives as well, and looking
/// inside one costs a read of its table of contents rather than an
/// unpacked pack.
///
/// **What it finds is grouped by the pack holding it, and the packs the
/// manifest declares are printed first.** A library is a hundred packs
/// and an ordinary word like `crate` is four hundred files in it, so the
/// answer has to be cut somewhere. Cutting it in whatever order the packs
/// were walked hid every match in the pack its owner was working in, and
/// left that pack reading as though it held no crates at all. The
/// manifest is this project's own statement of which packs it cares
/// about, so its packs rank above everything else and are cut last.
///
/// **Every directory that matched says how many matches it has**, fitted
/// or not. "This pack has thirty-eight crates in it" is most of what
/// somebody browsing a hundred packs is after, and it is the part a cap
/// throws away first.
fn find(needle: &str) -> Result<(), String> {
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let unpacked = cache.root.join("unpacked");
    let roots = vec![store.root.clone(), unpacked.clone()];
    let search = store::search(&roots, needle, &cache);
    for complaint in &search.trouble {
        eprintln!("art: {complaint}");
    }
    if search.hits.is_empty() {
        println!(
            "art: nothing under {} or {} has `{needle}` in its name",
            store.root.display(),
            unpacked.display()
        );
        return Ok(());
    }
    // A set per group, because one file can be found twice — once inside
    // the pack's archive and once in the copy of it the cache already
    // holds — and the two are the same manifest line. Sorted, so the same
    // search reads the same way twice running.
    let mut grouped: BTreeMap<Where, BTreeSet<String>> = BTreeMap::new();
    for hit in &search.hits {
        let (place, line) = placed(&manifest, &store, &unpacked, hit);
        grouped.entry(place).or_default().insert(line);
    }
    print_hits(&manifest, &store, needle, &grouped);
    if search.unread > 0 {
        println!(
            "\nart: {} sat beside a Source Files archive and {} not\n     \
             read. That archive holds the same meshes and costs a table of contents\n     \
             to read; a .unitypackage is a gzipped tar and costs the whole file.\n     \
             `cargo xtask art unpack <pack>` rebuilds one into the cache, which this\n     \
             searches.",
            many(search.unread, ".unitypackage file", ".unitypackage files"),
            if search.unread == 1 { "was" } else { "were" }
        );
    }
    Ok(())
}

/// How many matches a search prints, spent in the order the groups are
/// printed — so the packs the manifest declares have first claim on it.
/// A hundred is about two screens: enough to read, and short enough that
/// the per-directory counts under it are still on the screen.
const SHOWN: usize = 100;

/// How many matches one directory the manifest does not declare may
/// show.
/// Without it, one unrelated pack with four hundred crates in it spends
/// the whole budget and the other hundred directories print counts alone.
const PER_UNDECLARED: usize = 4;

/// One group of matches: a pack the manifest declares, or a directory
/// under the store that it does not.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Where {
    /// 0 for a pack `art/manifest.toml` declares and 1 for everything
    /// else. This is the whole of the ranking, and it is the manifest
    /// being worth something: a search that buries the two packs this
    /// project named under a store's worth of packs it did not is a
    /// search answering somebody else's question.
    tier: u8,
    /// Which line declared the pack, so the declared ones read in the
    /// order the manifest writes them rather than alphabetically.
    rank: usize,
    /// What stands above the group. For a pack the manifest declares it
    /// is the `pack` line to paste; for a directory it does not, the
    /// directory's own name, which is what a `dir` line would carry.
    heading: String,
}

fn declared(pack: &manifest::Pack) -> Where {
    Where {
        tier: 0,
        rank: pack.line,
        heading: format!("pack = \"{}\"", pack.id),
    }
}

const fn elsewhere(heading: String) -> Where {
    Where {
        tier: 1,
        rank: 0,
        heading,
    }
}

/// A word that does not take a plain `s`. [`store::count`] has the ones
/// that do.
fn many(count: usize, one: &str, more: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {more}")
    }
}

fn print_hits(
    manifest: &Manifest,
    store: &Store,
    needle: &str,
    grouped: &BTreeMap<Where, BTreeSet<String>>,
) {
    let total: usize = grouped.values().map(BTreeSet::len).sum();
    let (mine, mut rest): (Vec<_>, Vec<_>) = grouped.iter().partition(|(place, _)| place.tier == 0);
    // Most matches first among the packs nobody declared, because with
    // nothing else to go on the biggest pile is the likeliest answer.
    rest.sort_by(|(one, ones), (two, twos)| {
        twos.len()
            .cmp(&ones.len())
            .then_with(|| one.heading.cmp(&two.heading))
    });
    println!(
        "art: `{needle}` matches {}, in {} under {}",
        many(total, "file", "files"),
        many(grouped.len(), "pack directory", "pack directories"),
        store.root.display()
    );
    println!();
    let mut budget = SHOWN;
    // Whether the last group left a blank line under itself. A run of
    // groups with no room for a single match is a tight column of counts,
    // which is the point of them, and the closing line needs its own gap.
    let mut spaced = true;
    if !mine.is_empty() {
        println!(
            "In the {} {} declares:\n",
            many(mine.len(), "pack", "packs"),
            manifest.path.display()
        );
        for (place, lines) in mine {
            let shown = print_place(place, lines, budget);
            budget -= shown;
            spaced = shown > 0;
        }
    }
    if !rest.is_empty() {
        println!(
            "In {} it does not:\n",
            many(rest.len(), "directory", "directories")
        );
        for (place, lines) in rest {
            let shown = print_place(place, lines, budget.min(PER_UNDECLARED));
            budget -= shown;
            spaced = shown > 0;
        }
    }
    let hidden = total - (SHOWN - budget);
    if hidden > 0 {
        if !spaced {
            println!();
        }
        println!(
            "art: {hidden} of the {total} are not shown, and every directory above says how\n     \
             many it has. A pack the manifest declares is printed first and cut last;\n     \
             `dir` on its `[pack]` table is the directory name printed here."
        );
    }
}

/// One group: what to call it, how many matches it holds, and as many of
/// them as the budget left. The count is printed whether or not a single
/// match is, because a directory that goes unmentioned is a directory
/// that reads as empty.
fn print_place(place: &Where, lines: &BTreeSet<String>, budget: usize) -> usize {
    let shown = lines.len().min(budget);
    println!(
        "{:<48} {}",
        place.heading,
        many(lines.len(), "match", "matches")
    );
    for line in lines.iter().take(shown) {
        println!("  {line}");
    }
    if shown > 0 {
        if shown < lines.len() {
            println!("  ... and {} more here", lines.len() - shown);
        }
        println!();
    }
    shown
}

/// One hit, as the group it belongs to and the manifest line that would
/// name it.
///
/// Which key that line uses is the difference between a line that
/// resolves and a line that does not. A file inside a pack's raw archive
/// is `source`, because that archive is the Source Files download and the
/// resolver reads it in place. A file inside a `.unitypackage` is
/// `unity`. A file in the cache is spelled after the archive it came out
/// of, because the cache is not what a manifest points at.
fn placed(manifest: &Manifest, store: &Store, unpacked: &Path, hit: &Hit) -> (Where, String) {
    match hit {
        Hit::Inside { archive, member } => (
            place_of(manifest, store, unpacked, archive),
            format!("{} = \"{member}\"", key_of(archive)),
        ),
        Hit::Loose(path) => loose(manifest, store, unpacked, path),
    }
}

/// Which group something in the store belongs to: the pack the manifest
/// declares that holds it, or the directory under `$SYNTY_STORE` that
/// does. Something in the cache is filed under the pack it was rebuilt
/// for, because the cache is a copy of a pack rather than a pack.
fn place_of(manifest: &Manifest, store: &Store, unpacked: &Path, path: &Path) -> Where {
    if let Some(pack) = manifest
        .packs
        .values()
        .find(|pack| path.starts_with(store.pack_dir(pack)))
    {
        return declared(pack);
    }
    let relative = path
        .strip_prefix(unpacked)
        .or_else(|_| path.strip_prefix(&store.root));
    if let Ok(relative) = relative
        && relative.components().count() > 1
        && let Some(first) = relative.components().next()
    {
        return by_name(manifest, &first.as_os_str().to_string_lossy());
    }
    elsewhere(path.parent().unwrap_or(path).display().to_string())
}

/// A directory name, as the group it stands for. The cache files a
/// rebuilt tree under the pack's own id, so a name out of there can be a
/// pack the manifest declares.
fn by_name(manifest: &Manifest, name: &str) -> Where {
    manifest
        .packs
        .get(name)
        .map_or_else(|| elsewhere(name.to_owned()), declared)
}

fn loose(manifest: &Manifest, store: &Store, unpacked: &Path, path: &Path) -> (Where, String) {
    if let Some((pack, rest)) = manifest.packs.values().find_map(|pack| {
        path.strip_prefix(store.pack_dir(pack))
            .ok()
            .map(|rest| (pack, rest))
    }) {
        return (declared(pack), format!("source = \"{}\"", slashed(rest)));
    }
    if let Ok(rest) = path.strip_prefix(unpacked) {
        let mut parts = rest.components();
        // <pack>/<archive file name>/... — the first two are the cache's
        // own filing, and the rest is the path the archive stored.
        if let (Some(pack), Some(archive)) = (parts.next(), parts.next()) {
            let inside: PathBuf = parts.collect();
            return (
                by_name(manifest, &pack.as_os_str().to_string_lossy()),
                format!(
                    "{} = \"{}\"",
                    key_of(Path::new(archive.as_os_str())),
                    slashed(&inside)
                ),
            );
        }
    }
    if let Ok(rest) = path.strip_prefix(&store.root)
        && rest.components().count() > 1
    {
        let mut parts = rest.components();
        let first = parts.next().expect("more than one component");
        let inside: PathBuf = parts.collect();
        return (
            by_name(manifest, &first.as_os_str().to_string_lossy()),
            format!("source = \"{}\"", slashed(&inside)),
        );
    }
    (
        elsewhere(path.parent().unwrap_or(path).display().to_string()),
        slashed(Path::new(path.file_name().unwrap_or(path.as_os_str()))),
    )
}

// ---------------------------------------------------------------------
// The catalogue: `describe`, which writes it, and `dex`, which reads it
// ---------------------------------------------------------------------

/// How many found meshes one `describe` run looks at before it stops and
/// says how many it left. A cap, because `describe crate` over a
/// hundred-pack library is four hundred meshes, four hundred Blender
/// launches and four hundred hosted model calls — a bill, arriving
/// because somebody typed a common word. `--limit` raises it, and the
/// manifest's own assets are never capped: that list is short,
/// deliberate, and already in git.
const DESCRIBE_LIMIT: usize = 24;

/// **How many chunks are looked at at once.**
///
/// Eight, measured rather than picked. A chunk is one Blender launch and
/// then a model call per mesh in it, and with the picture down to 256
/// square the launch is no longer the expensive half — the network is, so
/// the useful number is higher than the core count. Over 128 real meshes
/// on a twelve-core machine: 0.82 s each at four, 0.54 s at eight, 0.46 s
/// at twelve. Eight takes most of what there is to take and leaves the
/// machine usable.
const DESCRIBE_JOBS: usize = 8;

/// Where the catalogue lives. In the repository, beside the manifest,
/// because it carries the same kind of thing the manifest does — names,
/// digests, counts and English — and because a gitignored copy would be
/// re-bought with Blender launches and model calls on every clone.
fn dex_dir() -> PathBuf {
    std::env::var_os("ART_DEX").map_or_else(|| repo().join("art").join("dex"), PathBuf::from)
}

/// What one `describe` run was asked for.
struct Wanted {
    /// What to search the packs for. Absent means the manifest's own
    /// assets, which is the run somebody makes after adding a line —
    /// unless `pack` names one, when it means the whole of that pack.
    needle: Option<String>,
    /// One pack, by the id `art/manifest.toml` gives it or by its own
    /// directory name under `$SYNTY_STORE`.
    pack: Option<String>,
    /// Absent means the default, which is not the same number for a pack
    /// somebody named as for a word somebody searched — see [`Wanted::limit`].
    limit: Option<usize>,
    jobs: usize,
    model: Option<String>,
    offline: bool,
    force: bool,
}

impl Wanted {
    /// **How many meshes this run will look at.**
    ///
    /// A search is capped, because `describe crate` over a library is
    /// hundreds of calls arriving because somebody typed a common word.
    /// **A pack is not**: naming one is naming exactly the work, and the
    /// unit somebody actually catalogues is a pack — a median one is 225
    /// meshes, two minutes and two cents. `--limit` still overrules
    /// either.
    fn limit(&self) -> usize {
        self.limit.unwrap_or_else(|| {
            if self.pack.is_some() {
                usize::MAX
            } else {
                DESCRIBE_LIMIT
            }
        })
    }
}

impl Wanted {
    /// **Read the arguments, and refuse an option nobody has.** The same
    /// rule the manifest dialect follows, for the same reason: a
    /// misspelled `--limt 200` that was quietly ignored is a run that
    /// describes twenty-four meshes and a person who thinks they asked
    /// for two hundred.
    fn parse(words: &[&str]) -> Result<Self, String> {
        let mut wanted = Self {
            needle: None,
            pack: None,
            limit: None,
            jobs: DESCRIBE_JOBS,
            model: None,
            offline: false,
            force: false,
        };
        let mut text: Vec<&str> = Vec::new();
        let mut at = 0;
        while at < words.len() {
            let word = words[at];
            at += 1;
            match word {
                "--force" => wanted.force = true,
                "--offline" => wanted.offline = true,
                "--limit" | "--jobs" | "--model" | "--pack" => {
                    let value = *words.get(at).ok_or_else(|| {
                        format!("`{word}` wants a value after it, and the arguments end there")
                    })?;
                    at += 1;
                    match word {
                        "--limit" => wanted.limit = Some(whole(word, value)?.max(1)),
                        "--jobs" => wanted.jobs = whole(word, value)?.clamp(1, 16),
                        "--pack" => wanted.pack = Some(value.to_owned()),
                        _ => wanted.model = Some(value.to_owned()),
                    }
                }
                other if other.starts_with('-') => {
                    return Err(format!("no option called `{other}`\n\n{}", usage()));
                }
                other => text.push(other),
            }
        }
        wanted.needle = (!text.is_empty()).then(|| text.join(" "));
        Ok(wanted)
    }
}

fn whole(option: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("`{option} {value}` wants a whole number"))
}

/// One mesh this run is going to look at.
#[derive(Clone)]
struct Candidate {
    /// The pack's directory under `$SYNTY_STORE`. Carried so that a mesh
    /// can be traced back to the pack it came out of without the manifest
    /// — which is what the second look at a mesh needs, to take the
    /// texture that mesh asked for out of the same pack.
    dir: String,
    /// The pack id the catalogue files it under: the manifest's, or the
    /// store directory's own name for a pack it does not declare.
    pack: String,
    /// What the pack is called, which is what the describer is told.
    title: String,
    /// Pack-relative, `/` separated: the spelling a manifest `source`
    /// line carries, so an entry worth using can be pasted into one.
    source: String,
    name: String,
    /// Where the bytes actually are on this machine.
    path: PathBuf,
    digest: String,
    /// The atlas to paint the preview with: the manifest's `texture`
    /// line, or the pack's own atlas, guessed.
    atlas: Option<PathBuf>,
    /// The manifest id already naming this file, if one does.
    asset: Option<String>,
    /// The pack a borrowed atlas came out of, when it came out of one
    /// that is not this mesh's own. A weaker answer than its own pack's
    /// texture, and the catalogue says so by naming the lender.
    borrowed: Option<String>,
    /// A texture this mesh named that nothing in its pack answers to.
    /// The flag: its picture was painted with something else, so the
    /// description written from that picture is a suspect line.
    unresolved: Option<String>,
}

impl Candidate {
    /// The pack it came out of, as something that can be looked in.
    fn pack_of(&self) -> Pack {
        Pack {
            id: self.pack.clone(),
            title: self.title.clone(),
            dir: self.dir.clone(),
            download: self.title.clone(),
            line: 0,
        }
    }
}

/// **Render, measure and describe, and write the catalogue.**
fn describe(words: &[&str]) -> Result<(), String> {
    let wanted = Wanted::parse(words)?;
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let dir = dex_dir();

    let candidates = match (&wanted.pack, &wanted.needle) {
        // A pack somebody named is looked in rather than searched for,
        // which is also the difference between listing one zip's table
        // and walking a hundred packs to find the same meshes.
        (Some(pack), needle) => in_one_pack(
            &store,
            &cache,
            &manifest,
            pack,
            needle.as_deref().unwrap_or(""),
            wanted.limit(),
        )?,
        (None, Some(needle)) => found_candidates(&store, &cache, &manifest, needle, wanted.limit()),
        (None, None) => declared_candidates(&store, &cache, &manifest),
    };
    if candidates.is_empty() {
        return Err(nothing_to_describe(&wanted, &manifest, &store));
    }

    let mut books: BTreeMap<String, dex::Dex> = BTreeMap::new();
    for candidate in &candidates {
        if !books.contains_key(&candidate.pack) {
            books.insert(
                candidate.pack.clone(),
                dex::Dex::open(&dir, &candidate.pack)?,
            );
        }
    }
    let (work, known): (Vec<Candidate>, Vec<Candidate>) =
        candidates.into_iter().partition(|candidate| {
            wanted.force
                || books[&candidate.pack]
                    .described(&candidate.source, &candidate.digest)
                    .is_none()
        });
    if !known.is_empty() {
        println!(
            "art: {} already described against the bytes on this machine; `--force` \
             describes {} again",
            many(known.len(), "mesh", "meshes"),
            if known.len() == 1 { "it" } else { "them" }
        );
    }
    if work.is_empty() {
        return Ok(());
    }

    let previewer = preview::find()?;
    let (describer, note) = chosen(&wanted);
    let work = one_per_digest(work);
    let chunk = chunk_size(work.len(), wanted.jobs);
    println!(
        "art: looking at {} with {}, {} at a time per launch",
        many(work.len(), "mesh", "meshes"),
        previewer.describe(),
        chunk
    );
    println!("{note}");
    let script = previewer.prepare(&cache)?;
    let run = Run {
        cache: &cache,
        script: &script,
        previewer: &previewer,
        describer: &describer,
    };
    let books = Mutex::new(books);
    let mut done = look_at_all(&run, &work, chunk, wanted.jobs, &books, true);

    // **What the deferred meshes asked for, and then another look.**
    // A mesh whose own texture was not in the pack directory got a
    // picture of the pack's atlas stretched over coordinates meant for
    // something else. Taking the file it named out of the pack — into
    // the tree it was exported from, where the mesh's own relative path
    // finds it — is what makes the second look right.
    if !done.deferred.is_empty() {
        let again = fetch_wanted(&store, &cache, &manifest, &work, &done.deferred);
        let over = chunk_size(again.len(), wanted.jobs);
        done.absorb(look_at_all(&run, &again, over, wanted.jobs, &books, false));
    }

    // Then the second look, at the families among what is now in the
    // catalogue. A mesh described on its own cannot be told from the four
    // others with its name — see `compare_all`.
    let titles: BTreeMap<String, String> = work
        .iter()
        .flatten()
        .map(|candidate| (candidate.pack.clone(), candidate.title.clone()))
        .collect();
    let (compared, complaints) = compare_all(
        &run,
        &books,
        &done.described,
        wanted.force,
        wanted.jobs,
        &titles,
    );
    if compared > 0 {
        println!(
            "art: {} told from its siblings",
            many(compared, "mesh", "meshes")
        );
    }
    done.troubles.extend(complaints);
    said(&done)
}

/// What a run says about itself when it is over: where the work went,
/// what went wrong, and whether "wrong" was the whole of it.
fn said(done: &Done) -> Result<(), String> {
    for path in &done.files {
        println!("art: wrote {}", path.display());
    }
    for trouble in &done.troubles {
        println!("\n{trouble}");
    }
    if done.written == 0 {
        return Err(format!(
            "{} could not be described, and nothing was written",
            many(done.troubles.len(), "mesh", "meshes")
        ));
    }
    if !done.troubles.is_empty() {
        println!(
            "\nart: {} described, {} not",
            many(done.written, "mesh", "meshes"),
            done.troubles.len()
        );
    }
    Ok(())
}

/// **How many meshes go into one launch of the previewer.**
///
/// Blender costs about 2.2 seconds to start whatever it is then asked to
/// do, so a launch per mesh spent a third of its life starting up. A
/// chunk amortises that — and is capped, for two reasons that pull the
/// same way: a chunk is the unit that is lost if a launch dies, and it is
/// also the unit that gets written to the catalogue, so a long overnight
/// run should be saving its work every minute or two rather than at the
/// end.
///
/// The floor is the other half of it. With four workers and eight meshes,
/// a chunk of thirty-two would put every mesh in one launch and leave
/// three workers holding nothing.
const CHUNK_MOST: usize = 32;

fn chunk_size(work: usize, jobs: usize) -> usize {
    work.div_ceil(jobs.max(1)).clamp(1, CHUNK_MOST)
}

/// **Identical bytes are one mesh, however many names a pack gives it.**
///
/// A pack ships the same geometry under two names about one time in a
/// hundred, and everything about this command is addressed by the digest
/// of that geometry — the picture it renders, the prompt it writes beside
/// it, the answer it files. Two names racing over one set of those files
/// is how a description ends up in the catalogue under a name it was not
/// written about, which is exactly what happened: three copies of one
/// mesh, and the third came back describing the second.
///
/// Looking once and giving every name the same answer fixes that, and it
/// is also the truer answer. Describing one mesh twice produces two
/// sentences that differ in adjectives and not in fact — the same defect
/// the family pass exists to fix, arriving from the other direction.
/// What tells the copies apart is their names, and that is the family
/// pass's job.
fn one_per_digest(work: Vec<Candidate>) -> Vec<Vec<Candidate>> {
    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for candidate in work {
        if let Some(&at) = seen.get(&candidate.digest) {
            groups[at].push(candidate);
        } else {
            seen.insert(candidate.digest.clone(), groups.len());
            groups.push(vec![candidate]);
        }
    }
    groups
}

/// **Which describer this run uses, and the sentence saying so.**
///
/// `--offline` and "there is nothing to ask with" produce the same
/// catalogue and are not the same sentence: one is a choice and the other
/// is a machine that has not been set up, and somebody who typed the flag
/// does not want to be told their key is missing.
fn chosen(wanted: &Wanted) -> (Describer, String) {
    if wanted.offline {
        return (
            Describer::Measurements,
            String::from(
                "art: --offline, so every entry carries its measurements and says so. The\n     \
                 pictures are still rendered and the counts are still true.",
            ),
        );
    }
    let describer = describe::find(wanted.model.clone());
    let note = describer.announce();
    (describer, note)
}

/// The message for a run that found nothing to do, which is a different
/// sentence depending on what was asked for.
fn nothing_to_describe(wanted: &Wanted, manifest: &Manifest, store: &Store) -> String {
    wanted.needle.as_ref().map_or_else(
        || {
            if manifest.assets.is_empty() {
                return format!(
                    "{} names no assets yet, so there is nothing of its own to describe.\n\n  \
                     `cargo xtask art describe <text>` catalogues what is in the packs \
                     themselves,\n  which is the half of this that helps before a line is \
                     written.",
                    manifest.path.display()
                );
            }
            // The assets are named and none of them is here, which the
            // lines above have already said one by one. Saying "nothing
            // to describe" without this would read as a manifest that
            // holds nothing.
            format!(
                "not one of the {} {} names is on this machine, so there was nothing to \
                 look at.\n\n  `cargo xtask art check` is the command that is about that.",
                count(manifest.assets.len(), "asset"),
                manifest.path.display()
            )
        },
        |needle| {
            format!(
                "nothing under {} is a mesh called `{needle}`.\n\n  \
                 `cargo xtask art find {needle}` shows everything of that name, meshes and\n  \
                 textures and prefabs alike; this describes only what Blender can open.",
                store.root.display()
            )
        },
    )
}

/// The manifest's own assets, found on this machine and hashed. An asset
/// that is not here is reported and skipped rather than stopping the run:
/// a catalogue of the four assets that are present is worth more than a
/// refusal about the fifth.
fn declared_candidates(store: &Store, cache: &Cache, manifest: &Manifest) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    for asset in manifest.assets.values() {
        let found = match store::locate(store, cache, manifest, asset, true) {
            Ok(found) => found,
            Err(missing) => {
                eprintln!("art: {missing}");
                continue;
            }
        };
        let digest = match sha256::of_file(found.path()) {
            Ok(digest) => digest,
            Err(err) => {
                eprintln!("art: cannot hash {}: {err}", found.path().display());
                continue;
            }
        };
        let pack = manifest.pack_of(asset);
        candidates.push(Candidate {
            pack: pack.id.clone(),
            title: pack.title.clone(),
            dir: pack.dir.clone(),
            unresolved: None,
            borrowed: None,
            source: asset.source.clone(),
            name: stem(&asset.source),
            path: found.path().clone(),
            digest,
            atlas: atlas(store, cache, manifest, asset)
                .unwrap_or_else(|why| {
                    eprintln!("art: {why}");
                    None
                })
                .map(|atlas| atlas.from),
            asset: Some(asset.id.clone()),
        });
    }
    candidates
}

/// **Every mesh in one named pack.**
///
/// A pack is the unit somebody actually catalogues, and naming one is
/// worth more than a shortcut for a search: the store-wide sweep walks a
/// hundred pack directories and reads the table of every archive in them
/// — twelve seconds before any work starts, and a good deal more on a
/// cold disk — where this reads one directory and one zip.
///
/// The pack is named by the id `art/manifest.toml` gives it or by its own
/// directory name, because the person typing it has one of those two in
/// front of them and should not have to know which this wants. A name
/// that is neither is a refusal listing what is there, since the usual
/// reason is a typo and the second-usual is a pack that has not been
/// downloaded.
fn in_one_pack(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    named: &str,
    needle: &str,
    limit: usize,
) -> Result<Vec<Candidate>, String> {
    let pack = pack_named(manifest, store, named)?;
    let dir = store.pack_dir(&pack);
    let unpacked = cache.unpacked(&pack.id);
    let search = store::search(&[dir, unpacked], needle, cache);
    for complaint in &search.trouble {
        eprintln!("art: {complaint}");
    }
    Ok(gather(store, cache, manifest, &search, limit, needle))
}

/// **Which pack somebody meant.** The manifest's id, the store directory
/// of that name, or the one whose name slugs to it — the three spellings
/// of a pack anybody has to hand.
fn pack_named(manifest: &Manifest, store: &Store, named: &str) -> Result<Pack, String> {
    if let Some(pack) = manifest.packs.get(named) {
        return Ok(copy_of(pack));
    }
    let found = by_directory(manifest, store, named);
    if store.pack_dir(&found).is_dir() {
        return Ok(found);
    }
    let mut there: Vec<String> = manifest.packs.keys().cloned().collect();
    there.extend(
        std::fs::read_dir(&store.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned()),
    );
    there.sort();
    there.dedup();
    Err(format!(
        "no pack called `{named}`: it is neither an id {} declares nor a directory\n  \
         under {}. These are there:\n\n    {}",
        manifest.path.display(),
        store.root.display(),
        there.join("\n    ")
    ))
}

/// **Every mesh in the store whose name matches, up to the cap.**
///
/// The same search `find` runs, cut to the files something can actually
/// open and resolved to bytes on this machine — which is what separates a
/// catalogue from a listing. A hit that cannot be resolved is counted and
/// mentioned rather than reported one by one: the usual reason is a
/// `.unitypackage` nothing has rebuilt, and the answer to that is one
/// command for the whole pack.
fn found_candidates(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    needle: &str,
    limit: usize,
) -> Vec<Candidate> {
    let search = store::search(
        &[store.root.clone(), cache.root.join("unpacked")],
        needle,
        cache,
    );
    for complaint in &search.trouble {
        eprintln!("art: {complaint}");
    }
    gather(store, cache, manifest, &search, limit, needle)
}

/// **What a search turned up, as meshes this run can actually look at.**
///
/// Shared by the two ways of choosing them — a word across the store, or
/// one named pack — because everything after "which files" is the same
/// question: which of these are meshes, where are their bytes, and what
/// paints them. A hit that cannot be resolved is counted and mentioned
/// rather than reported one by one: the usual reason is a
/// `.unitypackage` nothing has rebuilt, and the answer to that is one
/// command for the whole pack.
fn gather(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    search: &store::Search,
    limit: usize,
    needle: &str,
) -> Vec<Candidate> {
    let unpacked = cache.root.join("unpacked");
    // One entry per pack-relative path: the same mesh is commonly found
    // twice, once inside the pack's archive and once in the copy of it
    // the cache already holds, and they are one catalogue line.
    let mut wanted: BTreeMap<(String, String), Pack> = BTreeMap::new();
    for hit in &search.hits {
        let name = match hit {
            Hit::Loose(path) => slashed(Path::new(path.file_name().unwrap_or(path.as_os_str()))),
            Hit::Inside { member, .. } => member.clone(),
        };
        if !is_mesh(&name) {
            continue;
        }
        if let Some((pack, source)) = seat(manifest, store, &unpacked, hit) {
            wanted.entry((pack.id.clone(), source)).or_insert(pack);
        }
    }
    let wanted = one_format_per_mesh(wanted);
    let total = wanted.len();
    // **Out of the archives in one go.** One `tar` launch per file is
    // about four tenths of a second each, which was half the wall clock
    // of the first pack-wide sweep; the members of one archive come out
    // together. Only what this run will actually look at, so the cap is
    // applied before the extracting rather than after.
    let taking: Vec<((String, String), &Pack)> = wanted
        .iter()
        .take(limit)
        .map(|(key, pack)| (key.clone(), pack))
        .collect();
    let mut ready: BTreeMap<(String, String), PathBuf> = BTreeMap::new();
    for (pack_id, pack) in packs_of(&taking) {
        let paths: Vec<String> = taking
            .iter()
            .filter(|((id, _), _)| *id == pack_id)
            .map(|((_, source), _)| source.clone())
            .collect();
        for (source, path) in store::take_out_all(store, cache, pack, &paths) {
            ready.insert((pack_id.clone(), source), path);
        }
    }

    let mut atlases: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
    let mut candidates = Vec::new();
    let mut unreachable = 0;
    for ((pack_id, source), pack) in wanted {
        if candidates.len() >= limit {
            break;
        }
        // Whatever the batch above could not place — a mesh that is only
        // inside a `.unitypackage`, say — is asked for on its own, which
        // is the route that knows about those.
        let Some(path) = ready
            .remove(&(pack_id.clone(), source.clone()))
            .or_else(|| store::find_relative(store, cache, &pack, &source))
        else {
            unreachable += 1;
            continue;
        };
        let Ok(digest) = sha256::of_file(&path) else {
            unreachable += 1;
            continue;
        };
        let asset = named_by(manifest, &pack_id, &source);
        // A mesh the manifest names has an atlas somebody DECLARED, and a
        // declaration beats the guess made for the rest of its pack —
        // same rule the converter follows one file over.
        let declared = asset
            .as_ref()
            .and_then(|id| manifest.assets.get(id))
            .and_then(|asset| asset.texture.as_ref())
            .and_then(|texture| store::find_relative(store, cache, &pack, texture));
        let atlas = declared.or_else(|| {
            atlases
                .entry(pack_id.clone())
                .or_insert_with(|| pack_atlas(store, cache, &pack))
                .clone()
        });
        candidates.push(Candidate {
            asset,
            unresolved: None,
            borrowed: None,
            pack: pack_id,
            title: pack.title.clone(),
            dir: pack.dir.clone(),
            name: stem(&source),
            source,
            path,
            digest,
            atlas,
        });
    }
    if total > candidates.len() + unreachable {
        println!(
            "art: {total} meshes {}, and this run takes the first {}. `--limit` raises it.",
            if needle.is_empty() {
                String::from("in it")
            } else {
                format!("match `{needle}`")
            },
            candidates.len()
        );
    }
    if unreachable > 0 {
        println!(
            "art: {} could not be taken out of the pack holding {}. A mesh that is only \
             inside a\n     .unitypackage needs `cargo xtask art unpack <pack>` first.",
            many(unreachable, "match", "matches"),
            if unreachable == 1 { "it" } else { "them" }
        );
    }
    candidates
}

/// **Take the textures the deferred meshes asked for out of their packs,
/// and hand back the meshes to look at again.**
///
/// The file goes wherever the pack keeps it, which is the whole trick:
/// the cache mirrors the archive's own tree, so a mesh at
/// `SourceFiles/FBX/SM_Ivy.fbx` naming `../Textures/Leaf_01.png` finds it
/// there on the second look — and then the material names an image that
/// loads, so `paint_with` leaves it alone and the atlas is not forced
/// over it.
///
/// A name that is nowhere in the pack is left alone: Synty FBX files name
/// `.psd` files that were never shipped, and those will not resolve on
/// any number of passes. The mesh is looked at again regardless, and this
/// time described with whatever it has — one extra render is the whole
/// price of asking.
fn fetch_wanted(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    work: &[Vec<Candidate>],
    deferred: &[(usize, Vec<String>)],
) -> Vec<Vec<Candidate>> {
    let mut asked: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut again = Vec::new();
    for (at, wants) in deferred {
        let Some(group) = work.get(*at) else { continue };
        asked
            .entry(group[0].pack.clone())
            .or_default()
            .extend(wants.iter().cloned());
        again.push(group.clone());
    }
    let mut took = 0;
    let mut borrowed = 0;
    let mut missing: Vec<String> = Vec::new();
    // What each wanted name turned out to be, so the meshes that asked
    // for the same file resolve it once, and which pack it came out of
    // when that was not this one.
    let mut answers: BTreeMap<(String, String), PathBuf> = BTreeMap::new();
    let mut lending: BTreeMap<(String, String), String> = BTreeMap::new();
    for (pack_id, names) in asked {
        let Some(pack) = again
            .iter()
            .find(|group| group[0].pack == pack_id)
            .map(|group| group[0].pack_of())
        else {
            continue;
        };
        for name in names {
            // The file itself, which is the answer that also makes the
            // mesh's own reference resolve on the next look.
            if let Some(path) = store::find_named(store, cache, &pack, &name) {
                took += 1;
                answers.insert((pack_id.clone(), name), path);
                continue;
            }
            // Failing that, the file it MEANT. Synty's exporters name a
            // `.psd` from their own machine, or the name a pack's atlas
            // had two versions ago; the pack ships that texture under a
            // name that is nearly the same. A near name cannot be made
            // to resolve — a reference to a `.psd` is unsatisfiable by
            // any file — so it is handed to the mesh as its atlas
            // instead, which is the one thing that can still paint it.
            if let Some(path) = find_like(store, cache, &pack, &name) {
                answers.insert((pack_id.clone(), name), path);
                continue;
            }
            // And failing THAT, the other packs. A Synty pack is built
            // in a project holding all of them, so its meshes name each
            // other's atlases as readily as their own —
            // `PolygonAncientEgypt_Texture_01.psd` on a mesh in the
            // horror pack. The store has Ancient Egypt in it.
            if let Some((from, path)) = find_elsewhere(store, cache, manifest, &pack, &name) {
                borrowed += 1;
                // Where it came from travels with it. A texture out of
                // another pack is a weaker answer than one out of this
                // one — the name said which pack, not which file — and
                // the catalogue should say so rather than record a file
                // name that looks like any other.
                lending.insert((pack_id.clone(), name.clone()), from);
                answers.insert((pack_id.clone(), name), path);
            } else {
                missing.push(name);
            }
        }
    }
    // Each mesh takes the answer to what IT asked for, in preference to
    // the pack-wide guess it was painted with the first time — and the
    // names nothing answered are written onto it, as the flag saying its
    // picture may be of some other texture entirely.
    for group in &mut again {
        let pack_id = group[0].pack.clone();
        let Some((_, wants)) = deferred.iter().find(|(at, _)| {
            work.get(*at)
                .is_some_and(|had| had[0].source == group[0].source)
        }) else {
            continue;
        };
        let answered = wants
            .iter()
            .find(|name| answers.contains_key(&(pack_id.clone(), (*name).clone())));
        let found = answered
            .and_then(|name| answers.get(&(pack_id.clone(), name.clone())))
            .cloned();
        let from = answered.and_then(|name| lending.get(&(pack_id.clone(), name.clone())).cloned());
        let lost: Vec<String> = wants
            .iter()
            .filter(|name| !answers.contains_key(&(pack_id.clone(), (*name).clone())))
            .cloned()
            .collect();
        for candidate in group.iter_mut() {
            if let Some(atlas) = &found {
                candidate.atlas = Some(atlas.clone());
                candidate.borrowed.clone_from(&from);
            }
            if !lost.is_empty() {
                candidate.unresolved = Some(lost.join(", "));
            }
        }
    }
    println!(
        "art: {} asked for a texture of {} own; took {} out of the packs{} and looked again",
        many(again.len(), "mesh", "meshes"),
        if again.len() == 1 { "its" } else { "their" },
        took,
        if borrowed > 0 {
            format!(", {borrowed} of them out of another pack")
        } else {
            String::new()
        }
    );
    if !missing.is_empty() {
        // Named, because this is the one thing here that no amount of
        // looking will fix: a Synty FBX often names a `.psd` from the
        // machine it was exported on, and no pack has ever carried one.
        // Knowing which file is what tells that apart from a pack that
        // has not been unpacked.
        println!("     the packs carry none of: {}", missing.join(", "));
    }
    again
}

/// The packs a batch of wanted meshes came out of, once each and in a
/// settled order.
fn packs_of<'a>(taking: &[((String, String), &'a Pack)]) -> Vec<(String, &'a Pack)> {
    let mut packs: Vec<(String, &Pack)> = Vec::new();
    for ((pack_id, _), pack) in taking {
        if !packs.iter().any(|(id, _)| id == pack_id) {
            packs.push((pack_id.clone(), *pack));
        }
    }
    packs
}

/// **One entry per mesh, not one per format the pack shipped it in.**
///
/// A Source Files download carries `SourceFiles/FBX/SM_Prop_Barrel_01.fbx`
/// and `SourceFiles/OBJ/SM_Prop_Barrel_01.obj`, which are the same barrel
/// twice. Describing both costs two renders and two model calls to write
/// the same sentence in two places, and a catalogue holding both asks a
/// person to choose between a mesh and itself. That was measured rather
/// than predicted: the first sweep over a real pack described four
/// barrels and two of them were the other two.
///
/// The rule is the best format present for a name in a pack, FBX first —
/// it is the one the resolver prefers and the one that carries materials.
/// Two DIFFERENT meshes that happen to share a name, `Props/SM_Crate.fbx`
/// and `Buildings/SM_Crate.fbx`, are both FBX and both survive; the only
/// thing dropped is one name in a worse format.
fn one_format_per_mesh(
    found: BTreeMap<(String, String), Pack>,
) -> BTreeMap<(String, String), Pack> {
    let mut best: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (pack, source) in found.keys() {
        let rank = format_rank(source);
        best.entry((pack.clone(), stem(source)))
            .and_modify(|had| *had = (*had).min(rank))
            .or_insert(rank);
    }
    found
        .into_iter()
        .filter(|((pack, source), _)| {
            best.get(&(pack.clone(), stem(source)))
                .is_some_and(|best| *best == format_rank(source))
        })
        .collect()
}

/// Which format answers for a mesh when a pack ships several, in the
/// order `fbx_to_gltf.py` lists them.
fn format_rank(source: &str) -> usize {
    ["fbx", "obj", "dae", "glb", "gltf", "blend"]
        .iter()
        .position(|extension| named(source, std::slice::from_ref(extension)))
        .unwrap_or(usize::MAX)
}

/// The manifest id naming this exact file, if one does. It is what turns
/// the catalogue into an answer to "have I used this already?".
fn named_by(manifest: &Manifest, pack: &str, source: &str) -> Option<String> {
    manifest
        .assets
        .values()
        .find(|asset| asset.pack == pack && asset.source == source)
        .map(|asset| asset.id.clone())
}

/// Which files in a pack are meshes something here can open — the same
/// list `fbx_to_gltf.py` knows how to import, because a catalogue entry
/// for a file Blender will refuse is an entry nobody can act on.
fn is_mesh(name: &str) -> bool {
    named(name, &["fbx", "obj", "dae", "glb", "gltf", "blend"])
}

/// Whether a file name ends in one of these extensions, whatever case
/// the pack spelled it in. A Synty pack has `.FBX` and `.Fbx` in it.
fn named(name: &str, extensions: &[&str]) -> bool {
    Path::new(name).extension().is_some_and(|found| {
        extensions
            .iter()
            .any(|extension| found.eq_ignore_ascii_case(extension))
    })
}

/// A path's file name without its extension: what the pack calls the
/// thing, which is the half of a description the picture cannot supply.
fn stem(source: &str) -> String {
    let leaf = source.rsplit('/').next().unwrap_or(source);
    leaf.rsplit_once('.')
        .map_or(leaf, |(stem, _)| stem)
        .to_owned()
}

/// **Which pack a hit belongs to, and what the file is called inside
/// it.**
///
/// A pack the manifest declares, or a stand-in made out of the store
/// directory's own name for one it does not — because browsing a library
/// is precisely the moment before a pack gets declared, and a catalogue
/// that could only describe what was already in the manifest would be a
/// catalogue of the things somebody had already chosen.
fn seat(manifest: &Manifest, store: &Store, unpacked: &Path, hit: &Hit) -> Option<(Pack, String)> {
    match hit {
        Hit::Inside { archive, member } => {
            Some((holder(manifest, store, unpacked, archive)?, member.clone()))
        }
        Hit::Loose(path) => {
            for pack in manifest.packs.values() {
                if let Ok(rest) = path.strip_prefix(store.pack_dir(pack)) {
                    return Some((copy_of(pack), slashed(rest)));
                }
            }
            // <cache>/unpacked/<pack>/<archive file name>/... — the first
            // two components are the cache's own filing.
            let (root, skip) = path.strip_prefix(unpacked).map_or_else(
                |_| (path.strip_prefix(&store.root), 1),
                |rest| (Ok(rest), 2),
            );
            let rest = root.ok()?;
            let mut parts = rest.components();
            let first = parts.next()?;
            for _ in 1..skip {
                parts.next()?;
            }
            let inside: PathBuf = parts.collect();
            if inside.as_os_str().is_empty() {
                return None;
            }
            Some((
                by_directory(manifest, store, &first.as_os_str().to_string_lossy()),
                slashed(&inside),
            ))
        }
    }
}

/// Which pack directory holds a file, as a pack something can look in.
fn holder(manifest: &Manifest, store: &Store, unpacked: &Path, path: &Path) -> Option<Pack> {
    if let Some(pack) = manifest
        .packs
        .values()
        .find(|pack| path.starts_with(store.pack_dir(pack)))
    {
        return Some(copy_of(pack));
    }
    let relative = path
        .strip_prefix(unpacked)
        .or_else(|_| path.strip_prefix(&store.root))
        .ok()?;
    let first = relative.components().next()?;
    Some(by_directory(
        manifest,
        store,
        &first.as_os_str().to_string_lossy(),
    ))
}

/// **A directory name as a pack**, whichever of the two directory names
/// this is.
///
/// A pack is found under two different names, and the difference is a
/// defect that only appears on the second run. In the store it is called
/// what the shop called it — `POLYGON - Apocalypse`. In the cache, which
/// is where the first run left the meshes it took out of that pack's zip,
/// it is filed under the pack's id — `polygon_apocalypse` — and a cached
/// copy sorts before the store's own archive, so the second run finds it
/// first and names the pack after the cache.
///
/// A pack whose `dir` is a slug is a pack whose directory does not exist:
/// nothing can be found in it, so the pack's shared atlas is not found
/// either, and the run goes on to render a whole pack untextured and
/// describe the colour of Blender's missing-texture magenta. That is
/// exactly what happened, on the second sweep of a pack the first sweep
/// had warmed the cache for.
///
/// So a name is resolved back to a directory: the manifest's, if it
/// declares one; the store directory of that name, if there is one; and
/// otherwise the store directory whose own name slugs to it.
fn by_directory(manifest: &Manifest, store: &Store, name: &str) -> Pack {
    if let Some(pack) = manifest.packs.get(name) {
        return copy_of(pack);
    }
    let dir = if store.root.join(name).is_dir() {
        name.to_owned()
    } else {
        std::fs::read_dir(&store.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|directory| dex::id_of(directory) == name)
            .unwrap_or_else(|| name.to_owned())
    };
    Pack {
        id: dex::id_of(&dir),
        title: dir.clone(),
        download: dir.clone(),
        dir,
        line: 0,
    }
}

fn copy_of(pack: &Pack) -> Pack {
    Pack {
        id: pack.id.clone(),
        title: pack.title.clone(),
        dir: pack.dir.clone(),
        download: pack.download.clone(),
        line: pack.line,
    }
}

/// **The atlas a pack paints itself with, guessed.**
///
/// An asset the manifest declares has a `texture` line and never reaches
/// this. Everything else in a pack has nothing, and a preview rendered
/// with nothing is a grey mesh — which a vision model then describes,
/// accurately and uselessly, as grey. A Synty pack paints itself from one
/// shared atlas with `Texture` in the name, so the guess is a good one;
/// it goes into the catalogue's `atlas` field so a reader can see that a
/// guess is what it was.
fn pack_atlas(store: &Store, cache: &Cache, pack: &Pack) -> Option<PathBuf> {
    let mut names: Vec<String> = pack_images(store, cache, pack)
        .into_iter()
        .filter(|name| !is_other_map(name))
        .collect();
    let wanted = words(&pack.dir);
    names.sort_by_key(|name| atlas_rank(&wanted, name));
    names
        .iter()
        .find_map(|name| store::find_relative(store, cache, pack, name))
}

/// **Whether what a mesh asked for is near enough to what it was painted
/// with.**
///
/// Almost every Synty FBX names the `.psd` it was painted from, and for
/// almost all of them the pack's shared atlas IS that texture, shipped as
/// a `.png` under the same name — `PolygonSciFiSpace_Texture_01_A.psd`
/// asked for, `PolygonSciFiSpace_Texture_01_A.png` painted on. Those
/// meshes have exactly what they asked for and there is nothing to look
/// at again.
///
/// So the question is not "did the reference resolve" — for two meshes in
/// three it never does, and treating that as a fault flagged 477 of 702
/// entries in a pack whose pictures were nearly all correct. The question
/// is whether the two names are about the same thing, which is a word
/// they share that is not a number, a single letter, or one of the words
/// every Synty texture has.
fn answered(wants: &[String], atlas: Option<&Path>) -> bool {
    if wants.is_empty() {
        return true;
    }
    let Some(atlas) = atlas.and_then(|path| path.file_name()) else {
        return false;
    };
    let painted = distinctive(&atlas.to_string_lossy());
    wants.iter().any(|wanted| {
        distinctive(wanted)
            .iter()
            .any(|word| painted.contains(word))
    })
}

/// The words of a name that could tell one texture from another: not the
/// ones every Synty texture carries, and not a number or a single letter,
/// which two unrelated atlases share as readily as two related ones.
fn distinctive(name: &str) -> Vec<String> {
    telling(name)
        .into_iter()
        .filter(|word| word.len() > 1 && word.chars().any(char::is_alphabetic))
        .collect()
}

/// **The texture in somebody else's pack.**
///
/// A Synty pack is built in a project that holds all of them, so its
/// meshes name each other's atlases as readily as their own: the horror
/// pack's screens ask for `PolygonAncientEgypt_Texture_01.psd`, and
/// `city.psd` and `PolygonShops_Texture_01.psd` turn up in it too. The
/// store on this machine has Ancient Egypt and Shops in it, so there is
/// nothing here to guess at — the file name says which pack it belongs
/// to.
///
/// So the name picks the pack: a directory sharing a telling word with
/// the wanted file is asked first, and asked the same two questions its
/// own pack was — the file itself, then the file it meant. Only when the
/// name points nowhere is every other pack asked, and then only for an
/// exact name, because a near match across a hundred packs is how a mesh
/// would come to be painted with something out of a different game.
fn find_elsewhere(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    own: &Pack,
    name: &str,
) -> Option<(String, PathBuf)> {
    let wanted = distinctive(name);
    let others: Vec<Pack> = std::fs::read_dir(&store.root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| by_directory(manifest, store, &entry.file_name().to_string_lossy()))
        .filter(|pack| pack.dir != own.dir)
        .collect();

    let (hinted, rest): (Vec<&Pack>, Vec<&Pack>) = others.iter().partition(|pack| {
        let named = distinctive(&pack.dir);
        wanted.iter().any(|word| named.contains(word))
    });
    // Best-named first: `PolygonApocalypse_Texture_01.psd` names four
    // packs with `apocalypse` in them, and `POLYGON - Apocalypse` is the
    // one with nothing else in its name.
    let mut hinted: Vec<&&Pack> = hinted.iter().collect();
    hinted.sort_by_key(|pack| {
        let named = distinctive(&pack.dir);
        (
            std::cmp::Reverse(wanted.iter().filter(|word| named.contains(word)).count()),
            named.len(),
            pack.dir.clone(),
        )
    });

    for pack in &hinted {
        if let Some(path) = store::find_named(store, cache, pack, name)
            .or_else(|| find_like(store, cache, pack, name))
        {
            return Some((pack.dir.clone(), path));
        }
    }
    // **The named pack's own atlas.** A pack's textures are not always
    // named after it — the Vikings pack calls its atlas `Texture_01.png`
    // — so a mesh asking for `PolygonVikings2_Texture_01.psd` names a
    // pack this can find and a file it cannot. Knowing the pack is
    // enough: its atlas is what a mesh of that pack is painted with.
    // Only for the best-named pack, because this is the step that would
    // otherwise reach into a pack that merely shares a word.
    if let Some(pack) = hinted.first()
        && let Some(path) = pack_atlas(store, cache, pack)
    {
        return Some((pack.dir.clone(), path));
    }
    // And failing every hint, the file itself in any pack at all —
    // exactly named, never near, since a near match across a hundred
    // packs is how a mesh gets painted with something from another game.
    rest.iter().find_map(|pack| {
        store::find_named(store, cache, pack, name).map(|path| (pack.dir.clone(), path))
    })
}

/// Every image in a pack, by its path inside the pack.
fn pack_images(store: &Store, cache: &Cache, pack: &Pack) -> Vec<String> {
    let dir = store.pack_dir(pack);
    store::search(std::slice::from_ref(&dir), "", cache)
        .hits
        .iter()
        .filter_map(|hit| match hit {
            Hit::Loose(path) => path.strip_prefix(&dir).ok().map(slashed),
            Hit::Inside { member, .. } => Some(member.clone()),
        })
        .filter(|name| named(name, &["png", "jpg", "jpeg", "tga"]))
        .collect()
}

/// **The file a mesh meant, when the file it named is not there.**
///
/// A Synty FBX names its texture from the tree it was exported in:
/// `PolygonGeneric_Texture_01_A.psd`, where the pack ships
/// `Generic_01_A.png`, or `PolygonHorrorSpace_Texture_01_A.png`, which is
/// what that pack's atlas was called two versions ago. Neither can be
/// made to resolve — no file can satisfy a reference to a `.psd` — so the
/// nearest thing the pack does carry is handed to that mesh as its atlas.
///
/// Matched on the words the two names share, with the words every Synty
/// texture has thrown away first: `polygon` and `texture` are in nearly
/// all of them and so distinguish nothing. Two words of agreement are the
/// least this will act on, because a wrong texture is how a pack came to
/// be painted with water.
fn find_like(store: &Store, cache: &Cache, pack: &Pack, name: &str) -> Option<PathBuf> {
    let wanted = telling(name);
    if wanted.len() < 2 {
        return None;
    }
    let mut best: Option<(usize, usize, String)> = None;
    for candidate in pack_images(store, cache, pack) {
        if is_other_map(&candidate) {
            continue;
        }
        let spelling = telling(&candidate);
        let shared = wanted.iter().filter(|word| spelling.contains(word)).count();
        if shared < 2 {
            continue;
        }
        let rank = (shared, usize::MAX - spelling.len(), candidate);
        if best.as_ref().is_none_or(|had| *had < rank) {
            best = Some(rank);
        }
    }
    let (_, _, found) = best?;
    store::find_relative(store, cache, pack, &found)
}

/// The words of a name that tell one texture from another. `polygon` and
/// `texture` are in nearly every Synty texture's name, and a file
/// extension says nothing about what is in the file.
fn telling(name: &str) -> Vec<String> {
    words(name)
        .into_iter()
        .filter(|word| {
            !matches!(
                word.as_str(),
                "polygon"
                    | "poly"
                    | "texture"
                    | "textures"
                    | "png"
                    | "tga"
                    | "psd"
                    | "jpg"
                    | "jpeg"
                    | "sourcefiles"
                    | "source"
                    | "files"
            )
        })
        .collect()
}

/// **How well an image answers for a pack's shared atlas**, smallest
/// first.
///
/// Four questions, in the order they were learned. How much of the pack's
/// own name is in it — `PolygonSciFiHorror_01_A` against a pack called
/// `POLYGON - Sci-Fi Horror`. Whether it says `texture`, which is Synty's
/// own word for the thing but is missing from some packs' atlases
/// entirely. **How few other words it carries**, because the main atlas
/// is the least qualified name in the pack —
/// `PolygonSciFiSpace_Signs_Texture_01_A` is the atlas for the signs, and
/// its extra word is what says so. And then the copy nearest the root,
/// and the name, so that two machines guess the same file.
fn atlas_rank(
    wanted: &[String],
    name: &str,
) -> (std::cmp::Reverse<usize>, bool, usize, usize, String) {
    let spelling = words(name);
    (
        std::cmp::Reverse(
            wanted
                .iter()
                .filter(|word| spelling.contains(*word))
                .count(),
        ),
        !spelling.iter().any(|word| word == "texture"),
        spelling.len(),
        name.matches('/').count(),
        name.to_owned(),
    )
}

/// **Which images are not the thing a mesh is painted with.**
///
/// A pack's textures folder holds normal maps, emission masks and a
/// skybox beside the atlas, and painting a mesh with a normal map makes
/// it uniformly lavender — which a describer will then describe.
fn is_other_map(name: &str) -> bool {
    let spelling = words(name);
    [
        // Maps that are not colour. Painting a mesh with a normal map
        // makes it uniformly lavender, and a describer will say so.
        "normal",
        "normals",
        "emissive",
        "emission",
        "metallic",
        "roughness",
        "specular",
        "occlusion",
        "height",
        "displacement",
        "mask",
        // Not a texture for a mesh at all.
        "skybox",
        "lensdirt",
        // The pack's own furniture. A store icon is named exactly after
        // the pack and sits at the top of its directory, so it beat the
        // atlas on every question this asks until it was ruled out.
        "icon",
        "logo",
        "banner",
        "thumbnail",
        "screenshot",
    ]
    .iter()
    .any(|map| spelling.iter().any(|word| word == map))
}

/// A name as the words in it, lowercased: `PolygonSciFiHorror_01_A` is
/// `polygon sci fi horror 01 a`, and so is `POLYGON - Sci-Fi Horror`.
/// Splitting on case as well as punctuation is what lets a directory the
/// store named be compared with a file Synty named.
fn words(name: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut word = String::new();
    let mut previous_lower = false;
    let mut previous_alpha = false;
    for character in name.chars() {
        if !character.is_ascii_alphanumeric() {
            if !word.is_empty() {
                found.push(std::mem::take(&mut word));
            }
            continue;
        }
        // A capital after a lowercase starts a word, so `SciFiHorror` is
        // three words and `PNG` stays one. So does a digit after a
        // letter: `PolygonVikings2` is the Vikings pack, and while that
        // `2` stayed stuck to the name it was a pack nothing matched.
        let turned = (previous_lower && character.is_ascii_uppercase())
            || (previous_alpha && character.is_ascii_digit());
        if turned && !word.is_empty() {
            found.push(std::mem::take(&mut word));
        }
        previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        previous_alpha = character.is_ascii_alphabetic();
        word.push(character.to_ascii_lowercase());
    }
    if !word.is_empty() {
        found.push(word);
    }
    found
}

/// The three programs and the cache one describe run works through,
/// gathered so a worker can be handed one thing.
struct Run<'a> {
    cache: &'a Cache,
    script: &'a Path,
    previewer: &'a Previewer,
    describer: &'a Describer,
}

/// What a whole run came to.
struct Done {
    written: usize,
    troubles: Vec<String>,
    /// The catalogues it changed, so the run can say where its work went.
    files: BTreeSet<PathBuf>,
    /// The digests it described, which is how the family pass tells the
    /// families this run touched from the rest of the catalogue.
    described: BTreeSet<String>,
    /// **The meshes it did not describe because they had asked for a
    /// texture that was not there**, and what each asked for. Set only on
    /// the patient pass; see [`describe`].
    deferred: Vec<(usize, Vec<String>)>,
}

impl Done {
    /// Take in what a second look came to. The deferred list is not
    /// carried over: a mesh waits once, and is described on the next look
    /// with whatever it has by then.
    fn absorb(&mut self, more: Self) {
        self.written += more.written;
        self.troubles.extend(more.troubles);
        self.files.extend(more.files);
        self.described.extend(more.described);
    }
}

/// **Look at the work, a chunk per launch, several launches at once.**
///
/// A chunk is rendered in one launch of the previewer and then described
/// one mesh at a time, and `jobs` workers do that concurrently — so the
/// Blender startup is paid once per chunk rather than once per mesh, and
/// the network round trips of one worker overlap the rendering of
/// another.
///
/// **The catalogue is written as it goes**, at the end of each chunk.
/// Cataloguing a large pack is hours, and a run that only wrote at the
/// end would be a run where an interruption at hour three costs three
/// hours. Writing per chunk means an interrupted run keeps everything it
/// had described, and the next run skips exactly those — which is what
/// makes a big sweep something you can chip away at.
fn look_at_all(
    run: &Run<'_>,
    work: &[Vec<Candidate>],
    chunk: usize,
    jobs: usize,
    books: &Mutex<BTreeMap<String, dex::Dex>>,
    patient: bool,
) -> Done {
    let chunks: Vec<&[Vec<Candidate>]> = work.chunks(chunk).collect();
    let deferred: Mutex<Vec<(usize, Vec<String>)>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    let written = AtomicUsize::new(0);
    let troubles: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let files: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
    let described: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(chunks.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(batch) = chunks.get(at) else {
                        return;
                    };
                    let mut entries: Vec<dex::Entry> = Vec::new();
                    for (which, (group, look)) in batch.iter().zip(look_at(run, batch)).enumerate()
                    {
                        let candidate = &group[0];
                        // **A mesh that asked for a texture it did not
                        // get waits.** Its picture is the pack atlas
                        // stretched over coordinates meant for something
                        // else, and describing that spends a model call
                        // on a wrong answer. The run fetches what it
                        // asked for and looks again — see `describe`.
                        if patient
                            && let Ok(look) = &look
                            && !answered(&look.wants, candidate.atlas.as_deref())
                        {
                            deferred
                                .lock()
                                .expect("no worker panics holding this")
                                .push((at * chunk + which, look.wants.clone()));
                            continue;
                        }
                        let outcome = look.and_then(|look| {
                            let said = run.describer.say(
                                run.cache,
                                &Subject {
                                    name: &candidate.name,
                                    pack: &candidate.title,
                                },
                                &look,
                                &run.cache.dex_file(&candidate.digest, "preview.png"),
                                &candidate.digest,
                            )?;
                            Ok((look, said))
                        });
                        let sofar = finished.fetch_add(1, Ordering::Relaxed) + 1;
                        match outcome {
                            Ok((look, said)) => {
                                println!(
                                    "  {sofar:>4}/{total}  {:<32} {said}",
                                    candidate.name,
                                    total = work.len()
                                );
                                described
                                    .lock()
                                    .expect("no worker panics holding this")
                                    .insert(candidate.digest.clone());
                                // Every name these bytes go by gets the
                                // same answer; see `one_per_digest`.
                                entries.extend(
                                    group.iter().map(|also| {
                                        entry(also, &look, said.clone(), run.describer)
                                    }),
                                );
                            }
                            Err(trouble) => {
                                println!(
                                    "  {sofar:>4}/{total}  {:<32} (see below)",
                                    candidate.name,
                                    total = work.len()
                                );
                                troubles
                                    .lock()
                                    .expect("no worker panics holding this")
                                    .push(trouble);
                            }
                        }
                    }
                    written.fetch_add(entries.len(), Ordering::Relaxed);
                    match file(books, entries) {
                        Ok(paths) => files
                            .lock()
                            .expect("no worker panics holding this")
                            .extend(paths),
                        Err(trouble) => troubles
                            .lock()
                            .expect("no worker panics holding this")
                            .push(trouble),
                    }
                }
            });
        }
    });
    let mut deferred = deferred.into_inner().expect("every worker is finished");
    // In the order the work was given, so a second pass reads like the
    // first and two runs of one pack produce one catalogue.
    deferred.sort_by_key(|(at, _)| *at);
    Done {
        written: written.load(Ordering::Relaxed),
        troubles: troubles.into_inner().expect("every worker is finished"),
        files: files.into_inner().expect("every worker is finished"),
        described: described.into_inner().expect("every worker is finished"),
        deferred,
    }
}

/// **How many of one family are compared in a single call.**
///
/// A family is usually three or four, and the largest in a real library
/// is ninety-two modular pieces — which is not a message anybody can
/// send, and would not be a comparison worth reading if it were. A family
/// past this is compared in groups, so a member is told apart from the
/// seven nearest it in name rather than from all ninety-one.
const FAMILY_MOST: usize = 8;

/// **The second look: what tells the variants of one thing apart.**
///
/// Everything above this describes one mesh at a time, which is the one
/// thing that cannot produce a comparison — five light panels come back
/// as five sentences agreeing about everything that matters. Six tenths
/// of a real library is in a family of two or more, so this is most of
/// the catalogue rather than a corner of it.
///
/// It costs one call per family and no rendering at all: the pictures are
/// the ones the pass above already made, and a message can carry several.
///
/// Families are drawn from the whole catalogue rather than from this
/// run's work, so a family whose members were described across two runs
/// is still compared — but only families this run touched are asked
/// about, or every run would re-buy every comparison in the file.
fn compare_all(
    run: &Run<'_>,
    books: &Mutex<BTreeMap<String, dex::Dex>>,
    described: &BTreeSet<String>,
    force: bool,
    jobs: usize,
    titles: &BTreeMap<String, String>,
) -> (usize, Vec<String>) {
    let families = families(books, described, force);
    if families.is_empty() {
        return (0, Vec::new());
    }
    println!(
        "art: comparing {} against {}",
        many(families.len(), "family", "families"),
        many(
            families.iter().map(Vec::len).sum::<usize>(),
            "sibling",
            "siblings"
        )
    );
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let troubles: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(families.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let Some(family) = families.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        return;
                    };
                    match compare(run, books, family, titles) {
                        Ok(told) => {
                            done.fetch_add(told, Ordering::Relaxed);
                        }
                        Err(trouble) => troubles
                            .lock()
                            .expect("no worker panics holding this")
                            .push(trouble),
                    }
                }
            });
        }
    });
    (
        done.load(Ordering::Relaxed),
        troubles.into_inner().expect("every worker is finished"),
    )
}

/// One family: which pack, and the ids of its members in name order.
type Family = Vec<(String, String)>;

/// **Which families are worth asking about.**
///
/// One this run described a member of, and one that has a member with
/// nothing to say about its siblings yet — because a comparison already
/// written is a comparison already paid for.
fn families(
    books: &Mutex<BTreeMap<String, dex::Dex>>,
    described: &BTreeSet<String>,
    force: bool,
) -> Vec<Family> {
    let books = books.lock().expect("nothing else holds this yet");
    let mut families = Vec::new();
    for (pack, book) in books.iter() {
        let mut grouped: BTreeMap<&str, Family> = BTreeMap::new();
        for entry in book.entries.values() {
            grouped
                .entry(dex::family_of(&entry.name))
                .or_default()
                .push((pack.clone(), entry.id.clone()));
        }
        for (_, members) in grouped {
            if members.len() < 2 {
                continue;
            }
            let touched = members.iter().any(|(_, id)| {
                book.entries
                    .get(id)
                    .is_some_and(|entry| described.contains(&entry.sha256))
            });
            let wanting = members.iter().any(|(_, id)| {
                book.entries
                    .get(id)
                    .is_some_and(|entry| entry.differs.is_none())
            });
            if !touched || !(wanting || force) {
                continue;
            }
            // A family past the cap is compared in groups of its own,
            // which is what a hundred modular wall pieces have to be.
            for group in members.chunks(FAMILY_MOST) {
                if group.len() >= 2 {
                    families.push(group.to_vec());
                }
            }
        }
    }
    drop(books);
    families
}

/// Ask about one family, and write what comes back into the catalogue.
fn compare(
    run: &Run<'_>,
    books: &Mutex<BTreeMap<String, dex::Dex>>,
    family: &Family,
    titles: &BTreeMap<String, String>,
) -> Result<usize, String> {
    // Everything the comparison needs, copied out from under the lock:
    // the call is a network round trip and nothing else should wait on it.
    struct Member {
        id: String,
        name: String,
        triangles: u64,
        size: [f32; 3],
        description: String,
        picture: PathBuf,
    }
    let pack = family
        .first()
        .map(|(pack, _)| pack.clone())
        .unwrap_or_default();
    let members: Vec<Member> = {
        let books = books.lock().expect("no worker panics holding this");
        let book = books.get(&pack).ok_or("a family out of no pack")?;
        let members = family
            .iter()
            .filter_map(|(_, id)| book.entries.get(id))
            .map(|entry| Member {
                id: entry.id.clone(),
                name: entry.name.clone(),
                triangles: entry.triangles,
                size: entry.size,
                description: entry.description.clone(),
                picture: run.cache.dex_file(&entry.sha256, "preview.png"),
            })
            // A member whose picture the cache no longer holds cannot be
            // in a comparison, and its siblings still can.
            .filter(|member| member.picture.is_file())
            .collect();
        drop(books);
        members
    };
    if members.len() < 2 {
        return Ok(0);
    }
    let siblings: Vec<Sibling<'_>> = members
        .iter()
        .map(|member| Sibling {
            name: &member.name,
            triangles: member.triangles,
            size: member.size,
            description: &member.description,
            picture: &member.picture,
        })
        .collect();
    // Where this comparison's own evidence goes: the pack and the first
    // member's id, which is unique across the catalogue where a digest
    // would not be — a family of identical meshes is one digest.
    let digest = members
        .first()
        .map_or_else(String::new, |member| format!("{pack}-{}", member.id));
    // What the pack is called rather than what it is filed under. The
    // catalogue records the id; the title is worth having in the prompt
    // and comes from this run's own candidates, so a family whose members
    // were all described by an earlier run is named by its id — which is
    // the honest fallback rather than a wrong name.
    let title = titles.get(&pack).map_or(pack.as_str(), String::as_str);
    let told = run
        .describer
        .compare(run.cache, title, &siblings, &digest)?;

    let mut written = 0;
    {
        let mut books = books.lock().expect("no worker panics holding this");
        let book = books.get_mut(&pack).ok_or("a family out of no pack")?;
        for (member, line) in members.iter().zip(told) {
            let Some(line) = line else { continue };
            if let Some(entry) = book.entries.get_mut(&member.id) {
                entry.differs = Some(line);
                written += 1;
            }
        }
        if written > 0 {
            book.write()?;
        }
        drop(books);
    }
    Ok(written)
}

/// One chunk's pictures and numbers, in one launch. One job per group of
/// identical meshes, not one per name.
fn look_at(run: &Run<'_>, chunk: &[Vec<Candidate>]) -> Vec<Result<preview::Look, String>> {
    let pictures: Vec<PathBuf> = chunk
        .iter()
        .map(|group| run.cache.dex_file(&group[0].digest, "preview.png"))
        .collect();
    let jobs: Vec<preview::Job<'_>> = chunk
        .iter()
        .zip(&pictures)
        .map(|(group, picture)| preview::Job {
            source: &group[0].path,
            destination: picture,
            texture: group[0].atlas.as_deref(),
            digest: &group[0].digest,
        })
        .collect();
    run.previewer.run(run.script, run.cache, &jobs)
}

/// File a chunk's entries in their packs' catalogues, and write those
/// catalogues out. Under one lock, because two workers finishing chunks
/// out of the same pack at the same moment must not both write it.
fn file(
    books: &Mutex<BTreeMap<String, dex::Dex>>,
    entries: Vec<dex::Entry>,
) -> Result<Vec<PathBuf>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let mut written = Vec::new();
    // The lock is held across the write on purpose and let go the moment
    // it is done: two workers finishing chunks out of the same pack at
    // the same moment must not both be rendering that file.
    {
        let mut books = books.lock().expect("no worker panics holding this");
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for entry in entries {
            touched.insert(entry.pack.clone());
            books
                .get_mut(&entry.pack)
                .expect("a candidate's own pack")
                .insert(entry);
        }
        for pack in touched {
            books[&pack].write()?;
            written.push(books[&pack].path.clone());
        }
    }
    Ok(written)
}

/// One catalogue entry, out of what was measured and what was said.
fn entry(
    candidate: &Candidate,
    look: &preview::Look,
    description: String,
    describer: &Describer,
) -> dex::Entry {
    dex::Entry {
        id: String::new(), // filled in by the book, which knows what is taken
        name: candidate.name.clone(),
        pack: candidate.pack.clone(),
        source: candidate.source.clone(),
        sha256: candidate.digest.clone(),
        asset: candidate.asset.clone(),
        // Filled in by the family pass, which is the only thing that can
        // see a mesh's siblings. `Dex::insert` carries an existing one
        // over when the bytes have not changed.
        differs: None,
        // A borrowed atlas names its lender. `Texture_01.png` says
        // nothing about where a mesh's colours came from;
        // `POLYGON - Vikings Pack/Texture_01.png` on a horror prop says
        // both that the mesh asked for another pack's texture and that
        // this is the pack it was given.
        atlas: candidate.atlas.as_ref().map(|path| {
            let leaf = path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            candidate
                .borrowed
                .as_ref()
                .map_or_else(|| leaf.clone(), |from| format!("{from}/{leaf}"))
        }),
        textures: look.images.join(", "),
        // What it still could not load after being given what the pack
        // had. A description of a picture painted with the wrong texture
        // is worth reading, and worth reading with suspicion.
        wanted: candidate.unresolved.clone(),
        description,
        described_by: describer.describe(),
        triangles: look.triangles,
        meshes: look.meshes,
        materials: look.materials,
        size: look.size(),
    }
}

/// **Read the catalogue back.**
///
/// The command the whole of the rest of this exists for: a search over
/// what things look like rather than over what they are called. "hazard
/// stripe" is a search somebody has; `SM_Prop_Crate_04` is not.
fn catalogue(words: &[&str]) -> Result<(), String> {
    let dir = dex_dir();
    let needle = words.join(" ");
    let (books, trouble) = dex::open_all(&dir);
    for complaint in &trouble {
        eprintln!("art: {complaint}");
    }
    if books.is_empty() {
        return Err(format!(
            "there is no catalogue in {} yet.\n\n  \
             `cargo xtask art describe` writes one for the assets art/manifest.toml names,\n  \
             and `cargo xtask art describe <text>` writes one for whatever is in the packs.",
            dir.display()
        ));
    }
    let mut shown = 0;
    let mut held = 0;
    for book in &books {
        held += book.entries.len();
        let matching: Vec<&dex::Entry> = book
            .entries
            .values()
            .filter(|entry| needle.is_empty() || entry.matches(&needle))
            .collect();
        if matching.is_empty() {
            continue;
        }
        println!(
            "\n{} — {} of {}",
            book.path.display(),
            matching.len(),
            many(book.entries.len(), "mesh", "meshes")
        );
        for entry in matching {
            println!("  {}", entry.line());
            if let Some(wanted) = &entry.wanted {
                // Before the description rather than after it, because
                // it is a caveat on the sentence below and a reader
                // should meet it first.
                println!(
                    "  {:<32} asked for {wanted}, which the pack has not got",
                    "!"
                );
            }
            if let Some(differs) = &entry.differs {
                // Under the description rather than in it: one is what
                // the thing is, the other is why you would take this one
                // rather than the four beside it.
                println!("  {:<32} vs its siblings: {differs}", "");
            }
            shown += 1;
        }
    }
    if shown == 0 {
        println!(
            "art: nothing among the {} described says `{needle}`",
            many(held, "mesh", "meshes")
        );
        return Ok(());
    }
    println!(
        "\nart: {shown} of the {} described. A `*` is a mesh art/manifest.toml already names.",
        many(held, "mesh", "meshes")
    );
    Ok(())
}

/// Which manifest key names a file inside this archive.
fn key_of(archive: &Path) -> &'static str {
    if is_unitypackage(archive) {
        "unity"
    } else {
        "source"
    }
}

fn is_unitypackage(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("unitypackage"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(id: &str) -> Pack {
        Pack {
            id: id.to_owned(),
            title: id.to_owned(),
            dir: id.to_owned(),
            download: id.to_owned(),
            line: 0,
        }
    }

    /// **A pack that ships one mesh in two formats is catalogued once.**
    ///
    /// Measured rather than predicted: the first sweep over a real pack
    /// described four barrels, and two of them were the other two — Synty
    /// ship `SourceFiles/FBX/X.fbx` and `SourceFiles/OBJ/X.obj`, so half
    /// of every render and half of every model call went on writing the
    /// same sentence twice.
    ///
    /// And the thing this must not do is fold two different meshes
    /// together. A pack with `Props/SM_Crate.fbx` and
    /// `Buildings/SM_Crate.fbx` has two crates, both of them FBX, and
    /// both are still in the answer.
    #[test]
    fn one_mesh_shipped_in_two_formats_is_described_once() {
        let sources = [
            ("scifi", "SourceFiles/FBX/SM_Prop_Barrel_01.fbx"),
            ("scifi", "SourceFiles/OBJ/SM_Prop_Barrel_01.obj"),
            ("scifi", "SourceFiles/FBX/SM_Prop_Barrel_02.fbx"),
            ("scifi", "SourceFiles/OBJ/SM_Prop_Barrel_02.obj"),
            ("scifi", "Props/SM_Crate.fbx"),
            ("scifi", "Buildings/SM_Crate.fbx"),
            // Another pack's OBJ, with no FBX beside it: the only copy
            // there is, so it stays.
            ("nature", "SourceFiles/OBJ/SM_Tree.obj"),
        ];
        let found: BTreeMap<(String, String), Pack> = sources
            .iter()
            .map(|(id, source)| (((*id).to_owned(), (*source).to_owned()), pack(id)))
            .collect();
        let kept: Vec<String> = one_format_per_mesh(found)
            .into_keys()
            .map(|(_, source)| source)
            .collect();
        // In the order the answer is built in — by pack, then by path —
        // because the catalogue a run writes has to be the same catalogue
        // twice running.
        assert_eq!(
            kept,
            [
                "SourceFiles/OBJ/SM_Tree.obj",
                "Buildings/SM_Crate.fbx",
                "Props/SM_Crate.fbx",
                "SourceFiles/FBX/SM_Prop_Barrel_01.fbx",
                "SourceFiles/FBX/SM_Prop_Barrel_02.fbx",
            ]
        );
    }

    /// **A pack's atlas is the one named after the pack.**
    ///
    /// This was "the shortest name with `texture` in it", and it painted
    /// a thousand meshes of `POLYGON - Sci-Fi Horror` with
    /// `Generic_Water_Texture.png` — the shortest such name in the pack.
    /// The catalogue then described a sheet of ivy as "a jagged, faceted
    /// shard of near-black material", which was an accurate description
    /// of the picture and a useless one of the asset. The pack's real
    /// atlas is `PolygonSciFiHorror_01_A.png`, which does not have the
    /// word `texture` in it at all.
    ///
    /// So the rule is the pack's own name, then Synty's own word for the
    /// thing, then **how few other words the name carries** — the main
    /// atlas is the least qualified name in the pack, and
    /// `PolygonSciFiSpace_Signs_Texture_01_A` is the atlas for the signs.
    /// Two more things had to be ruled out first, and both were found by
    /// this picking them: a normal map, which paints a mesh lavender, and
    /// the pack's own store icon, which is named exactly after the pack
    /// and sits at the top of its directory.
    #[test]
    fn a_packs_atlas_is_the_one_named_after_the_pack() {
        let best = |dir: &str, mut names: Vec<&str>| -> String {
            let wanted = words(dir);
            names.retain(|name| !is_other_map(name));
            names.sort_by_key(|name| atlas_rank(&wanted, name));
            (*names.first().expect("a candidate")).to_owned()
        };

        // The pack that was painted with water. Every one of these beat
        // the atlas under some rule: the water texture when it was the
        // shortest `texture` name, the icon on the pack's own name and on
        // sitting at the top of the directory, the normal map on both.
        assert_eq!(
            best(
                "POLYGON - Sci-Fi Horror",
                vec![
                    "POLYGON_SciFi_Horror_ICON.png",
                    "SourceFiles/Generic/Textures/Generic_Water_Texture.png",
                    "SourceFiles/SciFiHorror/Textures/Normals/PolygonSciFiHorror_Texture_A_01_Normal.png",
                    "SourceFiles/SciFiHorror/Textures/Alts/PolygonSciFiHorror_01_A.png",
                    "SourceFiles/SciFiHorror/Textures/Alts/PolygonSciFiHorror_02_A.png",
                ],
            ),
            "SourceFiles/SciFiHorror/Textures/Alts/PolygonSciFiHorror_01_A.png"
        );

        // And the pack the old rule happened to get right stays right.
        // The signs atlas is the trap: it matches the pack's name just as
        // well and says `texture` too, and the one word between them is
        // the whole of what makes one the pack's atlas and the other the
        // atlas for its signs.
        assert_eq!(
            best(
                "POLYGON - Sci-Fi Space Pack",
                vec![
                    "SourceFiles/Textures/PolygonSciFiSpace_Signs_Texture_01_A.png",
                    "SourceFiles/Textures/Alts/PolygonSciFiSpace_Texture_03_F.png",
                    "SourceFiles/Textures/PolygonSciFiSpace_Texture_01_A.png",
                    "SourceFiles/Textures/FX_Textures/PolygonSciFiSpace_Skybox_01_Up.png",
                ],
            ),
            "SourceFiles/Textures/PolygonSciFiSpace_Texture_01_A.png"
        );

        // A pack whose atlas is not spelled like its directory at all
        // still beats a generic one, on the one word they share.
        assert_eq!(
            best(
                "POLYGON - Adventure Pack",
                vec![
                    "SourceFiles/Textures/Generic_Water_Texture.png",
                    "SourceFiles/PolyAdventureTexture_01.png",
                ],
            ),
            "SourceFiles/PolyAdventureTexture_01.png"
        );
    }

    /// **A name is the words in it**, however the thing that wrote it
    /// spelled them — which is what lets a directory a store named be
    /// compared with a file Synty named.
    #[test]
    fn a_name_is_the_words_in_it() {
        assert_eq!(
            words("PolygonSciFiHorror_01_A"),
            ["polygon", "sci", "fi", "horror", "01", "a"]
        );
        assert_eq!(
            words("POLYGON - Sci-Fi Horror"),
            ["polygon", "sci", "fi", "horror"]
        );
        // A digit stuck to the end of a name is its own word, or the
        // pack `PolygonVikings2_Texture_01.psd` names is a pack called
        // `vikings2`, which nobody has.
        assert_eq!(
            words("PolygonVikings2_Texture_01.psd"),
            ["polygon", "vikings", "2", "texture", "01", "psd"]
        );
        assert_eq!(
            words("Generic_Water_Texture.png"),
            ["generic", "water", "texture", "png"]
        );
    }

    /// **A pack found under its id resolves back to its directory.**
    ///
    /// The cache files what it took out of a pack under that pack's id,
    /// so the second run of a sweep meets `polygon_apocalypse` where the
    /// first met `POLYGON - Apocalypse`. A pack whose `dir` is a slug is
    /// a pack whose directory does not exist, and the visible cost of
    /// that is the pack's shared atlas going unfound: a whole pack
    /// rendered untextured and catalogued as the colour of Blender's
    /// missing-texture magenta, on the second sweep and never the first.
    #[test]
    fn a_pack_found_under_its_id_is_still_the_directory_it_came_from() {
        let root = std::env::temp_dir().join(format!("space-trucking-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        fsx::create_dir_all(&root.join("POLYGON - Apocalypse")).expect("a scratch store");
        let store = Store::at(root.clone());
        let manifest = Manifest::parse(
            Path::new("manifest.toml"),
            "[pack.demo]\ntitle = \"A Pack\"\ndir = \"a-pack\"\n",
        )
        .expect("a manifest");

        let by_slug = by_directory(&manifest, &store, "polygon_apocalypse");
        assert_eq!(by_slug.dir, "POLYGON - Apocalypse", "a slug is not a path");
        assert_eq!(by_slug.title, "POLYGON - Apocalypse");
        assert_eq!(by_slug.id, "polygon_apocalypse");

        // The directory's own name still answers for itself, and a pack
        // the manifest declares still beats both.
        assert_eq!(
            by_directory(&manifest, &store, "POLYGON - Apocalypse").dir,
            "POLYGON - Apocalypse"
        );
        assert_eq!(by_directory(&manifest, &store, "demo").dir, "a-pack");
        // And a name nothing in the store answers to stays itself rather
        // than becoming some other pack.
        assert_eq!(
            by_directory(&manifest, &store, "nothing_here").dir,
            "nothing_here"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **What counts as a mesh is what the importer can open**, and what
    /// a mesh is called is the file's own name without the extension —
    /// which is the half of every description the picture cannot supply.
    #[test]
    fn a_mesh_is_a_file_something_here_can_open_and_is_called_what_the_pack_calls_it() {
        for name in ["a/b/SM_Crate.fbx", "SM_Crate.FBX", "x.obj", "y.blend"] {
            assert!(is_mesh(name), "`{name}` is a mesh");
        }
        for name in [
            "SM_Crate.fbx.meta",
            "atlas.png",
            "readme.txt",
            "SM_Crate.mat",
            "SM_Crate",
        ] {
            assert!(!is_mesh(name), "`{name}` is not a mesh");
        }
        assert_eq!(
            stem("SourceFiles/FBX/SM_Prop_Crate_01.fbx"),
            "SM_Prop_Crate_01"
        );
        assert_eq!(stem("SM_Crate.fbx"), "SM_Crate");
    }
}
