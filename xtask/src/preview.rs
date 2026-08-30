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
//! ## The contract
//!
//! ```text
//! <program> <source> <destination.png> [texture]
//! ```
//!
//! The same shape as the converter's, deliberately, and the third
//! argument means the same thing: the atlas to paint with. What it prints
//! is one fact per line, and a program that prints none is not in breach —
//! it has simply left the catalogue with an entry that has a picture and
//! no numbers:
//!
//! ```text
//! tris 412
//! meshes 1
//! materials 1
//! image PolygonSciFiSpace_Texture_01_A.png
//! aabb <min x> <min y> <min z> <max x> <max y> <max z>
//! ```
//!
//! `aabb` is the converter's own line, in the same axes, so one reader
//! serves both. `$ART_PREVIEW` overrides the Blender route with any
//! program of that shape, which is what lets the whole command be
//! exercised on a machine with no Blender on it.

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
#[derive(Default)]
pub struct Look {
    pub triangles: u64,
    pub meshes: u64,
    pub materials: u64,
    /// The images the scene actually bore, in the order they were named.
    /// Empty is the mesh that rendered untextured, and it is the first
    /// thing to know before believing a description of its colours.
    pub images: Vec<String>,
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
    /// `$ART_PREVIEW`, run as `<program> <source> <destination> [texture]`.
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

    /// Render one mesh and read back what the program said about it.
    ///
    /// A program that exits nonzero is a complaint naming the mesh, and
    /// so is one that exits cleanly and writes no picture: a describer
    /// handed a file that is not there would otherwise go and spend a
    /// model call on nothing.
    pub fn run(
        &self,
        script: &Path,
        source: &Path,
        destination: &Path,
        texture: Option<&Path>,
    ) -> Result<Look, String> {
        if let Some(parent) = destination.parent() {
            fsx::create_dir_all(parent)?;
        }
        let mut command = self.command(script, source, destination, texture);
        let output = command
            .output()
            .map_err(|err| format!("cannot run {}: {err}", self.describe()))?;
        if !output.status.success() {
            return Err(format!(
                "{} could not look at {}\n{}",
                self.describe(),
                source.display(),
                indent(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        if std::fs::metadata(destination).map_or(0, |meta| meta.len()) == 0 {
            return Err(format!(
                "{} exited cleanly and wrote no picture to {}",
                self.describe(),
                destination.display()
            ));
        }
        Ok(read(&String::from_utf8_lossy(&output.stdout)))
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
    /// test can execute, and an argument dropped from it alone looks
    /// exactly like a texture that was staged and ignored.
    fn command(
        &self,
        script: &Path,
        source: &Path,
        destination: &Path,
        texture: Option<&Path>,
    ) -> Command {
        let mut command = match self {
            Self::Program(program) => {
                let mut command = Command::new(program);
                command.arg(source).arg(destination);
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
                    .arg(source)
                    .arg(destination);
                command
            }
        };
        if let Some(texture) = texture {
            command.arg(texture);
        }
        command
    }
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

/// Read what a program said about a mesh. Unknown lines are ignored and
/// the last of each kind wins, because Blender prints a banner of its own
/// and an add-on may print anything it likes — the lines that matter are
/// the ones our script wrote last.
fn read(said: &str) -> Look {
    let mut look = Look::default();
    for line in said.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(' ') else {
            continue;
        };
        let rest = rest.trim();
        match key {
            "tris" => look.triangles = rest.parse().unwrap_or(look.triangles),
            "meshes" => look.meshes = rest.parse().unwrap_or(look.meshes),
            "materials" => look.materials = rest.parse().unwrap_or(look.materials),
            "image" => {
                if !rest.is_empty() && !look.images.iter().any(|had| had == rest) {
                    look.images.push(rest.to_owned());
                }
            }
            "aabb" => look.bounds = bounds(rest).or(look.bounds),
            _ => {}
        }
    }
    look
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

    /// **What a program says about a mesh is read the way it is
    /// written.** Blender prints a banner of its own before the script
    /// runs, an add-on may print anything, and a conforming program may
    /// print nothing at all — none of which may become a triangle count.
    // The size of a mesh nothing measured is exactly nothing, and that
    // is the claim: a zero here is what the catalogue prints, so it is
    // asked for exactly rather than approximately.
    #[allow(clippy::float_cmp)]
    #[test]
    fn the_facts_are_read_out_of_whatever_else_a_program_printed() {
        let look = read(
            "Blender 5.0.0 (hash 1a2b3c)\n\
             Read prefs: /home/you/.config/blender\n\
             tris 412\n\
             meshes 2\n\
             materials 1\n\
             image PolygonSciFiSpace_Texture_01_A.png\n\
             image PolygonSciFiSpace_Texture_01_A.png\n\
             aabb -0.363 0.0 -0.336 0.363 0.613 0.336\n\
             fbx_to_preview: wrote /cache/dex/preview/abc.png\n",
        );
        assert_eq!(look.triangles, 412);
        assert_eq!(look.meshes, 2);
        assert_eq!(look.materials, 1);
        assert_eq!(
            look.images,
            ["PolygonSciFiSpace_Texture_01_A.png"],
            "one image named twice is one image"
        );
        let size = look.size();
        assert!((size[1] - 0.613).abs() < 0.001, "{size:?}");

        let silent = read("some program that has never heard of this repository\n");
        assert_eq!(silent.triangles, 0);
        assert!(silent.images.is_empty());
        assert!(silent.bounds.is_none());
        assert_eq!(silent.size(), [0.0; 3]);
    }

    /// **The Blender command line carries the atlas it was handed**, and
    /// the picture it was asked for. Same reasoning as the converter's
    /// guard: this branch is the one every real run takes and the one no
    /// test can run, so the arguments are asserted directly.
    #[test]
    fn the_blender_command_line_carries_the_picture_and_the_atlas() {
        let blender = Previewer::Blender(PathBuf::from("blender"));
        let arguments = |command: &Command| -> Vec<String> {
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect()
        };
        let painted = arguments(&blender.command(
            Path::new("/cache/blender/fbx_to_preview.py"),
            Path::new("/cache/stage/SM_Prop_Crate_01.fbx"),
            Path::new("/cache/dex/preview/abc.png"),
            Some(Path::new("/cache/stage/atlas.png")),
        ));
        assert_eq!(
            painted,
            [
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
                "/cache/blender/fbx_to_preview.py",
                "--",
                "/cache/stage/SM_Prop_Crate_01.fbx",
                "/cache/dex/preview/abc.png",
                "/cache/stage/atlas.png",
            ],
            "the atlas is not on the command line the preview is rendered with"
        );
        let silent = arguments(&blender.command(
            Path::new("/cache/blender/fbx_to_preview.py"),
            Path::new("/cache/stage/SM_Prop_Crate_01.fbx"),
            Path::new("/cache/dex/preview/abc.png"),
            None,
        ));
        assert_eq!(silent, painted[..painted.len() - 1], "{silent:#?}");
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
