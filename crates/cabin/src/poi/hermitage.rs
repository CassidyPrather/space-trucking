//! **The Hermitage** — a hollowed rock in the asteroid belt. The hermits
//! do not trade with strangers; they remember gifts, forever, and shelves
//! slowly grow things for people who gave first (DESIGN.md). **Nobody has
//! seen more than one lit window.**
//!
//! What the lore says, and what each line turned into:
//!
//! - *They do not trade with strangers.* `sim::barter::stock_kinds` puts
//!   this in arithmetic: the Hermitage's shelf count is `karma / 2` and
//!   nothing else, so a stranger's visit finds **empty shelves**. That is
//!   the room. Five bare stone ledges — three across the wall a customer
//!   faces, two down the starboard side — with nothing on any of them,
//!   and the gift economy has been explained without a word of text.
//! - *They remember gifts, forever.* So the port wall carries a rack of
//!   **votive niches**, most of them dark and three of them alight — the
//!   gifts somebody else gave, still burning, long after whoever gave
//!   them stopped calling. The sim's karma is a number that never goes
//!   down; this is what a number that never goes down looks like.
//! - *The Hermitage never deals in crates* (`stock_kinds` again: the
//!   crate roll is skipped here by name). Nothing in this room is bonded,
//!   barred, chuted or stamped. There is no machinery at all.
//! - *A bell at the Hermitage* — docs/ROOMS.md names the handshake by
//!   hand. So the fixture is a bronze bell on a stone corbel with a hemp
//!   pull, over a **gift ledge**: you do not buy here, you leave
//!   something and ring, and somebody who has not been seen answers or
//!   does not.
//! - *Nobody has seen more than one lit window* (DESIGN.md, and
//!   `sim::map::hermitage_lit` keeps it true tick by tick). The exterior
//!   is that sentence and nothing else: an unlit rock with **one** warm
//!   window in it, and **no running lights at all** — a hull that burns
//!   navigation lamps is a hull that wants to be found.
//!
//! ## Rock, and the one piece of metal
//!
//! Wherever a station would put plate, this one puts stone: `Worn::Hull`
//! and the warm sandstone of [`palette::POI_HERMITAGE`] carry the walls,
//! the ledges, the corbels and the tread. The one exception is the
//! **bell**, which is brass, and it is the one exception on purpose: the
//! only worked metal in a room full of rock is the thing you ring to ask
//! for something, and a community that keeps nothing and remembers every
//! gift would have exactly one such object, and would have been given it.
//!
//! And nothing here is electric. What lights the Hermitage is fire —
//! [`palette::EMBER`] in the votives and at the bell, [`palette::AMBER`]
//! in the lamp and in the window — which is why the room is dim without
//! being dark: a candle is not a lamp, but it is not nothing either.

use bevy::prelude::Vec3;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The Hermitage's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: BELL,
    light: VIGIL,
    decor: &CELL,
    outfit: Outfit {
        // Rough sandstone, and **not one running light**. Every other
        // station on the chart burns something at its corners because "a
        // station that let you dock in the dark would be a station with
        // something to hide" (docs/ROOMS.md) — the hermits are not
        // hiding, they simply do not advertise, and the one thing they
        // show the void is a window that is sometimes lit. The colour is
        // kept for the record, the way the derelict keeps its own.
        plate: palette::POI_HERMITAGE,
        lamp: palette::AMBER,
        lamps: 0,
    },
    dress: &HOLLOWED_ROCK,
};

/// **The floor of a hollowed rock.**
///
/// `Stock` keeps its filled field and `Offer` its struck line, but both
/// are stone here rather than paint: the shelf is a dressed sandstone
/// ledge, and the band where it ends is the unhewn rock it was cut out
/// of. A proposal stands inside a warm line — the hermits' one
/// invitation, and the only mark in the room somebody bothered to make
/// findable in the dark.
///
/// The threshold is where the rock is *worn*: stone studs rubbed pale by
/// however many centuries of feet, and a sill that has been polished
/// bright by the same traffic. In a room with no metal and no machinery,
/// the doorstep is the one thing that shows use.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::POI_HERMITAGE),
    rim: Coat::metal(Worn::Hull),
    chalk: Coat::etched(palette::AMBER),
    stud: Coat::enamel(palette::POI_HERMITAGE),
    sill: Coat::etched(palette::POI_HERMITAGE),
};

/// **The bell.** docs/ROOMS.md named it; here is what it is made of.
///
/// A bronze bell on a stone corbel, mouth down, with a hemp pull hanging
/// off it and a ledge beneath where a gift goes. It barely throws — a
/// bell rocks, it does not stroke — and the light that answers is a wick,
/// not a lamp: [`palette::EMBER`], because there is no electricity in
/// this rock and never has been.
///
/// The whole fixture is the transaction the Hermitage actually has: you
/// put something down, you ring, and you wait to find out whether anybody
/// remembers you.
const BELL: Handshake = Handshake {
    plate: Coat::enamel(palette::SOOT),
    // A cone, hung mouth-down. The Guild's dome is a stamp and the
    // market's cone is a snuffer; this one has a clapper in it.
    knob: Shape::Cone,
    knob_coat: Coat::metal(Worn::Brass),
    knob_at: Vec3::new(0.0, 0.14, 0.14),
    knob_half: Vec3::new(0.26, 0.30, 0.10),
    // A hand's width of swing, and no more: the fixture rings, it does
    // not commit anything with a bang.
    throw: 0.025,
    lamp: palette::EMBER,
    trim: &BELL_WORKS,
};

/// The bell's own hardware, in its cell's frame: x and y are fractions of
/// the declared cell, z is metres out of the wall.
const BELL_WORKS: [Fitting; 6] = [
    // The headstock the bell hangs from, and the two stone corbels
    // carrying it. Cut, not cast: this is the rock the room is in.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.50, 0.14),
        Vec3::new(0.34, 0.045, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(-0.40, 0.50, 0.10),
        Vec3::new(0.09, 0.09, 0.09),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(0.40, 0.50, 0.10),
        Vec3::new(0.09, 0.09, 0.09),
    ),
    // The pull: hemp, hanging down the port side of the bell where a hand
    // finds it without looking.
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(-0.26, -0.16, 0.13),
        Vec3::new(0.020, 0.30, 0.020),
    ),
    // The gift ledge, under the bell. This is the counter, and it is a
    // shelf you put things ON rather than a surface a deal crosses.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(0.0, -0.54, 0.11),
        Vec3::new(0.56, 0.05, 0.10),
    ),
    // And one votive standing on it, alight. Somebody has been here
    // before you, and the hermits have not forgotten them either.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 2.4),
        Vec3::new(0.40, -0.44, 0.12),
        Vec3::new(0.045, 0.045, 0.045),
    ),
];

/// **The vigil lamp.** A clay saucer under the deckhead with an actual
/// flame in it: a dome rather than a cone, sooted black on the outside by
/// however long it has been burning, with warm light under it.
///
/// It burns [`VIGIL_BURN`] of the caller budget. Dim, because it is one
/// flame in a rock and not a shop's lighting; not dark, because the
/// Hermitage is a place people *live* and the Umbra Market already owns
/// the room you cannot see (`poi::umbra`).
const VIGIL: Light = Light {
    color: palette::AMBER,
    burn: VIGIL_BURN,
    shade: Shape::Dome,
    shade_coat: Coat::enamel(palette::SOOT),
    glass: Coat::phosphor(palette::EMBER, 0.8),
    cage: &LAMP_CORDS,
};

/// A third of the caller budget: one flame's worth, in a room whose walls
/// are rock and give nothing back. The number's job is to leave the
/// corners of a hollowed rock *dark* — a hermitage lit corner to corner
/// is a chapel with the house lights up — while keeping the ledges and
/// the bell plainly readable, because the whole point of the room is that
/// you can see the shelves are empty.
const VIGIL_BURN: f32 = 0.28;

/// What the lamp hangs on: two cords and a ring, measured off a box one
/// shade across on every side of it. Nobody in this rock owns a chain.
const LAMP_CORDS: [Fitting; 3] = [
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(-0.62, 0.71, -0.62),
        Vec3::new(0.05, 0.95, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(0.62, 0.71, 0.62),
        Vec3::new(0.05, 0.95, 0.05),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 0.30, 0.0),
        Vec3::new(0.97, 0.08, 0.97),
    ),
];

/// **The cell**, inside the room: empty ledges over the goods and down
/// the starboard wall, the rack of votive niches to port, the one
/// window's inner reveal on the front wall with the candle that lights
/// it, and the rough rock the whole place was cut out of.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. Nothing stands in
/// the doorway, and nothing stands in the bell's own column.
///
/// The ledges over the goods hang in the **top course** of the aft wall,
/// where the Guild hangs its bars: a customer at the bell is looking
/// straight at them, and what they are looking at is a row of shelves
/// with nothing on any of them. The room's argument does not work if you
/// have to go and find it.
const CELL: [Fitting; 26] = [
    // ---- the empty shelves ----
    // Nothing is on any of them, because `barter::stock_kinds` puts
    // `karma / 2` goods out and a stranger's karma is zero. The hermits
    // are not out of stock: they have not decided about you.
    ledge(-0.10, 0.20, 0.90, 0.17),
    ledge(0.34, 0.20, 0.90, 0.17),
    ledge(0.78, 0.20, 0.90, 0.17),
    corbel(-0.22, 0.13, 0.90),
    corbel(0.46, 0.13, 0.90),
    // Two more down the starboard wall, for a body that walks round.
    ledge(0.90, 0.02, -0.30, 0.075),
    ledge(0.90, 0.02, 0.16, 0.075),
    // ---- the votive rack, to port ----
    // The gifts they remember. Five niches cut in the rock aft of the
    // crew's own window, three of them still alight — and the dark ones
    // are not empty either, they are simply older than anybody aboard.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(-0.93, -0.19, 0.26),
        Vec3::new(0.05, 0.030, 0.27),
    ),
    niche(0.03),
    niche(0.15),
    niche(0.27),
    niche(0.39),
    niche(0.50),
    votive(0.03),
    votive(0.27),
    votive(0.50),
    // ---- the one window, from the inside ----
    // A splayed stone reveal on the front wall with dark glass in it, and
    // the candle that makes it the lit one standing on its own sill. From
    // out there this is the window nobody has seen a second of; from in
    // here it is a candle on a stone shelf, which is all it ever was.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::GLASS),
        Vec3::new(0.24, 0.16, -0.96),
        Vec3::new(0.22, 0.20, 0.020),
    ),
    reveal(0.24, 0.40, 0.28, 0.045),
    reveal(0.24, -0.08, 0.28, 0.045),
    reveal(-0.02, 0.16, 0.045, 0.24),
    reveal(0.50, 0.16, 0.045, 0.24),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(0.24, -0.15, -0.90),
        Vec3::new(0.30, 0.035, 0.075),
    ),
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::GLINT),
        Vec3::new(0.24, -0.06, -0.90),
        Vec3::new(0.035, 0.060, 0.035),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 3.0),
        Vec3::new(0.24, 0.02, -0.90),
        Vec3::new(0.035, 0.045, 0.035),
    ),
    // ---- the rock itself ----
    // Two lumps left in the ceiling where whoever hollowed this out
    // stopped hollowing. A room cut from an asteroid does not have flat
    // corners, and two bosses is the cheapest honest way to say so.
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.62, 0.86, 0.42),
        Vec3::new(0.30, 0.14, 0.30),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(0.58, 0.88, -0.48),
        Vec3::new(0.26, 0.12, 0.26),
    ),
];

/// One empty ledge, `hx` half-wide, cut into the wall at `(x, y, z)`. The
/// long axis follows whichever wall it is on: the ones over the goods run
/// across the aft face, the ones down the starboard wall run fore-and-aft.
const fn ledge(x: f32, y: f32, z: f32, hx: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(x, y, z),
        Vec3::new(hx, 0.026, if hx > 0.10 { 0.055 } else { 0.15 }),
    )
}

/// The stone corbel a ledge sits on.
const fn corbel(x: f32, y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SOOT),
        Vec3::new(x, y, z),
        Vec3::new(0.05, 0.055, 0.045),
    )
}

/// One niche cut in the port wall, at `z` along the rack.
const fn niche(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SHADOW),
        Vec3::new(-0.95, -0.01, z),
        Vec3::new(0.035, 0.105, 0.048),
    )
}

/// A votive still burning in one of them.
const fn votive(z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 2.2),
        Vec3::new(-0.90, -0.05, z),
        Vec3::new(0.030, 0.030, 0.030),
    )
}

/// One side of the window's splayed stone reveal.
const fn reveal(x: f32, y: f32, hx: f32, hy: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_HERMITAGE),
        Vec3::new(x, y, -0.94),
        Vec3::new(hx, hy, 0.055),
    )
}

/// **The hollowed rock**, outside — and the whole of it is one sentence
/// from DESIGN.md: *nobody has seen more than one lit window*.
///
/// Out here there is no light at all, so a plate's own colour is very
/// nearly black and only what glows is seen. The rock is therefore drawn
/// in radium-floor sandstone ([`super::Finish::Etched`], the "findable
/// with every lamp aboard sold" tone): eight bosses and a spur that turn
/// a lattice box into something lumpy and unfinished, all of it just
/// barely there. And then **one window**, warm, at a height a body could
/// look out of, with a spill of light on the stone under its sill.
///
/// One lit thing on a whole hull, and no running lights round it. That is
/// not a derelict — a derelict burns nothing at all and `poi::wreck` owns
/// that reading — it is somebody at home, with the shutters closed on
/// every window but one.
const HOLLOWED_ROCK: [Fitting; 14] = [
    // The window: the one lit thing on the Hermitage, on the outboard
    // face, sized like something you could actually see a face in.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::AMBER, 2.8),
        Vec3::new(0.30, 0.26, -1.06),
        Vec3::new(0.15, 0.20, 0.030),
    ),
    // Its stone surround, and the spill on the rock below the sill —
    // which is what makes it a window in a wall rather than a rectangle
    // painted on a box.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_HERMITAGE),
        Vec3::new(0.30, 0.51, -1.08),
        Vec3::new(0.22, 0.055, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_HERMITAGE),
        Vec3::new(0.30, 0.01, -1.08),
        Vec3::new(0.22, 0.055, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_HERMITAGE),
        Vec3::new(0.08, 0.26, -1.08),
        Vec3::new(0.055, 0.20, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_HERMITAGE),
        Vec3::new(0.52, 0.26, -1.08),
        Vec3::new(0.055, 0.20, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::AMBER, 0.35),
        Vec3::new(0.30, -0.12, -1.09),
        Vec3::new(0.34, 0.10, 0.030),
    ),
    // The rock. Bosses standing off every face the void can see, so the
    // silhouette is a lump with a light in it and not a crate with a
    // decal on it.
    boss(-1.45, 0.20, -0.55, 0.42, palette::POI_HERMITAGE),
    boss(1.50, -0.05, 0.30, 0.46, palette::SOOT),
    boss(-1.38, -0.30, 0.62, 0.36, palette::SOOT),
    boss(1.42, 0.45, -0.62, 0.38, palette::POI_HERMITAGE),
    boss(0.10, 1.42, -0.35, 0.40, palette::SOOT),
    boss(-0.60, 1.36, 0.45, 0.34, palette::POI_HERMITAGE),
    boss(0.75, 1.34, 0.55, 0.30, palette::SOOT),
    // And the spur: a shard of the parent asteroid still attached, out
    // past the outboard face. Whatever the hermits hollowed, they did not
    // tidy up after themselves.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::SOOT),
        Vec3::new(-0.55, -0.30, -1.85),
        Vec3::new(0.30, 0.26, 0.80),
    ),
];

/// One boss of unhewn rock standing off the hull, `r` across. The tone
/// alternates between the sandstone the hermits cut and the burnt crust
/// they cut it out of: seven identical bosses read as a cloud of blobs,
/// and two tones of them read as a rock.
const fn boss(x: f32, y: f32, z: f32, r: f32, tone: bevy::prelude::Color) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::etched(tone),
        Vec3::new(x, y, z),
        Vec3::new(r, r, r),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The three sentences the Hermitage is.** Empty shelves, gifts
    /// remembered, one lit window — plus the bell docs/ROOMS.md named and
    /// the one piece of metal in the room it is allowed to be made of.
    /// Restyle it freely; a change that quietly lights a second window,
    /// or fills a shelf, has repealed the gift economy.
    #[test]
    fn the_hermitage_gives_and_does_not_sell() {
        assert_eq!(CHARACTER.handshake.knob, Shape::Cone, "it is a bell");
        assert_eq!(
            CHARACTER.handshake.knob_coat.color,
            palette::BRASS,
            "the bell is the one worked metal in the rock"
        );
        assert_eq!(
            CHARACTER.handshake.lamp,
            palette::EMBER,
            "a wick, not a lamp"
        );
        // Nothing on the hull burns except the window: no running lights,
        // and exactly one warm phosphor out there bright enough to be
        // seen as a window rather than as trim.
        assert_eq!(CHARACTER.outfit.lamps, 0, "the hermits do not advertise");
        let windows = CHARACTER
            .dress
            .iter()
            .filter(|fitting| {
                matches!(fitting.coat.finish, super::super::Finish::Phosphor(glow) if glow >= 1.0)
            })
            .count();
        assert_eq!(windows, 1, "nobody has seen more than one lit window");
        // The shelves are the gift economy, and they are empty: every
        // ledge in the room is a bare stone slab with nothing standing on
        // it, which is what `karma / 2` looks like at karma zero.
        let ledges = CHARACTER
            .decor
            .iter()
            .filter(|fitting| {
                **fitting == ledge(fitting.at.x, fitting.at.y, fitting.at.z, fitting.half.x)
            })
            .count();
        assert!(ledges >= 4, "the hermits have run out of shelves");
        // And the room is lit by fire, dimly: warm, and well under a
        // shop's.
        assert_eq!(CHARACTER.light.color, palette::AMBER);
        const {
            assert!(CHARACTER.light.burn < 0.5, "one flame, not a lighting rig");
        }
    }
}
