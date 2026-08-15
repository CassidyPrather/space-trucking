//! **The Umbra Market** — floats in Mercury's shadow and only answers
//! hails while the *caller's* clock reads deep night, which should not be
//! possible and is not explained (DESIGN.md). It bottles midnight and
//! sells it. It pays extra for rat-gnawed goods — "aged in transit,
//! artisanal" — and it prices light at **zero**, because light is a rival
//! product: it fences seized lamps and seized portholes cheap, snuffed,
//! in blackout tins.
//!
//! What the lore says, and what each line turned into:
//!
//! - *They bottle midnight and sell it* (DESIGN.md), and the sim agrees:
//!   `BottledMidnight` is "bottled at the Umbra Market during business
//!   hours only" (`sim::cargo`). So the market's own enamel is the
//!   **colour of its produce** — the floor a good stands on is painted
//!   the exact tone the bottle is, because a shop that sells the dark
//!   paints in it.
//! - *Light is a rival product.* Every lamp column in the market's row of
//!   `sim::barter::VALUE` is a zero, and a zero is where a kind ENTERS
//!   the world: the market fences seized lamps, and prices luminous paint
//!   at nothing so it can shelve the glow **snuffed, in blackout tins**.
//!   That is the room: a shelf of tins over the goods, a case of bottled
//!   midnight open on the deck, and three seized pendants hanging dead
//!   over your head. A shop with a dozen lamps in it and not one of them
//!   burning is the whole character in one look.
//! - *It fences portholes*, because "glass that lets starlight in is a
//!   rival product there" (`sim::barter::VALUE`, the window columns). So
//!   the portholes are stacked on the deck with their panes **blanked** —
//!   the market will sell you a window, and it has already put a lid on
//!   this one.
//! - *Open only at night.* The one thing the exterior says is **shut**:
//!   the outboard face is a shuttered front drawn in cold hairlines, with
//!   a night hatch at the bottom and one cold running light. There is no
//!   maw, no mast, and nothing to read.
//!
//! # The darkness, and where its floor is
//!
//! [`super::Light::burn`] is a fraction of the caller budget and the
//! pendant's reach is the room's, so a station may go as dark as it likes
//! and can never dim anybody else — which makes a near-zero burn the one
//! honest thing this market can do that no other station would. It burns
//! **[`NIGHT_BURN`]**, and the number is argued at the constant: a third
//! of one of the wall lamps it fences, for a whole trading floor.
//!
//! Everything a customer can actually read is therefore read by **leak**:
//! the lid seams of the blackout tins, the one seized lamp with a little
//! light still in it, and the market's own marks, which are struck at
//! `Finish::Etched` — the game's own lights-out floor, "the floor that
//! keeps hardware findable when every lamp aboard is cargo somebody sold"
//! (`poi::Finish`). Every other station treats that floor as a fallback.
//! The Umbra Market lives on it. And the joke the module note asks for
//! lands by itself: what you see the goods by is the lamps in your own
//! hold, which are cargo the market would very much like to buy.

use bevy::prelude::Vec3;
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The Umbra Market's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: SNUFFER,
    light: SNUFFED,
    decor: &NIGHT_SHOP,
    outfit: Outfit {
        // Plate the colour of the bottle, and **one** cold running light
        // rather than the pair every honest hull burns: the market
        // answers hails at night and advertises nothing. It is not dark
        // the way a derelict is dark — a derelict burns nothing at all
        // (`poi::wreck`) — it is dark the way a shop with the shutters
        // down and a light on in the back is dark.
        plate: palette::kind_color(Kind::BottledMidnight),
        lamp: palette::POI_UMBRA,
        lamps: 1,
    },
    dress: &SHUTTERED_FRONT,
};

/// How much of the caller budget the market burns: **a third of one
/// seized wall lamp**, and the number is an argument rather than a taste.
///
/// `pieces::LAMP_LUMENS` is 36,000 — the honest brightness of the very
/// lamps this market fences, snuffed, at a price of zero. The caller
/// budget is 150,000. So the whole trading floor runs on 12,000 lumens,
/// which is a third of one of the lamps it will not sell you lit, spread
/// over a room that would want twelve of them. A market that priced light
/// at zero and then burned it by the room would be lying about its own
/// value table.
///
/// **The floor is legibility, and it is set by eye.** Below about this
/// the goods on the paint stop having colour in them, and a market whose
/// stock you cannot tell apart is not dark, it is broken; above it the
/// room stops making you lean in, which is the whole feeling. Zero is
/// legal — `room::caller_lamp` spawns no light source at all for it, and
/// says so — and zero is not what this station is: the Umbra Market is
/// **open**. Shut with the lights off is a derelict's reading and it
/// belongs to `poi::wreck`.
const NIGHT_BURN: f32 = 0.08;

/// **The night floor.**
///
/// `Stock` keeps its filled field and `Offer` its struck line — that
/// reading is not a station's to spend — but in a room this dark the
/// *marks* have to carry, so the market's are self-lit and its fields are
/// not. The paint under the goods is the tone of the bottle it fills; the
/// band where that paint ends is a cold periwinkle hairline, the one
/// colour the market allows itself; and the line struck round a proposal
/// is drawn in **luminous paint**, which is the only thing here that
/// glows on purpose, because the market has more of it than it can sell
/// and thinks nothing of putting it on the floor.
///
/// The threshold follows the same rule: black studs you would never find
/// with a lamp, and a sill lit in the same snuffed green — cross here,
/// and mind the step.
const TILES: Tiles = Tiles {
    stock: Coat::enamel(palette::kind_color(Kind::BottledMidnight)),
    rim: Coat::etched(palette::POI_UMBRA),
    chalk: Coat::etched(palette::kind_color(Kind::LuminousPaint)),
    stud: Coat::metal(Worn::Socket),
    sill: Coat::etched(palette::kind_color(Kind::LuminousPaint)),
};

/// **The snuffer.** You do not shake hands at the Umbra Market and you do
/// not get stamped: you put a light out.
///
/// A black cap on a black plate, over a pinhole with the market's own
/// luminous green behind it. Working it drives the cap down the length of
/// the plate's own depth and the pinhole goes out — the one moment in the
/// game where committing a deal makes the room *darker*. It throws
/// further than the Guild's press and further than the neutral plunger,
/// because a snuffer has to reach the wick.
const SNUFFER: Handshake = Handshake {
    plate: Coat::metal(Worn::Socket),
    // A cone, mouth down: the shape a candle snuffer has had for four
    // hundred years, and nothing like the Guild's brass dome.
    knob: Shape::Cone,
    knob_coat: Coat::metal(Worn::Socket),
    knob_at: Vec3::new(0.0, 0.10, 0.10),
    knob_half: Vec3::new(0.22, 0.22, 0.05),
    throw: 0.07,
    lamp: palette::kind_color(Kind::LuminousPaint),
    trim: &SNUFFER_WORKS,
};

/// The snuffer's own hardware, in its cell's frame: x and y are fractions
/// of the declared cell, z is metres out of the wall.
const SNUFFER_WORKS: [Fitting; 5] = [
    // The light the cap does not quite cover: a ring of it escaping round
    // the snuffer's own rim, which is the one bright thing on the fixture
    // and the thing that goes out when a deal is struck. It is a ring
    // rather than a wick because the cap stands in front of the wick —
    // what a customer can see of a covered light is its edge.
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 2.2),
        Vec3::new(0.0, 0.10, 0.045),
        Vec3::new(0.28, 0.022, 0.28),
    ),
    // Its collar, outboard of the light, so the fixture reads as a socket
    // with something burning in it rather than as a decal on the plate.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::PlateShade),
        Vec3::new(0.0, 0.10, 0.038),
        Vec3::new(0.35, 0.040, 0.35),
    ),
    // The counter itself: a narrow black ledge under the fixture, worn
    // smooth. Goods change hands over this and nobody has ever seen it.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::PlateShade),
        Vec3::new(0.0, -0.44, 0.085),
        Vec3::new(0.52, 0.035, 0.075),
    ),
    // Two blackout tins standing on it, lids on. The market's produce,
    // where a counter at any other station would keep a bell or a book.
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(-0.34, -0.32, 0.085),
        Vec3::new(0.07, 0.09, 0.07),
    ),
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(0.34, -0.32, 0.085),
        Vec3::new(0.07, 0.09, 0.07),
    ),
];

/// **The snuffed pendant.** A blackout tin hung where a shop would hang a
/// lamp: a black drum, a lid, and a seam of luminous green that got out
/// round the rim because a lid is a lid and not a weld.
///
/// The shade is a **drum**, not the neutral cone — the market did not buy
/// a light fitting and modify it, it hung one of its own tins up there —
/// and the glass under it is the leak rather than a bulb. It burns
/// [`NIGHT_BURN`], which is the argument this whole module is built on.
const SNUFFED: Light = Light {
    color: palette::POI_UMBRA,
    burn: NIGHT_BURN,
    shade: Shape::Post,
    shade_coat: Coat::metal(Worn::Socket),
    glass: Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 0.10),
    cage: &TIN_LID,
};

/// The tin's lid and its clamp, measured off a box one shade across on
/// every side of the lamp. A lid on a lamp is the market's entire
/// argument about light, bolted over the fitting that would otherwise
/// make its point for it.
const TIN_LID: [Fitting; 3] = [
    // The lid, sitting proud of the drum.
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(0.0, 0.62, 0.0),
        Vec3::new(0.98, 0.16, 0.98),
    ),
    // Its clamp ring, and the hairline of green that got out under it.
    Fitting::new(
        Shape::Ring,
        Coat::metal(Worn::PlateShade),
        Vec3::new(0.0, 0.42, 0.0),
        Vec3::new(0.97, 0.10, 0.97),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 1.1),
        Vec3::new(0.0, 0.30, 0.0),
        Vec3::new(0.95, 0.045, 0.95),
    ),
];

/// **The night shop**, inside the room: the tin shelf over the goods, a
/// case of midnight open on the deck, a stack of blanked portholes, three
/// seized pendants hanging dead overhead, and one luminous line along the
/// foot of the goods wall so a customer can find it.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents, so none of this
/// knows how big a trade room is. Nothing stands in the doorway, which at
/// a `Trade` room is the port half of the aft wall, and nothing stands in
/// the handshake's own column.
///
/// The tins hang from a **rail across the top course** of the goods wall,
/// which is where the Guild hangs its bars and for the same reason: it is
/// the one line across a calling room's aft face that a customer looks
/// straight at. A solid shelf would be a station standing in front of its
/// own stock, so this is a rail with gaps — you see the market's goods
/// between the market's tins, which is exactly the relationship.
const NIGHT_SHOP: [Fitting; 30] = [
    // ---- the tin rail, a cell in front of the goods ----
    // Five blackout tins strung up like lanterns over the aisle, high
    // enough that a crew member walks under them, every one of them shut,
    // every one of them leaking a hairline at the rim. THIS is the
    // market's light: everything else in the room is seen by what its
    // goods let out, and what they let out is the glow it buys at zero
    // specifically so that nobody sells it lit.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::PlateShade),
        Vec3::new(0.36, 0.765, 0.640),
        Vec3::new(0.60, 0.014, 0.012),
    ),
    tin(-0.16),
    tin(0.10),
    tin(0.36),
    tin(0.62),
    tin(0.88),
    seam(-0.16),
    seam(0.10),
    seam(0.36),
    seam(0.62),
    seam(0.88),
    // ---- the case of midnight, on the deck to port ----
    // A case broken open on the floor rather than a rack on the wall:
    // the port flank of a `Trade` room is the one wall a crew hangs its
    // own things on (`room::preset`, "flank"), and a market that filled
    // it would be a market arguing with its customers about shelf space.
    // Four bottles and a gap where a fifth was — the Umbra has had
    // somebody in tonight.
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::PlateShade),
        Vec3::new(-0.72, -0.975, 0.12),
        Vec3::new(0.16, 0.020, 0.40),
    ),
    bottle(-0.20),
    bottle(0.06),
    bottle(0.32),
    bottle(0.50),
    // ---- the fenced portholes, on the deck to starboard ----
    // Somebody's windows, stacked flat with their panes blanked. The
    // market sells glass and has already put a lid on this lot; a
    // porthole that let starlight in would be a competitor lying on the
    // floor of the shop.
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::kind_color(Kind::Porthole)),
        Vec3::new(0.76, -0.955, 0.30),
        Vec3::new(0.15, 0.030, 0.15),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.76, -0.955, 0.30),
        Vec3::new(0.11, 0.014, 0.11),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::kind_color(Kind::Porthole)),
        Vec3::new(0.72, -0.895, 0.26),
        Vec3::new(0.15, 0.030, 0.15),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::Socket),
        Vec3::new(0.72, -0.895, 0.26),
        Vec3::new(0.11, 0.014, 0.11),
    ),
    // ---- the seized lamps, hanging ----
    // Three pendants that came off ships, hung up where a shopkeeper
    // would hang stock. Two are dead glass. The third still has a little
    // light in it, and the market has not noticed yet.
    lamp(-0.55, 0.72, -0.30),
    glass(-0.55, 0.72, -0.30),
    stem(-0.55, -0.30),
    lamp(0.30, 0.80, -0.30),
    Fitting::new(
        Shape::Post,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 0.9),
        Vec3::new(0.30, 0.670, -0.30),
        Vec3::new(0.070, 0.008, 0.070),
    ),
    stem(0.30, -0.30),
    lamp(0.62, 0.66, 0.30),
    glass(0.62, 0.66, 0.30),
    stem(0.62, 0.30),
    // ---- the one line on the floor ----
    // Luminous paint struck across the deck under the tin rail, starboard
    // of the doorway and stopping well short of it — off the goods' own
    // band, like everything else the market owns. The market has more of
    // this than it can sell and no reason at all to be tidy with it.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 0.5),
        Vec3::new(0.33, -0.965, 0.640),
        Vec3::new(0.62, 0.008, 0.020),
    ),
];

/// One blackout tin hanging off the rail, at `x` along it.
const fn tin(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::metal(Worn::Socket),
        Vec3::new(x, 0.600, 0.640),
        Vec3::new(0.052, 0.100, 0.062),
    )
}

/// The hairline of snuffed glow that got out under a tin's rim.
const fn seam(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 1.8),
        Vec3::new(x, 0.450, 0.640),
        Vec3::new(0.058, 0.008, 0.070),
    )
}

/// One bottle of midnight standing in the case, at `z` along it.
const fn bottle(z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::kind_color(Kind::BottledMidnight)),
        Vec3::new(-0.72, -0.80, z),
        Vec3::new(0.045, 0.155, 0.045),
    )
}

/// A seized pendant's shade, hanging where the market put it.
const fn lamp(x: f32, y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Cone,
        Coat::metal(Worn::PlateShade),
        Vec3::new(x, y, z),
        Vec3::new(0.10, 0.070, 0.10),
    )
}

/// Its glass, dead. The market sells lamps; it does not run them.
const fn glass(x: f32, y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::enamel(palette::GLASS),
        Vec3::new(x, y - 0.130, z),
        Vec3::new(0.070, 0.008, 0.070),
    )
}

/// The stem a seized pendant hangs from, up to the deckhead.
const fn stem(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::metal(Worn::PlateShade),
        Vec3::new(x, 0.90, z),
        Vec3::new(0.008, 0.100, 0.008),
    )
}

/// **The shuttered front**, outside: the one face the void sees, drawn as
/// a shop with the shutters down.
///
/// Out here there is no light at all — the void carries none and the art
/// direction runs no shadow maps — so a plate's own colour is very nearly
/// black and only what glows is seen. Every other station answers that by
/// putting something *lit* on its hull. The Umbra Market answers it by
/// refusing to: what it shows the void is an **outline**, in cold
/// hairlines a hand's width wide, with a night hatch at the bottom and
/// three tin bands stacked over the roof. A station rendered as a
/// wireframe because it will not be lit is a station you have already
/// understood before you dock.
const SHUTTERED_FRONT: [Fitting; 15] = [
    // The shopfront: four hairlines round the outboard face.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(0.0, 0.56, -1.05),
        Vec3::new(0.74, 0.028, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(0.0, -0.46, -1.05),
        Vec3::new(0.74, 0.028, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(-0.74, 0.05, -1.05),
        Vec3::new(0.028, 0.54, 0.020),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(0.74, 0.05, -1.05),
        Vec3::new(0.028, 0.54, 0.020),
    ),
    // Three slats across it. The market is open; the front is not.
    slat(0.36),
    slat(0.05),
    slat(-0.26),
    // The night hatch, at the foot of the shutter: a slot with a
    // luminous lip. Business is done through this, at an hour your own
    // clock has to agree to.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::kind_color(Kind::LuminousPaint), 0.9),
        Vec3::new(0.0, -0.62, -1.07),
        Vec3::new(0.19, 0.022, 0.030),
    ),
    // The bottling plant on the roof: three tin bands stacked, and no
    // body between them worth lighting. What you see of the Umbra Market
    // from a distance is a few rings and a rectangle.
    band(1.14),
    band(1.36),
    band(1.58),
    stave(0.14),
    stave(0.70),
    // Two corner hairlines, so the box has edges. Without these the
    // shopfront reads as a sign hanging in the void rather than as the
    // front of something.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(-1.05, 0.0, -1.05),
        Vec3::new(0.026, 0.92, 0.026),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(1.05, 0.0, -1.05),
        Vec3::new(0.026, 0.92, 0.026),
    ),
];

/// One shutter slat across the shopfront, at height `y`.
const fn slat(y: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(0.0, y, -1.04),
        Vec3::new(0.70, 0.016, 0.014),
    )
}

/// One band of the bottling plant's tin stack, at height `y` over the
/// roof.
const fn band(y: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(0.42, y, 0.18),
        Vec3::new(0.30, 0.050, 0.30),
    )
}

/// One stave up the tin stack, at `x` across it. Without these the bands
/// are three hoops adrift in the void; with them the roof has a drum on
/// it, which is the difference between hardware and debris.
const fn stave(x: f32) -> Fitting {
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::POI_UMBRA),
        Vec3::new(x, 1.36, 0.18),
        Vec3::new(0.020, 0.280, 0.020),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The market sells the dark, so the market is dark.** The three
    /// readings the lore actually pins: it burns next to nothing, its
    /// paint is the colour of its own produce, and its light comes out of
    /// its goods rather than out of a lamp. Retune it freely — but a
    /// change that lights this room has repealed the Umbra Market.
    #[test]
    fn the_umbra_market_is_the_dark_it_sells() {
        // Under a third of one of the wall lamps it fences (36,000 of a
        // 150,000 budget is 0.24), which is the argument at
        // [`NIGHT_BURN`]. The ceiling here is that argument with slack in
        // it, not the number itself: retune the room, keep the claim.
        const {
            assert!(
                CHARACTER.light.burn <= 0.10,
                "a market that sells darkness burns too much of the caller budget"
            );
        }
        const {
            assert!(
                CHARACTER.light.burn > 0.0,
                "the market is open: shut is a derelict's reading, not a shop's"
            );
        }
        assert_eq!(
            CHARACTER.tiles.stock.color,
            palette::kind_color(Kind::BottledMidnight),
            "the paint under the goods is the colour of the bottle"
        );
        // What lights the room is what the goods leak. Count the self-lit
        // fittings: the tins' seams, the one live pendant, the floor
        // line. If this ever falls to nothing, the room went black rather
        // than dark, and dark is the design.
        let leaks = CHARACTER
            .decor
            .iter()
            .filter(|fitting| {
                matches!(fitting.coat.finish, super::super::Finish::Phosphor(glow) if glow > 0.0)
            })
            .count();
        assert!(
            leaks >= 5,
            "nothing in the shop leaks; the room is unreadable"
        );
        // Form, not hue: the handshake is a snuffer, and it is nobody
        // else's silhouette.
        assert_eq!(CHARACTER.handshake.knob, Shape::Cone);
        const {
            assert!(CHARACTER.handshake.throw > super::super::NEUTRAL.handshake.throw);
        }
        // And the shutters are down: nothing outside is a lit face.
        assert_eq!(CHARACTER.outfit.lamps, 1, "one cold lamp, not a shopfront");
    }
}
