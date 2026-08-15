//! **Neptune** — the furthest of the outer ring's three, and the one the
//! lore has said least about, which is an invitation.
//!
//! What the economy says, since nothing else does:
//!
//! - Neptune's row in `sim::barter::VALUE` carries one zero, and a zero
//!   is where a kind ENTERS the world: **brine pearls**. Nobody makes a
//!   pearl. A pearl is *fetched*, out of water, by something that goes
//!   down and comes back up. So Neptune is not a works and not a yard —
//!   it is a **fishery**, and the room is a dive lock: a moon pool in the
//!   deck with a wire running out of it into the ceiling, trays of the
//!   catch along the port wall, and a lamp that would not embarrass
//!   a trawler.
//! - It pays four for ration bricks and five for bottled midnight and
//!   one for cryo cores. A crew working a deep ocean buys food and dark
//!   and has quite enough cold of its own, thank you.
//! - The dive vocabulary decides everything else. The paint is the deep:
//!   a **midnight field** with a pale **pearl** band at its edge, which
//!   is Uranus's chill store read the other way up — dark on pale
//!   against pale on dark — so the two blue stations on the chart cannot
//!   be confused even in one glance and one hue.
//!
//! **The space elevator, at Neptune.** An ice giant has no ground either,
//! so the ribbon question gets the same honest answer Jupiter's does and
//! a completely different picture: what runs off this shell is a **wire**,
//! not a ribbon, it carries no passengers, and its far end is not
//! anchored to anything — it hangs a sounding weight into the dark with
//! lamp beads strung up it, so the crew can see how much of it is out.
//! The other end is what the station is proud of: a **bathysphere** in a
//! gallows frame on the crown, parked, dripping, its one port lit. When
//! it goes down, the wire goes with it. That is a lift to somewhere; it
//! simply is not a lift to a surface, because there is not one.
//!
//! The handshake is a **glass float** in a six-bolt ring — a scuttle out
//! of a pressure hull, lit from behind, that you press with the flat of
//! your hand. It is deliberately the Guild's opposite: the same round
//! silhouette, and where the Guild's is a brass die that stamps you,
//! this one is dark glass with a light behind it that you push.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Neptune's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: GLASS_FLOAT,
    light: DIVE_HALO,
    decor: &THE_DIVE_LOCK,
    outfit: Outfit {
        plate: palette::POI_NEPTUNE,
        // Running lights in the catch's own pale blue, which is the only
        // colour anybody out here has ever seen come back up.
        lamp: palette::kind_color(Kind::BrinePearls),
        lamps: 2,
    },
    dress: &THE_DREDGE,
};

/// The dive lock's paint.
///
/// `Stock` keeps its filled field and `Offer` its struck line — not a
/// station's to spend — but the field under the goods is the **deep**
/// itself, in the hue this game already bottles and sells as midnight,
/// and the band where that paint ends is the pale of the catch. Dark
/// field, pale rim: the exact inverse of Uranus's chill store, which is
/// how the chart's two blue stations stay two places.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::kind_color(Kind::BottledMidnight)),
    rim: Coat::enamel(palette::kind_color(Kind::BrinePearls)),
    chalk: Coat::etched(palette::kind_color(Kind::BrinePearls)),
    stud: Coat::metal(Worn::Rivet),
    sill: Coat::etched(palette::POI_NEPTUNE),
};

/// **The glass float.** A dark round port in a six-bolt ring, lit from
/// behind, with a depth plate under it. You press the glass; something
/// below hears about it.
const GLASS_FLOAT: Handshake = Handshake {
    plate: Coat::metal(Worn::Socket),
    knob: Shape::Dome,
    knob_coat: Coat::phosphor(palette::kind_color(Kind::BrinePearls), 2.2),
    knob_at: Vec3::new(0.0, 0.0, 0.12),
    knob_half: Vec3::new(0.38, 0.38, 0.085),
    throw: 0.03,
    lamp: palette::POI_NEPTUNE,
    trim: &FLOAT_WORKS,
};

/// The float's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall. Six bolts on a
/// circle, because that is how a hull keeps water on the other side of a
/// piece of glass, and one plate under them.
const FLOAT_WORKS: [Fitting; 8] = [
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.0, 0.0, 0.03),
        Vec3::new(0.50, 0.50, 0.03),
    ),
    bolt(0.56, 0.0),
    bolt(0.28, 0.48),
    bolt(-0.28, 0.48),
    bolt(-0.56, 0.0),
    bolt(-0.28, -0.48),
    bolt(0.28, -0.48),
    // The depth plate: a lit sliver under the glass, reading something
    // nobody in this game will ever be told the units of.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::kind_color(Kind::BrinePearls), 2.0),
        Vec3::new(0.0, -0.70, 0.05),
        Vec3::new(0.30, 0.05, 0.012),
    ),
];

/// One bolt of the float's ring, at `(x, y)` on the cell.
const fn bolt(x: f32, y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(x, y, 0.05),
        Vec3::new(0.075, 0.075, 0.04),
    )
}

/// **The dive halo.** A hoop fixture rather than a shade: the light comes
/// off a ring hung flat, the way a lamp is rigged where it has to light
/// what is directly under it and nothing else. It burns three fifths of
/// the budget in the catch's own pale blue — a dive lock is dim on
/// purpose, because eyes that have been down do not come back up ready
/// for a shop.
const DIVE_HALO: Light = Light {
    color: palette::kind_color(Kind::BrinePearls),
    burn: 0.6,
    shade: Shape::Ring,
    shade_coat: Coat::metal(Worn::Rivet),
    glass: Coat::phosphor(palette::POI_NEPTUNE, 1.8),
    cage: &HALO_RIG,
};

/// The halo's rig, measured off a box one shade across on every side of
/// the lamp: a second hoop under it, and two weights on short falls to
/// keep the thing hanging level in a room somebody keeps walking through.
const HALO_RIG: [Fitting; 4] = [
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, -0.75, 0.0),
        Vec3::new(0.88, 0.20, 0.88),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.82, -0.40, 0.0),
        Vec3::new(0.045, 0.42, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.82, -0.40, 0.0),
        Vec3::new(0.045, 0.42, 0.045),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Socket),
        Vec3::new(0.0, -0.84, 0.0),
        Vec3::new(0.16, 0.16, 0.16),
    ),
];

/// **The dive lock**, inside: the moon pool in the deck with the wire
/// standing out of it, the catch in trays along the port wall, and
/// the string of depth lamps over the aft cornice that came off the last
/// bell but still work.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. The pool sits on
/// the plain deck between the two trading bands on purpose: it is
/// dressing, not a berth, and a hole in the floor that ate a crate would
/// be a station taking something the sim owns.
const THE_DIVE_LOCK: [Fitting; 17] = [
    // The pool: a collar hoop set into the deck, and the water in it —
    // which out here is the same near-black as everything else that goes
    // down a long way, with a shimmer on it.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Plate),
        Vec3::new(0.34, -0.93, 0.20),
        Vec3::new(0.28, 0.07, 0.33),
    ),
    Fitting::new(
        Shape::Post,
        Coat::phosphor(palette::kind_color(Kind::BottledMidnight), 1.6),
        Vec3::new(0.34, -0.96, 0.20),
        Vec3::new(0.23, 0.035, 0.28),
    ),
    // Three lamps round its rim, aimed in. Nobody works a hole in a
    // floor without lighting the hole.
    rim_lamp(0.07, 0.20),
    rim_lamp(0.61, 0.20),
    rim_lamp(0.34, -0.06),
    // The wire, standing out of the pool and running into the ceiling
    // block. This is the same wire that hangs off the shell outside; it
    // simply goes through the room on its way, which is what makes the
    // station one place instead of two pictures.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.34, 0.0, 0.20),
        Vec3::new(0.018, 0.90, 0.018),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(0.34, 0.90, 0.20),
        Vec3::new(0.12, 0.09, 0.14),
    ),
    inboard_bead(-0.42),
    inboard_bead(0.10),
    inboard_bead(0.62),
    // The catch: two shallow trays on the port wall with the day's
    // pearls in them, which is the produce sitting in the room.
    tray(-0.30),
    tray(0.30),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::kind_color(Kind::BrinePearls)),
        Vec3::new(-0.88, -0.20, -0.30),
        Vec3::new(0.035, 0.07, 0.042),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(palette::kind_color(Kind::BrinePearls)),
        Vec3::new(-0.88, -0.20, 0.34),
        Vec3::new(0.03, 0.06, 0.036),
    ),
    // The depth lamps: a string of three off a bell that is not coming
    // back, still burning, because the bulbs were the expensive part.
    // They hang a cell in from the aft cornice, off the goods' band.
    depth_lamp(-0.10),
    depth_lamp(0.32),
    depth_lamp(0.74),
];

/// One lamp on the moon pool's rim, at `(x, z)` on the deck.
const fn rim_lamp(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::BrinePearls), 2.4),
        Vec3::new(x, -0.88, z),
        Vec3::new(0.035, 0.05, 0.042),
    )
}

/// One lamp bead on the inboard length of wire, at height `y`. The same
/// beads the standing part outside carries, which is how a crew reads
/// that the two are one wire and not two decorations.
const fn inboard_bead(y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::BrinePearls), 2.4),
        Vec3::new(0.34, y, 0.20),
        Vec3::new(0.03, 0.042, 0.036),
    )
}

/// One tray of the catch on the port wall, at `z` along it.
const fn tray(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Plate),
        Vec3::new(-0.92, -0.28, z),
        Vec3::new(0.045, 0.03, 0.28),
    )
}

/// One depth lamp on the deckhead, at `x` across it — a cell in off the
/// aft cornice, so the string hangs clear of the goods' own air.
const fn depth_lamp(x: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::POI_NEPTUNE, 2.6),
        Vec3::new(x, 0.84, 0.650),
        Vec3::new(0.04, 0.055, 0.045),
    )
}

/// **The dredge**, outside: the bell parked in its gallows on the crown,
/// the winch drum beside it, and the standing part of the wire running
/// down past the outboard face with its lamp beads and its sounding
/// weight.
///
/// Out here there is **no light to speak of**, so every reading is either
/// etched (the "findable with the lamps sold" floor) or a phosphor — and
/// the beads burn at indicator brightness rather than as floodlights,
/// because a line you are reading depth off is a line with small lights
/// on it at known intervals.
const THE_DREDGE: [Fitting; 15] = [
    // ---- the gallows, and the bell in it ----
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.45, 1.30, 0.0),
        Vec3::new(0.055, 0.30, 0.055),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.45, 1.30, 0.0),
        Vec3::new(0.055, 0.30, 0.055),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.0, 1.62, 0.0),
        Vec3::new(0.58, 0.05, 0.05),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.0, 1.50, 0.0),
        Vec3::new(0.016, 0.07, 0.016),
    ),
    // The bell itself: a pressure sphere in the catch's own pale, with
    // its collar and its one lit port. Parked, and recently wet.
    Fitting::new(
        Shape::Dome,
        Coat::etched(palette::kind_color(Kind::BrinePearls)),
        Vec3::new(0.0, 1.24, 0.0),
        Vec3::new(0.30, 0.23, 0.30),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.0, 1.44, 0.0),
        Vec3::new(0.20, 0.04, 0.20),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::POI_NEPTUNE, 2.8),
        Vec3::new(0.0, 1.20, -0.32),
        Vec3::new(0.11, 0.11, 0.11),
    ),
    // The winch drum on the crown beside it, with the turns still on.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.80, 1.10, 0.35),
        Vec3::new(0.24, 0.10, 0.14),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::BrinePearls)),
        Vec3::new(0.80, 1.13, 0.35),
        Vec3::new(0.19, 0.11, 0.10),
    ),
    // ---- the standing part, going down ----
    // A wire, not a ribbon; no anchor at the bottom, because there is no
    // bottom to anchor to. The beads are how you read how much is out.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.62, -0.80, -1.12),
        Vec3::new(0.012, 1.30, 0.012),
    ),
    bead(-0.35),
    bead(-1.05),
    bead(-1.75),
    Fitting::new(
        Shape::Cone,
        Coat::etched(palette::RIVET),
        Vec3::new(-0.62, -2.24, -1.12),
        Vec3::new(0.10, 0.14, 0.10),
    ),
    // And the lock hatch in the outboard face, lit the way a flooded
    // compartment is lit: from inside, badly.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::kind_color(Kind::BrinePearls)),
        Vec3::new(0.40, -0.24, -1.06),
        Vec3::new(0.30, 0.30, 0.04),
    ),
];

/// One lamp bead on the standing part of the wire, at height `y`.
const fn bead(y: f32) -> Fitting {
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::kind_color(Kind::BrinePearls), 2.6),
        Vec3::new(-0.62, y, -1.12),
        Vec3::new(0.055, 0.055, 0.055),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Neptune's own reading: the produce is fetched rather than made
    /// (a hole in the floor, a wire through the room, a bell on the
    /// roof), the paint is the deep with the catch round its edge, and
    /// the whole station is one machine seen from inside and out — the
    /// wire in the room is the wire on the shell.
    #[test]
    fn neptune_is_a_fishery_with_one_wire_through_it() {
        assert_eq!(
            CHARACTER.tiles.stock.color,
            palette::kind_color(Kind::BottledMidnight)
        );
        assert_eq!(
            CHARACTER.tiles.rim.color,
            palette::kind_color(Kind::BrinePearls)
        );
        // The pool is a hoop in the deck, and it is lit.
        let pool = CHARACTER
            .decor
            .iter()
            .any(|fitting| fitting.shape == Shape::Ring && fitting.at.y < -0.8);
        assert!(pool, "the moon pool has been floored over");
        // The bell is UP: parked in its gallows, clear of the shell.
        let bell = CHARACTER.dress.iter().any(|fitting| {
            fitting.shape == Shape::Dome && fitting.at.y > 1.0 && fitting.half.x > 0.2
        });
        assert!(bell, "the bell went down and stayed there");
        // And the wire runs a long way below, with lights on it.
        let beads = CHARACTER
            .dress
            .iter()
            .filter(|fitting| fitting.at.y < -0.2 && fitting.shape == Shape::Dome)
            .count();
        assert!(beads >= 3, "nobody can read the depth any more");
        // A dive lock is dim on purpose.
        const { assert!(CHARACTER.light.burn < super::super::NEUTRAL.light.burn) }
    }
}
