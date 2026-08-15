//! **The Guild Station** — the worked exemplar, and the room the player
//! sees most: every run starts docked here.
//!
//! What the lore actually says, and what each line turned into:
//!
//! - The Spacing Guild is *headquartered in an unknown location in the
//!   Outer Ring, operates in shady legal areas, and handles shipping
//!   contracts* (DESIGN.md). So the room is not a market. It is a
//!   **bonded warehouse counter**: goods stand on the Guild's own paint,
//!   behind the Guild's own bars, and what you are doing is filing.
//! - *The spacing guild has a massive, inexplicable hangar. As players
//!   deliver Suspicious Cargo, the guild will **immediately steal** the
//!   cargo in front of the usual bartering, and it gets shuttled off into
//!   the hangar* (DESIGN.md). The sim already does exactly that
//!   (`Sim::steal_crate`, run before the visit's room is even built), so
//!   the room shows the machinery that does it: a **seizure chute** in
//!   the starboard wall with a violet throat, and the hangar's own light
//!   leaking over the aft cornice from whatever is on the other side.
//! - *The Guild seizes rather than pays* — its suspicious columns are the
//!   only zeros in the barter table (`sim::barter::VALUE`) — and *counts
//!   the bigger crate four times*, explaining nothing. So the tell over
//!   the doorway is a **beacon**, not a lamp: a light that comes on when
//!   the hangar has taken something.
//! - The violet register: `EERIE` is canon for *crate glow, omen cast,
//!   **hangar lamps*** (`palette`). The Guild's hue is that violet, held
//!   one step off the omen's saturation so the two never argue — the
//!   room's paint is [`palette::GUILD_BOND`], its hall light
//!   [`palette::GUILD_HALL`], and only the two things that are literally
//!   the hangar (the chute's throat, the seizure beacon) burn `EERIE`
//!   itself.
//! - It orbits *the wrong way round, and nobody asks why*
//!   (`sim::map::POIS`). The mast outside carries that: a spar with cross
//!   yards and a violet masthead, aimed at nothing.
//!
//! The handshake is the one docs/ROOMS.md named by hand: **a chit press
//! at the Guild**. It is a brass stamp dome on a dark machine plate, with
//! a stamping arm, a lit register slot beneath it, and two bolts — you
//! do not shake hands with the Guild, you get stamped.
//!
//! Nothing here moves the room. Every fitting is a fraction of a box the
//! lattice placed, the tiles are repaints of classes the sim declared, and
//! the light is a fraction of a budget it does not set. That is the whole
//! contract, and this file is what filling it in looks like.

use bevy::prelude::Vec3;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The Guild's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: CHIT_PRESS,
    light: HALL,
    decor: &BONDED_STORE,
    outfit: Outfit {
        // The Guild paints its stations in its own register, and burns
        // violet running lights — which is how you know, from a long way
        // off and with nothing written anywhere, whose hangar that is.
        plate: palette::POI_GUILD,
        lamp: palette::EERIE,
        lamps: 2,
    },
    dress: &HANGAR_FACE,
};

/// The bonded floor.
///
/// `Stock` keeps its filled field and `Offer` its struck line — the
/// filled-against-hollow reading is not a station's to spend — but the
/// paint under the goods is the Guild's deep bonded violet instead of the
/// neutral grey, the band where that paint ends is **brass** rather than
/// shadow, and the line struck round a proposal is the Guild's own violet
/// rather than chalk. The threshold follows: brass studs, because the
/// Guild maintains its gangways, and a faintly lit violet sill, because
/// crossing it puts you in bonded space.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::GUILD_BOND),
    rim: Coat::metal(Worn::Brass),
    chalk: Coat::etched(palette::EERIE_BRIGHT),
    stud: Coat::metal(Worn::Brass),
    sill: Coat::etched(palette::EERIE),
};

/// **The chit press.** A brass stamp dome you slam, on a dark machine
/// plate, with the arm that drives it and the slot the chit comes out of.
/// It throws further than the neutral plunger and takes longer about it,
/// because a stamp is a decision somebody made about you.
const CHIT_PRESS: Handshake = Handshake {
    plate: Coat::metal(Worn::PlateShade),
    knob: Shape::Dome,
    knob_coat: Coat::metal(Worn::Brass),
    knob_at: Vec3::new(0.0, 0.16, 0.11),
    knob_half: Vec3::new(0.26, 0.26, 0.055),
    throw: 0.05,
    // The register lamp, in the Guild's violet rather than the ordinary
    // amber: what lights here is a filing, not an invitation.
    lamp: palette::EERIE,
    trim: &PRESS_WORKS,
};

/// The press's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const PRESS_WORKS: [Fitting; 6] = [
    // The die plate the dome comes down on.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.16, 0.045),
        Vec3::new(0.40, 0.40, 0.015),
    ),
    // The stamping arm, reaching off to port the way a lever does.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.32, 0.36, 0.09),
        Vec3::new(0.30, 0.05, 0.03),
    ),
    // The register slot: where the chit goes in, lit from inside.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EERIE, 2.2),
        Vec3::new(0.0, -0.30, 0.055),
        Vec3::new(0.30, 0.045, 0.010),
    ),
    // Its lip, so the slot reads as a mouth rather than a decal.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, -0.42, 0.070),
        Vec3::new(0.34, 0.030, 0.020),
    ),
    // Two bolts. A machine bolted to a wall by somebody who expected it
    // to still be there in thirty years.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.60, 0.55, 0.05),
        Vec3::new(0.06, 0.06, 0.02),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.60, 0.55, 0.05),
        Vec3::new(0.06, 0.06, 0.02),
    ),
];

/// **The hall light.** A caged pan fixture, not a shopkeeper's pendant:
/// the Guild lights its floor the way a customs house lights a floor,
/// evenly and without warmth, in a violet-white that is unmistakably its
/// own register and deliberately a long way off the omen's saturation.
/// It burns the full caller budget — the Guild is many things, but it is
/// not going to make you squint at a manifest.
const HALL: Light = Light {
    color: palette::GUILD_HALL,
    burn: 1.0,
    shade: Shape::Slab,
    shade_coat: Coat::metal(Worn::PlateShade),
    glass: Coat::phosphor(palette::GUILD_HALL, 1.6),
    cage: &LAMP_CAGE,
};

/// The guard round the hall light, measured off a box one shade across on
/// every side of it. Four brass corner posts and a hoop: nothing in a
/// bonded store is left where a crate can swing into it.
const LAMP_CAGE: [Fitting; 5] = [
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.80, -0.35, -0.80),
        Vec3::new(0.09, 0.55, 0.09),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.80, -0.35, -0.80),
        Vec3::new(0.09, 0.55, 0.09),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.80, -0.35, 0.80),
        Vec3::new(0.09, 0.55, 0.09),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.80, -0.35, 0.80),
        Vec3::new(0.09, 0.55, 0.09),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, -0.85, 0.0),
        Vec3::new(0.90, 0.08, 0.90),
    ),
];

/// **The bonded store**, inside the room: the bars over the goods, the
/// chute that takes what the hangar wants, the hangar's own light coming
/// over the cornice, the beacon above the door, and the Guild's plaque on
/// the flank.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number below is a fraction of its half-extents, which is why
/// none of this had to know how big a trade room is.
const BONDED_STORE: [Fitting; 14] = [
    // The register bars: brass uprights standing a cell clear of the
    // shopfront, in front of the goods. You can see the Guild's stock;
    // you cannot reach it until the press says so. They stand off the
    // `Stock` band entirely, because the goods' own air is the goods',
    // and clear of the doorway and of the press's own column, because a
    // bar across either would be a station taking something the sim owns.
    bar(-0.28),
    bar(-0.05),
    bar(0.45),
    bar(0.70),
    bar(0.92),
    // The rail they hang from, above the press's cell.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.32, 0.40, 0.690),
        Vec3::new(0.62, 0.022, 0.014),
    ),
    // The hangar's light, over the aft cornice. Whatever is on the other
    // side of that wall is lit, and it is not lit like this room.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EERIE, 1.6),
        Vec3::new(0.0, 0.94, 0.660),
        Vec3::new(0.92, 0.030, 0.040),
    ),
    // The seizure chute, in the starboard wall: a drum, a brass collar,
    // and a violet throat. Suspicious cargo goes in here and comes out
    // somewhere nobody has been.
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(0.85, -0.30, 0.10),
        Vec3::new(0.13, 0.70, 0.156),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Brass),
        Vec3::new(0.85, 0.42, 0.10),
        Vec3::new(0.15, 0.05, 0.18),
    ),
    Fitting::new(
        Shape::Post,
        Coat::phosphor(palette::EERIE, 2.0),
        Vec3::new(0.85, 0.33, 0.10),
        Vec3::new(0.10, 0.015, 0.12),
    ),
    // The seizure beacon, over the doorway: the light that comes on when
    // the hangar has taken something, which at the Guild it usually has.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EERIE_BRIGHT, 3.0),
        Vec3::new(-0.67, 0.52, 0.650),
        Vec3::new(0.07, 0.07, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.67, 0.60, 0.640),
        Vec3::new(0.11, 0.015, 0.06),
    ),
    // And the plaque on the port flank: the Guild's own enamel, framed in
    // brass, saying nothing at all — this game has no text, and an
    // organization this size does not need any.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.98, 0.30, -0.20),
        Vec3::new(0.012, 0.23, 0.29),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_GUILD),
        Vec3::new(-0.96, 0.30, -0.20),
        Vec3::new(0.015, 0.155, 0.26),
    ),
];

/// One register bar, at `x` across the aft wall.
const fn bar(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Brass),
        Vec3::new(x, -0.34, 0.690),
        Vec3::new(0.014, 0.66, 0.014),
    )
}

/// **The hangar face**, outside: the maw on the outboard wall, the mast
/// over the top, and two bonded ribs down the flanks.
///
/// The aft face is deliberately bare — it is the face mated to your hull,
/// and the seam belongs to the doorway, not to the station.
/// Out here there is **no light at all** — the void carries none and the
/// art direction runs no shadow maps, so a plate's own colour is very
/// nearly black and only what glows is seen. That is why every reading
/// below is either radium brass (the game's "findable with the lamps
/// sold" floor, `Finish::Etched`) or a phosphor, and why the maw burns at
/// **0.7** rather than at indicator brightness: a mouth wants to look
/// like a lit depth you could fly into, and a big emissive at 1.8 was a
/// lightbox bolted to a hull.
const HANGAR_FACE: [Fitting; 13] = [
    // The maw: a lit rectangle in the outboard wall, framed in brass, and
    // crossed by the rails its shutter runs on — which is what keeps it
    // reading as a door with light behind it rather than as a screen.
    // Deliveries go in there. Nothing comes out of it.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EERIE, 0.55),
        Vec3::new(0.0, -0.14, -1.16),
        Vec3::new(0.42, 0.30, 0.04),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, -0.02, -1.20),
        Vec3::new(0.42, 0.018, 0.02),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, -0.28, -1.20),
        Vec3::new(0.42, 0.018, 0.02),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, 0.20, -1.14),
        Vec3::new(0.52, 0.05, 0.06),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.0, -0.48, -1.14),
        Vec3::new(0.52, 0.05, 0.06),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(-0.47, -0.14, -1.14),
        Vec3::new(0.05, 0.39, 0.06),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.47, -0.14, -1.14),
        Vec3::new(0.05, 0.39, 0.06),
    ),
    // The mast: a spar, two cross yards, and a violet masthead, standing
    // over the middle of the shell so it reads from either quarter. It is
    // aimed at nothing, which suits a station that orbits the wrong way
    // round and has never explained that either.
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::BRASS),
        Vec3::new(0.30, 1.38, -0.10),
        Vec3::new(0.040, 0.34, 0.040),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.30, 1.44, -0.10),
        Vec3::new(0.28, 0.022, 0.022),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(0.30, 1.58, -0.10),
        Vec3::new(0.18, 0.022, 0.022),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EERIE_BRIGHT, 4.0),
        Vec3::new(0.30, 1.75, -0.10),
        Vec3::new(0.09, 0.09, 0.09),
    ),
    // Two bonded ribs, one down each flank: the banding a sealed
    // container wears, at station scale, in the radium brass that keeps
    // a hull's shape readable with nothing lighting it.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(-1.10, 0.30, 0.0),
        Vec3::new(0.04, 0.05, 0.80),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(1.10, 0.30, 0.0),
        Vec3::new(0.04, 0.05, 0.80),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The Guild's own reading, held where the lore put it: the room is
    /// bonded (its paint and its bars), it seizes (the chute and the
    /// beacon burn the hangar's own violet), and it stamps rather than
    /// shakes. Repaint it freely — but a change that quietly retires one
    /// of these is a change that retires the Guild.
    #[test]
    fn the_guild_is_a_bonded_store_that_seizes() {
        assert_eq!(CHARACTER.tiles.stock.color, palette::GUILD_BOND);
        assert_eq!(CHARACTER.handshake.knob, Shape::Dome, "the Guild stamps");
        assert_eq!(CHARACTER.handshake.lamp, palette::EERIE);
        assert!(
            CHARACTER.decor.len() >= 10,
            "the bonded store is furniture, not a recolour"
        );
        // Every violet in the room is the hangar's or the Guild's, and
        // the two are told apart on purpose: `EERIE` belongs to the
        // things that ARE the hangar, and nothing else may spend it.
        let hangar = CHARACTER
            .decor
            .iter()
            .chain(CHARACTER.dress.iter())
            .filter(|fitting| fitting.coat.color == palette::EERIE)
            .count();
        assert!(hangar >= 3, "the hangar has stopped showing through");
        assert_ne!(CHARACTER.outfit.plate, super::super::NEUTRAL.outfit.plate);
        assert_eq!(CHARACTER.outfit.lamp, palette::EERIE);
    }
}
