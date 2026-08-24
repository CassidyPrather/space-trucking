//! Where converted art lands, and why it is not in the repository.
//!
//! Everything under the cache root is derived: the reconstructed trees a
//! `.unitypackage` gave up, the staging directory a mesh is converted
//! from, and the converted files themselves. All of it is purchased art
//! or made from it, so all of it is gitignored, and a fresh clone starts
//! with none of it and rebuilds it from `$SYNTY_STORE`.
//!
//! Converted files are addressed by **everything the conversion depended
//! on**, which is what makes "is this already converted?" answerable
//! without trusting a timestamp. The digest of the source mesh is the
//! first half of that and was for a while the whole of it — a pack update
//! changes the bytes, the bytes change the digest, the digest changes the
//! path, and the stale file is simply not where anything looks.
//!
//! The other half is [`Converted`], and it exists because the source mesh
//! stopped being the only input. A conversion now also reads the atlas
//! the manifest declared and follows the script this binary carries, so a
//! cache addressed by the mesh alone answers "already converted" about a
//! file made from a different atlas by a different script. That is not a
//! hypothetical: the first mesh this pipeline converted against a real
//! pack came out colourless, and a cache keyed on the source alone would
//! have gone on serving that colourless file for ever, with the fix
//! installed and nothing on disk to say why.

use std::path::{Path, PathBuf};

use crate::convert::SCRIPT;
use crate::sha256;

pub struct Cache {
    pub root: PathBuf,
}

/// **How one conversion is addressed**: the digest of the source mesh,
/// and a short digest of everything else that conversion depended on.
///
/// Both halves are in the file name on purpose. The source digest is the
/// part a person can check by hand — it is the number the manifest
/// carries and the number the index records — and the recipe beside it is
/// the part that answers "and made how?". A name that were only the
/// recipe would be a cache nobody could read; a name that were only the
/// source is the cache this type replaced.
pub struct Converted {
    source: String,
    recipe: String,
}

/// **What a conversion is, as a number.** Bumped by hand when the
/// resolver changes what it hands the converter without the script or the
/// inputs changing — the arguments it passes and the files it stages are
/// as much a part of a conversion as the program that reads them, and
/// nothing else here would notice them moving.
const RECIPE: u32 = 1;

/// How much of the recipe digest goes in the file name. Twelve hex digits
/// distinguish the handful of recipes one machine will ever hold, and
/// nothing here is defending against somebody choosing a collision: the
/// question this answers is "is this the same recipe?", asked of files
/// this tool wrote itself.
const RECIPE_SHOWN: usize = 12;

impl Converted {
    /// The identity of converting `source` with `texture` beside it,
    /// under the script this binary carries.
    pub fn of(source: &str, texture: Option<&str>) -> Self {
        Self::under(source, texture, SCRIPT)
    }

    /// The same, with the script spelled out, so the guard below can ask
    /// what a different one would be addressed as. Nothing else should
    /// need it: there is exactly one script and it is compiled in.
    fn under(source: &str, texture: Option<&str>, script: &str) -> Self {
        // A recipe is a few lines of text rather than a struct with a
        // hash of its own, because the thing that has to stay stable is
        // what the bytes ARE — a cache entry written by one build of this
        // tool is read by the next one, and a derived hash is a promise
        // about a data structure rather than about a file name.
        let recipe = sha256::of_bytes(
            format!(
                "art conversion recipe {RECIPE}\n\
                 source {source}\n\
                 texture {}\n\
                 script {}\n",
                texture.unwrap_or("none"),
                sha256::of_bytes(script.as_bytes()),
            )
            .as_bytes(),
        );
        Self {
            source: source.to_owned(),
            recipe: recipe[..RECIPE_SHOWN].to_owned(),
        }
    }

    /// The converted file, relative to the cache root, with `/`
    /// separators so the index reads the same on every platform.
    pub fn relative(&self) -> String {
        format!("glb/{}.glb", self.stem())
    }

    fn stem(&self) -> String {
        format!("{}-{}", self.source, self.recipe)
    }
}

impl Cache {
    /// `$ART_CACHE` if it is set, otherwise `art/cache` beside the
    /// manifest. The override is for a machine that would rather not put
    /// several gigabytes of unpacked art on the same disk as the source.
    pub fn open(repo: &Path) -> Self {
        let root = std::env::var_os("ART_CACHE")
            .map_or_else(|| repo.join("art").join("cache"), PathBuf::from);
        Self { root }
    }

    /// The tree rebuilt out of one pack's `.unitypackage` files, one
    /// directory per package so a pack shipping several stays legible.
    pub fn unpacked(&self, pack: &str) -> PathBuf {
        self.root.join("unpacked").join(pack)
    }

    /// Where a mesh and the textures it needs are gathered before the
    /// converter is pointed at them. Kept after a run on purpose: when a
    /// conversion comes out wrong, this is the directory to open.
    pub fn stage(&self, digest: &str) -> PathBuf {
        self.root.join("stage").join(digest)
    }

    /// The converted file. Its name is the whole of how a run answers
    /// "is there anything to do?", so it is [`Converted`] and not a
    /// digest: a file made from another atlas, or by another script, is
    /// a file at another path and is never mistaken for this one.
    pub fn glb(&self, converted: &Converted) -> PathBuf {
        self.root.join(converted.relative())
    }

    /// **What the converter measured, kept beside what it wrote.**
    ///
    /// The measurement is the fact the manifest's `fill` promise is
    /// checked against, and the check has to hold on every run rather
    /// than on the run that happened to convert. Converting again to
    /// re-measure would cost a Blender launch per asset to learn a number
    /// that cannot have changed — the file it describes is addressed by
    /// everything that made it — so the number is filed under that same
    /// name and read back.
    pub fn bounds(&self, converted: &Converted) -> PathBuf {
        self.root
            .join("glb")
            .join(format!("{}.aabb", converted.stem()))
    }

    pub fn index(&self) -> PathBuf {
        self.root.join("index.toml")
    }

    /// The converter script, written out beside the cache. It is carried
    /// in the binary rather than read from the source tree so that where
    /// the tool is run from stops mattering.
    pub fn blender_script(&self) -> PathBuf {
        self.root.join("blender").join("fbx_to_gltf.py")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "aae840d30cdda45baa5fa80367f754ed7ab92cc6c59c2beb4e8481a044f5f0cb";

    /// **Everything a conversion reads is in the name of what it wrote.**
    ///
    /// The source mesh was the whole of the old answer, and it was the
    /// whole of the old defect: the atlas the manifest declares and the
    /// script this binary carries are both inputs, and a cache that
    /// cannot tell them apart hands back a file made from neither. The
    /// owner's crate came out colourless for exactly that reason, and the
    /// run after the fix would have said "already converted" and changed
    /// nothing.
    #[test]
    fn a_conversion_is_addressed_by_everything_that_made_it() {
        let plain = Converted::of(SOURCE, None);
        let painted = Converted::of(SOURCE, Some("f".repeat(64).as_str()));
        let repainted = Converted::of(SOURCE, Some("e".repeat(64).as_str()));
        let elsewhere = Converted::of(&"b".repeat(64), None);
        let rewritten = Converted::under(SOURCE, None, "a script that says something else");

        let names = [
            plain.relative(),
            painted.relative(),
            repainted.relative(),
            elsewhere.relative(),
            rewritten.relative(),
        ];
        for (one, name) in names.iter().enumerate() {
            for other in &names[one + 1..] {
                assert_ne!(name, other, "two different conversions, one file name");
            }
        }
    }

    /// **The same conversion is the same name, run after run.** The whole
    /// value of the cache is that a second `resolve` converts nothing and
    /// needs no converter at all to work that out, and a name with
    /// anything of this machine or this moment in it would convert
    /// everything, every time.
    #[test]
    fn the_same_conversion_is_the_same_name_twice_running() {
        assert_eq!(
            Converted::of(SOURCE, Some("f".repeat(64).as_str())).relative(),
            Converted::of(SOURCE, Some("f".repeat(64).as_str())).relative()
        );
    }

    /// **The source digest is still legible in the name.** It is the
    /// number the manifest carries, the number the index records and the
    /// number `sha256sum` prints, so a person looking at the cache can
    /// still tell which mesh a file came out of without running anything.
    #[test]
    fn the_name_still_says_which_mesh_it_came_out_of() {
        let name = Converted::of(SOURCE, None).relative();
        let recipe = name
            .strip_prefix(&format!("glb/{SOURCE}-"))
            .and_then(|rest| rest.strip_suffix(".glb"))
            .unwrap_or_else(|| panic!("{name} does not say which mesh it came out of"));
        assert_eq!(recipe.len(), RECIPE_SHOWN, "{name}");
    }
}
