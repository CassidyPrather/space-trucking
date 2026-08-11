//! **Saturn** — the planet the outer-ring roster never mentions, and the
//! ring-barons like it that way (DESIGN.md). The rings are a debris field
//! of a thousand failed hauling companies, ground fine and picked over;
//! salvage is the whole economy, scrap trades like treasure, and every
//! docking clamp on the station was pulled off a repossessed freighter —
//! including, possibly, yours.
//!
//! The canon here is unusually thick for a place nobody admits exists,
//! and all of it points the same way — **this is a breaking yard**:
//!
//! - Saturn's row in `sim::barter::VALUE` pays **six** for scrap alloy,
//!   which is the highest number anywhere in the table. Scrap trades like
//!   treasure, exactly as the lore says. It also pays four and five for
//!   lamps, cabinets, couches and paint: *working fixtures*, the things a
//!   wreck stops having the moment somebody cuts it up.
//! - Two zeros, and a zero is where a kind ENTERS the world. Saturn
//!   produces the **gas canister** (tanks come off hulls) and — alone in
//!   the system — the **bay window**: *"that ring is somebody else's hull
//!   all the way round, and somebody else's hull is where big flat glass
//!   comes from"* (`Kind::BayWindow`, `sim::barter`). The big pane is a
//!   journey rather than a purchase because this is the only place it is
//!   cut. So the room has **panes in racks**, waiting on a buyer, and the
//!   counter has an offcut lit from behind.
//! - Everything Saturn owns came off something else, so **nothing in the
//!   room matches anything else in it**: the counter is a patch of a teal
//!   ship's hull with the original paint still on it, the sill bar came
//!   off a red one, the bolts are three different bolts, and the shell
//!   outside wears four other people's enamel in four unequal patches
//!   (`palette::enamel_color` is that paint — the same four coats a crew
//!   rolls onto its own hull, which is the joke).
//! - The yard-blue frame Saturn paints round everything it cuts is
//!   `Kind::BayWindow`'s own hue, and it is used here exactly as a yard
//!   uses paint: on the RIM, where the lot ends, not as a wash over the
//!   room.
//!
//! **No space elevator, and the refusal is the argument.** An elevator
//! needs a ground to stand on and a reason to reach it. Saturn has a
//! planet nobody lands on and a ring already made of other people's
//! ships — so the line runs *sideways* instead of down: a tow cable out
//! into the debris with three hulks strung along it like beads, hauling
//! in. The ring is the quarry; nothing here is interested in the planet.
//!
//! The handshake is a **tally hopper** cut off something bigger: you drop
//! the chit in, it goes somewhere, a lamp says it arrived. Nobody at
//! Saturn built you a counter — they cut you one.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// Saturn's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: TALLY_HOPPER,
    light: WORKLAMP,
    decor: &THE_YARD,
    outfit: Outfit {
        // Ring sand, and **one** running light where every other station
        // burns two: the second socket is empty, because a working
        // fitting is worth four here and nobody was going to leave one
        // bolted to the outside of a building.
        plate: palette::POI_SATURN,
        lamp: palette::AMBER,
        lamps: 1,
    },
    dress: &THE_BREAKING_LINE,
};

/// The yard's paint.
///
/// `Stock` keeps its filled field and `Offer` its struck line — not a
/// station's to spend — but the field under the goods is **ring sand**,
/// the dust everything out here is ground into, and the band where that
/// paint ends is the **yard blue** Saturn paints round every lot it has
/// marked for cutting. That is the same blue as the bay window's frame,
/// because it is the same paint out of the same tin.
///
/// The threshold is where the mismatch shows first: brass studs off one
/// ship, and a sill bar still wearing the oxide red of another.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::accent::SATURN_RING),
    rim: Coat::enamel(palette::kind_color(Kind::BayWindow)),
    chalk: Coat::etched(palette::POI_SATURN),
    stud: Coat::metal(Worn::Brass),
    sill: Coat::enamel(palette::enamel_color(0)),
};

/// **The tally hopper.** A funnel cut off something bigger, set into a
/// patch of a teal ship's hull, with an offcut of bay glass lit beside it
/// so you can see what the yard is selling while you are paying for it.
/// Three bolts hold the plate on and no two of them match.
const TALLY_HOPPER: Handshake = Handshake {
    // The plate is salvage, and it is wearing its last owner's colour.
    plate: Coat::enamel(palette::enamel_color(1)),
    knob: Shape::Cone,
    knob_coat: Coat::enamel(palette::POI_SATURN),
    knob_at: Vec3::new(0.0, -0.06, 0.11),
    knob_half: Vec3::new(0.34, 0.30, 0.09),
    throw: 0.05,
    lamp: palette::POI_SATURN,
    trim: &HOPPER_WORKS,
};

/// The hopper's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const HOPPER_WORKS: [Fitting; 8] = [
    // The throat behind the funnel, and the lip that catches the chit.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.0, -0.06, 0.03),
        Vec3::new(0.36, 0.32, 0.03),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, -0.44, 0.06),
        Vec3::new(0.40, 0.035, 0.05),
    ),
    // A tacked-on corner of somebody's mustard hull, because the plate
    // they cut was not quite big enough and nobody was going to cut two.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(2)),
        Vec3::new(-0.62, 0.34, 0.02),
        Vec3::new(0.28, 0.42, 0.012),
    ),
    // The sample: an offcut of bay glass in its yard-blue frame, lit from
    // behind. The one thing on the chart only this station cuts.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::BayWindow)),
        Vec3::new(0.62, 0.34, 0.03),
        Vec3::new(0.26, 0.30, 0.015),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::GLINT, 0.9),
        Vec3::new(0.62, 0.34, 0.05),
        Vec3::new(0.19, 0.23, 0.012),
    ),
    // Three bolts, three ships.
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(-0.70, -0.62, 0.04),
        Vec3::new(0.07, 0.07, 0.035),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.70, -0.60, 0.04),
        Vec3::new(0.055, 0.055, 0.03),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.66, 0.66, 0.04),
        Vec3::new(0.06, 0.06, 0.03),
    ),
];

/// **The worklamp.** A pan reflector over a bare bulb in a guard somebody
/// bent out of two different hoops, hung off a hook. It burns seven
/// tenths of the budget, because a yard is lit where the work is and dark
/// everywhere else, and the bright thing at Saturn is the torch.
const WORKLAMP: Light = Light {
    color: palette::GLINT,
    burn: 0.7,
    shade: Shape::Dome,
    shade_coat: Coat::metal(Worn::Plate),
    glass: Coat::phosphor(palette::GLINT, 1.7),
    cage: &BULB_GUARD,
};

/// The guard, measured off a box one shade across on every side of the
/// lamp. Two hoops of two different diameters and two wires: exactly the
/// cage a yard makes out of what a yard has.
const BULB_GUARD: [Fitting; 5] = [
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, -0.50, 0.0),
        Vec3::new(0.92, 0.16, 0.92),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.0, -0.86, 0.0),
        Vec3::new(0.62, 0.14, 0.62),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.80, -0.40, 0.0),
        Vec3::new(0.04, 0.60, 0.04),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(-0.80, -0.40, 0.0),
        Vec3::new(0.04, 0.60, 0.04),
    ),
    // The hook it all hangs off, clasped round the stem above the
    // reflector. It has to be WIDER than the stem to be a hook: it was
    // cut two millimetres narrower, which put it inside the stem, where
    // a brass band is nothing but four faces fighting a grey one.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Brass),
        Vec3::new(0.0, 1.10, 0.0),
        Vec3::new(0.16, 0.45, 0.16),
    ),
];

/// **The yard**, inside: panes racked against the starboard wall waiting
/// on somebody with the freight for one, a stack of cut plate on the
/// deck, a patch riveted over the aft wall in a colour that was never
/// this station's, and a torch bottle stood in the corner with its head
/// still warm.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents.
const THE_YARD: [Fitting; 15] = [
    // Two bay panes in the rack. Four cells of glass in a yard-blue frame
    // is the thing this station exists to cut, and it is standing in the
    // room whether or not it is on the shelf today.
    pane(-0.20),
    pane(0.42),
    glass(-0.20),
    glass(0.42),
    // The rack rail they lean on.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Rivet),
        Vec3::new(0.855, 0.30, 0.11),
        Vec3::new(0.045, 0.04, 0.72),
    ),
    // A stack of cut plate on the deck, three pieces off three hulls,
    // squared up by somebody with a strong opinion about stacking.
    plate(0, -0.86, 0.16),
    plate(3, -0.78, 0.12),
    plate(2, -0.70, 0.08),
    // The patch on the aft wall: a slate-blue rectangle riveted over the
    // yard's own sand, with four heads showing. The wall did not come
    // from here either.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(3)),
        Vec3::new(0.58, 0.46, 0.955),
        Vec3::new(0.30, 0.34, 0.014),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.32, 0.72, 0.94),
        Vec3::new(0.022, 0.045, 0.018),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::metal(Worn::Brass),
        Vec3::new(0.84, 0.20, 0.94),
        Vec3::new(0.022, 0.045, 0.018),
    ),
    // The torch: a bottle in the port-forward corner, its hose coiled on
    // the deck, and the cutting head laid on top still glowing. Whatever
    // is on the racks was on a ship this morning.
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.88, -0.60, -0.62),
        Vec3::new(0.055, 0.40, 0.066),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.62, -0.90, -0.60),
        Vec3::new(0.14, 0.10, 0.17),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 2.4),
        Vec3::new(-0.88, -0.16, -0.62),
        Vec3::new(0.045, 0.045, 0.055),
    ),
    // And the tally board on the port wall: a yard-blue slate with
    // nothing written on it, because this game has no text and a yard
    // this size keeps its books in somebody's head anyway.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::BayWindow)),
        Vec3::new(-0.96, 0.34, -0.10),
        Vec3::new(0.016, 0.26, 0.34),
    ),
];

/// One racked bay pane, at `z` along the starboard wall: the frame.
const fn pane(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::BayWindow)),
        Vec3::new(0.93, -0.16, z),
        Vec3::new(0.03, 0.52, 0.26),
    )
}

/// One racked bay pane's glass, a hair inboard of its frame.
const fn glass(z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::GLINT, 0.45),
        Vec3::new(0.89, -0.16, z),
        Vec3::new(0.02, 0.45, 0.21),
    )
}

/// One sheet of cut plate on the stack: which ship's paint, how high it
/// lies, and how far it overhangs the one below.
const fn plate(paint: u8, y: f32, over: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::enamel_color(paint)),
        Vec3::new(-0.52 + over, y, -0.10 + over),
        Vec3::new(0.26, 0.045, 0.30),
    )
}

/// **The breaking line**, outside: a shell wearing four other people's
/// paint, a gantry with a half-cut hull section hanging in it and a torch
/// working on the seam, and the tow line running out into the ring with
/// the next three hulks strung along it.
///
/// Out here there is **no light to speak of**, so every reading is either
/// etched (the "findable with the lamps sold" floor) or a phosphor — and
/// the torch spark burns at **4.5**, hotter than anything else any
/// station shows outside, because a cutting arc is the brightest thing in
/// this game that is not on fire.
const THE_BREAKING_LINE: [Fitting; 16] = [
    // ---- four hulls, one station ----
    patch(0, -0.48, 0.30, 0.34, 0.42),
    patch(1, 0.22, -0.34, 0.42, 0.30),
    patch(2, 0.72, 0.46, 0.24, 0.24),
    // One over the crown, and one down the starboard flank, so the
    // patchwork reads from above and from alongside as well as head on.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::enamel_color(1)),
        Vec3::new(0.30, 1.04, 0.30),
        Vec3::new(0.40, 0.03, 0.35),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::enamel_color(3)),
        Vec3::new(1.05, 0.05, -0.30),
        Vec3::new(0.03, 0.44, 0.40),
    ),
    // ---- the gantry, and what is hanging in it ----
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.75, 1.26, 0.10),
        Vec3::new(0.06, 0.26, 0.06),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.75, 1.26, -0.55),
        Vec3::new(0.06, 0.26, 0.06),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.75, 1.56, -0.75),
        Vec3::new(0.05, 0.05, 1.40),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(0.75, 0.30, -2.00),
        Vec3::new(0.012, 1.25, 0.012),
    ),
    // The hulk: a hull section off a slate-blue freighter, held in the
    // jaws with its top seam half open. Somebody's ship, this morning.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::enamel_color(3)),
        Vec3::new(0.75, -1.45, -2.00),
        Vec3::new(0.30, 0.30, 0.46),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::EMBER, 1.0),
        Vec3::new(0.75, -1.17, -2.12),
        Vec3::new(0.24, 0.015, 0.10),
    ),
    // The arc itself, working along that seam. One glance and you know
    // what this station does for a living.
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(palette::EMBER, 4.5),
        Vec3::new(0.50, -1.16, -2.12),
        Vec3::new(0.06, 0.06, 0.06),
    ),
    // ---- the tow line, running out into the ring ----
    // Not an elevator and pointedly not aimed at the planet: the ground
    // is out THERE, and it is already made of ships.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::RIVET),
        Vec3::new(1.90, -0.10, -0.35),
        Vec3::new(0.78, 0.015, 0.015),
    ),
    bead(2, 1.45, -0.10, 0.14, 0.12, 0.16),
    bead(0, 2.05, -0.06, 0.11, 0.15, 0.12),
    bead(1, 2.55, -0.12, 0.09, 0.10, 0.14),
];

/// One patch of somebody else's hull, tacked to the outboard face.
const fn patch(paint: u8, x: f32, y: f32, w: f32, h: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::enamel_color(paint)),
        Vec3::new(x, y, -1.05),
        Vec3::new(w, h, 0.03),
    )
}

/// One hulk on the tow line, in the paint it was wearing when it stopped
/// being a ship.
const fn bead(paint: u8, along: f32, up: f32, wide: f32, tall: f32, deep: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::enamel_color(paint)),
        Vec3::new(along, up, -0.35),
        Vec3::new(wide, tall, deep),
    )
}

#[cfg(test)]
mod tests {
    use super::super::Finish;
    use super::*;

    /// Saturn's canon, held where the lore and the value table put it: it
    /// cuts glass (the panes are in the room and the offcut is on the
    /// counter), it wears four other people's paint, and it is cutting
    /// something right now. A change that quietly retires one of these is
    /// a change that retires the yard.
    #[test]
    fn saturn_is_a_breaking_yard_that_cuts_the_big_pane() {
        // The yard blue is on the RIM, where a lot ends — the same tin
        // that frames every bay window it sells.
        assert_eq!(
            CHARACTER.tiles.rim.color,
            palette::kind_color(Kind::BayWindow)
        );
        let panes = CHARACTER
            .decor
            .iter()
            .chain(CHARACTER.handshake.trim.iter())
            .filter(|fitting| fitting.coat.color == palette::kind_color(Kind::BayWindow))
            .count();
        assert!(panes >= 3, "the racks are empty and the yard cuts nothing");
        // Four ships, four coats, and no two fittings agreeing.
        let mut worn = 0;
        for paint in 0_u8..4 {
            let hue = palette::enamel_color(paint);
            if CHARACTER
                .dress
                .iter()
                .any(|fitting| fitting.coat.color == hue)
            {
                worn += 1;
            }
        }
        assert_eq!(worn, 4, "the shell stopped being four other people's");
        // And the torch is lit: an arc hotter than any lamp on the chart.
        let arc = CHARACTER
            .dress
            .iter()
            .any(|fitting| matches!(fitting.coat.finish, Finish::Phosphor(glow) if glow > 4.0));
        assert!(arc, "nobody is cutting anything");
        assert_eq!(CHARACTER.outfit.lamps, 1, "somebody sold the other one");
    }
}
