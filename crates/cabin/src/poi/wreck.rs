//! **A derelict's hold** — an event room, and one of a kind by nature, so
//! its kind is its identity. A derelict has exactly one seam worth
//! trusting: the one you just mated. The rest of the hull is vacuum with
//! edges (docs/ROOMS.md).
//!
//! # Nobody keeps this
//!
//! A station's room is its premises: swept, painted, lit, and there next
//! time. **A derelict is somebody's grave that drifted into your path**,
//! and the whole character is written off that one sentence:
//!
//! - *`light.burn` is zero.* A derelict has no lights of its own, so it
//!   has no light source at all rather than one burning nothing — the
//!   lights-out case the pendant builder already allows. What you can
//!   see in here is what your own lamps reach through the seam and what
//!   the yard's radium paint has been doing on its own since.
//! - *Everything you CAN make out is `Etched`.* That finish is the
//!   game's declared lights-out floor (`glow::etched`, "legible on
//!   technicality when every lamp aboard is gone"), and this is the one
//!   room where it is not a technicality but the entire lighting plan.
//!   Two registers only: cold [`palette::ICON`] for structure — the
//!   role is literally *etched lines while the function sleeps* — and
//!   radium [`palette::BRASS`] for safety hardware, which is the one
//!   warm thing that outlived the crew.
//! - *Nothing is symmetrical.* The Guild's room is square to itself
//!   because a bonded store is maintained; here the ribs are unevenly
//!   spaced with one plate missing, the port rail is broken and the
//!   starboard one is not, and the sealed hatch is three dogs and an
//!   empty hole. `Fitting` cannot rotate — every body in this game is
//!   axis-aligned — so *wrecked* has to be said with spacing, and it is.
//! - *There is no paint under the goods.* `Stock` keeps its filled field
//!   (the class's form is not a station's to spend), but the field is
//!   bare [`palette::HULL`]: where a market paints its floor, a dead
//!   ship has hull, and the hull is `Etched` only so that a wall in a
//!   room with no lamp is *just* distinguishable from no wall at all.
//!   The rim band survives in radium, which is why you can find the bay.
//!
//! # What the sim does here, and what the room says about it
//!
//! The window opens and a hold's worth of somebody's story is on its own
//! floor — one odd piece and one ordinary one, sometimes a mysterious
//! crate (`sim::encounter`). The handshake is `Sim::claim_salvage`:
//! *"nobody is watching, and what you marked is what you carry. The
//! derelict asks nothing and answers nothing."* So the fixture is not a
//! counter and not a machine that decides about you — it is the hold's
//! own **cargo ring**, a thing you hook a load to, on a plate with a
//! staple through it. There is no register, no chit, no brasswork
//! arranged to be looked at. The one lamp on it is the fixture's own and
//! cannot be deleted; it burns [`palette::ICON_LIT`], *live but not
//! lamp-hot*, which is the honest reading of the last circuit aboard a
//! ship with nobody on it.
//!
//! # Why there is no violet in here
//!
//! `EERIE` is the game's one *something is wrong* signal — the omen's
//! cast, the suspicious crate's glow, the Guild's hangar. A derelict
//! looks like a plausible place to spend a little of it, and it is not:
//! the omen is a **portent**, something out there taking an interest,
//! and this room's entire thesis is that nothing here is signalling.
//! Nobody is watching. A violet glow would be the wreck talking, and the
//! wreck's characteristic act is silence. The one accent it does carry
//! is the opposite register: rime, in the ice hue the comet wears, where
//! the atmosphere froze on its way out.

use bevy::prelude::Vec3;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The derelict's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: CARGO_RING,
    light: NO_LIGHT,
    decor: &THE_HOLD,
    outfit: Outfit {
        // A hull nobody has repainted, and no running lights at all.
        // That is the whole tell from outside, and
        // `viewport::every_room_kind_is_dressed_for_the_void` holds a
        // future design agent to it.
        plate: palette::PLATE_SHADE,
        lamp: palette::SHADOW,
        lamps: 0,
    },
    dress: &WHAT_IS_LEFT,
};

/// Frost: what the atmosphere turned into on its way out through
/// whatever opened. [`palette::RIME`] is its own role — frost is not
/// comet ice, however alike they land.
const RIME: Coat = Coat::etched(palette::RIME);

/// Cold structure — the role is *etched lines while the function
/// sleeps*, and every function on this ship is asleep.
const STRUCTURE: Coat = Coat::etched(palette::ICON);

/// Radium safety paint: the one warm thing that outlived the crew, and
/// the reason a dark hold has any shape at all.
const RADIUM: Coat = Coat::etched(palette::BRASS);

/// The hold's floor.
///
/// `Stock` stays filled and `Offer` stays hollow — no station may spend
/// that reading — but the field under the salvage is bare hull, because
/// nobody painted it and nobody was going to. The rim is radium, so the
/// bay is findable with no lamp burning anywhere in the room; the
/// threshold is radium too, and it is bright, because the seam is the
/// one part of this arrangement that has been maintained in living
/// memory — by you, five minutes ago.
const TILES: Tiles = Tiles {
    stock: Coat::etched(palette::HULL),
    rim: RADIUM,
    // A wreck declares no `Offer` band (`RoomKind::tile_of`), so nothing
    // is ever painted in this: it is set to the structure's own coat
    // rather than left saying something the room cannot say.
    chalk: STRUCTURE,
    stud: RADIUM,
    sill: STRUCTURE,
};

/// **The cargo ring.** A hoop on a staple, in a socket well: you hook
/// what you marked and haul it through the seam yourself. It is not a
/// counter, because there is no counterparty — the derelict asks nothing
/// and answers nothing, and a fixture with manners would be a fixture
/// with somebody behind it.
const CARGO_RING: Handshake = Handshake {
    plate: Coat::metal(Worn::Socket),
    knob: Shape::Ring,
    knob_coat: RADIUM,
    knob_at: Vec3::new(0.0, -0.04, 0.11),
    knob_half: Vec3::new(0.34, 0.16, 0.11),
    // Longer than the neutral plunger's: a ring on a staple has slack in
    // it, and the slack is the whole feel of hauling rather than filing.
    throw: 0.07,
    // The last circuit aboard, live and meaning nothing to anybody.
    lamp: palette::ICON_LIT,
    trim: &STAPLE,
};

/// The ring's staple and the plate it is through, in the cell's own
/// frame: x and y are fractions of the declared cell, z is metres out of
/// the wall.
const STAPLE: [Fitting; 4] = [
    // The backing plate, bolted on crooked and never straightened.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(0.03, -0.02, 0.035),
        Vec3::new(0.46, 0.34, 0.012),
    ),
    // Two staple legs, one longer than the other.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.30, 0.10, 0.06),
        Vec3::new(0.05, 0.30, 0.03),
    ),
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(0.30, 0.02, 0.06),
        Vec3::new(0.05, 0.22, 0.03),
    ),
    // A stub of chain, still shackled to the plate and going nowhere.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(0.52, -0.44, 0.05),
        Vec3::new(0.04, 0.14, 0.025),
    ),
];

/// **No light.** The pendant is still bolted up there and its reflector
/// is gone: a bare guard hoop with a dead bulb in it, and one hook left
/// of the two that used to hang it.
///
/// `burn: 0.0` spawns no light source at all — the lights-out case the
/// builder already allows (docs/BAY.md, "lights-out is legal"). The
/// colour is recorded anyway, and it is the colour of glass that is not
/// lit, because that is what this fixture is.
const NO_LIGHT: Light = Light {
    color: palette::GLASS,
    burn: 0.0,
    shade: Shape::Ring,
    shade_coat: STRUCTURE,
    glass: Coat::enamel(palette::GLASS),
    cage: &BROKEN_HANGER,
};

/// What is left of the pendant's hanger, off a box one shade across on
/// every side of it: one hook, and the stub of the one that let go.
const BROKEN_HANGER: [Fitting; 3] = [
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-0.86, 0.30, 0.0),
        Vec3::new(0.07, 0.62, 0.07),
    ),
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(0.86, 0.74, 0.0),
        Vec3::new(0.07, 0.20, 0.07),
    ),
    // The hoop hangs crooked off the one hook that held.
    Fitting::new(
        Shape::Ring,
        RADIUM,
        Vec3::new(-0.20, 0.16, 0.0),
        Vec3::new(0.42, 0.10, 0.42),
    ),
];

/// **The hold**, inside: the frames overhead with a plate off them, a
/// torn deck, one whole grab rail and one broken one, the rime, a sconce
/// with no bulb, and the hatch to the rest of the ship, dogged shut.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft
/// — and every number is a fraction of its half-extents, so none of this
/// had to learn that a wreck is five cells by three.
const THE_HOLD: [Fitting; 23] = [
    // The yard's egress marking: a radium line round the edge of the
    // deck and one stripe up the jamb, painted when the ship was built
    // so a crew could find the way out with the power off. It is doing
    // its job. There is nobody left to do it for, and in a hold with no
    // lamp in it, it is most of what the room has left to say about its
    // own shape.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(0.930, -0.965, -0.10),
        Vec3::new(0.035, 0.018, 0.85),
    ),
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(0.0, -0.965, -0.930),
        Vec3::new(0.90, 0.018, 0.035),
    ),
    // The port run stops at the doorway, because a deck line does.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.930, -0.965, -0.30),
        Vec3::new(0.035, 0.018, 0.62),
    ),
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.140, -0.30, 0.950),
        Vec3::new(0.035, 0.66, 0.030),
    ),
    // Overhead frames, unevenly spaced, with the plating gone off them.
    // The gap between the second and third is where a plate used to be.
    rib(-0.78),
    rib(-0.44),
    rib(0.14),
    rib(0.62),
    // The deck, torn open along two edges: the plate is simply not
    // there, and in a room with no light a hole needs no body — only a
    // lit edge to be a hole against.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(0.44, -0.965, -0.12),
        Vec3::new(0.34, 0.02, 0.022),
    ),
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(0.11, -0.965, -0.40),
        Vec3::new(0.022, 0.02, 0.30),
    ),
    // The starboard grab rail, whole: a run and its two stanchions, in
    // the radium the yard painted it with when the ship was new.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(0.945, -0.06, -0.20),
        Vec3::new(0.022, 0.028, 0.70),
    ),
    stanchion(0.945, -0.86),
    stanchion(0.945, 0.46),
    // The port rail, broken: a short piece, and a stub of the rest of it
    // further aft with nothing between them.
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.945, -0.06, -0.68),
        Vec3::new(0.022, 0.028, 0.24),
    ),
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.945, -0.06, 0.06),
        Vec3::new(0.022, 0.028, 0.07),
    ),
    // The rime: a band low on the front wall and a tongue of it creeping
    // out across the deck, where the air went and froze on the way.
    Fitting::new(
        Shape::Slab,
        RIME,
        Vec3::new(0.10, -0.74, -0.955),
        Vec3::new(0.74, 0.11, 0.03),
    ),
    Fitting::new(
        Shape::Slab,
        RIME,
        Vec3::new(0.52, -0.962, -0.70),
        Vec3::new(0.28, 0.016, 0.20),
    ),
    // A wall sconce on the port flank with its bulb gone: the bracket
    // still glows and there is a hole where the light was.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-0.945, 0.30, -0.46),
        Vec3::new(0.022, 0.055, 0.11),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::GLASS),
        Vec3::new(-0.86, 0.22, -0.46),
        Vec3::new(0.045, 0.055, 0.075),
    ),
    // The hatch to the rest of the hull, on the front wall, dogged shut
    // — three dogs and an empty hole where the fourth was. Whoever
    // closed this was the last one to touch it.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-0.52, -0.14, -0.955),
        Vec3::new(0.30, 0.44, 0.028),
    ),
    dog(-0.78, 0.26),
    dog(-0.26, 0.26),
    dog(-0.78, -0.54),
];

/// One overhead frame, at `z` along the hold.
const fn rib(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(0.0, 0.945, z),
        Vec3::new(0.985, 0.045, 0.032),
    )
}

/// One grab-rail stanchion, on the wall at `x`, at `z` along it.
const fn stanchion(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        RADIUM,
        Vec3::new(x, -0.42, z),
        Vec3::new(0.020, 0.34, 0.030),
    )
}

/// One dog on the sealed hatch, at `(x, y)` on the front wall.
const fn dog(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        RADIUM,
        Vec3::new(x, y, -0.92),
        Vec3::new(0.045, 0.055, 0.04),
    )
}

/// **What is left**, outside: a hull with no lamp on it, the frames
/// showing where the plate went, a mast snapped off short with a dead
/// masthead, and the debris that came off it, still keeping station
/// because nothing out here has anywhere else to be.
///
/// Out in the void there is no light at all and no shadow maps, so a
/// plate's own colour is very nearly black and only what glows is seen
/// (`guild::HANGAR_FACE` argues it at length). At every other station
/// that is a constraint. Here it is the whole picture: **a derelict is a
/// black hole in the starfield with a scatter of radium hanging round
/// it**, and you know what has pulled alongside before you open the door
/// because nothing is lit and things are floating off it.
const WHAT_IS_LEFT: [Fitting; 15] = [
    // A plate peeled back off the port cornice and never fastened down
    // again, lying half off the hull it belongs to.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-0.78, 1.09, -0.32),
        Vec3::new(0.34, 0.030, 0.46),
    ),
    // Three frames standing proud of the outboard face where the plate
    // is gone. They are the ribs of the room you are about to walk into,
    // seen from the wrong side.
    frame(-0.62, 0.86),
    frame(-0.16, 0.52),
    frame(0.54, 0.78),
    // The snapped mast: a stub, one cross yard left of two, and a dead
    // masthead. Every station in this game carries a light up there.
    // This one carries the glass it was in.
    Fitting::new(
        Shape::Post,
        RADIUM,
        Vec3::new(-0.34, 1.30, 0.10),
        Vec3::new(0.035, 0.26, 0.035),
    ),
    Fitting::new(
        Shape::Slab,
        RADIUM,
        Vec3::new(-0.34, 1.42, 0.10),
        Vec3::new(0.24, 0.020, 0.020),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::GLASS),
        Vec3::new(-0.34, 1.60, 0.10),
        Vec3::new(0.075, 0.065, 0.075),
    ),
    // A hull seam split open along the port flank: the lit edge of a
    // plate that is no longer fastened to anything.
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-1.09, 0.34, -0.18),
        Vec3::new(0.035, 0.030, 0.62),
    ),
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(-1.09, -0.30, 0.30),
        Vec3::new(0.035, 0.030, 0.24),
    ),
    // Rime on the outside too, streaked across the face and banked
    // against a frame: the only patch of this hull that is a colour
    // rather than an absence, and it is where whatever happened,
    // happened.
    Fitting::new(
        Shape::Slab,
        RIME,
        Vec3::new(0.38, 0.10, -1.06),
        Vec3::new(0.38, 0.055, 0.030),
    ),
    // The debris field. It came off this ship and it is still keeping
    // station with it, because out here nothing has anywhere else to be.
    // Every piece is clear of the shell and well inside `DRESS_REACH`:
    // this is wreckage, not a second station.
    speck(-1.62, 0.86, -1.30, 0.075, false),
    speck(1.44, 1.22, -0.62, 0.055, true),
    speck(0.28, 1.66, 0.40, 0.045, false),
    speck(-1.28, -0.94, -1.44, 0.062, true),
    speck(1.86, 0.24, -1.10, 0.040, false),
];

/// One exposed frame on the outboard face, at `x` across it and `h`
/// long — the middle one is short, because it broke.
const fn frame(x: f32, h: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        STRUCTURE,
        Vec3::new(x, h - 0.78, -1.10),
        Vec3::new(0.032, h, 0.045),
    )
}

/// One piece of the debris field, at `(x, y, z)` off the shell and `r`
/// across. `painted` picks whether this piece came off a part of the
/// hull the yard had marked, which is the only reason any of it is
/// visible at all.
const fn speck(x: f32, y: f32, z: f32, r: f32, painted: bool) -> Fitting {
    Fitting::new(
        Shape::Slab,
        if painted { RADIUM } else { STRUCTURE },
        Vec3::new(x, y, z),
        Vec3::new(r, r, r),
    )
}

#[cfg(test)]
mod tests {
    use super::super::Finish;
    use super::*;

    /// **The derelict's own reading**, held where the lore put it: it
    /// burns nothing, inside or out; everything visible in it is the
    /// lights-out finish rather than a lamp; and it spends none of the
    /// omen's violet, because nothing here is signalling at anybody.
    /// Repaint it freely — a change that quietly retires one of these is
    /// a change that retires the derelict.
    #[test]
    fn a_derelict_burns_nothing_and_signals_nothing() {
        assert_eq!(CHARACTER.outfit.lamps, 0, "a derelict has no lights");
        assert!(
            CHARACTER.light.burn.abs() < f32::EPSILON,
            "a derelict has no lights INSIDE either"
        );
        // Every visible body in here is `Etched` or a plain dark
        // surface: there is no phosphor anywhere, because a phosphor is
        // something being powered and nothing here is.
        for fitting in CHARACTER
            .decor
            .iter()
            .chain(CHARACTER.dress.iter())
            .chain(CHARACTER.light.cage.iter())
            .chain(CHARACTER.handshake.trim.iter())
        {
            assert!(
                !matches!(fitting.coat.finish, Finish::Phosphor(_)),
                "something in the wreck still has power: {fitting:?}"
            );
        }
        // And no violet: the omen's register is a portent's, and a grave
        // is not a portent (see this module's note).
        for color in [
            CHARACTER.tiles.stock.color,
            CHARACTER.tiles.rim.color,
            CHARACTER.handshake.lamp,
            CHARACTER.light.color,
            CHARACTER.outfit.lamp,
        ] {
            assert_ne!(color, palette::EERIE);
            assert_ne!(color, palette::EERIE_BRIGHT);
        }
        // The salvage stands on bare hull, and the fixture is a hoop you
        // haul rather than a machine that decides about you.
        assert_eq!(CHARACTER.tiles.stock.color, palette::HULL);
        assert_eq!(CHARACTER.handshake.knob, Shape::Ring);
    }
}
