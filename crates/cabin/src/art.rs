//! **Purchased art: the declaration, and the loading of it.**
//!
//! The game has two graphical implementations of every object planned:
//! the whitebox this repository cuts in code, and a bought asset. This
//! module is the seam. It has two halves and they are gated differently
//! on purpose.
//!
//! **The declaration half is always here.** `art/manifest.toml` is in
//! the repository — it is the one part of the pipeline the licence lets
//! be public — and every asset in it may carry a `dresses` line saying
//! which body of the game it stands in for, plus the four numbers saying
//! how it sits in that body's box. Those are *promises*, they live in
//! git, and the gauntlet sweeps them in continuous integration
//! (`crate::gauntlet`, and docs/GAUNTLET.md's fill family). This has to
//! work in the default build, because **the build that can draw the mesh
//! is the build continuous integration cannot run**: the payload is not
//! in the repository and never will be. So the promise is what CI can
//! check, and CI checks it whether or not the feature is on.
//!
//! **The loading half is behind `--features art`.** That is where
//! `bevy_gltf` comes in, and where `$ART_CACHE/index.toml`
//! — written by `cargo xtask art resolve` on the machine of somebody who
//! holds the licence — is read at boot. Everything about it fails soft:
//! no cache, no entry, an entry that will not parse, a file that is not
//! there, and the whitebox stands in, because a game that will not start
//! because somebody moved a directory is worse than a game with grey
//! boxes in it. What it never does is put a word on the screen; the
//! zero-text law covers what is drawn, and a complaint belongs on stderr.
//!
//! # The placement frame, which is an API
//!
//! A cargo kind's description claims a box — its `Kind::upright` cells
//! across and up, and the one cell of depth every rig is composed within
//! (`pieces::RIG_NEAR..RIG_FAR`). **That box is `[-1, 1]` on every axis
//! of the placement frame**, which is the same normalised frame
//! `poi::Fitting` states a station's hardware in, one box down. In it:
//!
//! | | what it means |
//! | --- | --- |
//! | `scale` | the converted file's own units, carried into berth half-units |
//! | `rotation` | degrees about x, then y, then z, taken in the box's own axes |
//! | `offset` | where the body's middle sits, in berth half-units — `poi::Fitting`'s `at` |
//! | `fill` | what the body then occupies of the box, per axis — `poi::Fitting`'s `half`, and `poi::Shape::fill`'s meaning |
//!
//! In that order: the mesh is centred on its own measured middle, scaled,
//! turned, and set down at the offset. `|offset| + fill <= 1` on an axis
//! is exactly "the body stays inside its berth", and it is not enforced
//! here — a body that leaves its berth is a finding, and the gauntlet's
//! families already know how to say so.
//!
//! `scale` and `fill` are deliberately **redundant**, and the redundancy
//! is the mechanism: `fill` is a promise living in the repository, and
//! `scale` times the mesh's own measured size is the fact. `cargo xtask
//! art resolve` is where the two are made to meet.
//!
//! Two consequences worth knowing before writing numbers. A berth box is
//! a cube for every one-cell kind and 1:2:1 for a `1×2` one, so the
//! placement frame is anisotropic on the tall kinds: a rotation that is
//! not a quarter turn shears a body there, and a per-axis `scale` is how
//! to answer it. And the numbers the game draws are the *index's*, not
//! the manifest's — an edit to the manifest reaches the game through
//! `resolve`, which is also the only moment it is checked.

use std::sync::OnceLock;

use bevy::prelude::*;
use space_trucking::sim::cargo::KIND_COUNT;
use space_trucking::sim::{Kind, layout};

/// The manifest as it stands in the repository, read at compile time.
///
/// Compiled in rather than read off a disk because the thing that needs
/// it most is a sweep that runs in continuous integration on a machine
/// with no art on it, and because a promise that can go missing between
/// the build and the run is not a promise. Editing the manifest rebuilds
/// the cabin, which is correct: the docket depends on it.
const SHIPPED: &str = include_str!("../../../art/manifest.toml");

/// **One purchased body, as declared**: which kind it dresses, and the
/// four numbers that put it in that kind's box.
#[derive(Clone, Debug, PartialEq)]
pub struct Dressing {
    /// The asset's stable id, for saying which line an answer came from.
    pub id: String,
    /// The converted file, relative to the cache root. Only the index
    /// carries one; a manifest declares no such thing.
    pub glb: Option<String>,
    /// The converted file's own units, in berth half-units.
    pub scale: Vec3,
    /// Where the body's middle sits, in berth half-units.
    pub offset: Vec3,
    /// Degrees about x, then y, then z.
    pub rotation: Vec3,
    /// What fraction of the berth box the body occupies, per axis.
    pub fill: Vec3,
    /// The tight box the converter found round the mesh, in the
    /// converted file's own units. `None` where nothing measured it, in
    /// which case the mesh is placed on its own origin rather than on
    /// its own middle — the only honest thing to do with a body whose
    /// size nobody knows.
    pub measured: Option<(Vec3, Vec3)>,
}

impl Dressing {
    /// **The box a kind's own description claims**, as a `(middle, half)`
    /// in the rig's local sim units: its cells across and up, and the one
    /// cell of depth every rig is composed within.
    ///
    /// This is the same box `pieces::drawn_box` falls back to for a kind
    /// that draws nothing at all, and it is stated once here so a
    /// purchased body and a whitebox one are measured off one claim.
    #[must_use]
    pub fn berth_box(kind: Kind) -> (Vec3, Vec3) {
        let (w, h) = kind.upright();
        (
            Vec3::new(0.0, 0.0, crate::pieces::rig_mid()),
            Vec3::new(
                f32::from(w) * layout::CELL * 0.5,
                f32::from(h) * layout::CELL * 0.5,
                (crate::pieces::RIG_FAR - crate::pieces::RIG_NEAR) * 0.5,
            ),
        )
    }

    /// The turn the declaration asks for.
    #[must_use]
    pub fn turn(&self) -> Quat {
        Quat::from_euler(
            EulerRot::XYZ,
            self.rotation.x.to_radians(),
            self.rotation.y.to_radians(),
            self.rotation.z.to_radians(),
        )
    }

    /// **Where the purchased scene stands**, in the rig's own local sim
    /// units — the transform a `SceneRoot` is spawned under.
    ///
    /// The three overrides, folded into one transform: the mesh is
    /// carried off its own measured middle, scaled out of its own units
    /// into the berth's, turned, and set down at the offset.
    ///
    /// Unused in a whitebox build, which is every build made from this
    /// repository alone: the thing that calls it is `pieces::build_kind`
    /// under `--features art`, and the numbers it reads come from a file
    /// only a licence holder can write.
    #[must_use]
    #[cfg_attr(not(feature = "art"), allow(dead_code))]
    pub fn pose(&self, kind: Kind) -> Transform {
        let (mid, half) = Self::berth_box(kind);
        let turn = self.turn();
        let scale = self.scale * half;
        let recentre = self.measured.map_or(Vec3::ZERO, |(measured, _)| measured);
        Transform {
            translation: mid + self.offset * half - turn * (scale * recentre),
            rotation: turn,
            scale,
        }
    }

    /// **The box the harness measures and the aim meets**, as a
    /// `(middle, half)` in the rig's own local sim units.
    ///
    /// The declared body, not the frame it is drawn in — which is the
    /// whole of what `fill` is for. The turn is carried onto it the same
    /// way `pieces::drawn_box` carries a part's: the axis-aligned bound
    /// of the turned box, which is exact on a quarter turn and a hair
    /// generous otherwise.
    #[must_use]
    pub fn fill_box(&self, kind: Kind) -> (Vec3, Vec3) {
        let (mid, half) = Self::berth_box(kind);
        let body = (self.fill * half).abs();
        let m = Mat3::from_quat(self.turn());
        let reach = m.x_axis.abs() * body.x + m.y_axis.abs() * body.y + m.z_axis.abs() * body.z;
        (mid + self.offset * half, reach)
    }
}

/// **Every kind something is declared to dress**, by kind index.
///
/// An array rather than a map: there are thirty-two kinds, the sweep asks
/// about each of them many times over, and `Kind::index` is already the
/// stable number the save format is written in.
#[derive(Debug, Default)]
pub struct Dressings {
    by_kind: [Option<Dressing>; KIND_COUNT],
    /// Bindings that named a body this game does not have, kept rather
    /// than dropped so a guard can be about them. At runtime they are
    /// simply not drawn.
    pub strangers: Vec<String>,
}

impl Dressings {
    /// What the kind is dressed in, if anything.
    #[must_use]
    pub const fn of(&self, kind: Kind) -> Option<&Dressing> {
        self.by_kind[kind.index()].as_ref()
    }

    /// Whether anything at all is dressed. The answer is no in every
    /// build this repository can make on its own, and the sweep leans on
    /// that: a manifest with no `dresses` line in it changes nothing.
    /// Read by the guards that hold that claim, and by nothing else.
    #[must_use]
    #[allow(dead_code)]
    pub fn any(&self) -> bool {
        self.by_kind.iter().any(Option::is_some)
    }

    /// **What the manifest in this repository declares.** Parsed once.
    ///
    /// A manifest that will not parse answers as an empty set rather than
    /// panicking. The resolver is where a malformed manifest is a
    /// refusal, with the line in it; this is a reader, and a reader that
    /// brings the game down over a file the game does not need is a
    /// worse citizen than one that draws the whitebox.
    #[must_use]
    pub fn shipped() -> &'static Self {
        static ONCE: OnceLock<Dressings> = OnceLock::new();
        ONCE.get_or_init(|| Self::read(SHIPPED).unwrap_or_default())
    }

    /// Read a manifest or an index — they are one dialect, and which
    /// keys are present is the only difference between them.
    ///
    /// # Errors
    /// The line that does not read, and why.
    pub fn read(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for table in tables(text)? {
            if table.table != "asset" {
                continue;
            }
            let Some(binding) = table.string("dresses") else {
                continue;
            };
            let Some(name) = binding.strip_prefix("cargo/") else {
                // A namespace this build has no bodies for. The resolver
                // refuses one it has never heard of; one it knows and
                // this does not is a build that is simply older.
                out.strangers.push(binding.to_owned());
                continue;
            };
            let Some(kind) = kind_named(name) else {
                out.strangers.push(binding.to_owned());
                continue;
            };
            let mid = table.triple("measured_mid");
            let half = table.triple("measured_half");
            out.by_kind[kind.index()] = Some(Dressing {
                id: table.id.clone(),
                glb: table.string("glb").map(str::to_owned),
                scale: table.triple("scale").unwrap_or(Vec3::ONE),
                offset: table.triple("offset").unwrap_or(Vec3::ZERO),
                rotation: table.triple("rotation").unwrap_or(Vec3::ZERO),
                fill: table.triple("fill").unwrap_or(Vec3::ONE),
                measured: mid.zip(half),
            });
        }
        Ok(out)
    }
}

/// **Which cargo kind a `dresses` name means**, derived from the kind's
/// own spelling rather than looked up in a second table.
///
/// A table would be thirty-two lines that have to be kept in step with
/// `Kind::ALL` by hand, and the day one falls out of step is the day a
/// manifest line silently dresses the wrong crate. `Kind`'s own `Debug`
/// is the spelling, and snake case is the spelling of it a person types.
#[must_use]
pub fn kind_named(name: &str) -> Option<Kind> {
    Kind::ALL.into_iter().find(|kind| snake(*kind) == name)
}

/// One kind's name in a manifest: `VeryMysteriousCrate` as
/// `very_mysterious_crate`.
#[must_use]
pub fn snake(kind: Kind) -> String {
    let mut out = String::new();
    for letter in format!("{kind:?}").chars() {
        if letter.is_ascii_uppercase() && !out.is_empty() {
            out.push('_');
        }
        out.push(letter.to_ascii_lowercase());
    }
    out
}

// ------------------------------------------------------------ the dialect --

/// One `[table.id]` and the keys under it.
///
/// The reader is here rather than shared with `xtask` because the two
/// crates cannot see each other — the resolver has no dependencies at all
/// and the cabin has Bevy — and because eighty lines of reader is a
/// smaller thing to carry than a TOML crate in either graph. The dialect
/// is documented once, in `xtask/src/manifest.rs`.
#[allow(clippy::struct_field_names)]
struct Table {
    table: String,
    id: String,
    keys: Vec<(String, Value)>,
}

enum Value {
    Str(String),
    Triple(Vec3),
}

impl Table {
    fn get(&self, key: &str) -> Option<&Value> {
        self.keys
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, v)| v)
    }

    fn string(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(value) => Some(value),
            Value::Triple(_) => None,
        }
    }

    fn triple(&self, key: &str) -> Option<Vec3> {
        match self.get(key)? {
            Value::Triple(value) => Some(*value),
            Value::Str(_) => None,
        }
    }
}

fn tables(text: &str) -> Result<Vec<Table>, String> {
    let mut out: Vec<Table> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('[') {
            let header = header
                .strip_suffix(']')
                .ok_or_else(|| format!("{line}: `{trimmed}` never closes its table"))?;
            let (table, id) = header
                .split_once('.')
                .ok_or_else(|| format!("{line}: `[{header}]` is not `[<table>.<id>]`"))?;
            out.push(Table {
                table: table.to_owned(),
                id: id.to_owned(),
                keys: Vec::new(),
            });
            continue;
        }
        let (key, rest) = trimmed
            .split_once('=')
            .ok_or_else(|| format!("{line}: `{trimmed}` is not a `key = value`"))?;
        let holder = out
            .last_mut()
            .ok_or_else(|| format!("{line}: `{trimmed}` comes before any table"))?;
        let value = read_value(rest.trim()).map_err(|why| format!("{line}: {why}"))?;
        holder.keys.push((key.trim().to_owned(), value));
    }
    Ok(out)
}

fn read_value(text: &str) -> Result<Value, String> {
    if let Some(rest) = text.strip_prefix('"') {
        let end = rest
            .find('"')
            .ok_or_else(|| format!("`{text}` never closes its string"))?;
        return Ok(Value::Str(rest[..end].to_owned()));
    }
    let rest = text
        .strip_prefix('[')
        .ok_or_else(|| format!("`{text}` is neither a string nor three numbers"))?;
    let end = rest
        .find(']')
        .ok_or_else(|| format!("`{text}` never closes its array"))?;
    let numbers: Vec<f32> = rest[..end]
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .map_err(|_| format!("`{}` is not a number", part.trim()))
        })
        .collect::<Result<_, _>>()?;
    let triple: [f32; 3] = numbers
        .try_into()
        .map_err(|got: Vec<f32>| format!("`{text}` has {} numbers, not three", got.len()))?;
    Ok(Value::Triple(Vec3::from(triple)))
}

// ------------------------------------------------------- the loading half --

#[cfg(feature = "art")]
pub use loading::{Dressed, cache_root, plugin};

#[cfg(feature = "art")]
mod loading {
    use std::path::PathBuf;

    use bevy::prelude::*;
    use bevy::world_serialization::WorldAsset;
    use space_trucking::sim::Kind;
    use space_trucking::sim::cargo::KIND_COUNT;

    use super::{Dressing, Dressings};

    /// **Where the resolved art is**: `$ART_CACHE` if it is set, and
    /// `art/cache` beside wherever the game was started otherwise —
    /// exactly the two places `cargo xtask art resolve` writes to, said
    /// the same way so the two cannot drift.
    #[must_use]
    pub fn cache_root() -> PathBuf {
        std::env::var_os("ART_CACHE")
            .map_or_else(|| PathBuf::from("art").join("cache"), PathBuf::from)
    }

    /// **Where this run's resolved art is.** A resource rather than a
    /// call to [`cache_root`] at the point of use, because a guard
    /// cannot set an environment variable: this workspace forbids
    /// `unsafe`, `std::env::set_var` is unsafe, and a seam nothing can
    /// point at a fixture is a seam nothing can test.
    #[derive(Resource, Debug, Clone)]
    pub struct Cache(pub PathBuf);

    impl Default for Cache {
        fn default() -> Self {
            Self(cache_root())
        }
    }

    /// **What the game will draw instead of the whitebox**: one loaded
    /// scene per dressed kind, with the declaration that places it.
    ///
    /// The resource is inserted whether or not anything resolved, so the
    /// systems that read it need no `Option` and the "no art on this
    /// machine" case is an empty table rather than an absent one.
    #[derive(Resource, Default)]
    pub struct Dressed {
        scenes: [Option<(Handle<WorldAsset>, Dressing)>; KIND_COUNT],
    }

    impl Dressed {
        /// The scene and the numbers for one kind, or nothing — which is
        /// the answer for every kind in a build with no cache under it.
        #[must_use]
        pub fn of(&self, kind: Kind) -> Option<(&Handle<WorldAsset>, &Dressing)> {
            self.scenes[kind.index()]
                .as_ref()
                .map(|(scene, dressing)| (scene, dressing))
        }
    }

    /// Read the index at boot and ask for every scene it names.
    ///
    /// **Every way this can go wrong ends in the whitebox.** No cache
    /// directory, no index, an index that will not parse, an entry
    /// naming a file that is not there: each one leaves the kind
    /// undressed and puts a sentence on stderr. The game is playable
    /// without any of this and always will be — that is the whole point
    /// of cutting the geometry in code — so nothing here is worth a
    /// panic, and nothing here is worth a word on screen either.
    pub fn plugin(app: &mut App) {
        app.init_resource::<Cache>()
            .init_resource::<Dressed>()
            .add_systems(Startup, load_index);
    }

    pub(super) fn load_index(
        cache: Res<Cache>,
        assets: Res<AssetServer>,
        mut dressed: ResMut<Dressed>,
    ) {
        let index = cache.0.join("index.toml");
        let Ok(text) = std::fs::read_to_string(&index) else {
            eprintln!(
                "art: no {} — drawing the whitebox. `cargo xtask art resolve` writes one.",
                index.display()
            );
            return;
        };
        let declared = match Dressings::read(&text) {
            Ok(declared) => declared,
            Err(why) => {
                eprintln!(
                    "art: {} does not read ({why}) — drawing the whitebox. It is written \
                     by `cargo xtask art resolve` and rewritten by the next one.",
                    index.display()
                );
                return;
            }
        };
        for stranger in &declared.strangers {
            eprintln!(
                "art: {} dresses `{stranger}`, which this build has no body for",
                index.display()
            );
        }
        for kind in Kind::ALL {
            let Some(dressing) = declared.of(kind) else {
                continue;
            };
            let Some(glb) = &dressing.glb else {
                eprintln!(
                    "art: `{}` dresses {kind:?} and names no converted file — drawing the \
                     whitebox",
                    dressing.id
                );
                continue;
            };
            // The cache root is the asset root under this feature, so a
            // path out of the index is a path the server can take
            // verbatim. `#Scene0` is `GltfAssetLabel::Scene(0)`, glTF's
            // first scene, which is the one a single-object export has.
            dressed.scenes[kind.index()] =
                Some((assets.load(format!("{glb}#Scene0")), dressing.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A unit cube as a binary glTF**, written here byte by byte.
    ///
    /// There is no purchased mesh in this repository and there never will
    /// be, so the only way to prove that the loading path loads anything
    /// is to write a file it has to accept. A `.glb` is a twelve-byte
    /// header and two chunks — the JSON that describes the scene, and the
    /// buffer the vertex data lives in — and a cube with positions and
    /// indices is the smallest thing that exercises the whole of it:
    /// header, chunk framing, accessors, a buffer view, a mesh, a node
    /// and a scene.
    ///
    /// Byte by byte rather than through a crate for the reason the zip
    /// fixture in `xtask/tests/pipeline.rs` is: a fixture built by the
    /// same library the code under test uses proves that the library
    /// agrees with itself.
    #[cfg(feature = "art")]
    fn unit_cube_glb() -> Vec<u8> {
        // Eight corners of a cube half a unit each way, so the tight box
        // round it is exactly `[-0.5, 0.5]` — the number the index
        // fixture below declares it measured.
        // Twelve triangles, two per face, wound so the outside faces
        // out. The corner index is a bit per axis: 1 is +x, 2 is +y,
        // 4 is +z.
        const FACES: [[u16; 4]; 6] = [
            [0, 2, 3, 1], // -z
            [5, 7, 6, 4], // +z
            [4, 6, 2, 0], // -x
            [1, 3, 7, 5], // +x
            [0, 1, 5, 4], // -y
            [2, 6, 7, 3], // +y
        ];
        let mut bin: Vec<u8> = Vec::new();
        for z in [-0.5_f32, 0.5] {
            for y in [-0.5_f32, 0.5] {
                for x in [-0.5_f32, 0.5] {
                    for value in [x, y, z] {
                        bin.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        for [a, b, c, d] in FACES {
            for corner in [a, b, c, a, c, d] {
                bin.extend_from_slice(&corner.to_le_bytes());
            }
        }
        let positions = 8 * 3 * 4;
        let indices = bin.len() - positions;
        let json = format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\
             \"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],\
             \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\
             \"indices\":1}}]}}],\
             \"accessors\":[\
             {{\"bufferView\":0,\"componentType\":5126,\"count\":8,\"type\":\"VEC3\",\
             \"min\":[-0.5,-0.5,-0.5],\"max\":[0.5,0.5,0.5]}},\
             {{\"bufferView\":1,\"componentType\":5123,\"count\":36,\"type\":\"SCALAR\"}}],\
             \"bufferViews\":[\
             {{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{positions},\"target\":34962}},\
             {{\"buffer\":0,\"byteOffset\":{positions},\"byteLength\":{indices},\
             \"target\":34963}}],\
             \"buffers\":[{{\"byteLength\":{}}}]}}",
            bin.len()
        );
        // Both chunks are padded to four bytes — JSON with spaces and the
        // buffer with zeros, which is what the specification asks for and
        // what every reader checks.
        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2_u32.to_le_bytes());
        let total = 12 + 8 + json.len() + 8 + bin.len();
        out.extend_from_slice(&u32::try_from(total).expect("a small fixture").to_le_bytes());
        out.extend_from_slice(&u32::try_from(json.len()).expect("small").to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&u32::try_from(bin.len()).expect("small").to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    /// **A converted mesh in the cache is loaded, and the whitebox is
    /// what happens when it is not.**
    ///
    /// The end of the loading path, proved against a file this test
    /// wrote. What it establishes is exactly three things and no more:
    /// that the feature's Bevy list can decode a binary glTF at all, that
    /// the index this repository's own resolver writes is read back into
    /// a handle for the kind it names, and that every way the cache can
    /// be missing or wrong leaves the kind undressed instead of bringing
    /// the game down.
    ///
    /// What it does not prove, and cannot: that a real Synty FBX comes
    /// out of Blender looking right. That needs the owner's disk, and
    /// `docs/ART_PIPELINE.md` says which command closes it.
    #[cfg(feature = "art")]
    #[test]
    fn a_converted_mesh_in_the_cache_is_what_a_dressed_kind_draws() {
        use bevy::asset::{AssetPlugin, LoadState};
        use bevy::world_serialization::{WorldAsset, WorldSerializationPlugin};

        let dir = std::env::temp_dir().join("space-trucking-art-cabin-load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("glb")).expect("a scratch cache");
        std::fs::write(dir.join("glb/cube.glb"), unit_cube_glb()).expect("a fixture mesh");
        std::fs::write(
            dir.join("index.toml"),
            "[asset.crate_small]\nglb = \"glb/cube.glb\"\n\
             sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
             dresses = \"cargo/suspicious_crate\"\n\
             scale = [2.0, 2.0, 2.0]\noffset = [0.0, -0.5, 0.0]\n\
             rotation = [0.0, 90.0, 0.0]\nfill = [1.0, 1.0, 1.0]\n\
             measured_mid = [0.0, 0.0, 0.0]\nmeasured_half = [0.5, 0.5, 0.5]\n",
        )
        .expect("a fixture index");

        // The smallest app that can read a glTF: a task pool for the
        // load to run on, the asset server, the two asset kinds a mesh
        // becomes, and the loader itself. `finish` is not optional —
        // `GltfPlugin` only registers its loader there, and an app that
        // never finishes waits for a loader that was never installed.
        let stand = |root: &std::path::Path| {
            let mut app = App::new();
            app.add_plugins((
                bevy::app::TaskPoolPlugin::default(),
                AssetPlugin {
                    file_path: root.display().to_string(),
                    ..default()
                },
                WorldSerializationPlugin,
                bevy::mesh::MeshPlugin,
                bevy::gltf::GltfPlugin::default(),
            ))
            .insert_resource(super::loading::Cache(root.to_path_buf()));
            plugin(&mut app);
            app.finish();
            app.cleanup();
            app
        };

        let mut app = stand(&dir);
        app.update();
        let dressed = app.world().resource::<Dressed>();
        let (handle, dressing) = dressed
            .of(Kind::SuspiciousCrate)
            .expect("the index dresses the suspicious crate");
        let handle = handle.clone();
        // The numbers came out of the index and not out of a default,
        // and the pose the scene is spawned under is the one they make:
        // twice the berth box, a quarter turn about its own up, and half
        // a half-box down — which is what `build_kind` hands the
        // `WorldAssetRoot` it spawns in place of the whitebox parts.
        assert_eq!(dressing.scale, Vec3::splat(2.0));
        assert_eq!(dressing.rotation, Vec3::new(0.0, 90.0, 0.0));
        let pose = dressing.pose(Kind::SuspiciousCrate);
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        assert!(
            (pose.scale - half * 2.0).length() < 1e-4,
            "{:?}",
            pose.scale
        );
        assert!(
            (pose.translation - (mid - Vec3::Y * (half.y * 0.5))).length() < 1e-4,
            "{:?}",
            pose.translation
        );
        assert!(
            (pose.rotation * Vec3::Z - Vec3::X).length() < 1e-4,
            "a quarter turn about up did not carry the body's face onto +x"
        );
        assert!(dressed.of(Kind::Couch).is_none(), "an unnamed kind is bare");

        // Asset loading is asynchronous, so the frames are pumped until
        // the server has an answer either way rather than once.
        let mut state = LoadState::NotLoaded;
        for _ in 0..10_000 {
            app.update();
            state = app
                .world()
                .resource::<AssetServer>()
                .get_load_state(&handle)
                .unwrap_or(LoadState::NotLoaded);
            if matches!(state, LoadState::Loaded | LoadState::Failed(_)) {
                break;
            }
        }
        assert!(
            matches!(state, LoadState::Loaded),
            "the cabin could not load a glTF it wrote itself: {state:?}"
        );
        assert_eq!(
            app.world().resource::<Assets<WorldAsset>>().len(),
            1,
            "the scene the mesh describes did not become an asset"
        );

        // And every way the cache can be absent or wrong leaves the kind
        // undressed rather than bringing the game down.
        for (what, root) in [
            ("a cache that is not there", dir.join("nowhere")),
            ("a cache with no index in it", dir.join("glb")),
        ] {
            let mut app = stand(&root);
            app.update();
            assert!(
                app.world()
                    .resource::<Dressed>()
                    .of(Kind::SuspiciousCrate)
                    .is_none(),
                "{what} dressed something"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Every `dresses` line in the shipped manifest names a body this
    /// game actually has.**
    ///
    /// The resolver checks the shape of a binding and deliberately not
    /// its meaning: `xtask` cannot see a `cargo::Kind` and should not
    /// learn to. So this is the other half of that check, and it lives
    /// here because here is where the bodies are.
    ///
    /// **It passes vacuously today**, because the manifest declares no
    /// bindings at all, and a guard that can only pass is not a guard.
    /// What keeps it honest is the pair below it: the same reader, over a
    /// manifest with a name nobody has, has to fail.
    #[test]
    fn every_dressed_name_in_the_manifest_is_a_body_this_game_has() {
        let declared = Dressings::shipped();
        assert!(
            declared.strangers.is_empty(),
            "art/manifest.toml dresses {:?}, and this game has no such body. \
             The names it does have are: {}",
            declared.strangers,
            Kind::ALL
                .into_iter()
                .map(snake)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    /// **A binding that names nothing is caught, and a binding that
    /// names something is read.** The non-vacuity of the guard above:
    /// the shipped manifest cannot exercise either branch, so a manifest
    /// written here does.
    #[test]
    fn a_binding_naming_no_body_this_game_has_is_a_stranger() {
        let one = |dresses: &str| {
            Dressings::read(&format!(
                "[asset.crate_small]\nsource = \"a.fbx\"\ndresses = \"{dresses}\"\n"
            ))
            .expect("a manifest in the dialect")
        };
        let stranger = one("cargo/crate_of_holding");
        assert_eq!(stranger.strangers, vec!["cargo/crate_of_holding"]);
        assert!(!stranger.any());

        let known = one("cargo/suspicious_crate");
        assert!(known.strangers.is_empty(), "{:?}", known.strangers);
        assert_eq!(
            known.of(Kind::SuspiciousCrate).map(|one| one.id.as_str()),
            Some("crate_small")
        );

        // A namespace the resolver would refuse outright, which a build
        // older than the namespace still has to survive reading.
        assert_eq!(one("fitting/beacon").strangers, vec!["fitting/beacon"]);
    }

    /// **A kind's manifest name is its own spelling, in snake case.**
    /// The mapping is derived and not tabled, so this asks the derivation
    /// about the shapes it has to get right: a two-word name, a
    /// three-word one, and a one-word one.
    #[test]
    fn a_kinds_name_is_its_own_spelling() {
        assert_eq!(snake(Kind::PerfumeVial), "perfume_vial");
        assert_eq!(snake(Kind::VeryMysteriousCrate), "very_mysterious_crate");
        assert_eq!(snake(Kind::Couch), "couch");
        assert_eq!(kind_named("bay_window"), Some(Kind::BayWindow));
        assert_eq!(kind_named("Couch"), None);
        // Every kind round-trips, so no two of them can collide either.
        for kind in Kind::ALL {
            assert_eq!(kind_named(&snake(kind)), Some(kind), "{kind:?}");
        }
    }

    /// **The identity declaration is the berth box.** `scale`, `offset`
    /// and `rotation` at their defaults with a mesh that measures one
    /// half-unit each way is the claim the manifest's own comment makes
    /// for the identity line: that the mesh is exactly the box the
    /// description asks for.
    #[test]
    fn the_identity_declaration_is_the_box_the_description_claims() {
        let declared = Dressings::read(
            "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n\
             glb = \"glb/abc.glb\"\nmeasured_mid = [0.0, 0.0, 0.0]\n\
             measured_half = [1.0, 1.0, 1.0]\n",
        )
        .expect("the dialect");
        let one = declared.of(Kind::SuspiciousCrate).expect("a dressing");
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        let (body_mid, body_half) = one.fill_box(Kind::SuspiciousCrate);
        assert!((body_mid - mid).length() < 1e-4, "{body_mid:?} {mid:?}");
        assert!((body_half - half).length() < 1e-4, "{body_half:?} {half:?}");
        // And the pose puts a unit-half mesh over exactly that box.
        let pose = one.pose(Kind::SuspiciousCrate);
        assert!((pose.scale - half).length() < 1e-4, "{:?}", pose.scale);
        assert!(
            (pose.translation - mid).length() < 1e-4,
            "{:?}",
            pose.translation
        );
    }

    /// **A mesh whose origin is not its middle is carried onto its
    /// middle.** A Synty prop often sits on its own base rather than in
    /// the centre of its own bounds, and `offset = [0, 0, 0]` has to
    /// mean "centred in its berth" for the containment arithmetic above
    /// it to mean anything.
    #[test]
    fn a_mesh_is_placed_on_its_own_middle_and_not_on_its_origin() {
        let declared = Dressings::read(
            "[asset.crate_small]\ndresses = \"cargo/suspicious_crate\"\n\
             glb = \"glb/abc.glb\"\nmeasured_mid = [0.0, 0.5, 0.0]\n\
             measured_half = [1.0, 0.5, 1.0]\nfill = [1.0, 0.5, 1.0]\n",
        )
        .expect("the dialect");
        let one = declared.of(Kind::SuspiciousCrate).expect("a dressing");
        let (mid, half) = Dressing::berth_box(Kind::SuspiciousCrate);
        let pose = one.pose(Kind::SuspiciousCrate);
        // The mesh's own middle is half a unit up its own y, so the pose
        // pulls it back down by that much in berth units.
        assert!((pose.translation.y - 0.5f32.mul_add(-half.y, mid.y)).abs() < 1e-4);
        assert!((pose.translation.x - mid.x).abs() < 1e-4);
    }
}
