//! The cabin itself: an enclosed box with flavor, per DESIGN.md's first
//! pass. **The hull owns no panels at all now** — the star tank, the
//! launch handle, and the readings are cargo, and the last fixed panel
//! (the console face, with its toggle plate and hangar strip) came off
//! this pass; its meta-controls live in the `Esc` menu (`crate::menu`),
//! which is overlay, not room. What the ship itself owns is the walkable
//! cargo bay: the sim's room net — an 8×7 floor inside three courses of
//! wall — unfolded onto the whole hull at furniture scale (docs/BAY.md),
//! worked from roam with the crosshair instead of from a focus pose.
//!
//! Two camera postures. **Roaming**: a conventional first-person walk —
//! pointer locked, mouse to look, WASD to move, a crosshair dot; aim at
//! an instrument and its own rig invites with a glint frame
//! (`pieces::hover_glint`). **Focused**: click (or `E`) and the camera
//! glides to that station's viewpoint — wherever the cargo carrying it
//! happens to hang — the cursor frees, and precise sim interaction
//! happens exactly as in 2D. `Esc`, right-click, or `E` steps back out.
//! The camera never trails the cursor — deliberate moves only, nothing
//! to get seasick over.
//!
//! Structural geometry is *data first* ([`structure`]): one list, no
//! furniture derived twice, and a unit test walks the invariants. Focus
//! viewpoints are fitted from the live surfaces' extents and the
//! camera's FOV, so a station that rides a crate takes its pose with it.
//!
//! Also home to the pixel crunch (a 480×270 nearest-neighbour target —
//! "smoothing off" applied to the whole world) and the shared low-poly
//! material [`Skin`].

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::image::ImageSampler;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use space_trucking::sim::room::CABIN;
use space_trucking::sim::{Loc, Piece, Vec2 as SimVec2, layout};

use crate::palette;
use crate::surface::{SimSurface, Station};

/// The crunch target, in pixels. The window upscales this without
/// smoothing; hard pixel edges everywhere. One knob, like the 2D CRUNCH.
pub const CRUNCH_W: u32 = 480;
pub const CRUNCH_H: u32 = 270;

/// Vertical field of view, radians. Focus-distance math depends on it,
/// so it is pinned here rather than left to the projection default.
const FOV: f32 = 0.9;

/// Roaming eye height and walk envelope (an AABB the camera may occupy;
/// clear of every slab by construction, asserted by test). The envelope
/// stops short of the bay's floor plates: cargo is walked *up to*, never
/// stood on.
pub const EYE_HEIGHT: f32 = 1.5;

/// Half the head a standing body swings at [`EYE_HEIGHT`] — the height
/// a fitting has to clear to be a thing you walk under rather than a
/// thing you walk into.
///
/// It is one number because it is read twice: the walk's own detector
/// measures the eye's way with this box (`gauntlet::walk_clear`), and
/// what hangs over a room's middle is sized against it
/// (`room::CALLER_DROP`). A clearance designed to one figure and judged
/// against another is how a lamp ends up three centimetres inside the
/// player's forehead.
pub const HEAD_R: f32 = 0.09;

// The room grid made the whole floor walkable ground — cargo stands
// among the walker now, not beyond a strip — so the envelope spans the
// room up to body clearance at the walls. The clamp is collision
// with the HULL and nothing else: cargo has no body to bump, and the
// placement law no longer reserves a lane for one either.
// The cabin's own box is AUTHORED, because its hull is; every attached
// room derives its own from its pose, and the doorways join them
// (`room::walk_boxes`).
// The front edge used to stop 0.59 short of the front floor row, on the
// old front wall's account — the wall stood half a metre forward of the
// cabin's floor box across a gutter nothing could reach, and the
// envelope had to clear the wall rather than the room. The gutter went
// (`room::CABIN_TRIM`), so this reaches the front row with the same
// clearance the aft row gets, and `walk_envelope_covers_the_floor` now
// asserts the whole deck is standable rather than merely near.
pub const WALK_MIN: Vec3 = Vec3::new(-1.85, EYE_HEIGHT, -1.30);
pub const WALK_MAX: Vec3 = Vec3::new(1.85, EYE_HEIGHT, 2.27);
const WALK_SPEED: f32 = 1.3;

/// Eye height while passing a doorway, and how fast the body bends into
/// it. The stoop is a whole cell of clearance under the lintel, and it
/// finishes inside the half-second law like every other camera move.
pub const DUCK_HEIGHT: f32 = 0.82;
const DUCK_RATE: f32 = 3.4;
const LOOK_SPEED: f32 = 0.0026;
pub const PITCH_LIMIT: f32 = 1.35;

// ---- The bay: the hold grid unfolded onto the aft of the cabin ----

/// Bay cell edge, world units — square cells at furniture scale. One
/// lattice cell, shared by every room because the lattice is shared
/// (docs/ROOMS.md, "The lattice").
pub const BAY_CELL: f32 = 0.55;
/// Bay width: eight columns flush toward the side walls (the columns
/// beyond them are the walls, exactly where the wall-affix rule points).
const BAY_W: f32 = 8.0 * BAY_CELL;
/// The wall band's quad plane, just proud of the aft hull's inner face.
/// This is where lattice y = 0 lands, and `room::ANCHOR` pins the whole
/// ship to it.
pub const BAY_WALL_Z: f32 = 2.41;

/// The decal ladder: everything drawn flat over a chart quad sits at a
/// named rung along the chart's inward normal, and the z-fight test
/// (`pieces::tests::the_decal_ladder_never_z_fights`) keeps every rung
/// a depth-safe step from its neighbours — the playtest's shimmering
/// doormat is the defect class this retires. Flat paints on the ladder
/// keep their meshes ≤ [`layer::SKIN`] thick so rungs, not luck, decide
/// what draws over what.
///
/// **A reading gets a rung; a room does not get to share one.** The
/// three readings a tile can carry — its class's field, its class's
/// mark, and the tread of a doorway crossing it — are three rungs
/// ([`layer::TILE`], [`layer::MARK`], [`layer::TREAD`]), because the
/// playtest found all three landing on one square metre of the Guild's
/// floor and shimmering there. Adding a fourth reading means adding a
/// rung and naming it in the ladder test, which fails the build rather
/// than the eye.
pub mod layer {
    /// **The rung below the ladder: a backer plate's own face.**
    ///
    /// A backer is the worn slab a chart is painted on, and it is the
    /// only thing on the ladder that stands *behind* the mapping plane —
    /// so it carries two numbers, a depth and a thickness, and both are
    /// law. The depth keeps its face a step under [`TILE`]; the
    /// thickness keeps its BACK face off the hull plane it covers,
    /// because an aperture punch cuts a plate wherever the opening
    /// begins and a remainder that ends flush with the deck is two
    /// opaque faces on one plane. That remainder is the playtest's
    /// flickering floor, and this constant is why it cannot come back.
    pub const BACKER: f32 = 0.003;
    /// How thick a backer plate is. Kept under [`BACKER`] so the whole
    /// slab lives strictly between the chart and the hull.
    pub const BACKER_T: f32 = 0.002;
    /// **The tile field**: berth socket wells, and the flat paint a
    /// colored class lays over its whole region. A field is the ground
    /// a mark is read against and never a pattern itself, which is why
    /// it is the lowest rung a room paints on.
    pub const TILE: f32 = 0.002;
    /// **The tile mark**: the form a class carries so it never signals
    /// on hue alone — the offer's chalk line, the stock's border band,
    /// the burner's hazard tape. Marks are drawn on a region's own RIM,
    /// not stamped per cell, so this rung stays sparse.
    pub const MARK: f32 = 0.006;
    /// **The threshold's tread**: the sill bar and stud plate laid on the
    /// deck cells a doorway stands on. It rides over whatever field and
    /// mark those cells already carry, because a doorway crosses a
    /// room's paint rather than replacing it — the playtest's fighting
    /// doormat was this reading sharing a rung with the stock's.
    pub const TREAD: f32 = 0.010;
    /// Laid coverings' base; a rug's pile rises `RUG_THICK` above it.
    pub const LAID: f32 = 0.014;
    /// Placement hint quads.
    pub const HINT: f32 = 0.030;
    /// The hint's refusal slash.
    pub const SLASH: f32 = 0.034;
    /// The violation flash frame.
    pub const FLASH: f32 = 0.038;
    /// The violation glyph bars.
    pub const GLYPH: f32 = 0.042;
    /// The composed offer's claim frame — a standing reading rather than
    /// a flash, so it sits over everything else on its cell.
    pub const CLAIM: f32 = 0.046;
    /// Minimum step between occupied rungs that stays fight-free at
    /// room distances in the depth buffer. Consumed by the ladder test
    /// (`pieces::tests`), which is its whole job.
    #[allow(dead_code)]
    pub const STEP: f32 = 0.004;
    /// Maximum mesh thickness for a flat paint riding a rung.
    pub const SKIN: f32 = 0.0015;
}

/// How far the roaming crosshair can grab or place, in world units —
/// arm's length plus a step, so bay work happens near the body and the
/// far half of the room stays a view, not a reach.
pub const REACH: f32 = 2.0;

// ---- The burner is a room now ----
//
// The incinerator used to be an annex carved off the starboard wall, with
// its own hand-measured chamber, its own doorway constants, and four
// hazard tiles bound to a rail. It is an ordinary room attached at an
// ordinary port (docs/ROOMS.md, "The burner room"), so its box, its
// doorway, and its Consume-tiled deck all come out of the lattice like
// everybody else's, and the AIR_* constants retired with the annex they
// measured. What is left of the airlock module is the furnace's own
// flavour: the firebox glass and the doorway beacon.
//
// The cabin's doorway is likewise no longer written down: `structure`
// punches every one of the cabin's six declared apertures out of its
// hull, so no slab in this game says where a door is.

/// Focus glide length, seconds. A camera move is feedback: it answers a
/// click and finishes fast.
const GLIDE: f32 = 0.38;

/// Margin factor when fitting a panel into the focused view.
const FIT_MARGIN: f32 = 1.14;

/// How far a physical panel plate extends past its mapped quad. Nothing
/// on the hull wears one any more; the instruments' own rigs keep the
/// margin, and [`focus_pose`] still frames by it.
const PLATE_MARGIN: f32 = 0.03;

// **No panel list.** There used to be one — `panels()` — naming the sim
// regions screwed to the cabin's walls. It is gone, and the emptiness is
// the point: the hold unfolded into the bay ([`bay`]), the instruments
// became cargo (`Station::Map` and `Station::Lever` ride their pieces'
// cells, `pieces::instrument_surface`), the barter counter left with the
// interface it belonged to (docs/ROOMS.md), and the console face left
// with the last controls that were only ever *about* the game rather
// than in it (`crate::menu`). A hull that owns no screens is the ship
// this game was always describing: everything you can read, you can
// also carry, sell, or lose.

/// The cabin's six mapped surfaces: the room net unfolded like an opened
/// box. Rows 0–2 stand on the aft wall (row 0 the cornice), rows 3–9
/// fold onto the deck and the two side walls, rows 10–12 stand on the
/// front wall, and the ceiling chart folds on past the starboard
/// cornice. Sim +x runs to the player's right when facing the aft wall;
/// the seams are watertight (or declared gutters) by test. Cursor rays
/// from roam project through these exactly as focus cursors project
/// through panels, so the sim keeps every ruling.
///
/// Every room folds this way now ([`crate::room::charts`]); the cabin is
/// simply the room you start in, at the lattice origin. This wrapper is
/// what the geometry tests and the fittings measure against.
#[must_use]
pub fn bay() -> [(Station, SimSurface); 6] {
    crate::room::charts(CABIN, &crate::room::cabin_room())
}

/// The bay as it was hand-authored, before the lattice derived it. Kept
/// as the witness for [`crate::room`]'s own test: the generalization
/// moved no cabin cell, and this is what proves it.
#[must_use]
#[cfg(test)]
pub fn bay_authored() -> [(Station, SimSurface); 6] {
    // The wall band's height, the centre plane of the floor chart, and
    // the trims that were measured by eye before the lattice arrived: the
    // side charts stand proud of the wall ribs (which span ±1.63..±1.69),
    // the front chart sits just inside the front hull at -1.97, and the
    // ceiling chart hangs just under the ceiling slab at 2.32.
    //
    // The seam law pins every axis: columns match across the folds (floor
    // col 3 lies to port because the port chart's baseboard is x2),
    // cornices sit up, y3 rows lie aft. Every normal therefore points OUT
    // of the room (`Station::chart_flipped`); consumers use
    // `Station::inward`/`Station::face`.
    //
    // The front chart's plane moved when the front gutter went: it sits
    // just inside the cabin's own floor box now, like the side charts.
    const BAY_WALL_H: f32 = 3.0 * BAY_CELL;
    const BAY_FLOOR_D: f32 = 7.0 * BAY_CELL;
    const BAY_FLOOR_ZC: f32 = BAY_WALL_Z - BAY_FLOOR_D * 0.5;
    const BAY_SIDE_X: f32 = 2.17;
    const BAY_FRONT_Z: f32 = -1.41;
    // Four courses of the cargo grid, which is where the deckhead went
    // when the lattice took over the one axis it had never governed.
    const BAY_CEIL_Y: f32 = 4.0 * BAY_CELL;
    const BAY_FLOOR_Y: f32 = 0.012;

    // One chart's logical rect, in net cells.
    let chart = |cx: u8, cy: u8, w: u8, h: u8| {
        layout::Rect::new(
            f32::from(cx).mul_add(layout::CELL, layout::GRID_ORIGIN.x),
            f32::from(cy).mul_add(layout::CELL, layout::GRID_ORIGIN.y),
            f32::from(w) * layout::CELL,
            f32::from(h) * layout::CELL,
        )
    };
    let wall_mid = BAY_WALL_H * 0.5;
    [
        (
            Station::BayWall,
            SimSurface {
                center: Vec3::new(0.0, wall_mid, BAY_WALL_Z),
                half_u: Vec3::X * (BAY_W * 0.5),
                half_v: Vec3::NEG_Y * wall_mid,
                rect: chart(3, 0, 8, 3),
            },
        ),
        (
            Station::BayFloor,
            SimSurface {
                center: Vec3::new(0.0, BAY_FLOOR_Y, BAY_FLOOR_ZC),
                half_u: Vec3::X * (BAY_W * 0.5),
                half_v: Vec3::NEG_Z * (BAY_FLOOR_D * 0.5),
                rect: chart(3, 3, 8, 7),
            },
        ),
        (
            Station::BayPort,
            SimSurface {
                center: Vec3::new(-BAY_SIDE_X, wall_mid, BAY_FLOOR_ZC),
                half_u: Vec3::NEG_Y * wall_mid,
                half_v: Vec3::NEG_Z * (BAY_FLOOR_D * 0.5),
                rect: chart(0, 3, 3, 7),
            },
        ),
        (
            Station::BayStarboard,
            SimSurface {
                center: Vec3::new(BAY_SIDE_X, wall_mid, BAY_FLOOR_ZC),
                half_u: Vec3::Y * wall_mid,
                half_v: Vec3::NEG_Z * (BAY_FLOOR_D * 0.5),
                rect: chart(11, 3, 3, 7),
            },
        ),
        (
            Station::BayFront,
            SimSurface {
                center: Vec3::new(0.0, wall_mid, BAY_FRONT_Z),
                half_u: Vec3::X * (BAY_W * 0.5),
                half_v: Vec3::Y * wall_mid,
                rect: chart(3, 10, 8, 3),
            },
        ),
        (
            Station::BayCeiling,
            SimSurface {
                center: Vec3::new(0.0, BAY_CEIL_Y, BAY_FLOOR_ZC),
                half_u: Vec3::NEG_X * (BAY_W * 0.5),
                half_v: Vec3::NEG_Z * (BAY_FLOOR_D * 0.5),
                rect: chart(14, 3, 8, 7),
            },
        ),
    ]
}

// ---- Structural geometry as data ----

/// An axis-aligned structural mass: walls, ribs, and whatever else the
/// hull is made of. Every one of them is HULL now — the furniture-class
/// masses that used to stand in the cabin's structure left with the
/// barter counter, the burner annex, and the console face, so the
/// finish that told the two apart had nothing left to tell apart.
#[derive(Clone, Copy, Debug)]
pub struct Slab {
    pub center: Vec3,
    pub size: Vec3,
}

impl Slab {
    const fn new(center: Vec3, size: Vec3) -> Self {
        Self { center, size }
    }

    /// Whether a point sits inside this slab, shrunk by `eps` so flush
    /// contact does not count as penetration. Consumed by the geometry
    /// invariant tests; runtime code only spawns slabs.
    #[allow(dead_code)]
    #[must_use]
    pub fn contains(&self, p: Vec3, eps: f32) -> bool {
        let h = self.size * 0.5 - Vec3::splat(eps);
        (p - self.center).abs().cmplt(h).all()
    }
}

/// Every axis-aligned mass in the cabin: the box, and the ribs that say
/// somebody built this hull in a hurry — with every one of the cabin's
/// six declared apertures cut out of whatever it passes through.
///
/// **No slab here knows where a door is.** The holes come from
/// `room::cabin_holes`, which reads `RoomKind::Cabin`'s own port
/// declaration; move a door in the sim and the hull follows it, which is
/// the port law's first clause held by construction rather than by care.
/// What fills those holes — a plate drawn shut, a jamb standing open — is
/// `room::doorways`' business, because that depends on the graph.
#[must_use]
// One slab per line of the room's plan; splitting the list would
// scatter the one place the whole hull is written down.
#[allow(clippy::too_many_lines)]
pub fn structure() -> Vec<Slab> {
    let mut slabs = vec![
        // The box: floor, ceiling, four walls. The 8x7 floor put a cell
        // (0.55) between each of these and where it used to stand: the
        // room grew outward on every side at once, so nothing inside it
        // had to be re-composed, only re-measured.
        Slab::new(Vec3::new(0.0, -0.05, 0.45), Vec3::new(4.5, 0.1, 4.0)),
        Slab::new(Vec3::new(0.0, 2.32, 0.45), Vec3::new(4.5, 0.1, 4.0)),
        // The front wall stands on the cabin's own floor-box face, like
        // the other three. It used to stand at -1.97, half a metre
        // forward of it, with dead deck in between: no net cell reached
        // the gutter, so nothing could be berthed there, and the walk
        // envelope had to clear the wall rather than the room, so
        // nobody could stand there either. The console face it was built
        // around left two passes ago and the strip held nothing.
        Slab::new(Vec3::new(0.0, 1.15, -1.49), Vec3::new(4.5, 2.5, 0.1)),
        Slab::new(Vec3::new(0.0, 1.15, 2.47), Vec3::new(4.5, 2.5, 0.1)),
        Slab::new(Vec3::new(-2.27, 1.15, 0.45), Vec3::new(0.1, 2.5, 4.0)),
        Slab::new(Vec3::new(2.27, 1.15, 0.45), Vec3::new(0.1, 2.5, 4.0)),
    ];
    // Wall ribs: the junk that says somebody built this hull in a hurry.
    // The port wall runs its full set again — the chart tank that used
    // to be bolted through them is cargo now, hung in front of whatever
    // wall it is carried to, so the hull has nothing to make room for.
    // Neither wall dodges a doorway by hand any more; the punch below
    // takes the ribs out of every aperture the cabin declares.
    for i in 0..6 {
        let z = 0.7f32.mul_add(i as f32, -1.20);
        for sx in [-2.21f32, 2.21] {
            slabs.push(Slab::new(
                Vec3::new(sx, 1.15, z),
                Vec3::new(0.06, 2.3, 0.08),
            ));
        }
    }
    // No derived furniture: the counter's support came out with the
    // counter (docs/ROOMS.md), and the console face's plate came out
    // with the face. Nothing in the cabin is measured off a panel any
    // more, because there are no panels.
    // Every declared aperture, cut. Doors, ladder, and hatch alike: the
    // opening is architecture, and what stands in it is hardware.
    for (lo, hi) in crate::room::cabin_holes() {
        slabs = slabs
            .into_iter()
            .flat_map(|slab| {
                crate::room::punch(slab.center, slab.size, lo, hi)
                    .into_iter()
                    .map(|(center, size)| Slab::new(center, size))
            })
            .collect();
    }
    slabs
}

// ---- The camera rig ----

/// A focused viewpoint. Two of them, and **both ride cargo**: the face
/// that used to make a third came off the wall this pass, and its
/// controls are overlay now (`crate::menu`). A focus is a thing you own,
/// not a thing the ship came with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Tank,
    Lever,
}

impl Focus {
    /// Which focus a station belongs to. The bay surfaces have none —
    /// cargo is worked from roam, crosshair-first, and the camera never
    /// glides for it — and a standing rig's own face is bay surface
    /// that happens to travel with its piece.
    #[must_use]
    pub const fn of(station: Station) -> Option<Self> {
        match station {
            Station::Map => Some(Self::Tank),
            Station::Lever => Some(Self::Lever),
            Station::BayWall
            | Station::BayFloor
            | Station::BayPort
            | Station::BayStarboard
            | Station::BayFront
            | Station::BayCeiling
            | Station::Handshake
            | Station::Standing => None,
        }
    }
}

/// Camera state machine.
#[derive(Clone, Copy, Debug)]
pub enum Mode {
    /// First-person roam: pointer locked, WASD + mouse look.
    Roam,
    /// Gliding toward a focus viewpoint.
    ToFocus {
        focus: Focus,
        from: (Vec3, Quat),
        t: f32,
    },
    /// Parked at a focus viewpoint; the cursor is free and the sim
    /// receives pointer interaction.
    Focused { focus: Focus },
    /// Gliding back to the roaming pose.
    ToRoam { from: (Vec3, Quat), t: f32 },
}

/// The camera rig: roaming pose plus the current mode.
#[derive(Resource)]
pub struct CameraRig {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub mode: Mode,
    /// Roam-mode cursor parking: `Esc` frees the OS cursor (to reach
    /// other windows); the next click on the game reclaims it.
    pub parked: bool,
    /// Dev tooling only (`--view drydock`): let the cabin camera see the
    /// void layer too, so the ship's own exterior shells can be looked
    /// at from outside. Never set in play.
    pub drydock: bool,
}

impl CameraRig {
    /// Boot pose: standing mid-cabin, facing the wraparound.
    #[must_use]
    pub fn boot(view: Option<Focus>) -> Self {
        Self {
            pos: Vec3::new(0.0, EYE_HEIGHT, 0.9),
            yaw: 0.0,
            pitch: -0.12,
            mode: view.map_or(Mode::Roam, |focus| Mode::Focused { focus }),
            parked: false,
            drydock: false,
        }
    }

    /// Whether the sim should receive pointer interaction this frame.
    #[must_use]
    pub const fn interactive(&self) -> bool {
        matches!(self.mode, Mode::Focused { .. })
    }

    /// Whether the player is actively roaming: first-person, cursor
    /// locked, crosshair live — the regime the bay is worked in.
    #[must_use]
    pub const fn roaming(&self) -> bool {
        matches!(self.mode, Mode::Roam) && !self.parked
    }

    fn roam_rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }
}

/// The pose a focus parks at: ONE panel's extents fitted to the camera
/// FOV, eyed along that panel's normal, up running up-panel.
///
/// The surfaces are whatever is standing right now, not a fixed list:
/// an instrument's station rides its cargo, so its pose is its berth's
/// (BAY.md, "Focus poses become relative to the instrument's berth").
/// `None` when nothing carries that station — jettisoned, shelved, or
/// in the player's own hands — and the caller falls back to roam.
///
/// **One instrument, never a group.** This used to widen the fit until
/// it framed *every* face carrying the station at once, on the reading
/// that "the station is the instrument, not the piece". That reading
/// died with the singleton ship. A station rides cargo, cargo lives in
/// rooms, and a room that came alongside can put a second one of your
/// instruments a whole hull away — a market's shelf stocks a chart tank
/// like it stocks anything else (`barter::stock_kind`), and the fit then
/// centred itself between two rooms and parked the eye in the hull
/// between them, looking at neither. So the fit takes the ONE face
/// nearest `from` — the body's own standing position, which is the
/// instrument the player walked up to and clicked — and frames that.
/// Two tanks on one wall now focus the one you are in front of, which is
/// the one you asked for either way.
#[must_use]
pub fn focus_pose(
    focus: Focus,
    panels: &[(Station, SimSurface)],
    from: Vec3,
) -> Option<(Vec3, Quat)> {
    let face = panels
        .iter()
        .filter(|(station, _)| Focus::of(*station) == Some(focus))
        .map(|(_, surface)| surface)
        .min_by(|a, b| {
            a.center
                .distance_squared(from)
                .total_cmp(&b.center.distance_squared(from))
        })?;
    let v = face.half_v.normalize();
    let half_w = face.half_u.length() + PLATE_MARGIN;
    let half_h = face.half_v.length() + PLATE_MARGIN;
    let aspect = CRUNCH_W as f32 / CRUNCH_H as f32;
    let half_hfov = ((FOV * 0.5).tan() * aspect).atan();
    let distance =
        (half_w * FIT_MARGIN / half_hfov.tan()).max(half_h * FIT_MARGIN / (FOV * 0.5).tan());
    let eye = face.center + face.normal() * distance;
    let look = Transform::from_translation(eye).looking_at(face.center, -v);
    Some((eye, look.rotation))
}

/// **The camera stands where a body could stand.** A focus pose that
/// lands outside every room is not a view, it is a wall — and a player
/// looking at the inside of a hull plate has no way to know the state
/// machine thinks everything is fine. Any pose this refuses is treated
/// exactly like an instrument that was carried off: there is no focus,
/// and [`pose`] walks back to roam.
///
/// The law is stated here rather than assumed by [`focus_pose`]'s
/// arithmetic, because arithmetic is what put the camera in the wall.
#[must_use]
pub fn pose_is_aboard(plan: &crate::room::Plan, eye: Vec3) -> bool {
    plan.room_at(eye).is_some()
}

/// Every live surface as a plain pair, for the pose maths — the
/// instruments' stations move, so the fit reads the world instead of a
/// constant.
fn live_panels(
    surfaces: &Query<(&Station, &SimSurface), Without<CabinCamera>>,
) -> Vec<(Station, SimSurface)> {
    surfaces
        .iter()
        .map(|(station, surface)| (*station, *surface))
        .collect()
}

/// The furnace's own lights-out floor, as a `wear::worn` glow level.
///
/// Read against the brass hardware's 0.22 of its own role: a hair
/// brighter, because a room whose every cell will destroy what stands on
/// it has to be findable from the doorway of a dark ship, and because
/// [`palette::EMBER`] over the tape's dark stripes is half a stripe of
/// glow rather than a whole face of it. Still under any real lamp: put
/// one sconce in the chamber and the tape reads as tape again.
const EMBER_FLOOR: f32 = 0.30;

/// Shared meshes and materials for the worn-metal family. Views make
/// their own phosphors; the metal is communal. Nothing ships unweathered:
/// the broad surfaces carry `wear`'s deterministic multiplier textures —
/// the 2D console's scuffed panels, grown a dimension.
#[derive(Resource)]
pub struct Skin {
    pub hull: Handle<StandardMaterial>,
    pub plate: Handle<StandardMaterial>,
    /// Desk-class metal: brushed, scuffed where hands and crates live.
    pub desk: Handle<StandardMaterial>,
    /// Painted hazard stripes for accent strips — and, in the tile
    /// vocabulary, the one class allowed to wear them (`room::tiles`).
    /// Stripes are `Consume`'s alone and `Consume` is the furnace's
    /// alone, so this handle is the furnace's iron: see the ember floor
    /// it is built with.
    pub hazard: Handle<StandardMaterial>,
    pub plate_shade: Handle<StandardMaterial>,
    pub socket: Handle<StandardMaterial>,
    pub brass: Handle<StandardMaterial>,
    pub rivet: Handle<StandardMaterial>,
    pub cube: Handle<Mesh>,
}

impl Skin {
    fn build(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        let book = crate::wear::bake(images);
        let metal = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.92,
            metallic: 0.15,
            ..default()
        };
        Self {
            hull: crate::wear::worn(materials, &book.hull, palette::HULL, 4.0, 0.92, 0.15, None),
            plate: crate::wear::worn(
                materials,
                &book.plate,
                palette::PLATE,
                2.0,
                0.92,
                0.15,
                None,
            ),
            desk: crate::wear::worn(materials, &book.desk, palette::PLATE, 2.0, 0.9, 0.15, None),
            // **Warm iron.** Hazard tape is the furnace's and nobody
            // else's, and the furnace is the one riding room that owns
            // no lamp at all — its light is whatever the crew last fed
            // it. Bank no stoke and the chamber was a black frame: the
            // hopper, the cornices and the fire door were all there and
            // none of them was findable, which is a hazard room that
            // cannot warn anybody. So its tape keeps the same
            // lights-out floor the brass below keeps, in ember rather
            // than brass, because what is banked in there is fire.
            //
            // This is emissive paint and not a lumen: it lights its own
            // face and nothing else, so the lamps-are-cargo law stands
            // exactly as written (docs/BAY.md — the clause that gives
            // the etchings and the hardware a radium glow is this same
            // clause). The fire's own `PointLight` is still the only
            // light the room ever gets, and it is still cargo, burning.
            hazard: crate::wear::worn(
                materials,
                &book.hazard,
                palette::GLINT,
                2.0,
                0.85,
                0.0,
                Some((palette::EMBER, EMBER_FLOOR)),
            ),
            plate_shade: materials.add(metal(palette::PLATE_SHADE)),
            socket: materials.add(StandardMaterial {
                base_color: palette::SOCKET,
                perceptual_roughness: 1.0,
                metallic: 0.0,
                ..default()
            }),
            // Radium-painted hardware: the faint self-glow that keeps
            // every lever and fitting findable with the lights out —
            // the "playable on technicality" floor (BAY.md, "Lights
            // are cargo"). Dim enough to vanish under any real lamp.
            brass: materials.add(StandardMaterial {
                base_color: palette::BRASS,
                perceptual_roughness: 0.45,
                metallic: 0.8,
                emissive: palette::BRASS.to_linear() * 0.22,
                ..default()
            }),
            rivet: materials.add(metal(palette::RIVET)),
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        }
    }
}

/// The in-cabin camera (renders to the crunch target).
#[derive(Component)]
pub struct CabinCamera;

/// Cabin lights the omen may dim, remembering their honest brightness.
#[derive(Component)]
pub struct Dimmable {
    pub intensity: f32,
}

/// The roaming crosshair dot (UI, hidden while focused).
#[derive(Component)]
pub struct Crosshair;

/// One bay berth well. The grid is placement furniture, not wall decor:
/// [`fade_tiles`] shows it only while a carry is live, so an idle bay
/// reads as a furnished room instead of a warehouse diagram.
#[derive(Component)]
pub struct BerthTile;

/// The berth wells' shared translucent ink and its eased level. One ink
/// for every room aboard: the grid is one answer to one question.
#[derive(Resource)]
pub struct TileFade {
    pub mat: Handle<StandardMaterial>,
    level: f32,
}

/// How fast the berth grid answers a grab, per second — feedback, so it
/// finishes well inside the half-second law.
const TILE_FADE_RATE: f32 = 6.0;

// The glint frame that used to invite a hull panel's focus retired with
// the hull panels. The invitation itself did not: an instrument is cargo,
// and `pieces::hover_glint` lights the piece's own footprint frame — the
// tell now belongs to the thing you could also pick up.

/// Spawn the whole static cabin: crunch pipeline, camera, structure,
/// bay furniture, lights, version text.
#[allow(clippy::too_many_lines)]
pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    rig: Res<CameraRig>,
) {
    let skin = Skin::build(&mut meshes, &mut materials, &mut images);

    // --- The crunch: a small render target shown fullscreen, unsmoothed.
    let mut target = Image::new_target_texture(
        CRUNCH_W,
        CRUNCH_H,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    target.sampler = ImageSampler::nearest();
    let target = images.add(target);

    // A booted focus (`--view`) has no instrument to aim at yet — every
    // station rides a piece, and the riding surfaces are hung on the
    // first frame — so the camera opens on the roaming pose and `pose`
    // snaps it home a frame later.
    let (pos, rot) = (rig.pos, rig.roam_rotation());
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(palette::VOID),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: FOV,
                ..default()
            }),
            RenderTarget::Image(target.clone().into()),
            Hdr,
            Bloom::NATURAL,
            Msaa::Off,
            Transform::from_translation(pos).with_rotation(rot),
            CabinCamera,
        ))
        .id();
    if rig.drydock {
        // The drydock view (`--view drydock`): dev tooling that lets the
        // cabin camera see the VOID layer as well as its own room, and
        // parks it outside. The exterior is normally the porthole's
        // alone; this is the one way to look at the ship's own shells
        // without standing at a window, and it exists so a geometry
        // change to `viewport::hull_outside` can be *looked* at. The
        // cabin's fog stays off with it: hull-toned haze at twelve
        // metres would grey out the very thing being inspected.
        commands
            .entity(camera)
            .insert(RenderLayers::from_layers(&[0, crate::viewport::VOID_LAYER]));
    } else {
        // A breath of particulate: gentle depth haze in hull tones. The
        // far corners of the cabin soften; panels up close stay crisp.
        commands.entity(camera).insert(DistanceFog {
            color: palette::HULL.with_alpha(0.85),
            falloff: FogFalloff::Exponential { density: 0.16 },
            ..default()
        });
    }
    commands.spawn(Camera2d);
    commands.spawn((
        ImageNode::new(target),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            ..default()
        },
        // The crunch paints under every other UI root; root order alone
        // is not a stacking guarantee.
        GlobalZIndex(-1),
    ));

    // The roaming crosshair: a small glint dot, dead center, UI-side so
    // it stays crisp. Not text; barely a shape. A full-screen flex
    // container centers it exactly regardless of window size.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            Visibility::Visible,
            GlobalZIndex(1),
            Crosshair,
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: px(4),
                    height: px(4),
                    ..default()
                },
                BackgroundColor(palette::GLINT.with_alpha(0.65)),
            ));
        });

    // The game's one piece of text: the version, bottom-right, outside
    // the crunch — dev information, not part of the fiction.
    commands.spawn((
        Text::new(format!("space-trucking cabin {}", space_trucking::VERSION)),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(palette::VERSION_TEXT),
        Node {
            position_type: PositionType::Absolute,
            right: px(8),
            bottom: px(8),
            ..default()
        },
        GlobalZIndex(2),
    ));

    // --- Structure: every axis-aligned mass, from the one data source.
    for slab in structure() {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.hull.clone()),
            Transform::from_translation(slab.center).with_scale(slab.size),
        ));
    }
    // Ceiling pipes: oriented decor, outside the slab list on purpose.
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.09, 4.3))),
        MeshMaterial3d(skin.plate_shade.clone()),
        Transform::from_xyz(-1.90, 2.18, 0.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        // Brass, because somebody salvaged this line from a nicer ship.
        Mesh3d(meshes.add(Cylinder::new(0.05, 4.3))),
        MeshMaterial3d(skin.brass.clone()),
        Transform::from_xyz(1.97, 2.2, 0.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // --- No panels. Nothing to spawn here: the plates, their glint aim
    // frames, and the station tags that went with them all belonged to
    // surfaces the hull owned, and the hull owns none. Every `SimSurface`
    // entity in the game is hung by `room` (the charts) or by `pieces`
    // (an instrument's own riding face).

    // --- The bay: the cabin's own share of the room net. The charts
    // themselves, the berth wells, and every colored tile are `room`'s
    // now — one code path for every room aboard, spawned and retired with
    // the graph. What stays here is the cabin's own furniture: the backer
    // plates behind its aft wall and deck, the gantry, and the hazard lip
    // along the front gutter, all derived from the same charts so a
    // retuned bay still moves as one thing.
    //
    // The berth wells' one translucent ink lives here because it is one
    // ink for the whole ship: `fade_tiles` raises every room's grid
    // together, since the grid answers one question and there is only one.
    let tile_mat = materials.add(StandardMaterial {
        base_color: palette::SOCKET.with_alpha(0.0),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.insert_resource(TileFade {
        mat: tile_mat,
        level: 0.0,
    });
    for (station, surface) in bay() {
        // Backer plate: a worn slab just behind the mapped quad — except
        // on the wall and ceiling charts, whose backing IS the hull the
        // structure already built; a second slab would swallow the trim.
        //
        // It is punched by the cabin's own apertures for the same reason
        // the hull is: a backer that spanned the whole aft wall would
        // quietly board up every doorway cut through it, which is exactly
        // the defect this pass found by looking through one.
        if matches!(station, Station::BayWall | Station::BayFloor) {
            let n = station.inward(&surface);
            // The plate rides `layer::BACKER`, and it is thin on purpose:
            // a slab thick enough to reach the hull behind it gets sliced
            // at the hull's own plane by the aperture punch, and the
            // remainder's face and the hull's face are then one plane —
            // the deck's flicker. Face a step under the chart, back still
            // clear of the hull, and the punch has nothing to slice.
            let deep = layer::BACKER_T;
            // Overlap along the chart's u only. Growing along v would
            // hang the aft plate's skirt below the deck, where the
            // doorway punch would cut it off flush with the floor and
            // put two upward faces on one plane all over again.
            let flat = Vec3::new(
                surface.half_u.length().mul_add(2.0, 0.08),
                surface.half_v.length() * 2.0,
                deep,
            );
            // The charts are axis-aligned, so the plate's world extent is
            // its own frame's, spun onto the world axes.
            let size = (surface.orientation() * flat).abs();
            let center = surface.center - n * deep.mul_add(0.5, layer::BACKER);
            let material = match station {
                Station::BayFloor => skin.desk.clone(),
                _ => skin.plate.clone(),
            };
            let mut parts = vec![(center, size)];
            for (lo, hi) in crate::room::cabin_holes() {
                parts = parts
                    .into_iter()
                    .flat_map(|(c, s)| crate::room::punch(c, s, lo, hi))
                    .collect();
            }
            for (c, s) in parts {
                commands.spawn((
                    Mesh3d(skin.cube.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(c).with_scale(s),
                ));
            }
        }
    }
    {
        let charts = bay();
        let (_, wall) = charts[0];
        let wall_top = wall.center.y + wall.half_v.length();
        // Gantry: a top rail above row 0 and stiles down the seams to the
        // side walls — the rack the fixtures visibly mount to, grown to
        // room scale.
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.plate_shade.clone()),
            Transform::from_translation(Vec3::new(0.0, wall_top + 0.04, BAY_WALL_Z - 0.02))
                .with_scale(Vec3::new(BAY_W + 0.14, 0.07, 0.09)),
        ));
        for sx in [-1.0, 1.0] {
            commands.spawn((
                Mesh3d(skin.cube.clone()),
                MeshMaterial3d(skin.plate_shade.clone()),
                Transform::from_translation(Vec3::new(
                    sx * BAY_W.mul_add(0.5, 0.035),
                    (wall_top + 0.06) * 0.5,
                    BAY_WALL_Z - 0.015,
                ))
                .with_scale(Vec3::new(0.06, wall_top + 0.06, 0.08)),
            ));
        }
        // No hazard lip. There was one along the floor chart's front
        // edge — "the painted line between the front gutter and where
        // cargo lives" — and it retired with the gutter it was drawing
        // the edge of. Hazard tape means *this will hurt you*
        // (`docs/ART_DIRECTION_3D.md`, and `Consume` is its one
        // claimant); across the front of a deck the player is now meant
        // to walk on and berth cargo on, it was a warning about nothing.
        // The doormat that used to be hand-listed here is the threshold
        // TILE CLASS now: `room::tiles` stripes every aperture's own
        // cells, in every room, straight off the sim's declaration.
    }

    // --- Light: NONE of the ship's own. Every lumen aboard is cargo —
    // the starter ceiling lamp, sconces, floor lamps, luminous coats,
    // the firebox — and the omen dims them all through `Dimmable`.
    // Lights-out is therefore a legal, playable state: the instruments
    // are emissive (screens, phosphor readings, icon etchings, the
    // radium-painted brass) and carry the game on technicality while
    // the room itself goes black. No shadow maps anywhere, on purpose:
    // DESIGN.md's lighting direction is light *volumes* — authored,
    // placed light, not simulated occlusion.

    // Starlight through the pane: an ambient floor low enough that a
    // lampless room reads as darkness, high enough that silhouettes
    // survive it. The one light the player cannot trade away.
    commands.insert_resource(GlobalAmbientLight {
        color: palette::PLATE_LIT,
        brightness: 16.0,
        ..default()
    });

    commands.insert_resource(skin);
}

/// The focusable station the roaming crosshair rests on, if any: a ray
/// straight out of the camera against the panel quads. Bay surfaces are
/// skipped — they are worked in roam, never focused — but they also
/// never occlude a panel (they hang on the opposite wall), so skipping
/// is a filter, not an occlusion cheat.
///
/// **Within [`REACH`], like everything else the crosshair works.** A
/// station is hardware you walk up to — the same law the berths and the
/// detach latch already keep — and the arm's length matters twice: a
/// glint frame that invited from across the room was a promise the
/// camera made for the whole far wall, and a CLICK that resolved to a
/// station four metres off was a click the camera ate. `steer` spends
/// this answer before the carry sees the press, so an unbounded one is
/// a grab quietly swallowed by a panel the player was not near.
fn aimed_station(
    camera: &Transform,
    surfaces: &Query<(&Station, &SimSurface), Without<CabinCamera>>,
) -> Option<Station> {
    let ray = Ray3d::new(camera.translation, Dir3::new(camera.forward().into()).ok()?);
    let mut best: Option<(f32, Station)> = None;
    for (station, surface) in surfaces {
        if Focus::of(*station).is_none() {
            continue;
        }
        if let Some((t, _, _)) = surface.project(ray)
            && t <= REACH
            && best.is_none_or(|(bt, _)| t < bt)
        {
            best = Some((t, *station));
        }
    }
    best.map(|(_, station)| station)
}

/// The handle rule's click routing (BAY.md, "The handle rule"), pure:
/// where the crosshair rests on a click-functional piece, a press
/// inside its declared amber handle is a CARRY and passes to the sim
/// untouched; anywhere else on that piece it is the instrument's focus
/// interaction and the camera takes the click instead. Passive cargo
/// has no function to guard, so its whole body grabs and nothing is
/// consumed.
#[must_use]
pub fn handle_route(pieces: &[Piece], at: SimVec2) -> Option<Focus> {
    let piece = layout::piece_at(pieces, at)?;
    let handle = crate::pieces::carry_handle_rect(piece.kind, layout::piece_rect(pieces, piece))?;
    // Off its wall — staged on a hopper tile, boxed in a cubby, laid
    // on the deck — an instrument is only cargo again: it carries no
    // station, so its whole body grabs, handle or no handle.
    if handle.contains(at) || !matches!(piece.loc, Loc::Hold { .. }) {
        return None;
    }
    Focus::of(crate::pieces::instrument(piece.kind)?.station)
}

/// Mode transitions and roaming movement, from this frame's input.
#[allow(clippy::too_many_arguments)]
pub fn steer(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    surfaces: Query<(&Station, &SimSurface), Without<CabinCamera>>,
    pointer: Res<crate::surface::VirtualPointer>,
    shell: Res<crate::Shell>,
    envelope: Res<crate::room::Envelope>,
    menu: Res<crate::menu::Menu>,
    mut rig: ResMut<CameraRig>,
    camera: Single<&Transform, With<CabinCamera>>,
) {
    // While the menu stands it owns the keyboard and the cursor: no look,
    // no walk, no glide, and no click reclaiming the pointer out from
    // under a control the player is aiming at. `menu::keys` runs ahead of
    // this in `Phase::Input` and took the `Esc` that opened it.
    if menu.open {
        return;
    }
    let toggle = keys.just_pressed(KeyCode::KeyE);
    match rig.mode {
        Mode::Roam => {
            // A parked cursor belongs to the OS until the game is
            // clicked; the click that reclaims it does nothing else.
            if rig.parked {
                if buttons.just_pressed(MouseButton::Left) {
                    rig.parked = false;
                }
                return;
            }
            // The Super/Windows key summons the OS — the overlay may not
            // flip `window.focused`, so treat the key itself as a park:
            // the cursor is the desktop's now, click to reclaim. `Esc`
            // parks too, but by way of the menu (`menu::keys`): the
            // cursor still goes free, it just has something to click.
            if keys.just_pressed(KeyCode::SuperLeft) || keys.just_pressed(KeyCode::SuperRight) {
                rig.parked = true;
                return;
            }
            // Look.
            rig.yaw = motion.delta.x.mul_add(-LOOK_SPEED, rig.yaw);
            rig.pitch = motion
                .delta
                .y
                .mul_add(-LOOK_SPEED, rig.pitch)
                .clamp(-PITCH_LIMIT, PITCH_LIMIT);
            // Walk, on the yaw plane.
            let mut step = Vec3::ZERO;
            let forward = Quat::from_rotation_y(rig.yaw) * Vec3::NEG_Z;
            let right = Quat::from_rotation_y(rig.yaw) * Vec3::X;
            for (key, dir) in [
                (KeyCode::KeyW, forward),
                (KeyCode::KeyS, -forward),
                (KeyCode::KeyA, -right),
                (KeyCode::KeyD, right),
            ] {
                if keys.pressed(key) {
                    step += dir;
                }
            }
            if step != Vec3::ZERO {
                // The envelope is the ship's, not the cabin's: per-room
                // boxes joined at every mated doorway, hull collision
                // only. A step that leaves the union slides along it, so
                // walking into a jamb glances off instead of stopping —
                // and walking THROUGH one carries you into the next room.
                let want = rig.pos + step.normalize() * WALK_SPEED * time.delta_secs();
                rig.pos = if envelope.holds(want) {
                    want
                } else {
                    let slid = [
                        Vec3::new(want.x, want.y, rig.pos.z),
                        Vec3::new(rig.pos.x, want.y, want.z),
                    ];
                    slid.into_iter()
                        .find(|p| envelope.holds(*p))
                        .unwrap_or_else(|| envelope.nearest(rig.pos))
                };
            }
            // The eye ducks through a doorway and stands up again in the
            // room beyond: an aperture is two courses of the CARGO grid,
            // which makes it a hatch rather than a hall, and the sim's
            // declaration is not going to bend for a neck.
            let stance = if envelope.ducking(rig.pos) {
                DUCK_HEIGHT
            } else {
                EYE_HEIGHT
            };
            let bend = DUCK_RATE * time.delta_secs();
            rig.pos.y += (stance - rig.pos.y).clamp(-bend, bend);
            // Focus what the crosshair rests on — cargo in hand
            // included: the click carries the piece along (the carry
            // survives the glide; `advance` keeps the grip synthesized),
            // and because this runs before `advance`, the same click
            // never doubles as a placement.
            //
            // An instrument in the room answers to the handle rule
            // first. The pointer is last frame's, computed from exactly
            // the camera transform this system is reading, so the
            // routing and the hover tell that promised it can never
            // disagree about which half of the piece the aim is on.
            if buttons.just_pressed(MouseButton::Left) || toggle {
                let holding = shell.bridge.sim.held(0).is_some();
                let over = layout::piece_at(shell.bridge.sim.pieces(), pointer.sim);
                let focus = if !holding && over.is_some() {
                    handle_route(shell.bridge.sim.pieces(), pointer.sim)
                } else {
                    aimed_station(&camera, &surfaces).and_then(Focus::of)
                };
                if let Some(focus) = focus {
                    rig.mode = Mode::ToFocus {
                        focus,
                        from: (camera.translation, camera.rotation),
                        t: 0.0,
                    };
                }
            }
        }
        // **The way out is the same three keys from anywhere that is not
        // roam, including mid-glide.** A glide is sub-half-second and
        // used to swallow input on that argument, which is fine right up
        // until the pose at the far end of it is somewhere the player
        // cannot read — and then a swallowed `Esc` is the difference
        // between a bad camera and a soft-lock. `Mode::ToRoam` is
        // already on its way home and needs no second answer.
        Mode::Focused { .. } | Mode::ToFocus { .. } => {
            if toggle
                || keys.just_pressed(KeyCode::Escape)
                || buttons.just_pressed(MouseButton::Right)
            {
                rig.mode = Mode::ToRoam {
                    from: (camera.translation, camera.rotation),
                    t: 0.0,
                };
            }
        }
        Mode::ToRoam { .. } => {}
    }
}

/// Advance glides and write the camera transform for the current mode.
///
/// The poses come from the surfaces standing in the room this frame,
/// not from a constant: an instrument's station rides its cargo, so a
/// tank carried off mid-glide simply has no pose left — the camera
/// lets go and walks back (`Mode::ToRoam`) rather than aiming at a
/// hole in the wall.
///
/// A pose that would put the eye outside every room counts as no pose at
/// all ([`pose_is_aboard`]), and takes the same way out. The camera is
/// never anywhere the body could not be.
pub fn pose(
    time: Res<Time>,
    plan: Res<crate::room::Plan>,
    surfaces: Query<(&Station, &SimSurface), Without<CabinCamera>>,
    mut rig: ResMut<CameraRig>,
    mut camera: Single<&mut Transform, With<CabinCamera>>,
) {
    let panels = live_panels(&surfaces);
    let dt = time.delta_secs();
    let (roam_pos, roam_rot) = (rig.pos, rig.roam_rotation());
    // The fit is measured from where the BODY stands, not from where the
    // camera happens to have got to: mid-glide the camera is nowhere in
    // particular, and a fit that chased it would pick a different
    // instrument every frame.
    let aim =
        |focus| focus_pose(focus, &panels, roam_pos).filter(|(eye, _)| pose_is_aboard(&plan, *eye));
    // The instrument left with its cargo, or the pose it wants is not a
    // place to stand: let go of the focus here and walk back from
    // wherever the glide had got to.
    let orphaned = match rig.mode {
        Mode::Focused { focus } | Mode::ToFocus { focus, .. } => aim(focus).is_none(),
        Mode::Roam | Mode::ToRoam { .. } => false,
    };
    if orphaned {
        rig.mode = Mode::ToRoam {
            from: (camera.translation, camera.rotation),
            t: 0.0,
        };
    }
    let (pos, rot) = match &mut rig.mode {
        Mode::Roam => (roam_pos, roam_rot),
        Mode::Focused { focus } => aim(*focus).unwrap_or((roam_pos, roam_rot)),
        Mode::ToFocus { focus, from, t } => {
            let (to_pos, to_rot) = aim(*focus).unwrap_or((roam_pos, roam_rot));
            *t = (*t + dt / GLIDE).min(1.0);
            let s = smooth(*t);
            let out = (from.0.lerp(to_pos, s), from.1.slerp(to_rot, s));
            if *t >= 1.0 {
                let focus = *focus;
                rig.mode = Mode::Focused { focus };
            }
            out
        }
        Mode::ToRoam { from, t } => {
            *t = (*t + dt / GLIDE).min(1.0);
            let s = smooth(*t);
            let out = (from.0.lerp(roam_pos, s), from.1.slerp(roam_rot, s));
            if *t >= 1.0 {
                rig.mode = Mode::Roam;
            }
            out
        }
    };
    camera.translation = pos;
    camera.rotation = rot;
}

/// Cursor grab and crosshair follow the mode.
pub fn present_mode(
    rig: Res<CameraRig>,
    mut window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut crosshair: Single<&mut Visibility, With<Crosshair>>,
    mut was_focused: Local<bool>,
) {
    let (window, cursor) = &mut *window;
    let roaming = matches!(rig.mode, Mode::Roam);
    let focused = rig.interactive();
    if roaming && window.focused && !rig.parked {
        // Windows' winit cannot Lock the cursor, only Confine it — and a
        // confined, hidden cursor still wanders (to the taskbar, where a
        // click steals the window), so it gets pinned to center every
        // frame instead. Look input reads raw deltas and never notices.
        if cfg!(target_os = "windows") {
            cursor.grab_mode = CursorGrabMode::Confined;
            // Warp only while the cursor is actually ours (inside the
            // window) — never wrestle an OS overlay for it.
            if window.cursor_position().is_some() {
                let center = window.size() * 0.5;
                window.set_cursor_position(Some(center));
            }
        } else {
            cursor.grab_mode = CursorGrabMode::Locked;
        }
        cursor.visible = false;
    } else {
        // Focused stations and unfocused windows both hand the cursor
        // back — the player must always be able to click their way home.
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    if focused && !*was_focused {
        // Hand the freed cursor to the player mid-panel, not wherever the
        // lock left it.
        let center = window.size() * 0.5;
        window.set_cursor_position(Some(center));
    }
    *was_focused = focused;
    // The crosshair marks a live aim, so it goes with the lock: a freed
    // cursor (parked to the desktop, or standing in front of the menu)
    // has a real pointer to aim with and does not want a second one
    // painted at screen centre.
    **crosshair = if rig.roaming() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

/// The glide's easing: smoothstep, no overshoot.
fn smooth(t: f32) -> f32 {
    t * t * 2.0f32.mul_add(-t, 3.0)
}

/// Raise the berth wells while a carry is live and sink them after: the
/// grid is an answer to "where can this go?", so it appears when the
/// question does. One shared ink fades every tile as one.
pub fn fade_tiles(
    time: Res<Time>,
    shell: Res<crate::Shell>,
    fade: Option<ResMut<TileFade>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut fade) = fade else { return };
    let target = f32::from(u8::from(shell.bridge.sim.held(0).is_some()));
    let step = time.delta_secs() * TILE_FADE_RATE;
    fade.level = if fade.level < target {
        (fade.level + step).min(target)
    } else {
        (fade.level - step).max(target)
    };
    if let Some(mut mat) = materials.get_mut(&fade.mat) {
        mat.base_color = palette::SOCKET.with_alpha(smooth(fade.level));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use space_trucking::sim::Vec2 as SimVec2;

    /// Nearest positive parameter where a ray enters a slab, if any.
    /// `dir` need not be normalized — parameters are in units of `dir`.
    fn ray_slab_entry(origin: Vec3, dir: Vec3, slab: &Slab) -> Option<f32> {
        let half = slab.size * 0.5;
        let (lo, hi) = (slab.center - half, slab.center + half);
        let mut t_min = f32::NEG_INFINITY;
        let mut t_max = f32::INFINITY;
        for axis in 0..3 {
            let (o, d) = (origin[axis], dir[axis]);
            let (a, b) = (lo[axis], hi[axis]);
            if d.abs() < 1e-9 {
                if o < a || o > b {
                    return None;
                }
            } else {
                let (t1, t2) = ((a - o) / d, (b - o) / d);
                t_min = t_min.max(t1.min(t2));
                t_max = t_max.min(t1.max(t2));
            }
        }
        (t_max >= t_min && t_max > 0.0).then(|| t_min.max(0.0))
    }

    // The plate-ray helper retired with the plates: no surface in the
    // cabin has a physical panel slab behind it any more, so there is
    // nothing but hull left to occlude a sightline.

    /// The sightline rule the screenshots check by eye, made mechanical:
    /// `point` must sit inside the camera frustum AND nothing structural
    /// may stand between the eye and it.
    fn visible_from(eye: Vec3, rot: Quat, point: Vec3, slabs: &[Slab]) -> Result<(), String> {
        // Frustum containment, using the pinned FOV and crunch aspect.
        let local = rot.inverse() * (point - eye);
        if local.z >= -0.01 {
            return Err(format!("{point} is behind the eye at {eye}"));
        }
        let depth = -local.z;
        let half_v = (FOV * 0.5).tan();
        let half_h = half_v * (CRUNCH_W as f32 / CRUNCH_H as f32);
        if local.x.abs() > depth * half_h || local.y.abs() > depth * half_v {
            return Err(format!("{point} falls outside the frustum from {eye}"));
        }
        // Occlusion: nothing structural may enter the segment eye→point.
        let dir = point - eye;
        for slab in slabs {
            if let Some(t) = ray_slab_entry(eye, dir, slab)
                && t < 1.0 - 1e-3
            {
                return Err(format!(
                    "slab at {} blocks the line from {eye} to {point} (t={t:.3})",
                    slab.center
                ));
            }
        }
        Ok(())
    }

    /// A panel's must-see set: quad corners (nudged 2% inward so the
    /// test speaks about the face, not the trim) plus the center.
    fn corner_points(surface: &SimSurface) -> Vec<Vec3> {
        let mut points = vec![surface.center];
        for su in [-0.98f32, 0.98] {
            for sv in [-0.98f32, 0.98] {
                points.push(surface.center + surface.half_u * su + surface.half_v * sv);
            }
        }
        points
    }

    /// Every interactive control a station carries, as sim rect centers
    /// mapped onto its surface — the exact spots a click must reach.
    fn control_points(station: Station, surface: &SimSurface) -> Vec<Vec3> {
        let mid = |r: layout::Rect| SimVec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y));
        let mut spots: Vec<SimVec2> = Vec::new();
        match station {
            Station::Map => {
                spots.push(space_trucking::sim::map::SUN);
            }
            // The pull is the whole panel's business, so its rect's
            // ends matter as much as its middle: a grip that starts at
            // the left stop must be reachable, and the detent at the
            // right end is where the throw actually fires.
            Station::Lever => {
                let r = layout::LAUNCH_LEVER;
                spots.push(mid(r));
                spots.push(SimVec2::new(r.x + 1.0, r.h.mul_add(0.5, r.y)));
                spots.push(SimVec2::new(r.x + r.w - 1.0, r.h.mul_add(0.5, r.y)));
            }
            Station::BayWall
            | Station::BayFloor
            | Station::BayPort
            | Station::BayStarboard
            | Station::BayFront
            | Station::BayCeiling
            | Station::Handshake
            | Station::Standing => {}
        }
        spots.into_iter().map(|s| surface.to_world(s)).collect()
    }

    /// Every station standing in the starter cabin — **all of them
    /// riding cargo**, since the hull owns no panel to carry one.
    /// Derived from the sim's opening board through the very function
    /// the runtime rides them with, so moving a starter berth moves
    /// these tests with it.
    fn stations() -> Vec<(Station, SimSurface)> {
        let sim = space_trucking::sim::Sim::new(1);
        let charts = bay();
        sim.pieces()
            .iter()
            .filter_map(|piece| {
                matches!(piece.loc, Loc::Hold { .. })
                    .then(|| {
                        crate::pieces::instrument_surface(
                            &charts,
                            piece.kind,
                            layout::piece_rect(sim.pieces(), piece),
                        )
                    })
                    .flatten()
            })
            .collect()
    }

    /// The sightline contract: from a station's own focus viewpoint,
    /// every panel corner and every control it carries must be visible —
    /// framed and unoccluded. This is the "corner must be visible from
    /// the perspective" rule, enforced at build time.
    ///
    /// **The hull is the only thing that may never be in the way**, and
    /// now it is the only thing that could be: the furniture that used
    /// to shoulder into sightlines is gone with the counter and the
    /// console face. Cargo standing in a sightline is nobody's
    /// business here — the focus x-ray ghosts it at runtime.
    #[test]
    fn every_control_is_visible_from_its_focus() {
        let slabs = structure();
        for (station, surface) in stations() {
            let focus = Focus::of(station).expect("every station is focusable");
            let (eye, rot) =
                focus_pose(focus, &stations(), BOOT_EYE).expect("every station has a pose");
            let mut points = corner_points(&surface);
            points.extend(control_points(station, &surface));
            for point in points {
                // Lift each point a hair off the face so the ray test
                // asks about the air in front of it, not the face itself.
                let probe = point + surface.normal() * 0.004;
                if let Err(reason) = visible_from(eye, rot, probe, &slabs) {
                    panic!("{station:?} sightline broken: {reason}");
                }
            }
        }
    }

    // The "no slab swallows a panel face" regression retired with the
    // panels: there is no hull-owned face left for a slab to eat, and a
    // riding instrument's face moves with its cargo, which the placement
    // rules (not the hull) answer for.

    /// Focus viewpoints must be legal camera positions: inside the box,
    /// inside no slab, looking at their stations — every one of them a
    /// riding pose, since every station is wherever its cargo is.
    #[test]
    fn focus_poses_are_legal_camera_positions() {
        let slabs = structure();
        let stations = stations();
        for focus in [Focus::Tank, Focus::Lever] {
            let (eye, rot) =
                focus_pose(focus, &stations, BOOT_EYE).expect("the starter board hangs it");
            assert!(
                eye.y > 0.2 && eye.y < 2.2 && eye.x.abs() < 2.15 && eye.z > -1.85 && eye.z < 2.35,
                "{focus:?} eye {eye} left the cabin"
            );
            for slab in &slabs {
                assert!(
                    !slab.contains(eye, 0.0),
                    "{focus:?} eye {eye} is inside a slab at {}",
                    slab.center
                );
            }
            // The view axis must run into the instrument it framed: the
            // nearest face carrying that station, which is the one the
            // body is standing in front of.
            let framed = stations
                .iter()
                .filter(|(station, _)| Focus::of(*station) == Some(focus))
                .min_by(|(_, a), (_, b)| {
                    a.center
                        .distance_squared(BOOT_EYE)
                        .total_cmp(&b.center.distance_squared(BOOT_EYE))
                })
                .expect("the starter board hangs it");
            let to_panel = (framed.1.center - eye).normalize();
            let forward = rot * Vec3::NEG_Z;
            assert!(
                forward.dot(to_panel) > 0.7,
                "{focus:?} does not face {:?}",
                framed.0
            );
        }
    }

    /// A station with nothing carrying it has no pose at all, and the
    /// camera falls back to roam rather than aiming at a bare wall: an
    /// instrument is cargo, and cargo can be sold. **Every focus is
    /// jettisonable now** — sell the lot and the cabin keeps not one
    /// focusable surface, which is the ship this pass finished building.
    #[test]
    fn a_jettisoned_instrument_leaves_no_pose() {
        for focus in [Focus::Tank, Focus::Lever] {
            assert!(
                focus_pose(focus, &[], BOOT_EYE).is_none(),
                "{focus:?} found a pose on a hull that owns no panels"
            );
        }
    }

    /// Where the body stands when the fit is measured, for the tests
    /// that do not walk: the boot pose, mid-cabin.
    const BOOT_EYE: Vec3 = Vec3::new(0.0, EYE_HEIGHT, 0.9);

    /// Every mapped surface a running ship would stand up, from a save
    /// alone: each room's six charts, and the stations riding cargo in
    /// whichever room the cargo is berthed in.
    fn world_panels(sim: &space_trucking::sim::Sim) -> Vec<(Station, SimSurface)> {
        let mut charts: Vec<(Station, SimSurface)> = Vec::new();
        for (id, room) in sim.rooms().iter() {
            charts.extend(crate::room::charts(id, room));
        }
        let mut panels = charts.clone();
        for piece in sim.pieces() {
            if !matches!(piece.loc, Loc::Hold { .. }) {
                continue;
            }
            let rect = layout::piece_rect(sim.pieces(), piece);
            if let Some(pair) = crate::pieces::instrument_surface(&charts, piece.kind, rect) {
                panels.push(pair);
            }
        }
        panels
    }

    /// The plan a running ship would hold, from a save alone.
    fn world_plan(sim: &space_trucking::sim::Sim) -> crate::room::Plan {
        let mut plan = crate::room::Plan::default();
        plan.rooms = sim
            .rooms()
            .iter()
            .map(|(id, room)| crate::room::placed(id, room))
            .collect();
        plan
    }

    /// **A focus lands in the room that holds the instrument.**
    ///
    /// The playtest's soft-lock, mechanised. A station rides cargo and
    /// cargo lives in rooms, so the moment a room comes alongside the
    /// game can hold two faces answering as one station — a market's
    /// shelf stocks chart tanks and launch levers like it stocks
    /// anything else. The old fit averaged them, and the average of a
    /// face in your cabin and a face in somebody's market is a point in
    /// the hull between the two: black screen, free cursor, no crosshair,
    /// and nothing on screen to say the state machine is fine.
    ///
    /// Seed 40 is a real, unremarkable first dock that does exactly this.
    #[test]
    fn a_focus_lands_in_the_room_that_holds_the_instrument() {
        use space_trucking::sim::Sim;

        let sim = Sim::new(40);
        let cabin = space_trucking::sim::room::CABIN;
        assert!(
            sim.pieces()
                .iter()
                .filter(|piece| !matches!(piece.loc, Loc::Hold { room: 0, .. }))
                .any(|piece| crate::pieces::instrument(piece.kind).is_some()),
            "seed 40 is supposed to put an instrument in a room that is not the cabin"
        );
        let panels = world_panels(&sim);
        let plan = world_plan(&sim);
        // The condition the old fit could not survive, stated: one
        // station, two faces, two rooms.
        let split = [Focus::Tank, Focus::Lever].into_iter().any(|focus| {
            let rooms: Vec<Option<space_trucking::sim::room::RoomId>> = panels
                .iter()
                .filter(|(station, _)| Focus::of(*station) == Some(focus))
                .map(|(_, surface)| plan.room_at(surface.center))
                .collect();
            rooms.len() > 1 && rooms.iter().any(|room| *room != rooms[0])
        });
        assert!(
            split,
            "seed 40 no longer splits a station across two rooms, so this guards nothing"
        );
        for focus in [Focus::Tank, Focus::Lever] {
            let Some((eye, _)) = focus_pose(focus, &panels, BOOT_EYE) else {
                continue;
            };
            assert!(
                pose_is_aboard(&plan, eye),
                "{focus:?} parks the eye at {eye} — inside no room at all"
            );
            assert_eq!(
                plan.room_at(eye),
                Some(cabin),
                "{focus:?} left the cabin to frame somebody else's stock"
            );
        }
    }

    /// The roaming envelope stays clear of every slab at eye height.
    #[test]
    fn walk_envelope_is_clear() {
        let slabs = structure();
        for i in 0..=10 {
            for j in 0..=10 {
                let p = Vec3::new(
                    (i as f32 / 10.0).mul_add(WALK_MAX.x - WALK_MIN.x, WALK_MIN.x),
                    EYE_HEIGHT,
                    (j as f32 / 10.0).mul_add(WALK_MAX.z - WALK_MIN.z, WALK_MIN.z),
                );
                for slab in &slabs {
                    assert!(
                        !slab.contains(p, 0.0),
                        "walk point {p} is inside a slab at {}",
                        slab.center
                    );
                }
            }
        }
    }

    /// The net's charts land their seam cells where the room glues
    /// them. FOUR folds are physically watertight now — aft, port,
    /// starboard and front all meet the floor at their baseboards,
    /// because the front gutter that used to hold that seam open is
    /// gone. One declared trim seam is left: the ceiling chart sits a
    /// trim band above the starboard cornice (BAY.md).
    #[test]
    fn chart_seams_are_watertight_or_bounded_gutters() {
        let chart = |want: Station| {
            bay()
                .into_iter()
                .find(|(station, _)| *station == want)
                .map(|(_, s)| s)
                .expect("chart")
        };
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        let port = chart(Station::BayPort);
        let starboard = chart(Station::BayStarboard);
        let front = chart(Station::BayFront);
        let ceiling = chart(Station::BayCeiling);
        // A seam sample: the world gap between the two charts' images
        // of their shared edge, probed along its length.
        let gap = |a: &SimSurface,
                   a_edge: fn(&layout::Rect, f32) -> SimVec2,
                   b: &SimSurface,
                   b_edge: fn(&layout::Rect, f32) -> SimVec2| {
            (0..=6)
                .map(|i| {
                    let t = i as f32 / 6.0;
                    (a.to_world(a_edge(&a.rect, t)) - b.to_world(b_edge(&b.rect, t))).length()
                })
                .fold(0.0f32, f32::max)
        };
        let bottom = |r: &layout::Rect, t: f32| SimVec2::new(r.w.mul_add(t, r.x), r.y + r.h);
        let top = |r: &layout::Rect, t: f32| SimVec2::new(r.w.mul_add(t, r.x), r.y);
        let east = |r: &layout::Rect, t: f32| SimVec2::new(r.x + r.w, r.h.mul_add(t, r.y));
        let west = |r: &layout::Rect, t: f32| SimVec2::new(r.x, r.h.mul_add(t, r.y));
        // Watertight folds: under a cell's width of daylight.
        assert!(gap(&aft, bottom, &floor, top) < 0.10, "aft fold gapes");
        assert!(gap(&port, east, &floor, west) < 0.10, "port fold gapes");
        assert!(
            gap(&floor, east, &starboard, west) < 0.10,
            "starboard fold gapes"
        );
        assert!(gap(&floor, bottom, &front, top) < 0.10, "front fold gapes");
        // The one trim seam left: adjacent in the net, offset in the
        // room by a declared, bounded margin.
        assert!(
            gap(&starboard, east, &ceiling, west) < 0.75,
            "ceiling trim wider than declared"
        );
    }

    /// Every net cell can actually be worked: from some legal roaming
    /// position, its center is within REACH, within the pitch the neck
    /// allows, and no structure blocks the line.
    ///
    /// **The exemption is gone.** This test used to forgive cells whose
    /// only obstruction was a station panel standing in front of them —
    /// real cells the sim kept and rats hid behind, promised back as the
    /// instruments migrated off the walls (BAY.md). They migrated, the
    /// counter left, the console face left, and the promise is kept: the
    /// whole net is reachable, and any blocked cell is now a build error
    /// with no clause to hide behind.
    #[test]
    fn every_net_cell_is_workable() {
        let slabs = structure();
        for (station, surface) in bay() {
            for y in 0..layout::GRID_ROWS {
                for x in 0..layout::GRID_COLS {
                    if space_trucking::sim::RoomKind::Cabin
                        .surface_of(x, y)
                        .is_none()
                    {
                        continue;
                    }
                    let cell = layout::cell_rect(CABIN, x, y);
                    let mid =
                        SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
                    if !surface.rect.contains(mid) {
                        continue;
                    }
                    let probe = surface.to_world(mid) + station.inward(&surface) * 0.004;
                    let workable = (0..=12).any(|i| {
                        (0..=12).any(|j| {
                            let eye = Vec3::new(
                                (i as f32 / 12.0).mul_add(WALK_MAX.x - WALK_MIN.x, WALK_MIN.x),
                                EYE_HEIGHT,
                                (j as f32 / 12.0).mul_add(WALK_MAX.z - WALK_MIN.z, WALK_MIN.z),
                            );
                            let dir = probe - eye;
                            let horizontal = dir.xz().length();
                            let pitch = (-dir.y).atan2(horizontal).abs();
                            if dir.length() > REACH - 0.05 || pitch > PITCH_LIMIT - 0.02 {
                                return false;
                            }
                            // Hull in the way is a build error, and hull
                            // is all there is left to be in the way.
                            !slabs.iter().any(|slab| {
                                ray_slab_entry(eye, dir, slab).is_some_and(|t| t < 1.0 - 1e-3)
                            })
                        })
                    });
                    assert!(
                        workable,
                        "{station:?} cell ({x}, {y}) at {probe} is out of reach from everywhere"
                    );
                }
            }
        }
    }

    /// Nothing stands in a doorway. Every one of the cabin's declared
    /// apertures must be empty hull-wise: the punch is what makes a door
    /// a door, and a slab left in one is the defect this catches.
    #[test]
    fn no_slab_stands_in_a_declared_doorway() {
        let slabs = structure();
        for (port, (lo, hi)) in crate::room::cabin_holes().into_iter().enumerate() {
            // Probe the middle of the opening, and a little in from each
            // corner, so a partial cut cannot pass.
            for sx in [0.25f32, 0.5, 0.75] {
                for sy in [0.25f32, 0.5, 0.75] {
                    for sz in [0.25f32, 0.5, 0.75] {
                        let p = lo + (hi - lo) * Vec3::new(sx, sy, sz);
                        for slab in &slabs {
                            assert!(
                                !slab.contains(p, 1e-4),
                                "port {port}: hull at {} stands in the doorway at {p}",
                                slab.center
                            );
                        }
                    }
                }
            }
        }
    }

    /// The burner doorway's whole point, mechanised — and now derived:
    /// the aperture is APERTURE cells wide and tall by the sim's own
    /// declaration, and the biggest footprint in the game passes through
    /// it at bay scale, JUST. The hand-measured chamber that used to
    /// carry this promise retired with the annex.
    #[test]
    fn the_biggest_crate_just_fits_the_burner_doorway() {
        use space_trucking::sim::room::APERTURE;
        let biggest = 2.0 * BAY_CELL * crate::pieces::BAY_FIT;
        // The cabin's starboard door: two cells along the wall, two up.
        let (lo, hi) = crate::room::cabin_holes()[1];
        let clear = f32::from(APERTURE) * BAY_CELL;
        assert!(
            (hi.z - lo.z - clear).abs() < 1e-3,
            "the doorway is {} wide",
            hi.z - lo.z
        );
        assert!(
            (hi.y - lo.y - clear).abs() < 1e-3,
            "the doorway is {} tall",
            hi.y - lo.y
        );
        assert!(clear > biggest, "the biggest crate will not pass");
        assert!(
            clear < biggest + 0.10,
            "the doorway is roomier than 'just': {clear} against {biggest}"
        );
    }

    /// Every cell of the furnace room's own deck is workable from the
    /// joined envelope: within REACH, within the neck's pitch, and with
    /// no hull in the way — which is the doorway actually being open,
    /// and the body actually being able to walk through it.
    #[test]
    fn the_burner_deck_is_workable_from_the_joined_envelope() {
        use space_trucking::sim::room::{RoomKind, Rooms};
        let slabs = structure();
        let rooms = Rooms::new();
        let placed: Vec<crate::room::Placed> = rooms
            .iter()
            .map(|(id, room)| crate::room::placed(id, room))
            .collect();
        let boxes = crate::room::walk_boxes(&placed);
        let burner = placed
            .iter()
            .find(|room| room.kind == RoomKind::Burner)
            .expect("the burner rides");
        let floor = burner.chart(Station::BayFloor).expect("its deck");
        let (w, h) = RoomKind::Burner.floor();
        for j in 0..h {
            for i in 0..w {
                let cell = layout::cell_rect(burner.id, 3 + i, 3 + j);
                let mid = SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
                let probe = floor.to_world(mid) + Vec3::Y * 0.004;
                let workable = boxes.rooms.iter().any(|(lo, hi)| {
                    (0..=6u8).any(|a| {
                        (0..=6u8).any(|b| {
                            let eye = Vec3::new(
                                (f32::from(a) / 6.0).mul_add(hi.x - lo.x, lo.x),
                                EYE_HEIGHT,
                                (f32::from(b) / 6.0).mul_add(hi.z - lo.z, lo.z),
                            );
                            let dir = probe - eye;
                            let pitch = (-dir.y).atan2(dir.xz().length()).abs();
                            dir.length() <= REACH - 0.05
                                && pitch <= PITCH_LIMIT - 0.02
                                && !slabs.iter().any(|slab| {
                                    ray_slab_entry(eye, dir, slab).is_some_and(|t| t < 1.0 - 1e-3)
                                })
                        })
                    })
                });
                assert!(
                    workable,
                    "furnace cell ({i}, {j}) at {probe} is out of reach from everywhere"
                );
            }
        }
    }

    /// **The floor is floor**: a body can stand over the middle of every
    /// deck row, front row included.
    ///
    /// The playtest's dead strip, mechanised. The envelope used to stop
    /// 0.59 short of the front floor row because the cabin's front wall
    /// stood half a metre forward of its own floor box, across a gutter
    /// no net cell reached — so the front of the deck was ground you
    /// could neither walk on nor berth on. It is deck like the rest now,
    /// and this test is what stops the gutter coming back: it asks about
    /// rows rather than about margins, because a margin is a number
    /// somebody can retune and a row is a place a body stands.
    ///
    /// Columns are still stated as a margin: the outermost deck columns
    /// run under the side walls' baseboards, where a body's shoulders do
    /// not fit and never did (the workability test proves each of those
    /// cells is within working reach).
    #[test]
    fn walk_envelope_covers_the_floor() {
        let floor = bay()
            .into_iter()
            .find(|(station, _)| matches!(station, Station::BayFloor))
            .map(|(_, s)| s)
            .expect("floor chart");
        let (_, depth) = space_trucking::sim::RoomKind::Cabin.floor();
        for row in 0..depth {
            let cell = layout::cell_rect(CABIN, 3, space_trucking::sim::room::COURSES + row);
            let mid = SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
            let over = floor.to_world(mid).z;
            assert!(
                (WALK_MIN.z..=WALK_MAX.z).contains(&over),
                "deck row {row} at z={over} is outside the walk envelope                  ({}..{}) — ground the player cannot stand on",
                WALK_MIN.z,
                WALK_MAX.z
            );
        }
        assert!(
            WALK_MAX.x >= floor.half_u.length() - 0.40
                && -WALK_MIN.x >= floor.half_u.length() - 0.40,
            "envelope too narrow for the floor's side columns"
        );
    }

    /// **Every cell of the cabin's deck is a berth.** The other half of
    /// the dead strip: the gutter carried no net cell at all, so the
    /// ground in front of the front row was not merely unstandable but
    /// unusable — and a floor with a strip of nothing along one edge is
    /// exactly what the playtest reported. The net says 8×7; the hull
    /// now agrees, so every cell of it lies inside the room.
    #[test]
    fn the_hull_holds_the_whole_deck_and_no_more() {
        let floor = bay()
            .into_iter()
            .find(|(station, _)| matches!(station, Station::BayFloor))
            .map(|(_, s)| s)
            .expect("floor chart");
        let slabs = structure();
        let front = floor.center.z - floor.half_v.length();
        let aft = floor.center.z + floor.half_v.length();
        // The hull stops within a wall's thickness of the deck's own
        // edges fore and aft: no gutter, and no missing floor either.
        let (lo, hi) = slabs
            .iter()
            .map(|slab| (slab.center - slab.size * 0.5, slab.center + slab.size * 0.5))
            .fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(a, b), (c, d)| (a.min(c), b.max(d)),
            );
        assert!(
            (front - lo.z).abs() < 0.16,
            "the hull stands {} forward of the deck's front row",
            front - lo.z
        );
        assert!(
            (hi.z - aft).abs() < 0.16,
            "the hull stands {} aft of the deck's aft row",
            hi.z - aft
        );
    }
}
