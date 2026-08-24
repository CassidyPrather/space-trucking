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
//! ```
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
mod fsx;
mod manifest;
mod sha256;
mod store;
mod unitypackage;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cache::{Cache, Converted};
use convert::Converter;
use manifest::{Asset, Bounds, Manifest, Resolved};
use store::{Found, Hit, Store, count, slashed};

const USAGE: &str = "\
cargo xtask art <command>

  check            find every asset the manifest names, hash it, and report; converts nothing
  resolve          check, then convert anything not already in the cache, then write the index
  hash [id ...]    print the `sha256` lines to paste into the manifest
  unpack <pack>    rebuild the asset trees inside one pack's .unitypackage files
  find <text>      search the packs — inside their archives too — and print the manifest lines

Environment:
  SYNTY_STORE      where the packs are, one directory per pack, as downloaded (required)
  ART_MANIFEST     a manifest other than art/manifest.toml
  ART_CACHE        a cache other than art/cache
  BLENDER          the Blender executable, if it is not on PATH
  ART_CONVERTER    a program run as `<program> <source> <destination.glb> [texture]`,
                   instead of Blender
";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let outcome = match words.as_slice() {
        ["art", "check"] => report(false),
        ["art", "resolve"] => report(true),
        ["art", "hash", ids @ ..] => hashes(ids),
        ["art", "unpack", pack] => unpack(pack),
        ["art", "find", needle @ ..] if !needle.is_empty() => find(&needle.join(" ")),
        _ => {
            eprint!("{USAGE}");
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
