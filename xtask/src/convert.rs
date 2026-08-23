//! FBX in, glTF out, on somebody else's machine.
//!
//! Bevy loads glTF. Synty ships FBX. Nothing in this repository can read
//! either, and writing an FBX reader would be the largest thing in the
//! project by a wide margin, so the conversion is somebody else's
//! program.
//!
//! **Blender is the default because it is the one that is already
//! installed.** `blender --background --python` is a supported, headless,
//! scriptable path, it reads every mesh format Synty ships, and an art
//! machine has it. It is also not present in continuous integration, in
//! this container, or on a machine that has only ever run the whitebox —
//! so its absence is a first-class outcome here with a message that names
//! the thing to install, and never a stack trace.
//!
//! `$ART_CONVERTER` overrides it with any program taking a source and a
//! destination. That exists for a real reason and a test reason. The real
//! one is `FBX2glTF` and its forks: a single binary that does this job and
//! nothing else, which somebody may reasonably prefer to a 300 MB
//! install. The test one is that it is the seam that lets the pipeline be
//! exercised end to end where no converter exists at all.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cache::Cache;
use crate::fsx;

/// The script, carried in the binary. See `xtask/blender/fbx_to_gltf.py`.
const SCRIPT: &str = include_str!("../blender/fbx_to_gltf.py");

#[derive(Debug)]
pub enum Converter {
    /// `$ART_CONVERTER`, run as `<program> <source> <destination>`.
    Program(PathBuf),
    Blender(PathBuf),
}

impl Converter {
    /// What to say about it in a report line.
    pub fn describe(&self) -> String {
        match self {
            Self::Program(path) => format!("$ART_CONVERTER {}", path.display()),
            Self::Blender(path) => format!("blender {}", path.display()),
        }
    }

    pub fn run(&self, cache: &Cache, source: &Path, destination: &Path) -> Result<(), String> {
        if let Some(parent) = destination.parent() {
            fsx::create_dir_all(parent)?;
        }
        let mut command = match self {
            Self::Program(program) => {
                let mut command = Command::new(program);
                command.arg(source).arg(destination);
                command
            }
            Self::Blender(blender) => {
                let script = cache.blender_script();
                fsx::write(&script, SCRIPT)?;
                let mut command = Command::new(blender);
                command
                    .arg("--background")
                    .arg("--factory-startup")
                    .arg("--python-exit-code")
                    .arg("1")
                    .arg("--python")
                    .arg(&script)
                    .arg("--")
                    .arg(source)
                    .arg(destination);
                command
            }
        };
        let output = command
            .output()
            .map_err(|err| format!("cannot run {}: {err}", self.describe()))?;
        if !output.status.success() {
            return Err(format!(
                "{} could not convert {}\n{}",
                self.describe(),
                source.display(),
                indent(&String::from_utf8_lossy(&output.stderr))
            ));
        }
        let bytes = std::fs::metadata(destination).map_or(0, |meta| meta.len());
        if bytes == 0 {
            return Err(format!(
                "{} exited cleanly and wrote nothing to {}",
                self.describe(),
                destination.display()
            ));
        }
        Ok(())
    }
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Everywhere a converter might be named, gathered in one place so that
/// "there is no converter here" is a state a guard can put this code in
/// on a machine that happens to have Blender installed.
pub struct Lookup {
    pub converter: Option<PathBuf>,
    pub blender: Option<PathBuf>,
    pub on_path: Option<PathBuf>,
    pub installed: Vec<PathBuf>,
}

impl Lookup {
    pub fn from_env() -> Self {
        Self {
            converter: std::env::var_os("ART_CONVERTER").map(PathBuf::from),
            blender: std::env::var_os("BLENDER").map(PathBuf::from),
            on_path: on_path("blender"),
            installed: known_installs(),
        }
    }
}

/// **Find a converter, or say what to install.**
pub fn find() -> Result<Converter, String> {
    choose(&Lookup::from_env())
}

pub fn choose(lookup: &Lookup) -> Result<Converter, String> {
    if let Some(program) = &lookup.converter {
        if program.components().count() > 1 && !program.is_file() {
            return Err(format!(
                "$ART_CONVERTER is {}, and there is no program there",
                program.display()
            ));
        }
        return Ok(Converter::Program(program.clone()));
    }
    if let Some(blender) = &lookup.blender {
        if !blender.is_file() {
            return Err(format!(
                "$BLENDER is {}, and there is no program there",
                blender.display()
            ));
        }
        return Ok(Converter::Blender(blender.clone()));
    }
    if let Some(blender) = lookup
        .on_path
        .clone()
        .or_else(|| lookup.installed.iter().find(|path| path.is_file()).cloned())
    {
        return Ok(Converter::Blender(blender));
    }
    Err(NO_CONVERTER.to_owned())
}

const NO_CONVERTER: &str = "\
no converter: Blender is not on PATH, and $BLENDER and $ART_CONVERTER are not set.

  Bevy loads glTF and Synty ships FBX, so something has to translate. Pick one:

    Blender          https://www.blender.org/download/ — the default. Once it is
                     installed, `blender --version` should work in this shell; if it
                     is installed somewhere odd, set $BLENDER to the executable.
    $ART_CONVERTER   any program run as `<program> <source.fbx> <destination.glb>`,
                     which is what FBX2glTF and its forks already are.

  Everything up to conversion works without either of these:
  `cargo xtask art check` finds and hashes the sources, and says so.";

fn on_path(program: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_owned()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
}

/// The places an installer puts Blender when it is not on `PATH`, which
/// on two of the three platforms is the normal outcome.
fn known_installs() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
        PathBuf::from("/usr/local/bin/blender"),
        PathBuf::from("/snap/bin/blender"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        candidates
            .push(PathBuf::from(&home).join("Applications/Blender.app/Contents/MacOS/Blender"));
    }
    for root in [
        "C:\\Program Files\\Blender Foundation",
        "C:\\Program Files (x86)\\Blender Foundation",
    ] {
        for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
            candidates.push(entry.path().join("blender.exe"));
        }
    }
    candidates
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

    /// **A machine with no converter on it is told what to install.**
    /// Blender is absent from continuous integration, from the container
    /// this was written in, and from every machine that has only ever
    /// built the whitebox, so its absence is a normal outcome and not an
    /// exceptional one. What a person needs at that moment is the name
    /// of the program, where to get it, the two variables that override
    /// the search, and the fact that the half of the pipeline that finds
    /// and hashes their art works without any of it.
    #[test]
    fn a_machine_with_no_converter_is_told_what_to_install() {
        let complaint = choose(&nothing_anywhere()).expect_err("nothing to convert with");
        for wanted in [
            "Blender",
            "blender.org",
            "$BLENDER",
            "$ART_CONVERTER",
            "cargo xtask art check",
        ] {
            assert!(complaint.contains(wanted), "no `{wanted}` in:\n{complaint}");
        }
        assert!(
            !complaint.contains("panicked"),
            "the absence of a converter is a sentence, not a trace"
        );
    }

    /// **A converter that was named and is not there says which one.**
    /// The two overrides exist so somebody can point at an install the
    /// search would not have found, and the commonest thing to get wrong
    /// about a path typed by hand is the path.
    #[test]
    fn a_converter_that_was_named_and_is_not_there_is_named_back() {
        let missing = PathBuf::from("/nowhere/at/all/blender");
        let by_env = choose(&Lookup {
            blender: Some(missing.clone()),
            ..nothing_anywhere()
        })
        .expect_err("$BLENDER points at nothing");
        assert!(by_env.contains("/nowhere/at/all/blender"), "{by_env}");
        assert!(by_env.contains("$BLENDER"), "{by_env}");

        let by_converter = choose(&Lookup {
            converter: Some(missing),
            ..nothing_anywhere()
        })
        .expect_err("$ART_CONVERTER points at nothing");
        assert!(by_converter.contains("$ART_CONVERTER"), "{by_converter}");
    }

    /// **`$ART_CONVERTER` wins over Blender, and Blender over the
    /// search.** Somebody who names a converter has named it for a
    /// reason, and an install the search happens to find is the weakest
    /// claim of the three.
    #[test]
    fn a_converter_named_by_hand_beats_one_that_was_found() {
        let blender = std::env::current_exe().expect("this test binary is a file");
        let both = choose(&Lookup {
            converter: Some(PathBuf::from("fbx2gltf")),
            blender: Some(blender.clone()),
            on_path: Some(blender),
            installed: Vec::new(),
        })
        .expect("a converter was named");
        assert!(
            both.describe().contains("$ART_CONVERTER"),
            "{}",
            both.describe()
        );
    }
}
