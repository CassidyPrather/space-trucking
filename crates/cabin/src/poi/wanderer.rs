//! **`???`** — only there when three mysterious crates hum in a hold at
//! once. It trades three for one bigger one. The Guild counts the bigger
//! one four times. Nobody explains the arithmetic, least of all Cor
//! (DESIGN.md).
//!
//! It brings a trade room and **stocks nothing**: the room comes
//! alongside empty, and the only thing in it is whatever you carried in.
//!
//! What the lore says, and what each line turned into:
//!
//! - *Three for one, counted as four.* The room is built on that sum and
//!   on nothing else. Three pillars down the starboard side, each with a
//!   collar at its head — and, alone on the port side, **a fourth collar
//!   with no pillar under it**. Three tally bars at the toll, and a
//!   fourth beside them lit brighter than the rest. Three rings hanging
//!   over the bay where your three crates go. Count anything in this
//!   room and you get four, and the fourth is never quite an object.
//! - *It stocks nothing.* So the `Stock` band is painted the near-black
//!   violet of `VeryMysteriousCrate` — the room is the colour of the
//!   thing it hands you — and its rim is a faint line you can barely
//!   find, because a shelf with nothing on it does not need an edge. The
//!   `Offer` band, where your crates go, is the **brightest thing in the
//!   room**: this place cares about exactly one rectangle of floor.
//! - *It is parked in space, ignoring gravity* (`sim::map`'s
//!   `Track::Fixed`, which nothing else on the chart uses). So the
//!   exterior does not orbit, does not signal, and does not face
//!   anything: no running lights at all, a black hull, a mint orb where
//!   a hangar or a mast would be, and — out to starboard, at the reach a
//!   dressing is allowed — **the empty wireframe of a second room the
//!   same size as this one**, which is not there.
//!
//! # The omen's register, and why `???` gets a little of it
//!
//! `EERIE_BRIGHT` is described in `palette` as *"the jump flash, crate
//! hot core, **`???` toll**"*. The omen's violet is the game's one
//! *something is wrong* signal and no station's ordinary furniture may
//! wear it — but the palette names this station in that role's own
//! documentation, and what it names is the **toll**, not the room. So the
//! spend here is exactly two things, and both of them are the
//! arithmetic: the fixture's own lamp, and the fourth tally mark. The
//! pillars, the collars, the floor lines, the hull and the twin are all
//! [`palette::POI_WANDERER`], a mint that belongs to nobody else on the
//! chart and reads cold and manufactured beside every warm station in the
//! game. A visitor should feel the wrongness in the *shapes*; the violet
//! is reserved for the moment the sum happens.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles};
use crate::palette;

/// `???`'s own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: TOLL,
    light: NOT_A_LAMP,
    decor: &NOT_A_ROOM,
    outfit: Outfit {
        // Hull the colour of the crate it gives you, and **no running
        // lights at all**. Every station in this game burns something at
        // its corners because "a station that let you dock in the dark
        // would be a station with something to hide" (docs/ROOMS.md).
        // Quite.
        plate: palette::kind_color(Kind::VeryMysteriousCrate),
        lamp: palette::POI_WANDERER,
        lamps: 0,
    },
    dress: &THE_TWIN,
};

/// **The floor of a room that stocks nothing.**
///
/// `Stock` keeps its filled field and `Offer` its struck line, and the
/// *relationship* between them is the character: the shelf is painted in
/// the crate's own near-black and edged in a line so faint you have to
/// look for it, while the bay where your three crates go is struck in
/// mint bright enough to read from the doorway. Everything this station
/// is interested in happens inside that rectangle.
///
/// The threshold is black studs and a lit sill: nothing marks the way in,
/// and the line you cross is unmissable.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::kind_color(Kind::VeryMysteriousCrate)),
    rim: Coat::phosphor(palette::POI_WANDERER, 0.30),
    chalk: Coat::phosphor(palette::POI_WANDERER, 1.60),
    stud: Coat::enamel(palette::SHADOW),
    sill: Coat::phosphor(palette::POI_WANDERER, 1.00),
};

/// **The toll.** Not a press, not a bell, not a snuffer: a black body
/// standing in a lit opening, at the height a hand goes in, with a travel
/// deeper than the wall it is set in.
///
/// The knob is the **colour of the room and of the crate you leave
/// with**, so what you actually see at the fixture is the light around
/// it rather than the thing you work — a hole with something in it. It
/// throws twice what the Guild's press does. Nothing about that is
/// explained.
///
/// The lamp is [`palette::EERIE_BRIGHT`], which `palette` calls the
/// `???` toll by name. It is one of exactly two places this module spends
/// the omen's register; the other is the fourth tally.
const TOLL: Handshake = Handshake {
    plate: Coat::enamel(palette::kind_color(Kind::VeryMysteriousCrate)),
    knob: Shape::Dome,
    knob_coat: Coat::enamel(palette::kind_color(Kind::VeryMysteriousCrate)),
    knob_at: Vec3::new(0.0, 0.02, 0.13),
    knob_half: Vec3::new(0.24, 0.24, 0.09),
    throw: 0.10,
    lamp: palette::EERIE_BRIGHT,
    trim: &THE_ARITHMETIC,
};

/// The toll's own hardware, in its cell's frame: x and y are fractions of
/// the declared cell, z is metres out of the wall.
///
/// **This is the lore, as furniture.** Three tallies and a fourth; three
/// sockets and no fourth socket. Whoever keeps this count is keeping it
/// where you can see it and is not going to discuss it.
const THE_ARITHMETIC: [Fitting; 8] = [
    // The opening the body stands in: a lit square, cut clean, with no
    // frame, no bezel and no bolts. Every other fixture in this game is
    // a thing bolted to a wall; this one is a hole that was always there.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_WANDERER, 1.20),
        Vec3::new(0.0, 0.02, 0.030),
        Vec3::new(0.34, 0.34, 0.010),
    ),
    // Three tallies, in the room's own mint: the three crates you
    // brought.
    tally(-0.46, palette::POI_WANDERER, 1.20),
    tally(-0.22, palette::POI_WANDERER, 1.20),
    tally(0.02, palette::POI_WANDERER, 1.20),
    // And the fourth, which is not one of your crates and is lit like it
    // matters more than they do. The Guild counts this one; nobody has
    // ever said who told them to.
    tally(0.30, palette::EERIE_BRIGHT, 2.60),
    // Three sockets round the ring, at three of the four quarters. The
    // missing one is where the fourth thing goes, and there is nothing
    // there.
    socket(-0.62, 0.42),
    socket(0.62, 0.42),
    socket(-0.62, -0.24),
];

/// One tally mark beside the toll.
const fn tally(x: f32, color: bevy::prelude::Color, glow: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(color, glow),
        Vec3::new(x, -0.52, 0.045),
        Vec3::new(0.030, 0.11, 0.020),
    )
}

/// One socket in the toll's plate.
const fn socket(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::SHADOW),
        Vec3::new(x, y, 0.030),
        Vec3::new(0.070, 0.070, 0.030),
    )
}

/// **Not a lamp.**
///
/// The fitting where a pendant should be is a short mint column that
/// glows on its outside and is **black underneath**, which is exactly
/// backwards: at every other station in the registry the shade is the
/// dark part and the glass is the lit part. Nothing about the room is
/// obviously wrong until you look up, and then one thing is, and it is
/// hard to say what.
///
/// It burns half the caller budget in mint, which lights the room
/// perfectly adequately and makes nothing in it look well.
const NOT_A_LAMP: Light = Light {
    color: palette::POI_WANDERER,
    burn: 0.50,
    shade: Shape::Post,
    shade_coat: Coat::phosphor(palette::POI_WANDERER, 1.10),
    glass: Coat::enamel(palette::SHADOW),
    cage: &THREE_AND_ONE,
};

/// Three rings round the fitting at even spacing — and a fourth, off to
/// one side, hanging on nothing. Measured off a box one shade across on
/// every side of the lamp, so the odd one out is still the lamp's and not
/// a beam across the room.
const THREE_AND_ONE: [Fitting; 4] = [
    ring(0.0, 0.55, 0.0, 0.97),
    ring(0.0, 0.10, 0.0, 0.97),
    ring(0.0, -0.35, 0.0, 0.97),
    ring(0.45, -0.14, 0.0, 0.52),
];

/// One ring round the fitting.
const fn ring(x: f32, y: f32, z: f32, r: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(palette::POI_WANDERER, 0.70),
        Vec3::new(x, y, z),
        Vec3::new(r, 0.10, r),
    )
}

/// **Not a room**, inside: three pillars and a fourth collar, three rings
/// over the bay, a lit line where the deck meets the walls, and a lining
/// over the deckhead so the box has no corners in it worth calling
/// corners.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. Nothing stands in
/// the doorway, and nothing stands in the toll's own column.
///
/// **The deck line stops.** It runs the port wall, the front wall and the
/// starboard wall, and the port one stops well short of the doorway —
/// because a fitting may not stand in a threshold, which two rooms share
/// and a character owns neither of. The gap it leaves is the one honest
/// seam in a room that has no others, and it looks entirely deliberate,
/// which is the joke.
const NOT_A_ROOM: [Fitting; 15] = [
    // ---- three pillars, starboard ----
    // Evenly spaced, identical, floor to deckhead, each with a collar at
    // its head. They hold nothing up: the ceiling is the lattice's.
    pillar(-0.52),
    pillar(0.06),
    pillar(0.64),
    collar(0.86, -0.52),
    collar(0.86, 0.06),
    collar(0.86, 0.64),
    // ---- and the fourth ----
    // A collar at the same height, on the port side, with nothing under
    // it and nothing through it. Three crates went in and four
    // deliveries came out; this is the fourth, and it is the shape of an
    // absence with a rim round it.
    collar(-0.86, 0.06),
    // ---- the hum ----
    // Three rings stacked over the offer bay, at the pitch three crates
    // sound at when they are in a hold together. They hang on nothing
    // either.
    hum(0.42, 0.60),
    hum(0.58, 0.42),
    hum(0.74, 0.26),
    // ---- the deck line ----
    // A lit seam where the deck meets each wall. It is the room's whole
    // sense of scale, and the reason a black box reads as an interior at
    // all.
    deck(-0.965, -0.20, 0.025, 0.74),
    deck(0.965, 0.01, 0.025, 0.95),
    front_deck(0.0, -0.965),
    // ---- and the fourth line ----
    // The same seam again, out in the middle of the floor, where there is
    // no wall for it to be the foot of. Three of these say "this is where
    // the room ends"; the fourth says it somewhere the room does not.
    deck(-0.10, -0.10, 0.025, 0.62),
    // ---- the panel ----
    // A slab of the room's own lining standing free, a good half-metre
    // off the starboard wall, parallel to it, holding nothing and held by
    // nothing. It is the only thing in here you could walk into, and the
    // only surface in the game with no visible way it was made.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::VeryMysteriousCrate)),
        Vec3::new(0.42, -0.12, -0.10),
        Vec3::new(0.020, 0.60, 0.42),
    ),
];

/// One pillar down the starboard side, at `z`.
const fn pillar(z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::kind_color(Kind::VeryMysteriousCrate)),
        Vec3::new(0.86, 0.0, z),
        Vec3::new(0.075, 0.96, 0.075),
    )
}

/// One collar at a pillar's head — or where a pillar would be.
const fn collar(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(palette::POI_WANDERER, 0.90),
        Vec3::new(x, 0.86, z),
        Vec3::new(0.135, 0.035, 0.135),
    )
}

/// One ring of the hum, hanging over the offer bay.
const fn hum(y: f32, glow: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(palette::POI_WANDERER, glow),
        Vec3::new(-0.16, y, -0.62),
        Vec3::new(0.20, 0.045, 0.20),
    )
}

/// One lit seam along the deck, running fore-and-aft at `x`.
const fn deck(x: f32, z: f32, hx: f32, hz: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_WANDERER, 0.55),
        Vec3::new(x, -0.965, z),
        Vec3::new(hx, 0.020, hz),
    )
}

/// The seam across the front of the deck.
const fn front_deck(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_WANDERER, 0.55),
        Vec3::new(x, -0.965, z),
        Vec3::new(0.96, 0.020, 0.025),
    )
}

/// **The twin**, outside — and this is the part that should get at you
/// before you dock.
///
/// The shell itself shows the void almost nothing: a black hull, no
/// running lights, a mint orb standing off the outboard face where every
/// other station puts a maw or a mast, and three short bars beside it.
/// Then, out to starboard at the reach a dressing is allowed, there is
/// **the fourth thing**: twelve hairlines describing a box exactly the
/// size of the room you are about to walk into, with nothing inside it.
///
/// It is not a hangar, it is not a scaffold, it is not under
/// construction, and it does not do anything. It is the same room, empty,
/// parked alongside — and it is the only structure in this game that is
/// drawn entirely as edges, which is what makes a viewer count the boxes
/// and then stop.
const THE_TWIN: [Fitting; 20] = [
    // The orb: one mint body on an unlit hull, at the height of a face.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::POI_WANDERER, 1.30),
        Vec3::new(0.0, 0.12, -1.38),
        Vec3::new(0.30, 0.30, 0.30),
    ),
    // Three bars beside it, and no fourth bar on this side of the hull.
    bar(-0.62, 0.34),
    bar(-0.62, 0.12),
    bar(-0.62, -0.10),
    // The twin: twelve edges, one empty box, one room's distance out.
    edge(1.20, 0.90, 0.0, 0.014, 0.014, 0.90),
    edge(2.98, 0.90, 0.0, 0.014, 0.014, 0.90),
    edge(1.20, -0.90, 0.0, 0.014, 0.014, 0.90),
    edge(2.98, -0.90, 0.0, 0.014, 0.014, 0.90),
    edge(2.09, 0.90, 0.90, 0.89, 0.014, 0.014),
    edge(2.09, 0.90, -0.90, 0.89, 0.014, 0.014),
    edge(2.09, -0.90, 0.90, 0.89, 0.014, 0.014),
    edge(2.09, -0.90, -0.90, 0.89, 0.014, 0.014),
    edge(1.20, 0.0, 0.90, 0.014, 0.90, 0.014),
    edge(1.20, 0.0, -0.90, 0.014, 0.90, 0.014),
    edge(2.98, 0.0, 0.90, 0.014, 0.90, 0.014),
    edge(2.98, 0.0, -0.90, 0.014, 0.90, 0.014),
    // The twin's own toll, drawn on the face of a room that has no faces:
    // a small orb where this room's orb is, at the same height, on
    // nothing.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::POI_WANDERER, 0.55),
        Vec3::new(2.09, 0.12, -0.90),
        Vec3::new(0.10, 0.10, 0.10),
    ),
    // And the fourth bar, out there rather than here, which is where the
    // fourth of anything at this station turns up.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EERIE_BRIGHT, 0.90),
        Vec3::new(2.09, -0.34, -0.90),
        Vec3::new(0.16, 0.026, 0.026),
    ),
    // Two hairlines down the hull's own outboard corners, so the shell
    // has an edge to be told from the twin by. Barely: the point is that
    // one of these two boxes is drawn better than the other, and it is
    // not the one you are standing in.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_WANDERER),
        Vec3::new(-1.04, 0.0, -1.04),
        Vec3::new(0.022, 0.92, 0.022),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_WANDERER),
        Vec3::new(1.04, 0.0, -1.04),
        Vec3::new(0.022, 0.92, 0.022),
    ),
];

/// One bar beside the orb, at height `y`.
const fn bar(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_WANDERER, 0.80),
        Vec3::new(x, y, -1.06),
        Vec3::new(0.16, 0.026, 0.026),
    )
}

/// One edge of the twin.
const fn edge(x: f32, y: f32, z: f32, hx: f32, hy: f32, hz: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_WANDERER),
        Vec3::new(x, y, z),
        Vec3::new(hx, hy, hz),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Three, and a fourth that is not an object.** The arithmetic is
    /// the whole station, so it is pinned: four tallies at the toll of
    /// which one is the odd one, four collars of which one has no pillar,
    /// four cage rings of which one hangs off to the side, and a twin
    /// outside that is the fourth room nobody built.
    ///
    /// And the omen's register is **rationed**: `EERIE_BRIGHT` is
    /// `palette`'s "??? toll" and it is spent on the toll and on the
    /// counting, never on a pillar, a wall, a floor line or a lamp.
    #[test]
    fn the_wanderer_counts_to_four_and_says_nothing() {
        // The body you work is the colour of the room and of the crate
        // you leave with, so what you see at the fixture is the light
        // round it. The Guild's dome is brass on a dark plate and reads
        // as a stamp; this one reads as a hole with something in it.
        assert_eq!(
            CHARACTER.handshake.knob_coat.color,
            palette::kind_color(Kind::VeryMysteriousCrate),
            "the toll has grown a knob you can see"
        );
        assert_eq!(
            CHARACTER.handshake.plate.color, CHARACTER.handshake.knob_coat.color,
            "and a plate to tell it apart from"
        );
        assert_eq!(CHARACTER.handshake.lamp, palette::EERIE_BRIGHT);
        assert_eq!(CHARACTER.outfit.lamps, 0, "it does not want to be seen");
        // Four collars, and only three pillars to wear them.
        let collars = CHARACTER
            .decor
            .iter()
            .filter(|fitting| **fitting == collar(fitting.at.x, fitting.at.z))
            .count();
        let pillars = CHARACTER
            .decor
            .iter()
            .filter(|fitting| **fitting == pillar(fitting.at.z))
            .count();
        assert_eq!((collars, pillars), (4, 3), "the arithmetic has been fixed");
        assert_eq!(CHARACTER.light.cage.len(), 4, "and so has the lamp's");
        // The omen's colour appears exactly twice in the whole station,
        // and both are the count: the fourth tally and the fourth bar.
        let omen = CHARACTER
            .handshake
            .trim
            .iter()
            .chain(CHARACTER.decor)
            .chain(CHARACTER.dress)
            .filter(|fitting| fitting.coat.color == palette::EERIE_BRIGHT)
            .count();
        assert_eq!(omen, 2, "the omen's violet has leaked into the furniture");
        for fitting in CHARACTER.decor {
            assert_ne!(fitting.coat.color, palette::EERIE);
            assert_ne!(fitting.coat.color, palette::EERIE_BRIGHT);
        }
        // The twin is out there, drawn only as edges.
        let twin = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.at.x > 1.0)
            .count();
        assert!(twin >= 12, "the room next door has stopped not being there");
    }
}
