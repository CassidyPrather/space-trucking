//! **The gas station's pump bay** — an event room, one of a kind. A
//! forecourt: come alongside, top up, cast off, and the fuel reaches the
//! furnace in a crewman's arms the way everything else in this game
//! travels (docs/ROOMS.md).
//!
//! # The least designed room in the game
//!
//! *Fuel, snacks, flickering sign. Nobody knows who runs it*
//! (`sim::encounter`). The Guild's room is somebody's premises and the
//! parlor is somebody's act; **this is neither, because there is nobody
//! here.** A pump is a machine that does not care whether it is being
//! watched, and the room round it is what is left over after the
//! plumbing was routed: the smallest box in the game, three cells by
//! three, two thirds of its one wall taken by the door you came in by,
//! and the rest of it pipe.
//!
//! So the design rule for this file is a refusal. Nothing in here is
//! arranged. There is no cornice, no reveal, no symmetry and no trim;
//! risers run where a riser has to run, ducts join them along the
//! cornice because that is where a duct goes, and the deck under the big
//! one is stained because it has been dripping for years and there is
//! nobody to mind. The only three objects anybody *chose* are the pump
//! itself, which is a bought machine in its maker's own orange; the
//! snack case, which is nearly empty; and the sign outside, which is the
//! one thing the owner paid for and the one thing that has failed. One
//! valve cap is painted, and it is painted because a hand has to find
//! it, not because anybody wanted a colour in here.
//!
//! # The forecourt light
//!
//! It burns the **whole** caller budget, which no other room here does,
//! and it burns it in the pale luminous green-white a cheap strip
//! fitting throws. A forecourt at three in the morning is not lit
//! for atmosphere, it is lit so a camera nobody watches can see the
//! plates, and the honest reading of that in a box this small is
//! over-lit and shadowless. The fitting is a bare galvanised pan with
//! two tubes flanking it, and one of the tubes is out.
//!
//! # What the sim does here
//!
//! The handshake is `Sim::gas_top_up`: once per encounter, five percent
//! of what remains of the leg, skipped. Twice and the pump says no
//! (`Cue::Reject`). So the fixture is a **coupling**, not a counter —
//! you shove the bell home, the counter runs, and you take your hand
//! back. It is the one fixture in the game whose lamp means *there is
//! still fuel in this for you*, and its hue is the gas canister's own,
//! because that is what comes out of it.
//!
//! A pump bay declares neither a `Stock` band nor an `Offer` band
//! (`RoomKind::tile_of`) — the only calling room in the game with no
//! coloured region at all. Nothing is for sale and nothing is proposed;
//! you are buying a service from a machine. Three of the five tile knobs
//! are therefore never painted, and they are set below to what the deck
//! is actually made of rather than to a colour the room cannot show.

use bevy::prelude::{Color, Vec3};
use space_trucking::sim::Kind;

use super::{Character, Coat, Fitting, Handshake, Light, Outfit, Shape, Tiles, Worn};
use crate::palette;

/// The pump bay's own room.
pub const CHARACTER: Character = Character {
    tiles: TILES,
    handshake: THE_COUPLING,
    light: FORECOURT,
    decor: &PLUMBING,
    outfit: Outfit {
        // Galvanised, unpainted, and **one** running light. Not two,
        // because the other one went years ago and there is nobody to
        // replace it; not none, because the thing is still working.
        plate: palette::RIVET,
        lamp: FUEL,
        lamps: 1,
    },
    dress: &FORECOURT_RIG,
};

/// The gas canister's own orange: what is in the tanks, on everything
/// that carries it.
const FUEL: Color = palette::kind_color(Kind::GasCanister);

/// The tube's own light: the pale luminous green-white of a cheap strip
/// fitting, which is the hue the palette carries as luminous paint and
/// the only cold high-value white it has that is not the warm [`GLINT`]
/// every other pendant in the game burns.
///
/// [`GLINT`]: palette::GLINT
const TUBE: Color = palette::TUBE;

/// Galvanised pipe.
const PIPE: Coat = Coat::metal(Worn::Rivet);
/// The dark inset metal of collars, trays and wells.
const IRON: Coat = Coat::metal(Worn::Socket);

/// The deck.
///
/// A pump bay has no coloured region, so `stock`, `rim` and `chalk` are
/// never painted anywhere in it. What IS drawn is the tread: plain steel
/// studs, because nobody here paints a floor, and a sill in the fuel
/// orange, etched so the kerb still reads with the forecourt lamp out.
/// The one line of paint on the premises is the one that keeps a hose
/// off the seam.
const TILES: Tiles = Tiles {
    stock: PIPE,
    rim: IRON,
    chalk: Coat::etched(palette::ICON),
    stud: PIPE,
    sill: Coat::etched(FUEL),
};

/// **The coupling.** A bell-mouthed fitting on a bought orange machine:
/// you shove it home, the counter runs, and that is the transaction.
/// There is no lever, no stamp and no wheel — nobody is on the other end
/// of this to have manners about.
const THE_COUPLING: Handshake = Handshake {
    // The one painted thing in the room, and it was painted at a
    // factory. A pump is a product; the bay is what is left over.
    plate: Coat::enamel(FUEL),
    knob: Shape::Cone,
    knob_coat: PIPE,
    knob_at: Vec3::new(-0.02, -0.16, 0.13),
    knob_half: Vec3::new(0.30, 0.28, 0.13),
    // A long clunky travel: this is a coupling being shoved home, not a
    // button being pressed.
    throw: 0.10,
    lamp: FUEL,
    trim: &PUMP_FACE,
};

/// The pump's face, in the cell's own frame: x and y are fractions of the
/// declared cell, z is metres out of the wall.
const PUMP_FACE: [Fitting; 6] = [
    // The counter bezel and the counter, which is the only thing in the
    // room keeping track of anything.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(0.0, 0.50, 0.035),
        Vec3::new(0.42, 0.17, 0.015),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::ICON_LIT, 1.8),
        Vec3::new(0.0, 0.50, 0.055),
        Vec3::new(0.34, 0.10, 0.012),
    ),
    // The hose: down off the coupling and away to the port edge of the
    // machine, in rubber that has been out here as long as the rest.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.44, -0.30, 0.11),
        Vec3::new(0.055, 0.26, 0.050),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.60, -0.615, 0.11),
        Vec3::new(0.22, 0.055, 0.050),
    ),
    // Two bolts, because it is bolted to a wall and that is all.
    Fitting::new(
        Shape::Dome,
        PIPE,
        Vec3::new(-0.62, 0.72, 0.04),
        Vec3::new(0.06, 0.06, 0.02),
    ),
    Fitting::new(
        Shape::Dome,
        PIPE,
        Vec3::new(0.62, 0.72, 0.04),
        Vec3::new(0.06, 0.06, 0.02),
    ),
];

/// **The forecourt light.** A bare galvanised pan burning the whole
/// caller budget in the pale green-white of a tube that is technically
/// on. Two more tubes are clipped either side of it and one of them is
/// dead — which is the room's entire opinion about maintenance.
const FORECOURT: Light = Light {
    color: TUBE,
    burn: 1.0,
    shade: Shape::Slab,
    shade_coat: PIPE,
    glass: Coat::phosphor(TUBE, 2.4),
    cage: &TUBES,
};

/// The two flanking tubes and the brackets they clip into, off a box one
/// shade across on every side of the lamp.
const TUBES: [Fitting; 4] = [
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(TUBE, 1.6),
        Vec3::new(-0.72, 0.10, 0.0),
        Vec3::new(0.22, 0.16, 0.97),
    ),
    // The dead one. Glass that is dark and still visibly a lamp.
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::GLASS),
        Vec3::new(0.72, 0.10, 0.0),
        Vec3::new(0.22, 0.16, 0.97),
    ),
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.0, 0.34, -0.76),
        Vec3::new(0.90, 0.14, 0.16),
    ),
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.0, 0.34, 0.76),
        Vec3::new(0.90, 0.14, 0.16),
    ),
];

/// **The plumbing**, which is the room.
///
/// The frame is the room's own box — `+x` starboard, `+y` up, `+z` aft —
/// and every number is a fraction of its half-extents. Two thirds of the
/// aft wall is the door you came in by and the last third is the pump,
/// so everything else lives on the other three walls, the deck and the
/// cornice, exactly where plant goes when nobody is thinking about the
/// room.
const PLUMBING: [Fitting; 24] = [
    // The manifold, crammed into the strip of aft wall above the door
    // because that is the only wall left and the pipe had to go
    // somewhere: a steel plate, one painted valve cap, and the two drops
    // that feed it off the cornice run. Everything in here clears the
    // doorway, which is a threshold and belongs to two rooms.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(0.10, 0.80, 0.955),
        Vec3::new(0.30, 0.17, 0.030),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::enamel(FUEL),
        Vec3::new(-0.02, 0.80, 0.885),
        Vec3::new(0.145, 0.115, 0.060),
    ),
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.50, 0.62, 0.920),
        Vec3::new(0.040, 0.36, 0.040),
    ),
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(-0.20, 0.68, 0.920),
        Vec3::new(0.032, 0.30, 0.032),
    ),
    // Two thin conduits under it, clipped on at whatever height the last
    // pair of hands found convenient — and both stopping clear of the
    // doorway, which is a threshold and belongs to two rooms.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(-0.06, 0.39, 0.888),
        Vec3::new(0.84, 0.026, 0.026),
    ),
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(-0.14, 0.45, 0.925),
        Vec3::new(0.76, 0.022, 0.022),
    ),
    // The duct across the aft wall, over the top of the door: the only
    // thing on the one wall this room has, and it is there because the
    // pipe had to get to the machine, not because the wall wanted it.
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.0, 0.56, 0.900),
        Vec3::new(0.90, 0.075, 0.075),
    ),
    // A junction box on it, hung crooked, with nothing to say.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(-0.46, 0.70, 0.900),
        Vec3::new(0.16, 0.13, 0.085),
    ),
    // The main riser, deck to cornice in the starboard-forward corner,
    // with three flange collars up it. A `Ring` lies flat in the room's
    // own plane, which is what a flange round a standing pipe does.
    Fitting::new(
        Shape::Post,
        PIPE,
        Vec3::new(0.78, 0.03, -0.76),
        Vec3::new(0.115, 0.87, 0.115),
    ),
    collar(0.78, -0.62, -0.76),
    collar(0.78, 0.04, -0.76),
    collar(0.78, 0.66, -0.76),
    // A second, thinner riser in the port-forward corner. It goes into
    // the deck and it does not come back, and nothing in this room says
    // where either of them go.
    Fitting::new(
        Shape::Post,
        PIPE,
        Vec3::new(-0.80, 0.08, -0.78),
        Vec3::new(0.070, 0.86, 0.070),
    ),
    // The ducts that join them, square-section, along the cornice —
    // where a duct goes, at the height a duct goes.
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.0, 0.78, -0.82),
        Vec3::new(0.92, 0.075, 0.075),
    ),
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.86, 0.78, -0.01),
        Vec3::new(0.075, 0.075, 0.735),
    ),
    // The branch that feeds the pump, dropping down the aft-starboard
    // corner behind the machine.
    Fitting::new(
        Shape::Slab,
        PIPE,
        Vec3::new(0.895, 0.16, 0.895),
        Vec3::new(0.052, 0.68, 0.055),
    ),
    // The drip tray, and the stain that says how long the tray has been
    // losing the argument.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(0.72, -0.926, -0.72),
        Vec3::new(0.22, 0.018, 0.18),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::SOOT),
        Vec3::new(0.53, -0.974, -0.60),
        Vec3::new(0.44, 0.012, 0.36),
    ),
    // The one gauge, on the starboard wall: a dial in a steel surround,
    // reading something for nobody.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(0.955, 0.16, -0.12),
        Vec3::new(0.030, 0.16, 0.16),
    ),
    Fitting::new(
        Shape::Dome,
        Coat::phosphor(FUEL, 1.5),
        Vec3::new(0.905, 0.16, -0.12),
        Vec3::new(0.040, 0.115, 0.115),
    ),
    // A hose coiled on the deck, perished, not put away by anybody.
    Fitting::new(
        Shape::Ring,
        Coat::enamel(palette::SOOT),
        Vec3::new(-0.42, -0.930, -0.50),
        Vec3::new(0.30, 0.060, 0.30),
    ),
    // Snacks. The case is the second thing on these premises anybody
    // chose, it is lit from behind, and there is one thing left in it.
    Fitting::new(
        Shape::Slab,
        IRON,
        Vec3::new(-0.955, 0.12, -0.28),
        Vec3::new(0.030, 0.24, 0.19),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(TUBE, 0.7),
        Vec3::new(-0.918, 0.12, -0.28),
        Vec3::new(0.012, 0.20, 0.155),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::kind_color(Kind::RationBricks)),
        Vec3::new(-0.895, -0.02, -0.24),
        Vec3::new(0.020, 0.055, 0.05),
    ),
];

/// One flange collar round a riser, at `(x, y, z)`.
const fn collar(x: f32, y: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Ring,
        IRON,
        Vec3::new(x, y, z),
        Vec3::new(0.165, 0.045, 0.165),
    )
}

/// **The forecourt rig**, outside: a canopy on four legs with a strip
/// light under it, two tanks strapped to the outboard face, and the
/// sign.
///
/// Out in the void there is no light and no shadow maps, so a plate's
/// own colour is very nearly black and only what glows is seen. What
/// that leaves is exactly right for this: **a lit flat roof standing on
/// legs over a black box, and a sign on a pole with a dead band across
/// it.** Nothing else in this game has a canopy, and nothing else has a
/// sign that is half out. You know what has pulled alongside before you
/// open the door, and what it is is a filling station in the middle of
/// nowhere with the lights left on.
const FORECOURT_RIG: [Fitting; 15] = [
    // The canopy, overhanging the shell on every side.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(0.0, 1.44, -0.20),
        Vec3::new(1.32, 0.060, 1.16),
    ),
    // The strip under it: the one lit floor in the void, and the reason
    // a forecourt is a forecourt rather than a shed.
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(palette::ICON_LIT, 1.2),
        Vec3::new(0.0, 1.345, -0.20),
        Vec3::new(1.06, 0.035, 0.92),
    ),
    leg(-1.18, -1.06),
    leg(1.18, -1.06),
    leg(-1.18, 0.58),
    leg(1.18, 0.58),
    // Two tanks on the outboard face, with a collar round each and the
    // pipe that ties them into the canopy.
    tank(-0.46),
    tank(0.46),
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::ICON),
        Vec3::new(-0.46, 0.46, -1.34),
        Vec3::new(0.34, 0.055, 0.34),
    ),
    Fitting::new(
        Shape::Ring,
        Coat::etched(palette::ICON),
        Vec3::new(0.46, 0.46, -1.34),
        Vec3::new(0.34, 0.055, 0.34),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(0.0, 0.60, -1.30),
        Vec3::new(0.54, 0.045, 0.045),
    ),
    // The sign: a lit board bolted across the outboard face over the
    // tanks, in the fuel's own orange, with a dead band across it. The
    // game animates nothing out here, so a sign that flickers is drawn
    // as a sign that is half out — the same information, and it stays
    // true in a screenshot.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(0.0, 0.90, -1.28),
        Vec3::new(1.00, 0.200, 0.045),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::phosphor(FUEL, 2.6),
        Vec3::new(0.0, 0.90, -1.35),
        Vec3::new(0.88, 0.140, 0.022),
    ),
    Fitting::new(
        Shape::Slab,
        Coat::enamel(palette::GLASS),
        Vec3::new(-0.30, 0.94, -1.38),
        Vec3::new(0.38, 0.052, 0.022),
    ),
    // The conduit that feeds it, run up the face by somebody who was not
    // being watched either.
    Fitting::new(
        Shape::Slab,
        Coat::etched(palette::ICON),
        Vec3::new(0.86, 0.42, -1.30),
        Vec3::new(0.035, 0.500, 0.035),
    ),
];

/// One canopy leg, at `(x, z)` round the shell.
const fn leg(x: f32, z: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::ICON),
        Vec3::new(x, 0.28, z),
        Vec3::new(0.050, 1.12, 0.050),
    )
}

/// One fuel tank on the outboard face, at `x` across it.
const fn tank(x: f32) -> Fitting {
    Fitting::new(
        Shape::Post,
        Coat::etched(palette::ICON),
        Vec3::new(x, -0.08, -1.34),
        Vec3::new(0.28, 0.72, 0.28),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The pump's own reading**: it is plumbing, it is over-lit, it
    /// carries the fuel's own hue on everything that carries fuel, and
    /// it is not maintained. Repaint it freely — a change that quietly
    /// retires one of these is a change that retires the forecourt.
    #[test]
    fn nobody_designed_the_pump_bay() {
        // Over-lit, alone among the calling rooms in this directory: a
        // forecourt is lit for a camera, not for a customer.
        assert!(
            (CHARACTER.light.burn - 1.0).abs() < 1e-6,
            "the forecourt stopped being over-lit"
        );
        assert_eq!(CHARACTER.light.color, TUBE);
        // One running light. Not two.
        assert_eq!(CHARACTER.outfit.lamps, 1, "somebody fixed the other one");
        assert_eq!(CHARACTER.outfit.lamp, FUEL);
        assert_eq!(CHARACTER.handshake.lamp, FUEL);
        // The machine is the one painted thing in the room; the room
        // itself is bare metal.
        assert_eq!(CHARACTER.handshake.plate, Coat::enamel(FUEL));
        assert_eq!(CHARACTER.tiles.stud, PIPE, "nobody paints this floor");
        // Plumbing: risers and their flanges, in quantity. A pump bay
        // with three fittings in it is a cupboard.
        let pipes = CHARACTER
            .decor
            .iter()
            .filter(|f| matches!(f.shape, Shape::Post | Shape::Ring))
            .count();
        assert!(pipes >= 5, "the plumbing has been tidied away");
    }
}
