//! **Station character: one module per place, so no two of them look the
//! same.**
//!
//! Every trading POI used to get the identical room — the same violet-grey
//! enamel on its goods, the same brass plunger for a handshake, the same
//! white pendant overhead, the same plain plate outside. Twelve stations,
//! one room. This directory is where that stops: a **registry** keyed by
//! [`Host`], with one module per station holding one `const CHARACTER`,
//! and a [`NEUTRAL`] default that reproduces exactly what every room wears
//! today. A station nobody has filled in yet keeps the neutral form; a
//! station somebody has filled in wears its own.
//!
//! # If you are the design agent for one station, this is your whole brief
//!
//! **You own exactly one file**: `poi/<your station>.rs`. You edit the
//! `CHARACTER` constant in it, and nothing else in this directory. You do
//! not touch `mod.rs`, you do not touch another station's module, and you
//! do not touch `room.rs` or `viewport.rs` — those build whatever your
//! constant says. That is the whole point of the seam: twelve agents can
//! work at once and never meet.
//!
//! Two files outside the directory you MAY touch, both by **appending**:
//!
//! - `palette.rs`, in its "Station character roles" section at the end,
//!   if your station genuinely needs a tone the palette does not carry.
//!   Compose from the existing roles first — most stations can.
//! - your station's own test, in your own module.
//!
//! ## What you may change
//!
//! Look, light, form, and flavor:
//!
//! | Knob | What it decides |
//! | --- | --- |
//! | [`Character::tiles`] | the enamel and treatment each colored tile class wears |
//! | [`Character::handshake`] | the form of the fixture a deal is struck at, and where in its declared cell it sits |
//! | [`Character::light`] | what the room's own pendant burns, how hard, and what it hangs in |
//! | [`Character::decor`] | fittings inside the room — the station's own furniture |
//! | [`Character::outfit`] | the shell's plate and its running lights |
//! | [`Character::dress`] | hardware bolted to the shell: a mast, a hangar maw, a space elevator's ribbon |
//!
//! ## What you may not change, and why you cannot
//!
//! > **A character is dressing. It may not move the room, re-shape it, or
//! > change what anything means.**
//!
//! The room's **grid**, its **tile classes**, its **port declarations**,
//! and its **box** belong to the sim (`space_trucking::sim::room`), and
//! the presentation derives every one of them from that declaration. A
//! `Character` cannot express a change to any of them: there is no field
//! here that names a cell, a class, a slot, or a pose, and the builders
//! never consult one. That is the structural half.
//!
//! ## The dressing law, and the frame that is it
//!
//! > **A station dresses the band a room keeps over the crew's heads. A
//! > berth's air is cargo's, and furniture stands in what is left.**
//!
//! [`Character::decor`] is measured off `room::dressing_box` — the band
//! between a room's own walls and its own ceiling's cargo — and not off
//! the room. That is the law made structural rather than asserted, and
//! it is the same trick as everything else here: `at` and `half` are
//! fractions of the frame, so `|at| + |half| <= 1` stops meaning "inside
//! the room" and starts meaning **"clear of every berth in it"**. A
//! station cannot stand a post in a crate's air, in the same way it
//! cannot name a cell.
//!
//! It cost the fleet the same thing it cost every station: the band is
//! the ship's storey less the floor's cargo, the ceiling's cargo, and a
//! standing head, which is 82 mm — so a station's furniture is a plan
//! and no longer an elevation, and what was a metre-tall cabinet is a
//! metre-wide one. Nothing moved sideways: the fractions are the ones
//! their agents wrote, so a plaque on the port flank is still on the
//! port flank.
//!
//! The asserted half, in [`tests`]:
//!
//! - every interior [`Fitting`] lies **wholly inside its own room's
//!   dressing box**, so a character can never reach into cargo's air or
//!   into the room next door;
//! - every exterior fitting stands **wholly outside** every room's
//!   interior, so dressing hangs in the void and never inside somebody;
//! - no fitting stands in a **doorway**, because a threshold belongs to
//!   two rooms and a character owns neither of them — which the dressing
//!   band settles by arithmetic, standing a whole course over a
//!   doorway's own head;
//! - [`Light::burn`] is a fraction of the caller budget and the pendant's
//!   *reach* is not a knob at all, so **a station lights its own floor and
//!   never a riding room's** — the containment rule survives every
//!   character that will ever be written.
//!
//! Four art laws ride along, and they are laws rather than taste:
//!
//! 1. **No signal on hue alone.** A tile class is told by the *kind* of
//!    mark it wears, and [`Tiles`] carries no form knobs — you can repaint
//!    the field and the rim, you cannot turn a rim band into a field or a
//!    struck line into a fill. `Offer` stays **hollow** and `Stock` stays
//!    **filled**, at every station, so the two read against each other
//!    from any angle under any lamp.
//! 2. **Stripes belong to one class.** Hazard tape is `Consume`'s, which
//!    is the furnace's, which is a riding room with no host — so [`Worn`]
//!    simply does not offer it.
//! 3. **Palette purity.** Every colour is a role from `palette.rs`. The
//!    purity test walks this directory too.
//! 4. **The crunch look.** Low-poly primitives built in code
//!    ([`Shape`]) — no asset files, nothing to download, nothing to
//!    credit.
//!
//! ## How a room learns which station it is
//!
//! A `Trade` room does not know which POI it serves; room kind is not
//! station identity, and one kind serves twelve places. It is **derived**,
//! by [`of`], from the pair the sim already keeps: the room's kind and
//! `ShipState`. That is not a shortcut — it is the law the sim itself
//! runs on, written down at the save's own reconstruction of a trade:
//!
//! > *"A counterparty is alongside exactly when its room is: the trade is
//! > derived from the graph, never stored beside it."* (`sim::save`)
//!
//! A `Trade` room is attached by `dock` and detached by `dismiss_callers`
//! on the way out, so an attached trade room's station is the docked one
//! or there is no attached trade room. The event rooms are one-of-a-kind
//! by nature — a derelict is a derelict — so their kind *is* their
//! identity, which is why [`Host`] carries them as their own variants
//! rather than as a POI. Riding rooms (the cabin, the furnace) have no
//! host at all: the ship is nobody's station.
//!
//! Identity therefore costs the save nothing and the tape nothing, and a
//! replay reproduces a station's character because it reproduces the two
//! facts it is derived from. `sim::room` did not have to grow a field
//! that would have been a second opinion about the counterparty.

// The registry is a **surface**, and a surface is used by whoever arrives
// next. `HOSTS`, `DRESS_REACH`, `Fitting::span` and the unspent half of
// `Worn` are the vocabulary a station's agent writes against; they are
// exercised by this module's own tests and by the one station that has
// been filled in, and the eleven files still holding the neutral form
// have not reached for them yet. A dead-code warning here is the seam
// complaining that the fleet has not sailed.
#![allow(dead_code)]

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use space_trucking::sim::ShipState;
use space_trucking::sim::map::PoiId;
use space_trucking::sim::room::RoomKind;

use crate::palette;

mod comet;
mod earth;
mod guild;
mod hermitage;
mod jupiter;
mod mars;
mod neptune;
mod parlor;
mod pump;
mod saturn;
mod umbra;
mod uranus;
mod venus;
mod wanderer;
mod wreck;

// ------------------------------------------------------------- who it is --

/// Whose room this is.
///
/// The registry's key, and the reason one module can own one station: the
/// twelve places on the chart, plus the three event rooms that are
/// one-of-a-kind by nature. Riding rooms are absent on purpose — the ship
/// is nobody's station, and a cabin with a character would be a cabin
/// pretending to be somewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Venus,
    Earth,
    Mars,
    Jupiter,
    Uranus,
    Neptune,
    /// The Guild Station, where every run begins.
    Guild,
    Saturn,
    /// The Umbra Market, in Mercury's shadow.
    Umbra,
    /// The Hermitage: a hollowed rock, and a gift economy.
    Hermitage,
    /// The unnamed comet. It brings no room today — you chip ice off it —
    /// so its module waits for the day it does.
    Comet,
    /// `???`. It brings a trade room and stocks nothing.
    Wanderer,
    /// A derelict's hold.
    Wreck,
    /// The casino's parlor, which has no visible doors.
    Parlor,
    /// The gas station's pump bay.
    Pump,
}

/// Every host, in registry order. The fleet's manifest: one entry, one
/// module, one agent.
pub const HOSTS: [Host; 15] = [
    Host::Venus,
    Host::Earth,
    Host::Mars,
    Host::Jupiter,
    Host::Uranus,
    Host::Neptune,
    Host::Guild,
    Host::Saturn,
    Host::Umbra,
    Host::Hermitage,
    Host::Comet,
    Host::Wanderer,
    Host::Wreck,
    Host::Parlor,
    Host::Pump,
];

impl Host {
    /// The host of the POI at map index `poi`. Total over the chart —
    /// pinned by [`tests::the_registry_is_exhaustive_over_the_chart`], so
    /// a thirteenth POI fails the build rather than falling quietly back
    /// to the neutral room.
    #[must_use]
    pub const fn at(poi: PoiId) -> Option<Self> {
        match poi {
            0 => Some(Self::Venus),
            1 => Some(Self::Earth),
            2 => Some(Self::Mars),
            3 => Some(Self::Jupiter),
            4 => Some(Self::Uranus),
            5 => Some(Self::Neptune),
            6 => Some(Self::Guild),
            7 => Some(Self::Saturn),
            8 => Some(Self::Umbra),
            9 => Some(Self::Hermitage),
            10 => Some(Self::Comet),
            11 => Some(Self::Wanderer),
            _ => None,
        }
    }

    /// A stable small integer, for the plan's change-detection signature.
    /// Not a save token: nothing about a host is ever written down.
    #[must_use]
    pub const fn token(self) -> u8 {
        self as u8
    }
}

/// **Which station a room serves**, derived from the two facts the sim
/// already keeps.
///
/// See the module note: this is the sim's own law about counterparties,
/// read from the presentation's side. `None` means *no character* — a
/// riding room, or (a state the sim's laws forbid, and
/// [`tests::a_trade_room_is_only_ever_alongside_a_dock`] holds them to it)
/// a trade room adrift between stations, which wears the neutral form
/// rather than guessing whose it is.
#[must_use]
pub const fn of(kind: RoomKind, state: ShipState) -> Option<Host> {
    match kind {
        // The ship is nobody's station.
        RoomKind::Cabin | RoomKind::Burner => None,
        // An event room is one of a kind, so its kind is its identity.
        RoomKind::Wreck => Some(Host::Wreck),
        RoomKind::Parlor => Some(Host::Parlor),
        RoomKind::Pump => Some(Host::Pump),
        // A market is alongside exactly while you are docked at it.
        RoomKind::Trade => match state {
            ShipState::Docked(at) => Host::at(at),
            ShipState::Traveling { .. } => None,
        },
    }
}

// --------------------------------------------------------- the vocabulary --

/// The primitive silhouettes a character builds from.
///
/// Low-poly bodies made in code, scaled by a [`Fitting`]'s extents — the
/// art direction's whole geometry budget, and deliberately a short list:
/// a station is told apart by *arrangement*, not by modelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A box. The workhorse: plates, bars, slabs, sills, spars.
    Slab,
    /// A cylinder standing on its own +y axis. Posts, drums, pipes.
    Post,
    /// A sphere. Bulbs, bosses, knobs.
    Dome,
    /// A torus lying in the x/z plane. Rings, wheels, collars.
    Ring,
    /// A tapered drum, wide at the bottom. Shades, hoppers, horns.
    Cone,
}

/// The shared unit-sized bodies, built once per rebuild and handed round.
pub struct Shapes {
    slab: Handle<Mesh>,
    post: Handle<Mesh>,
    dome: Handle<Mesh>,
    ring: Handle<Mesh>,
    cone: Handle<Mesh>,
}

impl Shapes {
    /// Build the set. Every body is one unit across, so a fitting's world
    /// size is exactly its scale.
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        Self {
            slab: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            post: meshes.add(Cylinder::new(0.5, 1.0)),
            dome: meshes.add(Sphere::new(0.5).mesh().ico(2).expect("ico in range")),
            ring: meshes.add(Torus::new(0.32, 0.5)),
            cone: meshes.add(ConicalFrustum {
                radius_top: 0.18,
                radius_bottom: 0.5,
                height: 1.0,
            }),
        }
    }

    /// The body for one silhouette.
    #[must_use]
    pub fn of(&self, shape: Shape) -> Handle<Mesh> {
        match shape {
            Shape::Slab => self.slab.clone(),
            Shape::Post => self.post.clone(),
            Shape::Dome => self.dome.clone(),
            Shape::Ring => self.ring.clone(),
            Shape::Cone => self.cone.clone(),
        }
    }
}

/// The ship's own worn metals, from [`crate::rig::Skin`].
///
/// **Hazard striping is not on this list**, and its absence is the law:
/// stripes belong to `Consume` alone (`docs/ART_DIRECTION_3D.md`), no
/// calling room has a `Consume` tile, and a character that could reach the
/// hazard material would be a second claimant on the one idiom the game
/// reserves for *this will hurt you*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worn {
    Hull,
    Plate,
    PlateShade,
    Socket,
    /// Radium-painted hardware: findable with every lamp aboard sold.
    Brass,
    Rivet,
}

impl Worn {
    /// The palette role this metal is painted in — what the purity test
    /// and the character tests read.
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Hull => palette::HULL,
            Self::Plate => palette::PLATE,
            Self::PlateShade => palette::PLATE_SHADE,
            Self::Socket => palette::SOCKET,
            Self::Brass => palette::BRASS,
            Self::Rivet => palette::RIVET,
        }
    }
}

/// How a surface is finished — the 3D art direction's two material
/// families, plus the ship's own weathered metals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finish {
    /// Painted enamel: a lit surface (`glow::enamel`).
    Enamel,
    /// Enamel with the lights-out self-glow (`glow::etched`) — the floor
    /// that keeps hardware findable when every lamp aboard is cargo
    /// somebody sold.
    Etched,
    /// A phosphor: near-black body, coloured glow, at this intensity
    /// (`glow::phosphor`). 1.0 reads as a lit indicator; 4.0 blooms.
    Phosphor(f32),
    /// One of the ship's shared worn metals. The colour field of the
    /// [`Coat`] is the role it is painted in, for the record.
    Metal(Worn),
}

/// A colour and the way it is finished. Every visible thing a character
/// declares wears one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coat {
    pub color: Color,
    pub finish: Finish,
}

impl Coat {
    /// Painted enamel in a palette role.
    #[must_use]
    pub const fn enamel(color: Color) -> Self {
        Self {
            color,
            finish: Finish::Enamel,
        }
    }

    /// Enamel with the lights-out self-glow.
    #[must_use]
    pub const fn etched(color: Color) -> Self {
        Self {
            color,
            finish: Finish::Etched,
        }
    }

    /// A phosphor at `glow` intensity.
    #[must_use]
    pub const fn phosphor(color: Color, glow: f32) -> Self {
        Self {
            color,
            finish: Finish::Phosphor(glow),
        }
    }

    /// One of the ship's worn metals.
    #[must_use]
    pub const fn metal(worn: Worn) -> Self {
        Self {
            color: worn.color(),
            finish: Finish::Metal(worn),
        }
    }

    /// The material this coat asks for. `skin` is the ship's shared
    /// weathered metals where the caller has them (the interior does; the
    /// void layer builds its own).
    #[must_use]
    pub fn material(
        self,
        materials: &mut Assets<StandardMaterial>,
        skin: Option<&crate::rig::Skin>,
    ) -> Handle<StandardMaterial> {
        match self.finish {
            Finish::Enamel => crate::glow::enamel(materials, self.color),
            Finish::Etched => crate::glow::etched(materials, self.color),
            Finish::Phosphor(glow) => crate::glow::phosphor(materials, self.color, glow),
            Finish::Metal(worn) => match (skin, worn) {
                (Some(skin), Worn::Hull) => skin.hull.clone(),
                (Some(skin), Worn::Plate) => skin.plate.clone(),
                (Some(skin), Worn::PlateShade) => skin.plate_shade.clone(),
                (Some(skin), Worn::Socket) => skin.socket.clone(),
                (Some(skin), Worn::Brass) => skin.brass.clone(),
                (Some(skin), Worn::Rivet) => skin.rivet.clone(),
                // Out in the void there is no `Skin`; the role still is
                // the role, so the metal is rebuilt from it.
                (None, _) => materials.add(StandardMaterial {
                    base_color: worn.color(),
                    perceptual_roughness: 0.92,
                    metallic: if worn == Worn::Brass { 0.8 } else { 0.15 },
                    ..default()
                }),
            },
        }
    }
}

/// One piece of a station's own hardware, measured off the box it hangs
/// in and nothing else.
///
/// **Both `at` and `half` are fractions of the host box's half-extents**,
/// so `at: ZERO, half: ONE` fills the box and `|at| + |half| <= 1` on every
/// axis is "inside it". That is not a convention, it is the containment
/// law: a fitting is checked against exactly that arithmetic, so a
/// character physically cannot reach into the room next door — or, since
/// the host box for furniture is the room's **dressing box**, into the
/// air a berth spends.
///
/// The frames, all right-handed and all derived:
///
/// - **inside a room**: `+x` to starboard, `+y` up, `+z` **aft** — the
///   wall a calling room's one door is on, where its goods stand and its
///   handshake is set. The box is the room's dressing band
///   (`room::dressing_box`), which is the room's own plan and the height
///   between its walls and its ceiling's cargo — so `y: -1` is the top
///   of the last wall course and `y: 1` is as low as a ceiling rig
///   hangs, not the deck and the deckhead.
/// - **on the shell, outside**: the same axes, one wall thickness out.
/// - **on a handshake's cell**: `+x` along the chart's own u axis, `+y` up
///   the wall, `+z` out of the wall into the room — and on that one frame
///   `z` is **metres**, because a cell has no depth to be a fraction of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fitting {
    pub shape: Shape,
    pub coat: Coat,
    /// Centre, in fractions of the host box's half-extents.
    pub at: Vec3,
    /// Half-extents, in fractions of the host box's half-extents.
    pub half: Vec3,
}

impl Fitting {
    /// One fitting, spelled out.
    #[must_use]
    pub const fn new(shape: Shape, coat: Coat, at: Vec3, half: Vec3) -> Self {
        Self {
            shape,
            coat,
            at,
            half,
        }
    }

    /// The fitting's own box in host fractions, `(lo, hi)` — the pure form
    /// every containment law is stated against.
    #[must_use]
    pub fn span(&self) -> (Vec3, Vec3) {
        (self.at - self.half.abs(), self.at + self.half.abs())
    }
}

/// The box a character measures fittings off: where it is, how big it is
/// in its own axes, and the turn that carries its axes into the world's.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub mid: Vec3,
    /// Local half-extents: x starboard, y up, z aft.
    pub half: Vec3,
    /// Local → world.
    pub rot: Quat,
}

impl Frame {
    /// The frame of a world-axis box at `yaw` quarter turns. The lattice
    /// only ever turns a room by quarter turns, so the local extents are
    /// the world ones with x and z traded on the odd yaws — no
    /// trigonometry, and no room ever half a degree off its own hardware.
    #[must_use]
    pub fn of(lo: Vec3, hi: Vec3, yaw: u8) -> Self {
        let span = hi - lo;
        let flip = yaw % 2 == 1;
        Self {
            mid: (lo + hi) * 0.5,
            half: Vec3::new(
                if flip { span.z } else { span.x } * 0.5,
                span.y * 0.5,
                if flip { span.x } else { span.z } * 0.5,
            ),
            rot: Quat::from_rotation_y(f32::from(yaw % 4) * FRAC_PI_2),
        }
    }

    /// Where one fitting stands, in the world.
    #[must_use]
    pub fn place(&self, fitting: &Fitting) -> Transform {
        Transform::from_translation(self.mid + self.rot * (fitting.at * self.half))
            .with_rotation(self.rot)
            .with_scale(
                (fitting.half * self.half * 2.0)
                    .abs()
                    .max(Vec3::splat(1e-4)),
            )
    }
}

// ------------------------------------------------------------ the knobs --

/// What each colored tile class is painted in.
///
/// There is no knob here for the *form* of a class, and that is the
/// no-hue-alone law made structural: `Stock` is a filled field with a rim
/// band, `Offer` is bare deck inside a struck line, and no character can
/// swap them. Repaint them freely; the pair still reads filled-against-
/// hollow from across the room with the lights half out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tiles {
    /// `Stock`: the room's own enamel, filled to the region's edge.
    pub stock: Coat,
    /// `Stock`'s rim band, where that paint ends.
    pub rim: Coat,
    /// `Offer`: the line struck round the bare deck a proposal stands on.
    pub chalk: Coat,
    /// `Threshold`: the studs on the tread plate.
    pub stud: Coat,
    /// `Threshold`: the sill bar along the jamb line.
    pub sill: Coat,
}

/// The room's own pendant.
///
/// **There is no reach knob, on purpose.** A calling room's lamp is sized
/// to its own box (`room::caller_reach`) so it lights its own floor and
/// dies before the middle of any riding room; a character that could set a
/// range would be a station billing the crew for its electricity. What a
/// station picks is what it burns, how hard, and what it hangs in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// The colour it burns.
    pub color: Color,
    /// Fraction of the caller budget, `0..=1`. Zero is a dark room, which
    /// is a legal thing for a station to be.
    pub burn: f32,
    /// The shade's silhouette.
    pub shade: Shape,
    /// What the shade is made of.
    pub shade_coat: Coat,
    /// The glass under it.
    pub glass: Coat,
    /// Extra hardware on the pendant — a cage, a hood, a second bulb —
    /// measured off the shade's own extents.
    pub cage: &'static [Fitting],
}

/// The handshake: the one click-functional fixture in a calling room, and
/// the one place a station's manners show.
///
/// Its **behavior is fixed** — it commits the standing offer, and the sim
/// rules it through the cell `RoomKind::handshake` declares. What a
/// station owns is the FORM: what you strike, what it is made of, where in
/// the declared cell it sits, and how far it throws. The lamp's *colour*
/// is yours; the lamp itself is the fixture's, because it means something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handshake {
    /// The plate the whole fixture is set into.
    pub plate: Coat,
    /// The moving part — what a hand actually works.
    pub knob: Shape,
    pub knob_coat: Coat,
    /// The knob's centre and half-extents in the cell's own frame (see
    /// [`Fitting`]: x and y are cell fractions, z is metres).
    pub knob_at: Vec3,
    pub knob_half: Vec3,
    /// How far it throws when worked, in metres.
    pub throw: f32,
    /// What its one lamp burns while there is something to commit.
    pub lamp: Color,
    /// Brasswork bolted to the plate, in the same cell frame.
    pub trim: &'static [Fitting],
}

/// What a room's outside is made of, before anybody dresses it: the plate
/// the shell wears and the running lights it burns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outfit {
    /// The plate the shell wears.
    pub plate: Color,
    /// What colour its running lights burn.
    pub lamp: Color,
    /// How many running lights it carries, **0, 1 or 2** — they ride the
    /// two top corners, so a third would stack on the first and burn
    /// where a lamp already burns. Capped by test rather than by type
    /// because the cap is a fact about where a shell has corners, not
    /// about what a number may be; if a shell ever wants four, the
    /// placement grows first and this doc follows it.
    pub lamps: u8,
}

/// Everything one station looks like, inside and out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// The colored tile classes, repainted.
    pub tiles: Tiles,
    /// The fixture a deal is struck at.
    pub handshake: Handshake,
    /// The room's own pendant.
    pub light: Light,
    /// The station's furniture, measured off the room's box.
    pub decor: &'static [Fitting],
    /// The shell's kit.
    pub outfit: Outfit,
    /// Hardware bolted to the shell, measured off its half-extents.
    pub dress: &'static [Fitting],
}

/// **The neutral room**: exactly what every calling room wore before this
/// directory existed, and exactly what a station nobody has filled in
/// still wears.
///
/// Every value here is the constant the builders used to hold inline, so
/// filling in a station is a change and leaving one alone is not
/// ([`tests::the_default_character_is_the_room_we_already_had`]). Deliberately
/// dull: a neutral form with character already on it would fight whatever
/// the station is eventually given.
pub const NEUTRAL: Character = Character {
    tiles: Tiles {
        // The room's enamel, in the trim the retired station shelf wore —
        // which is where the goods stood then and where they stand now.
        stock: Coat::enamel(palette::TRIM_SHELF),
        rim: Coat::enamel(palette::PLATE_SHADE),
        // A struck chalk line, faintly self-lit so the bay is findable
        // with the lamps sold.
        chalk: Coat::etched(palette::GLINT),
        stud: Coat::metal(Worn::Rivet),
        sill: Coat::metal(Worn::Brass),
    },
    handshake: Handshake {
        plate: Coat::metal(Worn::Brass),
        // A brass slug that visibly throws when it is worked.
        knob: Shape::Slab,
        knob_coat: Coat::metal(Worn::Brass),
        knob_at: Vec3::new(0.0, 0.0, 0.075),
        knob_half: Vec3::new(0.344_4, 0.344_4, 0.045),
        throw: 0.045,
        lamp: palette::AMBER,
        trim: &[],
    },
    light: Light {
        color: palette::GLINT,
        burn: 1.0,
        shade: Shape::Cone,
        shade_coat: Coat::enamel(palette::PLATE),
        glass: Coat::phosphor(palette::GLINT, 1.4),
        cage: &[],
    },
    decor: &[],
    outfit: Outfit {
        // Somebody else's plate, and an amber lamp because a station that
        // let you dock in the dark would be a station with something to
        // hide (docs/ROOMS.md).
        plate: palette::PLATE,
        lamp: palette::AMBER,
        lamps: 2,
    },
    dress: &[],
};

/// How much of the plate the handshake's backing covers, as a fraction of
/// its declared cell. Not a character knob: the fixture's pick face is
/// bound to the whole cell, and a plate that shrank away from it would
/// leave the crosshair meeting the chart behind the brass.
pub const PLATE_SPAN: f32 = 0.82;

/// How far off its own shell a station's exterior dressing may reach, in
/// shell half-extents. A mast twice the room's height is dressing; a mast
/// ten times it is a second station, and a second station is a `Plan`
/// change, which is the sim's.
pub const DRESS_REACH: f32 = 3.0;

// -------------------------------------------------------------- the shelf --

/// **The registry.** One station, one module, one constant.
///
/// Exhaustive by construction: adding a [`Host`] without a module is a
/// compile error, which is the whole reason the fleet can work in parallel
/// without a coordinator.
#[must_use]
pub const fn character(host: Host) -> Character {
    match host {
        Host::Venus => venus::CHARACTER,
        Host::Earth => earth::CHARACTER,
        Host::Mars => mars::CHARACTER,
        Host::Jupiter => jupiter::CHARACTER,
        Host::Uranus => uranus::CHARACTER,
        Host::Neptune => neptune::CHARACTER,
        Host::Guild => guild::CHARACTER,
        Host::Saturn => saturn::CHARACTER,
        Host::Umbra => umbra::CHARACTER,
        Host::Hermitage => hermitage::CHARACTER,
        Host::Comet => comet::CHARACTER,
        Host::Wanderer => wanderer::CHARACTER,
        Host::Wreck => wreck::CHARACTER,
        Host::Parlor => parlor::CHARACTER,
        Host::Pump => pump::CHARACTER,
    }
}

/// The character of a room that may not have a host — a riding room, or a
/// caller whose station cannot be named. Both wear [`NEUTRAL`].
#[must_use]
pub const fn character_of(host: Option<Host>) -> Character {
    match host {
        Some(host) => character(host),
        None => NEUTRAL,
    }
}

/// The exterior kit for one room.
///
/// The ship's own two rooms keep their kind-keyed kits — a hull is a hull
/// and a furnace wears its soot outside as well as in — and everything
/// that came alongside wears its station's.
#[must_use]
pub const fn outfit(kind: RoomKind, host: Option<Host>) -> Outfit {
    match host {
        Some(host) => character(host).outfit,
        None => match kind {
            // The ship's own: working hull, and the two lamps every hull
            // in this game is legally required to burn.
            RoomKind::Cabin => Outfit {
                plate: palette::HULL,
                lamp: palette::LAMP_OK,
                lamps: 2,
            },
            // The furnace wears its soot on the outside too.
            RoomKind::Burner => Outfit {
                plate: palette::SOOT,
                lamp: palette::EMBER,
                lamps: 1,
            },
            // A caller whose station cannot be named wears the neutral
            // kit, the same way it wears the neutral room.
            RoomKind::Trade | RoomKind::Wreck | RoomKind::Parlor | RoomKind::Pump => NEUTRAL.outfit,
        },
    }
}

#[cfg(test)]
mod tests {
    use space_trucking::sim::map::POI_COUNT;
    use space_trucking::sim::room::{APERTURE, CABIN, Port, ROOM_KINDS, Rooms};

    use super::*;

    /// Every place on the chart has a host, and every host has a
    /// character. A thirteenth POI, or a fifth room kind that calls,
    /// fails here rather than falling quietly back to the neutral room.
    #[test]
    fn the_registry_is_exhaustive_over_the_chart() {
        for poi in 0..POI_COUNT as PoiId {
            let host = Host::at(poi).unwrap_or_else(|| panic!("POI {poi} has no host module"));
            assert_eq!(
                of(RoomKind::Trade, ShipState::Docked(poi)),
                Some(host),
                "docking at POI {poi} must name its own host"
            );
        }
        assert_eq!(Host::at(POI_COUNT as PoiId), None, "the chart ends at 12");
        // And every room kind is answered: riding rooms by `None`,
        // callers by a host with a module behind it.
        for kind in ROOM_KINDS {
            let host = of(kind, ShipState::Docked(6));
            assert_eq!(
                host.is_none(),
                kind.riding(),
                "{kind:?} disagrees with itself about whether it is the ship's"
            );
            if let Some(host) = host {
                let _ = character(host);
            }
        }
        // The manifest is the enum, with nothing left off it.
        let mut seen: Vec<u8> = HOSTS.iter().map(|host| host.token()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), HOSTS.len(), "two hosts share a token");
    }

    /// **The neutral character is the room we already had.** Every value
    /// below is the constant the builders held inline before the registry
    /// existed, so a station nobody has filled in cannot regress — and a
    /// retune of the neutral form has to come through this test.
    #[test]
    fn the_default_character_is_the_room_we_already_had() {
        let neutral = NEUTRAL;
        assert_eq!(neutral.tiles.stock, Coat::enamel(palette::TRIM_SHELF));
        assert_eq!(neutral.tiles.rim, Coat::enamel(palette::PLATE_SHADE));
        assert_eq!(neutral.tiles.chalk, Coat::etched(palette::GLINT));
        assert_eq!(neutral.tiles.stud.color, palette::RIVET);
        assert_eq!(neutral.tiles.sill.color, palette::BRASS);
        assert_eq!(neutral.handshake.knob, Shape::Slab);
        assert_eq!(neutral.handshake.knob_coat.color, palette::BRASS);
        assert!((neutral.handshake.throw - 0.045).abs() < 1e-6);
        assert_eq!(neutral.handshake.lamp, palette::AMBER);
        assert!(neutral.handshake.trim.is_empty());
        assert_eq!(neutral.light.color, palette::GLINT);
        assert!((neutral.light.burn - 1.0).abs() < 1e-6);
        assert_eq!(neutral.light.shade, Shape::Cone);
        assert!(neutral.decor.is_empty(), "the neutral room is unfurnished");
        assert!(neutral.dress.is_empty(), "and undressed");
        assert_eq!(neutral.outfit.plate, palette::PLATE);
        assert_eq!(neutral.outfit.lamp, palette::AMBER);
        assert_eq!(neutral.outfit.lamps, 2);
    }

    /// **Every host has been designed.** This assertion began life the
    /// other way up: while the fleet was half-finished it named the
    /// stations that were filled and held the rest to the neutral room,
    /// so a partly-designed chart still shipped. The whole fleet has
    /// since sailed, and the useful reading inverted with it — what is
    /// worth guarding now is that nobody adds a place to the chart and
    /// leaves it wearing the default. A new host fails here until
    /// somebody gives it a character, which is the note the old
    /// enumerated list was really trying to leave.
    #[test]
    fn every_host_has_a_character_of_its_own() {
        for host in HOSTS {
            assert_ne!(
                character(host),
                NEUTRAL,
                "{host:?} is still the room we already had — give it one of its own"
            );
        }
    }

    /// **A character cannot move the room.**
    ///
    /// Structurally it has no field that names a cell, a class, a port, or
    /// a pose. What is left to assert is reach, and it is asserted in
    /// fractions of the host box, which is the only frame a fitting has:
    ///
    /// - inside, a fitting stays wholly within the band its room keeps
    ///   for dressing (`room::dressing_box`), so no character reaches
    ///   through a partition **or into a berth's air** — the dressing
    ///   law, and the reason the same arithmetic now says both;
    /// - outside, dressing stands wholly OUT of the interior — a mast is
    ///   a mast and never a beam through somebody's ceiling — and no
    ///   further out than [`DRESS_REACH`], because dressing is furniture
    ///   on a hull and not a second station;
    /// - nothing stands in a doorway, because a threshold belongs to two
    ///   rooms and a character owns neither.
    #[test]
    fn a_character_stays_inside_the_room_it_dresses() {
        for host in HOSTS {
            let ch = character(host);
            for (i, fitting) in ch.decor.iter().enumerate() {
                let (lo, hi) = fitting.span();
                for axis in 0..3 {
                    assert!(
                        lo[axis] >= -1.0 - 1e-5 && hi[axis] <= 1.0 + 1e-5,
                        "{host:?} decor[{i}] reaches through the room's own wall on \
                         axis {axis}: {lo:?}..{hi:?}"
                    );
                }
            }
            for (i, fitting) in ch.dress.iter().enumerate() {
                let (lo, hi) = fitting.span();
                for axis in 0..3 {
                    assert!(
                        lo[axis] >= -DRESS_REACH - 1e-5 && hi[axis] <= DRESS_REACH + 1e-5,
                        "{host:?} dress[{i}] is a second station on axis {axis}: \
                         {lo:?}..{hi:?}"
                    );
                }
                // Outside furniture stands outside: its box may touch the
                // shell's faces and may never enter it.
                let inside = (0..3).all(|axis| hi[axis].min(1.0) - lo[axis].max(-1.0) > 1e-4);
                assert!(
                    !inside,
                    "{host:?} dress[{i}] hangs INSIDE the room: {lo:?}..{hi:?}"
                );
            }
        }
    }

    /// **A station's dressing band stands over the head of its own
    /// doorway.**
    ///
    /// Nothing a character owns may stand in a doorway, because a
    /// threshold belongs to two rooms and a character owns neither. That
    /// used to be checked fitting by fitting; it is arithmetic now, and
    /// the arithmetic is worth saying out loud because it is the reason
    /// the check went away. A door is two courses tall
    /// ([`APERTURE`]) and the band a station dresses in begins at the
    /// top of the third ([`crate::room::WALL_H`]), so a doorway's head
    /// and a station's lowest possible fitting are a whole course apart
    /// — for every kind that calls, at every port it declares.
    #[test]
    fn a_stations_dressing_band_stands_over_the_head_of_its_own_doorway() {
        for host in HOSTS {
            let kind = serves(host);
            let mut rooms = Rooms::new();
            let id = rooms
                .spawn(kind, CABIN)
                .unwrap_or_else(|why| panic!("a {kind:?} comes alongside: {why:?}"));
            let room = rooms.get(id).expect("the room it just attached");
            let placed = crate::room::placed(id, room);
            let (band, _) = crate::room::dressing_box(&placed);
            for (_, port) in kind.declared() {
                let Port::Door { .. } = port else { continue };
                let head = f32::from(APERTURE).mul_add(crate::rig::BAY_CELL, placed.lo.y);
                assert!(
                    band.y >= head,
                    "{host:?} dresses a {kind:?} from {:.3} m, and its doorway's \
                     head is at {head:.3} m",
                    band.y - placed.lo.y
                );
            }
        }
    }

    /// **A room's own fixtures stay in the boxes they are measured off.**
    ///
    /// The law above is about a character's furniture and the room's
    /// box. This is the other two fittings — the ones the ROOM carries
    /// and a station only dresses — and neither is measured off the room
    /// at all: a handshake off its own declared cell, a cage off a box
    /// one shade across on the lamp it hangs on. Both frames said so in
    /// prose and neither was ever checked, and twelve of the fifteen
    /// cages had become beams across the room while nobody was looking.
    ///
    /// Up is the one direction a cage may leave its box, because a
    /// hanger reaching for the ceiling is what a pendant is — and it
    /// stops at the ceiling ([`crate::room::CAGE_RISE`]).
    #[test]
    fn a_rooms_own_fixtures_stay_in_the_boxes_they_are_measured_off() {
        for host in HOSTS {
            let ch = character(host);
            let knob = Fitting::new(
                ch.handshake.knob,
                ch.handshake.knob_coat,
                ch.handshake.knob_at,
                ch.handshake.knob_half,
            );
            for (what, fitting) in std::iter::once(("knob".to_owned(), &knob)).chain(
                ch.handshake
                    .trim
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (format!("trim[{i}]"), f)),
            ) {
                let (lo, hi) = fitting.span();
                for axis in 0..2 {
                    assert!(
                        lo[axis] >= -1.0 - 1e-5 && hi[axis] <= 1.0 + 1e-5,
                        "{host:?} handshake {what} reaches out of its own cell on \
                         axis {axis}: {lo:?}..{hi:?}"
                    );
                }
                // Depth is metres on this one frame, and the room keeps
                // exactly one cell of deck for the counter to be worked
                // over (`RoomKind::fixture`). Brass recessed INTO the
                // wall is a fixture being set into fabric; brass past
                // that cell would be standing over deck nobody kept.
                assert!(
                    hi.z <= crate::rig::BAY_CELL + 1e-5,
                    "{host:?} handshake {what} reaches past the deck the room keeps \
                     for it: {lo:?}..{hi:?}"
                );
            }
            for (i, fitting) in ch.light.cage.iter().enumerate() {
                let (lo, hi) = fitting.span();
                for axis in 0..3 {
                    let ceiling = if axis == 1 {
                        crate::room::CAGE_RISE
                    } else {
                        1.0
                    };
                    assert!(
                        lo[axis] >= -1.0 - 1e-5,
                        "{host:?} cage[{i}] hangs below the shade it is measured off \
                         on axis {axis}: {lo:?}..{hi:?}"
                    );
                    assert!(
                        hi[axis] <= ceiling + 1e-5,
                        "{host:?} cage[{i}] is a beam across the room on axis {axis}: \
                         {lo:?}..{hi:?}, past {ceiling}"
                    );
                }
            }
        }
    }

    /// Which room kind a host's character actually dresses.
    const fn serves(host: Host) -> RoomKind {
        match host {
            Host::Wreck => RoomKind::Wreck,
            Host::Parlor => RoomKind::Parlor,
            Host::Pump => RoomKind::Pump,
            _ => RoomKind::Trade,
        }
    }

    /// **A station lights its own floor and never a riding room's**, for
    /// every character that will ever be written: the pendant's reach is
    /// derived from the room's box and is not a knob, and what a station
    /// may pick is bounded by the same budget the neutral lamp burns.
    #[test]
    fn no_station_can_outshine_the_caller_budget() {
        for host in HOSTS {
            let light = character(host).light;
            assert!(
                (0.0..=1.0).contains(&light.burn),
                "{host:?} burns {} of the caller budget",
                light.burn
            );
        }
    }

    /// A trade room is alongside exactly while the ship is docked, which
    /// is the premise the whole derivation rests on. If this ever fails,
    /// [`of`] is a guess and the identity wants a field of its own.
    #[test]
    fn a_trade_room_is_only_ever_alongside_a_dock() {
        let mut rooms = Rooms::new();
        assert!(rooms.spawn(RoomKind::Trade, CABIN).is_ok());
        // Underway, the derivation refuses to name a station rather than
        // inventing one — and the room falls back to the neutral form.
        assert_eq!(
            of(
                RoomKind::Trade,
                ShipState::Traveling {
                    from: 0,
                    to: 1,
                    progress: 3,
                    leg_ticks: 90,
                }
            ),
            None
        );
        assert_eq!(character_of(None), NEUTRAL);
    }

    /// **The Guild is not a generic room.** The worked exemplar has to
    /// differ from the default in more than one axis, or it is a recolour
    /// and the seam has not been shown to work.
    #[test]
    fn the_guild_reads_as_the_guild() {
        let guild = character(Host::Guild);
        assert_ne!(guild, NEUTRAL);
        let axes = [
            ("tiles", guild.tiles != NEUTRAL.tiles),
            ("handshake", guild.handshake != NEUTRAL.handshake),
            ("light", guild.light != NEUTRAL.light),
            ("decor", !guild.decor.is_empty()),
            ("outfit", guild.outfit != NEUTRAL.outfit),
            ("dress", !guild.dress.is_empty()),
        ];
        for (name, differs) in axes {
            assert!(differs, "the Guild wears the neutral {name}");
        }
        // Form, not hue alone: the handshake is a different SHAPE, not a
        // repainted plunger.
        assert_ne!(guild.handshake.knob, NEUTRAL.handshake.knob);
        assert!(!guild.handshake.trim.is_empty());
        // And the filled-against-hollow reading survives the repaint,
        // because there is no knob that could break it.
        assert_eq!(guild.tiles.stock.finish, Finish::Enamel);
    }

    /// **A cage passes the stem clear, or through it, never along it.**
    /// A hanger may clasp the pendant's stem and a strap may run through
    /// it, but a flank cut to within a hair of the stem's own is four
    /// faces of brass fighting four of grey for the depth buffer, all the
    /// way up. Saturn's hook was cut a millimetre inside the stem, which
    /// is how the gauntlet found it.
    ///
    /// A cage is measured off a box one shade across on every side of the
    /// lamp (`room::caller_lamp`), so all three numbers are the room's:
    /// [`crate::room::SHADE_R`] scales the fraction,
    /// [`crate::room::STEM_T`] is the flank it has to miss, and
    /// [`crate::rig::layer::SKIN`] is how far missing it takes.
    #[test]
    fn a_cage_passes_the_stem_clear_or_through_it() {
        use crate::rig::layer::SKIN;
        use crate::room::{SHADE_R, STEM_T};

        let stem = STEM_T * 0.5;
        for host in HOSTS {
            for (i, fitting) in character(host).light.cage.iter().enumerate() {
                let (lo, hi) = fitting.span();
                let (lo, hi) = (lo * SHADE_R, hi * SHADE_R);
                // The stem runs UP from the lamp point, so a hoop slung
                // under the shade has no stem to run along.
                if hi.y <= 0.0 {
                    continue;
                }
                for axis in [0, 2] {
                    for face in [lo[axis], hi[axis]] {
                        assert!(
                            (face.abs() - stem).abs() > SKIN,
                            "{host:?}: cage[{i}] has a flank at {face:.4} on axis \
                             {axis}, which is the stem's own {stem:.4}"
                        );
                    }
                }
            }
        }
    }
}
