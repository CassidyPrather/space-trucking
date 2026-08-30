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

/// A previewer: it copies the mesh to where the picture goes, so there is
/// a non-empty file there, and prints the facts a real one measures.
fn previewer(dir: &Path) -> PathBuf {
    const FACTS: [&str; 5] = [
        "tris 12",
        "meshes 1",
        "materials 1",
        "image checker.png",
        "aabb -0.5 0 -0.25 0.5 1 0.25",
    ];
    let said = |before: &str, after: &str| -> String {
        FACTS.iter().fold(String::new(), |mut script, fact| {
            script.push_str(before);
            script.push_str(fact);
            script.push_str(after);
            script
        })
    };
    stand_in(
        dir,
        "preview",
        &format!("cp \"$1\" \"$2\"\n{}", said("printf '%s\\n' '", "'\n")),
        &format!("copy /y \"%~1\" \"%~2\" >nul\r\n{}", said("echo ", "\r\n")),
    )
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
            ("ART_PREVIEW", &previewer(&dir)),
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
            ("ART_PREVIEW", &previewer(&dir)),
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
            ("ART_PREVIEW", &previewer(&dir)),
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
        ("ART_PREVIEW", previewer(&dir)),
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
            ("ART_PREVIEW", &previewer(&dir)),
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
            ("ART_PREVIEW", &previewer(&dir)),
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
