//! Where converted art lands, and why it is not in the repository.
//!
//! Everything under the cache root is derived: the reconstructed trees a
//! `.unitypackage` gave up, the staging directory a mesh is converted
//! from, and the converted files themselves. All of it is purchased art
//! or made from it, so all of it is gitignored, and a fresh clone starts
//! with none of it and rebuilds it from `$SYNTY_STORE`.
//!
//! Converted files are addressed by the digest of the source they came
//! from. That is what makes "is this already converted?" answerable
//! without trusting a timestamp: a pack update changes the bytes, the
//! bytes change the digest, the digest changes the path, and the stale
//! file is simply not where anything looks.

use std::path::{Path, PathBuf};

pub struct Cache {
    pub root: PathBuf,
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

    pub fn glb(&self, digest: &str) -> PathBuf {
        self.root.join("glb").join(format!("{digest}.glb"))
    }

    /// **What the converter measured, kept beside what it wrote.**
    ///
    /// The measurement is the fact the manifest's `fill` promise is
    /// checked against, and the check has to hold on every run rather
    /// than on the run that happened to convert. Converting again to
    /// re-measure would cost a Blender launch per asset to learn a number
    /// that cannot have changed — the file it describes is addressed by
    /// the digest of its own source — so the number is filed under that
    /// same digest and read back.
    pub fn bounds(&self, digest: &str) -> PathBuf {
        self.root.join("glb").join(format!("{digest}.aabb"))
    }

    /// The same path the index writes down, so the two cannot disagree.
    pub fn glb_relative(digest: &str) -> String {
        format!("glb/{digest}.glb")
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
