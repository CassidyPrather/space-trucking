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
//! $SYNTY_STORE        the packs as downloaded, on the owner's disk
//! art/cache/          rebuilt, converted, content-addressed, gitignored
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

mod cache;
mod convert;
mod fsx;
mod manifest;
mod sha256;
mod store;
mod unitypackage;

use std::path::{Path, PathBuf};

use cache::Cache;
use manifest::{Asset, Manifest, Resolved};
use store::{Found, Store};

const USAGE: &str = "\
cargo xtask art <command>

  check            find every asset the manifest names, hash it, and report; converts nothing
  resolve          check, then convert anything not already in the cache, then write the index
  hash [id ...]    print the `sha256` lines to paste into the manifest
  unpack <pack>    rebuild the asset trees inside one pack's .unitypackage files
  find <text>      search the packs for a file name, and print the manifest line for each hit

Environment:
  SYNTY_STORE      where the packs are, unzipped, one directory per pack (required)
  ART_MANIFEST     a manifest other than art/manifest.toml
  ART_CACHE        a cache other than art/cache
  BLENDER          the Blender executable, if it is not on PATH
  ART_CONVERTER    a program run as `<program> <source> <destination.glb>`, instead of Blender
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
    /// Set when the manifest carried no digest for it yet.
    unrecorded: bool,
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
    convert_all(&manifest, &store, &cache, &sourced)
}

/// Find one asset's bytes and check them against the line that named it.
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
    if asset.sha256 == Asset::NO_DIGEST_YET {
        return Ok(Sourced {
            digest,
            found,
            unrecorded: true,
        });
    }
    if asset.sha256 != digest {
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
    Ok(Sourced {
        digest,
        found,
        unrecorded: false,
    })
}

fn convert_all(
    manifest: &Manifest,
    store: &Store,
    cache: &Cache,
    sourced: &[(&Asset, Sourced)],
) -> Result<(), String> {
    let wanted: Vec<&(&Asset, Sourced)> = sourced
        .iter()
        .filter(|(_, one)| !cache.glb(&one.digest).is_file())
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
        if let Some(texture) = &asset.texture {
            let pack = manifest.pack_of(asset);
            let from = store::find_relative(store, cache, pack, texture).ok_or_else(|| {
                format!(
                    "{} names texture `{texture}`, and it is not in {}",
                    asset.id,
                    store.pack_dir(pack).display()
                )
            })?;
            let leaf = from.file_name().expect("a texture file has a name");
            // Two copies, because an FBX names its texture by a relative
            // path and the two spellings Synty's exporters use are the
            // file beside the mesh and the file in a Textures folder.
            fsx::copy(&from, &stage.join(leaf))?;
            fsx::copy(&from, &stage.join("Textures").join(leaf))?;
        }
        converter.run(cache, &staged, &cache.glb(&one.digest))?;
        println!(
            "  converted {:<24} -> {}",
            asset.id,
            Cache::glb_relative(&one.digest)
        );
    }

    let resolved: Vec<Resolved> = sourced
        .iter()
        .map(|(asset, one)| Resolved {
            id: asset.id.clone(),
            glb: Cache::glb_relative(&one.digest),
            sha256: one.digest.clone(),
            scale: asset.scale,
            offset: asset.offset,
            rotation: asset.rotation,
            fill: asset.fill,
        })
        .collect();
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
                .file_stem()
                .and_then(|stem| stem.to_str())
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
/// and reading them off a store page is the part that does not scale.
fn find(needle: &str) -> Result<(), String> {
    let manifest = read_manifest()?;
    let store = Store::open()?;
    let cache = Cache::open(&repo());
    let unpacked = cache.root.join("unpacked");
    let roots = vec![store.root.clone(), unpacked.clone()];
    let hits = store::search(&roots, needle);
    if hits.is_empty() {
        println!(
            "art: nothing under {} or {} has `{needle}` in its name",
            store.root.display(),
            unpacked.display()
        );
        return Ok(());
    }
    let shown = hits.len().min(120);
    for hit in hits.iter().take(shown) {
        println!("{}", manifest_lines_for(&manifest, &store, &unpacked, hit));
    }
    if hits.len() > shown {
        println!(
            "art: {} more not shown; narrow the search",
            hits.len() - shown
        );
    }
    Ok(())
}

/// "1 asset", "2 assets". A report that says "1 assets" reads like a
/// machine talking, and every other line this tool prints is a sentence.
fn count(many: usize, thing: &str) -> String {
    if many == 1 {
        format!("1 {thing}")
    } else {
        format!("{many} {thing}s")
    }
}

/// One hit, written as the manifest lines that would name it. A hit
/// inside a rebuilt tree is spelled `unity`, not `source`: the file is in
/// the cache, and the cache is not what a manifest points at.
fn manifest_lines_for(manifest: &Manifest, store: &Store, unpacked: &Path, hit: &Path) -> String {
    if let Some((pack, rest)) = manifest.packs.values().find_map(|pack| {
        hit.strip_prefix(store.pack_dir(pack))
            .ok()
            .map(|rest| (pack, rest))
    }) {
        return format!(
            "  pack = \"{}\"\n  source = \"{}\"\n",
            pack.id,
            slashed(rest)
        );
    }
    if let Ok(rest) = hit.strip_prefix(unpacked) {
        let mut parts = rest.components();
        // <pack>/<archive>/Assets/... — the first two are the cache's own
        // filing and the rest is the path Unity stored.
        if let (Some(pack), Some(_archive)) = (parts.next(), parts.next()) {
            let inside: PathBuf = parts.collect();
            return format!(
                "  pack = \"{}\"\n  unity = \"{}\"\n",
                pack.as_os_str().to_string_lossy(),
                slashed(&inside)
            );
        }
    }
    format!("  {}\n", hit.display())
}

/// A relative path as the manifest spells it: `/` on every platform.
fn slashed(path: &Path) -> String {
    path.components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}
