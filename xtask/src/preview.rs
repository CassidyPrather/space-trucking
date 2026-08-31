//! A picture of one mesh, and the numbers beside it.
//!
//! The describer next door writes English about an asset, and the only
//! honest way to write English about a mesh is to look at one. Nothing in
//! this repository can open an FBX — that is why [`crate::convert`]
//! exists — so looking is somebody else's program too, and it is the same
//! program: `blender --background --python`, pointed at a second script.
//!
//! What comes back is a PNG and five facts. The PNG is four views of the
//! mesh in one image, because a model shown one three-quarter view of a
//! crate cannot say what is on the back of it, and a strip of four costs
//! the same render. The facts — triangles, meshes, materials, the images
//! it actually bore, and the box round it — are measured while the scene
//! is open, because that is the one moment anything in this pipeline can
//! see a mesh, and they are worth having with or without a description:
//! "which of these forty crates is under a thousand triangles" is a
//! question the catalogue can answer on its own.
//!
//! ## The contract, and why it is a file of jobs
//!
//! ```text
//! <program> <jobs file>
//! ```
//!
//! where the file holds one job per line:
//!
//! ```text
//! <source>|<destination.png>|<texture>
//! ```
//!
//! **One launch, many meshes.** The contract was one mesh per call, the
//! shape the converter's has, and it was measured to be the wrong one
//! here: Blender takes about 2.2 seconds to start and import its own
//! Python before looking at anything, against about 5 seconds of actual
//! work, so nearly a third of describing a library went on starting up —
//! thirty hours of it over fifty thousand meshes. A converter runs once
//! per asset a manifest names, which is a handful; a previewer runs once
//! per mesh in a pack, which is thousands. Same shape, different scale,
//! different contract.
//!
//! `|` separates the fields because Windows forbids it in a path and a
//! shell stand-in can split on it in one line — which matters, because
//! `$ART_PREVIEW` is a seam a person is expected to write a program for.
//! The texture may be empty, and it means what it means to the converter:
//! the atlas to paint with.
//!
//! What comes back is one block per job, headed by the job's line number:
//!
//! ```text
//! look 1
//! tris 412
//! meshes 1
//! materials 1
//! image PolygonSciFiSpace_Texture_01_A.png
//! aabb <min x> <min y> <min z> <max x> <max y> <max z>
//! trouble 2 SM_Broken.fbx imported without producing a single mesh
//! ```
//!
//! A program that prints no facts for a job is not in breach — it has
//! left the catalogue an entry with a picture and no numbers. A program
//! that says nothing about a job at all has failed that job, and `trouble
//! <n> <why>` is how it says which and why without ending the launch.
//! `aabb` is the converter's own line, in the same axes, so one reader
//! serves both.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cache::Cache;
use crate::convert::{self, Lookup};
use crate::fsx;
use crate::manifest::Bounds;

/// The script, carried in the binary. See `xtask/blender/fbx_to_preview.py`.
pub const SCRIPT: &str = include_str!("../blender/fbx_to_preview.py");

/// **What one look at a mesh turned up.**
///
/// Every field is optional in the sense that a conforming program may
/// print none of them; a zero here means "nothing said so", which is why
/// the catalogue prints the counts it was given rather than deriving
/// anything from them.
#[derive(Default, Debug)]
pub struct Look {
    pub triangles: u64,
    pub meshes: u64,
    pub materials: u64,
    /// The images the scene actually bore, in the order they were named.
    /// Empty is the mesh that rendered untextured, and it is the first
    /// thing to know before believing a description of its colours.
    pub images: Vec<String>,
    /// **What the mesh asked for and did not get**, by file name.
    ///
    /// A pack's shared atlas is not the only texture in a pack: ivy,
    /// decals and screens carry their own, and a mesh painted with the
    /// atlas instead has its UVs land wherever those coordinates happen
    /// to fall — which is how a sheet of ivy was catalogued as "a jagged,
    /// faceted shard of near-black material". These are the names to take
    /// out of the pack before looking again.
    pub wants: Vec<String>,
    pub bounds: Option<Bounds>,
}

impl Look {
    /// How big the mesh is across each axis, in the file's own units.
    /// The catalogue records the size rather than the half-extents the
    /// manifest works in, because "how big is this crate" is the question
    /// somebody choosing between forty of them is asking.
    pub fn size(&self) -> [f32; 3] {
        self.bounds
            .map_or([0.0; 3], |bounds| bounds.half.map(|half| half * 2.0))
    }
}

#[derive(Debug)]
pub enum Previewer {
    /// `$ART_PREVIEW`, run as `<program> <jobs file>`.
    Program(PathBuf),
    Blender(PathBuf),
}

impl Previewer {
    pub fn describe(&self) -> String {
        match self {
            Self::Program(path) => format!("$ART_PREVIEW {}", path.display()),
            Self::Blender(path) => format!("blender {}", path.display()),
        }
    }

    /// **Look at a chunk of meshes in one launch, and answer for every
    /// one of them.**
    ///
    /// The answer is one outcome per job, in the order the jobs were
    /// given, so a caller can zip it against what it asked for. A job the
    /// program said nothing about is an `Err` rather than an empty
    /// `Look`: a describer handed a picture that is not there would spend
    /// a model call on nothing.
    ///
    /// A program that exits nonzero fails the whole chunk — every job in
    /// it gets the same complaint, because there is nothing to say which
    /// of them was the reason. That is why the script this ships prints
    /// `trouble <n>` and keeps going instead of exiting.
    pub fn run(&self, script: &Path, cache: &Cache, jobs: &[Job<'_>]) -> Vec<Result<Look, String>> {
        match self.attempt(script, cache, jobs) {
            Ok(looks) => looks,
            Err(complaint) => jobs.iter().map(|_| Err(complaint.clone())).collect(),
        }
    }

    fn attempt(
        &self,
        script: &Path,
        cache: &Cache,
        jobs: &[Job<'_>],
    ) -> Result<Vec<Result<Look, String>>, String> {
        for job in jobs {
            if let Some(parent) = job.destination.parent() {
                fsx::create_dir_all(parent)?;
            }
        }
        let listing = cache.dex_jobs(jobs.first().map_or("empty", |job| job.digest));
        fsx::write(&listing, &render_jobs(jobs))?;
        let output = self
            .command(script, &listing)
            .output()
            .map_err(|err| format!("cannot run {}: {err}", self.describe()))?;
        if !output.status.success() {
            return Err(format!(
                "{} could not look at this chunk of {} meshes\n{}",
                self.describe(),
                jobs.len(),
                indent(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        let mut looks = read(&String::from_utf8_lossy(&output.stdout), jobs.len());
        for (job, look) in jobs.iter().zip(&mut looks) {
            if look.is_ok() && std::fs::metadata(job.destination).map_or(0, |meta| meta.len()) == 0
            {
                *look = Err(format!(
                    "{} said it had looked at {} and wrote no picture to {}",
                    self.describe(),
                    job.source.display(),
                    job.destination.display()
                ));
            }
        }
        Ok(looks)
    }

    /// **Both Blender scripts, written out beside the cache, and the one
    /// to run.** The preview script imports the converter's, because the
    /// question "does this material know its own texture?" has exactly
    /// one right answer in this pipeline and two copies of it would drift
    /// — so both files have to be in the same directory before either
    /// runs.
    ///
    /// Done once, before anything is rendered, and not inside [`run`]:
    /// a describe run looks at several meshes at once, and four threads
    /// truncating and rewriting the file a fifth is importing is a
    /// Blender reading half a script.
    ///
    /// [`run`]: Self::run
    pub fn prepare(&self, cache: &Cache) -> Result<PathBuf, String> {
        match self {
            Self::Program(_) => Ok(PathBuf::new()),
            Self::Blender(_) => {
                fsx::write(&cache.blender_script(), convert::SCRIPT)?;
                let script = cache.preview_script();
                fsx::write(&script, SCRIPT)?;
                Ok(script)
            }
        }
    }

    /// The exact command line, built and not run — split out for the
    /// guard, for the reason [`crate::convert::Converter::command`] is:
    /// the Blender branch is the one every real run takes and the one no
    /// test can execute.
    fn command(&self, script: &Path, jobs: &Path) -> Command {
        match self {
            Self::Program(program) => {
                let mut command = Command::new(program);
                command.arg(jobs);
                command
            }
            Self::Blender(blender) => {
                let mut command = Command::new(blender);
                command
                    .arg("--background")
                    .arg("--factory-startup")
                    .arg("--python-exit-code")
                    .arg("1")
                    .arg("--python")
                    .arg(script)
                    .arg("--")
                    .arg(jobs);
                command
            }
        }
    }
}

/// One mesh to look at: where it is, where its picture goes, and what to
/// paint it with.
pub struct Job<'a> {
    pub source: &'a Path,
    pub destination: &'a Path,
    pub texture: Option<&'a Path>,
    /// The source mesh's digest, which is what the jobs file for this
    /// chunk is named after — so two chunks running at once cannot write
    /// over each other's list.
    pub digest: &'a str,
}

/// The jobs file: one `source|destination|texture` per line.
fn render_jobs(jobs: &[Job<'_>]) -> String {
    let mut text = String::new();
    for job in jobs {
        text.push_str(&job.source.display().to_string());
        text.push('|');
        text.push_str(&job.destination.display().to_string());
        text.push('|');
        if let Some(texture) = job.texture {
            text.push_str(&texture.display().to_string());
        }
        text.push('\n');
    }
    text
}

/// **Find something that can look at a mesh, or say what to install.**
///
/// `$ART_CONVERTER` is deliberately not consulted. A converter is a
/// program that writes glTF, and asking one for a PNG would run it, get
/// an exit code of zero and a file that is not a picture, and hand the
/// result to a vision model.
pub fn find() -> Result<Previewer, String> {
    choose(&Lookup::from_env(), std::env::var_os("ART_PREVIEW"))
}

pub fn choose(lookup: &Lookup, named: Option<std::ffi::OsString>) -> Result<Previewer, String> {
    if let Some(program) = named.map(PathBuf::from) {
        if program.components().count() > 1 && !program.is_file() {
            return Err(format!(
                "$ART_PREVIEW is {}, and there is no program there",
                program.display()
            ));
        }
        return Ok(Previewer::Program(program));
    }
    if let Some(blender) = &lookup.blender {
        if !blender.is_file() {
            return Err(format!(
                "$BLENDER is {}, and there is no program there",
                blender.display()
            ));
        }
        return Ok(Previewer::Blender(blender.clone()));
    }
    lookup
        .on_path
        .clone()
        .or_else(|| lookup.installed.iter().find(|path| path.is_file()).cloned())
        .map(Previewer::Blender)
        .ok_or_else(|| NOTHING_TO_LOOK_WITH.to_owned())
}

const NOTHING_TO_LOOK_WITH: &str = "\
nothing here can look at a mesh: Blender is not on PATH and $BLENDER is not set.

  Describing an asset means rendering it and showing the picture to a model, and
  the picture has to come from something that can open an FBX. Pick one:

    Blender          https://www.blender.org/download/ — the default, and the same
                     install `cargo xtask art resolve` converts with.
    $ART_PREVIEW     any program run as `<program> <source> <destination.png>
                     [texture]`, which may print `tris`, `meshes`, `materials`,
                     `image` and `aabb` lines for the catalogue.

  A catalogue written without either would be file names and nothing else, which is
  the state this command exists to get out of — so it refuses rather than writing
  one. `cargo xtask art find` searches those names meanwhile.";

/// **Read what a program said about a chunk of meshes**, as one outcome
/// per job.
///
/// A `look <n>` line opens the block for job `n`; everything after it
/// belongs to that job until the next `look` or `trouble`. Lines before
/// any block, and lines nobody here has a meaning for, are dropped on the
/// floor: Blender prints a banner of its own before the script runs and
/// an add-on may print whatever it likes.
///
/// A job with no block at all comes back as an error rather than as an
/// empty `Look`, because those are two quite different things — one is a
/// mesh with no triangle count and the other is a mesh nothing looked at.
fn read(said: &str, jobs: usize) -> Vec<Result<Look, String>> {
    let mut looks: Vec<Result<Look, String>> = (0..jobs)
        .map(|_| Err(String::from("the previewer said nothing about it")))
        .collect();
    let mut at: Option<usize> = None;
    for line in said.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(' ') else {
            continue;
        };
        let rest = rest.trim();
        // `look` and `trouble` are the two that move the reader on; the
        // number is one-based, because it is the line of the jobs file.
        if key == "look" || key == "trouble" {
            let (number, why) = rest.split_once(' ').unwrap_or((rest, ""));
            let Some(index) = number.parse::<usize>().ok().filter(|n| *n >= 1) else {
                at = None;
                continue;
            };
            at = looks.get(index - 1).map(|_| index - 1);
            if let Some(index) = at {
                looks[index] = if key == "look" {
                    Ok(Look::default())
                } else {
                    Err(if why.is_empty() {
                        String::from("the previewer refused it and did not say why")
                    } else {
                        why.to_owned()
                    })
                };
            }
            continue;
        }
        let Some(look) = at.and_then(|index| looks[index].as_mut().ok()) else {
            continue;
        };
        match key {
            "tris" => look.triangles = rest.parse().unwrap_or(look.triangles),
            "meshes" => look.meshes = rest.parse().unwrap_or(look.meshes),
            "materials" => look.materials = rest.parse().unwrap_or(look.materials),
            "image" => {
                if let Some(name) = a_file_name(rest)
                    && !look.images.iter().any(|had| had == &name)
                {
                    look.images.push(name);
                }
            }
            "wants" => {
                if let Some(name) = a_file_name(rest)
                    && !look.wants.iter().any(|had| had == &name)
                {
                    look.wants.push(name);
                }
            }
            "aabb" => look.bounds = bounds(rest).or(look.bounds),
            _ => {}
        }
    }
    looks
}

/// **A file name, or nothing.**
///
/// The value on an `image` or `wants` line is supposed to be one file's
/// name, and what arrived instead was
/// `PolygonSciFiHorror_01_A.pngFra:1 Mem:2.4M | Saved: 'C:\...'` —
/// Blender's renderer writing over the top of the script's own line. That
/// went into the catalogue as a texture's name, and the backslash in it
/// is a character this dialect's strings may not hold, so the whole file
/// was refused and a run's worth of work stayed unwritten.
///
/// The script no longer lets that happen. This is the other half of not
/// letting it happen: what the reader will accept is what a file name can
/// be — one line, no separators, nothing enormous.
fn a_file_name(rest: &str) -> Option<String> {
    let name = rest.split(['\r', '\n']).next()?.trim();
    let sound = !name.is_empty()
        && name.len() <= 120
        && !name.contains(['\\', '/', '\'', '"', '|'])
        && !name.chars().any(char::is_control);
    sound.then(|| name.to_owned())
}

/// The six numbers of an `aabb` line, as a middle and a half. The same
/// reading [`crate::convert`] does of the same line; it is a few lines
/// rather than a shared function because the converter reads it out of a
/// run that also wrote a file and this reads it out of a run that wrote a
/// picture, and neither wants the other's error handling.
fn bounds(rest: &str) -> Option<Bounds> {
    let numbers: Vec<f32> = rest
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    let box3 = <[f32; 6]>::try_from(numbers.as_slice()).ok()?;
    if !box3.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(Bounds {
        mid: [0, 1, 2].map(|axis| f32::midpoint(box3[axis], box3[axis + 3])),
        half: [0, 1, 2].map(|axis| (box3[axis + 3] - box3[axis]) * 0.5),
    })
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing_anywhere() -> Lookup {
        Lookup {
            converter: None,
            blender: None,
            on_path: None,
            installed: Vec::new(),
        }
    }

    /// **A machine that cannot look at a mesh is told so, and told what
    /// a catalogue written anyway would be worth.** The command spends a
    /// hosted model call per asset; doing that with no picture to show
    /// would buy four hundred sentences written from a file name, which
    /// is the thing this whole command exists to replace.
    #[test]
    fn a_machine_with_no_blender_is_told_what_to_install_and_why_it_refuses() {
        let complaint = choose(&nothing_anywhere(), None).expect_err("nothing to look with");
        for wanted in ["Blender", "blender.org", "$ART_PREVIEW", "file names"] {
            assert!(complaint.contains(wanted), "no `{wanted}` in:\n{complaint}");
        }
    }

    /// **A converter is not a previewer.** `$ART_CONVERTER` is set on
    /// every machine that has ever pointed this pipeline at something
    /// other than Blender, and a program that writes glTF handed a `.png`
    /// to write would exit zero having written a glTF — which is a file a
    /// vision model would be shown.
    #[test]
    fn the_converter_override_is_not_mistaken_for_a_previewer() {
        let complaint = choose(
            &Lookup {
                converter: Some(PathBuf::from("fbx2gltf")),
                ..nothing_anywhere()
            },
            None,
        )
        .expect_err("a converter cannot draw a picture");
        assert!(complaint.contains("$ART_PREVIEW"), "{complaint}");
    }

    /// **What a program says about a chunk is read the way it is
    /// written.** Blender prints a banner of its own before the script
    /// runs, an add-on may print anything, and a conforming program may
    /// print no facts at all — none of which may become a triangle count,
    /// and none of which may be filed against the wrong mesh.
    // The size of a mesh nothing measured is exactly nothing, and that
    // is the claim: a zero here is what the catalogue prints, so it is
    // asked for exactly rather than approximately.
    #[allow(clippy::float_cmp)]
    #[test]
    fn the_facts_are_read_out_of_whatever_else_a_program_printed() {
        let looks = read(
            "Blender 5.0.0 (hash 1a2b3c)\n\
             Read prefs: /home/you/.config/blender\n\
             look 1\n\
             tris 412\n\
             meshes 2\n\
             materials 1\n\
             image PolygonSciFiSpace_Texture_01_A.png\n\
             image PolygonSciFiSpace_Texture_01_A.png\n\
             aabb -0.363 0.0 -0.336 0.363 0.613 0.336\n\
             Fra:1 Mem:12.34M | Rendering 1 / 16 samples\n\
             look 3\n\
             tris 7\n",
            3,
        );
        let first = looks[0].as_ref().expect("the first block");
        assert_eq!(first.triangles, 412);
        assert_eq!(first.meshes, 2);
        assert_eq!(first.materials, 1);
        assert_eq!(
            first.images,
            ["PolygonSciFiSpace_Texture_01_A.png"],
            "one image named twice is one image"
        );
        assert!(
            (first.size()[1] - 0.613).abs() < 0.001,
            "{:?}",
            first.size()
        );

        // The mesh in the middle, which the program never mentioned. It
        // is an error and not an empty answer: nothing looked at it, and
        // a describer handed its missing picture would spend a model
        // call on nothing.
        assert!(looks[1].is_err(), "a mesh nothing said anything about");

        // And the facts after the second header belong to the mesh that
        // header named, not to the one before it.
        assert_eq!(looks[2].as_ref().expect("the third block").triangles, 7);

        let silent = read("some program that has never heard of this repository\n", 2);
        assert!(silent.iter().all(Result::is_err), "{silent:?}");
    }

    /// **A mesh a launch refused is that mesh's complaint, not the
    /// chunk's.** A chunk is up to thirty-two meshes through one Blender,
    /// and one unreadable FBX among them must cost one line of the
    /// catalogue rather than thirty-two.
    #[test]
    fn one_mesh_a_launch_refused_does_not_take_the_chunk_with_it() {
        let looks = read(
            "look 1\ntris 12\n\
             trouble 2 SM_Broken.fbx imported without producing a single mesh\n\
             look 3\ntris 9\n",
            3,
        );
        assert!(looks[0].is_ok());
        assert_eq!(
            looks[1].as_ref().expect_err("the middle one was refused"),
            "SM_Broken.fbx imported without producing a single mesh"
        );
        assert_eq!(looks[2].as_ref().expect("the last one").triangles, 9);
    }

    /// **The jobs file is one line per mesh, and an absent atlas is an
    /// absent field.** It is read by a shell stand-in as well as by
    /// Blender, so the shape has to stay something `IFS='|' read` and
    /// `for /f "delims=|"` both handle.
    #[test]
    fn the_jobs_file_is_one_line_per_mesh() {
        let text = render_jobs(&[
            Job {
                source: Path::new("/store/SM_Crate.fbx"),
                destination: Path::new("/cache/dex/aa/preview.png"),
                texture: Some(Path::new("/store/atlas.png")),
                digest: "aa",
            },
            Job {
                source: Path::new("/store/SM_Bare.fbx"),
                destination: Path::new("/cache/dex/bb/preview.png"),
                texture: None,
                digest: "bb",
            },
        ]);
        assert_eq!(
            text,
            "/store/SM_Crate.fbx|/cache/dex/aa/preview.png|/store/atlas.png\n\
             /store/SM_Bare.fbx|/cache/dex/bb/preview.png|\n"
        );
    }

    /// **The Blender command line carries the script and the jobs
    /// file.** Same reasoning as the converter's guard: this branch is
    /// the one every real run takes and the one no test can run, so the
    /// arguments are asserted directly.
    #[test]
    fn the_blender_command_line_carries_the_script_and_the_jobs() {
        let blender = Previewer::Blender(PathBuf::from("blender"));
        let arguments = |command: &Command| -> Vec<String> {
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            arguments(&blender.command(
                Path::new("/cache/blender/fbx_to_preview.py"),
                Path::new("/cache/dex/abc/jobs.txt"),
            )),
            [
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
                "/cache/blender/fbx_to_preview.py",
                "--",
                "/cache/dex/abc/jobs.txt",
            ]
        );
        // And a program of somebody's own is handed the same one file,
        // which is the whole of the contract it has to know.
        assert_eq!(
            arguments(
                &Previewer::Program(PathBuf::from("render.sh"))
                    .command(Path::new("unused"), Path::new("/cache/dex/abc/jobs.txt"),)
            ),
            ["/cache/dex/abc/jobs.txt"]
        );
    }

    /// **The script this binary carries is the one that shares the
    /// converter's answer about textures.** Two copies of `usable_image`
    /// is how a preview comes out grey while the conversion beside it
    /// comes out painted, and the catalogue then describes a colourless
    /// mesh in a pack whose whole personality is its atlas.
    #[test]
    fn the_preview_script_borrows_the_converters_answer_rather_than_copying_it() {
        for wanted in ["import fbx_to_gltf", "fbx_to_gltf.paint_with", "def render"] {
            assert!(
                SCRIPT.contains(wanted),
                "no `{wanted}` in the compiled-in preview script"
            );
        }
        assert!(
            convert::SCRIPT.contains("if __name__ ==") && convert::SCRIPT.contains("def bounds"),
            "the converter script is not importable, so the preview cannot borrow from it"
        );
    }

    /// **A path this side can read is one the other side can open.**
    ///
    /// Rust's standard library reaches past Windows' 260-character limit
    /// without being asked and Blender's Python does not, so a mesh deep
    /// in the cache is found, hashed, written into a jobs file and handed
    /// over — and then answered for with `no such source file`. That was
    /// a quarter of a real sweep, and every one of them read like a
    /// broken pack rather than a spelling.
    ///
    /// Asked of the text because the rule only does anything on Windows
    /// and only past 240 characters, which is a state no unit test on
    /// another platform can put the script in. What it pins is that the
    /// rule is in the shared script and that both readers of a path go
    /// through it.
    #[test]
    fn a_path_too_long_for_windows_is_spelled_so_blender_can_open_it() {
        assert!(
            convert::SCRIPT.contains("def openable"),
            "the shared script has no long-path rule"
        );
        for (script, what) in [
            (convert::SCRIPT, "the converter"),
            (SCRIPT, "the previewer"),
        ] {
            assert!(
                script.contains("openable(path)") || script.contains("openable(source)"),
                "{what} reads a path without going through the long-path rule"
            );
        }
    }

    /// **The mesh is measured before the turntable copies it.**
    ///
    /// Four views of one mesh are four objects standing a metre apart in
    /// one scene, and the box round that scene is not the size of
    /// anything. The first real crate through this pipeline went into the
    /// catalogue at 1.84 units across, being 0.73 — a number that looks
    /// entirely plausible beside a description, which is what makes it
    /// worth a guard.
    ///
    /// Asked of the script's text, because the scene it is about only
    /// exists inside a Blender, and there is none here. It is the order
    /// of two calls, which is exactly the thing that went wrong.
    #[test]
    fn the_size_in_the_catalogue_is_measured_before_anything_is_duplicated() {
        // The last of each, because the first `turntable(centre` in the
        // file is the `def` line rather than the call.
        let measured = SCRIPT
            .rfind("fbx_to_gltf.report_bounds()")
            .expect("the preview script measures the mesh");
        let copied = SCRIPT
            .rfind("turntable(centre")
            .expect("the preview script lays out four views");
        assert!(
            measured < copied,
            "the mesh is measured after it is copied, so the size is the grid's"
        );
    }
}
