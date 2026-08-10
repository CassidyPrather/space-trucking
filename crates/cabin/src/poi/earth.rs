//! **Earth** — a dystopia of some sort; pick a creative one (DESIGN.md).
//! The ledger picked one before anybody wrote it down, and this file only
//! reads it back.
//!
//! - Earth's produce is **ration bricks**: the one kind its barter row
//!   prices at zero (`sim::barter::VALUE`), and a zero is where a kind
//!   enters the world. So the goods come out of a hopper, down a chute,
//!   onto a tray, pressed. Nobody grew them.
//! - Earth pays **four for seedlings**, and a station that pays for
//!   seedlings is a station that cannot grow them. The grow rack on the
//!   port flank is therefore real, lit, powered — and empty. That is the
//!   dystopia, and it is one shelf.
//! - Its dearest column is **bottled midnight**, at five: on a world
//!   whose floodlights never go off, the luxury import is dark. Which is
//!   the other half of why the lamp in here is **metered**: light is
//!   issued, on an allowance, by the box on the pendant's own stem. The
//!   room burns half the caller budget and the brightest thing in it is
//!   the dial that counts the burning.
//! - The inner ring's factions refuse each other's traffic
//!   (`sim::map::INNER_RING`), and Earth is the one that made the paper
//!   work: the floor lines are painted rather than lit, the studs are
//!   worn down to the socket, and the fixture you deal at is a meter
//!   with a token slot.
//!
//! The handshake is a **meter**: a galvanised grip bar on a dark state
//! plate, a ratchet track beside it, a caged readout above, and a slot.
//! It throws barely at all. You are not shaking hands with Earth; you
//! are being processed by it, and it does not gild the fact.
//!
//! Outside, the owner's space elevator in its plainest and most working
//! form — a **twin freight ribbon**, tape-thin, running clean past the
//! roof and away under the keel, with a climber on it and its lower
//! length already the colour of the smog it disappears into
//! (`palette::accent::SMOG`) — which is the only way, from out there, to
//! tell which end of the ribbon the world is on. No gold anywhere.

use bevy::prelude::{Color, Vec3};
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Earth's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: METER,
    light: ALLOWANCE,
    decor: &RATION_COUNTER,
    outfit: Outfit {
        // Slate plate and one running light. Two would have been the
        // ordinary allowance; Earth issues itself one.
        plate: palette::POI_EARTH,
        lamp: palette::POI_EARTH,
        lamps: 1,
    },
    dress: &THE_RIBBON,
};

/// The issued floor.
///
/// `Stock` keeps its filled field and `Offer` its struck line — that
/// reading is not a station's to spend — but everything here is a
/// municipal surface: slate paint on the goods band, a galvanised rim
/// where it stops, and the line round a proposal **painted rather than
/// lit**, in the ochre a factory floor is marked in. Earth spends light
/// on the meter, not on the floor. The studs are worn through to the
/// socket by the queue that has stood on them.
///
/// Both painted surfaces are **blends toward the smog**, and that is the
/// whole trick of the room: the wall is the chart's Earth blue with the
/// sky's own stain in it, and the floor line is the ochre a factory
/// marks a floor in after forty years of being walked on. Raw, the two
/// roles read as a clean municipal building, which is a different and
/// much less frightening dystopia.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(STAINED_SLATE),
    rim: Coat::metal(Worn::Rivet),
    chalk: Coat::enamel(WALKED_OCHRE),
    stud: Coat::metal(Worn::Socket),
    sill: Coat::metal(Worn::Rivet),
};

/// The municipal blue, with the sky it is under mixed into it.
const STAINED_SLATE: Color = palette::blend(palette::POI_EARTH, palette::accent::SMOG, 0.28);

/// The floor line: amber that nobody has repainted since it was issued.
const WALKED_OCHRE: Color = palette::blend(palette::AMBER, palette::accent::SMOG, 0.42);

/// **The meter.** A galvanised grip bar on a dark state plate, with the
/// ratchet track it travels in, a readout behind a guard, and a token
/// slot. Its throw is the shortest of any station's: the machine gives
/// you a centimetre and takes its time about it.
const METER: Handshake = Handshake {
    plate: Coat::metal(Worn::Socket),
    knob: Shape::Post,
    knob_coat: Coat::metal(Worn::Rivet),
    knob_at: Vec3::new(0.0, -0.10, 0.10),
    knob_half: Vec3::new(0.16, 0.40, 0.050),
    throw: 0.030,
    // The one indicator, in a cold administrative grey-green rather than
    // an invitation's amber.
    lamp: palette::ICON_LIT,
    trim: &METER_WORKS,
};

/// The meter's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const METER_WORKS: [Fitting; 11] = [
    // The readout, and the hood over it. This is the brightest thing in
    // the room, which is the joke and also the policy.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::AMBER, 2.2),
        Vec3::new(0.0, 0.56, 0.055),
        Vec3::new(0.30, 0.17, 0.018),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, 0.80, 0.100),
        Vec3::new(0.40, 0.035, 0.075),
    ),
    // Three bars across the readout: even the number is behind a guard.
    guard(-0.19),
    guard(0.0),
    guard(0.19),
    // The token slot.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.50, 0.10, 0.045),
        Vec3::new(0.055, 0.15, 0.020),
    ),
    // The track the bar travels in.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.36, -0.10, 0.055),
        Vec3::new(0.045, 0.44, 0.030),
    ),
    // Four bolts, on a box that was fitted by a department.
    bolt(-0.66, 0.66),
    bolt(0.66, 0.66),
    bolt(-0.66, -0.62),
    bolt(0.66, -0.62),
];

/// One bar of the readout's guard, at `x` across the cell.
const fn guard(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(x, 0.56, 0.075),
        Vec3::new(0.014, 0.19, 0.012),
    )
}

/// One bolt on the meter plate.
const fn bolt(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(x, y, 0.040),
        Vec3::new(0.05, 0.05, 0.020),
    )
}

/// **The allowance.** A sealed utility puck under a wire guard, burning
/// **half** the caller budget in a cold grey-green — the dimmest room on
/// the inner ring, and legally lit. What hangs above it on the stem is
/// the meter box, with the only warm light in the place on its face:
/// Earth will not spend a lumen it has not counted.
///
/// The pendant's reach is still the room's own (`room::caller_reach`),
/// because that is not a station's to pick. All Earth gets to do is
/// issue less of it.
const ALLOWANCE: Light = Light {
    color: palette::ICON_LIT,
    burn: 0.5,
    shade: Shape::Post,
    shade_coat: Coat::metal(Worn::Plate),
    glass: Coat::phosphor(palette::ICON_LIT, 1.0),
    cage: &SUPPLY,
};

/// The guard and the meter above it, measured off a box one shade across
/// on every side of the lamp — never off the room.
const SUPPLY: [Fitting; 6] = [
    // The hoop, and three bars under the glass.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, -0.30, 0.0),
        Vec3::new(1.05, 0.35, 1.05),
    ),
    bar(-0.55),
    bar(0.0),
    bar(0.55),
    // The meter box on the stem, and its dial.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, 1.55, 0.0),
        Vec3::new(0.38, 0.45, 0.38),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::AMBER, 2.6),
        Vec3::new(0.0, 1.55, 0.42),
        Vec3::new(0.24, 0.24, 0.030),
    ),
];

/// One bar of the lamp guard, at `z` across the fitting.
const fn bar(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.0, -0.62, z),
        Vec3::new(1.05, 0.05, 0.05),
    )
}

/// **The ration counter**, inside the room: the hopper the produce comes
/// out of, the grow rack that grows nothing, and the duct that keeps the
/// air moving because nothing else will.
///
/// The frame is the room's box — `+x` starboard, `+y` up, `+z` aft — and
/// every number is a fraction of its half-extents, so none of this had
/// to know how big a trade room is.
const RATION_COUNTER: [Fitting; 14] = [
    // The hopper, starboard aft: bin, chute, tray, and two bricks that
    // came out of it while you were looking at something else.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.70, 0.42, 0.66),
        Vec3::new(0.26, 0.28, 0.22),
    ),
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(0.70, -0.05, 0.66),
        Vec3::new(0.085, 0.22, 0.085),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.70, -0.34, 0.66),
        Vec3::new(0.24, 0.025, 0.19),
    ),
    brick(0.60, -0.280, 0.60),
    brick(0.79, -0.285, 0.71),
    // The grow rack, port flank: two uprights, two shelves, a strip
    // still burning over the top one, and one empty tray. Earth pays
    // four for seedlings and this is why.
    upright(-0.66),
    upright(-0.06),
    shelf(0.06),
    shelf(-0.44),
    Fitting::new(
        Shape::Slab,
        // The dimmest green the palette carries: a grow light at the end
        // of its life over a shelf that has not needed one in years.
        Coat::phosphor(palette::PHOSPHOR_DIM, 2.4),
        Vec3::new(-0.82, 0.015, -0.36),
        Vec3::new(0.115, 0.014, 0.30),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.82, -0.40, -0.30),
        Vec3::new(0.12, 0.020, 0.15),
    ),
    // The duct along the ceiling, and the two clamps holding it there.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.10, 0.90, -0.20),
        Vec3::new(0.09, 0.055, 0.72),
    ),
    clamp(-0.70),
    clamp(0.24),
];

/// One ration brick in the tray.
const fn brick(x: f32, y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::RationBricks)),
        Vec3::new(x, y, z),
        Vec3::new(0.070, 0.038, 0.055),
    )
}

/// One upright of the grow rack, at `z` along the port flank.
const fn upright(z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.82, -0.42, z),
        Vec3::new(0.022, 0.56, 0.022),
    )
}

/// One shelf of the grow rack, at height `y`.
const fn shelf(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.82, y, -0.36),
        Vec3::new(0.145, 0.020, 0.34),
    )
}

/// One duct clamp, at `z` along the ceiling.
const fn clamp(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.10, 0.90, z),
        Vec3::new(0.12, 0.075, 0.035),
    )
}

/// **The ribbon**, outside: the owner's space elevator with the romance
/// taken out of it.
///
/// Two tapes, not a cable — flat, thin, and paired, the way a freight
/// ribbon is drawn by people who have to lift things with it. It runs
/// **past** the station rather than up from it, off the top of any frame
/// you can photograph it in and off the bottom of the same one: clean
/// above, and below the keel already the colour of what it goes down
/// into. Two cars are on it, one lit and climbing, one nearly gone.
///
/// The smog was three sheets of haze under the keel for one build, and
/// they read as a *floor* — a station standing on a stage. The world is
/// carried by the tape's own colour now, which is cheaper and truer: you
/// can tell which way Earth is because the ribbon changes colour on the
/// way there.
///
/// Out here there is **no light at all**: the void carries none and the
/// art direction runs no shadow maps, so a plate's own colour is nearly
/// black and only what glows is seen. So every reading below is either
/// `Finish::Etched` (the findable-with-the-lamps-sold floor) or a
/// phosphor, and there is not one gram of brass on any of it.
const THE_RIBBON: [Fitting; 20] = [
    // The upper tapes, above the smog and the colour of clean metal.
    tape(-0.24, 1.90, 0.88, Coat::etched(palette::GLINT)),
    tape(0.24, 1.90, 0.88, Coat::etched(palette::GLINT)),
    // The lower tapes, already the colour of what they go down into.
    tape(-0.24, -1.95, 0.93, Coat::etched(palette::accent::SMOG)),
    tape(0.24, -1.95, 0.93, Coat::etched(palette::accent::SMOG)),
    // The climber: a car spanning both tapes, with its lights on.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(0.0, 1.42, -0.10),
        Vec3::new(0.32, 0.14, 0.10),
    ),
    car_lamp(-0.32),
    car_lamp(0.32),
    // Guide rollers where the tapes cross the shell, top and bottom.
    roller(-0.24, 1.12),
    roller(0.24, 1.12),
    roller(-0.24, -1.12),
    roller(0.24, -1.12),
    // And the down car, most of the way into the murk with one lamp
    // still showing. Freight goes down as well; nobody meets it.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::accent::SMOG),
        Vec3::new(0.0, -1.72, -0.10),
        Vec3::new(0.30, 0.13, 0.10),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::AMBER, 1.4),
        Vec3::new(0.0, -1.72, -0.20),
        Vec3::new(0.045, 0.045, 0.045),
    ),
    // A strobe on the roof line, and two floods on the outboard face:
    // a world that never turns its lights off does not start with the
    // docks.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::AMBER, 3.5),
        Vec3::new(-0.62, 1.10, -0.30),
        Vec3::new(0.060, 0.060, 0.060),
    ),
    flood(-0.62),
    flood(0.62),
    // A louvre across the face, because the plant that keeps a world
    // breathing is the only thing Earth is willing to put on the
    // outside of a building.
    louvre(-0.10),
    louvre(-0.28),
    louvre(-0.46),
    louvre(-0.64),
];

/// One louvre bar across the outboard face, at height `y`.
const fn louvre(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(-0.30, y, -1.08),
        Vec3::new(0.55, 0.030, 0.030),
    )
}

/// One length of ribbon: half-width fixed, `at`/`span` in y.
const fn tape(x: f32, y: f32, span: f32, coat: Coat) -> Fitting {
    Fitting::new(
        Shape::Slab,
        coat,
        Vec3::new(x, y, -0.10),
        Vec3::new(0.090, span, 0.016),
    )
}

/// One of the climber's running lights, at `x`.
const fn car_lamp(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::AMBER, 3.0),
        Vec3::new(x, 1.42, -0.20),
        Vec3::new(0.050, 0.050, 0.050),
    )
}

/// One guide roller where a tape crosses the shell.
const fn roller(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::ICON),
        Vec3::new(x, y, -0.10),
        Vec3::new(0.10, 0.090, 0.10),
    )
}

/// One flood on the outboard face, at `x`.
const fn flood(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::POI_EARTH, 3.0),
        Vec3::new(x, 0.55, -1.08),
        Vec3::new(0.070, 0.070, 0.050),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Earth's own reading, held where the ledger put it: light is
    /// issued, food is pressed, nothing grows, and the fixture you deal
    /// at counts before it consents. Repaint it freely — a change that
    /// quietly retires one of these is a change that retires the
    /// dystopia.
    #[test]
    fn earth_rations_the_light_and_presses_the_food() {
        // The dimmest lit room on the chart, and still lit: a station
        // that let you trade in the dark would be a station with
        // something to hide, and Earth's whole manner is that it has
        // nothing to hide and no intention of being pleasant about it.
        const {
            assert!(
                CHARACTER.light.burn > 0.0 && CHARACTER.light.burn <= 0.5,
                "Earth stopped rationing its light"
            );
        }
        assert_eq!(CHARACTER.outfit.lamps, 1, "Earth issues itself one lamp");
        assert_eq!(CHARACTER.handshake.knob, Shape::Post, "Earth meters");
        const {
            assert!(
                CHARACTER.handshake.throw < super::super::NEUTRAL.handshake.throw,
                "the machine gives you a centimetre"
            );
        }
        // Nothing in this room is gilt, and that is the point: the whole
        // inner ring is not one faction with three paint jobs.
        assert!(
            !CHARACTER
                .decor
                .iter()
                .chain(CHARACTER.handshake.trim.iter())
                .any(|fitting| fitting.coat.color == palette::BRASS),
            "Earth has found some brass from somewhere"
        );
        // And the grow rack is lit and empty — one shelf of dystopia,
        // which is the only place in the room a green burns.
        assert!(
            CHARACTER
                .decor
                .iter()
                .any(|fitting| fitting.coat.color == palette::PHOSPHOR_DIM),
            "the grow light went out and took the joke with it"
        );
    }
}
