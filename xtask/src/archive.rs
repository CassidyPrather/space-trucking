//! The archives a pack directory holds, read where they lie.
//!
//! A pack in `$SYNTY_STORE` is a download rather than an unpacked tree.
//! The store hands over a `.unitypackage`, an icon, and the raw assets in
//! a zip, and nothing here asks for any of it to be unzipped first. That
//! is not politeness. A manifest naming fifty props out of a
//! five-thousand-file pack should cost fifty files on disk; unzipping the
//! pack to reach them costs the whole pack, on every machine, for ever.
//!
//! Two operations, and the distance between what they cost is the whole
//! design. **Listing is cheap.** A zip ends with a table naming every
//! member, so "what is in here?" is a seek and a few kilobytes however
//! many gigabytes the members take. **Extracting is not.** So `find`
//! lists and never extracts, and `check` and `resolve` extract by name.
//!
//! ## What opens a zip
//!
//! [`crate::unitypackage`] shells out to `tar` and lets GNU tar and the
//! bsdtar Windows ships sniff the compression themselves. The obvious
//! hope was that the same call would open a zip, since libarchive reads
//! zip perfectly well. **It does not, and this was run rather than
//! assumed:** GNU tar 1.35 answers `tar: This does not look like a tar
//! archive` and exits 2. bsdtar — which is `tar` on macOS and `tar.exe`
//! on Windows 10 build 1803 and later — does read zip.
//!
//! So a zip is offered to `tar` first, because on the two platforms where
//! that works there is nothing whatever to install, and to `unzip`
//! second, which is what Linux has and has had for thirty years. Neither
//! is a dependency of this crate; both are things the operating system
//! already shipped, which is the same bargain `tar` was. A machine with
//! neither is told which one to install, rather than told a mesh is
//! missing.
//!
//! A vendored inflate was the alternative and is the wrong trade here.
//! Reading a zip properly means DEFLATE, the central directory, and Zip64
//! for the packs that pass four gigabytes — several hundred lines whose
//! bugs are silently wrong bytes in a mesh, in a repository that has no
//! Synty pack to test any of it against.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How an archive is packed, which decides what can open it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A zip: how Synty ships a pack's raw assets, and what a pack
    /// directory holds when nothing has been unzipped.
    Zip,
    /// A tar, compressed or not. Supported because it costs nothing —
    /// `tar` is already required for `.unitypackage` — and because a
    /// store that hands out `.tar.gz` should not need a code change.
    Tar,
}

/// **What a file in a pack directory is, or `None` for the furniture.**
///
/// A pack directory now has things in it that are not the art: the icon,
/// a readme, a licence, whatever else the download carried. They are
/// recognised by not being archives, which is the only way that stays
/// true as the store changes what it ships.
///
/// A `.unitypackage` is a tar and is deliberately not one of these. Its
/// members are named after Unity GUIDs rather than after files, so
/// listing one answers nothing anybody asked; [`crate::unitypackage`]
/// reads the `pathname` entries instead.
pub fn kind_of(path: &Path) -> Option<Kind> {
    // The whole file name, folded, rather than `Path::extension`: a store
    // that writes `.ZIP` means the same thing by it, and `.tar.gz` is two
    // extensions to a path and one format to everybody else.
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    let ends_with_any = |suffixes: &[&str]| suffixes.iter().any(|suffix| name.ends_with(suffix));
    if ends_with_any(&[".zip"]) {
        return Some(Kind::Zip);
    }
    if ends_with_any(&[".tar", ".tar.gz", ".tgz"]) {
        return Some(Kind::Tar);
    }
    None
}

/// An archive this tool recognises and cannot open, named so the refusal
/// can say which one it is. Nothing here reads 7-Zip or RAR, and a pack
/// reported missing because its archive was one of those is a person
/// looking for a file that is right there.
pub fn unreadable_kind(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    [".7z", ".rar"]
        .into_iter()
        .find(|extension| name.ends_with(extension))
}

/// One file inside an archive.
pub struct Member {
    /// Exactly as the archive stores it, which is what the program that
    /// opens it will match on. GNU tar will not find `a/b` in an archive
    /// that wrote the entry as `./a/b`, so the spelling is kept verbatim
    /// rather than tidied.
    pub name: String,
    /// The same path with the archive's own decoration off, checked, and
    /// safe to write under a directory here. This is what a member is
    /// matched by and where it lands.
    pub inside: PathBuf,
}

/// **Where a path inside an archive may be written, or why it may not.**
///
/// The archive is a file somebody downloaded, this code writes files at
/// the paths inside it, and `../../.ssh/authorized_keys` is a perfectly
/// well-formed member name. Every route into the cache — a zip member, a
/// `.unitypackage`'s `pathname` — comes through here.
pub fn inside(member: &str) -> Result<PathBuf, String> {
    let normalized = member.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err("a leading `/` makes it absolute".to_owned());
    }
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err("a `..` in it would climb out of the tree".to_owned()),
            other if other.contains(':') => {
                return Err(
                    "a `:` in it makes it a drive or a stream, not a relative path".to_owned(),
                );
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err("it is empty".to_owned());
    }
    Ok(parts.iter().collect())
}

/// A relative path spelled the way a manifest spells it: `/` on every
/// platform, so a line pasted on Windows reads on Linux.
pub fn slashed(path: &Path) -> String {
    path.components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// **Every file in an archive, without extracting any of it.**
///
/// Directory entries are dropped — nothing here wants them — and so is
/// any member whose name would climb out of the tree, because a name
/// that may never be written is not a hit anybody can act on.
pub fn list(archive: &Path) -> Result<Vec<Member>, String> {
    let Some(kind) = kind_of(archive) else {
        return Err(format!(
            "{} is not an archive this tool opens",
            archive.display()
        ));
    };
    let mut absent = 0;
    for tool in tools(kind) {
        match tool.list(archive) {
            Outcome::Absent => absent += 1,
            Outcome::Refused => {}
            Outcome::Answered(text) => {
                let mut members: Vec<Member> = text
                    .lines()
                    .map(str::trim_end)
                    .filter(|line| !line.is_empty() && !line.ends_with('/'))
                    .filter_map(|line| {
                        inside(line).ok().map(|inside| Member {
                            name: line.to_owned(),
                            inside,
                        })
                    })
                    .collect();
                members.sort_by(|one, two| one.inside.cmp(&two.inside));
                return Ok(members);
            }
        }
    }
    Err(unopenable(archive, kind, absent == tools(kind).len()))
}

/// **Take exactly these members out, and nothing else.**
///
/// They land under `into` at the paths the archive gave them, which is
/// what makes a second run free: the caller looks for the file before
/// asking for it.
pub fn extract(archive: &Path, members: &[&Member], into: &Path) -> Result<(), String> {
    if members.is_empty() {
        return Ok(());
    }
    let Some(kind) = kind_of(archive) else {
        return Err(format!(
            "{} is not an archive this tool opens",
            archive.display()
        ));
    };
    crate::fsx::create_dir_all(into)?;
    let mut absent = 0;
    for tool in tools(kind) {
        match tool.extract(archive, members, into) {
            Outcome::Absent => absent += 1,
            Outcome::Refused => {}
            Outcome::Answered(_) => {
                // The program said it worked; the file is the proof. A
                // member name carrying a `*`, a `?` or a `[` is a glob to
                // `unzip` and may match nothing at all, and a silent
                // nothing here would surface a long way away as a mesh
                // that is missing for no stated reason.
                if let Some(lost) = members
                    .iter()
                    .find(|member| !into.join(&member.inside).exists())
                {
                    return Err(format!(
                        "{} said it took `{}` out of {}, and there is nothing at {}",
                        tool.program(),
                        lost.name,
                        archive.display(),
                        into.join(&lost.inside).display()
                    ));
                }
                return Ok(());
            }
        }
    }
    Err(unopenable(archive, kind, absent == tools(kind).len()))
}

/// One way of opening an archive: a program the operating system already
/// has, and the arguments that make it list and extract.
#[derive(Clone, Copy)]
enum Tool {
    Tar,
    Unzip,
}

/// What running a program came to. Kept apart from a plain `Result`
/// because "this machine has no such program" and "this program read the
/// file and refused it" want different sentences said to a person.
enum Outcome {
    /// Not on `PATH`.
    Absent,
    /// Ran, and would not read this archive.
    Refused,
    /// Ran, and this is its standard output.
    Answered(String),
}

/// The programs worth offering an archive of this kind, in order.
const fn tools(kind: Kind) -> &'static [Tool] {
    match kind {
        Kind::Zip => &[Tool::Tar, Tool::Unzip],
        Kind::Tar => &[Tool::Tar],
    }
}

impl Tool {
    const fn program(self) -> &'static str {
        match self {
            Self::Tar => "tar",
            Self::Unzip => "unzip",
        }
    }

    fn list(self, archive: &Path) -> Outcome {
        let mut command = Command::new(self.program());
        match self {
            Self::Tar => command.arg("-tf").arg(archive),
            // -Z1 is zipinfo's terse form: one member name per line and
            // nothing else, which is the whole central directory and none
            // of the payload.
            Self::Unzip => command.arg("-Z1").arg(archive),
        };
        run(command)
    }

    fn extract(self, archive: &Path, members: &[&Member], into: &Path) -> Outcome {
        let mut command = Command::new(self.program());
        match self {
            Self::Tar => {
                command
                    .arg("-xf")
                    .arg(archive)
                    .arg("-C")
                    .arg(into)
                    .arg("--");
                for member in members {
                    command.arg(&member.name);
                }
            }
            Self::Unzip => {
                command.arg("-o").arg("-q").arg(archive);
                for member in members {
                    command.arg(&member.name);
                }
                command.arg("-d").arg(into);
            }
        }
        run(command)
    }
}

fn run(mut command: Command) -> Outcome {
    let output = match command.stderr(Stdio::null()).output() {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Outcome::Absent,
        Err(_) => return Outcome::Refused,
    };
    if output.status.success() {
        Outcome::Answered(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Outcome::Refused
    }
}

/// The refusal a machine gets when it cannot open a pack's archive.
///
/// Two quite different situations, and telling them apart is the whole
/// value: nothing is installed, which is a package manager away, or
/// something is installed and would not read this file, which means the
/// download is wrong.
fn unopenable(archive: &Path, kind: Kind, nothing_installed: bool) -> String {
    let beside = archive
        .parent()
        .map_or_else(|| PathBuf::from("the pack directory"), Path::to_path_buf);
    if !nothing_installed {
        return format!(
            "{} is here and cannot be opened.\n\n  \
             The program that reads {} is installed and refused this file, so the download \
             is\n  truncated or is not what its name says. Check its size against the store \
             page\n  and download it again.",
            archive.display(),
            match kind {
                Kind::Zip => "a zip",
                Kind::Tar => "a tar",
            }
        );
    }
    match kind {
        Kind::Tar => format!(
            "no `tar` on PATH, and {} is a tar archive\n  \
             Linux and macOS ship one. Windows 10 build 1803 and later ship bsdtar as \
             tar.exe; if `tar --version` fails in your shell, install Git for Windows or \
             7-Zip and put it on PATH.",
            archive.display()
        ),
        Kind::Zip => format!(
            "nothing on this machine can open {}, and it is a zip.\n\n  \
             A pack keeps its raw assets zipped and this tool reads them where they lie, so \
             that a\n  manifest naming fifty props costs fifty files rather than a whole \
             unzipped pack. Either\n  of two programs does that, and this machine needs one \
             of them:\n\n    \
             tar    already reads zip on macOS and on Windows 10 build 1803 and later, whose\n           \
             tar.exe is bsdtar. Linux ships GNU tar, which reads tar and not zip.\n    \
             unzip  apt install unzip, dnf install unzip, brew install unzip.\n\n  \
             Failing both, unzip the pack by hand into {} — a loose tree beside the\n  \
             archive is found before the archive is.",
            archive.display(),
            beside.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A member name is data, not an instruction.** An archive is a
    /// file somebody downloaded, this code writes files at the paths
    /// inside it, and the shortest distance between those two facts is a
    /// `..` that lands a payload outside the cache. Every route in —
    /// a zip member, a `.unitypackage`'s `pathname` — comes through
    /// [`inside`], so this is the law for all of them.
    #[test]
    fn a_member_name_that_climbs_out_of_the_tree_is_refused() {
        for hostile in [
            "../../.ssh/authorized_keys",
            "Assets/../../../etc/passwd",
            "/etc/passwd",
            "..\\..\\Windows\\System32\\drivers\\etc\\hosts",
            "C:/Windows/System32/hosts",
            "",
            "./",
            "Assets/Demo/../../../../root/.bashrc",
        ] {
            assert!(
                inside(hostile).is_err(),
                "`{hostile}` was accepted as a destination"
            );
        }
    }

    /// **The icon and the readme in a pack directory are not archives.**
    /// A pack directory is a download with furniture in it now, and a
    /// tool that offered `icon.png` to `unzip` would report a failure
    /// about a file nobody asked about.
    #[test]
    fn only_the_archives_in_a_pack_directory_are_treated_as_archives() {
        for archive in [
            "POLYGON Sci-Fi Space.zip",
            "Raw Assets.ZIP",
            "sources.tar.gz",
            "sources.tgz",
        ] {
            assert!(
                kind_of(Path::new(archive)).is_some(),
                "{archive} is an archive"
            );
        }
        for furniture in [
            "icon.png",
            "readme.txt",
            "Licence.pdf",
            // A tar, and its members are GUIDs rather than file names, so
            // it is read by the module that knows that.
            "POLYGON Sci-Fi Space.unitypackage",
        ] {
            assert!(
                kind_of(Path::new(furniture)).is_none(),
                "{furniture} is not an archive to open by name"
            );
        }
    }
}
