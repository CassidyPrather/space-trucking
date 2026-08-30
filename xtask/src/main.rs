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
use describe::{Describer, Subject};
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
  --limit <n>      how many found meshes to describe (default {limit}); the manifest's own
                   assets are never capped
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
  ART_PREVIEW      a program run as `<program> <source> <destination.png> [texture]`,
                   instead of Blender, for the pictures `describe` renders
  OPENROUTER_API_KEY  the key `describe` reaches a hosted vision model with
  ART_DESCRIBER_MODEL the same thing `--model` says
  ART_DESCRIBER    a program run as `<program> <prompt.txt> <picture.png>` printing a
                   description, instead of a hosted model
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

/// How many meshes are looked at at once. Each one is a Blender launch
/// and then a network round trip, so nearly all of the wall clock is
/// waiting, and four keeps a laptop usable while a pack is catalogued.
const DESCRIBE_JOBS: usize = 4;

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
    /// assets, which is the run somebody makes after adding a line.
    needle: Option<String>,
    limit: usize,
    jobs: usize,
    model: Option<String>,
    offline: bool,
    force: bool,
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
            limit: DESCRIBE_LIMIT,
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
                "--limit" | "--jobs" | "--model" => {
                    let value = *words.get(at).ok_or_else(|| {
                        format!("`{word}` wants a value after it, and the arguments end there")
                    })?;
                    at += 1;
                    match word {
                        "--limit" => wanted.limit = whole(word, value)?.max(1),
                        "--jobs" => wanted.jobs = whole(word, value)?.clamp(1, 16),
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
struct Candidate {
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
}

/// **Render, measure and describe, and write the catalogue.**
fn describe(words: &[&str]) -> Result<(), String> {
    let wanted = Wanted::parse(words)?;
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let dir = dex_dir();

    let candidates = match &wanted.needle {
        Some(needle) => found_candidates(&store, &cache, &manifest, needle, wanted.limit),
        None => declared_candidates(&store, &cache, &manifest),
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
    // `--offline` and "there is nothing to ask" produce the same
    // catalogue and are not the same sentence: one is a choice and the
    // other is a machine that has not been set up, and a person who
    // typed the flag does not want to be told their key is missing.
    let (describer, note) = if wanted.offline {
        (
            Describer::Measurements,
            String::from(
                "art: --offline, so every entry carries its measurements and says so. The\n     \
                 pictures are still rendered and the counts are still true.",
            ),
        )
    } else {
        let describer = describe::find(wanted.model.clone());
        let note = describer.announce();
        (describer, note)
    };
    println!(
        "art: looking at {} with {}",
        many(work.len(), "mesh", "meshes"),
        previewer.describe()
    );
    println!("{note}");
    let script = previewer.prepare(&cache)?;
    let outcomes = look_at_all(&cache, &script, &previewer, &describer, &work, wanted.jobs);

    let mut written = 0;
    let mut troubles = Vec::new();
    // Only the catalogues this run actually changed are rewritten. A run
    // where every description failed should leave the files it loaded
    // exactly as it found them, rather than reporting that it wrote them.
    let mut touched: BTreeSet<String> = BTreeSet::new();
    for (candidate, outcome) in work.iter().zip(outcomes) {
        match outcome {
            Err(trouble) => troubles.push(trouble),
            Ok((look, description)) => {
                let book = books.get_mut(&candidate.pack).expect("its own pack");
                book.insert(entry(candidate, &look, description, &describer));
                touched.insert(candidate.pack.clone());
                written += 1;
            }
        }
    }
    for (pack, book) in &books {
        if !touched.contains(pack) {
            continue;
        }
        book.write()?;
        println!("art: wrote {}", book.path.display());
    }
    for trouble in &troubles {
        println!("\n{trouble}");
    }
    if written == 0 {
        return Err(format!(
            "{} could not be described, and nothing was written",
            many(troubles.len(), "mesh", "meshes")
        ));
    }
    if !troubles.is_empty() {
        println!(
            "\nart: {} described, {} not",
            many(written, "mesh", "meshes"),
            troubles.len()
        );
    }
    Ok(())
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
    let unpacked = cache.root.join("unpacked");
    let search = store::search(&[store.root.clone(), unpacked.clone()], needle, cache);
    for complaint in &search.trouble {
        eprintln!("art: {complaint}");
    }
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
    let mut atlases: BTreeMap<String, Option<PathBuf>> = BTreeMap::new();
    let mut candidates = Vec::new();
    let mut unreachable = 0;
    for ((pack_id, source), pack) in wanted {
        if candidates.len() >= limit {
            break;
        }
        let Some(path) = store::find_relative(store, cache, &pack, &source) else {
            unreachable += 1;
            continue;
        };
        let Ok(digest) = sha256::of_file(&path) else {
            unreachable += 1;
            continue;
        };
        let atlas = atlases
            .entry(pack_id.clone())
            .or_insert_with(|| pack_atlas(store, cache, &pack))
            .clone();
        candidates.push(Candidate {
            asset: named_by(manifest, &pack_id, &source),
            pack: pack_id,
            title: pack.title.clone(),
            name: stem(&source),
            source,
            path,
            digest,
            atlas,
        });
    }
    if total > candidates.len() + unreachable {
        println!(
            "art: {total} meshes match `{needle}`, and this run takes the first {}. \
             `--limit` raises it.",
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
                by_directory(manifest, &first.as_os_str().to_string_lossy()),
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
    Some(by_directory(manifest, &first.as_os_str().to_string_lossy()))
}

/// A directory name as a pack. The cache files a rebuilt tree under the
/// pack's own id, so a name out of there can be a pack the manifest
/// declares; anything else becomes a pack that exists for the length of
/// this run and whose `dir` is the name it was found under.
fn by_directory(manifest: &Manifest, name: &str) -> Pack {
    manifest.packs.get(name).map_or_else(
        || Pack {
            id: dex::id_of(name),
            title: name.to_owned(),
            dir: name.to_owned(),
            download: name.to_owned(),
            line: 0,
        },
        copy_of,
    )
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
    let dir = store.pack_dir(pack);
    let search = store::search(std::slice::from_ref(&dir), "texture", cache);
    let mut names: Vec<String> = search
        .hits
        .iter()
        .filter_map(|hit| match hit {
            Hit::Loose(path) => path.strip_prefix(&dir).ok().map(slashed),
            Hit::Inside { member, .. } => Some(member.clone()),
        })
        .filter(|name| named(name, &["png", "jpg", "jpeg", "tga"]))
        .collect();
    // Shortest first, then alphabetically: the pack's own atlas sits at
    // the top of its Textures folder and the long names are the variants.
    // Ties broken by name, so two machines guess the same file.
    names.sort_by_key(|name| (name.len(), name.clone()));
    names
        .iter()
        .find_map(|name| store::find_relative(store, cache, pack, name))
}

/// **Look at several meshes at once, and keep the answers in order.**
///
/// Each one is a Blender launch and then a network round trip, so a
/// serial run of two dozen is minutes of a laptop doing nothing. The
/// answers are collected against the index they came from rather than in
/// the order they finish, because a catalogue that came out in a
/// different order on every run would be a diff nobody can read.
fn look_at_all(
    cache: &Cache,
    script: &Path,
    previewer: &Previewer,
    describer: &Describer,
    work: &[Candidate],
    jobs: usize,
) -> Vec<Looked> {
    let next = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Looked)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(work.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(candidate) = work.get(index) else {
                        return;
                    };
                    let outcome = look_and_say(cache, script, previewer, describer, candidate);
                    let sofar = finished.fetch_add(1, Ordering::Relaxed) + 1;
                    println!(
                        "  {sofar:>3}/{total}  {:<32} {}",
                        candidate.name,
                        match &outcome {
                            Ok((_, said)) => said.clone(),
                            Err(_) => String::from("(see below)"),
                        },
                        total = work.len()
                    );
                    done.lock()
                        .expect("no worker panics while holding this")
                        .push((index, outcome));
                }
            });
        }
    });
    let mut answers = done.into_inner().expect("every worker is finished");
    answers.sort_by_key(|(index, _)| *index);
    answers.into_iter().map(|(_, outcome)| outcome).collect()
}

/// What looking at one mesh comes to: what was measured and what was
/// said about it, or the one sentence saying why neither happened.
type Looked = Result<(preview::Look, String), String>;

/// One mesh: a picture and its numbers, then a sentence about it.
fn look_and_say(
    cache: &Cache,
    script: &Path,
    previewer: &Previewer,
    describer: &Describer,
    candidate: &Candidate,
) -> Looked {
    let picture = cache.dex_file(&candidate.digest, "preview.png");
    let look = previewer.run(
        script,
        &candidate.path,
        &picture,
        candidate.atlas.as_deref(),
    )?;
    let subject = Subject {
        name: &candidate.name,
        pack: &candidate.title,
    };
    let said = describer.say(cache, &subject, &look, &picture, &candidate.digest)?;
    Ok((look, said))
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
        atlas: candidate.atlas.as_ref().map(|path| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        }),
        textures: look.images.join(", "),
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
