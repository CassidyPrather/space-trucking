//! `$SYNTY_STORE`: the packs as downloaded, on the owner's own disk.
//!
//! Nothing in this module ever writes to the store. It is somebody's
//! downloads folder, it is large, and the tool's whole job is to read a
//! reference out of the repository and find what it names in there.
//!
//! The interesting type here is [`Missing`], because on a fresh machine
//! the missing-asset message IS this tool. Anybody can print "file not
//! found"; what a person needs at that moment is which pack to download,
//! which of its downloads, where to put it, and which line of the
//! manifest decided that.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::cache::Cache;
use crate::fsx;
use crate::manifest::{Asset, Manifest, Pack};
use crate::unitypackage;

pub struct Store {
    pub root: PathBuf,
}

impl Store {
    /// Read `$SYNTY_STORE`, or explain what it is for.
    pub fn open() -> Result<Self, String> {
        let Some(root) = std::env::var_os("SYNTY_STORE") else {
            return Err(NO_STORE.to_owned());
        };
        let root = PathBuf::from(root);
        if !root.is_dir() {
            return Err(format!(
                "$SYNTY_STORE is {}, and there is no directory there.\n\n  \
                 That variable points at wherever you keep Synty packs after unzipping \
                 them. Make the directory, or point the variable at the one you already \
                 have.",
                root.display()
            ));
        }
        Ok(Self { root })
    }

    pub fn pack_dir(&self, pack: &Pack) -> PathBuf {
        self.root.join(under(&pack.dir))
    }
}

const NO_STORE: &str = "\
$SYNTY_STORE is not set, and it is the one thing this tool cannot work out for itself.

  Synty's licence lets their art ship inside a built game and not be redistributed as
  source, so the packs are not in this repository and never will be. They live on your
  disk, and $SYNTY_STORE is where.

  Set it to the directory you unzip packs into, and make one directory per pack whose
  name matches the `dir` line for that pack in art/manifest.toml:

      export SYNTY_STORE=$HOME/art/synty

  Then see docs/ART_PIPELINE.md.";

/// A path written with `/` in the manifest, as this platform spells it.
pub fn under(relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

/// Where an asset's source bytes actually were.
pub enum Found {
    /// Straight out of the pack's Source Files download. Preferred:
    /// no archive to unpack and no reconstruction to get wrong.
    SourceFiles(PathBuf),
    /// Rebuilt out of a `.unitypackage`, which is what packs without a
    /// Source Files download leave you.
    Unpacked { path: PathBuf, package: PathBuf },
}

impl Found {
    pub const fn path(&self) -> &PathBuf {
        match self {
            Self::SourceFiles(path) | Self::Unpacked { path, .. } => path,
        }
    }

    /// Which of the two routes answered, for the report line. A pack
    /// that had to come out of an archive is worth seeing at a glance,
    /// because it is the one whose material assignments were left behind.
    pub fn via(&self) -> String {
        match self {
            Self::SourceFiles(_) => "source files".to_owned(),
            Self::Unpacked { package, .. } => format!(
                "in {}",
                package
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("a .unitypackage")
            ),
        }
    }
}

/// **Find an asset's source bytes, or say precisely what is not there.**
///
/// Source Files first, and not merely because it is faster. A pack's
/// `.unitypackage` is a Unity project fragment: prefabs, materials and
/// meshes, where a prop is often a prefab assembling several meshes
/// against a shared material. Reconstructing the tree gets the meshes and
/// drops the assembly, so the unitypackage path is not a richer answer
/// than the FBX — it is the same answer through more machinery. It is
/// here for the packs that ship no source download.
pub fn locate(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    asset: &Asset,
    may_unpack: bool,
) -> Result<Found, Box<Missing>> {
    let pack = manifest.pack_of(asset);
    let pack_dir = store.pack_dir(pack);

    let direct = pack_dir.join(under(&asset.source));
    if direct.is_file() {
        return Ok(Found::SourceFiles(direct));
    }

    let packages = if pack_dir.is_dir() {
        find_packages(&pack_dir)
    } else {
        Vec::new()
    };
    if let Some(unity) = &asset.unity {
        let relative = under(unity);
        for package in &packages {
            let into = cache.unpacked(&pack.id).join(stem(package));
            if !into.is_dir() && may_unpack {
                eprintln!(
                    "art: rebuilding the tree inside {} (once per pack, then cached)",
                    package.display()
                );
                match unitypackage::unpack(package, &into) {
                    Ok(report) => eprintln!(
                        "art: {} files and {} folders out of {}",
                        report.files,
                        report.folders,
                        package.display()
                    ),
                    Err(why) => eprintln!("art: {why}"),
                }
            }
            let candidate = into.join(&relative);
            if candidate.is_file() {
                return Ok(Found::Unpacked {
                    path: candidate,
                    package: package.clone(),
                });
            }
        }
    }

    Err(Box::new(Missing {
        id: asset.id.clone(),
        pack_title: pack.title.clone(),
        pack_download: pack.download.clone(),
        pack_line: pack.line,
        store_root: store.root.clone(),
        pack_dir_spelling: pack.dir.clone(),
        pack_dir: pack_dir.clone(),
        pack_dir_files: count_files(&pack_dir),
        source_expected: direct,
        source_spelling: asset.source.clone(),
        unity_expected: asset.unity.clone(),
        packages,
        archive_beside: archive_beside(&store.root, &pack.dir),
        manifest_path: manifest.path.clone(),
        manifest_line: asset.line,
    }))
}

/// Every `.unitypackage` anywhere under a pack directory. Synty put them
/// at the top level in some packs and one level down in others, so this
/// looks rather than assumes.
pub fn find_packages(pack_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let _ = fsx::walk(pack_dir, &mut |path| {
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("unitypackage"))
        {
            found.push(path.to_path_buf());
        }
    });
    found.sort();
    found
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("package")
        .to_owned()
}

fn count_files(dir: &Path) -> Option<usize> {
    if !dir.is_dir() {
        return None;
    }
    let mut count = 0;
    let _ = fsx::walk(dir, &mut |_| count += 1);
    Some(count)
}

/// A still-zipped download sitting where the unzipped pack should be. The
/// commonest way to get this error, and the cheapest one to fix.
fn archive_beside(store_root: &Path, dir: &str) -> Option<PathBuf> {
    let wanted = dir.to_ascii_lowercase();
    let mut best = None;
    for entry in std::fs::read_dir(store_root)
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        if [".zip", ".7z", ".rar", ".tar.gz"]
            .iter()
            .any(|ext| name.ends_with(ext))
            && (name.starts_with(&wanted) || wanted.starts_with(name.split('.').next()?))
        {
            best = Some(path);
        }
    }
    best
}

/// Everything the tool knows about one asset it could not find. The
/// fields are separate rather than a pre-baked string because the message
/// changes shape depending on which of them are set, and because the
/// guard on this message asserts about the parts.
pub struct Missing {
    pub id: String,
    pub pack_title: String,
    pub pack_download: String,
    pub pack_line: usize,
    pub store_root: PathBuf,
    pub pack_dir_spelling: String,
    pub pack_dir: PathBuf,
    /// `None` when the pack directory is not there at all.
    pub pack_dir_files: Option<usize>,
    pub source_expected: PathBuf,
    pub source_spelling: String,
    pub unity_expected: Option<String>,
    pub packages: Vec<PathBuf>,
    pub archive_beside: Option<PathBuf>,
    pub manifest_path: PathBuf,
    pub manifest_line: usize,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} is not on this machine.", self.id)?;
        writeln!(f)?;
        writeln!(f, "  pack      {}", self.pack_title)?;
        writeln!(f, "  download  {}", self.pack_download)?;
        writeln!(
            f,
            "  declared  {}:{}",
            self.manifest_path.display(),
            self.manifest_line
        )?;
        writeln!(f, "  wanted    {}", self.source_expected.display())?;
        if let Some(unity) = &self.unity_expected {
            writeln!(
                f,
                "  or        {unity}, inside a .unitypackage in that pack ({} found)",
                self.packages.len()
            )?;
        }
        match self.pack_dir_files {
            None => writeln!(
                f,
                "  found     nothing: {} does not exist",
                self.pack_dir.display()
            )?,
            Some(files) => writeln!(
                f,
                "  found     {} ({files} files), but nothing at that path",
                self.pack_dir.display()
            )?,
        }
        writeln!(f)?;
        if let Some(archive) = &self.archive_beside {
            writeln!(
                f,
                "  fix       {} is still an archive. Unzip it so it becomes\n            \
                 {}",
                archive.display(),
                self.pack_dir.display()
            )?;
        } else if self.pack_dir_files.is_none() {
            writeln!(
                f,
                "  fix       Download \"{}\" from your Synty account's downloads, unzip it,\n            \
                 and put the result at {}",
                self.pack_download,
                self.pack_dir.display()
            )?;
        } else if self.unity_expected.is_none() && !self.packages.is_empty() {
            writeln!(
                f,
                "  fix       The pack is here and does not carry that path. It does carry {} \
                 .unitypackage\n            file(s); add a `unity = \"Assets/...\"` line to \
                 [asset.{}] in\n            {} and the tree inside them will be searched too.",
                self.packages.len(),
                self.id,
                self.manifest_path.display()
            )?;
        } else {
            writeln!(
                f,
                "  fix       The pack is here and does not carry that path. Run\n            \
                 cargo xtask art find {}\n            to see what it does carry, then correct \
                 `source` on {}:{}.",
                needle(&self.source_spelling),
                self.manifest_path.display(),
                self.manifest_line
            )?;
        }
        writeln!(
            f,
            "            The directory is $SYNTY_STORE ({}) plus `dir = \"{}\"` on {}:{}.",
            self.store_root.display(),
            self.pack_dir_spelling,
            self.manifest_path.display(),
            self.pack_line
        )
    }
}

/// The part of a path worth searching for: the file name without its
/// extension, which is what a person would type.
fn needle(source: &str) -> &str {
    let name = source.rsplit('/').next().unwrap_or(source);
    name.split('.').next().unwrap_or(name)
}

/// Every file under the store whose name contains `needle`, case
/// insensitively — plus, so the search is not blind to what is inside an
/// archive, everything already rebuilt into the cache.
pub fn search(roots: &[PathBuf], needle: &str) -> Vec<PathBuf> {
    let needle = needle.to_ascii_lowercase();
    let mut hits = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let _ = fsx::walk(root, &mut |path| {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.to_ascii_lowercase().contains(&needle))
            {
                hits.push(path.to_path_buf());
            }
        });
    }
    hits.sort();
    hits
}

/// A second pack-relative path — a texture, usually — looked for in the
/// same places the mesh was: the Source Files tree first, then whatever
/// has already been rebuilt out of the pack's archives.
pub fn find_relative(store: &Store, cache: &Cache, pack: &Pack, relative: &str) -> Option<PathBuf> {
    let direct = store.pack_dir(pack).join(under(relative));
    if direct.is_file() {
        return Some(direct);
    }
    let unpacked = cache.unpacked(&pack.id);
    for entry in std::fs::read_dir(&unpacked).into_iter().flatten().flatten() {
        let candidate = entry.path().join(under(relative));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "\
[pack.scifi_space]
title = \"POLYGON Sci-Fi Space\"
dir = \"polygon-scifi-space\"
download = \"POLYGON Sci-Fi Space, the Source Files download\"

[asset.crate_small]
pack = \"scifi_space\"
source = \"SourceFiles/FBX/SM_Crate.fbx\"
";

    fn missing_message(extra: &str) -> String {
        let path = PathBuf::from("art/manifest.toml");
        let manifest = Manifest::parse(&path, &format!("{MANIFEST}{extra}")).expect("a manifest");
        let store = Store {
            root: PathBuf::from("/somebody/synty"),
        };
        let cache = Cache {
            root: PathBuf::from("/somebody/cache"),
        };
        locate(
            &store,
            &cache,
            &manifest,
            &manifest.assets["crate_small"],
            false,
        )
        .err()
        .expect("nothing is on this machine")
        .to_string()
    }

    /// **The missing-asset message names the pack to download and the
    /// path it wanted.**
    ///
    /// On a fresh machine this message IS the tool: nobody who clones
    /// this repository has any of the art, and the first thing they will
    /// ever see the resolver do is refuse. So the refusal has to carry
    /// everything the next action needs — which asset, which pack, which
    /// of that pack's downloads, where the file was expected, which line
    /// of the manifest decided that, and which directory to put the
    /// download in — because anything it leaves out is a search through
    /// somebody's downloads folder.
    #[test]
    fn a_missing_asset_names_the_pack_to_download_and_the_path_it_wanted() {
        let message = missing_message("");
        for wanted in [
            "crate_small",
            "POLYGON Sci-Fi Space",
            "the Source Files download",
            "art/manifest.toml:6",
            "SM_Crate.fbx",
            "polygon-scifi-space",
            "$SYNTY_STORE",
            "/somebody/synty",
            "Download",
        ] {
            assert!(message.contains(wanted), "no `{wanted}` in:\n{message}");
        }
    }

    /// **A pack that is here and does not carry the path says so, and
    /// says how to find out what it does carry.** "Not found" is the same
    /// four words whether the pack was never downloaded or the path is a
    /// typo, and those two situations have nothing in common: one is a
    /// download and the other is a search.
    #[test]
    fn a_pack_that_is_present_and_wrong_reads_differently_from_one_that_is_absent() {
        let absent = missing_message("");
        assert!(absent.contains("does not exist"), "{absent}");
        assert!(absent.contains("Download"), "{absent}");

        let here = std::env::temp_dir().join(format!(
            "space-trucking-store-{}-{}",
            std::process::id(),
            line!()
        ));
        let pack = here.join("polygon-scifi-space");
        crate::fsx::create_dir_all(&pack).expect("a scratch pack");
        crate::fsx::write(&pack.join("readme.txt"), "not the mesh").expect("a file in it");
        let path = PathBuf::from("art/manifest.toml");
        let manifest = Manifest::parse(&path, MANIFEST).expect("a manifest");
        let store = Store { root: here.clone() };
        let cache = Cache {
            root: PathBuf::from("/somebody/cache"),
        };
        let message = locate(
            &store,
            &cache,
            &manifest,
            &manifest.assets["crate_small"],
            false,
        )
        .err()
        .expect("the mesh is still not there")
        .to_string();
        let _ = crate::fsx::remove_dir_all(&here);
        assert!(message.contains("1 files"), "{message}");
        assert!(
            message.contains("cargo xtask art find SM_Crate"),
            "a present pack should be searched, not downloaded again:\n{message}"
        );
        assert!(!message.contains("Download \""), "{message}");
    }
}
