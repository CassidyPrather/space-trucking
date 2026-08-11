//! **The comet** — no name on any chart. When it dives near the sun,
//! people go chip ice off it, because it is there and the ice is free.
//! Sometimes there is something else in the ice (DESIGN.md).
//!
//! It brings **no room** in the shipping build: docking at the comet
//! harvests shards and attaches nothing, so on the chart today nothing
//! here is ever built. The character is written anyway, for two reasons —
//! the registry is total (`poi::mod`'s exhaustiveness test), and the
//! developer fixture moors the same board at every station (`--docked
//! 10`), so this file is the answer to "what does the comet look like
//! when it does have a room", written while the lore is in front of us
//! rather than a year from now.
//!
//! What the lore says, and what each line turned into:
//!
//! - *It is not a station.* Nobody built it, nobody maintains it, and
//!   nobody is behind the counter. So there is no plate, no enamel, no
//!   brass and no counter: the room is a **cut face** in a dirty
//!   snowball, and every human thing in it — a work light on a hoop, an
//!   ice screw, a strung line, a bolted tread — was left by whoever was
//!   here last apparition. That is the one-glance tell, and it is the
//!   opposite of every other room in the game: the fittings are somebody
//!   else's, and there are almost none of them.
//! - *The ice is free.* `sim::barter::VALUE`'s comet row is a placeholder
//!   of twos, because the comet never opens a trade room and never haggles
//!   (`stock_kinds` is never called for it). Nothing here is priced, so
//!   nothing here is painted.
//! - *A dirty snowball.* The colour scheme is the physics: bright ice
//!   ([`palette::POI_COMET`], and `CometIce`'s own hue where the room
//!   means the cargo you chip off it) inside a **black crust**
//!   ([`palette::SOOT`]) — a comet is one of the darkest things in the
//!   solar system with one of the brightest interiors, which is why the
//!   `Stock` field here is pale and its rim band is nearly black. No other
//!   station reads bright-inside-dark-edge, and the reading survives the
//!   filled-against-hollow law untouched.
//! - *It only exists at perihelion.* `map::comet_visible` gates the dock
//!   on the dive, and `map::comet_apparition` counts the passes. So the
//!   room is **lit hard** — you are working next to the sun with the tail
//!   blazing off you — and the exterior is not a hull at all. It is a
//!   coma and a tail: the one silhouette on the chart that is weather
//!   rather than architecture.
//! - *Sometimes there is something else in the ice.* One block in the cut
//!   face has a **dark lump frozen inside it**, and that is all the room
//!   says about it. It is not violet and it does not glow: the omen's
//!   register belongs to the crates and to `???`, and a comet that
//!   flagged its own mystery in the game's one *something is wrong*
//!   colour would be a comet explaining itself. This one lets you notice.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The comet's own room, for the day it has one.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: ICE_SCREW,
    light: WORK_LIGHT,
    decor: &CUT_FACE,
    outfit: Outfit {
        // Crust: the black rind a comet wears everywhere the sun has not
        // just cracked it open. One running light, and it is not the
        // comet's — somebody bolted a beacon to a rock so the next crew
        // could find it in the glare, which is the only maintenance this
        // object has ever had.
        plate: palette::SOOT,
        lamp: palette::GLINT,
        lamps: 1,
    },
    dress: &COMA_AND_TAIL,
};

/// **The cut face.**
///
/// `Stock` is the ice itself — pale, filled, and bordered by the black
/// crust it was cut out of, which is the dirty-snowball reading and is
/// nobody else's on the chart. `Offer` is scratched into that crust with
/// the corner of a chisel: a frost line, not a chalk line, because there
/// is no chalk out here and nobody to hold it.
///
/// The threshold is the one place a human being spent money: a bolted
/// steel tread, going green at the edges where the rime has started to
/// take it back.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::kind_color(Kind::CometIce)),
    rim: Coat::enamel(palette::SOOT),
    chalk: Coat::etched(palette::POI_COMET),
    stud: Coat::metal(Worn::Rivet),
    sill: Coat::etched(palette::POI_COMET),
};

/// **The ice screw.** There is nobody to shake hands with on a comet, so
/// the fixture is not a counter — it is the anchor you drive into the
/// face before you start cutting, and committing is driving it home.
///
/// A steel spike through a hanger ring, straight into the ice, with a
/// hank of line made off to it and two shards knocked loose lying where
/// they fell. It throws further than anything else in the registry
/// because it is the only fixture in the game you hit with a hammer.
const ICE_SCREW: Handshake = Handshake {
    plate: Coat::enamel(palette::kind_color(Kind::CometIce)),
    // A post, driven. Not a dome to stamp, not a cone to snuff, not a
    // bell to ring: a spike.
    knob: Shape::Post,
    knob_coat: Coat::metal(Worn::Rivet),
    knob_at: Vec3::new(0.0, 0.05, 0.13),
    knob_half: Vec3::new(0.10, 0.24, 0.10),
    throw: 0.10,
    // A bare work lamp, warm-white rather than the amber of an invitation:
    // nobody is inviting you, the light is just on.
    lamp: palette::GLINT,
    trim: &SCREW_WORKS,
};

/// The screw's own hardware, in its cell's frame: x and y are fractions of
/// the declared cell, z is metres out of the wall.
const SCREW_WORKS: [Fitting; 5] = [
    // The hanger the spike runs through.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.05, 0.075),
        Vec3::new(0.26, 0.055, 0.26),
    ),
    // The line, made off to it and running away down the face. Everything
    // on a comet is tied to something.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(-0.42, -0.28, 0.055),
        Vec3::new(0.44, 0.020, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(0.44, -0.34, 0.055),
        Vec3::new(0.020, 0.42, 0.020),
    ),
    // Two shards knocked off the face and left where they landed, because
    // there is no floor here that anybody sweeps.
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::POI_COMET),
        Vec3::new(-0.52, -0.62, 0.075),
        Vec3::new(0.10, 0.09, 0.09),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::POI_COMET),
        Vec3::new(-0.30, -0.68, 0.065),
        Vec3::new(0.07, 0.06, 0.06),
    ),
];

/// **Somebody's work light**, on a hoop.
///
/// The shade is a **ring** — which is to say there is no shade: a bare
/// bulb hung in a wire hoop by a crew who were not planning to stay, and
/// the only fixture in the registry that does not attempt to soften what
/// it burns. It burns hard and cold, because at perihelion the light out
/// here is hard and cold and because a face you are cutting is a face you
/// need to see. The pool it throws is still the room's own, derived from
/// the room's box: even a comet cannot bill you for electricity.
const WORK_LIGHT: Light = Light {
    color: palette::POI_COMET,
    burn: 0.92,
    shade: Shape::Ring,
    shade_coat: Coat::metal(Worn::Rivet),
    glass: Coat::phosphor(palette::POI_COMET, 2.8),
    cage: &LAMP_HOOK,
};

/// The hook and the wire it hangs by. Two fittings and a bent bit of
/// steel: the whole installation.
const LAMP_HOOK: [Fitting; 2] = [
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.91, 0.0),
        Vec3::new(0.06, 0.75, 0.06),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, 0.55, 0.0),
        Vec3::new(0.55, 0.14, 0.55),
    ),
];

/// **The cut face**, inside: the quarried ice to starboard, the black
/// crust above it, the safety line strung across, a scatter of shards on
/// the deck, and one block with something in it.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. Nothing stands in
/// the doorway, and nothing stands in the screw's own column.
///
/// There is deliberately **not much of it**. Every other station fills its
/// room with the business it is in; the comet is not in a business, and a
/// half-empty room with a work light and somebody's rope in it is the
/// reading. Furnishing this place properly would be the one mistake this
/// file can make.
const CUT_FACE: [Fitting; 19] = [
    // ---- the quarried face, to starboard ----
    // Ice cut back in steps, the way a face is actually worked: big
    // blocks low, small ones high, and the crust left on above them.
    block(0.86, -0.62, -0.42, 0.26, 0.34),
    block(0.88, -0.60, 0.16, 0.22, 0.30),
    block(0.90, -0.06, -0.30, 0.18, 0.24),
    block(0.90, 0.02, 0.30, 0.14, 0.20),
    block(0.90, 0.44, -0.10, 0.12, 0.26),
    // The crust over the cut: what the whole rock looks like from
    // outside, seen here in section, one course of black over the ice.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SOOT),
        Vec3::new(0.95, 0.82, 0.0),
        Vec3::new(0.05, 0.16, 0.86),
    ),
    // ---- something else in the ice ----
    // A block with a dark lump frozen in the middle of it. No glow, no
    // violet, no beacon: the game's *something is wrong* colour is the
    // omen's and the crates', and a comet that pointed at its own mystery
    // would have explained it. This one is just there, and either you
    // notice it or you chip somewhere else.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_COMET),
        Vec3::new(0.80, -0.20, 0.62),
        Vec3::new(0.20, 0.24, 0.22),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::SHADOW),
        Vec3::new(0.74, -0.20, 0.62),
        Vec3::new(0.10, 0.11, 0.10),
    ),
    // ---- the crust overhead ----
    // The ceiling is the inside of the rind: black, lumpy, and much
    // closer than you would like.
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.44, 0.88, -0.30),
        Vec3::new(0.34, 0.11, 0.34),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::SOOT),
        Vec3::new(0.24, 0.90, 0.44),
        Vec3::new(0.28, 0.09, 0.28),
    ),
    // ---- what the last crew left ----
    // A line strung the width of the room at chest height, because the
    // comet is moving and you are not tied to it. Two eyes and a rope.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::TRIM_GIVE),
        Vec3::new(0.0, 0.10, -0.55),
        Vec3::new(0.94, 0.014, 0.014),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.93, 0.10, -0.55),
        Vec3::new(0.05, 0.05, 0.05),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.93, 0.10, -0.55),
        Vec3::new(0.05, 0.05, 0.05),
    ),
    // The pick, leaned where somebody leaned it and never came back for.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.86, -0.55, -0.62),
        Vec3::new(0.020, 0.44, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.86, -0.10, -0.62),
        Vec3::new(0.13, 0.020, 0.020),
    ),
    // Shards, on the deck, where they fell.
    shard(0.42, -0.923, 0.28, 0.11),
    shard(0.10, -0.944, -0.10, 0.08),
    shard(-0.30, -0.951, 0.34, 0.07),
    shard(0.62, -0.937, -0.36, 0.09),
];

/// One block of the cut face, standing off the starboard wall.
const fn block(x: f32, y: f32, z: f32, hy: f32, hz: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::POI_COMET),
        Vec3::new(x, y, z),
        Vec3::new(0.09, hy, hz),
    )
}

/// One shard of ice on the deck, `r` across.
const fn shard(x: f32, y: f32, z: f32, r: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::kind_color(Kind::CometIce)),
        Vec3::new(x, y, z),
        Vec3::new(r, r * 0.7, r),
    )
}

/// **The coma and the tail**, outside — and this is the point of the
/// comet, because *a comet is not a station*.
///
/// Every other place on the chart shows the void a hull with hardware
/// bolted to it. This one shows weather. The shell disappears inside a
/// lumpy crust of ice and rind, a knot of bright coma sits over the
/// sunward shoulder of it, and a dozen streamers run away from the
/// outboard face out to nearly the whole of [`super::DRESS_REACH`] — a
/// **tail**, which is a shape nothing else in this game has and nothing
/// else ever should.
///
/// It is drawn in phosphors at low glow rather than in the radium-brass
/// hairlines the built stations use, because a tail is not hardware: it
/// is the only thing out here genuinely made of light, and it should be
/// the thing you see first and the thing you cannot mistake for a mast.
///
/// **Everything out here is opaque**, which is the constraint that shaped
/// it: the art direction has one material family for glow and it is not
/// translucent, so a coma modelled as a big soft sphere is a big hard
/// sphere that swallows the rock inside it. The fix is the one a
/// low-poly painter would use anyway — a *cluster* of small bright bodies
/// where the coma is brightest, and the tail carrying the rest of the
/// reading by length.
const COMA_AND_TAIL: [Fitting; 25] = [
    // The nucleus: ice and rind lumped over the shell until the box stops
    // being a box.
    lump(-1.46, 0.10, 0.30, 0.44, palette::POI_COMET),
    lump(1.52, -0.10, -0.35, 0.48, palette::SOOT),
    lump(1.40, 0.40, 0.55, 0.38, palette::POI_COMET),
    lump(-1.36, -0.35, -0.55, 0.34, palette::SOOT),
    lump(-0.20, 1.44, 0.30, 0.42, palette::POI_COMET),
    lump(0.65, 1.36, -0.40, 0.34, palette::SOOT),
    // The coma: sublimating ice standing off the sunward face, in bright
    // knots rather than an envelope.
    puff(-0.42, 0.28, -1.24, 0.20, 0.60),
    puff(0.38, -0.15, -1.30, 0.22, 0.45),
    puff(0.06, 0.58, -1.22, 0.17, 0.52),
    puff(-0.22, -0.46, -1.26, 0.16, 0.38),
    puff(0.52, 0.42, -1.20, 0.14, 0.40),
    puff(-0.62, -0.12, -1.22, 0.15, 0.34),
    // The tail: twelve lengths of light off the outboard face, the short
    // bright ones nested inside the long faint ones so the whole thing
    // fades outward without a gradient anywhere. Anything longer than
    // this is a second station, and a comet's tail coming out ten times
    // too short is a compromise the containment law is worth.
    streamer(0.0, 0.06, 0.95, 0.50),
    streamer(-0.22, 0.24, 0.50, 0.44),
    streamer(0.26, 0.22, 0.50, 0.44),
    streamer(-0.22, 0.24, 0.92, 0.16),
    streamer(0.26, 0.22, 0.92, 0.16),
    streamer(-0.40, -0.08, 0.44, 0.34),
    streamer(0.44, -0.12, 0.44, 0.34),
    streamer(-0.40, -0.08, 0.88, 0.12),
    streamer(0.44, -0.12, 0.88, 0.12),
    streamer(-0.12, 0.48, 0.74, 0.14),
    streamer(0.18, -0.40, 0.74, 0.14),
    streamer(0.04, -0.22, 0.96, 0.10),
    // The beacon's bracket, up beside the one running light somebody
    // bolted on: a stub of angle iron holding a lamp to a rock.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::BRASS),
        Vec3::new(-1.06, 0.86, -0.86),
        Vec3::new(0.05, 0.22, 0.05),
    ),
];

/// One lump of nucleus standing off the hull, `r` across.
const fn lump(x: f32, y: f32, z: f32, r: f32, tone: bevy::prelude::Color) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::etched(tone),
        Vec3::new(x, y, z),
        Vec3::new(r, r, r),
    )
}

/// One length of tail, running out of the outboard face from `(x, y)`,
/// `len` long in shell half-extents and glowing at `glow`. Thin: a
/// streamer as thick as a spar is a spar.
const fn streamer(x: f32, y: f32, len: f32, glow: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::POI_COMET, glow),
        Vec3::new(x, y, -1.05 - len),
        Vec3::new(0.014, 0.014, len),
    )
}

/// One knot of coma standing off the sunward face, `r` across.
const fn puff(x: f32, y: f32, z: f32, r: f32, glow: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::CometIce), glow),
        Vec3::new(x, y, z),
        Vec3::new(r, r, r),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A comet is not a station**, and every clause of that is here:
    /// nothing in it is painted or bonded, the fixture is driven rather
    /// than worked, the light is somebody else's, and the thing you see
    /// from outside is weather rather than hardware. Also: the mystery in
    /// the ice stays unlit, because the omen's violet is not the comet's
    /// to spend.
    #[test]
    fn the_comet_is_weather_with_ice_in_it() {
        assert_eq!(CHARACTER.handshake.knob, Shape::Post, "you drive it");
        const {
            assert!(
                CHARACTER.handshake.throw > super::super::NEUTRAL.handshake.throw * 2.0,
                "an ice screw goes in further than a shop counter's slug"
            );
        }
        // Bright ice inside a black rim: the dirty-snowball reading, and
        // the inverse of every painted station on the chart.
        assert_eq!(CHARACTER.tiles.rim.color, palette::SOOT);
        assert_eq!(CHARACTER.light.shade, Shape::Ring, "a bulb on a hoop");
        const {
            assert!(
                CHARACTER.light.burn > 0.8,
                "you cannot cut what you cannot see"
            );
        }
        // The tail: long, many, and made of light rather than metal.
        let tail = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.half.z > 0.2 && fitting.at.z < -1.0)
            .count();
        assert!(tail >= 6, "the comet lost its tail");
        let reach = CHARACTER
            .dress
            .iter()
            .map(|fitting| fitting.span().0.z)
            .fold(0.0_f32, f32::min);
        assert!(
            reach < -2.5,
            "the tail stops at {reach} shell half-extents; it is meant to stream"
        );
        // Nothing in this room wears the omen's register. The comet has a
        // mystery; it does not have a warning light.
        for fitting in CHARACTER.decor.iter().chain(CHARACTER.dress) {
            assert_ne!(fitting.coat.color, palette::EERIE);
            assert_ne!(fitting.coat.color, palette::EERIE_BRIGHT);
        }
    }
}
