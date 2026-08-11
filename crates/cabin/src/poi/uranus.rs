//! **Uranus** — outer ring, and the chart draws it with its rings
//! (`palette::accent::URANUS_RING`).
//!
//! What there was to go on, and what each thing turned into:
//!
//! - Uranus's row in `sim::barter::VALUE` carries one zero, and a zero is
//!   where a kind ENTERS the world: **the cryo core**. So the station is
//!   a **cold works** — the place the system's cold is made and canned —
//!   and the room reads like the inside of a chill store: pale, flat, too
//!   bright, rimed along every low edge, and colder than anywhere else on
//!   the chart by a wide margin.
//! - It pays four for gas and four for seedlings and one for pearls: a
//!   works that buys what keeps people alive and has no use at all for
//!   what looks nice on a shelf. Nothing in the room is decorative. The
//!   cold bank over the counter is stock hanging in its own rack,
//!   because a cryo core cannot be put down on a warm floor.
//! - **The planet lies on its side.** That is the one fact about Uranus
//!   everybody knows, and it is the design: this station is built to a
//!   different up. Where every other place on the chart stands its mast
//!   on the crown, Uranus's derrick comes **straight out of the flank**,
//!   with the ice it has caught still hanging off it. From alongside, the
//!   station reads as a building that fell over and kept working.
//! - So the space elevator gets the same treatment as the ice giant's
//!   missing ground: there is no ribbon, because there is nowhere to
//!   drop one to. What there is instead is that sideways derrick, lowered
//!   into the **ring plane** — which at Uranus stands upright while
//!   everybody else's lies flat — and hauling ice back up it. The
//!   elevator is horizontal here, and that is not a joke, it is the
//!   obliquity.
//! - Heat is the whole problem with making cold. The crown carries a
//!   **comb of radiator fins** in the ring's own pale blue: five thin
//!   plates standing up off the roof, which is the silhouette you see
//!   first and the thing no other station has.
//!
//! The handshake is a **cold spindle**: a frosted drum you turn a quarter
//! turn, like the catch on a chill-store door. It has the shortest throw
//! on the chart — a hand's width of nothing — because everything here is
//! stiff, and a mechanism that moved freely at this temperature would be
//! a mechanism with a leak.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Uranus's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: COLD_SPINDLE,
    light: CHILL_DRUM,
    decor: &THE_COLD_STORE,
    outfit: Outfit {
        plate: palette::POI_URANUS,
        // Running lights in the cryo core's own cyan: the colour of the
        // one thing this station makes, burning on the outside of it.
        lamp: palette::kind_color(Kind::CryoCore),
        lamps: 2,
    },
    dress: &THE_COLD_WORKS,
};

/// The chill store's paint.
///
/// `Stock` keeps its filled field and `Offer` its struck line — not a
/// station's to spend — but the value relationship is **inverted** here
/// against every warm station on the chart: a pale, almost white field
/// with a near-black band at its edge, the way a cold room is painted so
/// that frost shows and dirt cannot hide. Jupiter's shelf is soot banded
/// in ochre; this one is ice banded in shadow, and the two do not read as
/// the same room with the hue turned.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::accent::URANUS_RING),
    rim: Coat::metal(Worn::Socket),
    chalk: Coat::etched(palette::kind_color(Kind::CryoCore)),
    stud: Coat::metal(Worn::Socket),
    sill: Coat::etched(palette::accent::URANUS_RING),
};

/// **The cold spindle.** A white drum standing proud of a dark plate,
/// with a frost ledge under it, two vapour bleeds either side, and a pair
/// of ice pips over the top. A quarter turn commits; it barely moves,
/// which is the point.
const COLD_SPINDLE: Handshake = Handshake {
    plate: Coat::metal(Worn::Socket),
    knob: Shape::Post,
    knob_coat: Coat::enamel(palette::accent::URANUS_RING),
    knob_at: Vec3::new(0.0, 0.0, 0.10),
    knob_half: Vec3::new(0.26, 0.34, 0.055),
    // The shortest travel on the chart. Cold seizes everything.
    throw: 0.018,
    lamp: palette::kind_color(Kind::CryoCore),
    trim: &SPINDLE_WORKS,
};

/// The spindle's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const SPINDLE_WORKS: [Fitting; 7] = [
    // The ledge it turns on, thick with frost.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::URANUS_RING),
        Vec3::new(0.0, -0.46, 0.07),
        Vec3::new(0.44, 0.06, 0.07),
    ),
    // The collar behind it, cut into the plate.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, 0.0, 0.025),
        Vec3::new(0.34, 0.40, 0.025),
    ),
    // Two vapour bleeds, venting where the seal is losing.
    bleed(-0.60),
    bleed(0.60),
    // Two ice pips over the top, lit from inside the way a core is.
    pip(-0.58),
    pip(0.58),
    // The catch plate the spindle stops against.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.52, 0.05),
        Vec3::new(0.30, 0.05, 0.045),
    ),
];

/// One vapour bleed on the spindle's plate, at `x` across the cell.
const fn bleed(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::accent::URANUS_RING, 1.2),
        Vec3::new(x, -0.30, 0.04),
        Vec3::new(0.16, 0.03, 0.012),
    )
}

/// One ice pip over the spindle, at `x` across the cell.
const fn pip(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::CometIce), 2.0),
        Vec3::new(x, 0.68, 0.05),
        Vec3::new(0.06, 0.06, 0.04),
    )
}

/// **The chill drum.** A finned cylinder burning the full budget in the
/// ring's own pale blue: a cold store is over-lit and flat-lit, because
/// shadow is where spoilage hides, and because the crew here have been
/// awake for nineteen hours and would like everybody to know it.
const CHILL_DRUM: Light = Light {
    color: palette::accent::URANUS_RING,
    burn: 1.0,
    shade: Shape::Post,
    shade_coat: Coat::metal(Worn::Plate),
    glass: Coat::phosphor(palette::POI_URANUS, 2.0),
    cage: &DRUM_FINS,
};

/// Four fins round the drum, measured off a box one shade across on every
/// side of the lamp. The same heat problem the crown outside has, at the
/// scale of one light fitting.
const DRUM_FINS: [Fitting; 4] = [
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.65, -0.10, 0.0),
        Vec3::new(0.30, 0.45, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.65, -0.10, 0.0),
        Vec3::new(0.30, 0.45, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, -0.10, 0.65),
        Vec3::new(0.05, 0.45, 0.30),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, -0.10, -0.65),
        Vec3::new(0.05, 0.45, 0.30),
    ),
];

/// **The cold store**, inside: the bank of cores hanging off its rail
/// over the counter, rime along both low walls where the hull is coldest, a
/// core out of its cradle on the deck, and the two ducts overhead that
/// keep the whole room at whatever this is.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents.
const THE_COLD_STORE: [Fitting; 15] = [
    // The bank: three cores hung off a rail over the counter, lit from
    // inside. Cold is the produce, and the produce is glowing — over the
    // one wall anybody standing in the doorway is looking at.
    core(-0.25),
    core(0.58),
    core(0.90),
    // The rail they hang from, and the chill main down the port wall
    // that charged them.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.32, 0.32, 0.62),
        Vec3::new(0.62, 0.03, 0.03),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.93, 0.30, -0.30),
        Vec3::new(0.05, 0.05, 0.62),
    ),
    // Rime along the foot of both side walls: the low edges are where a
    // hull this cold sweats, and the sweat has been there for years.
    rime(-0.955),
    rime(0.955),
    // A core out of the bank, sitting in its cradle on the deck where
    // somebody left it. The cradle is a hoop; the core is a bulb of
    // very cold nothing.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Plate),
        Vec3::new(0.68, -0.92, 0.50),
        Vec3::new(0.17, 0.08, 0.20),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::CometIce), 1.6),
        Vec3::new(0.68, -0.86, 0.50),
        Vec3::new(0.11, 0.12, 0.13),
    ),
    // Two chill ducts down the ceiling, each with its vent lit pale.
    duct(-0.52),
    duct(0.52),
    vent(-0.52),
    vent(0.52),
    // The frost line where the aft wall meets the cornice: the coldest
    // seam in the room, and the only mark on it.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::URANUS_RING),
        Vec3::new(0.25, 0.90, 0.955),
        Vec3::new(0.68, 0.035, 0.014),
    ),
    // And the door seal on the aft wall's own jamb side: a white gasket
    // strip, because a cold store's door is the expensive part.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::URANUS_RING),
        Vec3::new(-0.26, -0.30, 0.955),
        Vec3::new(0.025, 0.66, 0.014),
    ),
];

/// One cryo core hanging off the rail, at `x` across the room.
const fn core(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::phosphor(palette::kind_color(Kind::CryoCore), 1.5),
        Vec3::new(x, 0.02, 0.62),
        Vec3::new(0.05, 0.22, 0.06),
    )
}

/// One rime line along the foot of a side wall, at `x`.
const fn rime(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::accent::URANUS_RING),
        Vec3::new(x, -0.82, -0.20),
        Vec3::new(0.03, 0.09, 0.72),
    )
}

/// One chill duct along the ceiling, at `x` across the room.
const fn duct(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(x, 0.93, -0.20),
        Vec3::new(0.13, 0.06, 0.62),
    )
}

/// One duct's vent, lit the pale blue everything cold is lit here.
const fn vent(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::accent::URANUS_RING, 1.0),
        Vec3::new(x, 0.86, -0.20),
        Vec3::new(0.09, 0.02, 0.50),
    )
}

/// **The cold works**, outside: the radiator comb standing off the crown,
/// the ice derrick out of the starboard flank with the catch still on it,
/// and the frost-blown hatch in the outboard face.
///
/// Out here there is **no light to speak of**, so every reading is either
/// etched (the "findable with the lamps sold" floor) or a phosphor. The
/// fins are etched rather than lit: a radiator that glowed would be a
/// radiator that had failed, and this station's whole business is that
/// its radiators have not.
const THE_COLD_WORKS: [Fitting; 15] = [
    // ---- the comb ----
    // Five fins standing off the roof. Making cold means throwing heat
    // away, and there is nowhere out here to throw it but at the sky.
    fin(-0.70),
    fin(-0.35),
    fin(0.0),
    fin(0.35),
    fin(0.70),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.0, 1.03, 0.0),
        Vec3::new(0.80, 0.03, 0.66),
    ),
    // ---- the derrick, out of the FLANK ----
    // Every other station on the chart stands its spar on the crown.
    // This one comes out of the side, because at Uranus the ring plane
    // stands upright and the ice is over there, not down there.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(1.76, 0.05, -0.30),
        Vec3::new(0.66, 0.045, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(1.48, 0.05, -0.30),
        Vec3::new(0.03, 0.22, 0.03),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(2.02, 0.05, -0.30),
        Vec3::new(0.03, 0.17, 0.03),
    ),
    // The catch: two lumps of ring ice still hanging off the line.
    Fitting::new(
        Shape::Dome,
        Coat::etched(palette::kind_color(Kind::CometIce)),
        Vec3::new(1.62, 0.05, -0.30),
        Vec3::new(0.13, 0.16, 0.16),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::etched(palette::kind_color(Kind::CometIce)),
        Vec3::new(2.10, 0.02, -0.30),
        Vec3::new(0.10, 0.12, 0.12),
    ),
    // The head lamp on the end of it, so the derrick can see what it is
    // grabbing at.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::CryoCore), 3.5),
        Vec3::new(2.42, 0.05, -0.30),
        Vec3::new(0.10, 0.10, 0.10),
    ),
    // ---- the freight hatch, in the outboard face ----
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::URANUS_RING),
        Vec3::new(0.0, -0.15, -1.06),
        Vec3::new(0.40, 0.34, 0.04),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_URANUS, 0.6),
        Vec3::new(0.0, -0.15, -1.13),
        Vec3::new(0.32, 0.26, 0.03),
    ),
    // And the plume off its seal, which nobody has got round to.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::accent::URANUS_RING, 0.5),
        Vec3::new(-0.62, -0.52, -1.10),
        Vec3::new(0.03, 0.30, 0.04),
    ),
];

/// One radiator fin on the crown, at `x` across it.
const fn fin(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::URANUS_RING),
        Vec3::new(x, 1.34, 0.0),
        Vec3::new(0.03, 0.34, 0.62),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uranus's own reading: cold is the produce and it is glowing in the
    /// room, the paint is inverted against every warm station (pale field,
    /// dark rim), the mechanism barely moves, and the spar comes out of
    /// the SIDE. A change that quietly retires one of these retires the
    /// obliquity, which is the only thing anybody knows about this planet.
    #[test]
    fn uranus_is_a_cold_works_lying_on_its_side() {
        assert_eq!(CHARACTER.tiles.stock.color, palette::accent::URANUS_RING);
        assert_eq!(CHARACTER.tiles.rim.color, palette::SOCKET, "pale on dark");
        // Cold seizes everything: the shortest travel on the chart.
        const { assert!(CHARACTER.handshake.throw < super::super::NEUTRAL.handshake.throw) }
        // The produce is standing in the room, lit from inside.
        let cores = CHARACTER
            .decor
            .iter()
            .filter(|fitting| fitting.coat.color == palette::kind_color(Kind::CryoCore))
            .count();
        assert!(cores >= 3, "the cold bank is empty");
        // The comb: five fins, standing clear of the crown.
        let fins = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.at.y > 1.2 && fitting.half.z > 0.5)
            .count();
        assert_eq!(fins, 5, "the radiators have gone");
        // And the derrick reaches further out the flank than anything
        // this station puts over its own roof.
        let flank = CHARACTER
            .dress
            .iter()
            .map(|fitting| fitting.at.x + fitting.half.x)
            .fold(0.0_f32, f32::max);
        let crown = CHARACTER
            .dress
            .iter()
            .map(|fitting| fitting.at.y + fitting.half.y)
            .fold(0.0_f32, f32::max);
        assert!(flank > crown, "the spar stood up like everybody else's");
    }
}
