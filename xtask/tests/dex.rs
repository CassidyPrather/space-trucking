//! The catalogue, written the way a person writes it.
//!
//! Every guard here runs the real binary against the fixture packs in
//! this repository, with two stand-in programs in place of the two things
//! this machine does not have: something that can render a mesh, and
//! something that can look at a picture. Both seams exist for their own
//! reasons — `$ART_PREVIEW` for anybody who would rather not install
//! 300 MB, `$ART_DESCRIBER` for anybody with a model of their own — and
//! being able to run the whole command in continuous integration is what
//! they cost nothing to also be.
//!
//! **The describer stand-in prints the prompt it was handed.** That is
//! the trick the guards below turn on, and it is worth saying why. The
//! one thing that makes a catalogue of five thousand Synty meshes worth
//! anything is that the model is told which mesh it is looking at:
//! shown a picture and asked what it is, a model writes "a low-poly
//! crate", which is the sentence the file name already contained. So the
//! stand-in answers with its own prompt, the prompt lands in the
//! catalogue as the description, and the guard reads the file to prove
//! the asset's own name and pack were in it.
//!
//! What is proved here: that a run writes a catalogue somebody can read
//! back, that the numbers a previewer measured reach it, that the
//! describer is told what it is looking at, that a pack the manifest
//! never declared can still be catalogued, that a mesh already described
//! is left alone until it changes or `--force` says otherwise, that the
//! catalogue is searched by what it says as well as by what things are
//! called, and that a describer which answers nothing writes nothing.
//!
//! What is not proved here, and is not provable without the owner's disk
//! and a key: that Blender renders four legible views of a Synty prop,
//! and that a hosted model writes something true about the picture.
//! `docs/ART_PIPELINE.md` says which command closes those.

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("space-trucking-dex-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    std::fs::write(path, text).expect("a file");
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Run the catalogue commands, and hand back everything they said on
/// either stream. Every variable this command reads is cleared first: a
/// developer with `$OPENROUTER_API_KEY` exported would otherwise have
/// these guards spend money on a fixture cube.
fn xtask(arguments: &[&str], environment: &[(&str, &Path)]) -> (bool, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(arguments);
    for variable in [
        "SYNTY_STORE",
        "ART_MANIFEST",
        "ART_CACHE",
        "ART_DEX",
        "ART_CONVERTER",
        "ART_PREVIEW",
        "ART_DESCRIBER",
        "ART_DESCRIBER_MODEL",
        "OPENROUTER_API_KEY",
        "OPENROUTER_URL",
        "BLENDER",
    ] {
        command.env_remove(variable);
    }
    for (key, value) in environment {
        command.env(key, value);
    }
    let output = command.output().expect("the errand runner runs");
    let mut said = String::from_utf8_lossy(&output.stdout).into_owned();
    said.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), said)
}

/// A program the host can actually run — a shell script on unix, the same
/// thoughts as a `.cmd` on Windows — so this suite runs on the owner's
/// machine and not only in the Linux container.
fn stand_in(dir: &Path, name: &str, sh: &str, cmd: &str) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join(format!("{name}.cmd"));
        write(&path, &format!("@echo off\r\n{cmd}"));
        return path;
    }
    let path = dir.join(format!("{name}.sh"));
    write(&path, &format!("#!/bin/sh\n{sh}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("a program this test can run");
    }
    path
}

/// **A previewer: it reads the jobs file, copies each mesh to where its
/// picture goes so there is a non-empty file there, and prints the facts
/// a real one measures.**
///
/// Four lines of shell, which is the point of the `|` separator — the
/// contract is one a person can write a program for in either dialect
/// without a parser.
///
/// It also appends one line per launch to `launches`, which is how the
/// guard below proves a chunk of meshes costs one launch rather than one
/// each.
fn previewer(dir: &Path) -> (PathBuf, PathBuf) {
    const FACTS: [&str; 4] = ["tris 12", "meshes 1", "materials 1", "image checker.png"];
    let launches = dir.join("launches.txt");
    let said = |before: &str, after: &str| -> String {
        FACTS.iter().fold(String::new(), |mut script, fact| {
            script.push_str(before);
            script.push_str(fact);
            script.push_str(after);
            script
        })
    };
    let program = stand_in(
        dir,
        "preview",
        &format!(
            "printf 'x\\n' >> '{launches}'\n\
             n=0\n\
             while IFS='|' read -r src dst tex; do\n\
             n=$((n+1))\n\
             [ -n \"$src\" ] || continue\n\
             mkdir -p \"$(dirname \"$dst\")\"\n\
             cp \"$src\" \"$dst\"\n\
             printf 'look %s\\n' \"$n\"\n\
             {facts}\
             printf 'aabb -0.5 0 -0.25 0.5 1 0.25\\n'\n\
             done < \"$1\"\n",
            launches = launches.display(),
            facts = said("printf '%s\\n' '", "'\n"),
        ),
        &format!(
            "setlocal enabledelayedexpansion\r\n\
             >>\"{launches}\" echo x\r\n\
             set /a n=0\r\n\
             for /f \"usebackq tokens=1,2,3 delims=|\" %%a in (\"%~1\") do (\r\n\
             set /a n+=1\r\n\
             copy /y \"%%~a\" \"%%~b\" >nul\r\n\
             echo look !n!\r\n\
             {facts}\
             echo aabb -0.5 0 -0.25 0.5 1 0.25\r\n\
             )\r\n",
            launches = launches.display(),
            facts = said("echo ", "\r\n"),
        ),
    );
    (program, launches)
}

/// A describer that answers with the prompt it was given, keeps every
/// prompt it was handed, and keeps a tally of how many times it was
/// asked.
///
/// The prompt is the thing under test — see this file's own
/// documentation. It is kept as well as answered with because a
/// description is cut to three lines on the way into the catalogue and a
/// prompt is longer than that: what lands in the file proves the front of
/// it, and the kept copy proves the rest.
struct Answering {
    program: PathBuf,
    /// One line per time the describer was asked.
    tally: PathBuf,
    /// Every prompt handed over, one after another.
    prompts: PathBuf,
}

fn describer(dir: &Path) -> Answering {
    let tally = dir.join("asked.txt");
    let prompts = dir.join("prompts.txt");
    let program = stand_in(
        dir,
        "describe",
        &format!(
            "printf 'x\\n' >> '{tally}'\ncat \"$1\" >> '{prompts}'\ncat \"$1\"\n",
            tally = tally.display(),
            prompts = prompts.display(),
        ),
        &format!(
            ">>\"{tally}\" echo x\r\ntype \"%~1\" >>\"{prompts}\"\r\ntype \"%~1\"\r\n",
            tally = tally.display(),
            prompts = prompts.display(),
        ),
    );
    Answering {
        program,
        tally,
        prompts,
    }
}

fn asked(tally: &Path) -> usize {
    std::fs::read_to_string(tally)
        .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or_default()
}

/// Everything one run of `describe` wrote, as text.
fn catalogue(dex: &Path, pack: &str) -> String {
    std::fs::read_to_string(dex.join(format!("{pack}.toml")))
        .unwrap_or_else(|err| panic!("no catalogue for `{pack}`: {err}"))
}

/// **A described mesh becomes a line somebody can read, with the numbers
/// beside it.** The whole point of the command: a file called
/// `unit_cube.obj` was a name and is now a name, a size, a triangle
/// count, the texture it bore and a sentence.
#[test]
fn a_described_mesh_becomes_a_catalogue_line_with_its_measurements_in_it() {
    let dir = scratch("described");
    let dex = dir.join("dex");
    let answering = describer(&dir);
    let (ok, said) = xtask(
        &["art", "describe"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer(&dir).0),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");

    let written = catalogue(&dex, "demo");
    for wanted in [
        "[mesh.unit_cube]",
        "name = \"unit_cube\"",
        "source = \"SourceFiles/OBJ/unit_cube.obj\"",
        "asset = \"unit_cube\"",
        "atlas = \"checker.png\"",
        "textures = \"checker.png\"",
        "triangles = 12",
        "meshes = 1",
        "materials = 1",
        "size = [1.0, 1.0, 0.5]",
        "sha256 = \"200bd103547c00b871f1c90d2d00756415feceebd7db35edf20801744fb79b4c\"",
    ] {
        assert!(written.contains(wanted), "no `{wanted}` in:\n{written}");
    }
    // The pack that arrived zipped is catalogued out of its zip, into a
    // file of its own, because a pack is the unit somebody buys.
    assert!(catalogue(&dex, "zipped").contains("[mesh.unit_pyramid]"));

    // And what was written is read back by the reader, not by this test:
    // a catalogue that only this suite can parse is not a catalogue.
    let (ok, listed) = xtask(&["art", "dex"], &[("ART_DEX", &dex)]);
    assert!(ok, "{listed}");
    assert!(listed.contains("unit_cube"), "{listed}");
    assert!(listed.contains("12 tris"), "{listed}");
}

/// **The describer is told what it is looking at.**
///
/// The guard this whole command is shaped around, proved end to end
/// rather than at the function that builds the prompt. A vision model
/// shown a picture and asked what it is answers with the category — "a
/// low-poly crate" — which is precisely the sentence the file name
/// already carried and precisely what makes a catalogue of four hundred
/// crates worthless. So the asset's own name, the pack's name and the
/// measurements go in the prompt, and the instruction is to spend the
/// sentence on what the picture adds.
#[test]
fn the_describer_is_told_the_name_and_the_pack_of_what_it_is_looking_at() {
    let dir = scratch("prompted");
    let dex = dir.join("dex");
    let answering = describer(&dir);
    // One at a time, so the kept prompts are one after another rather
    // than two workers writing into one file.
    let (ok, said) = xtask(
        &["art", "describe", "--jobs", "1"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer(&dir).0),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");

    let handed = std::fs::read_to_string(&answering.prompts).expect("the prompts it was handed");
    for wanted in [
        "unit_cube",        // the pack's own name for this one
        "unit_pyramid",     // and for the other
        "The fixture pack", // the pack they came out of
        "12 triangles",     // what was measured, so it is not invented
        "checker.png",      // what it was painted with
        "do not repeat the name",
        "do not restate the size",
    ] {
        assert!(
            handed.contains(wanted),
            "the describer was not told `{wanted}`:\n{handed}"
        );
    }

    // And the front of that prompt is what the stand-in answered with,
    // so the name reaches the catalogue through the description as well
    // as through the prompt.
    let written = catalogue(&dex, "demo");
    let description = written
        .lines()
        .find_map(|line| line.strip_prefix("description = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("no description in:\n{written}"));
    assert!(description.contains("unit_cube"), "{description}");
    assert!(description.contains("The fixture pack"), "{description}");
}

/// **A pack the manifest never declared is still catalogued.** Browsing a
/// library is the moment before a pack gets declared: a catalogue that
/// could only describe what was already in `art/manifest.toml` would be a
/// catalogue of the things somebody had already chosen, which is the one
/// set of assets nobody needs help choosing between.
#[test]
fn a_pack_the_manifest_never_declared_is_still_catalogued() {
    let dir = scratch("undeclared");
    let dex = dir.join("dex");
    let manifest = dir.join("empty.toml");
    write(&manifest, "# A manifest that declares nothing at all.\n");
    let answering = describer(&dir);
    let (ok, said) = xtask(
        &["art", "describe", "unit_cube"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer(&dir).0),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");

    let written = catalogue(&dex, "demo");
    assert!(written.contains("[mesh.unit_cube]"), "{written}");
    assert!(written.contains("pack = \"demo\""), "{written}");
    assert!(
        !written.contains("asset = "),
        "nothing in an empty manifest names this mesh:\n{written}"
    );
}

/// **A mesh already described is left alone until it changes.** Every
/// line costs a render and a hosted model call, and a command that spent
/// both again on every run would be a command nobody runs twice.
#[test]
fn a_mesh_already_described_is_left_alone_until_forced() {
    let dir = scratch("again");
    let dex = dir.join("dex");
    let cache = dir.join("cache");
    let answering = describer(&dir);
    let tally = answering.tally.clone();
    let environment: Vec<(&str, PathBuf)> = vec![
        ("ART_MANIFEST", fixtures().join("manifest.toml")),
        ("SYNTY_STORE", fixtures().join("store")),
        ("ART_CACHE", cache),
        ("ART_DEX", dex),
        ("ART_PREVIEW", previewer(&dir).0),
        ("ART_DESCRIBER", answering.program),
    ];
    let run = |arguments: &[&str]| {
        let borrowed: Vec<(&str, &Path)> = environment
            .iter()
            .map(|(key, value)| (*key, value.as_path()))
            .collect();
        xtask(arguments, &borrowed)
    };

    let (ok, said) = run(&["art", "describe"]);
    assert!(ok, "{said}");
    assert_eq!(asked(&tally), 2, "two fixture assets, two descriptions");

    let (ok, said) = run(&["art", "describe"]);
    assert!(ok, "{said}");
    assert_eq!(asked(&tally), 2, "the second run described them again");
    assert!(said.contains("already described"), "{said}");

    let (ok, said) = run(&["art", "describe", "--force"]);
    assert!(ok, "{said}");
    assert_eq!(
        asked(&tally),
        4,
        "`--force` described nothing again:\n{said}"
    );
}

/// **A pack is the same pack on the second run as on the first.**
///
/// A pack is found under two names. In the store it is called what the
/// shop called it — `A Zipped Pack` — and in the cache, where the first
/// run left the mesh it took out of that pack's zip, it is filed under
/// the pack's id: `a_zipped_pack`. A cached copy sorts first, so the
/// second run meets the slug rather than the directory.
///
/// A pack named after the slug is a pack whose directory does not exist,
/// and nothing can be found in one of those — including the shared atlas
/// every Synty pack paints itself from. That is not a cosmetic
/// difference: the second sweep of a real pack rendered every mesh in it
/// untextured and filed descriptions of the colour of Blender's
/// missing-texture magenta.
#[test]
fn a_mesh_found_only_in_the_cache_still_knows_which_pack_it_came_out_of() {
    let dir = scratch("twice");
    let dex = dir.join("dex");
    let store = dir.join("store");
    let pack = store.join("A Zipped Pack");
    let zip = pack.join("POLYGON Fixture Pack.zip");
    std::fs::create_dir_all(&pack).expect("a scratch pack");
    std::fs::copy(
        fixtures().join("store/A Zipped Pack/POLYGON Fixture Pack.zip"),
        &zip,
    )
    .expect("the fixture pack, copied so this test can take it away again");
    let answering = describer(&dir);
    let (previewer, _) = previewer(&dir);
    let manifest = dir.join("empty.toml");
    write(&manifest, "# A manifest that declares nothing at all.\n");
    let run = || {
        xtask(
            &["art", "describe", "unit_pyramid", "--force"],
            &[
                ("ART_MANIFEST", &manifest),
                ("SYNTY_STORE", &store),
                ("ART_CACHE", &dir.join("cache")),
                ("ART_DEX", &dex),
                ("ART_PREVIEW", &previewer),
                ("ART_DESCRIBER", &answering.program),
            ],
        )
    };
    // The first run takes the mesh out of the zip and leaves it in the
    // cache, filed under the pack's id.
    let (ok, said) = run();
    assert!(ok, "{said}");

    // Then the archive goes, so the second run can only find the copy in
    // the cache — which is the whole point. Leaving the zip there would
    // make this guard depend on which of two paths sorted first, and a
    // guard that only sometimes reaches the thing it is about is worse
    // than no guard: this one passed against the defect that way.
    std::fs::remove_file(&zip).expect("the archive goes away");
    let (ok, said) = run();
    assert!(ok, "{said}");

    let handed = std::fs::read_to_string(&answering.prompts).expect("the prompts");
    assert_eq!(
        handed.matches("A Zipped Pack").count(),
        2,
        "the pack was called something else the second time round:\n{handed}"
    );
    assert!(
        !handed.contains("a_zipped_pack"),
        "a describer was told a pack is called by its own id:\n{handed}"
    );
}

/// **The variants of one thing are compared against each other.**
///
/// A description is written about one mesh, and six tenths of a real
/// library is numbered variants of something — five light panels whose
/// individual descriptions all say "low, elongated, dark grey housing
/// with a recessed pale panel" and differ in adjectives rather than in
/// fact. So a second pass looks at the whole family at once and writes
/// one line per member about what sets it apart.
///
/// The stand-in describer answers with the prompt it was handed, and the
/// comparison prompt lists each member as `<name>: <facts>` — the same
/// shape the answer is asked for. So the echo comes back parseable, and
/// what lands in `differs` proves the member's own measurements were in
/// the prompt and that the answer was filed against the right sibling.
#[test]
fn the_variants_of_one_thing_are_told_apart_from_each_other() {
    let dir = scratch("family");
    let dex = dir.join("dex");
    let store = dir.join("store");
    let pack = store.join("A Family Pack/SourceFiles/OBJ");
    std::fs::create_dir_all(&pack).expect("a scratch pack");
    // Two meshes of one name, which is what makes them a family — and
    // two meshes that are not the same bytes, so each has a description
    // of its own for the comparison to be filed against. Identical copies
    // are one mesh, which is the guard below this one.
    let cube = std::fs::read_to_string(fixtures().join("store/demo/SourceFiles/OBJ/unit_cube.obj"))
        .expect("the fixture cube");
    write(&pack.join("SM_Test_Panel_01.obj"), &cube);
    write(
        &pack.join("SM_Test_Panel_02.obj"),
        &format!("{cube}# a second variant, which differs in its bytes\n"),
    );
    let answering = describer(&dir);
    let (previewer, _) = previewer(&dir);
    let manifest = dir.join("empty.toml");
    write(&manifest, "# A manifest that declares nothing at all.\n");
    let (ok, said) = xtask(
        &["art", "describe", "SM_Test_Panel"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");
    assert!(said.contains("told from its siblings"), "{said}");

    let written = catalogue(&dex, "a_family_pack");
    let differs: Vec<&str> = written
        .lines()
        .filter_map(|line| line.strip_prefix("differs = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .collect();
    assert_eq!(
        differs.len(),
        2,
        "not every variant was told apart:\n{written}"
    );

    // **Each line is that member's own, not its neighbour's.** The
    // stand-in echoes the prompt, whose per-member line carries that
    // member's own name and description — so a line filed against the
    // wrong sibling shows up here as a line naming the other one.
    for (variant, line) in (1..=2).zip(&differs) {
        assert!(
            line.contains(&format!("SM_Test_Panel_0{variant}")),
            "variant {variant} was given a line about another one:\n{line}"
        );
    }

    // And a run over the same meshes again neither re-compares them nor
    // throws away what the comparison already cost.
    let asked_once = asked(&answering.tally);
    let (ok, said) = xtask(
        &["art", "describe", "SM_Test_Panel"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");
    assert_eq!(
        asked(&answering.tally),
        asked_once,
        "it asked again:\n{said}"
    );
    assert_eq!(
        catalogue(&dex, "a_family_pack")
            .lines()
            .filter(|line| line.starts_with("differs = "))
            .count(),
        2,
        "the comparisons did not survive a second run"
    );
}

/// **One mesh under two names is looked at once, and both names get the
/// same answer.**
///
/// A pack ships the same geometry twice under different names about one
/// time in a hundred, and everything here is addressed by the digest of
/// that geometry — the picture, the prompt beside it, the answer. Two
/// names racing over one set of those files put a description in the
/// catalogue under a name it was not written about; it was caught by
/// three copies of one mesh, where the third came back describing the
/// second.
///
/// Looking once is also the truer answer: describing one mesh twice
/// produces two sentences that differ in adjectives and not in fact,
/// which is the very thing the family pass exists to stop.
#[test]
fn one_mesh_under_two_names_is_looked_at_once() {
    let dir = scratch("twins");
    let dex = dir.join("dex");
    let store = dir.join("store");
    let pack = store.join("A Twin Pack/SourceFiles/OBJ");
    std::fs::create_dir_all(&pack).expect("a scratch pack");
    // Two names, one set of bytes. Not `_01`/`_02`, so that no family
    // comparison runs and the tally below counts only the descriptions.
    for name in ["SM_Twin_First", "SM_Twin_Second"] {
        std::fs::copy(
            fixtures().join("store/demo/SourceFiles/OBJ/unit_cube.obj"),
            pack.join(format!("{name}.obj")),
        )
        .expect("a twin");
    }
    let answering = describer(&dir);
    let (previewer, launches) = previewer(&dir);
    let manifest = dir.join("empty.toml");
    write(&manifest, "# A manifest that declares nothing at all.\n");
    let (ok, said) = xtask(
        &["art", "describe", "SM_Twin"],
        &[
            ("ART_MANIFEST", &manifest),
            ("SYNTY_STORE", &store),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");
    assert_eq!(asked(&answering.tally), 1, "one mesh, two calls:\n{said}");
    assert_eq!(asked(&launches), 1, "one mesh, two renders:\n{said}");

    let written = catalogue(&dex, "a_twin_pack");
    assert!(written.contains("[mesh.sm_twin_first]"), "{written}");
    assert!(written.contains("[mesh.sm_twin_second]"), "{written}");
    let descriptions: Vec<&str> = written
        .lines()
        .filter(|line| line.starts_with("description = "))
        .collect();
    assert_eq!(descriptions.len(), 2, "{written}");
    assert_eq!(
        descriptions[0], descriptions[1],
        "the same mesh was described two different ways"
    );
}

/// **A chunk of meshes costs one launch, not one launch each.**
///
/// The whole reason the previewer's contract is a file of jobs rather
/// than a mesh per call. Blender costs about 2.2 seconds to start before
/// it has looked at anything, against about 5 seconds of work, so a
/// launch per mesh spent a third of a library on starting up — thirty
/// hours over fifty thousand meshes. `--jobs 1` puts both fixture assets
/// in one chunk, and the stand-in previewer writes a line each time it is
/// run.
#[test]
fn a_chunk_of_meshes_is_looked_at_in_one_launch() {
    let dir = scratch("batched");
    let dex = dir.join("dex");
    let answering = describer(&dir);
    let (previewer, launches) = previewer(&dir);
    let (ok, said) = xtask(
        &["art", "describe", "--jobs", "1"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");
    assert_eq!(asked(&launches), 1, "two meshes, two launches:\n{said}");
    assert_eq!(asked(&answering.tally), 2, "and still two descriptions");
    // Both of them landed, in the two catalogues they belong to, which
    // is also the path where one chunk writes more than one file.
    assert!(catalogue(&dex, "demo").contains("[mesh.unit_cube]"));
    assert!(catalogue(&dex, "zipped").contains("[mesh.unit_pyramid]"));
}

/// **The catalogue is searched by what it says.** A file name is the
/// thing that was not searchable in the first place; the search somebody
/// actually has is a word out of the description, and answering it is the
/// whole return on having written one.
#[test]
fn the_catalogue_is_searched_by_what_it_says_and_not_only_by_what_things_are_called() {
    let dir = scratch("searched");
    let dex = dir.join("dex");
    let answering = describer(&dir);
    let (ok, said) = xtask(
        &["art", "describe"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer(&dir).0),
            ("ART_DESCRIBER", &answering.program),
        ],
    );
    assert!(ok, "{said}");

    // `checker` is nowhere in either file name; it is in what the
    // describer said about them.
    let (ok, found) = xtask(&["art", "dex", "checker"], &[("ART_DEX", &dex)]);
    assert!(ok, "{found}");
    assert!(found.contains("unit_cube"), "{found}");
    assert!(found.contains("unit_pyramid"), "{found}");

    let (ok, empty) = xtask(&["art", "dex", "helicopter"], &[("ART_DEX", &dex)]);
    assert!(ok, "{empty}");
    assert!(empty.contains("nothing"), "{empty}");
}

/// **A describer that answers nothing writes nothing.** An empty
/// description in a catalogue is worse than an absent line: it reads as a
/// mesh nobody could say anything about, and the next run will skip it as
/// already described.
#[test]
fn a_describer_that_answers_nothing_leaves_no_line_behind() {
    let dir = scratch("silent");
    let dex = dir.join("dex");
    let silent = stand_in(&dir, "silent", "exit 1\n", "exit /b 1\r\n");
    let (ok, said) = xtask(
        &["art", "describe"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_CACHE", &dir.join("cache")),
            ("ART_DEX", &dex),
            ("ART_PREVIEW", &previewer(&dir).0),
            ("ART_DESCRIBER", &silent),
        ],
    );
    assert!(
        !ok,
        "a run that described nothing reported success:\n{said}"
    );
    assert!(said.contains("could not be described"), "{said}");
    assert!(
        !dex.join("demo.toml").exists(),
        "a catalogue was written for meshes nothing described"
    );
}

/// **An option nobody has is a refusal.** The same rule the manifest
/// dialect follows: a misspelled `--limt 200` that was quietly ignored is
/// a run that describes twenty-four meshes for somebody who asked for two
/// hundred, and finds out when the catalogue is short.
#[test]
fn an_option_this_command_does_not_have_is_refused_rather_than_ignored() {
    let dir = scratch("mistyped");
    let (ok, said) = xtask(
        &["art", "describe", "--limt", "200"],
        &[
            ("ART_MANIFEST", &fixtures().join("manifest.toml")),
            ("SYNTY_STORE", &fixtures().join("store")),
            ("ART_DEX", &dir.join("dex")),
        ],
    );
    assert!(!ok, "{said}");
    assert!(said.contains("--limt"), "{said}");
    assert!(said.contains("--limit"), "the usage is not shown:\n{said}");
}
