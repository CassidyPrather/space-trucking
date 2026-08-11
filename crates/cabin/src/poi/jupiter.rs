//! **Jupiter** — the outer ring's frontier, where matter extraction and
//! processing went once the inner ring was exhausted (DESIGN.md).
//!
//! What the lore and the economy actually say, and what each line turned
//! into:
//!
//! - Jupiter's row in `sim::barter::VALUE` carries exactly one zero, and
//!   a zero is where a kind ENTERS the world: **the gas canister**. So
//!   Jupiter is not a market and not an office. It is a **works** — the
//!   thing at the far end of a pipe — and the room you trade in is its
//!   dispatch floor, with the plant audibly next door.
//! - It pays five for seedlings and five for bottled midnight and one for
//!   perfume. A frontier buys food, air and dark, and has nothing to say
//!   about scent. The room is therefore furnished in bottles, pipework
//!   and burnt paint, and there is not one decorative object in it.
//! - **A gas giant has no surface**, which is the whole difficulty with
//!   the owner's space elevator. You cannot anchor a ribbon to weather.
//!   Jupiter's answer is the only one the physics leaves: the ribbon
//!   hangs the other way up. The riser drops off the shell's outboard
//!   quarter to a **buoyed aerostat** floating in the cloud deck with the
//!   ram scoop under it — held up by the air it is drinking, tied to
//!   nothing at the bottom. An elevator with no ground station is still
//!   an elevator; it is just an elevator that would fall if it ever
//!   stopped working, which is the most Jovian sentence in this file.
//! - Everything a works cannot sell, it burns. The **flare stack** on the
//!   crown is the one-glance tell from outside, and its pilot shows
//!   through a slit over the aft cornice inside, so the two readings are
//!   the same fire seen from two sides.
//!
//! The handshake is a **meter valve**: a gas-orange handwheel on a
//! scorched plate, lying flat on a spindle out of the wall the way a
//! valve on a horizontal stem does. You do not shake hands at a works and
//! you do not get stamped — you crack a valve, the meter runs, and that
//! is the deal. It throws further than anything else on the chart because
//! a valve has travel in it.
//!
//! Nothing here moves the room: every fitting is a fraction of a box the
//! lattice placed, and the tiles are repaints of classes the sim
//! declared.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Jupiter's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: METER_VALVE,
    light: FLARE_LAMP,
    decor: &DISPATCH_FLOOR,
    outfit: Outfit {
        // The works paints its plate in its own ochre and burns ember
        // running lights — a station lit by the same fire it sells, which
        // is how you know whose smoke that is from two legs out.
        plate: palette::POI_JUPITER,
        lamp: palette::EMBER,
        lamps: 2,
    },
    dress: &THE_WORKS,
};

/// The dispatch floor's paint.
///
/// `Stock` keeps its filled field and `Offer` its struck line — that pair
/// is not a station's to spend — but the paint under the goods is
/// **scorched deck** rather than the neutral violet-grey, banded at its
/// edge in the works' own ochre. A shop paints its shelf; a works paints
/// a line round the part of the floor that is not on fire.
///
/// The threshold follows the same argument: dark studs, because nobody
/// polishes anything here, and a sill struck in gas-bottle orange,
/// because the colour code is the only signage a works has ever needed.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::SOOT),
    rim: Coat::enamel(palette::POI_JUPITER),
    chalk: Coat::etched(palette::POI_JUPITER),
    stud: Coat::metal(Worn::Socket),
    sill: Coat::etched(palette::kind_color(Kind::GasCanister)),
};

/// **The meter valve.** A painted handwheel on a spindle, a boss, a dial
/// with its own lit face, and the bleed pipe that runs off the top of it.
/// The wheel lies flat, because the stem comes out of the wall — which is
/// also why it reads as a wheel from the doorway and as a rim from the
/// deck, and never as a button.
const METER_VALVE: Handshake = Handshake {
    plate: Coat::enamel(palette::SOOT),
    knob: Shape::Ring,
    // Gas fittings are colour-coded everywhere humans have ever piped
    // anything, and the code here is the produce's own hue.
    knob_coat: Coat::enamel(palette::kind_color(Kind::GasCanister)),
    knob_at: Vec3::new(0.0, -0.08, 0.15),
    knob_half: Vec3::new(0.48, 0.46, 0.066),
    // Half a hand of travel: a valve is worked, not pressed.
    throw: 0.06,
    lamp: palette::kind_color(Kind::GasCanister),
    trim: &VALVE_WORKS,
};

/// The valve's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const VALVE_WORKS: [Fitting; 8] = [
    // The stem the wheel turns on, and the boss it comes out of.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.0, -0.08, 0.100),
        Vec3::new(0.07, 0.07, 0.050),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, -0.08, 0.025),
        Vec3::new(0.30, 0.30, 0.025),
    ),
    // The dial: a box, and a lit face that is telling somebody something.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.62, 0.52, 0.05),
        Vec3::new(0.18, 0.18, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::AMBER, 2.2),
        Vec3::new(-0.62, 0.52, 0.08),
        Vec3::new(0.12, 0.12, 0.01),
    ),
    // The bleed line up the flank, and its nozzle.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.66, -0.10, 0.05),
        Vec3::new(0.05, 0.55, 0.05),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Socket),
        Vec3::new(0.66, 0.50, 0.06),
        Vec3::new(0.09, 0.09, 0.06),
    ),
    // Two bolts, because the plant vibrates and always has.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.72, -0.62, 0.04),
        Vec3::new(0.06, 0.06, 0.03),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.72, -0.62, 0.04),
        Vec3::new(0.06, 0.06, 0.03),
    ),
];

/// **The flare lamp.** Not a pendant: a scorched hood with a flue into
/// the ceiling and a mantle burning under the glass, which is what a
/// works hangs over a floor when it has more gas than electricity. Amber
/// at four fifths of the budget — bright enough to work by, and visibly
/// *burning* rather than switched on.
const FLARE_LAMP: Light = Light {
    color: palette::AMBER,
    burn: 0.85,
    shade: Shape::Cone,
    shade_coat: Coat::enamel(palette::SOOT),
    glass: Coat::phosphor(palette::EMBER, 2.0),
    cage: &LAMP_FLUE,
};

/// The flue and the mantle, measured off a box one shade across on every
/// side of the lamp — never off the room, so a hood cannot become a beam.
const LAMP_FLUE: [Fitting; 3] = [
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::SOOT),
        Vec3::new(0.0, 0.86, 0.0),
        Vec3::new(0.34, 0.8, 0.34),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.75, 0.0),
        Vec3::new(0.42, 0.06, 0.42),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 2.6),
        Vec3::new(0.0, -0.35, 0.0),
        Vec3::new(0.24, 0.24, 0.24),
    ),
];

/// **The dispatch floor**, inside: the bottle rack, the manifold that
/// fills it, the pilot showing over the aft cornice, the furnace port in
/// the port wall, and a hose left coiled on the deck by somebody who is
/// coming back for it.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents, which is why none
/// of this had to know how big a trade room is.
const DISPATCH_FLOOR: [Fitting; 20] = [
    // The line across the counter: a pipe standing a hand off the aft
    // wall in front of the goods, with two stub valves on it. The
    // handshake is one more of these, which is the argument the whole
    // station makes — you are not at a shop, you are at a tap, and the
    // tap you are allowed to turn is the one with the lamp under it.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.66, -0.10, 0.86),
        Vec3::new(0.28, 0.05, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.16, -0.10, 0.86),
        Vec3::new(0.14, 0.05, 0.05),
    ),
    stem(-0.16),
    stem(0.78),
    wheel(-0.16),
    wheel(0.78),
    // Three bottles racked against the starboard wall, in the produce's
    // own colour code. This is the stock room of a works: what it sells
    // is standing in the room, whether or not it is for sale today.
    bottle(0.10),
    bottle(0.45),
    bottle(0.80),
    // The strap across them, because a loose bottle is a torpedo.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.90, -0.10, 0.45),
        Vec3::new(0.03, 0.03, 0.50),
    ),
    // The manifold, high along the starboard cornice, with its collars.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.86, 0.60, 0.0),
        Vec3::new(0.06, 0.06, 0.92),
    ),
    collar(-0.55),
    collar(0.55),
    // The riser off it, dropping to the aft wall where the meter is.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.86, 0.16, 0.90),
        Vec3::new(0.05, 0.50, 0.05),
    ),
    // The pilot, over the aft cornice: the stack outside is lit, and this
    // is that same fire seen from indoors through a slit in the cornice.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EMBER, 1.8),
        Vec3::new(0.28, 0.92, 0.93),
        Vec3::new(0.62, 0.028, 0.035),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.28, 0.968, 0.93),
        Vec3::new(0.66, 0.028, 0.06),
    ),
    // The furnace port in the port wall: a hooded slit with the cracking
    // plant on the other side of it. Nothing in this room is a wall with
    // nothing behind it.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.94, -0.15, -0.35),
        Vec3::new(0.05, 0.35, 0.42),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EMBER, 1.2),
        Vec3::new(-0.90, -0.15, -0.35),
        Vec3::new(0.02, 0.24, 0.30),
    ),
    // A drip pan under the manifold, and a hose coiled on the deck. Both
    // are the same argument: this is a floor people work on, and people
    // who work on floors leave things on them.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.86, -0.95, 0.0),
        Vec3::new(0.10, 0.04, 0.55),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.55, -0.90, 0.10),
        Vec3::new(0.15, 0.10, 0.18),
    ),
];

/// One valve stem standing on the counter pipe, at `x` across the wall.
const fn stem(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Plate),
        Vec3::new(x, 0.02, 0.86),
        Vec3::new(0.022, 0.12, 0.026),
    )
}

/// One valve handwheel on a stem, at `x`. It lies flat on top of the
/// stem, so it reads as a wheel from the doorway — which is where a
/// crew stands when it is deciding whether to come in.
const fn wheel(x: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::kind_color(Kind::GasCanister)),
        Vec3::new(x, 0.15, 0.86),
        Vec3::new(0.085, 0.09, 0.102),
    )
}

/// One racked bottle, at `z` along the starboard wall.
const fn bottle(z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::kind_color(Kind::GasCanister)),
        Vec3::new(0.88, -0.42, z),
        Vec3::new(0.055, 0.56, 0.066),
    )
}

/// One manifold collar, at `z` along the pipe run.
const fn collar(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.86, 0.60, z),
        Vec3::new(0.085, 0.085, 0.05),
    )
}

/// **The works**, outside: the flare stack on the crown, the cracking
/// louvres in the outboard face, the riser and its aerostat hanging off
/// the outboard quarter, and two more bottles strapped to the flank
/// because there is nowhere else to put them.
///
/// Out here there is **no light to speak of** — the void carries one
/// slack star source and the art direction runs no shadow maps, so a
/// plate's own colour is very nearly black and only what glows is seen.
/// Every reading below is therefore either etched (the "findable with the
/// lamps sold" floor) or a phosphor, and the louvres burn at **0.8**
/// rather than at indicator brightness: a furnace wants to look like heat
/// behind a grate, not like a sign.
const THE_WORKS: [Fitting; 19] = [
    // ---- the flare stack ----
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.45, 1.06, -0.20),
        Vec3::new(0.16, 0.06, 0.16),
    ),
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.45, 1.20, -0.20),
        Vec3::new(0.085, 0.19, 0.10),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_JUPITER),
        Vec3::new(-0.45, 1.18, -0.20),
        Vec3::new(0.11, 0.03, 0.13),
    ),
    Fitting::new(
        Shape::Cone,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.45, 1.45, -0.20),
        Vec3::new(0.13, 0.08, 0.15),
    ),
    // The burn-off itself. It is the tell: a works that had nothing to
    // waste would be a works with nothing to sell.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 3.2),
        Vec3::new(-0.45, 1.66, -0.20),
        Vec3::new(0.12, 0.20, 0.14),
    ),
    // ---- the cracking louvres, in the outboard face ----
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.10, -0.12, -1.10),
        Vec3::new(0.48, 0.34, 0.06),
    ),
    louvre(-0.30),
    louvre(-0.12),
    louvre(0.06),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_JUPITER),
        Vec3::new(0.10, 0.26, -1.13),
        Vec3::new(0.54, 0.05, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_JUPITER),
        Vec3::new(0.10, -0.50, -1.13),
        Vec3::new(0.54, 0.05, 0.05),
    ),
    // ---- the riser, and what holds it up ----
    // The head sheave at the shell's lower outboard corner, two lengths
    // of riser, and then the aerostat: a gasbag with a scoop under it,
    // floating in the cloud deck and tied to nothing below. The ribbon
    // hangs from the balloon, not from the ground; there is no ground.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.62, -0.92, -1.10),
        Vec3::new(0.14, 0.10, 0.10),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.62, -1.16, -1.16),
        Vec3::new(0.05, 0.24, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.62, -1.52, -1.30),
        Vec3::new(0.045, 0.20, 0.045),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::etched(palette::POI_JUPITER),
        Vec3::new(0.62, -1.88, -1.42),
        Vec3::new(0.40, 0.30, 0.40),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.62, -1.62, -1.42),
        Vec3::new(0.14, 0.04, 0.14),
    ),
    Fitting::new(
        Shape::Cone,
        Coat::etched(palette::RIVET),
        Vec3::new(0.62, -2.18, -1.42),
        Vec3::new(0.13, 0.09, 0.13),
    ),
    // Its beacon, so the lighter can find the station in weather.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::AMBER, 3.0),
        Vec3::new(0.30, -1.94, -1.42),
        Vec3::new(0.06, 0.06, 0.06),
    ),
    // ---- and two more bottles, on the flank, strapped down ----
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::POI_JUPITER),
        Vec3::new(1.10, 0.10, -0.45),
        Vec3::new(0.09, 0.52, 0.107),
    ),
];

/// One lit louvre bar across the cracking grate, at height `y`.
const fn louvre(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EMBER, 0.8),
        Vec3::new(0.10, y, -1.17),
        Vec3::new(0.42, 0.045, 0.02),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jupiter's own reading, held where the economy put it: the room is
    /// a works (bottles and pipe, not shelves), the deal is a valve
    /// rather than a press or a plunger, and the same fire shows on both
    /// sides of the aft wall. Repaint it freely — a change that quietly
    /// retires one of these retires Jupiter.
    #[test]
    fn jupiter_is_a_works_that_burns_what_it_cannot_sell() {
        assert_eq!(CHARACTER.handshake.knob, Shape::Ring, "you crack a valve");
        const { assert!(CHARACTER.handshake.throw > super::super::NEUTRAL.handshake.throw) }
        assert_eq!(CHARACTER.tiles.stock.color, palette::SOOT);
        assert_eq!(CHARACTER.outfit.lamp, palette::EMBER);
        assert!(
            CHARACTER.decor.len() >= 10,
            "the dispatch floor is furniture, not a recolour"
        );
        // The flare is the tell, and it is lit on both sides of the wall:
        // a pilot over the cornice inside, a stack and its louvres out.
        let inside = CHARACTER
            .decor
            .iter()
            .filter(|fitting| fitting.coat.color == palette::EMBER)
            .count();
        let outside = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.coat.color == palette::EMBER)
            .count();
        assert!(inside >= 2 && outside >= 3, "the works stopped burning");
        // And the elevator ends in a balloon, a long way under the shell,
        // because a gas giant has nothing to bolt the bottom to.
        let hangs = CHARACTER.dress.iter().any(|fitting| {
            fitting.shape == Shape::Dome && fitting.at.y < -1.5 && fitting.half.x > 0.3
        });
        assert!(hangs, "the aerostat has floated off");
    }
}
