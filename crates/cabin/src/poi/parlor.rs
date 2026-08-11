//! **The casino's parlor** — an event room, one of a kind, whose whole
//! conceit is that it has no visible doors. Stake cargo on its offer area
//! and work the handshake: double, or a commemorative chip (docs/ROOMS.md).
//!
//! # The one place in this game allowed to be gaudy
//!
//! Every other room here is a working surface: hulls, bonded paint,
//! scorched plate, honest brass. The parlor is the exception the palette
//! was waiting for, and it spends it on **two hues and one joke**.
//!
//! - The gold is [`Kind::GildedIdol`]'s, because a gilded idol is
//!   literally what the house pays a winner with (`Sim::spin_wager`).
//!   It is the colour of everything the room *promises*: the wheel, the
//!   chandelier, the lamp that lights when there is a stake to settle.
//! - The rose is [`Kind::CasinoChip`]'s, because a commemorative chip is
//!   literally what the house gives a loser — "which the house insists
//!   is priceless". It is the colour of everything the room actually
//!   *delivers*: the coving, the neon on the sill, the sign inside and
//!   the halo outside. The gold on the shell is the swag of bulbs
//!   across its face, which is bait, and there is nothing else gold out
//!   there at all.
//!
//! That split is the whole design. **Look at what is gold and what is
//! rose and you are looking at the odds.** The lamp over the fixture
//! promises gold; the wall the fixture faces is a rack of rose.
//!
//! # The architecture quietly admits the house always wins
//!
//! The room's furniture is a **trophy rack**: a row of commemorative
//! chips mounted like medals, all identical, all worthless, all somebody
//! else's stake. Nothing in here displays a prize. A house that hangs
//! its consolation prizes on the wall in a lit frame is telling you the
//! truth in the only language this game has, which is furniture.
//!
//! And there is **no clock and no window**, which is not a thing a
//! character can add — it is a thing it can decline to break. What it
//! does instead is run one unbroken band of neon round all four walls at
//! a height the doorway cannot reach, so the room reads as continuous
//! from any corner: the coving passes over the one door as if it were
//! not there. The house does not hide the exit. It decorates it — the
//! sill is neon too, in the loser's rose, so the way out looks like more
//! casino.
//!
//! # The seven
//!
//! Canon says *neon heptagram, no visible doors* (`sim::encounter`).
//! Nothing in this game rotates — every body is axis-aligned, and a
//! character has no rotation knob — so a seven-pointed star cannot be
//! drawn with chords. It is drawn with **lamps** instead, seven of them
//! on a hoop, three times: on the sign over the betting floor, round
//! the halo outside, and round the wheel as the pins the pointer will
//! land on.
//! Seven pins and a coin that is fifty-fifty (`Encounters::casino_coin`)
//! is the house's other quiet admission, and it is on the fixture you
//! work with your own hand.

use bevy::prelude::{Color, Vec3};
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The parlor's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: THE_WHEEL,
    light: CHANDELIER,
    decor: &THE_HOUSE,
    outfit: Outfit {
        // A travelling house, painted the colour of its own upholstery,
        // burning the loser's rose for running lights. It is not
        // guiding you in. It is advertising.
        plate: palette::kind_color(Kind::Couch),
        lamp: palette::kind_color(Kind::CasinoChip),
        lamps: 2,
    },
    dress: &THE_SIGN,
};

/// What the house pays a winner with, and therefore what it promises.
const IDOL: Color = palette::kind_color(Kind::GildedIdol);
/// What the house gives a loser, and therefore what it delivers.
const CHIP: Color = palette::kind_color(Kind::CasinoChip);
/// Lamp gold: the ceiling-lamp hue, on the thing that is a ceiling lamp.
const LAMPLIGHT: Color = palette::kind_color(Kind::CeilingLamp);
/// Upholstery plum, on the upholstery.
const PLUSH: Color = palette::kind_color(Kind::Couch);

/// The betting floor.
///
/// A parlor declares an `Offer` band and no `Stock` band
/// (`RoomKind::tile_of`): the house lays out nothing and you lay out
/// everything, which is a fair description of a casino. So the knob that
/// matters here is the **chalk** — the line struck round the bare deck a
/// proposal stands on — and in this room it is not chalk, it is neon.
/// Stand your stake inside the glowing line.
///
/// The threshold is the house's, too: gold studs where feet cross, and a
/// rose neon sill. The one door it cannot do without is dressed until it
/// looks like part of the sign.
const TILES: Tiles = Tiles {
    // Never painted here — a parlor has no `Stock` band. Recorded in the
    // house's own two hues rather than left saying something else.
    stock: Coat::enamel(PLUSH),
    rim: Coat::metal(Worn::Brass),
    chalk: Coat::phosphor(CHIP, 2.4),
    stud: Coat::metal(Worn::Brass),
    sill: Coat::phosphor(CHIP, 2.0),
};

/// **The wheel.** A gold disc set in a padded plum surround, ringed by
/// seven rose pins, with a pointer over the top of it. You do not strike
/// a bargain in here and you do not get stamped: you set it going and it
/// comes down where it comes down.
///
/// The pointer is a lie of exactly the useful kind — the coin behind it
/// is a fifty-fifty on the sim's own stream — and the pins are rose
/// because whichever one it lands on, the house has a chip for you.
const THE_WHEEL: Handshake = Handshake {
    plate: Coat::enamel(PLUSH),
    knob: Shape::Dome,
    knob_coat: Coat::enamel(IDOL),
    knob_at: Vec3::new(0.0, 0.10, 0.07),
    knob_half: Vec3::new(0.34, 0.34, 0.045),
    // A wheel does not travel. It gives, just enough that a hand knows
    // it did something.
    throw: 0.022,
    // The lamp promises gold. The rack on the wall behind you does not.
    lamp: IDOL,
    trim: &WHEEL_WORKS,
};

/// The wheel's pins and its pointer, in the cell's own frame: x and y are
/// fractions of the declared cell, z is metres out of the wall. Seven
/// pins on a circle, because a heptagram cannot be drawn out of
/// axis-aligned boxes and seven lamps on a hoop can.
const WHEEL_WORKS: [Fitting; 8] = [
    pin(0.000, 0.620),
    pin(-0.407, 0.424),
    pin(-0.507, -0.016),
    pin(-0.226, -0.369),
    pin(0.226, -0.369),
    pin(0.507, -0.016),
    pin(0.407, 0.424),
    // The pointer, over the top, in the gold it is pointing at.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(IDOL),
        Vec3::new(0.0, 0.76, 0.075),
        Vec3::new(0.055, 0.10, 0.03),
    ),
];

/// One pin on the wheel, at `(x, y)` in the cell.
const fn pin(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(CHIP, 2.6),
        Vec3::new(x, y, 0.075),
        Vec3::new(0.075, 0.075, 0.045),
    )
}

/// **The chandelier.** A gold globe in a gold hoop, burning a little
/// under two thirds of the caller budget: a room with no clocks is not
/// lit like a warehouse, and a house that made you squint at a wager
/// would be a house nobody wagers in twice.
///
/// The seven are NOT on this fitting, and that is a decision the room
/// made for them: a pendant hangs at the middle of a 2.2-metre box, half
/// a metre from a standing body's face, so anything hung on it is in
/// your eyes and nowhere else. They went up onto the ceiling instead,
/// where the sign reads from the doorway and from the betting floor
/// both, which is what a sign is for.
const CHANDELIER: Light = Light {
    color: LAMPLIGHT,
    burn: 0.64,
    shade: Shape::Dome,
    shade_coat: Coat::enamel(IDOL),
    glass: Coat::phosphor(LAMPLIGHT, 2.2),
    cage: &THE_GILT,
};

/// The gold work round the globe, off a box one shade across on every
/// side of the lamp: a hoop under it and a boss over it, and that is
/// all a fitting this close to a standing body may have.
const THE_GILT: [Fitting; 2] = [
    Fitting::new(
        Shape::Ring,
        Coat::enamel(IDOL),
        Vec3::new(0.0, -0.42, 0.0),
        Vec3::new(0.86, 0.085, 0.86),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(IDOL),
        Vec3::new(0.0, 0.70, 0.0),
        Vec3::new(0.24, 0.24, 0.24),
    ),
];

/// **The house**, inside: one unbroken band of neon round every wall,
/// plush on the flanks, and the rack of commemorative chips.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. The coving runs
/// at a height the doorway cannot reach, so it can cross the aft wall
/// unbroken: the room has no corner where it stops being the room.
const THE_HOUSE: [Fitting; 23] = [
    // **The seven**, on the front wall, over the betting floor: a plum
    // board in a gold frame with seven rose lamps set round it. It hangs
    // on the wall you face coming through the seam and the wall you face
    // laying a stake down, which is the whole job of a sign.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.06, 0.10, -0.962),
        Vec3::new(0.56, 0.56, 0.016),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(PLUSH),
        Vec3::new(0.06, 0.10, -0.945),
        Vec3::new(0.50, 0.50, 0.018),
    ),
    lamp(0.060, 0.500),
    lamp(-0.253, 0.349),
    lamp(-0.330, 0.011),
    lamp(-0.114, -0.260),
    lamp(0.234, -0.260),
    lamp(0.450, 0.011),
    lamp(0.373, 0.349),
    // The coving, all the way round, over the top of the one door.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(CHIP, 1.8),
        Vec3::new(0.0, 0.62, 0.955),
        Vec3::new(0.98, 0.035, 0.030),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(CHIP, 1.8),
        Vec3::new(0.0, 0.62, -0.955),
        Vec3::new(0.98, 0.035, 0.030),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(CHIP, 1.8),
        Vec3::new(-0.955, 0.62, 0.0),
        Vec3::new(0.030, 0.035, 0.925),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(CHIP, 1.8),
        Vec3::new(0.955, 0.62, 0.0),
        Vec3::new(0.030, 0.035, 0.925),
    ),
    // Plush on both flanks, in a gold reveal: what a room is lined with
    // when nobody in it is meant to notice the time.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.965, -0.18, -0.28),
        Vec3::new(0.020, 0.44, 0.62),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(PLUSH),
        Vec3::new(-0.950, -0.18, -0.28),
        Vec3::new(0.022, 0.40, 0.58),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.965, -0.52, 0.22),
        Vec3::new(0.020, 0.40, 0.56),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(PLUSH),
        Vec3::new(0.950, -0.52, 0.22),
        Vec3::new(0.022, 0.36, 0.52),
    ),
    // **The rack.** Five commemorative chips, mounted like medals on a
    // lit gold board on the starboard wall, with the rail they sit on
    // under them. Every one of them is a stake somebody laid on the
    // floor you are standing on, and the house kept all of them.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(IDOL),
        Vec3::new(0.965, 0.24, -0.40),
        Vec3::new(0.018, 0.20, 0.60),
    ),
    chip(-0.86),
    chip(-0.63),
    chip(-0.40),
    chip(-0.17),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.945, 0.02, -0.40),
        Vec3::new(0.030, 0.022, 0.60),
    ),
];

/// One lamp of the sign's seven, at `(x, y)` on the front wall.
const fn lamp(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(CHIP, 2.4),
        Vec3::new(x, y, -0.905),
        Vec3::new(0.058, 0.058, 0.038),
    )
}

/// One commemorative chip on the rack, at `z` along the starboard wall.
/// A `Dome` flattened against the wall is a disc facing the room, which
/// is what a chip is.
const fn chip(z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(CHIP, 2.0),
        Vec3::new(0.940, 0.24, z),
        Vec3::new(0.022, 0.13, 0.095),
    )
}

/// **The sign**, outside: a neon halo standing over the roof on two
/// struts, seven lamps on it, and a swag of bulbs across the outboard
/// face. It is a rig, not a building — the house packs up and goes, and
/// the one thing it bolts on is the thing that says it is open.
///
/// Out in the void there is no light and no shadow maps, so a plate's
/// own colour is very nearly black and only what glows is seen. Here
/// that is a gift: a casino is *supposed* to be a shape made entirely of
/// its own lights, and this one is. You can tell what has pulled
/// alongside from a long way off, and what it is is a funfair.
const THE_SIGN: [Fitting; 15] = [
    // The halo, hanging flat over the roof.
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(CHIP, 2.4),
        Vec3::new(0.0, 1.10, -0.10),
        Vec3::new(0.92, 0.060, 0.92),
    ),
    // Two struts, standing clear of the shell they hold it over.
    Fitting::new(
        Shape::Post,
        Coat::enamel(IDOL),
        Vec3::new(-0.52, 1.05, -0.10),
        Vec3::new(0.045, 0.05, 0.045),
    ),
    Fitting::new(
        Shape::Post,
        Coat::enamel(IDOL),
        Vec3::new(0.52, 1.05, -0.10),
        Vec3::new(0.045, 0.05, 0.045),
    ),
    // The seven, on the halo. Rose and gold alternating round an odd
    // number of points, which never comes out even — a detail the house
    // is not going to lose sleep over.
    spike(0.000, 0.920, true),
    spike(-0.719, 0.574, false),
    spike(-0.897, -0.205, true),
    spike(-0.399, -0.829, false),
    spike(0.399, -0.829, true),
    spike(0.897, -0.205, false),
    spike(0.719, 0.574, true),
    // A swag of bulbs across the outboard face, sagging in the middle
    // the way a string of lights does when nobody has tightened it.
    swag(-0.80, 0.46),
    swag(-0.40, 0.20),
    swag(0.00, 0.10),
    swag(0.40, 0.20),
    swag(0.80, 0.46),
];

/// One lamp on the halo, at `(x, z)` round it; `rose` picks which of the
/// house's two hues it burns.
const fn spike(x: f32, z: f32, rose: bool) -> Fitting {
    Fitting::new(
        Shape::Dome,
        if rose {
            Coat::phosphor(CHIP, 3.2)
        } else {
            Coat::phosphor(IDOL, 3.0)
        },
        Vec3::new(x, 1.21, z - 0.10),
        Vec3::new(0.10, 0.10, 0.10),
    )
}

/// One bulb in the swag on the outboard face, at `x` across it and `y`
/// up it.
const fn swag(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(IDOL, 2.6),
        Vec3::new(x, y, -1.10),
        Vec3::new(0.085, 0.085, 0.085),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The house's own reading**, held where the lore put it: gold is
    /// what it promises and rose is what it hands over, the wheel is a
    /// wheel rather than a counter, the sign is seven, and the rack of
    /// chips on the wall is furniture rather than decoration. Repaint it
    /// freely — a change that quietly retires one of these is a change
    /// that retires the joke.
    #[test]
    fn the_house_promises_gold_and_delivers_rose() {
        assert_eq!(CHARACTER.handshake.lamp, IDOL, "the lamp promises gold");
        assert_eq!(CHARACTER.handshake.knob, Shape::Dome, "the wheel is a disc");
        assert_eq!(
            CHARACTER.tiles.chalk.color, CHIP,
            "the betting line is neon"
        );
        // Seven pins on the wheel, seven bulbs on the chandelier, seven
        // lamps on the halo: the heptagram, three times, drawn the one
        // way axis-aligned bodies can draw it.
        let sevens = [
            CHARACTER
                .handshake
                .trim
                .iter()
                .filter(|f| f.shape == Shape::Dome)
                .count(),
            CHARACTER
                .decor
                .iter()
                .filter(|f| f.shape == Shape::Dome && f.at.x.abs() < 0.6)
                .count(),
            CHARACTER
                .dress
                .iter()
                .filter(|f| f.at.y > 1.0 && f.shape == Shape::Dome)
                .count(),
        ];
        for count in sevens {
            assert_eq!(count, 7, "the sign has lost a point");
        }
        // The rack: every chip on the wall is rose, and there is not one
        // gilded idol among them.
        let rack = CHARACTER
            .decor
            .iter()
            .filter(|f| f.shape == Shape::Dome && f.at.x.abs() > 0.6)
            .collect::<Vec<_>>();
        assert!(rack.len() >= 4, "the rack has stopped being furniture");
        for chip in rack {
            assert_eq!(chip.coat.color, CHIP, "the house is displaying a prize");
        }
    }
}
