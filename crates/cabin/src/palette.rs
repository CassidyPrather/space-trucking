//! All color lives here, by role — the cabin restates the 2D console's
//! palette discipline in 3D terms (`docs/ART_DIRECTION_3D.md`). The hex
//! values are the 2D prototype's, verbatim: retuning still happens in one
//! place. What changes is the physics: in 3D a role is either a *surface*
//! (a lit material — worn metal family) or a *phosphor* (an emissive —
//! screens, lamps, glows), and light itself is real, so the omen dims the
//! cabin by dimming actual lights instead of multiplying draw calls.
//!
//! No raw color constructors outside this file — the purity test at the
//! bottom greps the crate the same way the 2D `palette.rs` does.

// Hex colors read as `0xRRGGBB`, the same spelling as the art doc's
// tables; separators would break the correspondence.
#![allow(clippy::unreadable_literal)]

use bevy::prelude::*;

/// Build a full-alpha sRGB color from `0xRRGGBB`, the shared idiom.
const fn hex(rgb: u32) -> Color {
    Color::srgb_u8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    )
}

// ---- Material families (values from docs/ART_DIRECTION.md) ----

/// Space, behind everything; the window clear color.
pub const VOID: Color = hex(0x0a0d10);
/// Cabin walls between plates.
pub const HULL: Color = hex(0x1a1f1d);
/// Riveted plate faces, green-gray metal.
pub const PLATE: Color = hex(0x242b27);
/// Bevel edge toward the light.
pub const PLATE_LIT: Color = hex(0x3a443d);
/// Bevel edge away from it.
pub const PLATE_SHADE: Color = hex(0x121614);
/// Rivet heads; also the rat's fur.
pub const RIVET: Color = hex(0x465049);
/// Inset wells: hold sockets, slot cubbies, lever tracks.
pub const SOCKET: Color = hex(0x151a17);
/// Unlit CRT glass.
pub const SCREEN: Color = hex(0x0a120c);
/// CRT foreground: POI readings, routes, rings, sweep.
pub const PHOSPHOR: Color = hex(0x7fd962);
/// CRT furniture: range rings, grids, trails.
pub const PHOSPHOR_DIM: Color = hex(0x2c4a2a);
/// Hottest trace: the ship, arrival pulses.
pub const PHOSPHOR_HOT: Color = hex(0xbdf29b);
/// Invitation and warning lamps, ETA arc, want pips.
pub const AMBER: Color = hex(0xe8a33d);
/// The burner's firebox: banked fire behind the hatch glass, feeding
/// flares. Hotter than [`AMBER`], redder than any lamp.
pub const EMBER: Color = hex(0xe0662a);
/// Lever handles and hardware accents.
pub const BRASS: Color = hex(0xb08d57);
/// Ready/go lamps.
pub const LAMP_OK: Color = hex(0x59c135);
/// Refusal flashes, violation glyphs.
pub const LAMP_NO: Color = hex(0xd84a35);
/// The suspicious violet: crate glow, omen cast, hangar lamps.
pub const EERIE: Color = hex(0x8f5fd6);
/// The jump flash, crate hot core, ??? toll.
pub const EERIE_BRIGHT: Color = hex(0xb388ea);
/// Warm off-white: needles, glints. Pure white is banned.
pub const GLINT: Color = hex(0xe7e3d1);
/// Near-black: wells and shadows. Pure black is banned.
pub const SHADOW: Color = hex(0x05070a);
/// Off-lamp glass: dark but visibly still a lamp.
pub const GLASS: Color = hex(0x101512);
/// Etched icon lines while the function sleeps.
pub const ICON: Color = hex(0x5b665e);
/// Icon lines while live but not lamp-hot.
pub const ICON_LIT: Color = hex(0xa7b3a9);

// ---- Barter furniture trims ----

pub const TRIM_SHELF: Color = hex(0x6b5f80);
pub const TRIM_RECEIVED: Color = hex(0x567a5d);
pub const TRIM_GIVE: Color = hex(0x76714f);
pub const TRIM_TAKE: Color = hex(0x4d7a80);

/// The version string, the game's one piece of text.
pub const VERSION_TEXT: Color = Color::srgba(0.851, 0.851, 0.902, 0.75);

// ---- POI identity hues (enamel; the tank phosphorizes its readings) ----

pub const POI_VENUS: Color = hex(0xf0b38c);
pub const POI_EARTH: Color = hex(0x738ca6);
pub const POI_MARS: Color = hex(0xc25738);
pub const POI_JUPITER: Color = hex(0xdb9957);
pub const POI_URANUS: Color = hex(0x9ed9e0);
pub const POI_NEPTUNE: Color = hex(0x4266d9);
pub const POI_GUILD: Color = hex(0x6b618c);
pub const POI_SATURN: Color = hex(0xd6c28a);
pub const POI_UMBRA: Color = hex(0x8c93c9);
pub const POI_HERMITAGE: Color = hex(0xa68a6b);
pub const POI_COMET: Color = hex(0xcfeaf2);
pub const POI_WANDERER: Color = hex(0x7de0c0);

/// Accent hues the glyphs spend on planet detail — Venus's halo,
/// Earth's smog, the ring systems — consumed by the faithful CRT port.
pub mod accent {
    use super::{Color, hex};
    pub const VENUS_HALO: Color = hex(0xffdb99);
    pub const SMOG: Color = hex(0x7a5c3d);
    pub const MARS_PATCH: Color = hex(0x943d2b);
    pub const URANUS_RING: Color = hex(0xb8d6e6);
    pub const GUILD_EDGE: Color = hex(0x9e8fc7);
    pub const SATURN_RING: Color = hex(0xa8936b);
}

/// Identity hue per POI index, for the tank's readings and enamel badges.
#[must_use]
pub const fn poi_color(id: u8) -> Color {
    match id {
        0 => POI_VENUS,
        1 => POI_EARTH,
        2 => POI_MARS,
        3 => POI_JUPITER,
        4 => POI_URANUS,
        5 => POI_NEPTUNE,
        7 => POI_SATURN,
        8 => POI_UMBRA,
        9 => POI_HERMITAGE,
        10 => POI_COMET,
        11 => POI_WANDERER,
        _ => POI_GUILD,
    }
}

// ---- Cargo kind hues (one saturated identity hue per kind) ----

use space_trucking::sim::Kind;

/// The story color of a cargo kind, matching the 2D console exactly.
#[must_use]
pub const fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::PerfumeVial => hex(0xd987a6),
        Kind::GildedIdol => hex(0xe3bc4a),
        Kind::RationBricks => hex(0x8f8f4e),
        Kind::ScrapAlloy => hex(0xb06a45),
        Kind::Seedlings => hex(0x6fb85a),
        Kind::GasCanister => hex(0xde8433),
        Kind::CryoCore => hex(0x6cc4d9),
        Kind::BrinePearls => hex(0x93a8cf),
        Kind::SuspiciousCrate => hex(0x16131c),
        Kind::MysteriousCrate => hex(0x5c5449),
        Kind::VeryMysteriousCrate => hex(0x241038),
        Kind::CometIce => hex(0xbfe4ef),
        Kind::BottledMidnight => hex(0x1c254d),
        Kind::Fluff => hex(0xe0c9a8),
        Kind::TransitChit => hex(0xc9d256),
        Kind::CasinoChip => hex(0xd44f6f),
        // Fixture hues, canon: lamp golds cooling from ceiling to floor,
        // upholstery plum, gallery violet.
        Kind::CeilingLamp => hex(0xe8d06f),
        Kind::WallLamp => hex(0xd9a94e),
        Kind::FloorLamp => hex(0xc08d5a),
        Kind::Couch => hex(0x96566a),
        Kind::Painting => hex(0x8f76c0),
        // Oiled oak and brass fittings: furniture that means business.
        Kind::Cabinet => hex(0x8a6f4d),
        // Dressing hues, canon: madder weave, battered tin, and the
        // pale luminous coat (docs/BAY.md's dressing layer).
        Kind::Rug => hex(0x9c4a3a),
        Kind::PaintTin => hex(0x7a7d82),
        Kind::LuminousPaint => hex(0xd8f0c0),
    }
}

/// Ship enamel by the paint tin's variant roll, canon: oxide red, teal,
/// mustard, slate blue — the muted register, so a coated cell reads as
/// paintwork on a working hull rather than a highlight.
#[must_use]
pub const fn enamel_color(variant: u8) -> Color {
    match variant % 4 {
        0 => hex(0x8c3f2e),
        1 => hex(0x2e6b62),
        2 => hex(0xb08a3c),
        _ => hex(0x44586e),
    }
}

/// Variant tints, the 2D console's cosmetic shifts carried over.
const VARIANT_SHIFT: [(f32, f32, f32); 4] = [
    (0.0, 0.0, 0.0),
    (0.06, -0.03, -0.02),
    (-0.05, 0.04, 0.02),
    (0.02, 0.02, -0.06),
];

/// Nudge a kind hue by the piece's cosmetic variant roll.
#[must_use]
pub fn variant_tint(col: Color, variant: u8) -> Color {
    let (dr, dg, db) = VARIANT_SHIFT[(variant % 4) as usize];
    let c = col.to_srgba();
    Color::srgba(
        (c.red + dr).clamp(0.0, 1.0),
        (c.green + dg).clamp(0.0, 1.0),
        (c.blue + db).clamp(0.0, 1.0),
        c.alpha,
    )
}

/// Channel-wise mix, alpha included — the dial ramp's helper.
#[must_use]
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let t = t.clamp(0.0, 1.0);
    Color::srgba(
        (b.red - a.red).mul_add(t, a.red),
        (b.green - a.green).mul_add(t, a.green),
        (b.blue - a.blue).mul_add(t, a.blue),
        (b.alpha - a.alpha).mul_add(t, a.alpha),
    )
}

/// The eagerness dial's color ramp: red short of break-even, amber at it,
/// green into generosity. `value` is in dial units, `0..=EAGER_MAX`.
#[must_use]
pub fn dial_color(value: f32) -> Color {
    if value <= 1.0 {
        mix(LAMP_NO, AMBER, value)
    } else {
        mix(AMBER, LAMP_OK, (value - 1.0).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The purity rule, restated for the cabin: no raw color constructors
    /// outside this file. Same enforcement style as the 2D palette.
    #[test]
    fn palette_purity() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&dir).expect("src dir readable") {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "palette.rs" || path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source readable");
            for banned in [
                "Color::srgb",
                "Color::linear_rgb",
                "Color::hsl",
                "Color::oklab",
            ] {
                assert!(
                    !text.contains(banned),
                    "{name} constructs a raw color ({banned}); use a palette role"
                );
            }
        }
    }

    #[test]
    fn dial_ramp_hits_its_landmarks() {
        let close = |a: Color, b: Color| {
            let (a, b) = (a.to_srgba(), b.to_srgba());
            (a.red - b.red).abs() < 1e-5
                && (a.green - b.green).abs() < 1e-5
                && (a.blue - b.blue).abs() < 1e-5
        };
        assert!(close(dial_color(0.0), LAMP_NO));
        assert!(close(dial_color(1.0), AMBER));
        assert!(close(dial_color(2.0), LAMP_OK));
    }

    #[test]
    fn variant_tints_stay_in_gamut() {
        for variant in 0..4 {
            let c = variant_tint(kind_color(Kind::GildedIdol), variant).to_srgba();
            for ch in [c.red, c.green, c.blue] {
                assert!((0.0..=1.0).contains(&ch));
            }
        }
    }
}
