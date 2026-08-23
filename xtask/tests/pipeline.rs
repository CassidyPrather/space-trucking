//! The resolver, driven the way a person drives it.
//!
//! Every guard here runs the real binary against a store built for the
//! occasion. There are no Synty assets in this repository and there never
//! will be, so the packs below are synthetic: a Source Files tree of
//! files this repository wrote, and a `.unitypackage` built here out of
//! GUID directories, `pathname` files and `asset` files, which is what
//! one is.
//!
//! What that proves and what it does not is worth being exact about.
//! Proved here: the manifest dialect, the missing-asset message, the
//! Source-Files-before-archive order, the reconstruction of a tree out of
//! a `.unitypackage`, content addressing, the digest check, and that a
//! partial resolve indexes nothing. Not proved here, and not provable
//! without the owner's disk and a Blender install: that a real Synty FBX
//! converts to a glTF that looks right. `docs/ART_PIPELINE.md` says which
//! commands close that gap.

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("space-trucking-art-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    std::fs::write(path, text).expect("a file");
}

/// Run the resolver, and hand back everything it said on either stream —
/// the guards are about the words, and which stream they came out of is
/// not the law.
fn xtask(arguments: &[&str], environment: &[(&str, &Path)]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(arguments);
    command.env_remove("SYNTY_STORE");
    command.env_remove("ART_MANIFEST");
    command.env_remove("ART_CACHE");
    command.env_remove("ART_CONVERTER");
    command.env_remove("BLENDER");
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().expect("the resolver runs");
    let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), said)
}

/// A `.unitypackage`: a gzipped tar of one directory per asset, named
/// after the asset's Unity GUID, each holding the file's bytes as
/// `asset`, its import settings as `asset.meta`, and its project path as
/// `pathname`. Built here rather than downloaded, because the format is
/// the claim under test and a purchased package cannot be committed.
fn unitypackage(at: &Path, entries: &[(&str, &str, Option<&str>)]) {
    let build = at.with_extension("build");
    let _ = std::fs::remove_dir_all(&build);
    for (guid, pathname, contents) in entries {
        let dir = build.join(guid);
        write(&dir.join("pathname"), &format!("{pathname}\n"));
        write(&dir.join("asset.meta"), &format!("guid: {guid}\n"));
        if let Some(contents) = contents {
            write(&dir.join("asset"), contents);
        }
    }
    let status = Command::new("tar")
        .arg("-czf")
        .arg(at)
        .arg("-C")
        .arg(&build)
        .arg(".")
        .status()
        .expect("tar builds the fixture");
    assert!(status.success(), "tar could not build {}", at.display());
    let _ = std::fs::remove_dir_all(&build);
}

const PACK: &str = "\
[pack.demo]
title = \"POLYGON Demonstration\"
dir = \"demo\"
download = \"POLYGON Demonstration, the Source Files download\"
";

/// **Every asset that is not on this machine is reported in one run.**
/// The tool exists because trawling a pack by hand does not scale, and
/// being told about one missing asset, fixing it, and being told about
/// the next is that same trawl with extra steps.
#[test]
fn every_asset_that_is_missing_is_reported_in_one_run() {
    let dir = scratch("all-at-once");
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n\
             [asset.crate_small]\npack = \"demo\"\nsource = \"SourceFiles/FBX/SM_Crate.fbx\"\n\n\
             [asset.lamp]\npack = \"demo\"\nsource = \"SourceFiles/FBX/SM_Lamp.fbx\"\n"
        ),
    );
    let store = dir.join("store");
    std::fs::create_dir_all(&store).expect("an empty store");
    let (ok, said) = xtask(
        &["art", "check"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(!ok, "a check that found nothing must not succeed:\n{said}");
    assert!(
        said.contains("crate_small is not on this machine"),
        "{said}"
    );
    assert!(said.contains("lamp is not on this machine"), "{said}");
    assert!(
        said.contains("2 of 2 assets are not on this machine"),
        "{said}"
    );
}

/// **A Source Files download answers before an archive does.**
///
/// Not only because it is faster. A pack's `.unitypackage` is a Unity
/// project fragment — prefabs and materials as well as meshes — and
/// rebuilding the tree recovers the meshes while dropping the assembly.
/// So the archive route is not the richer answer, it is the same answer
/// through more machinery, and it is here for packs that ship no source
/// download.
#[test]
fn a_source_files_tree_answers_before_an_archive_does() {
    let dir = scratch("source-first");
    let store = dir.join("store");
    write(
        &store.join("demo/SourceFiles/FBX/SM_Crate.fbx"),
        "the source files mesh",
    );
    unitypackage(
        &store.join("demo/Demo.unitypackage"),
        &[(
            "0123456789abcdef0123456789abcdef",
            "Assets/Demo/SM_Crate.fbx",
            Some("the archived mesh"),
        )],
    );
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n[asset.crate_small]\npack = \"demo\"\n\
             source = \"SourceFiles/FBX/SM_Crate.fbx\"\n\
             unity = \"Assets/Demo/SM_Crate.fbx\"\n"
        ),
    );
    let (ok, said) = xtask(
        &["art", "check"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("source files"), "{said}");
    assert!(
        !said.contains("Demo.unitypackage"),
        "the archive was opened even though the source files were here:\n{said}"
    );
}

/// **An asset that only exists inside a `.unitypackage` is still
/// found.** The claim the whole archive route rests on is that a
/// `.unitypackage` is a tar of GUID directories, each holding the bytes
/// as `asset` and the project path as `pathname` — so reading the second
/// and writing the first rebuilds the tree with no Unity involved. This
/// is that claim, executed.
#[test]
fn an_asset_that_only_exists_inside_an_archive_is_still_found() {
    let dir = scratch("archive-only");
    let store = dir.join("store");
    std::fs::create_dir_all(store.join("demo")).expect("a pack directory");
    unitypackage(
        &store.join("demo/Demo.unitypackage"),
        &[
            (
                "0123456789abcdef0123456789abcdef",
                "Assets/Demo/Meshes/SM_Crate.fbx",
                Some("the archived mesh"),
            ),
            // A folder entry: a pathname and no bytes, which is how
            // Unity records a directory. Nothing should be written for
            // it and nothing should choke on it.
            (
                "fedcba9876543210fedcba9876543210",
                "Assets/Demo/Meshes",
                None,
            ),
        ],
    );
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n[asset.crate_small]\npack = \"demo\"\n\
             source = \"SourceFiles/FBX/SM_Crate.fbx\"\n\
             unity = \"Assets/Demo/Meshes/SM_Crate.fbx\"\n"
        ),
    );
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "check"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("Demo.unitypackage"), "{said}");
    let rebuilt = cache.join("unpacked/demo/Demo/Assets/Demo/Meshes/SM_Crate.fbx");
    assert_eq!(
        std::fs::read_to_string(&rebuilt).ok().as_deref(),
        Some("the archived mesh"),
        "nothing at {}",
        rebuilt.display()
    );
    assert!(
        !cache
            .join("unpacked/demo/Demo/Assets/Demo/Meshes/Meshes")
            .exists(),
        "a folder entry was written as a file"
    );
}

/// **A pack with no archive in it is told that this is normal.**
/// `unpack` is the command for a pack that ships only a Unity build, and
/// reaching for it on a pack that came as Source Files is a person
/// looking in the wrong place — which is a different thing to say than
/// "no such file".
#[test]
fn a_pack_with_no_archive_says_so_without_calling_it_a_fault() {
    let dir = scratch("nothing-to-unpack");
    let store = dir.join("store");
    write(&store.join("demo/SourceFiles/FBX/SM_Crate.fbx"), "here");
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n[asset.crate_small]\npack = \"demo\"\nsource = \"SourceFiles/FBX/SM_Crate.fbx\"\n"
        ),
    );
    let (ok, said) = xtask(
        &["art", "unpack", "demo"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(!ok, "{said}");
    assert!(said.contains("no .unitypackage anywhere under"), "{said}");
    assert!(said.contains("That is normal"), "{said}");
    assert!(said.contains("Source Files"), "{said}");
}

/// **A pack that changed under the manifest stops the run and shows both
/// digests.** The digest is the manifest's only claim about the bytes it
/// was written against; a pack updated in the store is otherwise a mesh
/// that silently became a different mesh, and the override numbers beside
/// it were measured against the old one.
#[test]
fn a_pack_that_changed_under_the_manifest_stops_the_run() {
    let dir = scratch("changed-under-it");
    let store = dir.join("store");
    write(
        &store.join("demo/SourceFiles/FBX/SM_Crate.fbx"),
        "version two",
    );
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n[asset.crate_small]\npack = \"demo\"\n\
             source = \"SourceFiles/FBX/SM_Crate.fbx\"\n\
             sha256 = \"{}\"\n",
            "0".repeat(64)
        ),
    );
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(!ok, "{said}");
    assert!(
        said.contains("is not the file the manifest was written against"),
        "{said}"
    );
    assert!(
        said.contains(&"0".repeat(64)),
        "the recorded digest is not shown:\n{said}"
    );
    assert!(said.contains("cargo xtask art hash crate_small"), "{said}");
}

/// **Nothing is indexed until every asset resolves.** A half-written
/// index is a build that comes up missing one object with no error
/// anywhere near the thing that caused it — the failure has to stay where
/// the cause is.
#[test]
fn nothing_is_indexed_until_every_asset_resolves() {
    let dir = scratch("all-or-nothing");
    let store = dir.join("store");
    write(&store.join("demo/SourceFiles/FBX/SM_Crate.fbx"), "here");
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n\
             [asset.crate_small]\npack = \"demo\"\nsource = \"SourceFiles/FBX/SM_Crate.fbx\"\n\n\
             [asset.lamp]\npack = \"demo\"\nsource = \"SourceFiles/FBX/SM_Lamp.fbx\"\n"
        ),
    );
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(!ok, "{said}");
    assert!(
        !cache.join("index.toml").exists(),
        "a partial index was written"
    );
}

/// **The manifest that ships in this repository is one the resolver can
/// read.** It names no assets yet and it is still the file every reader
/// will copy a table out of, so a dialect error in its worked example
/// would be discovered by the first person to try it.
#[test]
fn the_manifest_in_the_repository_is_one_the_resolver_can_read() {
    let dir = scratch("shipped-manifest");
    let store = dir.join("store");
    std::fs::create_dir_all(&store).expect("an empty store");
    let (ok, said) = xtask(
        &["art", "check"],
        &[("SYNTY_STORE", &store), ("ART_CACHE", &dir.join("cache"))],
    );
    assert!(ok, "{said}");
    assert!(said.contains("names no assets yet"), "{said}");
}

/// **The cabin ships the whitebox unless art is asked for.**
///
/// Continuous integration cannot build the art version and never will:
/// the payload is not in this repository. So the feature that gates
/// purchased art has to exist — the seam is the expensive half — and it
/// has to be off by default, because the day it is on by default is the
/// day CI goes red everywhere at once with a missing file it has no way
/// to fetch.
#[test]
fn the_cabin_ships_the_whitebox_unless_art_is_asked_for() {
    let cargo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root")
        .join("crates/cabin/Cargo.toml");
    let text = std::fs::read_to_string(&cargo).expect("the cabin's manifest");
    let features = text
        .split("\n[features]\n")
        .nth(1)
        .expect("the cabin declares [features]")
        .split("\n[")
        .next()
        .expect("a section ends");
    assert!(
        features
            .lines()
            .any(|line| line.trim_start().starts_with("art")),
        "no `art` feature in:\n{features}"
    );
    let default = features
        .lines()
        .find(|line| line.trim_start().starts_with("default"))
        .expect("an explicit default");
    assert_eq!(
        default.trim(),
        "default = []",
        "the default build must be the whitebox"
    );
}

/// **A resolved asset is cached under the digest of the source it came
/// from, and the index says the same digest.** That is what makes "is
/// this already converted?" answerable without trusting a timestamp: a
/// pack update changes the bytes, the bytes change the digest, the digest
/// changes the path, and nothing stale is where anything looks.
///
/// Unix only, because the stand-in converter is `cp`. What it stands in
/// for is real: `$ART_CONVERTER` is any program taking a source and a
/// destination, which is what `FBX2glTF` already is. What it cannot stand
/// in for is a real FBX becoming a real glTF, and this guard does not
/// claim to.
#[cfg(unix)]
#[test]
fn a_resolved_asset_is_cached_under_the_digest_of_its_source() {
    let dir = scratch("content-addressed");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &fixtures.join("manifest.toml")),
            ("SYNTY_STORE", &fixtures.join("store")),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", Path::new("/bin/cp")),
        ],
    );
    assert!(ok, "{said}");

    let index = std::fs::read_to_string(cache.join("index.toml")).expect("an index");
    let digest = index
        .lines()
        .find_map(|line| line.strip_prefix("sha256 = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("the index records a digest")
        .to_owned();
    assert_eq!(digest.len(), 64, "{index}");
    assert!(
        index.contains(&format!("glb = \"glb/{digest}.glb\"")),
        "the cache is not addressed by the digest the index records:\n{index}"
    );
    assert!(cache.join(format!("glb/{digest}.glb")).is_file(), "{index}");
    // The texture the mesh names was staged beside it, which is the only
    // arrangement in which a relative texture path inside a mesh file
    // resolves anywhere but the tree it was exported in.
    assert!(cache.join(format!("stage/{digest}/checker.png")).is_file());
    assert!(
        cache
            .join(format!("stage/{digest}/Textures/checker.png"))
            .is_file()
    );

    // A second run has nothing left to do, and needs no converter at all
    // to discover that.
    let (again, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &fixtures.join("manifest.toml")),
            ("SYNTY_STORE", &fixtures.join("store")),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(again, "a cached asset should not need a converter:\n{said}");
}

/// **A search over the packs prints the manifest lines to paste.** A
/// Synty pack is thousands of files whose names nobody guesses, and the
/// step this tool has to remove is reading them off a store page.
#[cfg(unix)]
#[test]
fn a_search_prints_the_manifest_line_for_what_it_finds() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dir = scratch("search");
    let (ok, said) = xtask(
        &["art", "find", "unit_cube"],
        &[
            ("ART_MANIFEST", &fixtures.join("manifest.toml")),
            ("SYNTY_STORE", &fixtures.join("store")),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("pack = \"demo\""), "{said}");
    assert!(
        said.contains("source = \"SourceFiles/OBJ/unit_cube.obj\""),
        "{said}"
    );
}
