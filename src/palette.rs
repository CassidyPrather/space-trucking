//! Every colour in the game, named by role.
//!
//! `docs/ART_DIRECTION.md`'s palette table lives here and nowhere else: the
//! frontend draws exclusively through these roles, and the purity test at
//! the bottom greps the frontend sources for raw colour constructors so it
//! stays that way. Naming is by role, not hue — `AMBER` could turn red one
//! day and every lamp on the console would follow without a renderer edit.

use macroquad::color::Color;
use space_trucking::sim::Kind;

/// `#rrggbb` to a full-alpha [`Color`], so entries read like the doc table.
const fn hex(rgb: u32) -> Color {
    Color::new(
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        1.0,
    )
}

// ------------------------------------------------- the two material families

/// Space and the letterbox bars: behind everything.
pub const VOID: Color = hex(0x0a_0d_10);

/// Panel background between plates.
pub const HULL: Color = hex(0x1a_1f_1d);

/// Riveted plate faces: green-gray metal.
pub const PLATE: Color = hex(0x24_2b_27);

/// Bevel edge facing the light (top/left of raised things).
pub const PLATE_LIT: Color = hex(0x3a_44_3d);

/// Bevel edge away from the light (bottom/right of raised things).
pub const PLATE_SHADE: Color = hex(0x12_16_14);

/// Rivet heads; also etch lines that want to read as bare metal.
pub const RIVET: Color = hex(0x46_50_49);

/// Inset wells: hold cells, shelf/pad slots, lever tracks.
pub const SOCKET: Color = hex(0x15_1a_17);

/// Unlit CRT glass inside the bezels — screens are not space.
pub const SCREEN: Color = hex(0x0a_12_0c);

/// CRT foreground: POI readings, routes, rings, the sweep.
pub const PHOSPHOR: Color = hex(0x7f_d9_62);

/// CRT furniture: range rings, trails, scanline tint.
pub const PHOSPHOR_DIM: Color = hex(0x2c_4a_2a);

/// Hottest phosphor trace: the ship, sweep glints, arrival pulses.
pub const PHOSPHOR_HOT: Color = hex(0xbd_f2_9b);

/// Invitation and warning lamps, ETA arc, want pips.
pub const AMBER: Color = hex(0xe8_a3_3d);

/// Lever handles and hardware accents.
pub const BRASS: Color = hex(0xb0_8d_57);

/// Ready/go lamps (accept lever, launch lever).
pub const LAMP_OK: Color = hex(0x59_c1_35);

/// Refusal flashes, violation glyphs.
pub const LAMP_NO: Color = hex(0xd8_4a_35);

/// The suspicious violet: crate glow, omen cast.
pub const EERIE: Color = hex(0x8f_5f_d6);

/// The violet at full flash: the jump itself, the crate's hot core.
pub const EERIE_BRIGHT: Color = hex(0xb3_88_ea);

/// Warm off-white for needles, sparkles, and glass glints — the doc bans
/// pure white, so anything that wants "white" takes this.
pub const GLINT: Color = hex(0xe7_e3_d1);

/// Near-black for drop shadows, vignettes, and omen bands — the doc bans
/// pure black too.
pub const SHADOW: Color = hex(0x05_07_0a);

/// Off-lamp glass: dark, but visibly still a lamp.
pub const GLASS: Color = hex(0x10_15_12);

/// Etched icon lines on metal while their function sleeps.
pub const ICON: Color = hex(0x5b_66_5e);

/// Icon lines while live but not lamp-hot (the speaker with audio running).
pub const ICON_LIT: Color = hex(0xa7_b3_a9);

// -------------------------------------------------- painted trims and misc

/// Painted trim naming the station-shelf row.
pub const TRIM_SHELF: Color = hex(0x6b_5f_80);

/// Painted trim naming the received-goods row.
pub const TRIM_RECEIVED: Color = hex(0x56_7a_5d);

/// Painted trim naming the give pads. Cooler than it was, so the amber
/// invite glow reads as light on top of it rather than more paint.
pub const TRIM_GIVE: Color = hex(0x76_71_4f);

/// Painted trim naming the take pads.
pub const TRIM_TAKE: Color = hex(0x4d_7a_80);

/// The version string: dev information outside the fiction, so it alone may
/// carry an alpha and skip the omen tint.
pub const VERSION_TEXT: Color = fade(hex(0xd9_d9_e6), 0.75);

/// Identity tint for the crunch blit — a texture multiplier, never drawn.
pub const BLIT: Color = Color::new(1.0, 1.0, 1.0, 1.0);

/// The hue the omen leans the whole panel toward (used via [`omen_tint`]).
const CAST: Color = hex(0x6b_38_94);

// ------------------------------------------------------- POI identity hues
//
// Enamel-badge colours for the planets. On the CRTs they run through
// [`phosphorize`]; at full strength they only ever appear on metal (the
// dial's station badge).

/// Venus: pink-gold resort glow.
pub const POI_VENUS: Color = hex(0xf0_b3_8c);

/// Venus's glitter halo.
pub const POI_VENUS_HALO: Color = hex(0xff_db_99);

/// Earth: blue-gray under the smog.
pub const POI_EARTH: Color = hex(0x73_8c_a6);

/// Earth's brown smog arc.
pub const POI_SMOG: Color = hex(0x7a_5c_3d);

/// Mars: rust-red.
pub const POI_MARS: Color = hex(0xc2_57_38);

/// Mars's patchwork wedge.
pub const POI_MARS_PATCH: Color = hex(0x94_3d_2b);

/// Jupiter: banded orange.
pub const POI_JUPITER: Color = hex(0xdb_99_57);

/// Uranus: pale cyan.
pub const POI_URANUS: Color = hex(0x9e_d9_e0);

/// Uranus's thin tilted ring.
pub const POI_URANUS_RING: Color = hex(0xb8_d6_e6);

/// Neptune: deep blue.
pub const POI_NEPTUNE: Color = hex(0x42_66_d9);

/// The Guild Station's heptagon fill.
pub const POI_GUILD: Color = hex(0x6b_61_8c);

/// The Guild Station's heptagon edge.
pub const POI_GUILD_EDGE: Color = hex(0x9e_8f_c7);

/// Saturn: bone-pale salvage gold.
pub const POI_SATURN: Color = hex(0xd6_c2_8a);

/// Saturn's ring of a thousand failed hauling companies.
pub const POI_SATURN_RING: Color = hex(0xa8_93_6b);

/// The Umbra Market: a sliver of moonlit slate, night trade only.
pub const POI_UMBRA: Color = hex(0x8c_93_c9);

/// The Hermitage: warm rock, one lit window.
pub const POI_HERMITAGE: Color = hex(0xa6_8a_6b);

/// The comet's icy head.
pub const POI_COMET: Color = hex(0xcf_ea_f2);

/// ???'s not-quite-there shimmer.
pub const POI_WANDERER: Color = hex(0x7d_e0_c0);

// -------------------------------------------------- cargo identity hues
//
// One saturated hue per kind — cargo tells the story and must stay legible
// at socket size — harmonized toward the palette's warmth.

/// Flat identity colour per cargo kind; variants tint it via [`variant_tint`].
#[must_use]
pub const fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::PerfumeVial => hex(0xd9_87_a6),
        Kind::GildedIdol => hex(0xe3_bc_4a),
        Kind::RationBricks => hex(0x8f_8f_4e),
        Kind::ScrapAlloy => hex(0xb0_6a_45),
        Kind::Seedlings => hex(0x6f_b8_5a),
        Kind::GasCanister => hex(0xde_84_33),
        Kind::CryoCore => hex(0x6c_c4_d9),
        Kind::BrinePearls => hex(0x93_a8_cf),
        Kind::SuspiciousCrate => hex(0x16_13_1c),
        Kind::MysteriousCrate => hex(0x5c_54_49),
        Kind::VeryMysteriousCrate => hex(0x24_10_38),
        Kind::CometIce => hex(0xbf_e4_ef),
        Kind::BottledMidnight => hex(0x1c_25_4d),
        Kind::Fluff => hex(0xe0_c9_a8),
        Kind::TransitChit => hex(0xc9_d2_56),
        Kind::CasinoChip => hex(0xd4_4f_6f),
        // Placeholder fixture hues: lamp golds, upholstery, gilt frame.
        // fixture glyph pass pending
        Kind::CeilingLamp => hex(0xe6_c9_6e),
        Kind::WallLamp => hex(0xdd_b8_5f),
        Kind::FloorLamp => hex(0xd2_a8_52),
        Kind::Couch => hex(0x8a_5a_6b),
        Kind::Painting => hex(0xb0_8a_3e),
    }
}

/// Sibling-crate hue shifts, one per cosmetic variant roll.
const VARIANT_SHIFT: [(f32, f32, f32); 4] = [
    (0.0, 0.0, 0.0),
    (0.06, -0.03, -0.02),
    (-0.05, 0.04, 0.02),
    (0.02, 0.02, -0.06),
];

/// Same kind, sibling crate: a small deterministic hue shift.
#[must_use]
pub fn variant_tint(col: Color, variant: u8) -> Color {
    let (dr, dg, db) = VARIANT_SHIFT[usize::from(variant % 4)];
    Color::new(
        (col.r + dr).clamp(0.0, 1.0),
        (col.g + dg).clamp(0.0, 1.0),
        (col.b + db).clamp(0.0, 1.0),
        col.a,
    )
}

// ----------------------------------------------------------------- helpers

/// Alpha-scaled copy of `col`.
#[must_use]
pub const fn fade(col: Color, alpha: f32) -> Color {
    Color::new(col.r, col.g, col.b, col.a * alpha)
}

/// Brightness-scaled copy of `col`.
#[must_use]
pub const fn dim(col: Color, by: f32) -> Color {
    Color::new(col.r * by, col.g * by, col.b * by, col.a)
}

/// Linear interpolation, shared by every colour ramp.
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}

/// Channel-wise blend from `a` to `b`.
#[must_use]
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        lerp(a.r, b.r, t),
        lerp(a.g, b.g, t),
        lerp(a.b, b.b, t),
        lerp(a.a, b.a, t),
    )
}

/// The console light level times the omen's violet cast: every tinted draw
/// call runs through here, so the whole palette dims as one and no element
/// opts out.
#[must_use]
pub fn omen_tint(col: Color, light: f32, omen: f32) -> Color {
    let cast = omen * 0.35;
    Color::new(
        (CAST.r - col.r).mul_add(cast, col.r) * light,
        (CAST.g - col.g).mul_add(cast, col.g) * light,
        (CAST.b - col.b).mul_add(cast, col.b) * light,
        col.a,
    )
}

/// A CRT's reading of `col`: its brightness mapped onto the phosphor ramp,
/// then leaned toward that reading by `amount` (0 = the real colour, 1 =
/// pure phosphor). Identity survives because silhouettes and brightness do.
#[must_use]
pub fn phosphorize(col: Color, amount: f32) -> Color {
    let luma = col.r.mul_add(0.299, col.g.mul_add(0.587, col.b * 0.114));
    let ramp = if luma > 0.75 {
        mix(PHOSPHOR, PHOSPHOR_HOT, (luma - 0.75) * 4.0)
    } else {
        mix(PHOSPHOR_DIM, PHOSPHOR, luma / 0.75)
    };
    // Keep the caller's alpha: phosphor changes hue, not translucency.
    let leaned = mix(col, ramp, amount);
    Color::new(leaned.r, leaned.g, leaned.b, col.a)
}

// ------------------------------------------------------------------ guards

#[cfg(test)]
mod tests {
    /// The frontend sources that must stay palette-pure. `palette.rs` itself
    /// is exempt: it is where the constructors are supposed to live.
    const SOURCES: [(&str, &str); 3] = [
        ("src/main.rs", include_str!("main.rs")),
        ("src/render.rs", include_str!("render.rs")),
        ("src/juice.rs", include_str!("juice.rs")),
    ];

    /// String scan only — macroquad globals panic under `cargo test`, so
    /// this guard must never touch the graphics context.
    #[test]
    fn frontend_draws_only_through_palette_roles() {
        // Constructor spellings. `Color {` alone would also match type
        // ascriptions like `-> Color {`, so the struct-literal patterns
        // include the first field.
        let raw = [
            "Color::new",
            "color_u8",
            "Color { r",
            "Color{r",
            "Color::from",
            "::colors::",
        ];
        for (name, src) in SOURCES {
            for pattern in raw {
                assert!(
                    !src.contains(pattern),
                    "{name} contains raw colour constructor `{pattern}`; \
                     add or reuse a role in src/palette.rs instead"
                );
            }
        }
    }
}
