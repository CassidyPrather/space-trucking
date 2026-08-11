//! **Venus** — where the rich went to live, and they are unimaginably
//! tacky (DESIGN.md).
//!
//! What the lore and the ledger say, and what each line turned into:
//!
//! - Venus's produce is **perfume**: the one kind its barter row prices
//!   at zero (`sim::barter::VALUE`), and a zero is where a kind enters
//!   the world. So the goods are shown the way a perfumer shows them —
//!   two vials, lit from behind, on a plinth nobody may lean on.
//! - It pays **six** for a painting, the highest number anywhere in the
//!   table, and one more again for a painting that arrives under
//!   lamplight (`barter::LIT_BONUS`). A house that pays a premium for
//!   light on art hangs its own art under a picture light, so it does,
//!   on the port flank, and the light is on.
//! - It pays **one** for a gilded idol. Gilt is cheap on Venus because
//!   Venus is already gilt: the bead, the bell, the stanchions, the
//!   cornice and the cable outside are all the same brass, and none of
//!   it is holding anything up.
//! - The inner ring's factions refuse each other's traffic
//!   (`sim::map::INNER_RING`), which is why a transit chit exists at
//!   all — so this is not a market floor. It is a **salon**, and the
//!   stock stands behind a velvet rope. The Guild keeps its goods
//!   behind bars; Venus keeps them behind manners, which is the same
//!   sentence in a better register.
//!
//! The handshake is a **bell pull**: a brass ring on a rose-lacquer
//! escutcheon, under a bell and its clapper. You do not get stamped
//! here and you do not touch anything — you ring, and somebody comes.
//!
//! Outside, the owner's space elevator in Venus's reading of it: a
//! gold-plated cable, beaded like a bracelet, a lit finial on top, a
//! tasselled car under the keel, and three lit windows along the face
//! like a shopfront. Nothing about the cable looks strong enough to
//! lift anything, and nobody has ever raised the point.

use bevy::prelude::{Color, Vec3};
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Venus's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: BELL_PULL,
    light: CHANDELIER,
    decor: &SALON,
    outfit: Outfit {
        // Rose-gold plate and a pale gold running light: the house
        // colours, worn outside so nobody arrives by accident.
        plate: palette::POI_VENUS,
        lamp: palette::accent::VENUS_HALO,
        lamps: 2,
    },
    dress: &GILT_FACE,
};

/// The salon floor.
///
/// `Stock` keeps its filled field and `Offer` its struck line — that
/// reading is not a station's to spend — but the paint under the goods
/// is Venus's own rose rather than the neutral grey, and the line struck
/// round a proposal is **gilt, and lit**: what you have put on the floor
/// is being framed while you watch, by a house that pays six for a
/// frame. The threshold is gilded too, studs and sill, because the
/// expensive part of this room is the part you walk on.
///
/// The field is [`palette::POI_VENUS`] taken **down** toward the shadow
/// role rather than used at full strength, and the reason is the gilt:
/// raw, the chart's peach sits at almost exactly brass's value, so every
/// fitting in the room sank into the wall and read as shaded peach
/// instead of as gold. A salon is deep rose with gold on it. It is not
/// gold on peach, which is a corridor.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(SALON_ROSE),
    rim: Coat::metal(Worn::Brass),
    chalk: Coat::etched(palette::accent::VENUS_HALO),
    stud: Coat::metal(Worn::Brass),
    sill: Coat::etched(palette::accent::VENUS_HALO),
};

/// The house rose: the chart's own Venus hue, dropped toward
/// [`palette::SHADOW`] until brass reads as metal against it.
const SALON_ROSE: Color = palette::blend(palette::POI_VENUS, palette::SHADOW, 0.38);

/// **The bell pull.** A brass ring on a cord under a plum-lacquer plate,
/// with the bell it rings and the clapper above it, and a gilt bead
/// moulding round the whole arrangement. It throws further than the
/// neutral plunger and further than the Guild's press, because a pull is
/// a pull; what it does not do is stamp you.
const BELL_PULL: Handshake = Handshake {
    plate: Coat::enamel(palette::kind_color(Kind::Couch)),
    knob: Shape::Ring,
    knob_coat: Coat::metal(Worn::Brass),
    knob_at: Vec3::new(0.0, -0.34, 0.12),
    knob_half: Vec3::new(0.34, 0.42, 0.085),
    throw: 0.06,
    // The lamp that says there is something to commit, in the house's
    // own gold rather than the ordinary amber.
    lamp: palette::accent::VENUS_HALO,
    trim: &BELL_WORKS,
};

/// The pull's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const BELL_WORKS: [Fitting; 8] = [
    // The rosette the cord comes out of.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.20, 0.045),
        Vec3::new(0.26, 0.26, 0.030),
    ),
    // The cord itself, bell to ring. Without it a hoop on a wall is a
    // hoop on a wall; with it, the whole fixture is a bell pull.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, -0.06, 0.115),
        Vec3::new(0.030, 0.30, 0.014),
    ),
    // The bell above it — a cone is a bell, seen from anywhere.
    Fitting::new(
        Shape::Cone,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.56, 0.085),
        Vec3::new(0.22, 0.20, 0.075),
    ),
    // Its clapper, hanging where a clapper hangs.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.33, 0.085),
        Vec3::new(0.055, 0.055, 0.035),
    ),
    // Two tassels, because two tassels.
    Fitting::new(
        Shape::Cone,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.62, -0.20, 0.050),
        Vec3::new(0.10, 0.22, 0.035),
    ),
    Fitting::new(
        Shape::Cone,
        Coat::metal(Worn::Brass),
        Vec3::new(0.62, -0.20, 0.050),
        Vec3::new(0.10, 0.22, 0.035),
    ),
    // Gilt bead moulding, top and bottom: lit, so the fixture is gold
    // even with every lamp aboard sold as cargo.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.0, 0.80, 0.040),
        Vec3::new(0.80, 0.030, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.0, -0.76, 0.040),
        Vec3::new(0.80, 0.030, 0.020),
    ),
];

/// **The chandelier.** Venus lights its floor the way a jeweller lights
/// a window: the full caller budget, poured through gold, off a gilt
/// crown hoop with a second hoop under it and four glass drops hanging
/// between them. It is a glare rather than a light, and that is the
/// point — a customer who can see comfortably is a customer thinking.
const CHANDELIER: Light = Light {
    color: palette::accent::VENUS_HALO,
    burn: 1.0,
    shade: Shape::Ring,
    shade_coat: Coat::metal(Worn::Brass),
    glass: Coat::phosphor(palette::accent::VENUS_HALO, 2.6),
    cage: &LUSTRE,
};

/// The chandelier's own tiers, measured off a box one shade across on
/// every side of the lamp — never off the room, so no amount of gilt
/// becomes a beam across it.
const LUSTRE: [Fitting; 8] = [
    // The crown hoop, above.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.62, 0.0),
        Vec3::new(0.80, 0.45, 0.80),
    ),
    // The skirt hoop, below and wider.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, -0.56, 0.0),
        Vec3::new(0.97, 0.42, 0.97),
    ),
    // Four drops between the tiers.
    crystal(-0.78, 0.0),
    crystal(0.78, 0.0),
    crystal(0.0, -0.78),
    crystal(0.0, 0.78),
    // A gilt ball where the drops meet, and a small one on top of the
    // crown: the fixture has two finials and needs none.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, -0.76, 0.0),
        Vec3::new(0.17, 0.17, 0.17),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 1.00, 0.0),
        Vec3::new(0.16, 0.16, 0.16),
    ),
];

/// One glass drop off the chandelier, at `(x, z)` on the lamp's box.
const fn crystal(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::GLINT, 1.8),
        Vec3::new(x, -0.42, z),
        Vec3::new(0.17, 0.34, 0.17),
    )
}

/// **The salon**, inside the room: the rope line in front of the goods,
/// the vitrine the perfume is shown in, the house's own painting under
/// its own light, and gilt along the cornice.
///
/// The frame is the room's box — `+x` starboard, `+y` up, `+z` aft — and
/// every number is a fraction of its half-extents, so none of this had
/// to know how big a trade room is.
const SALON: [Fitting; 17] = [
    // The rope line: two stanchions with ball finials and a velvet rope
    // between them, standing a hand off the goods. It stops well short
    // of the doorway, because a threshold belongs to two rooms and this
    // one owns neither.
    stanchion(-0.14),
    stanchion(0.92),
    finial(-0.14),
    finial(0.92),
    Fitting::new(
        Shape::Slab,
        // Upholstery plum: the rope is the same cloth as the couches
        // Venus buys at four, which is not a coincidence anybody here
        // would admit to.
        Coat::enamel(palette::kind_color(Kind::Couch)),
        Vec3::new(0.39, -0.55, 0.52),
        Vec3::new(0.53, 0.026, 0.026),
    ),
    // The vitrine on the starboard flank: a rose plinth, a shelf, a lit
    // back panel, and the house produce standing on it. Two vials, both
    // out of reach, both worth nothing here and a fortune anywhere else.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_VENUS),
        Vec3::new(0.88, -0.74, -0.42),
        Vec3::new(0.10, 0.26, 0.20),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.88, -0.46, -0.42),
        Vec3::new(0.10, 0.020, 0.19),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::accent::VENUS_HALO, 0.9),
        Vec3::new(0.945, -0.28, -0.42),
        Vec3::new(0.020, 0.14, 0.18),
    ),
    vial(-0.52),
    vial(-0.32),
    // The house painting, port flank, in a brass frame under a hooded
    // picture light. The subject is gold. The subject is always gold.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.965, 0.12, -0.60),
        Vec3::new(0.020, 0.32, 0.34),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::VENUS_HALO),
        Vec3::new(-0.935, 0.12, -0.60),
        Vec3::new(0.014, 0.27, 0.29),
    ),
    Fitting::new(
        Shape::Cone,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.86, 0.58, -0.60),
        Vec3::new(0.065, 0.10, 0.11),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::accent::VENUS_HALO, 3.2),
        Vec3::new(-0.828, 0.43, -0.60),
        Vec3::new(0.020, 0.018, 0.090),
    ),
    // Gilt along the aft cornice, with two bosses on it. Lit, because a
    // house that gilds a cornice is not going to let the dark have it.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.24, 0.88, 0.945),
        Vec3::new(0.72, 0.025, 0.020),
    ),
    boss(-0.20),
    boss(0.68),
];

/// One rope stanchion, at `x` across the room.
const fn stanchion(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Brass),
        Vec3::new(x, -0.74, 0.52),
        Vec3::new(0.030, 0.26, 0.030),
    )
}

/// The ball on top of one stanchion.
const fn finial(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(x, -0.43, 0.52),
        Vec3::new(0.048, 0.09, 0.048),
    )
}

/// One vial of the house perfume, standing on the vitrine shelf at `z`.
const fn vial(z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::PerfumeVial), 2.4),
        Vec3::new(0.86, -0.36, z),
        Vec3::new(0.026, 0.070, 0.034),
    )
}

/// One gilt boss on the cornice, at `x`.
const fn boss(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(x, 0.88, 0.92),
        Vec3::new(0.050, 0.09, 0.035),
    )
}

/// **The gilt face**, outside: three lit windows along the outboard
/// wall, gold banding above and below them, ball finials on the roof
/// line, and the elevator.
///
/// Venus's cable is the owner's space elevator taken to the salon and
/// handed to a jeweller. It is beaded, because a plain cable would have
/// been a plain cable; it is finialled, because it ends; and it **ends**,
/// a body's length above the roof and a body's length under the keel,
/// which is the joke. Earth's ribbon runs off both ends of any frame you
/// can photograph it in and Mars's is severed and under repair. This one
/// is jewellery, and the house has never been asked what it lifts.
///
/// Out here there is **no light at all**: the void carries none and the
/// art direction runs no shadow maps, so a plate's own colour is nearly
/// black and only what glows is seen. Hence every reading below is
/// either radium gilt (`Finish::Etched`, the game's findable-with-the-
/// lamps-sold floor) or a phosphor, and the windows burn at **0.6**
/// rather than at indicator brightness: a lit room seen from outside is
/// a depth, not a lightbox.
const GILT_FACE: [Fitting; 16] = [
    // Three lit windows, evenly spaced along the face. A shopfront, not
    // a hangar mouth: you are meant to look in, and not to come in that
    // way.
    window(-0.52),
    window(0.0),
    window(0.52),
    // Gold banding over and under them.
    band(0.36),
    band(-0.46),
    // The cable, above the roof and under the keel, with three beads on
    // the upper length and a lit finial on top of it.
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.0, 1.30, -0.12),
        Vec3::new(0.045, 0.28, 0.045),
    ),
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.0, -1.22, -0.12),
        Vec3::new(0.045, 0.20, 0.045),
    ),
    bead(1.12),
    bead(1.34),
    bead(1.52),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::accent::VENUS_HALO, 4.0),
        Vec3::new(0.0, 1.70, -0.12),
        Vec3::new(0.11, 0.11, 0.11),
    ),
    // The car, under the keel: a tassel with a light in it. Whether it
    // has ever gone anywhere is not a question the house entertains.
    Fitting::new(
        Shape::Cone,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, -1.50, -0.12),
        Vec3::new(0.17, 0.17, 0.17),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::accent::VENUS_HALO, 2.5),
        Vec3::new(0.0, -1.72, -0.12),
        Vec3::new(0.08, 0.08, 0.08),
    ),
    // Two ball finials on the roof line, and a gilt swag along the keel.
    roof_ball(-0.72),
    roof_ball(0.72),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, -1.06, 0.0),
        Vec3::new(0.75, 0.030, 0.60),
    ),
];

/// One lit window in the outboard wall, at `x`.
const fn window(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_VENUS, 0.6),
        Vec3::new(x, -0.05, -1.10),
        Vec3::new(0.12, 0.34, 0.030),
    )
}

/// One gold band across the outboard wall, at height `y`.
const fn band(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::VENUS_HALO),
        Vec3::new(0.0, y, -1.10),
        Vec3::new(0.80, 0.045, 0.035),
    )
}

/// One bead on the elevator cable, at height `y`.
const fn bead(y: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, y, -0.12),
        Vec3::new(0.17, 0.09, 0.17),
    )
}

/// One ball finial on the roof line, at `x`.
const fn roof_ball(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::etched(palette::BRASS),
        Vec3::new(x, 1.12, -0.55),
        Vec3::new(0.10, 0.10, 0.10),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Venus's own reading, held where the lore and the ledger put it:
    /// the house rings rather than stamps, the gilt is everywhere and
    /// load-bearing nowhere, and the produce is shown under glass.
    /// Repaint it freely — a change that quietly retires one of these is
    /// a change that retires Venus.
    #[test]
    fn venus_is_a_salon_that_rings_for_service() {
        assert_eq!(CHARACTER.handshake.knob, Shape::Ring, "Venus rings");
        assert_ne!(
            CHARACTER.handshake.knob,
            super::super::NEUTRAL.handshake.knob,
            "a repainted plunger is not a bell pull"
        );
        // The field is the chart's Venus hue taken down, not the raw
        // role: gold on peach is a corridor, gold on deep rose is a
        // salon.
        assert_eq!(CHARACTER.tiles.stock.color, SALON_ROSE);
        assert_ne!(SALON_ROSE, palette::POI_VENUS, "the rose went raw again");
        const {
            assert!(
                CHARACTER.light.burn > 0.99,
                "the salon burns the whole budget: a comfortable customer is a customer thinking"
            );
        }
        // Gilt outnumbers everything: over half the furniture in the room
        // is brass, and none of it is holding the room up.
        let brass = CHARACTER
            .decor
            .iter()
            .chain(CHARACTER.handshake.trim.iter())
            .filter(|fitting| fitting.coat.color == palette::BRASS)
            .count();
        assert!(brass >= 10, "Venus has stopped being tacky: {brass} gilt");
        // And the elevator is out there, beaded, running both ways past
        // a station it is not obviously attached to.
        let rings = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.shape == Shape::Ring)
            .count();
        assert!(rings >= 3, "the cable lost its beads");
    }
}
