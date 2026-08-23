//! `$SYNTY_STORE`: the packs as downloaded, on the owner's own disk.
//!
//! **As downloaded.** A pack directory is whatever the store handed over
//! and nothing more: a `.unitypackage`, an icon, and the raw assets still
//! inside a zip. Nothing here writes to the store, nothing here asks for
//! anything to be unzipped first, and the archives are read where they
//! lie — see [`crate::archive`] for what that costs and why it is worth
//! it.
//!
//! The interesting type here is [`Missing`], because on a fresh machine
//! the missing-asset message IS this tool. Anybody can print "file not
//! found"; what a person needs at that moment is which pack to download,
//! which of its downloads, where to put it, and which line of the
//! manifest decided that.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::archive::{self, Member};
use crate::cache::Cache;
use crate::fsx;
use crate::manifest::{Asset, Manifest, Pack};
use crate::unitypackage;

pub use crate::archive::slashed;

/// What one archive turned out to hold, or why that could not be read.
type Listing = Rc<Result<Vec<Member>, String>>;

pub struct Store {
    pub root: PathBuf,
    /// The archives already listed this run. A manifest naming fifty
    /// props out of one pack should read that pack's table once, not
    /// fifty times, and every asset asks the same question of the same
    /// zip.
    listings: RefCell<BTreeMap<PathBuf, Listing>>,
}

impl Store {
    pub const fn at(root: PathBuf) -> Self {
        Self {
            root,
            listings: RefCell::new(BTreeMap::new()),
        }
    }

    /// Read `$SYNTY_STORE`, or explain what it is for.
    pub fn open() -> Result<Self, String> {
        let Some(root) = std::env::var_os("SYNTY_STORE") else {
            return Err(NO_STORE.to_owned());
        };
        let root = PathBuf::from(root);
        if !root.is_dir() {
            return Err(format!(
                "$SYNTY_STORE is {}, and there is no directory there.\n\n  \
                 That variable points at wherever you keep Synty packs after downloading \
                 them — as downloaded, zipped or not. Make the directory, or point the \
                 variable at the one you already have.",
                root.display()
            ));
        }
        Ok(Self::at(root))
    }

    pub fn pack_dir(&self, pack: &Pack) -> PathBuf {
        self.root.join(under(&pack.dir))
    }

    /// What is inside one archive, read once per run.
    fn listing(&self, archive_path: &Path) -> Listing {
        if let Some(known) = self.listings.borrow().get(archive_path) {
            return Rc::clone(known);
        }
        let listing = Rc::new(archive::list(archive_path));
        self.listings
            .borrow_mut()
            .insert(archive_path.to_path_buf(), Rc::clone(&listing));
        listing
    }
}

const NO_STORE: &str = "\
$SYNTY_STORE is not set, and it is the one thing this tool cannot work out for itself.

  Synty's licence lets their art ship inside a built game and not be redistributed as
  source, so the packs are not in this repository and never will be. They live on your
  disk, and $SYNTY_STORE is where.

  Set it to the directory you download packs into, and give each pack a directory whose
  name matches the `dir` line for that pack in art/manifest.toml. Put the download in
  it exactly as it arrived — the .unitypackage, the icon, the zip of raw assets. None
  of it needs unzipping; this tool reads what is inside:

      export SYNTY_STORE=$HOME/art/synty

  Then see docs/ART_PIPELINE.md.";

/// "1 asset", "2 assets". A report that says "1 assets" reads like a
/// machine talking, and every other line this tool prints is a sentence.
pub fn count(many: usize, thing: &str) -> String {
    if many == 1 {
        format!("1 {thing}")
    } else {
        format!("{many} {thing}s")
    }
}

/// A path written with `/` in the manifest, as this platform spells it.
pub fn under(relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

/// Where an asset's source bytes actually were.
pub enum Found {
    /// Lying loose in the pack directory, in a Source Files tree somebody
    /// already unzipped. Nothing to open and nothing to reconstruct.
    SourceFiles(PathBuf),
    /// Inside the pack's raw archive, taken out into the cache one file
    /// at a time. The same Source Files tree, still zipped, which is how
    /// a pack arrives.
    InArchive { path: PathBuf, archive: PathBuf },
    /// Rebuilt out of a `.unitypackage`, which is what packs without a
    /// Source Files download leave you.
    Unpacked { path: PathBuf, package: PathBuf },
}

impl Found {
    pub const fn path(&self) -> &PathBuf {
        match self {
            Self::SourceFiles(path)
            | Self::InArchive { path, .. }
            | Self::Unpacked { path, .. } => path,
        }
    }

    /// Which route answered, for the report line. A pack that had to come
    /// out of a `.unitypackage` is worth seeing at a glance, because it
    /// is the one whose material assignments were left behind.
    pub fn via(&self) -> String {
        match self {
            Self::SourceFiles(_) => "source files".to_owned(),
            Self::InArchive { archive, .. } => format!("in {}", leaf(archive)),
            Self::Unpacked { package, .. } => format!("in {}", leaf(package)),
        }
    }
}

fn leaf(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("an archive")
}

/// **Find an asset's source bytes, or say precisely what is not there.**
///
/// Three places, in this order: a loose Source Files tree, the pack's raw
/// archive, then a `.unitypackage`. The first two are the same download
/// and rank together; the third ranks last, and not merely because it is
/// slower. A pack's `.unitypackage` is a Unity project fragment: prefabs,
/// materials and meshes, where a prop is often a prefab assembling
/// several meshes against a shared material. Reconstructing the tree gets
/// the meshes and drops the assembly, so the unitypackage path is not a
/// richer answer than the FBX — it is the same answer through more
/// machinery. It is here for the packs that ship no source download.
///
/// `may_extract` is off for the guards that are about the wording of the
/// refusal, where nothing should be written anywhere.
pub fn locate(
    store: &Store,
    cache: &Cache,
    manifest: &Manifest,
    asset: &Asset,
    may_extract: bool,
) -> Result<Found, Box<Missing>> {
    let pack = manifest.pack_of(asset);
    let pack_dir = store.pack_dir(pack);

    let direct = pack_dir.join(under(&asset.source));
    if direct.is_file() {
        return Ok(Found::SourceFiles(direct));
    }

    let archives = find_archives(&pack_dir);
    let mut trouble = Vec::new();
    for archive_path in &archives {
        match take_out(store, cache, pack, archive_path, &asset.source, may_extract) {
            Ok(Some(path)) => {
                return Ok(Found::InArchive {
                    path,
                    archive: archive_path.clone(),
                });
            }
            Ok(None) => {}
            Err(why) => trouble.push(why),
        }
    }

    let packages = if pack_dir.is_dir() {
        find_packages(&pack_dir)
    } else {
        Vec::new()
    };
    if let Some(unity) = &asset.unity {
        let relative = under(unity);
        for package in &packages {
            let into = cache.unpacked(&pack.id).join(leaf(package));
            if !into.is_dir() && may_extract {
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
        pack_dir_files: count_files(&pack_dir),
        unreadable: unreadable_archives(&pack_dir),
        pack_dir: pack_dir.clone(),
        source_expected: direct,
        source_spelling: asset.source.clone(),
        unity_expected: asset.unity.clone(),
        archives,
        archive_trouble: trouble,
        packages,
        archive_beside: archive_beside(&store.root, &pack.dir),
        manifest_path: manifest.path.clone(),
        manifest_line: asset.line,
    }))
}

/// **One pack-relative path, out of one archive, into the cache.**
///
/// `Ok(None)` means the archive does not carry it, which is ordinary: a
/// pack directory may hold several downloads. `Err` means the archive
/// could not be opened or the file could not be taken out, which is
/// something a person has to act on.
///
/// A second run does no work at all: the file it would write is the file
/// it looks for first.
fn take_out(
    store: &Store,
    cache: &Cache,
    pack: &Pack,
    archive_path: &Path,
    relative: &str,
    may_extract: bool,
) -> Result<Option<PathBuf>, String> {
    let listing = store.listing(archive_path);
    let members = listing.as_ref().as_ref().map_err(Clone::clone)?;
    let Some(member) = pick(members, relative) else {
        return Ok(None);
    };
    let into = cache.unpacked(&pack.id).join(leaf(archive_path));
    let landed = into.join(&member.inside);
    if landed.is_file() {
        return Ok(Some(landed));
    }
    if !may_extract {
        return Ok(None);
    }
    eprintln!(
        "art: taking {} out of {} (the archive stays as it is)",
        slashed(&member.inside),
        archive_path.display()
    );
    archive::extract(archive_path, &[member], &into)?;
    Ok(landed.is_file().then_some(landed))
}

/// **Which member of an archive a pack-relative path names.**
///
/// Exact first. Then a match on whole path components from the right,
/// because a Synty zip wraps its contents in one folder named after the
/// pack, and `SourceFiles/FBX/SM_Crate.fbx` is the same file as
/// `POLYGON Sci-Fi Space/SourceFiles/FBX/SM_Crate.fbx`. Matching on
/// component boundaries and not on raw text is what keeps `Crate.fbx`
/// from answering for `SM_Crate.fbx`.
///
/// Several members can end the same way. The shortest wins, so the copy
/// nearest the root of the archive is the one that answers, and ties go
/// alphabetically: the same manifest must resolve to the same file on
/// every machine, every run.
fn pick<'a>(members: &'a [Member], relative: &str) -> Option<&'a Member> {
    let wanted = relative.trim_start_matches('/');
    let tail = format!("/{wanted}");
    let mut best: Option<(usize, String, &Member)> = None;
    for member in members {
        let name = slashed(&member.inside);
        if name == wanted {
            return Some(member);
        }
        if !name.ends_with(&tail) {
            continue;
        }
        let rank = (name.matches('/').count(), name, member);
        if best
            .as_ref()
            .is_none_or(|had| (had.0, &had.1) > (rank.0, &rank.1))
        {
            best = Some(rank);
        }
    }
    best.map(|(_, _, member)| member)
}

/// Every raw asset archive anywhere under a pack directory. The icon, the
/// readme and the `.unitypackage` are not among them; see
/// [`archive::kind_of`].
pub fn find_archives(pack_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let _ = fsx::walk(pack_dir, &mut |path| {
        if archive::kind_of(path).is_some() {
            found.push(path.to_path_buf());
        }
    });
    found.sort();
    found
}

/// The archives in a pack directory that nothing here opens, so the
/// refusal can name one instead of reporting a missing file.
fn unreadable_archives(pack_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let _ = fsx::walk(pack_dir, &mut |path| {
        if archive::unreadable_kind(path).is_some() {
            found.push(path.to_path_buf());
        }
    });
    found.sort();
    found
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

fn count_files(dir: &Path) -> Option<usize> {
    if !dir.is_dir() {
        return None;
    }
    let mut count = 0;
    let _ = fsx::walk(dir, &mut |_| count += 1);
    Some(count)
}

/// A download sitting loose in the store root, where a directory of its
/// own should be. The commonest way to get this error, and the cheapest
/// one to fix — and the fix is no longer "unzip it": a pack directory
/// holding that same archive is read as it stands.
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
    /// The raw archives in the pack directory, all of them read.
    pub archives: Vec<PathBuf>,
    /// Why an archive could not be opened. Empty is the ordinary case,
    /// and a non-empty one outranks every other explanation: an archive
    /// nothing can open is not a missing file, it is a missing program.
    pub archive_trouble: Vec<String>,
    /// Archives in the pack directory this tool does not open at all.
    pub unreadable: Vec<PathBuf>,
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
        if !self.archives.is_empty() {
            writeln!(
                f,
                "  or        {}, inside {} in that pack",
                self.source_spelling,
                count(self.archives.len(), "archive")
            )?;
        }
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
                "  found     {} ({}), but nothing at that path",
                self.pack_dir.display(),
                count(files, "file")
            )?,
        }
        writeln!(f)?;
        self.fix(f)?;
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

impl Missing {
    /// The `fix` line: what to do next, which depends on which of the
    /// several quite different situations this is.
    fn fix(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(trouble) = self.archive_trouble.first() {
            // An archive that cannot be opened outranks everything else
            // here. The file may well be in it, and saying "not found"
            // would send somebody looking for a mesh that is right there.
            return writeln!(f, "  fix       {}", indented(trouble));
        }
        if let Some(archive) = &self.archive_beside {
            return writeln!(
                f,
                "  fix       {} has no directory of its own. Make\n            \
                 {}\n            and move it in. It does not need unzipping — this tool \
                 reads what is\n            inside it.",
                archive.display(),
                self.pack_dir.display()
            );
        }
        if self.pack_dir_files.is_none() {
            return writeln!(
                f,
                "  fix       Download \"{}\" from your Synty account's downloads and put it,\n            \
                 exactly as it arrives, at {}\n            \
                 Leave it zipped if it came zipped; the archives are read where they lie.",
                self.pack_download,
                self.pack_dir.display()
            );
        }
        if let Some(foreign) = self.unreadable.first() {
            return writeln!(
                f,
                "  fix       The pack is here and carries {}, which nothing here opens.\n            \
                 Zip and tar are read where they lie; 7-Zip and RAR are not. Extract it\n            \
                 into {}, or re-download the pack as a .zip.",
                foreign.display(),
                self.pack_dir.display()
            );
        }
        if self.unity_expected.is_none() && !self.packages.is_empty() {
            return writeln!(
                f,
                "  fix       The pack is here and does not carry that path. It does carry\n            \
                 {}; add a `unity = \"Assets/...\"` line to\n            \
                 [asset.{}] in {}, and the tree inside\n            \
                 them will be searched too.",
                count(self.packages.len(), ".unitypackage file"),
                self.id,
                self.manifest_path.display()
            );
        }
        let read = if self.archives.is_empty() {
            "The pack is here and does not carry that path."
        } else {
            "The pack is here, its archives were read where they lie, and none of\n            them carries that path either."
        };
        writeln!(
            f,
            "  fix       {read} Run\n            \
             cargo xtask art find {}\n            to see what it does carry, then correct \
             `source` on {}:{}.",
            needle(&self.source_spelling),
            self.manifest_path.display(),
            self.manifest_line
        )
    }
}

/// A borrowed complaint laid out under a `fix` label, so a multi-line
/// explanation from somewhere else keeps the column everything else here
/// is written in.
fn indented(text: &str) -> String {
    text.replace('\n', "\n            ")
}

/// The part of a path worth searching for: the file name without its
/// extension, which is what a person would type.
fn needle(source: &str) -> &str {
    let name = source.rsplit('/').next().unwrap_or(source);
    name.split('.').next().unwrap_or(name)
}

/// One thing a search found.
pub enum Hit {
    /// A file lying in the store, or already rebuilt into the cache.
    Loose(PathBuf),
    /// A member of an archive, seen without extracting anything.
    Inside { archive: PathBuf, member: String },
}

impl Hit {
    /// For sorting, so a search prints the same order twice running.
    fn key(&self) -> (PathBuf, String) {
        match self {
            Self::Loose(path) => (path.clone(), String::new()),
            Self::Inside { archive, member } => (archive.clone(), member.clone()),
        }
    }
}

/// **Everything called `needle`, including what is inside the archives.**
///
/// A pack arrives zipped, so a search that only walked the filesystem
/// would find the icon and nothing else — which is the state the owner is
/// in before this tool exists. Listing an archive reads its table of
/// contents and extracts nothing, so this stays a read.
///
/// The roots are the store and whatever the cache has already rebuilt.
pub fn search(roots: &[PathBuf], needle: &str, cache: &Cache) -> (Vec<Hit>, Vec<String>) {
    let needle = needle.to_ascii_lowercase();
    let mut hits = Vec::new();
    let mut trouble = Vec::new();
    let mut archives = Vec::new();
    let mut packages = Vec::new();
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
                hits.push(Hit::Loose(path.to_path_buf()));
            }
            if archive::kind_of(path).is_some() {
                archives.push(path.to_path_buf());
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("unitypackage"))
            {
                packages.push(path.to_path_buf());
            }
        });
    }
    archives.sort();
    packages.sort();
    for path in archives {
        match archive::list(&path) {
            Ok(members) => hits.extend(members.into_iter().filter_map(|member| {
                let name = slashed(&member.inside);
                matches(&name, &needle).then(|| Hit::Inside {
                    archive: path.clone(),
                    member: name,
                })
            })),
            Err(why) => trouble.push(why),
        }
    }
    for path in packages {
        // The tree may already be in the cache, in which case the walk
        // above has seen every one of these names and reading the archive
        // again would only print each hit twice.
        if rebuilt(cache, &path) {
            continue;
        }
        eprintln!(
            "art: reading the names inside {} (nothing is written)",
            path.display()
        );
        match unitypackage::pathnames(&path) {
            Ok(names) => hits.extend(names.into_iter().filter_map(|name| {
                matches(&name, &needle).then(|| Hit::Inside {
                    archive: path.clone(),
                    member: name,
                })
            })),
            Err(why) => trouble.push(why),
        }
    }
    hits.sort_by_key(Hit::key);
    (hits, trouble)
}

/// Whether an archive's tree has already been rebuilt into the cache,
/// under any pack. The cache files a rebuild under the archive's own file
/// name, which is what makes this answerable from the name alone.
fn rebuilt(cache: &Cache, package: &Path) -> bool {
    let Some(name) = package.file_name() else {
        return false;
    };
    std::fs::read_dir(cache.root.join("unpacked"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|pack| pack.path().join(name).is_dir())
}

/// The needle against one path: the file name, not the directories above
/// it, so searching for `crate` does not answer with every file in a
/// folder called Crates.
fn matches(name: &str, needle: &str) -> bool {
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase()
        .contains(needle)
}

/// A second pack-relative path — a texture, usually — looked for exactly
/// where the mesh was: the loose tree, then the pack's archives, then
/// whatever a `.unitypackage` rebuild has already left in the cache.
pub fn find_relative(store: &Store, cache: &Cache, pack: &Pack, relative: &str) -> Option<PathBuf> {
    let direct = store.pack_dir(pack).join(under(relative));
    if direct.is_file() {
        return Some(direct);
    }
    for archive_path in find_archives(&store.pack_dir(pack)) {
        match take_out(store, cache, pack, &archive_path, relative, true) {
            Ok(Some(path)) => return Some(path),
            Ok(None) => {}
            Err(why) => eprintln!("art: {why}"),
        }
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
        let store = Store::at(PathBuf::from("/somebody/synty"));
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

    /// **A pack that has not been downloaded is never told to unzip
    /// anything.** The store holds packs as they arrive and the archives
    /// are read where they lie, so an instruction to unzip one is an
    /// afternoon spent on a step this tool exists to delete — and it was
    /// the instruction this message used to give.
    #[test]
    fn the_download_instruction_does_not_ask_for_anything_to_be_unzipped() {
        let message = missing_message("");
        assert!(
            !message.to_ascii_lowercase().contains("unzip it"),
            "{message}"
        );
        assert!(message.contains("exactly as it arrives"), "{message}");
        assert!(message.contains("Leave it zipped"), "{message}");
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
        let store = Store::at(here.clone());
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
        assert!(message.contains("(1 file)"), "{message}");
        assert!(
            message.contains("cargo xtask art find SM_Crate"),
            "a present pack should be searched, not downloaded again:\n{message}"
        );
        assert!(!message.contains("Download \""), "{message}");
    }

    /// **A path names the member nearest the root of the archive, and
    /// names it on whole folders.** A Synty zip wraps everything in one
    /// folder named after the pack, so the path a person pastes is a tail
    /// of the member name rather than the whole of it. Matching raw text
    /// instead of whole folders would let `Crate.fbx` answer for
    /// `SM_Crate.fbx`, which is a different mesh with a plausible name;
    /// and where a pack ships the same tree twice, taking whichever the
    /// archive happened to list first would resolve one manifest to two
    /// different files on two machines.
    #[test]
    fn a_source_path_names_the_member_it_is_a_whole_tail_of() {
        let members: Vec<Member> = [
            "POLYGON Pack/Deep/Copy/SourceFiles/FBX/SM_Crate.fbx",
            "POLYGON Pack/SourceFiles/FBX/SM_Crate.fbx",
            "POLYGON Pack/SourceFiles/FBX/SM_Crate.fbx.meta",
        ]
        .into_iter()
        .map(|name| Member {
            name: name.to_owned(),
            inside: archive::inside(name).expect("a safe name"),
        })
        .collect();

        let picked = pick(&members, "SourceFiles/FBX/SM_Crate.fbx").expect("a hit");
        assert_eq!(
            picked.name, "POLYGON Pack/SourceFiles/FBX/SM_Crate.fbx",
            "the copy nearest the root of the archive answers"
        );
        assert_eq!(
            pick(&members, "SM_Crate.fbx").map(|member| member.name.as_str()),
            Some("POLYGON Pack/SourceFiles/FBX/SM_Crate.fbx"),
            "a bare file name is a whole tail"
        );
        assert!(
            pick(&members, "Crate.fbx").is_none(),
            "`Crate.fbx` answered for `SM_Crate.fbx`"
        );
        assert!(
            pick(&members, "SourceFiles/FBX/SM_Missing.fbx").is_none(),
            "a path nothing carries answered"
        );
    }
}
