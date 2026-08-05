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
