//! Rebuilding the asset tree inside a `.unitypackage`, with no Unity
//! anywhere near it.
//!
//! The format is a tar of one directory per asset, named after the
//! asset's Unity GUID rather than after the file. Each such directory
//! holds the original bytes as `asset`, Unity's import settings as
//! `asset.meta`, the project-relative path as `pathname`, and sometimes a
//! `preview.png`. A directory in the project is an entry with a
//! `pathname` and no `asset`. So untarring gives a heap of hex-named
//! folders, and reading each `pathname` while writing each `asset` gives
//! back the tree.
//!
//! **The tar is usually gzipped and is not always.** Some exporters write
//! it plain. Both `tar -xf` implementations that matter — GNU tar and the
//! bsdtar that Windows 10 1803 and later ship as `tar.exe` — sniff the
//! compression themselves, so this passes no `-z` and both cases work.
//!
//! Shelling out to `tar` rather than reading the archive here is one of
//! the two places this tool leans on the machine it runs on; the other is
//! [`crate::archive`]. Inflating DEFLATE and parsing ustar headers is a
//! few hundred lines that every operating system already has, and the
//! failure — no `tar` on `PATH` — is loud, immediate, and has an obvious
//! fix.
//!
//! **This one rebuilds the whole tree, where a zip is read a file at a
//! time, and the asymmetry is the format's rather than a shortcut.** A
//! zip member carries the file's own name, so one named file can be
//! found and taken out. A `.unitypackage` member carries a GUID, and the
//! only thing that says which GUID holds `Assets/.../SM_Crate.fbx` is
//! every `pathname` in the archive — so finding one file means reading
//! all of them, and a tar is a stream, so reading any of it decompresses
//! all of it. The tree is that reading, written down once and kept.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::archive;
use crate::fsx;

pub struct Report {
    /// Files written into the reconstructed tree.
    pub files: usize,
    /// Entries that named a path and carried no bytes: Unity's folders.
    pub folders: usize,
}

/// Rebuild `package` under `into`, replacing whatever was there.
pub fn unpack(package: &Path, into: &Path) -> Result<Report, String> {
    check_tar()?;
    fsx::remove_dir_all(into)?;
    let raw = into.with_extension("raw");
    fsx::remove_dir_all(&raw)?;
    fsx::create_dir_all(&raw)?;
    fsx::create_dir_all(into)?;

    let status = Command::new("tar")
        .arg("-xf")
        .arg(package)
        .arg("-C")
        .arg(&raw)
        .status()
        .map_err(|err| format!("cannot run tar: {err}"))?;
    if !status.success() {
        return Err(format!(
            "tar could not read {}\n  \
             A .unitypackage is a tar archive, usually gzipped. If tar refuses it, the \
             download is truncated or is not a .unitypackage at all — check its size against \
             the store page and download it again.",
            package.display()
        ));
    }

    let mut report = Report {
        files: 0,
        folders: 0,
    };
    let mut guid_dirs = Vec::new();
    collect_entry_dirs(&raw, &mut guid_dirs)?;
    guid_dirs.sort();
    for dir in guid_dirs {
        let pathname = fsx::read_to_string(&dir.join("pathname"))?;
        let relative = destination(&pathname).map_err(|why| {
            format!(
                "{} names `{}`, and {why}",
                dir.join("pathname").display(),
                pathname.trim()
            )
        })?;
        let asset = dir.join("asset");
        if asset.is_file() {
            fsx::copy(&asset, &into.join(&relative))?;
            report.files += 1;
        } else {
            report.folders += 1;
        }
    }
    fsx::remove_dir_all(&raw)?;
    Ok(report)
}

/// Every directory under `root` that holds a `pathname`. Real packages
/// put them at the top level and some tars wrap everything in `./`, so
/// this looks rather than assumes.
fn collect_entry_dirs(root: &Path, into: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
        let mut has_pathname = false;
        for entry in entries {
            let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|name| name == "pathname") {
                has_pathname = true;
            }
        }
        if has_pathname {
            into.push(dir);
        }
    }
    Ok(())
}

/// **Where a `pathname` file says its asset goes, or why it may not go
/// there.**
///
/// The file holds the project-relative path and a trailing newline, and
/// some exporters add a second line that is not a path. Only the first
/// line is the answer.
///
/// Whether the answer may be written at all is [`archive::inside`]'s
/// question, not this one's: a `pathname` is a string chosen by whoever
/// built the archive and a zip member is too, so the law that
/// `../../.ssh/authorized_keys` is refused rather than obeyed is stated
/// once, for every route into the cache.
pub fn destination(pathname: &str) -> Result<PathBuf, String> {
    archive::inside(pathname.lines().next().unwrap_or("").trim())
}

/// **Every path a package would rebuild to, without rebuilding any of
/// it.**
///
/// A zip names its members after the files, so listing one answers "what
/// is in this pack?" directly. A `.unitypackage` names its members after
/// Unity GUIDs, so listing one answers nothing; the file names are the
/// *contents* of the `pathname` entries. `tar -xO` writes entry contents
/// to standard output and nothing to disk, so asking it for the
/// `pathname` entries alone reads the names out of a package that stays
/// exactly as downloaded.
///
/// It is not as cheap as listing a zip and the difference is worth
/// knowing: a zip's table is at the end of the file, while a tar is a
/// stream, so this decompresses the whole archive to print a few
/// kilobytes. It writes nothing, which is the property that matters.
pub fn pathnames(package: &Path) -> Result<Vec<String>, String> {
    check_tar()?;
    // GNU tar treats a member name as a literal unless `--wildcards`
    // says otherwise; bsdtar globs by default and rejects the option
    // outright. So: ask the way GNU tar wants, and if the program would
    // not take that, ask again the way bsdtar wants.
    for wildcards in [true, false] {
        let mut command = Command::new("tar");
        command.arg("-xOf").arg(package);
        if wildcards {
            command.arg("--wildcards");
        }
        command.arg("*pathname");
        let Ok(output) = command.stderr(std::process::Stdio::null()).output() else {
            break;
        };
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect());
        }
    }
    Err(format!(
        "tar could not read the names inside {}\n  \
         A .unitypackage is a tar archive, usually gzipped. If tar refuses it, the \
         download is truncated or is not a .unitypackage at all — check its size against \
         the store page and download it again.",
        package.display()
    ))
}

fn check_tar() -> Result<(), String> {
    Command::new("tar")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|_| ())
        .map_err(|_| {
            "no `tar` on PATH, and a .unitypackage is a tar archive\n  \
             Linux and macOS ship one. Windows 10 build 1803 and later ship bsdtar as \
             tar.exe; if `tar --version` fails in your shell, install Git for Windows or \
             7-Zip and put it on PATH."
                .to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A pathname is data, not an instruction.** The archive is a file
    /// somebody downloaded, this code writes files at the paths inside
    /// it, and the shortest distance between those two facts is a
    /// `..` that lands a payload outside the cache.
    #[test]
    fn a_pathname_that_climbs_out_of_the_tree_is_refused() {
        for hostile in [
            "../../.ssh/authorized_keys",
            "Assets/../../../etc/passwd",
            "/etc/passwd",
            "..\\..\\Windows\\System32\\drivers\\etc\\hosts",
            "C:/Windows/System32/hosts",
            "",
            "   \n",
        ] {
            assert!(
                destination(hostile).is_err(),
                "`{hostile}` was accepted as a destination"
            );
        }
    }

    /// **The path an entry lands at is the first line of its `pathname`,
    /// and nothing else in the file.** Exporters end it with a newline
    /// and some of them write a second line that is not a path at all;
    /// taking the whole file would make a directory named after it.
    #[test]
    fn an_entry_lands_where_the_first_line_of_its_pathname_says() {
        let want: PathBuf = ["Assets", "Polygon", "Meshes", "SM_Crate.fbx"]
            .iter()
            .collect();
        for spelling in [
            "Assets/Polygon/Meshes/SM_Crate.fbx",
            "Assets/Polygon/Meshes/SM_Crate.fbx\n",
            "Assets/Polygon/Meshes/SM_Crate.fbx\n00\n",
            "./Assets/Polygon/Meshes/SM_Crate.fbx\n",
            "Assets\\Polygon\\Meshes\\SM_Crate.fbx\n",
        ] {
            assert_eq!(destination(spelling).as_deref(), Ok(want.as_path()));
        }
    }
}
