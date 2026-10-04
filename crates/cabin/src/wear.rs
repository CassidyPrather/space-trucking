//! Deterministic procedural wear: the 2D console's scuffed-metal
//! discipline (`src/render.rs`'s `wear`, the playtest's favourite) reborn
//! as code-generated tiling textures. No asset files — every tile bakes
//! at boot from [`splitmix`] draws off the 2D renderer's own wear seed,
//! so every ship is scuffed the same way every boot, and the "flat gray,
//! barren surfaces" verdict gets material grain without art assets.
//!
//! # The multiplier contract
//!
//! [`WearBook::plate`], [`WearBook::hull`], and [`WearBook::desk`] are
//! near-white **multiplier maps**: neutral-gray `Rgba8UnormSrgb` tiles
//! for `StandardMaterial::base_color_texture`, which the renderer
//! multiplies into the palette-tinted `base_color`. They carry value
//! only, never hue, so the palette stays the one place color lives:
//!
//! - every texel sits inside `BAND_LO..=BAND_HI` (205..=255, sRGB
//!   0.918..=1.0), enforced by test — wear whispers, per the art doc's
//!   alpha ≤ 0.15 rule;
//! - each map bakes around `BASE` (250/255 ≈ 0.98) and its average stays
//!   within 246..=252, so a worn surface dims ≤ 2% in sRGB terms
//!   (≈ 4% in linear light after decode). That dimming is the intended
//!   patina — "no unweathered surfaces" — and [`worn`] passes the
//!   palette color through untouched instead of compensating with raw
//!   color math.
//!
//! [`WearBook::hazard`] is the one full-color tile: hard 45° `AMBER` /
//! `SHADOW` stripes for standalone accent strips, never a multiplier.
//!
//! # Tiling
//!
//! Every feature draws with wrapping coordinates — blotch noise is
//! lattice-periodic, and scratches, scuffs, streaks, and slide arcs wrap
//! their texel writes — so the tiles repeat seamlessly (no seam cliff,
//! enforced by test). Samplers are nearest-filtered and repeat-addressed
//! on both axes; [`worn`]'s `uv_scale` sets how many times a tile
//! repeats across a face, so a wall tiles the 96×96 several times while
//! small furniture uses ~1.

// Wear math walks texel grids with wrapped signed coordinates; every
// cast back to an index follows a `rem_euclid` or clamp that keeps the
// value non-negative.
#![allow(clippy::cast_sign_loss)]

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use space_trucking::sim::splitmix;

use crate::canvas::lerp;
use crate::palette;

// ------------------------------------------------------------------ seeds --

/// The 2D renderer's wear seed, verbatim (`src/render.rs`): the same
/// decade of shipping scuffed both ships.
const WEAR_SEED: u64 = 0x5C2A_7C4E;

// Per-map salts, so each tile wears its own history.
const SALT_PLATE: u64 = 0x504C_4154; // "PLAT"
const SALT_HULL: u64 = 0x4855_4C4C; // "HULL"
const SALT_DESK: u64 = 0x4445_534B; // "DESK"

// Feature salt bases: disjoint ranges, so every mark hashes its own
// stream and a retuned count never reshuffles its neighbours.
const SALT_NOISE: u64 = 0x10;
const SALT_SCRATCH: u64 = 0x100;
const SALT_SCUFF: u64 = 0x200;
const SALT_GLINT: u64 = 0x300;
const SALT_STREAK: u64 = 0x400;
const SALT_ARC: u64 = 0x500;

// ----------------------------------------------------------------- tuning --

/// Multiplier tile edge, texels.
const TILE: i32 = 96;
/// Hazard tile edge, texels.
const HAZARD: i32 = 32;
/// Hazard stripe period along the 45° diagonal, texels. Divides
/// [`HAZARD`], so the stripes wrap whole.
// Fat stripes on purpose: the first bake's 8-texel period shimmered
// into noise under minification ("flashing effect", the playtest said),
// and chunky paint reads honest at every distance.
const HAZARD_PERIOD: i32 = 16;

/// The multiplier band, in 8-bit sRGB value units: every baked texel is
/// clamped inside, so the worst overlap of features still whispers.
const BAND_LO: u8 = 205;
const BAND_HI: u8 = 255;
/// The unweathered value a tile starts from, ≈ 0.98 of full white.
const BASE: f32 = 246.0;

// Feature depths, in the same 8-bit value units (negative darkens).
const PLATE_BLOTCH: f32 = 20.0; // ±8%, large soft value noise
const HULL_BLOTCH: f32 = 16.0; // ±6%, coarser and quieter
const BRUSH: f32 = 9.0; // ±3.5% per brushed 1-texel row
const SCRATCH_LIGHT: f32 = 9.0; // bare-metal glint scratches
const SCRATCH_DARK: f32 = -30.0; // grimed-in scratches
const SCUFF_DEPTH: f32 = -32.0; // 13% darker at a scuff's heart
const GLINT_LIFT: f32 = 9.0; // single chipped-paint sparkles
const GRIME_DEPTH: f32 = -18.0; // panel-edge grime, full strength
const GRIME_REACH: i32 = 6; // …fading out this many texels in
const STREAK_DEPTH: f32 = -11.0; // hull drip stains
const ARC_DEPTH: f32 = -26.0; // 10% darker where crates slide
const ARC_SOFT: f32 = 1.8; // slide-arc half-thickness, texels

// Feature counts per tile.
const PLATE_SCRATCHES: u64 = 14;
const PLATE_SCUFFS: u64 = 4;
const PLATE_GLINTS: u64 = 10;
const HULL_STREAKS: u64 = 4;
const DESK_SCUFFS: u64 = 6;
const DESK_ARCS: u64 = 2;

// ------------------------------------------------------------ the tile ----

/// Flat index of a wrapped texel coordinate.
const fn idx(x: i32, y: i32) -> usize {
    (y.rem_euclid(TILE) * TILE + x.rem_euclid(TILE)) as usize
}

/// A uniform draw in `0..1` from one splitmix hash.
fn unit(state: u64, salt: u64) -> f32 {
    (splitmix(state, salt) & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Smoothstep, the same easing the camera glide uses.
fn smooth(t: f32) -> f32 {
    t * t * 2.0f32.mul_add(-t, 3.0)
}

/// A coordinate difference wrapped onto the tile torus, `-TILE/2..TILE/2`,
/// so distance-based features tile by construction.
fn wrap_delta(d: f32) -> f32 {
    let len = TILE as f32;
    let half = len * 0.5;
    (d + half).rem_euclid(len) - half
}

/// A working value grid: one f32 per texel in 8-bit value units, drawn
/// on with wrapping coordinates, collapsed into RGBA8 at the end.
struct Tile {
    v: Vec<f32>,
}

impl Tile {
    fn new() -> Self {
        Self {
            v: vec![BASE; (TILE * TILE) as usize],
        }
    }

    /// Add `delta` to one texel, wrapping both coordinates.
    fn add(&mut self, x: i32, y: i32, delta: f32) {
        self.v[idx(x, y)] += delta;
    }

    /// Soft value-noise blotches: a periodic `points × points` lattice of
    /// hashed values, smoothstep-interpolated, ±`amp` — the large-scale
    /// mottling that keeps a plate from reading as one flat fill.
    fn blotches(&mut self, seed: u64, points: i32, amp: f32) {
        debug_assert_eq!(TILE % points, 0, "lattice must divide the tile");
        let cell = TILE / points;
        let corner = |i: i32, j: i32| {
            let salt = (j.rem_euclid(points) * points + i.rem_euclid(points)) as u64;
            unit(seed, salt).mul_add(2.0, -1.0)
        };
        for y in 0..TILE {
            for x in 0..TILE {
                let (cx, cy) = (x / cell, y / cell);
                let fx = smooth((x - cx * cell) as f32 / cell as f32);
                let fy = smooth((y - cy * cell) as f32 / cell as f32);
                let top = lerp(corner(cx, cy), corner(cx + 1, cy), fx);
                let bottom = lerp(corner(cx, cy + 1), corner(cx + 1, cy + 1), fx);
                self.add(x, y, lerp(top, bottom, fy) * amp);
            }
        }
    }

    /// Grime creeping in from the tile border: panel edges collect it
    /// where rags don't reach. Symmetric, so opposite edges still meet.
    fn edge_grime(&mut self) {
        for y in 0..TILE {
            for x in 0..TILE {
                let d = x.min(y).min(TILE - 1 - x).min(TILE - 1 - y);
                if d < GRIME_REACH {
                    let fade = 1.0 - d as f32 / GRIME_REACH as f32;
                    self.add(x, y, GRIME_DEPTH * fade);
                }
            }
        }
    }

    /// One fine scratch: a 1-texel polyline wandering from a hashed
    /// start, wrapping wherever it leaves the tile.
    fn scratch(&mut self, h: u64, delta: f32) {
        let mut x = unit(h, 0) * TILE as f32;
        let mut y = unit(h, 1) * TILE as f32;
        let mut angle = unit(h, 2) * TAU;
        let steps = 10 + splitmix(h, 3) % 16;
        for step in 0..steps {
            self.add(x.floor() as i32, y.floor() as i32, delta);
            angle = (unit(h, 4 + step) - 0.5).mul_add(0.3, angle);
            x += angle.cos();
            y += angle.sin();
        }
    }

    /// One elliptical scuff with a soft edge, stamped through the wrap.
    /// `spread` widens the radius roll for heavier-handled surfaces.
    fn scuff(&mut self, h: u64, spread: f32) {
        let cx = (unit(h, 0) * TILE as f32).floor() as i32;
        let cy = (unit(h, 1) * TILE as f32).floor() as i32;
        let rx = unit(h, 2).mul_add(spread, 4.0);
        let ry = unit(h, 3).mul_add(spread * 0.7, 3.0);
        let (rxi, ryi) = (rx.ceil() as i32, ry.ceil() as i32);
        for dy in -ryi..=ryi {
            for dx in -rxi..=rxi {
                let (fx, fy) = (dx as f32 / rx, dy as f32 / ry);
                let level = fx.mul_add(-fx, fy.mul_add(-fy, 1.0));
                if level > 0.0 {
                    self.add(cx + dx, cy + dy, SCUFF_DEPTH * smooth(level));
                }
            }
        }
    }

    /// One faint vertical drip stain: a 1-texel run downward with the
    /// occasional sidestep, as condensation finds a new path.
    fn streak(&mut self, h: u64) {
        let mut x = unit(h, 0) * TILE as f32;
        let top = unit(h, 1) * TILE as f32;
        let length = 40 + splitmix(h, 2) % 50;
        for i in 0..length {
            let y = top + i as f32;
            self.add(x.floor() as i32, y.floor() as i32, STREAK_DEPTH);
            if splitmix(h, 100 + i).is_multiple_of(9) {
                x += if splitmix(h, 200 + i) & 1 == 0 {
                    1.0
                } else {
                    -1.0
                };
            }
        }
    }

    /// One arc of slide wear — the path a crate corner drags across the
    /// counter — rasterized per texel with wrapped distances, so it
    /// tiles like everything else.
    fn slide_arc(&mut self, h: u64) {
        let cx = unit(h, 0) * TILE as f32;
        let cy = unit(h, 1) * TILE as f32;
        let radius = unit(h, 2).mul_add(12.0, 16.0);
        let from = unit(h, 3) * TAU;
        let sweep = unit(h, 4).mul_add(0.8, 0.9);
        for y in 0..TILE {
            for x in 0..TILE {
                let dx = wrap_delta((x as f32 + 0.5) - cx);
                let dy = wrap_delta((y as f32 + 0.5) - cy);
                let band = (dx.hypot(dy) - radius).abs();
                if band < ARC_SOFT && (dy.atan2(dx) - from).rem_euclid(TAU) <= sweep {
                    self.add(x, y, ARC_DEPTH * (1.0 - band / ARC_SOFT));
                }
            }
        }
    }

    /// Collapse the grid into RGBA8: clamp into the band, splat the
    /// value across the channels (neutral gray), opaque alpha.
    fn bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.v.len() * 4);
        for value in self.v {
            let v = value.round().clamp(f32::from(BAND_LO), f32::from(BAND_HI)) as u8;
            out.extend_from_slice(&[v, v, v, 255]);
        }
        out
    }
}

// --------------------------------------------------------------- builders --

/// The panel-plate multiplier, 96×96: soft value blotches, fine
/// scratches both bare and grimed, a few elliptical scuffs, chipped
/// glint texels, and grime creeping in from the panel seams.
pub fn bake_plate_bytes() -> Vec<u8> {
    let seed = WEAR_SEED ^ SALT_PLATE;
    let mut tile = Tile::new();
    tile.blotches(splitmix(seed, SALT_NOISE), 4, PLATE_BLOTCH);
    tile.edge_grime();
    for i in 0..PLATE_SCUFFS {
        tile.scuff(splitmix(seed, SALT_SCUFF + i), 5.0);
    }
    for i in 0..PLATE_SCRATCHES {
        let h = splitmix(seed, SALT_SCRATCH + i);
        let delta = if h & 1 == 0 {
            SCRATCH_LIGHT
        } else {
            SCRATCH_DARK
        };
        tile.scratch(h, delta);
    }
    for i in 0..PLATE_GLINTS {
        let h = splitmix(seed, SALT_GLINT + i);
        tile.add(
            (unit(h, 0) * TILE as f32).floor() as i32,
            (unit(h, 1) * TILE as f32).floor() as i32,
            GLINT_LIFT,
        );
    }
    tile.bytes()
}

/// The hull multiplier, 96×96: coarser and quieter — big soft blotches
/// and a few long faint drip stains, nothing sharp. Walls recede.
pub fn bake_hull_bytes() -> Vec<u8> {
    let seed = WEAR_SEED ^ SALT_HULL;
    let mut tile = Tile::new();
    tile.blotches(splitmix(seed, SALT_NOISE), 3, HULL_BLOTCH);
    for i in 0..HULL_STREAKS {
        tile.streak(splitmix(seed, SALT_STREAK + i));
    }
    tile.bytes()
}

/// The desk multiplier, 96×96: directional — horizontal brushed
/// micro-streaks, heavier scuffing (hands live here), and two arcs of
/// slide wear where crates drag between hold and counter.
pub fn bake_desk_bytes() -> Vec<u8> {
    let seed = WEAR_SEED ^ SALT_DESK;
    let rows = splitmix(seed, SALT_NOISE);
    let mut tile = Tile::new();
    for y in 0..TILE {
        let delta = unit(rows, y as u64).mul_add(2.0, -1.0) * BRUSH;
        for x in 0..TILE {
            tile.add(x, y, delta);
        }
    }
    for i in 0..DESK_SCUFFS {
        tile.scuff(splitmix(seed, SALT_SCUFF + i), 7.0);
    }
    for i in 0..DESK_ARCS {
        tile.slide_arc(splitmix(seed, SALT_ARC + i));
    }
    tile.bytes()
}

/// A palette role as one RGBA8 texel.
fn texel(col: Color) -> [u8; 4] {
    let c = col.to_srgba();
    [
        (c.red * 255.0).round() as u8,
        (c.green * 255.0).round() as u8,
        (c.blue * 255.0).round() as u8,
        255,
    ]
}

/// The hazard accent, 32×32: hard 45° stripes alternating `AMBER` and
/// `SHADOW` on an 8-texel period. Full color, worn by accent strips
/// directly — decor, not signal, so no meaning rides on it.
pub fn bake_hazard_bytes() -> Vec<u8> {
    let amber = texel(palette::AMBER);
    let shadow = texel(palette::SHADOW);
    let mut out = Vec::with_capacity((HAZARD * HAZARD * 4) as usize);
    for y in 0..HAZARD {
        for x in 0..HAZARD {
            let lit = (x + y).rem_euclid(HAZARD_PERIOD) < HAZARD_PERIOD / 2;
            out.extend_from_slice(if lit { &amber } else { &shadow });
        }
    }
    out
}

// ------------------------------------------------------------- the book ---

/// The baked wear tiles, one handle per surface class. Baked once at
/// rig time and shared — wear is static, so shared handles are correct
/// here (unlike dynamic emissives).
pub struct WearBook {
    pub plate: Handle<Image>,
    pub hull: Handle<Image>,
    pub desk: Handle<Image>,
    pub hazard: Handle<Image>,
}

/// One mip level downsampled from the last: each texel averages the 2×2
/// it covers (clamped at odd edges). Averaging sRGB bytes directly is
/// technically gamma-space math; on low-contrast multiplier maps and a
/// two-color stripe tile the error is invisible, and determinism is
/// exact either way.
fn downsample(src: &[u8], src_size: u32) -> (Vec<u8>, u32) {
    let dst_size = (src_size / 2).max(1);
    let mut dst = Vec::with_capacity((dst_size * dst_size * 4) as usize);
    for y in 0..dst_size {
        for x in 0..dst_size {
            for channel in 0..4 {
                let mut sum: u32 = 0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(src_size - 1);
                    let sy = (y * 2 + dy).min(src_size - 1);
                    sum += u32::from(src[((sy * src_size + sx) * 4 + channel) as usize]);
                }
                dst.push((sum / 4) as u8);
            }
        }
    }
    (dst, dst_size)
}

/// The full mip chain, level 0 first, concatenated the way wgpu expects,
/// plus the level count.
fn mip_chain(base: Vec<u8>, size: u32) -> (Vec<u8>, u32) {
    let mut data = base;
    let mut levels = 1;
    let mut cursor = 0usize;
    let mut dim = size;
    while dim > 1 {
        let level_len = (dim * dim * 4) as usize;
        let (next, next_dim) = downsample(&data[cursor..cursor + level_len], dim);
        cursor += level_len;
        data.extend_from_slice(&next);
        dim = next_dim;
        levels += 1;
    }
    (data, levels)
}

/// Finished bytes as a repeat-addressed GPU tile with a hand-baked mip
/// chain: hard texels up close (`mag: Nearest` — the aesthetic), stable
/// at distance (`min`/`mipmap: Linear` over real mips — no shimmer).
/// The playtest's "flashing" hazard tape was exactly the missing mips.
///
/// `crisp` keeps nearest magnification (the multiplier tiles); the
/// hazard tape passes `false` for full linear + anisotropic sampling —
/// painted stripes seen at a grazing angle need anisotropy or they
/// average into mud, and they are paint, not pixel art.
fn tile_image(bytes: Vec<u8>, size: i32, crisp: bool) -> Image {
    let (data, levels) = mip_chain(bytes, size as u32);
    // `Image::new` asserts data length against level 0 alone, so the
    // full chain and its level count are installed after construction.
    let mut image = Image::new(
        Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (size * size * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: if crisp {
            ImageFilterMode::Nearest
        } else {
            ImageFilterMode::Linear
        },
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        // wgpu requires all-linear filtering for anisotropy > 1.
        anisotropy_clamp: if crisp { 1 } else { 8 },
        ..default()
    });
    image
}

/// Bake every wear tile. Call once while building [`crate::rig::Skin`].
pub fn bake(images: &mut Assets<Image>) -> WearBook {
    WearBook {
        plate: images.add(tile_image(bake_plate_bytes(), TILE, true)),
        hull: images.add(tile_image(bake_hull_bytes(), TILE, true)),
        desk: images.add(tile_image(bake_desk_bytes(), TILE, true)),
        hazard: images.add(tile_image(bake_hazard_bytes(), HAZARD, false)),
    }
}

/// A worn material: the palette role as `base_color`, a wear tile
/// multiplying it, `uv_scale` repeats across each face. The tile's
/// sub-white average dims the role ≤ 2% (sRGB) — intended patina, see
/// the module doc — so the color needs no compensating math.
///
/// `glow` is the **lights-out floor**: a role and a level for the faint
/// self-glow a surface keeps when every lamp aboard has been sold
/// (docs/BAY.md, "Lights are cargo" — the clause that gives the icon
/// etchings and the brass hardware a radium-paint glow). `None` for a
/// surface that is only ever seen by somebody else's light, which is
/// most of them. A floor is dim enough to vanish under any real lamp.
///
/// **The floor rides the same wear tile the color does**, so the paint
/// glows exactly where the paint is: a striped surface stays striped in
/// the dark instead of flattening into one warm band, and the *kind* of
/// mark a tile class wears survives the lights going out — which is the
/// no-hue-alone law's own requirement, spent on the one case where hue
/// is all that is left.
pub fn worn(
    materials: &mut Assets<StandardMaterial>,
    texture: &Handle<Image>,
    color: Color,
    uv_scale: f32,
    roughness: f32,
    metallic: f32,
    glow: Option<(Color, f32)>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        base_color_texture: Some(texture.clone()),
        uv_transform: Affine2::from_scale(Vec2::splat(uv_scale)),
        perceptual_roughness: roughness,
        metallic,
        emissive: glow.map_or(LinearRgba::BLACK, |(role, level)| role.to_linear() * level),
        emissive_texture: glow.map(|_| texture.clone()),
        ..default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mip chain has the exact level count and byte length wgpu
    /// expects, and is itself deterministic.
    #[test]
    fn mip_chains_measure_out() {
        // 96 → 48/24/12/6/3/1: seven levels.
        let (data, levels) = mip_chain(bake_plate_bytes(), TILE as u32);
        assert_eq!(levels, 7);
        let expected: usize = [96u32, 48, 24, 12, 6, 3, 1]
            .iter()
            .map(|d| (d * d * 4) as usize)
            .sum();
        assert_eq!(data.len(), expected);
        // 32 → 16/8/4/2/1: six levels.
        let (data, levels) = mip_chain(bake_hazard_bytes(), HAZARD as u32);
        assert_eq!(levels, 6);
        let expected: usize = [32u32, 16, 8, 4, 2, 1]
            .iter()
            .map(|d| (d * d * 4) as usize)
            .sum();
        assert_eq!(data.len(), expected);
        let (again, _) = mip_chain(bake_hazard_bytes(), HAZARD as u32);
        assert_eq!(data, again);
    }

    /// Fat stripes survive one downsample: level 1 of the hazard tile
    /// still carries both paint colors instead of averaging to mud.
    #[test]
    fn hazard_stripes_survive_the_first_mip() {
        let (level1, dim) = downsample(&bake_hazard_bytes(), HAZARD as u32);
        assert_eq!(dim, 16);
        let mut lightest = 0u8;
        let mut darkest = 255u8;
        for texel in level1.as_chunks::<4>().0 {
            lightest = lightest.max(texel[0]);
            darkest = darkest.min(texel[0]);
        }
        assert!(
            i16::from(lightest) - i16::from(darkest) > 60,
            "level 1 lost the stripes: {darkest}..{lightest}"
        );
    }

    /// Same bytes every bake: the whole point of splitmix wear.
    #[test]
    fn baking_is_deterministic() {
        assert_eq!(bake_plate_bytes(), bake_plate_bytes());
        assert_eq!(bake_hull_bytes(), bake_hull_bytes());
        assert_eq!(bake_desk_bytes(), bake_desk_bytes());
        assert_eq!(bake_hazard_bytes(), bake_hazard_bytes());
    }

    fn multiplier_maps() -> [(&'static str, Vec<u8>); 3] {
        [
            ("plate", bake_plate_bytes()),
            ("hull", bake_hull_bytes()),
            ("desk", bake_desk_bytes()),
        ]
    }

    /// The multiplier contract: neutral gray, opaque, inside the band,
    /// averaging close to the 250 center so the patina stays ≤ 2%.
    #[test]
    fn multiplier_maps_hold_the_band() {
        for (name, bytes) in multiplier_maps() {
            assert_eq!(bytes.len(), (TILE * TILE * 4) as usize);
            let mut sum: u64 = 0;
            for texel in bytes.as_chunks::<4>().0 {
                assert!(
                    texel[0] == texel[1] && texel[1] == texel[2],
                    "{name} texel is not neutral gray: {texel:?}"
                );
                assert_eq!(texel[3], 255, "{name} texel is not opaque");
                assert!(
                    (BAND_LO..=BAND_HI).contains(&texel[0]),
                    "{name} texel {} left the band",
                    texel[0]
                );
                sum += u64::from(texel[0]);
            }
            let mean = sum as f64 / f64::from(TILE * TILE);
            assert!(
                (238.0..=250.0).contains(&mean),
                "{name} mean {mean} drifted off the 246 center"
            );
        }
    }

    /// Opposite edges must meet without a cliff: the repeat sampler
    /// abuts row 95 against row 0 (and column 95 against column 0), so
    /// texel-for-texel they may differ by at most the band width, and
    /// on average by far less — wrap-drawing guarantees it.
    #[test]
    fn tiles_meet_their_own_edges() {
        let band = i32::from(BAND_HI - BAND_LO);
        for (name, bytes) in multiplier_maps() {
            let at = |x: i32, y: i32| i32::from(bytes[idx(x, y) * 4]);
            let mut worst = 0;
            let mut total = 0;
            for i in 0..TILE {
                for (a, b) in [(at(i, 0), at(i, TILE - 1)), (at(0, i), at(TILE - 1, i))] {
                    let diff = (a - b).abs();
                    worst = worst.max(diff);
                    total += diff;
                }
            }
            assert!(worst < band, "{name} seam cliff: {worst} >= {band}");
            let mean = f64::from(total) / f64::from(TILE * 2);
            assert!(mean < 12.0, "{name} seam mean {mean} reads as a line");
        }
    }

    /// Hazard stripes really alternate: every texel is exactly `AMBER`
    /// or exactly `SHADOW`, split evenly, and the 8-texel diagonal
    /// period wraps the 32-texel tile whole.
    #[test]
    fn hazard_stripes_alternate() {
        let bytes = bake_hazard_bytes();
        assert_eq!(bytes.len(), (HAZARD * HAZARD * 4) as usize);
        let amber = texel(palette::AMBER);
        let shadow = texel(palette::SHADOW);
        let (mut lit, mut dark) = (0u32, 0u32);
        for texel in bytes.as_chunks::<4>().0 {
            if *texel == amber {
                lit += 1;
            } else if *texel == shadow {
                dark += 1;
            } else {
                panic!("hazard texel is neither AMBER nor SHADOW: {texel:?}");
            }
        }
        assert_eq!(lit, dark, "stripes must split the tile evenly");
        let at = |x: i32, y: i32| {
            let i = ((y.rem_euclid(HAZARD) * HAZARD + x.rem_euclid(HAZARD)) * 4) as usize;
            &bytes[i..i + 4]
        };
        for y in 0..HAZARD {
            for x in 0..HAZARD {
                assert_eq!(at(x, y), at(x + HAZARD_PERIOD, y));
                assert_eq!(at(x, y), at(x, y + HAZARD_PERIOD));
            }
        }
    }
}
