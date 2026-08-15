//! **Mars** — broke off in a rebellion and is now a scrappy republic
//! (DESIGN.md). The ledger says the rest of it:
//!
//! - Mars's produce is **scrap alloy**: the one kind its barter row
//!   prices at zero (`sim::barter::VALUE`), and a zero is where a kind
//!   enters the world. Offcuts stand in a bin by the goods, sold by the
//!   armful, cut off something that used to be a ship.
//! - It pays **four for ration bricks** and **four for a tin of
//!   enamel** — the joint best price for paint anywhere on the chart.
//!   A republic that buys food and paint and sells metal is a republic
//!   that welds its own counter and then paints it, so the counter is
//!   three slabs at three heights in three different tins, and the tins
//!   are still on the deck where somebody put them down.
//! - Nothing here matches. The bolts are three different bolts, the
//!   conduit is clamped up with the same mustard enamel as the lamp's
//!   taped collar, and the second bulb is wired in beside the first
//!   because the first one is not enough and nobody is coming.
//! - It pays **one for a gilded idol**, same as Venus, for the exact
//!   opposite reason. There is no brass in this room at all.
//!
//! The floor lines are struck in **luminous paint** (`Kind::LuminousPaint`,
//! which the Umbra Market sells snuffed and Mars buys by the tin): Mars
//! marks its own bay, in the dark, with a brush, and the marks are still
//! there when every lamp aboard has been sold.
//!
//! The handshake is a **welded valve**: a scavenged bonnet painted teal
//! on a raw plate patch, with a bar welded across it and a glow stripe
//! under it so you can find it with the lights out. It throws further
//! than anything else on the ring, and its lamp is plain green. Mars is
//! the one inner-ring station that will simply do the deal.
//!
//! Outside, the owner's space elevator as Mars has it: the cable they
//! cut in the rebellion, and have been mending downward ever since.
//! Above the roof it is three lengths in three different enamels, two
//! splice collars welded out of the local produce, a repair sled still
//! clamped on with its light burning, and a strut braced off the roof
//! because the splice does not trust itself. Below the keel it stops at
//! a torn ferrule with three strands hanging out of it. The length that
//! used to reach the ground is simply not there.

use bevy::prelude::{Color, Vec3};
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Mars's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: WELDED_VALVE,
    light: MENDED_LAMP,
    decor: &FIELD_SHOP,
    outfit: Outfit {
        // Rust plate and two ember running lights: the colour of the
        // planet and the colour of the torch that built the station.
        plate: palette::POI_MARS,
        lamp: palette::EMBER,
        lamps: 2,
    },
    dress: &SPLICED_CABLE,
};

/// The republic's floor.
///
/// `Stock` keeps its filled field and `Offer` its struck line — that
/// reading is not a station's to spend — but the paint is rust over rust:
/// the goods band in Mars's own oxide, the rim where it stops in the
/// darker patch tone, as though the edge were repainted a year later out
/// of a different tin, which it was. The line round a proposal is struck
/// in **luminous paint**, by hand, and the sill is the plain oxide red
/// every hull in the game gets coated in.
///
/// The field is the chart's Mars hue taken toward [`palette::SOOT`],
/// because raw it is a traffic cone: a fresh, even, arterial red, which
/// is the one thing a scrappy republic's paint is not. Weathered, it
/// also does the job the rim needs — the rim stays the **raw** patch
/// tone, so the band where the paint stops reads as the harsher, newer
/// tin it was actually repainted out of.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(OXIDISED),
    rim: Coat::enamel(palette::accent::MARS_PATCH),
    chalk: Coat::etched(palette::kind_color(Kind::LuminousPaint)),
    stud: Coat::metal(Worn::Plate),
    sill: Coat::enamel(palette::enamel_color(0)),
};

/// Rust country: the chart's Mars red, weathered toward the scorch role
/// until it stops looking freshly sprayed.
const OXIDISED: Color = palette::blend(palette::POI_MARS, palette::SOOT, 0.35);

/// **The welded valve.** A scavenged bonnet, painted in whichever tin
/// was open, on a patch of raw plate somebody cut to fit. The bar across
/// it is welded on, the glow stripe under it is brushed on, and no two
/// of the three bolts are the same bolt. Its throw is the longest on the
/// chart, because it is a valve and valves travel.
const WELDED_VALVE: Handshake = Handshake {
    plate: Coat::metal(Worn::Plate),
    knob: Shape::Cone,
    knob_coat: Coat::enamel(palette::enamel_color(1)),
    knob_at: Vec3::new(0.0, -0.10, 0.11),
    knob_half: Vec3::new(0.30, 0.32, 0.085),
    throw: 0.075,
    // Plain green: Mars is the one inner-ring station whose fixture
    // says *yes* rather than *filed* or *noted*.
    lamp: palette::LAMP_OK,
    trim: &FIELD_REPAIR,
};

/// The valve's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const FIELD_REPAIR: [Fitting; 8] = [
    // The patch the whole fixture is set into, and an older, smaller
    // patch under one edge of it in another tin entirely.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::MARS_PATCH),
        Vec3::new(0.06, 0.02, 0.028),
        Vec3::new(0.72, 0.62, 0.010),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(3)),
        Vec3::new(-0.72, 0.10, 0.020),
        Vec3::new(0.20, 0.34, 0.008),
    ),
    // The glow stripe under it: find the fixture with the lamps sold.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::LuminousPaint)),
        Vec3::new(0.06, -0.62, 0.040),
        Vec3::new(0.72, 0.025, 0.016),
    ),
    // The bar welded across the bonnet, for hands and for leverage.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.20, 0.145),
        Vec3::new(0.44, 0.045, 0.030),
    ),
    // Three bolts, no two alike, and none of them brass.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.64, 0.58, 0.045),
        Vec3::new(0.060, 0.060, 0.025),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Socket),
        Vec3::new(0.70, 0.52, 0.045),
        Vec3::new(0.080, 0.080, 0.030),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.60, -0.55, 0.055),
        Vec3::new(0.070, 0.070, 0.028),
    ),
    // And a run of mustard down the plate where the brush got away.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(2)),
        Vec3::new(0.52, -0.33, 0.038),
        Vec3::new(0.030, 0.33, 0.012),
    ),
];

/// **The mended lamp.** A battered steel cone on straps, burning warm
/// amber at nine tenths of the caller budget — Mars lights its floor
/// properly, it just does not light it *tidily*. The collar round the
/// stem is a wrap of enamel out of the same tin as the conduit clamps,
/// and there is a second bulb wired in beside the first because one was
/// not enough and the fitting for a second one never arrived.
const MENDED_LAMP: Light = Light {
    color: palette::AMBER,
    burn: 0.9,
    shade: Shape::Cone,
    shade_coat: Coat::metal(Worn::Plate),
    glass: Coat::phosphor(palette::AMBER, 1.8),
    cage: &JURY_RIG,
};

/// The straps, the tape, and the second bulb, measured off a box one
/// shade across on every side of the lamp — never off the room.
const JURY_RIG: [Fitting; 5] = [
    // Two straps, crossed, holding it up.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.55, 0.0),
        Vec3::new(0.97, 0.060, 0.070),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.72, 0.0),
        Vec3::new(0.070, 0.060, 0.93),
    ),
    // A wrap of mustard enamel round the stem, over whatever is under
    // it. Nobody has asked.
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::enamel_color(2)),
        Vec3::new(0.0, 0.35, 0.0),
        Vec3::new(0.55, 0.55, 0.55),
    ),
    // The flex, and the bare second bulb on the end of it.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.72, 0.62, 0.35),
        Vec3::new(0.030, 0.53, 0.030),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::AMBER, 3.2),
        Vec3::new(0.72, -0.10, 0.35),
        Vec3::new(0.22, 0.22, 0.22),
    ),
];

/// **The field shop**, inside the room: the welded counter, the patch on
/// the port wall, the tins that patched it, the offcut bin the republic
/// actually sells, and the conduit somebody clamped up on the way past.
///
/// The frame is the room's box — `+x` starboard, `+y` up, `+z` aft — and
/// every number is a fraction of its half-extents, so none of this had
/// to know how big a trade room is.
const FIELD_SHOP: [Fitting; 20] = [
    // The counter: three slabs at three heights in three finishes,
    // butted end to end and standing on two legs off a scrapped rack.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(0)),
        Vec3::new(0.86, -0.30, 0.48),
        Vec3::new(0.12, 0.055, 0.22),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.86, -0.24, 0.12),
        Vec3::new(0.12, 0.055, 0.14),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(3)),
        Vec3::new(0.86, -0.34, -0.22),
        Vec3::new(0.12, 0.055, 0.20),
    ),
    leg(0.48, 0.32),
    leg(-0.30, 0.30),
    // The patch on the port wall: newer plate over a hole nobody talks
    // about, and four rivets that went in by hand.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::MARS_PATCH),
        Vec3::new(-0.955, 0.18, -0.12),
        Vec3::new(0.025, 0.34, 0.40),
    ),
    rivet(0.44, 0.21),
    rivet(0.44, -0.45),
    rivet(-0.08, 0.21),
    rivet(-0.08, -0.45),
    // The tins that did it, still on the deck, and the run one of them
    // left down the wall.
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::enamel_color(1)),
        Vec3::new(-0.70, -0.88, -0.36),
        Vec3::new(0.050, 0.100, 0.050),
    ),
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::enamel_color(2)),
        Vec3::new(-0.53, -0.90, -0.30),
        Vec3::new(0.045, 0.085, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(1)),
        Vec3::new(-0.965, -0.48, -0.36),
        Vec3::new(0.020, 0.42, 0.045),
    ),
    // The offcut bin: the republic's whole export, standing loose in a
    // box on the deck a step off the goods' own band, where anybody could
    // take one, because anybody could cut another.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.46, -0.82, 0.30),
        Vec3::new(0.19, 0.15, 0.14),
    ),
    offcut(0.38, -0.56, 0.30, 0.16),
    offcut(0.47, -0.50, 0.34, 0.20),
    offcut(0.55, -0.60, 0.26, 0.13),
    // The conduit along the ceiling, clamped up in the mustard tin.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.10, 0.90, 0.140),
        Vec3::new(0.055, 0.025, 0.55),
    ),
    pipe_clamp(-0.30),
    pipe_clamp(0.22),
];

/// One leg under the welded counter, at `z`, `high` tall.
const fn leg(z: f32, high: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.86, high - 1.0, z),
        Vec3::new(0.025, high, 0.025),
    )
}

/// One hand-driven rivet in the port wall's patch.
const fn rivet(y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.920, y, z),
        Vec3::new(0.025, 0.045, 0.030),
    )
}

/// One offcut of scrap alloy standing in the bin.
const fn offcut(x: f32, y: f32, z: f32, high: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(x, y, z),
        Vec3::new(0.020, high, 0.020),
    )
}

/// One clamp holding the ceiling conduit up, at `z`.
const fn pipe_clamp(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(2)),
        Vec3::new(-0.10, 0.90, z),
        Vec3::new(0.090, 0.075, 0.035),
    )
}

/// **The spliced cable**, outside: the owner's space elevator, cut in the
/// rebellion and mended upward ever since.
///
/// Above the roof it is three lengths in three different tins, joined by
/// collars welded out of the local produce, with the repair sled still
/// clamped on and its lamp burning and a strut braced off the roof
/// because the splice has never been trusted. Below the keel it stops:
/// a torn ferrule and three frayed strands, and then nothing. The length
/// that used to reach the ground is not there.
///
/// That is the third reading of the owner's one idea, and the reason all
/// three are worth having in one group: Venus gilded a cable that goes
/// nowhere, Earth's runs off both ends of any frame you can photograph
/// it in, and Mars's is severed and being rebuilt from the far end.
///
/// Out here there is **no light at all**: the void carries none and the
/// art direction runs no shadow maps, so a plate's own colour is nearly
/// black and only what glows is seen. Hence every reading below is
/// `Finish::Etched` (the findable-with-the-lamps-sold floor) or a
/// phosphor — and the one lit doorway burns at **0.55**, because a
/// mouth wants to look like a depth rather than a lightbox.
const SPLICED_CABLE: [Fitting; 19] = [
    // Three lengths, three tins. The join is the point.
    length(-1.68, 0.66, 0.055, palette::enamel_color(0)),
    length(1.32, 0.30, 0.062, palette::enamel_color(3)),
    length(2.20, 0.50, 0.048, palette::enamel_color(2)),
    // Two collars, cut from the republic's own alloy.
    collar(1.66),
    collar(-1.08),
    // And the break: a torn ferrule with three strands hanging out of
    // it, which is where the elevator ends and the rebellion is still,
    // as far as anybody down there is concerned, going on.
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::RIVET),
        Vec3::new(0.10, -2.30, -0.05),
        Vec3::new(0.185, 0.100, 0.185),
    ),
    strand(0.06, -2.48, -0.09, 0.16),
    strand(0.15, -2.42, -0.01, 0.10),
    strand(0.10, -2.54, -0.04, 0.22),
    // The sled that did the work, still clamped on, still lit.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(0.42, 1.30, -0.05),
        Vec3::new(0.20, 0.14, 0.12),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 3.0),
        Vec3::new(0.42, 1.30, -0.20),
        Vec3::new(0.060, 0.060, 0.060),
    ),
    // The strut braced off the roof to the splice.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.55, 1.062, -0.05),
        Vec3::new(0.50, 0.035, 0.060),
    ),
    // A patch on the outboard face, with a brushed glow line round it —
    // the same trick as the floor: mark the repair so the next crew can
    // find it in the dark.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::MARS_PATCH),
        Vec3::new(-0.35, 0.20, -1.08),
        Vec3::new(0.40, 0.30, 0.030),
    ),
    weld(0.52),
    weld(-0.12),
    // One lit doorway, hand-sized against the Guild's hangar mouth, in
    // a frame cut from the same alloy as everything else.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EMBER, 0.55),
        Vec3::new(0.45, -0.20, -1.10),
        Vec3::new(0.22, 0.30, 0.030),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(0.45, 0.13, -1.10),
        Vec3::new(0.26, 0.035, 0.025),
    ),
    // Two ribs down the flanks, at two different heights, because they
    // came off two different ships.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(-1.06, 0.20, 0.10),
        Vec3::new(0.030, 0.050, 0.62),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(1.06, -0.10, -0.20),
        Vec3::new(0.030, 0.050, 0.48),
    ),
];

/// One length of the cable: centre `y`, half-span `span`, girth `girth`,
/// painted out of whichever tin.
const fn length(y: f32, span: f32, girth: f32, tin: Color) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::etched(tin),
        Vec3::new(0.10, y, -0.05),
        Vec3::new(girth, span, girth),
    )
}

/// One strand hanging out of the cut end, `high` long.
const fn strand(x: f32, y: f32, z: f32, high: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(x, y, z),
        Vec3::new(0.012, high, 0.012),
    )
}

/// One splice collar, welded out of the local produce, at height `y`.
const fn collar(y: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::kind_color(Kind::ScrapAlloy)),
        Vec3::new(0.10, y, -0.05),
        Vec3::new(0.155, 0.075, 0.155),
    )
}

/// One brushed glow line above or below the hull patch, at height `y`.
const fn weld(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::LuminousPaint)),
        Vec3::new(-0.35, y, -1.09),
        Vec3::new(0.42, 0.025, 0.020),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mars's own reading, held where the lore and the ledger put it:
    /// nothing matches, the marks are brushed on by hand, the fixture
    /// says yes, and there is no gilt anywhere. Repaint it freely — a
    /// change that quietly retires one of these is a change that retires
    /// the republic.
    #[test]
    fn mars_is_a_field_shop_that_shows_its_repairs() {
        assert_eq!(CHARACTER.handshake.knob, Shape::Cone, "Mars welds");
        const {
            assert!(
                CHARACTER.handshake.throw > super::super::NEUTRAL.handshake.throw,
                "a valve travels"
            );
        }
        assert_eq!(CHARACTER.handshake.lamp, palette::LAMP_OK, "Mars says yes");
        // Weathered, not raw: a republic that has been repainting the
        // same wall for thirty years does not have arterial red on it.
        assert_eq!(CHARACTER.tiles.stock.color, OXIDISED);
        assert_ne!(OXIDISED, palette::POI_MARS, "the rust came up fresh again");
        // No brass. Venus pays one for a gilded idol because it is
        // drowning in gilt; Mars pays one because it has never had any.
        assert!(
            !CHARACTER
                .decor
                .iter()
                .chain(CHARACTER.handshake.trim.iter())
                .chain(CHARACTER.dress.iter())
                .any(|fitting| fitting.coat.color == palette::BRASS),
            "somebody has gilded the republic"
        );
        // Three tins at least, in the room, on things that are not the
        // same thing: the shop is a patchwork or it is a recolour.
        let tins = [0_u8, 1, 2, 3]
            .into_iter()
            .filter(|&v| {
                CHARACTER
                    .decor
                    .iter()
                    .chain(CHARACTER.dress.iter())
                    .any(|fitting| fitting.coat.color == palette::enamel_color(v))
            })
            .count();
        assert!(tins >= 3, "Mars found a paint scheme: {tins} tins");
    }
}
