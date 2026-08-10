//! The cabin itself: an enclosed box with flavor, per DESIGN.md's first
//! pass. Three focusable panels forward — star tank on the left wall,
//! console face front-right, barter counter low-right — and the walkable
//! cargo bay aft: the sim's room net — an 8×7 floor inside three courses
//! of wall — unfolded onto the whole hull at furniture scale
//! (docs/BAY.md), worked from roam with the crosshair instead of from a
//! focus pose.
//!
//! Two camera postures. **Roaming**: a conventional first-person walk —
//! pointer locked, mouse to look, WASD to move, a crosshair dot; aim at a
//! station and it invites with a glint frame. **Focused**: click (or `E`)
//! and the camera glides to that station's authored viewpoint, the cursor
//! frees, and precise sim interaction happens exactly as in 2D. `Esc`,
//! right-click, or `E` steps back out. The camera never trails the cursor
//! — deliberate moves only, nothing to get seasick over.
//!
//! Structural geometry is *data first* (`structure()`), and the desk
//! masses under the tilted panels are **derived from the panel corners**
//! rather than authored twice — the class of clipping bug where furniture
//! swallows a panel's lower row cannot recur, and a unit test walks every
//! panel face against every slab to keep it that way. Focus viewpoints
//! are likewise fitted from the panel extents and the camera's FOV.
//!
//! Also home to the pixel crunch (a 480×270 nearest-neighbour target —
//! "smoothing off" applied to the whole world) and the shared low-poly
//! material [`Skin`].

use bevy::camera::{Hdr, RenderTarget};
use bevy::image::ImageSampler;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use space_trucking::sim::room::{CABIN, RoomKind};
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
const EYE_HEIGHT: f32 = 1.5;
// The room grid made the whole floor walkable ground — cargo stands
// among the walker now, not beyond a strip — so the envelope spans the
// room up to body clearance at the aft wall. The clamp is collision
// with the HULL and nothing else: cargo has no body to bump, and the
// placement law no longer reserves a lane for one either.
const WALK_MIN: Vec3 = Vec3::new(-1.85, EYE_HEIGHT, -0.85);
const WALK_MAX: Vec3 = Vec3::new(1.85, EYE_HEIGHT, 2.27);
const WALK_SPEED: f32 = 1.3;
const LOOK_SPEED: f32 = 0.0026;
const PITCH_LIMIT: f32 = 1.35;

// ---- The bay: the hold grid unfolded onto the aft of the cabin ----

/// Bay cell edge, world units — square cells at furniture scale.
pub const BAY_CELL: f32 = 0.55;
/// Bay width: eight columns flush toward the side walls (the columns
/// beyond them are the walls, exactly where the wall-affix rule points).
const BAY_W: f32 = 8.0 * BAY_CELL;
/// Wall band height: grid rows 0–2 stand on the aft wall.
const BAY_WALL_H: f32 = 3.0 * BAY_CELL;
/// The wall band's quad plane, just proud of the aft hull's inner face.
const BAY_WALL_Z: f32 = 2.41;

/// The floor chart's depth (7 cells) and its centre plane in z: from the
/// aft wall forward, leaving a working gutter before the front console
/// wall (the room resized to cell multiples; BAY.md, "The room grid").
/// The widening pushed every hull plane out by exactly one cell per
/// side, so the gutter, the trim band, and every seam kept their gaps.
const BAY_FLOOR_D: f32 = 7.0 * BAY_CELL;
const BAY_FLOOR_ZC: f32 = BAY_WALL_Z - BAY_FLOOR_D * 0.5;

/// The side wall charts' planes: proud of the wall ribs (which span
/// ±1.63..±1.69), so every chart cell stays workable in front of the
/// hull's junk rather than behind it.
const BAY_SIDE_X: f32 = 2.17;

/// The front wall chart's plane, just inside the front hull at -1.97.
const BAY_FRONT_Z: f32 = -1.91;

/// The ceiling chart's plane, just under the ceiling slab at 2.32 — the
/// band between the wall cornices (1.65) and here is headroom trim; the
/// net's fold seam glues logically, not physically.
const BAY_CEIL_Y: f32 = 2.26;

/// The decal ladder: everything drawn flat over a chart quad sits at a
/// named rung along the chart's inward normal, and the z-fight test
/// (`pieces::tests::the_decal_ladder_never_z_fights`) keeps every rung
/// a depth-safe step from its neighbours — the playtest's shimmering
/// doormat is the defect class this retires. Flat paints on the ladder
/// keep their meshes ≤ [`layer::SKIN`] thick so rungs, not luck, decide
/// what draws over what.
pub mod layer {
    /// Berth socket wells — the contextual tiles.
    pub const TILE: f32 = 0.002;
    /// The burner threshold's hazard paint.
    pub const DOORMAT: f32 = 0.006;
    /// Laid coverings' base; a rug's pile rises `RUG_THICK` above it.
    pub const LAID: f32 = 0.010;
    /// Placement hint quads.
    pub const HINT: f32 = 0.026;
    /// The hint's refusal slash.
    pub const SLASH: f32 = 0.030;
    /// The violation flash frame.
    pub const FLASH: f32 = 0.034;
    /// The violation glyph bars.
    pub const GLYPH: f32 = 0.038;
    /// Minimum step between occupied rungs that stays fight-free at
    /// room distances in the depth buffer. Consumed by the ladder test
    /// (`pieces::tests`), which is its whole job.
    #[allow(dead_code)]
    pub const STEP: f32 = 0.004;
    /// Maximum mesh thickness for a flat paint riding a rung.
    pub const SKIN: f32 = 0.0015;
}
/// The deck strip's quad plane, just above the deck.
const BAY_FLOOR_Y: f32 = 0.012;

/// How far the roaming crosshair can grab or place, in world units —
/// arm's length plus a step, so bay work happens near the body and the
/// far half of the room stays a view, not a reach.
pub const REACH: f32 = 2.0;

// ---- The burner: an annex off the starboard wall, the fuel hopper ----
//
// Cargo bound for the fire stages here (the sim's outboard rail — the
// same four Flotsam slots, unchanged); underway the stoker's beat feeds
// the lowest tile to the firebox in the far wall, and what burns pushes
// the ship. Sized so the largest footprint in the game JUST fits through
// the door and inside the chamber — proven by test. (The AIR_* names
// survive from the annex's airlock past; renaming them is churn the
// geometry doesn't need.)

/// Chamber interior width and depth: the biggest crate plus a squeeze.
pub const AIR_INNER: f32 = 2.0 * BAY_CELL + 0.08;
/// Doorway span along the wall (z) — the full chamber width opens.
/// Anchored to the aft hull, so the widening slid it forward with the
/// wall it is cut into and it still fronts the same net cells.
pub const AIR_DOOR_Z0: f32 = 1.05;
pub const AIR_DOOR_Z1: f32 = AIR_DOOR_Z0 + AIR_INNER;
/// Doorway clear height: a two-cell piece passes upright, barely.
pub const AIR_DOOR_H: f32 = 2.0f32 * BAY_CELL + 0.18;
/// The chamber floor's x extent: from the hull's outer face outboard.
pub const AIR_X0: f32 = 2.32;
pub const AIR_X1: f32 = AIR_X0 + AIR_INNER;

/// Floor cells painted as the burner's threshold: the doormat in front
/// of the doorway. Pure decoration now — the placement law reserves
/// nothing, and cargo may stand on the stripes like anywhere else —
/// but a doorway that reads as a doorway is worth the paint, and the
/// stoker's path is still where the stripes say it is.
const THRESHOLD: [(u8, u8); 2] = [(10, 3), (10, 4)];

/// Focus glide length, seconds. A camera move is feedback: it answers a
/// click and finishes fast.
const GLIDE: f32 = 0.38;

/// Margin factor when fitting a panel into the focused view.
const FIT_MARGIN: f32 = 1.14;

/// How far the physical panel plate extends past its mapped quad.
const PLATE_MARGIN: f32 = 0.03;

/// The panels still screwed to the hull: where each sim region lives in
/// the cabin. Width/height keep each rect's aspect; scales differ per
/// panel on purpose. The hold is not here — it unfolded into the bay
/// ([`bay`]) — and neither is the star tank: the instruments are cargo,
/// so `Station::Map` and `Station::Lever` ride their pieces' cells
/// (`pieces::instrument_surface`) and this list keeps only what the
/// ship itself owns. The barter counter left with the interface it
/// belonged to (docs/ROOMS.md); the console face is next.
#[must_use]
pub fn panels() -> [(Station, SimSurface); 1] {
    [(
        Station::Console,
        SimSurface::panel(
            Vec3::new(0.50, 1.52, -1.83),
            0.54,
            0.84,
            0.10,
            layout::CONSOLE,
        ),
    )]
}

/// The bay's six mapped surfaces: the room net unfolded like an opened
/// box. Rows 0–2 stand on the aft wall (row 0 the cornice), rows 3–9
/// fold onto the deck and the two side walls, rows 10–12 stand on the
/// front wall, and the ceiling chart folds on past the starboard
/// cornice. Sim +x runs to the player's right when facing the aft wall;
/// the seams are watertight (or declared gutters) by test. Cursor rays
/// from roam project through these exactly as focus cursors project
/// through panels, so the sim keeps every ruling.
#[must_use]
pub fn bay() -> [(Station, SimSurface); 6] {
    // One chart's logical rect, in net cells.
    let chart = |cx: u8, cy: u8, w: u8, h: u8| {
        layout::Rect::new(
            f32::from(cx).mul_add(layout::CELL, layout::GRID_ORIGIN.x),
            f32::from(cy).mul_add(layout::CELL, layout::GRID_ORIGIN.y),
            f32::from(w) * layout::CELL,
            f32::from(h) * layout::CELL,
        )
    };
    // The seam law pins every axis: columns match across the folds
    // (floor col 3 lies to port because the port chart's baseboard is
    // x2), cornices sit up, y3 rows lie aft. Every normal therefore
    // points OUT of the room (`Station::chart_flipped`); consumers use
    // `Station::inward`/`Station::face`.
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

/// Which shared material a slab wears.
#[derive(Clone, Copy, Debug)]
pub enum Finish {
    Hull,
    Plate,
}

/// An axis-aligned structural mass: walls, ribs, desk supports.
#[derive(Clone, Copy, Debug)]
pub struct Slab {
    pub center: Vec3,
    pub size: Vec3,
    pub finish: Finish,
}

impl Slab {
    const fn new(center: Vec3, size: Vec3, finish: Finish) -> Self {
        Self {
            center,
            size,
            finish,
        }
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

/// Every axis-aligned mass in the cabin. Supports under the tilted desk
/// panels are derived from the panel corners (plate margin included), so
/// they can never swallow a panel's lower edge.
#[must_use]
// One slab per line of the room's plan; splitting the list would
// scatter the one place the whole hull is written down.
#[allow(clippy::too_many_lines)]
pub fn structure(panels: &[(Station, SimSurface)]) -> Vec<Slab> {
    let mut slabs = vec![
        // The box: floor, ceiling, four walls. The 8x7 floor put a cell
        // (0.55) between each of these and where it used to stand: the
        // room grew outward on every side at once, so nothing inside it
        // had to be re-composed, only re-measured.
        Slab::new(
            Vec3::new(0.0, -0.05, 0.2),
            Vec3::new(4.5, 0.1, 4.5),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 2.32, 0.2),
            Vec3::new(4.5, 0.1, 4.5),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 1.15, -1.97),
            Vec3::new(4.5, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 1.15, 2.47),
            Vec3::new(4.5, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(-2.27, 1.15, 0.2),
            Vec3::new(0.1, 2.5, 4.5),
            Finish::Hull,
        ),
        // The starboard wall parts around the airlock doorway: a fore
        // segment, a sliver aft of it, and the lintel over the opening.
        Slab::new(
            Vec3::new(2.27, 1.15, f32::midpoint(-2.05, AIR_DOOR_Z0)),
            Vec3::new(0.1, 2.5, AIR_DOOR_Z0 + 2.05),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(2.27, 1.15, f32::midpoint(AIR_DOOR_Z1, 2.45)),
            Vec3::new(0.1, 2.5, 2.45 - AIR_DOOR_Z1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(
                2.27,
                f32::midpoint(AIR_DOOR_H, 2.40),
                f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1),
            ),
            Vec3::new(0.1, 2.40 - AIR_DOOR_H, AIR_INNER),
            Finish::Hull,
        ),
        // The airlock annex: outer wall, two cheeks, floor and ceiling.
        Slab::new(
            Vec3::new(AIR_X1 + 0.05, 1.15, f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1)),
            Vec3::new(0.1, 2.5, AIR_INNER + 0.2),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(f32::midpoint(AIR_X0, AIR_X1), 1.15, AIR_DOOR_Z0 - 0.05),
            Vec3::new(AIR_INNER, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(f32::midpoint(AIR_X0, AIR_X1), 1.15, AIR_DOOR_Z1 + 0.05),
            Vec3::new(AIR_INNER, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(
                f32::midpoint(AIR_X0, AIR_X1),
                -0.05,
                f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1),
            ),
            Vec3::new(AIR_INNER + 0.1, 0.1, AIR_INNER + 0.2),
            Finish::Plate,
        ),
        Slab::new(
            Vec3::new(
                f32::midpoint(AIR_X0, AIR_X1),
                2.32,
                f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1),
            ),
            Vec3::new(AIR_INNER + 0.1, 0.1, AIR_INNER + 0.2),
            Finish::Hull,
        ),
    ];
    // Wall ribs: the junk that says somebody built this hull in a hurry.
    // The port wall runs its full set again — the chart tank that used
    // to be bolted through them is cargo now, hung in front of whatever
    // wall it is carried to, so the hull has nothing to make room for.
    for i in 0..6 {
        let z = 0.7f32.mul_add(i as f32, -1.55);
        slabs.push(Slab::new(
            Vec3::new(-2.21, 1.15, z),
            Vec3::new(0.06, 2.3, 0.08),
            Finish::Hull,
        ));
        // The starboard ribs still skip the airlock doorway's span:
        // that hole is architecture, not furniture.
        if !(AIR_DOOR_Z0 - 0.10..=AIR_DOOR_Z1 + 0.10).contains(&z) {
            slabs.push(Slab::new(
                Vec3::new(2.21, 1.15, z),
                Vec3::new(0.06, 2.3, 0.08),
                Finish::Hull,
            ));
        }
    }
    // The counter's derived support came out with the counter it held
    // up (docs/ROOMS.md): the last screen-shaped surface in the game.
    let _ = panels;
    slabs
}

// ---- The camera rig ----

/// A focused viewpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Tank,
    Console,
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
            Station::Console => Some(Self::Console),
            Station::Lever => Some(Self::Lever),
            Station::BayWall
            | Station::BayFloor
            | Station::BayPort
            | Station::BayStarboard
            | Station::BayFront
            | Station::BayCeiling
            | Station::Airlock
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

/// The pose a focus parks at: panel extents fitted to the camera FOV,
/// eyed along the panel normal, up running up-panel.
///
/// The surfaces are whatever is standing right now, not a fixed list:
/// an instrument's station rides its cargo, so its pose is its berth's
/// (BAY.md, "Focus poses become relative to the instrument's berth").
/// `None` when nothing carries that station — jettisoned, shelved, or
/// in the player's own hands — and the caller falls back to roam.
/// Owning two chart tanks widens the fit to frame both: the station is
/// the instrument, not the piece.
#[must_use]
pub fn focus_pose(focus: Focus, panels: &[(Station, SimSurface)]) -> Option<(Vec3, Quat)> {
    let group: Vec<&SimSurface> = panels
        .iter()
        .filter(|(station, _)| Focus::of(*station) == Some(focus))
        .map(|(_, s)| s)
        .collect();
    // Combined center and planar extents, measured in the first panel's
    // frame (desk panels share a tilt by construction).
    let lead = *group.first()?;
    let u = lead.half_u.normalize();
    let v = lead.half_v.normalize();
    let center = group.iter().fold(Vec3::ZERO, |acc, s| acc + s.center) / group.len() as f32;
    let mut half_w: f32 = 0.0;
    let mut half_h: f32 = 0.0;
    for s in &group {
        let offset = s.center - center;
        half_w = half_w.max(offset.dot(u).abs() + s.half_u.length() + PLATE_MARGIN);
        half_h = half_h.max(offset.dot(v).abs() + s.half_v.length() + PLATE_MARGIN);
    }
    let aspect = CRUNCH_W as f32 / CRUNCH_H as f32;
    let half_hfov = ((FOV * 0.5).tan() * aspect).atan();
    let distance =
        (half_w * FIT_MARGIN / half_hfov.tan()).max(half_h * FIT_MARGIN / (FOV * 0.5).tan());
    let eye = center + lead.normal() * distance;
    let look = Transform::from_translation(eye).looking_at(center, -v);
    Some((eye, look.rotation))
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
    /// Painted hazard stripes for accent strips.
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
            hull: crate::wear::worn(materials, &book.hull, palette::HULL, 4.0, 0.92, 0.15),
            plate: crate::wear::worn(materials, &book.plate, palette::PLATE, 2.0, 0.92, 0.15),
            desk: crate::wear::worn(materials, &book.desk, palette::PLATE, 2.0, 0.9, 0.15),
            hazard: crate::wear::worn(materials, &book.hazard, palette::GLINT, 2.0, 0.85, 0.0),
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

/// The berth wells' shared translucent ink and its eased level.
#[derive(Resource)]
pub struct TileFade {
    mat: Handle<StandardMaterial>,
    level: f32,
}

/// How fast the berth grid answers a grab, per second — feedback, so it
/// finishes well inside the half-second law.
const TILE_FADE_RATE: f32 = 6.0;

/// The glint frame inviting a station's focus while aimed at in roam.
#[derive(Component)]
pub struct AimFrame(pub Station);

/// Spawn the whole static cabin: crunch pipeline, camera, structure,
/// panels, sockets, lights, version text.
#[allow(clippy::too_many_lines)]
pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    rig: Res<CameraRig>,
) {
    let skin = Skin::build(&mut meshes, &mut materials, &mut images);
    let panels = panels();

    // --- The crunch: a small render target shown fullscreen, unsmoothed.
    let mut target = Image::new_target_texture(
        CRUNCH_W,
        CRUNCH_H,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    target.sampler = ImageSampler::nearest();
    let target = images.add(target);

    // A booted focus (`--view`) has no instrument to aim at yet — the
    // riding stations are hung on the first frame — so the camera opens
    // on the roaming pose and `pose` snaps it home a frame later.
    let (pos, rot) = match rig.mode {
        Mode::Focused { focus } => focus_pose(focus, &panels),
        _ => None,
    }
    .unwrap_or_else(|| (rig.pos, rig.roam_rotation()));
    commands.spawn((
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
        // A breath of particulate: gentle depth haze in hull tones. The
        // far corners of the cabin soften; panels up close stay crisp.
        DistanceFog {
            color: palette::HULL.with_alpha(0.85),
            falloff: FogFalloff::Exponential { density: 0.16 },
            ..default()
        },
        Msaa::Off,
        Transform::from_translation(pos).with_rotation(rot),
        CabinCamera,
    ));
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
    for slab in structure(&panels) {
        let material = match slab.finish {
            Finish::Hull => skin.hull.clone(),
            Finish::Plate => skin.desk.clone(),
        };
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(material),
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

    // --- Panels: each SimSurface entity carries its station tag; a PLATE
    // slab sits just behind each mapped quad as the physical panel, and a
    // glint aim frame waits hidden for the roaming crosshair.
    for (station, surface) in panels {
        let n = surface.normal();
        let size = Vec3::new(
            surface.half_u.length().mul_add(2.0, PLATE_MARGIN * 2.0),
            surface.half_v.length().mul_add(2.0, PLATE_MARGIN * 2.0),
            0.05,
        );
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.plate.clone()),
            Transform::from_translation(surface.center - n * 0.028)
                .with_rotation(surface.orientation())
                .with_scale(size),
        ));
        let frame = materials.add(StandardMaterial {
            base_color: palette::SHADOW,
            emissive: palette::GLINT.to_linear() * 1.4,
            perceptual_roughness: 0.85,
            ..default()
        });
        let w = size.x + 0.015;
        let h = size.y + 0.015;
        commands
            .spawn((
                Transform::from_translation(surface.center + n * 0.004)
                    .with_rotation(surface.orientation()),
                Visibility::Hidden,
                AimFrame(station),
            ))
            .with_children(|parent| {
                for (offset, bar) in [
                    (Vec3::new(0.0, h * 0.5, 0.0), Vec3::new(w, 0.008, 0.006)),
                    (Vec3::new(0.0, -h * 0.5, 0.0), Vec3::new(w, 0.008, 0.006)),
                    (Vec3::new(w * 0.5, 0.0, 0.0), Vec3::new(0.008, h, 0.006)),
                    (Vec3::new(-w * 0.5, 0.0, 0.0), Vec3::new(0.008, h, 0.006)),
                ] {
                    parent.spawn((
                        Mesh3d(skin.cube.clone()),
                        MeshMaterial3d(frame.clone()),
                        Transform::from_translation(offset).with_scale(bar),
                    ));
                }
            });
        commands.spawn((station, surface));
    }

    // --- The bay: the hold grid unfolded onto the aft wall and deck.
    // Each surface entity carries its station tag so the roam crosshair
    // projects through it exactly as focus cursors project through
    // panels. The berth marking (socket plates per cell), the backer
    // plates, the gantry frame, and the hazard lip are all derived from
    // the same two surfaces, so a retuned bay moves as one thing.
    // The berth wells' one translucent ink: contextual placement
    // furniture, raised by [`fade_tiles`] only while a carry is live.
    let tile_mat = materials.add(StandardMaterial {
        base_color: palette::SOCKET.with_alpha(0.0),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.insert_resource(TileFade {
        mat: tile_mat.clone(),
        level: 0.0,
    });
    for (station, surface) in bay() {
        let n = station.inward(&surface);
        // Backer plate: a worn slab just behind the mapped quad — except
        // on the wall and ceiling charts, whose backing IS the hull the
        // structure already built; a second slab would swallow the trim.
        if matches!(station, Station::BayWall | Station::BayFloor) {
            let deep = 0.03;
            commands.spawn((
                Mesh3d(skin.cube.clone()),
                MeshMaterial3d(match station {
                    Station::BayFloor => skin.desk.clone(),
                    _ => skin.plate.clone(),
                }),
                Transform::from_translation(surface.center - n * (deep * 0.5 + 0.004))
                    .with_rotation(surface.orientation())
                    .with_scale(Vec3::new(
                        surface.half_u.length().mul_add(2.0, 0.08),
                        surface.half_v.length().mul_add(2.0, 0.08),
                        deep,
                    )),
            ));
        }
        // Socket wells per cell, the berth marking — each surface takes
        // exactly the net cells its rect covers; holes (the doorway, the
        // window) get no well.
        let (cols, rows) = RoomKind::Cabin.grid();
        for y in 0..rows {
            for x in 0..cols {
                if RoomKind::Cabin.surface_of(x, y).is_none() {
                    continue;
                }
                let cell = layout::cell_rect(CABIN, x, y);
                let mid = space_trucking::sim::Vec2::new(
                    cell.w.mul_add(0.5, cell.x),
                    cell.h.mul_add(0.5, cell.y),
                );
                if !surface.rect.contains(mid) {
                    continue;
                }
                commands.spawn((
                    Mesh3d(skin.cube.clone()),
                    MeshMaterial3d(tile_mat.clone()),
                    Transform::from_translation(surface.to_world(mid) + n * layer::TILE)
                        .with_rotation(surface.orientation())
                        .with_scale(Vec3::new(
                            (cell.w - 4.0) * surface.scale_u(),
                            (cell.h - 4.0) * surface.scale_v(),
                            layer::SKIN,
                        )),
                    BerthTile,
                ));
            }
        }
        commands.spawn((station, surface));
    }
    {
        let charts = bay();
        let (_, wall) = charts[0];
        let (_, floor) = charts[1];
        let wall_top = wall.center.y + wall.half_v.length();
        let strip_front = floor.center.z - floor.half_v.length();
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
        // Hazard lip along the floor chart's front edge: the painted
        // line between the front gutter and where cargo lives.
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.hazard.clone()),
            Transform::from_translation(Vec3::new(0.0, 0.017, strip_front - 0.032))
                .with_scale(Vec3::new(BAY_W + 0.14, 0.034, 0.05)),
        ));
        // The burner doormat: hazard stripes on the floor cells
        // fronting the doorway. Paint only — nothing refuses a berth
        // here — but the eye still reads the way out to the fire.
        for &(ax, ay) in &THRESHOLD {
            let cell = layout::cell_rect(CABIN, ax, ay);
            let mid = space_trucking::sim::Vec2::new(
                cell.w.mul_add(0.5, cell.x),
                cell.h.mul_add(0.5, cell.y),
            );
            commands.spawn((
                Mesh3d(skin.cube.clone()),
                MeshMaterial3d(skin.hazard.clone()),
                Transform::from_translation(floor.to_world(mid) + Vec3::Y * layer::DOORMAT)
                    .with_rotation(floor.orientation())
                    .with_scale(Vec3::new(
                        (cell.w - 6.0) * floor.scale_u(),
                        (cell.h - 6.0) * floor.scale_v(),
                        layer::SKIN,
                    )),
            ));
        }
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
    // Off its wall — staged on a hopper tile, boxed in a cubby, sat on
    // the counter — an instrument is only cargo again: it carries no
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
    mut rig: ResMut<CameraRig>,
    camera: Single<&Transform, With<CabinCamera>>,
) {
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
            // the cursor is the desktop's now, click to reclaim.
            if keys.just_pressed(KeyCode::Escape)
                || keys.just_pressed(KeyCode::SuperLeft)
                || keys.just_pressed(KeyCode::SuperRight)
            {
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
                let pos = rig.pos + step.normalize() * WALK_SPEED * time.delta_secs();
                rig.pos = pos.clamp(WALK_MIN, WALK_MAX);
            }
            // Focus what the crosshair rests on — cargo in hand included:
            // the click carries the piece to the counter (the carry
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
        Mode::Focused { .. } => {
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
        // Glides ignore input; they are sub-half-second.
        Mode::ToFocus { .. } | Mode::ToRoam { .. } => {}
    }
}

/// Advance glides and write the camera transform for the current mode.
///
/// The poses come from the surfaces standing in the room this frame,
/// not from a constant: an instrument's station rides its cargo, so a
/// tank carried off mid-glide simply has no pose left — the camera
/// lets go and walks back (`Mode::ToRoam`) rather than aiming at a
/// hole in the wall.
pub fn pose(
    time: Res<Time>,
    surfaces: Query<(&Station, &SimSurface), Without<CabinCamera>>,
    mut rig: ResMut<CameraRig>,
    mut camera: Single<&mut Transform, With<CabinCamera>>,
) {
    let panels = live_panels(&surfaces);
    let dt = time.delta_secs();
    let (roam_pos, roam_rot) = (rig.pos, rig.roam_rotation());
    // The instrument left with its cargo: let go of the focus here and
    // walk back from wherever the glide had got to.
    let orphaned = match rig.mode {
        Mode::Focused { focus } | Mode::ToFocus { focus, .. } => {
            focus_pose(focus, &panels).is_none()
        }
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
        Mode::Focused { focus } => focus_pose(*focus, &panels).unwrap_or((roam_pos, roam_rot)),
        Mode::ToFocus { focus, from, t } => {
            let (to_pos, to_rot) = focus_pose(*focus, &panels).unwrap_or((roam_pos, roam_rot));
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

/// Cursor grab, crosshair, and aim frames follow the mode.
pub fn present_mode(
    rig: Res<CameraRig>,
    mut window: Single<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut crosshair: Single<&mut Visibility, (With<Crosshair>, Without<AimFrame>)>,
    camera: Single<&Transform, With<CabinCamera>>,
    surfaces: Query<(&Station, &SimSurface), Without<CabinCamera>>,
    mut frames: Query<(&AimFrame, &mut Visibility), Without<Crosshair>>,
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
    **crosshair = if roaming {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let aimed = if roaming {
        aimed_station(&camera, &surfaces).and_then(Focus::of)
    } else {
        None
    };
    for (frame, mut visibility) in &mut frames {
        *visibility = if aimed.is_some() && aimed == Focus::of(frame.0) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
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

    /// Ray versus a panel's physical plate (the oriented slab behind the
    /// mapped quad), in the panel's own frame.
    fn ray_plate_entry(origin: Vec3, dir: Vec3, surface: &SimSurface) -> Option<f32> {
        let frame = surface.orientation().inverse();
        let center = surface.center + surface.normal() * -0.028;
        let local = Slab::new(
            Vec3::ZERO,
            Vec3::new(
                surface.half_u.length().mul_add(2.0, PLATE_MARGIN * 2.0),
                surface.half_v.length().mul_add(2.0, PLATE_MARGIN * 2.0),
                0.05,
            ),
            Finish::Plate,
        );
        ray_slab_entry(frame * (origin - center), frame * dir, &local)
    }

    /// The sightline rule the screenshots check by eye, made mechanical:
    /// `point` must sit inside the camera frustum AND nothing structural
    /// may stand between the eye and it.
    fn visible_from(
        eye: Vec3,
        rot: Quat,
        point: Vec3,
        slabs: &[Slab],
        panels: &[(Station, SimSurface)],
    ) -> Result<(), String> {
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
        for (station, surface) in panels {
            if let Some(t) = ray_plate_entry(eye, dir, surface)
                && t < 1.0 - 1e-3
            {
                return Err(format!(
                    "{station:?}'s plate blocks the line from {eye} to {point} (t={t:.3})"
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
            // The face keeps only the toggles and the hangar strip; the
            // readings and the pull are cargo, checked at their own
            // stations above.
            Station::Console => {
                spots.push(mid(layout::PAUSE_BTN));
                spots.push(mid(layout::WARP_BTN));
                spots.push(mid(layout::SPEAKER));
            }
            Station::BayWall
            | Station::BayFloor
            | Station::BayPort
            | Station::BayStarboard
            | Station::BayFront
            | Station::BayCeiling
            | Station::Airlock
            | Station::Standing => {}
        }
        spots.into_iter().map(|s| surface.to_world(s)).collect()
    }

    /// Every station standing in the starter cabin: the hull's own
    /// panels plus the instruments' riding stations, derived from the
    /// sim's opening board through the very function the runtime rides
    /// them with. Move a starter berth and these tests move with it —
    /// which is the point, now that the poses belong to the cargo.
    fn stations() -> Vec<(Station, SimSurface)> {
        let sim = space_trucking::sim::Sim::new(1);
        let charts = bay();
        let mut all: Vec<(Station, SimSurface)> = panels().to_vec();
        all.extend(sim.pieces().iter().filter_map(|piece| {
            matches!(piece.loc, Loc::Hold { .. })
                .then(|| {
                    crate::pieces::instrument_surface(
                        &charts,
                        piece.kind,
                        layout::piece_rect(sim.pieces(), piece),
                    )
                })
                .flatten()
        }));
        all
    }

    /// The sightline contract: from a station's own focus viewpoint,
    /// every panel corner and every control it carries must be visible —
    /// framed and unoccluded. This is the "corner must be visible from
    /// the perspective" rule, enforced at build time.
    ///
    /// The hull is what must never be in the way. Desk-class furniture
    /// is the migrating kind of blocker — the barter counter still
    /// shoulders in beside the front wall's baseboard, and it is on its
    /// way out with the barter redesign (BAY.md) — so an instrument's
    /// pose answers to the hull alone, exactly as the net's workability
    /// budget already does. Cargo standing in a sightline is neither's
    /// business: the focus x-ray ghosts it at runtime.
    #[test]
    fn every_control_is_visible_from_its_focus() {
        let panels = panels();
        let slabs = structure(&panels);
        let hull: Vec<Slab> = slabs
            .iter()
            .copied()
            .filter(|slab| matches!(slab.finish, Finish::Hull))
            .collect();
        for (station, surface) in stations() {
            let focus = Focus::of(station).expect("every station is focusable");
            let (eye, rot) = focus_pose(focus, &stations()).expect("every station has a pose");
            let mut points = corner_points(&surface);
            points.extend(control_points(station, &surface));
            let riding = panels.iter().all(|(tag, _)| *tag != station);
            for point in points {
                // Lift each point a hair off the face so the ray test
                // asks about the air in front of it, not the face itself.
                let probe = point + surface.normal() * 0.004;
                let broken = if riding {
                    visible_from(eye, rot, probe, &hull, &[])
                } else {
                    visible_from(eye, rot, probe, &slabs, &panels)
                };
                if let Err(reason) = broken {
                    panic!("{station:?} sightline broken: {reason}");
                }
            }
        }
    }

    /// Sample points across a panel's plate face (quad + margin).
    fn plate_face_points(surface: &SimSurface) -> Vec<Vec3> {
        let u = surface.half_u + surface.half_u.normalize() * PLATE_MARGIN;
        let v = surface.half_v + surface.half_v.normalize() * PLATE_MARGIN;
        let mut points = Vec::new();
        for i in 0..=8 {
            for j in 0..=8 {
                let a = (i as f32 / 8.0).mul_add(2.0, -1.0);
                let b = (j as f32 / 8.0).mul_add(2.0, -1.0);
                points.push(surface.center + u * a + v * b);
            }
        }
        points
    }

    /// The regression the screenshot caught: no structural slab may
    /// swallow any part of any panel's visible face.
    #[test]
    fn structure_never_swallows_a_panel() {
        let panels = panels();
        let slabs = structure(&panels);
        for (station, surface) in &panels {
            for point in plate_face_points(surface) {
                for slab in &slabs {
                    assert!(
                        !slab.contains(point, 1e-3),
                        "{station:?} face point {point} sits inside slab at {}",
                        slab.center
                    );
                }
            }
        }
    }

    /// Focus viewpoints must be legal camera positions: inside the box,
    /// inside no slab, looking at their stations — the instruments'
    /// riding poses included, since those are wherever the cargo is.
    #[test]
    fn focus_poses_are_legal_camera_positions() {
        let slabs = structure(&panels());
        let stations = stations();
        for focus in [Focus::Tank, Focus::Console, Focus::Lever] {
            let (eye, rot) = focus_pose(focus, &stations).expect("the starter board hangs it");
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
            // The view axis should pass close to every grouped panel
            // center: no panel of the group may sit behind the camera.
            for (station, surface) in &stations {
                if Focus::of(*station) == Some(focus) {
                    let to_panel = (surface.center - eye).normalize();
                    let forward = rot * Vec3::NEG_Z;
                    assert!(
                        forward.dot(to_panel) > 0.7,
                        "{focus:?} does not face {station:?}"
                    );
                }
            }
        }
    }

    /// A station with nothing carrying it has no pose at all, and the
    /// camera falls back to roam rather than aiming at a bare wall: an
    /// instrument is cargo, and cargo can be sold.
    #[test]
    fn a_jettisoned_instrument_leaves_no_pose() {
        let hull = panels();
        assert!(focus_pose(Focus::Tank, &hull).is_none());
        assert!(focus_pose(Focus::Lever, &hull).is_none());
        assert!(focus_pose(Focus::Console, &hull).is_some());
    }

    /// The roaming envelope stays clear of every slab at eye height.
    #[test]
    fn walk_envelope_is_clear() {
        let panels = panels();
        let slabs = structure(&panels);
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
    /// them. Three folds are physically watertight (aft, port,
    /// starboard meet the floor at the baseboards); two are declared
    /// gutter/trim seams with a bounded gap — the front wall stands a
    /// working gutter past the floor's front row, and the ceiling
    /// chart sits a trim band above the starboard cornice (BAY.md).
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
        // Gutter and trim seams: adjacent in the net, offset in the
        // room by a declared, bounded margin.
        assert!(
            gap(&floor, bottom, &front, top) < 0.60,
            "front gutter wider than declared"
        );
        assert!(
            gap(&starboard, east, &ceiling, west) < 0.75,
            "ceiling trim wider than declared"
        );
    }

    /// Every net cell can actually be worked: from some legal roaming
    /// position, its center is within REACH, within the pitch the neck
    /// allows, and no structure blocks the line — EXCEPT cells whose
    /// only obstruction is a station panel standing in front of them.
    /// Those cells are real (the sim keeps them; rats hide behind the
    /// console) and become workable as the instruments migrate off the
    /// walls (BAY.md); a cell blocked by hull is still a build error.
    #[test]
    fn every_net_cell_is_workable_or_station_fronted() {
        let panels = panels();
        let slabs = structure(&panels);
        let mut station_fronted = 0_u32;
        for (station, surface) in bay() {
            for y in 0..layout::GRID_ROWS {
                for x in 0..layout::GRID_COLS {
                    if RoomKind::Cabin.surface_of(x, y).is_none() {
                        continue;
                    }
                    let cell = layout::cell_rect(CABIN, x, y);
                    let mid =
                        SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
                    if !surface.rect.contains(mid) {
                        continue;
                    }
                    let probe = surface.to_world(mid) + station.inward(&surface) * 0.004;
                    let mut panel_blocked = false;
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
                            // Hull in the way is a build error; plate-finish
                            // furniture (desk supports) and station
                            // panels are the migrating kind of blocker.
                            if slabs.iter().any(|slab| {
                                matches!(slab.finish, Finish::Hull)
                                    && ray_slab_entry(eye, dir, slab)
                                        .is_some_and(|t| t < 1.0 - 1e-3)
                            }) {
                                return false;
                            }
                            if slabs.iter().any(|slab| {
                                matches!(slab.finish, Finish::Plate)
                                    && ray_slab_entry(eye, dir, slab)
                                        .is_some_and(|t| t < 1.0 - 1e-3)
                            }) || panels.iter().any(|(_, panel)| {
                                ray_plate_entry(eye, dir, panel).is_some_and(|t| t < 1.0 - 1e-3)
                            }) {
                                panel_blocked = true;
                                return false;
                            }
                            true
                        })
                    });
                    // The floor takes no exemption: walking IS its job,
                    // so a desk-shadowed floor cell is a build error.
                    if !workable && panel_blocked && !matches!(station, Station::BayFloor) {
                        station_fronted += 1;
                        continue;
                    }
                    assert!(
                        workable,
                        "{station:?} cell ({x}, {y}) at {probe} is out of reach from everywhere"
                    );
                }
            }
        }
        // The exemption stays an exception, not a loophole: the floor
        // and aft wall must be fully workable, so a regression that
        // fronts half the net with panels cannot pass quietly. The
        // budget tightened when the chart tank walked off the port
        // wall — the counter's own shadow is all that is left of it,
        // and that goes with the barter redesign.
        assert!(
            station_fronted <= 6,
            "{station_fronted} cells are station-fronted; the walls are vanishing"
        );
    }

    /// The airlock's whole point, mechanised: the biggest footprint in
    /// the game fits through the door and inside the chamber — and only
    /// JUST. Both bounds matter; a roomy airlock is a second cargo bay.
    #[test]
    fn the_biggest_crate_just_fits_the_airlock() {
        let biggest = 2.0 * BAY_CELL;
        assert!(AIR_INNER >= biggest + 0.04, "chamber too tight");
        assert!(AIR_INNER <= biggest + 0.20, "chamber roomier than 'just'");
        assert!((AIR_DOOR_Z1 - AIR_DOOR_Z0 - AIR_INNER).abs() < 1e-6);
        assert!(AIR_DOOR_H >= biggest + 0.10, "door too low");
        assert!(AIR_DOOR_H <= biggest + 0.30, "door taller than 'just'");
        assert!((AIR_X1 - AIR_X0 - AIR_INNER).abs() < 1e-6);
    }

    /// Every airlock tile is workable from the walk envelope: within
    /// REACH, within the neck's pitch, unoccluded — the doorway segments
    /// must actually leave the doorway open.
    #[test]
    fn airlock_tiles_are_workable_from_the_envelope() {
        let panels = panels();
        let slabs = structure(&panels);
        for slot in 0..4u8 {
            let (probe, _) = crate::airlock::site(slot);
            let probe = probe + Vec3::Y * 0.004;
            let workable = (0..=12u8).any(|i| {
                (0..=12u8).any(|j| {
                    let eye = Vec3::new(
                        (f32::from(i) / 12.0).mul_add(WALK_MAX.x - WALK_MIN.x, WALK_MIN.x),
                        EYE_HEIGHT,
                        (f32::from(j) / 12.0).mul_add(WALK_MAX.z - WALK_MIN.z, WALK_MIN.z),
                    );
                    let dir = probe - eye;
                    let pitch = (-dir.y).atan2(dir.xz().length()).abs();
                    dir.length() <= REACH - 0.05
                        && pitch <= PITCH_LIMIT - 0.02
                        && !slabs.iter().any(|slab| {
                            ray_slab_entry(eye, dir, slab).is_some_and(|t| t < 1.0 - 1e-3)
                        })
                })
            });
            assert!(
                workable,
                "airlock tile {slot} is out of reach from everywhere"
            );
        }
    }

    /// The walk envelope spans the floor chart with body clearance at
    /// the walls: the room grid made the whole floor walkable ground,
    /// and every floor cell must be reachable on foot. The clamp
    /// answers to the hull alone — cargo is walked through, so no
    /// stack of crates can ever fence a corner off.
    #[test]
    fn walk_envelope_covers_the_floor() {
        let floor = bay()
            .into_iter()
            .find(|(station, _)| matches!(station, Station::BayFloor))
            .map(|(_, s)| s)
            .expect("floor chart");
        let aft_edge = floor.center.z + floor.half_v.length();
        let front_edge = floor.center.z - floor.half_v.length();
        assert!(
            WALK_MAX.z >= aft_edge - 0.20,
            "envelope stops {} short of the aft floor row",
            aft_edge - WALK_MAX.z
        );
        // The envelope's front edge stops short of the desk furniture;
        // the front floor rows need only be within working reach of it
        // (the workability test proves each cell individually).
        assert!(
            WALK_MIN.z <= front_edge + REACH - 0.6,
            "the front floor rows sit beyond working reach of the envelope"
        );
        assert!(
            WALK_MAX.x >= floor.half_u.length() - 0.40
                && -WALK_MIN.x >= floor.half_u.length() - 0.40,
            "envelope too narrow for the floor's side columns"
        );
    }
}
