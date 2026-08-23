//! The resolver, driven the way a person drives it.
//!
//! Every guard here runs the real binary against a store built for the
//! occasion. There are no Synty assets in this repository and there never
//! will be, so the packs below are synthetic and built from nothing but
//! what this repository wrote: a loose Source Files tree, a
//! `.unitypackage` assembled here out of GUID directories, `pathname`
//! files and `asset` files, and a zip written byte by byte — a header per
//! member, a table at the end, and no compressor.
//!
//! What that proves and what it does not is worth being exact about.
//! Proved here: the manifest dialect, the missing-asset message, the
//! order the three places are looked in, the reconstruction of a tree out
//! of a `.unitypackage`, reading the names inside an archive without
//! extracting it, taking only the files a manifest named out of one,
//! refusing a member name that would climb out of the tree, a pack
//! directory named and filled the way a store leaves it, content
//! addressing, the digest check, and that a partial resolve indexes
//! nothing.
//!
//! Not proved here, and not provable without the owner's disk: that a
//! real Synty FBX converts to a glTF that looks right, that a real
//! Synty zip is laid out the way these are, and that `tar` opens a zip
//! where it is bsdtar rather than GNU tar — the Linux runner these guards
//! meet has GNU tar, so what runs here is the fallback to `unzip`.
//! `docs/ART_PIPELINE.md` says which commands close those gaps.

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

/// A zip, written here byte by byte rather than shelled out to.
///
/// `zip` is not on every machine that runs these guards and the format's
/// stored form needs no compressor at all: a header and the bytes for
/// each member, a table of the same headers at the end, and a record
/// saying where that table starts. That is the whole of what the
/// resolver's readers are pointed at, and building it here means these
/// guards depend on nothing this repository did not write.
fn zip_archive(at: &Path, entries: &[(&str, &str)]) {
    let mut body: Vec<u8> = Vec::new();
    let mut table: Vec<u8> = Vec::new();
    for (name, contents) in entries {
        let offset = u32::try_from(body.len()).expect("a small fixture");
        let crc = crc32(contents.as_bytes());
        let size = u32::try_from(contents.len()).expect("a small member");
        let name_len = u16::try_from(name.len()).expect("a short name");
        // Local file header, then the bytes, stored (method 0).
        body.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        body.extend_from_slice(&[10, 0, 0, 0, 0, 0]); // version, flags, method
        body.extend_from_slice(&[0, 0, 0x21, 0]); // 1980-01-01, 00:00
        body.extend_from_slice(&crc.to_le_bytes());
        body.extend_from_slice(&size.to_le_bytes());
        body.extend_from_slice(&size.to_le_bytes());
        body.extend_from_slice(&name_len.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes()); // no extra field
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(contents.as_bytes());
        // The same header again in the central directory, plus where the
        // local one is.
        table.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        table.extend_from_slice(&[20, 0, 10, 0, 0, 0, 0, 0]);
        table.extend_from_slice(&[0, 0, 0x21, 0]);
        table.extend_from_slice(&crc.to_le_bytes());
        table.extend_from_slice(&size.to_le_bytes());
        table.extend_from_slice(&size.to_le_bytes());
        table.extend_from_slice(&name_len.to_le_bytes());
        table.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        table.extend_from_slice(&offset.to_le_bytes());
        table.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(entries.len()).expect("a small fixture");
    let start = u32::try_from(body.len()).expect("a small fixture");
    let table_len = u32::try_from(table.len()).expect("a small fixture");
    body.extend_from_slice(&table);
    body.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(&count.to_le_bytes());
    body.extend_from_slice(&count.to_le_bytes());
    body.extend_from_slice(&table_len.to_le_bytes());
    body.extend_from_slice(&start.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes()); // no comment
    std::fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    std::fs::write(at, &body).expect("a zip");
}

/// The checksum a zip records for each member, which every reader checks.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
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
    let rebuilt = cache.join("unpacked/demo/Demo.unitypackage/Assets/Demo/Meshes/SM_Crate.fbx");
    assert_eq!(
        std::fs::read_to_string(&rebuilt).ok().as_deref(),
        Some("the archived mesh"),
        "nothing at {}",
        rebuilt.display()
    );
    assert!(
        !cache
            .join("unpacked/demo/Demo.unitypackage/Assets/Demo/Meshes/Meshes")
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

/// The pack directory the owner actually has: a name the store chose,
/// with spaces and capitals in it, holding the icon, the `.unitypackage`
/// and the raw assets still zipped.
const ZIPPED_PACK: &str = "\
[pack.scifi]
title = \"POLYGON Sci-Fi Space\"
dir = \"POLYGON Sci-Fi Space\"
download = \"POLYGON Sci-Fi Space, the Source Files download\"
";

/// Build that pack under `store`, and hand back the pack directory.
fn zipped_pack(store: &Path) -> PathBuf {
    let pack = store.join("POLYGON Sci-Fi Space");
    write(&pack.join("icon.png"), "not a mesh, and not an archive");
    zip_archive(
        &pack.join("POLYGON Sci-Fi Space.zip"),
        &[
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Crate_01.fbx",
                "the crate",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Crate_02.fbx",
                "a second crate",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Lamp_01.fbx",
                "a lamp",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/Textures/atlas.png",
                "the atlas the whole pack is painted from",
            ),
            ("POLYGON Sci-Fi Space/Prefabs/Crate.prefab", "assembly"),
            ("POLYGON Sci-Fi Space/readme.txt", "terms"),
        ],
    );
    pack
}

/// Every file under a directory, so a guard can say what a run did and
/// did not put on disk.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                stack.push(entry.path());
            } else {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found
}

/// **An asset still inside the pack's archive resolves, and only the
/// files the manifest named come out of it.**
///
/// This is the whole bargain. A Synty pack is thousands of files in one
/// zip, a manifest names a few dozen of them, and unzipping the pack to
/// reach those few is the cost the tool exists to avoid — on every
/// machine, for ever, for art the game already has a whitebox for. So the
/// store keeps the download exactly as it arrived and the cache holds the
/// named files and nothing else.
#[test]
fn only_the_files_a_manifest_names_come_out_of_a_packs_archive() {
    let dir = scratch("named-only");
    let store = dir.join("store");
    let pack = zipped_pack(&store);
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{ZIPPED_PACK}\n[asset.crate_small]\npack = \"scifi\"\n\
             source = \"SourceFiles/FBX/SM_Crate_01.fbx\"\n\
             texture = \"SourceFiles/Textures/atlas.png\"\n"
        ),
    );
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", Path::new("/bin/cp")),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("in POLYGON Sci-Fi Space.zip"), "{said}");

    // Two files were named — the mesh and the atlas it is painted from —
    // out of an archive holding six.
    let taken = files_under(&cache.join("unpacked"));
    assert_eq!(
        taken.len(),
        2,
        "the archive holds six files and the manifest named two:\n{taken:#?}"
    );
    assert!(
        taken.iter().all(|path| {
            let name = path.file_name().expect("a name").to_string_lossy();
            name == "SM_Crate_01.fbx" || name == "atlas.png"
        }),
        "{taken:#?}"
    );

    // And the store is exactly what it was: the icon and the archive.
    assert_eq!(files_under(&pack).len(), 2, "the store was written to");
}

/// **A pack directory is read exactly as it arrived, furniture and
/// all.** The owner's store holds, per pack, a name the store humanized
/// — spaces and capitals in it — wrapped around an icon, a
/// `.unitypackage` and a compressed archive of the raw assets. Nothing in
/// that has been unzipped and nothing in it should have to be. `dir` is
/// the owner's own directory name rather than a guess at Synty's naming
/// precisely so this case is an ordinary one.
#[test]
fn a_pack_directory_named_and_filled_the_way_the_store_left_it_is_read() {
    let dir = scratch("as-it-arrived");
    let store = dir.join("store");
    let pack = zipped_pack(&store);
    unitypackage(
        &pack.join("POLYGON Sci-Fi Space.unitypackage"),
        &[(
            "0123456789abcdef0123456789abcdef",
            "Assets/Polygon/SM_Crate_01.fbx",
            Some("the crate, through Unity"),
        )],
    );
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{ZIPPED_PACK}\n[asset.crate_small]\npack = \"scifi\"\n\
             source = \"SourceFiles/FBX/SM_Crate_01.fbx\"\n"
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
    assert!(said.contains("in POLYGON Sci-Fi Space.zip"), "{said}");
    assert!(
        !said.contains("icon.png"),
        "the icon was mistaken for something to open:\n{said}"
    );
    assert!(
        !said.contains("rebuilding the tree"),
        "the .unitypackage was unpacked to find a file the zip carries:\n{said}"
    );
}

/// **The pack's own archive answers before its `.unitypackage` does.**
///
/// The order is the same one a loose Source Files tree already had, for
/// the same reason: a `.unitypackage` is a Unity project fragment —
/// prefabs and materials as well as meshes — and rebuilding the tree
/// recovers the meshes while dropping the assembly. A prop is often a
/// prefab assembling several meshes against a shared material, so the
/// archive route is not the richer answer. It is the same answer through
/// more machinery, and it is here for the packs that ship no source
/// download. Zipping the source download changes none of that.
#[test]
fn the_packs_own_archive_answers_before_its_unitypackage_does() {
    let dir = scratch("zip-before-unity");
    let store = dir.join("store");
    let pack = store.join("demo");
    zip_archive(
        &pack.join("Source Files.zip"),
        &[("Demo/SourceFiles/FBX/SM_Crate.fbx", "the zipped mesh")],
    );
    unitypackage(
        &pack.join("Demo.unitypackage"),
        &[(
            "0123456789abcdef0123456789abcdef",
            "Assets/Demo/SM_Crate.fbx",
            Some("the Unity mesh"),
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
    assert!(said.contains("in Source Files.zip"), "{said}");
    assert!(
        !said.contains("Demo.unitypackage"),
        "the archive was opened even though the zip carried the mesh:\n{said}"
    );
    assert!(
        !cache.join("unpacked/demo/Demo.unitypackage").exists(),
        "a whole .unitypackage was rebuilt for a file the zip carries"
    );
}

/// **A search reads the names inside an archive and extracts nothing.**
///
/// Before this, a store of zipped packs was unsearchable: the walk saw
/// the icon and the archive and none of the thousands of names in it, and
/// trawling a pack by hand is the problem the tool exists to solve. A zip
/// keeps a table of its members at the end of the file, so the answer
/// costs a read of that table however many gigabytes the members are —
/// which is why `find` may do this for every pack and `resolve` may not.
#[test]
fn a_search_reads_the_names_inside_an_archive_and_extracts_nothing() {
    let dir = scratch("search-inside");
    let store = dir.join("store");
    zipped_pack(&store);
    let manifest = dir.join("manifest.toml");
    write(&manifest, ZIPPED_PACK);
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "find", "crate"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("pack = \"scifi\""), "{said}");
    assert!(
        said.contains("source = \"POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Crate_01.fbx\""),
        "a manifest line for a file only the zip carries:\n{said}"
    );
    assert!(
        said.contains("SM_Crate_02.fbx"),
        "both crates are in there:\n{said}"
    );
    assert!(
        !said.contains("SM_Lamp_01.fbx"),
        "a search for crates answered with a lamp:\n{said}"
    );
    assert!(
        files_under(&cache).is_empty(),
        "a search wrote something: {:#?}",
        files_under(&cache)
    );
}

/// **A member whose name climbs out of the tree is never offered as a
/// hit, and never taken out.** The archive is a file somebody
/// downloaded, and `../../.ssh/authorized_keys` is a perfectly
/// well-formed name for something inside it. `unzip` happens to strip
/// the `..` and write the file somewhere else, which is not the same as
/// refusing it and is not a promise every reader on every platform
/// makes.
#[test]
fn a_member_that_climbs_out_of_the_tree_is_never_offered_as_a_hit() {
    let dir = scratch("hostile-member");
    let store = dir.join("store");
    zip_archive(
        &store.join("demo/Raw Assets.zip"),
        &[
            ("../evil.fbx", "outside"),
            ("Demo/SourceFiles/FBX/evil.fbx", "inside"),
        ],
    );
    let manifest = dir.join("manifest.toml");
    write(&manifest, PACK);
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "find", "evil"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(ok, "{said}");
    assert!(
        said.contains("source = \"Demo/SourceFiles/FBX/evil.fbx\""),
        "{said}"
    );
    assert!(
        !said.contains("../evil.fbx"),
        "a name that may never be written was offered as a manifest line:\n{said}"
    );

    // And asking for it outright gets nothing, anywhere.
    write(
        &manifest,
        &format!("{PACK}\n[asset.evil]\npack = \"demo\"\nsource = \"../evil.fbx\"\n"),
    );
    let (resolved, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", Path::new("/bin/cp")),
        ],
    );
    assert!(!resolved, "{said}");
    assert!(
        !files_under(&dir)
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "evil.fbx")),
        "a member climbed out of the archive: {:#?}",
        files_under(&dir)
    );
}

/// **A machine that cannot open a zip is told which program to install,
/// not that its art is missing.**
///
/// The two situations look identical from the outside and have nothing in
/// common: one is a download, the other is a package manager. `tar`
/// already reads zip on macOS and on Windows 10 build 1803 and later,
/// where it is bsdtar; Linux ships GNU tar, which does not, and needs
/// `unzip`. Saying "not on this machine" about a mesh that is sitting in
/// the archive would send somebody looking through their downloads for a
/// pack they already have.
#[cfg(unix)]
#[test]
fn a_machine_that_cannot_open_a_zip_is_told_which_program_to_install() {
    let dir = scratch("no-zip-reader");
    let store = dir.join("store");
    zipped_pack(&store);
    let nothing = dir.join("empty-path");
    std::fs::create_dir_all(&nothing).expect("a directory with no programs in it");
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{ZIPPED_PACK}\n[asset.crate_small]\npack = \"scifi\"\n\
             source = \"SourceFiles/FBX/SM_Crate_01.fbx\"\n"
        ),
    );
    let (ok, said) = xtask(
        &["art", "check"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
            ("PATH", &nothing),
        ],
    );
    assert!(!ok, "{said}");
    assert!(said.contains("can open"), "{said}");
    assert!(said.contains("unzip"), "{said}");
    assert!(said.contains("bsdtar"), "{said}");
    assert!(
        !said.contains("Download \""),
        "the pack is here; it is the reader that is not:\n{said}"
    );
}

/// The shape a real library has, and the shape that made a search mislead
/// its owner: one pack the manifest declares, and a store's worth of
/// packs it does not, holding many times more matches for the same
/// ordinary word.
fn crowded_store(dir: &Path) -> PathBuf {
    let store = dir.join("store");
    zip_archive(
        &store.join("POLYGON Sci-Fi Space/POLYGON Sci-Fi Space.zip"),
        &[
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Prop_Crate_01.fbx",
                "the crate",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Prop_Crate_02.fbx",
                "a second crate",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Prop_Crate_03.fbx",
                "a third crate",
            ),
            (
                "POLYGON Sci-Fi Space/SourceFiles/Textures/atlas.png",
                "the atlas the whole pack is painted from",
            ),
        ],
    );
    for pack in 1..=40 {
        let name = format!("POLYGON Pack {pack:02}");
        let members: Vec<(String, String)> = (1..=5)
            .map(|one| {
                (
                    format!("{name}/SourceFiles/FBX/SM_Crate_{pack:02}_{one}.fbx"),
                    format!("a crate nobody asked about, in {name}"),
                )
            })
            .collect();
        let entries: Vec<(&str, &str)> = members
            .iter()
            .map(|(name, contents)| (name.as_str(), contents.as_str()))
            .collect();
        zip_archive(&store.join(&name).join(format!("{name}.zip")), &entries);
    }
    store
}

/// **A match in a pack the manifest declares is printed before anything
/// else, and is never what a cap cuts.**
///
/// This is what a hundred-pack library did to the old answer. `crate` is
/// a word four hundred files in such a library are called, the answer was
/// cut at a fixed number of lines in whatever order the packs happened to
/// be walked, and every match in the pack the owner was working in landed
/// in the hidden tail — so that pack read as though it held no crates at
/// all. The manifest is this project's own statement of which packs it
/// cares about, and this is that statement being worth something.
#[test]
fn matches_in_a_pack_the_manifest_declares_are_never_the_ones_cut() {
    let dir = scratch("crowded-search");
    let store = crowded_store(&dir);
    let manifest = dir.join("manifest.toml");
    write(&manifest, ZIPPED_PACK);
    let (ok, said) = xtask(
        &["art", "find", "crate"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(ok, "{said}");
    for crated in [
        "SM_Prop_Crate_01.fbx",
        "SM_Prop_Crate_02.fbx",
        "SM_Prop_Crate_03.fbx",
    ] {
        assert!(
            said.contains(crated),
            "{crated} is in the one pack the manifest declares and was cut:\n{said}"
        );
    }
    let declared = said
        .find("pack = \"scifi\"")
        .expect("the declared pack is named");
    let rest = said
        .find("POLYGON Pack ")
        .expect("the undeclared packs are named");
    assert!(
        declared < rest,
        "a pack the manifest declares was printed below one it does not:\n{said}"
    );
}

/// **A directory whose matches are cut still says how many it has.**
///
/// A cap that hides four hundred files is a poor answer however it is
/// ordered. "This pack has five crates in it" is most of what somebody
/// browsing a hundred packs wanted from the search, and it is the part
/// the old cap threw away first: a pack past the line simply was not
/// mentioned. So every directory that matched prints its count whether or
/// not any of its matches fit, and the line saying the answer was cut
/// says how much of it was.
#[test]
fn a_pack_whose_matches_are_cut_still_says_how_many_it_has() {
    let dir = scratch("crowded-counts");
    let store = crowded_store(&dir);
    let manifest = dir.join("manifest.toml");
    write(&manifest, ZIPPED_PACK);
    let (ok, said) = xtask(
        &["art", "find", "crate"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(ok, "{said}");
    assert!(
        !said.contains("SM_Crate_40_"),
        "this guard needs a pack whose matches did not fit:\n{said}"
    );
    let counted = said
        .lines()
        .find(|line| line.starts_with("POLYGON Pack 40"))
        .expect("a pack with no room for its matches is still named");
    assert!(
        counted.contains("5 matches"),
        "a pack with no room for its matches does not say how many it has: {counted}"
    );
    assert!(
        said.contains("not shown"),
        "nothing said the answer had been cut:\n{said}"
    );
}

/// **A search reads a pack's `.unitypackage` only when it is the only
/// archive that pack has.**
///
/// That is the order `locate` already looks in, for the reason it already
/// gives: the Source Files download holds the same meshes and holds them
/// as files. Here it is also what makes searching a whole library
/// finish. A zip keeps a table of its members at the end, so listing one
/// is a seek however large it is; a `.unitypackage` is a gzipped tar and
/// keeps no table at all, so the names inside it are only reachable by
/// decompressing the whole file — a second per hundred megabytes, and a
/// Synty library is a hundred packs of them. What is skipped is counted
/// and said out loud, because a search that quietly left out part of the
/// store is a search whose empty answer means two things.
#[test]
fn a_search_reads_a_packs_unitypackage_only_when_nothing_else_is_there() {
    let dir = scratch("search-skips-unity");
    let store = dir.join("store");
    let both = store.join("POLYGON Sci-Fi Space");
    zip_archive(
        &both.join("POLYGON Sci-Fi Space.zip"),
        &[(
            "POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Zipped_Crate.fbx",
            "the crate in the source files",
        )],
    );
    unitypackage(
        &both.join("POLYGON Sci-Fi Space.unitypackage"),
        &[(
            "1111",
            "Assets/PolygonSciFi/SM_Unity_Crate.fbx",
            Some("the same crate, through more machinery"),
        )],
    );
    let alone = store.join("POLYGON Sci-Fi Horror");
    unitypackage(
        &alone.join("POLYGON Sci-Fi Horror.unitypackage"),
        &[(
            "2222",
            "Assets/PolygonSciFiHorror/SM_Only_Crate.fbx",
            Some("a pack that ships no source download"),
        )],
    );
    let manifest = dir.join("manifest.toml");
    write(&manifest, ZIPPED_PACK);
    let (ok, said) = xtask(
        &["art", "find", "crate"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
        ],
    );
    assert!(ok, "{said}");
    assert!(
        said.contains("SM_Zipped_Crate.fbx"),
        "the archive beside it was not read:\n{said}"
    );
    assert!(
        !said.contains("SM_Unity_Crate.fbx"),
        "a .unitypackage was decompressed with its own Source Files archive beside it:\n{said}"
    );
    assert!(
        said.contains("SM_Only_Crate.fbx"),
        "a pack that ships nothing but a .unitypackage went unsearched:\n{said}"
    );
    assert!(
        said.contains("1 .unitypackage file"),
        "nothing said which part of the store went unread:\n{said}"
    );
}

/// A converter that copies its source and reports a size for it, written
/// into `dir` and handed back as a path.
///
/// The measurement is the only new thing in the contract and it is one
/// line on standard output, so the whole of a conforming converter is
/// still `cp` and a `printf` — which is the property the contract was
/// shaped for. `$ART_CONVERTER` is documented as any program taking a
/// source and a destination, and the day it has to be a program that
/// links a glTF library is the day nobody can write one.
#[cfg(unix)]
fn measuring_converter(dir: &Path, aabb: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("convert.sh");
    write(
        &path,
        &format!("#!/bin/sh\ncp \"$1\" \"$2\"\nprintf 'aabb {aabb}\\n'\n"),
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("a converter this test can run");
    path
}

/// A one-asset store and manifest for the fill guards below, with
/// whatever `dresses`, `scale` and `fill` lines the caller wants on it.
#[cfg(unix)]
fn dressed_store(dir: &Path, lines: &str) -> (PathBuf, PathBuf) {
    let store = dir.join("store");
    write(&store.join("demo/SourceFiles/FBX/SM_Crate.fbx"), "a crate");
    let manifest = dir.join("manifest.toml");
    write(
        &manifest,
        &format!(
            "{PACK}\n[asset.crate_small]\npack = \"demo\"\n\
             source = \"SourceFiles/FBX/SM_Crate.fbx\"\n{lines}"
        ),
    );
    (store, manifest)
}

/// **A `dresses` line reaches the index, and so does the size the
/// converter measured.**
///
/// The index is the one artefact the game's own build parses, and it is
/// parsed by a reader in another crate that cannot call into this one. So
/// every field the cabin needs has to make the crossing: which body draws
/// the mesh, the four overrides that place it, and — new here — the box
/// the converter actually found round it, which is what lets the cabin
/// put the mesh in the middle of its berth instead of wherever the
/// exporter happened to leave the origin.
#[cfg(unix)]
#[test]
fn what_a_mesh_dresses_and_what_it_measures_reach_the_index() {
    let dir = scratch("dresses");
    let (store, manifest) = dressed_store(
        &dir,
        "dresses = \"cargo/suspicious_crate\"\n\
         scale = [2.0, 2.0, 2.0]\nfill = [0.5, 0.5, 0.5]\n",
    );
    let cache = dir.join("cache");
    let converter = measuring_converter(&dir, "-0.25 -0.25 -0.25 0.25 0.25 0.25");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", &converter),
        ],
    );
    assert!(ok, "{said}");
    let index = std::fs::read_to_string(cache.join("index.toml")).expect("an index");
    assert!(
        index.contains("dresses = \"cargo/suspicious_crate\""),
        "the binding did not survive the crossing:\n{index}"
    );
    assert!(
        index.contains("measured_half = [0.25, 0.25, 0.25]"),
        "the measurement did not survive the crossing:\n{index}"
    );
    assert!(
        index.contains("measured_mid = [0.0, 0.0, 0.0]"),
        "the mesh's own middle did not survive the crossing:\n{index}"
    );

    // A second run converts nothing and still knows the size, because
    // the measurement is filed under the same digest the glb is: a check
    // that only ran on the run that happened to convert would be a check
    // that stops running the moment the cache is warm.
    let (again, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
        ],
    );
    assert!(again, "{said}");
    assert!(
        std::fs::read_to_string(cache.join("index.toml"))
            .expect("an index")
            .contains("measured_half = [0.25, 0.25, 0.25]"),
        "a warm cache forgot what the mesh measured:\n{said}"
    );
}

/// **A mesh that is not the size its `fill` claims stops the run, and
/// the run says which line to paste.**
///
/// This is the whole point of the measurement. `fill` is a promise about
/// how much of its berth a purchased body occupies, and every containment
/// rule in the game downstream reads it as a fact — so a promise nothing
/// ever checks is exactly the shape of defect the field was invented to
/// stop, moved one level up. The converter is the only program in the
/// pipeline that can see a mesh, so this is the only moment the two can
/// be made to meet.
///
/// Unix only, for the same reason the content-addressing guard is: the
/// stand-in converter is a shell script.
#[cfg(unix)]
#[test]
fn a_mesh_that_is_not_the_size_its_fill_claims_stops_the_run() {
    let dir = scratch("fill-promise");
    // Half a unit each way at unit scale: it occupies half its berth box
    // on every axis, and the line below says it fills the whole thing.
    let (store, manifest) = dressed_store(
        &dir,
        "dresses = \"cargo/suspicious_crate\"\nfill = [1.0, 1.0, 1.0]\n",
    );
    let cache = dir.join("cache");
    let converter = measuring_converter(&dir, "-0.5 -0.5 -0.5 0.5 0.5 0.5");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", &converter),
        ],
    );
    assert!(!ok, "a fill twice the mesh was accepted:\n{said}");
    for wanted in [
        "crate_small",
        "cargo/suspicious_crate",
        "fill = ",
        "scale = ",
    ] {
        assert!(said.contains(wanted), "no `{wanted}` in:\n{said}");
    }
    assert!(
        !cache.join("index.toml").exists(),
        "a broken promise was indexed anyway"
    );

    // The same mesh, with the line the refusal printed. Nothing else
    // changes, and the cache is warm — so this also proves the check
    // reads the measurement back rather than re-measuring.
    let (fixed, said) = {
        let (store, manifest) = dressed_store(
            &dir,
            "dresses = \"cargo/suspicious_crate\"\nfill = [0.5, 0.5, 0.5]\n",
        );
        xtask(
            &["art", "resolve"],
            &[
                ("ART_MANIFEST", &manifest),
                ("SYNTY_STORE", &store),
                ("ART_CACHE", &cache),
            ],
        )
    };
    assert!(
        fixed,
        "the fill the refusal printed was refused too:\n{said}"
    );
    assert!(cache.join("index.toml").is_file(), "{said}");
}

/// **A converter that measures nothing leaves the promise unchecked and
/// says so.**
///
/// The contract is deliberately open: `$ART_CONVERTER` is any program
/// taking a source and a destination, and `FBX2glTF` has never heard of
/// this repository. Refusing every asset such a converter produces would
/// make the escape hatch useless; indexing them silently would make the
/// `fill` check look like it ran. So the run says which assets went
/// unchecked and what a converter would have to print to be checked.
#[cfg(unix)]
#[test]
fn a_converter_that_measures_nothing_says_which_promises_went_unchecked() {
    let dir = scratch("unmeasured");
    let (store, manifest) = dressed_store(
        &dir,
        "dresses = \"cargo/suspicious_crate\"\nfill = [1.0, 1.0, 1.0]\n",
    );
    let cache = dir.join("cache");
    let (ok, said) = xtask(
        &["art", "resolve"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &cache),
            ("ART_CONVERTER", Path::new("/bin/cp")),
        ],
    );
    assert!(ok, "an unmeasured mesh was refused:\n{said}");
    assert!(said.contains("crate_small"), "{said}");
    assert!(said.contains("aabb"), "nothing said what to print:\n{said}");
}
