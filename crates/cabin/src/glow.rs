//! Shared light-emitting material helpers: phosphor readings, enamel
//! badges, lamp glass. The 2D console's two material families translate
//! to 3D as *lit metal* ([`crate::rig::Skin`]) versus *emissives* (here).
//!
//! Anything that changes brightness per frame needs its own material
//! instance — bevy materials are shared assets, and mutating a shared
//! handle would light every lamp on the ship at once.

use bevy::prelude::*;

use crate::palette;

/// A phosphor mark: near-black surface, colored glow. `glow` is emissive
/// intensity — 1.0 reads as a lit indicator, 4.0+ blooms.
pub fn phosphor(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    glow: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: color.to_linear() * glow,
        perceptual_roughness: 0.85,
        metallic: 0.0,
        ..default()
    })
}

/// Painted enamel: a full-color lit surface, for badges and identity
/// marks that are metal, not screens.
pub fn enamel(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.7,
        metallic: 0.05,
        ..default()
    })
}

/// Etched hardware markings: enamel with a faint self-glow, the
/// lights-out floor — icons and instrument markings must stay legible
/// on technicality when every lamp aboard is gone (the lamps are
/// cargo). Dim enough to vanish under any real light.
pub fn etched(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.7,
        metallic: 0.05,
        emissive: color.to_linear() * 0.35,
        ..default()
    })
}

/// The calm sine loop every decoration breathes with; returns `0..=1`.
#[must_use]
pub fn breathe(t: f32, freq: f32, phase: f32) -> f32 {
    t.mul_add(freq, phase).sin().mul_add(0.5, 0.5)
}

/// A lamp's glass-and-glow, written into its own material each frame:
/// `level` 0 is dark glass, 1 is fully lit.
pub fn set_lamp(material: &mut StandardMaterial, color: Color, level: f32) {
    let level = level.clamp(0.0, 1.0);
    material.base_color = palette::GLASS;
    material.emissive = color.to_linear() * (level * 6.0);
}

/// How hard a bought fitting's lit faces burn at full level: the
/// emissive factor over the pack's own emissive atlas, which is black
/// everywhere the pack meant no light and the lamp's colour where it
/// did. The same order as a whitebox bulb's glass at full wake, so the
/// two lamps bloom alike; the hue stays the pack's, because repainting
/// a bought material is throwing away the thing that was bought.
const FITTING_GLOW: f32 = 6.0;

/// A bought lamp's glass, written into its own material each frame the
/// way [`set_lamp`] writes a whitebox bulb's: `level` 0 is the fitting
/// unlit, 1 is every face the pack lit burning. Only the emissive is
/// touched — the body keeps its atlas, and the atlas's black masks the
/// level to exactly the faces meant to glow.
pub fn set_fitting(material: &mut StandardMaterial, level: f32) {
    let level = level.clamp(0.0, 1.0);
    material.emissive = LinearRgba::WHITE * (level * FITTING_GLOW);
}

/// How much of a bought shade's own colour is left over what is behind
/// it. The packs that model a shade put the bulb inside it, so an
/// opaque shade over a lit bulb is a lamp that reads dark.
#[cfg_attr(not(feature = "art"), allow(dead_code))] // only a bought shade is glazed
const GLAZE_ALPHA: f32 = 0.45;

/// Draw a bought lamp's shade as glass: see-through, its atlas colour
/// kept as a tint, and a shade's gloss rather than a housing's matte.
#[cfg_attr(not(feature = "art"), allow(dead_code))] // only a bought shade is glazed
pub fn glaze(material: &mut StandardMaterial) {
    material.alpha_mode = AlphaMode::Blend;
    material.base_color = material.base_color.with_alpha(GLAZE_ALPHA);
    material.perceptual_roughness = 0.25;
}
