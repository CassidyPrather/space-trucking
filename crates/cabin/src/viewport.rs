//! The cabin's front window, and what is actually on the other side of it.
//!
//! This module used to *paint* the void: a software canvas of hashed
//! stars, a berth disc, a growing destination, streak dashes. It looked
//! the same from every angle, because it was a picture hung in a frame —
//! and a picture is exactly what a window is not. What lives here now is
//! a **real exterior**, in world space, seen through a **real aperture**.
//!
//! Four ideas carry it:
//!
//! - **The void is a place, not a texture.** Stars, the destination, the
//!   berth alongside, the company that calls, and the ship's own hull all
//!   stand in world coordinates on their own render layer
//!   ([`VOID_LAYER`]). Nothing about them knows a window exists.
//! - **The window is a hole, and a hole has a frustum.** [`aim_skies`]
//!   reads the glass quad's live pose — the piece rig's, wherever the
//!   crew rehung it — and builds the eye's **off-axis** projection
//!   through it ([`Aperture`]): apex at the eye, near plane cut on the
//!   aperture. Move your head and the frustum shears; the same pane
//!   frames different space. Look at it from the side and you see what is
//!   out there in *that* direction. That is not a parallax effect, it is
//!   the projection actually being right.
//! - **Windows are cargo, so there are as many as the crew bought — and
//!   one wall is one sky.** Panes bolted to the same plane share ONE
//!   render: the sky is drawn once through the rectangle that bounds
//!   them all, and each pane reads its own rectangle back out of it
//!   ([`Sky`], [`sub_uv`]). That is not an approximation. Two co-planar
//!   apertures from one eye differ only by which sub-rectangle of the
//!   same near plane they cut, so the bigger render CONTAINS the smaller
//!   one exactly, and the remap between them is affine — which is what a
//!   material's `uv_transform` is. Eight windows on a wall cost one pass
//!   over the exterior, not eight ([`MAX_SKIES`] bounds the whole thing).
//! - **The ship's outside is the lattice's outside.** Every attached
//!   room's shell ([`hull_outside`]) is grown from the very `room::Plan`
//!   pose its interior is built from, so the trade room you walked out of
//!   is bolted to your hull exactly where you left it. The treatment is
//!   deliberately plain — plate, seams, a running light — because the
//!   per-POI design agents dress it later ([`outfit`], [`dress`]).
//!
//! The sim stays the authority for all of it: `ShipState` decides which
//! world is ahead and which astern, `is_warp`/`stoked` set the stream,
//! `encounter`/`parade`/`advertising` bring the company, `Cue::Jump`
//! floods the glass, and `light`/`omen` lean on the starlight itself. The
//! only clocks kept here are the ones the sim refuses to own
//! ([`SkyClock`]), and they freeze wholesale while the world is paused.
//!
//! Sell the window and the hull is simply solid: no pane, no aperture, no
//! camera, no view. Rehang it and the void follows (docs/BAY.md).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::camera::primitives::{Frustum, Sphere as Bounds};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraProjection, Hdr, RenderTarget, SubCameraView};
use bevy::image::ImageSampler;
use bevy::math::{Affine2, Vec3A};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::transform::TransformSystems;

use space_trucking::sim::room::RoomKind;
use space_trucking::sim::{
    COMET, Cue, EncounterKind, GUILD, PoiId, SATURN, ShipState, Sim, Vec2 as SimVec2, WANDERER,
    WARP_FACTOR, splitmix,
};

use crate::palette;
use crate::rig::CabinCamera;
use crate::room::Plan;
use crate::{Phase, Shell};

// ------------------------------------------------------------------ layer --

/// The void's own render layer. Space and the ship's outside are drawn
/// only by the porthole camera; the cabin camera never sees them, so the
/// hull stays solid from inside and no exterior lamp can leak through a
/// wall. One number, and it is the whole isolation story.
pub const VOID_LAYER: usize = 1;

/// The layer as a component, spelled once.
const fn outside() -> RenderLayers {
    RenderLayers::layer(VOID_LAYER)
}

// ------------------------------------------------------------------- tune --

/// Render-side seeds; cosmetic only, never the sim's.
const STAR_SEED: u64 = 0x0BE7_57A6;
const STREAM_SEED: u64 = 0xD057_0001;
const METEOR_SEED: u64 = 0x3E7E_0C0C;
const DRONE_SEED: u64 = 0x00AD_2222;

/// Where the deep field hangs, metres. Far enough that walking the cabin
/// moves it not at all — stars are meant to sit at infinity — and inside
/// the porthole's far plane with room to spare.
const STAR_SHELL: f32 = 900.0;

/// How many stars each tint bucket carries, and how many buckets there
/// are. The buckets are what let the field stay "glint-white with the odd
/// faraway POI tint" on a handful of draw calls: one mesh apiece.
const STAR_PER_BUCKET: u64 = 1_000;
const STAR_BUCKETS: u64 = 6;

/// A star's half-size on the shell, metres, for the two size classes the
/// hash sorts them into. At [`STAR_SHELL`] these read as one and two
/// texels of the porthole's own crunch.
const STAR_SMALL: f32 = 2.2;
const STAR_BIG: f32 = 4.0;

/// How hard a star burns, and how hard a stream fleck does. Emissive
/// multipliers on near-black bodies, the same family every lamp aboard
/// belongs to — bloom supplies the halo.
const STAR_GLOW: f32 = 3.2;
const FLECK_GLOW: f32 = 1.4;

/// How fast the deep field turns, radians per second: ambient drift,
/// alive even docked, calm enough to watch with coffee.
const DRIFT: f32 = 0.0032;

/// The stream: near-field flecks the ship actually passes. Count, the
/// radius band they ride in (clear of the widest hull), how far fore and
/// aft the band runs, and how many metres of travel one odometer unit
/// buys.
const STREAM_COUNT: u64 = 150;
const STREAM_NEAR: f32 = 5.0;
const STREAM_FAR: f32 = 34.0;
const STREAM_SPAN: f32 = 110.0;
const STREAM_GAIN: f32 = 26.0;

/// A stream fleck's resting body, metres, and how far one unit of
/// smoothed speed stretches it. At 16× warp the flecks draw honest comet
/// tails; at cruise they are dashes.
const FLECK: f32 = 0.06;
const FLECK_LONG: f32 = 0.30;
const STREAK: f32 = 1.6;

/// Seconds for the smoothed speed to close most of a gap (feedback fast).
const SPEED_EASE: f32 = 0.22;

/// Stream-speed multiplier while the burner is stoked. The sim makes two
/// ticks of way per tick of fire — at cruise and under warp alike — so
/// the void doubles its streaks honestly rather than by vibes.
const STOKED_FACTOR: f32 = 2.0;

/// The idle crawl the stream keeps while parked, in speed units. Space is
/// never quite still, and a dead-still window reads as a broken one.
const IDLE_DRIFT: f32 = 0.05;

/// Where the ship's own middle is, in world units, and how far its nose
/// reaches. Every body outside is placed in this frame: the destination
/// grows off the bow (world −Z, the cabin's front wall), the origin
/// shrinks astern (+Z), the berth hangs off the port flank. Which is why
/// a window rehung on another wall shows a different sky — the sky was
/// never centred on the window.
const SHIP_MID: Vec3 = Vec3::new(0.0, 1.2, 0.2);

/// How far off the ship the three world slots stand, metres.
const AHEAD_AT: Vec3 = Vec3::new(-34.0, 0.0, -260.0);
const ASTERN_AT: Vec3 = Vec3::new(34.0, -12.0, 240.0);
const BERTH_AT: Vec3 = Vec3::new(-140.0, -34.0, -168.0);

/// The destination's radius at the handoff and at arrival, metres —
/// cubed in, so the middle of a long haul stays honestly empty and the
/// world then arrives all at once.
const DEST_R0: f32 = 1.2;
const DEST_R1: f32 = 62.0;

/// The origin's radius as it is cast off, metres; it squares away.
const HOME_R: f32 = 52.0;

/// The berth world's radius, metres — a flank of sky while docked.
const BERTH_R: f32 = 68.0;

/// Leg fraction where the view stops looking back and starts looking
/// ahead. Below it the origin shrinks astern; above it the destination
/// grows. Long hauls should feel long.
const HANDOFF: f32 = 0.15;

/// The jump seen from inside: the aperture floods violet and fades
/// squared.
const JUMP_LEN: f32 = 0.45;

/// The dock's own structure, metres: how far off the port flank the mast
/// stands, how tall it is, and how long its arms reach back toward the
/// hull. Near-field on purpose — this is where a docked crew's parallax
/// actually comes from.
const MAST_AT: Vec3 = Vec3::new(-7.4, -1.4, -3.2);
const MAST_H: f32 = 9.0;
const MAST_ARM: f32 = 3.4;

/// How thick a room's exterior seam rail is, and how far its running
/// lights stand off the plate.
const RAIL: f32 = 0.055;
const LAMP_R: f32 = 0.055;

/// The porthole's near plane stands this far INBOARD of the glass,
/// metres. The aperture the sky is seen through is the hole in the hull,
/// not the pane sitting proud of it — and a neighbour bolted flush to
/// that wall has its plate inside the hull's own thickness. Cut at the
/// pane and a mated room would be behind the near plane, which is a room
/// you cannot see because it is too close, which is nonsense.
const HULL_REACH: f32 = 0.5;

/// Texels per world metre of aperture. The porthole is crunched exactly
/// like everything else (`docs/ART_DIRECTION_3D.md`) — this is the CRT's
/// "pick a density and stick to it" discipline, chosen so the void reads
/// about as chunky as the room around it from a pace back.
const PANE_DENSITY: f32 = 176.0;

/// Texel bounds per axis: never a smear, never a second render budget.
/// The ceiling is the graceful degradation the whole pass leans on — a
/// sky wider than `PANE_MAX / PANE_DENSITY` metres crunches coarser
/// rather than costing more, which is what stops a wall of glass from
/// quietly becoming the frame budget.
const PANE_MIN: u32 = 32;
const PANE_MAX: u32 = 256;

/// **The hard bound on the whole exterior pass**: how many skies the
/// cabin will draw in one frame, whatever the crew has hung.
///
/// A sky is one render of the outside through one aperture. Panes on the
/// same plane share one ([`Grouping::Wall`]), so this is a count of
/// *walls with glass in them that the eye can actually see*, not of
/// windows — a room has four walls, and a crew would have to be looking
/// at eight window-bearing planes at once to reach it. Beyond it, the
/// remaining panes go dark: honest black glass, never a stale sky or
/// somebody else's. The cameras and their targets are allocated ONCE at
/// boot and reused, so hanging a ninth window allocates nothing at all.
pub const MAX_SKIES: usize = 8;

/// How far apart two panes' planes may drift and still count as one
/// wall, in millimetres of normal and of offset. Rigs bolted to the same
/// chart land on the same plane to the last bit; the tolerance is for
/// float noise, not for architecture.
const PLANE_GRAIN: f32 = 1000.0;

/// The porthole's far plane, metres — past the deep field.
const VOID_FAR: f32 = 4000.0;

/// The emissive multiplier the pane's glass wears. The window is not a
/// tube: no scanlines, no phosphor ramp, and the glass keeps a real
/// specular finish so the crew's own lamps glint off it.
const PANE_GLOW: f32 = 1.0;

/// Starlight: the one light the void owns, and the omen leans on it.
const STARLIGHT: f32 = 22_000.0;

// ------------------------------------------------------------- the plumbing --

/// The pool of exterior renders, allocated once at boot and reused
/// forever. Crate-visible because the transit window is CARGO: a piece
/// rig asks whether the void is available at all, and the glass it hangs
/// is dressed from here every frame ([`aim_skies`]).
///
/// `sizes` is each target's live texel count, tracked so a rehung window
/// re-cuts its sky to the new aperture instead of stretching the old one.
#[derive(Resource)]
pub struct Skies {
    images: [Handle<Image>; MAX_SKIES],
    sizes: [UVec2; MAX_SKIES],
    /// The jump's violet, shared by every flood in the pool: one flash,
    /// however many windows it comes through.
    flood: Handle<StandardMaterial>,
    /// How many skies the last frame actually drew — the number the
    /// stress test reads, and the number that must not track pane count.
    lit: usize,
}

impl Skies {
    /// How many skies were drawn last frame.
    #[must_use]
    pub const fn lit(&self) -> usize {
        self.lit
    }
}

/// How panes are gathered into skies. Dev tooling in one respect only:
/// [`Grouping::Pane`] exists so the shipped law can be MEASURED against
/// the thing it replaced, on the same code path, in the same frame loop.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grouping {
    /// **The law.** One sky per wall plane: every co-planar pane reads
    /// its own rectangle out of one render of the outside.
    #[default]
    Wall,
    /// One sky per pane, which is what the first exterior pass did — a
    /// camera and a target apiece. Kept reachable (`--grouping pane`)
    /// because "it scales" is a claim, and a claim wants a control.
    Pane,
}

/// The window's own glass quad. `pieces::build_kind` tags it; the skies
/// aim through whatever wears this, so hitting the tag is the whole
/// contract between the furniture and the view.
#[derive(Component)]
pub struct SkyPane;

/// One of the pooled cameras that draws the void, and which slot of
/// [`Skies`] it paints.
#[derive(Component, Clone, Copy)]
struct Sky(usize);

/// The flood quad, parented to a sky camera and sized to its near plane.
#[derive(Component)]
struct Flood;

/// The deep field's shell — pinned to the eye every frame, because stars
/// do not parallax for a walk across a freighter.
#[derive(Component)]
struct Deep;

/// One near-field fleck of the stream: its lane around the ship's axis
/// and its resting station along it.
#[derive(Component)]
struct Fleck {
    lane: Vec3,
    along: f32,
}

/// Which world slot a body rig is standing in.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// Alongside, while docked.
    Berth,
    /// Growing off the bow.
    Ahead,
    /// Shrinking astern.
    Astern,
}

/// A world rig: the POI it was built for, so a changed leg rebuilds it.
#[derive(Component)]
struct World {
    slot: Slot,
    poi: PoiId,
}

/// The dock's structure alongside, shown only while docked.
#[derive(Component)]
struct Mast;

/// Whatever is calling this leg, as real bodies rather than silhouettes.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Caller {
    Whale,
    /// One of three meteor lanes.
    Meteor(u8),
    /// One of five in the Grand Parade.
    Parade(u8),
    Drone,
}

/// The void's own starlight, dimmed and violet-cast by the omen exactly
/// as the cabin's lamps are.
#[derive(Component)]
struct Starlight;

/// Everything the ship's outside spawned last rebuild, so a graph change
/// retires exactly what it replaces.
#[derive(Component)]
struct Outside;

/// One room's plate, with the box it encloses.
///
/// The plate is double-sided (a neighbour bolted flush to your wall has
/// its shell inside your own hull's thickness, and a single-sided box
/// seen from inside is a room that vanished) — which means the shell of
/// the room you are STANDING IN is a closed box around the eye, and a
/// closed box around the eye is a blindfold. [`mind_the_hull`] drops
/// exactly that one plate each frame. Its seams and running lights stay:
/// your own hull's edge is a thing you should see out your own window.
#[derive(Component, Clone, Copy)]
struct Plate {
    lo: Vec3,
    hi: Vec3,
}

impl Plate {
    /// Whether the eye is inside this plate.
    fn holds(self, p: Vec3) -> bool {
        p.cmpge(self.lo).all() && p.cmple(self.hi).all()
    }
}

/// The three-slot world query, named so the signature stays readable.
type WorldQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static World,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (Without<Deep>, Without<Fleck>),
>;

/// Whatever is calling this leg, posed each frame.
type CallerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Caller,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (Without<Deep>, Without<Fleck>, Without<World>, Without<Mast>),
>;

/// The pooled sky cameras: which slot each paints, its aperture, and its
/// pose.
type SkyQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Sky,
        &'static mut Camera,
        &'static mut Projection,
        &'static mut Transform,
    ),
    (Without<Flood>, Without<CabinCamera>),
>;

/// The flood quads riding each sky's near plane, tagged with the slot
/// they belong to.
type FloodQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Sky,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    With<Flood>,
>;

/// The crew's own eye: where it stands, and what it can see from there.
type EyeQuery<'w, 's> =
    Query<'w, 's, (&'static GlobalTransform, &'static Frustum), (With<CabinCamera>, Without<Sky>)>;

/// Every pane standing this frame: its live pose and the glass material
/// the sky is dressed onto.
type GlassQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        &'static MeshMaterial3d<StandardMaterial>,
    ),
    With<SkyPane>,
>;

/// Motion clocks the sim refuses to own: a held phase clock, a streaming
/// odometer, the smoothed stream speed, and the jump-flood timer. Frozen
/// wholesale while paused — a paused world should look paused.
#[derive(Resource, Default)]
struct SkyClock {
    t: f32,
    run: f32,
    speed: f32,
    jump: f32,
}

impl SkyClock {
    /// Latch cues, then advance every phase — unless the world is paused,
    /// in which case only the latch happens and all motion holds.
    fn update(&mut self, dt: f32, sim: &Sim) {
        if sim.cues().iter().any(|cue| matches!(cue, Cue::Jump)) {
            self.jump = JUMP_LEN;
        }
        if sim.is_paused() {
            return;
        }
        self.jump = (self.jump - dt).max(0.0);
        let target = match sim.ship().state {
            ShipState::Traveling { .. } => {
                let base = if sim.is_warp() { WARP_FACTOR } else { 1.0 };
                if sim.stoked() {
                    base * STOKED_FACTOR
                } else {
                    base
                }
            }
            ShipState::Docked(_) => 0.0,
        };
        let ease = 1.0 - (-dt / SPEED_EASE).exp();
        self.speed = (target - self.speed).mul_add(ease, self.speed);
        self.t += dt;
        self.run = dt.mul_add(self.speed + IDLE_DRIFT, self.run);
    }

    /// Normalized jump heat.
    fn heat(&self) -> f32 {
        self.jump / JUMP_LEN
    }
}

/// The viewport plugin: build the void once, keep it honest every frame,
/// and aim the porthole through the glass after the transforms settle.
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyClock>()
            .init_resource::<Grouping>()
            .add_systems(PostStartup, spawn_void)
            .add_systems(
                Update,
                (hull_outside, drive_void, mind_the_hull).in_set(Phase::View),
            )
            // The aperture is the piece rig's own pose, and a rig's world
            // pose is only true once the propagation has run — so the aim
            // waits for it rather than reading a frame-old window. The
            // eye's own frustum is settled by then too, which is what the
            // "a pane nobody is looking at costs nothing" rule reads.
            .add_systems(
                PostUpdate,
                aim_skies
                    .after(TransformSystems::Propagate)
                    .after(bevy::camera::visibility::VisibilitySystems::UpdateFrusta),
            );
    }
}

/// The window's glass, hung DARK: a near-black pane with a real specular
/// finish and no sky on it at all. [`aim_skies`] dresses it every frame —
/// which sky it reads and which rectangle of that sky is its own is a
/// fact about where the crew last hung it, so the rig cannot know it and
/// does not try. Deliberately NOT `crt::tube_glass`: that is a tube
/// showing a rendering, and this is a hole showing space.
///
/// Unlit is also the honest resting state. A pane nobody is looking at,
/// a pane past [`MAX_SKIES`], and a pane facing away all read exactly
/// like this, because in each case the truthful answer is "no view".
#[must_use]
pub fn pane_glass(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: palette::GLASS,
        emissive: LinearRgba::BLACK,
        // Low roughness on purpose: the painted gleam that used to be
        // drawn into the canvas is a real reflection now, and the crew's
        // own lamps make it.
        perceptual_roughness: 0.22,
        metallic: 0.0,
        ..default()
    })
}

// -------------------------------------------------------------- the frustum --

/// An off-axis perspective through a rectangular hole.
///
/// The generalized projection every portal wants: the apex is the eye,
/// the frustum's cross-section at the near plane is the aperture, and the
/// four edges are therefore asymmetric whenever the eye is not square on
/// to the glass. Reverse-Z, finite far, to match the depth conventions
/// the rest of the pipeline is built on.
#[derive(Debug, Clone, Copy)]
struct Aperture {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
}

impl Aperture {
    /// The frustum an eye at `eye` sees through a quad centred at
    /// `centre` with in-plane half-extents `half_u`/`half_v` — plus the
    /// camera rotation that goes with it (the quad's own frame, turned so
    /// the eye looks OUT through the hole).
    ///
    /// The quad's own facing is `u × v`, which for the pane the rig hangs
    /// is its local +Z: into the room, where the crew is. `None` when the
    /// eye is on the other side of it or flush with its plane — a window
    /// has a front, and there is no view out of one you are standing
    /// behind.
    fn through(eye: Vec3, centre: Vec3, half_u: Vec3, half_v: Vec3) -> Option<(Self, Quat)> {
        let u = half_u.try_normalize()?;
        let v = half_v.try_normalize()?;
        // The pane's own facing, and how far off it the eye stands.
        let facing = u.cross(v);
        let toward = eye - centre;
        let depth = toward.dot(facing);
        if depth <= 1e-3 {
            return None;
        }
        // The camera's own frame: +X along the aperture's u, +Y along its
        // v, +Z back at the eye — so Bevy's own −Z looks out through the
        // hole, which is the whole trick.
        let rot = Quat::from_mat3(&Mat3::from_cols(u, v, facing));
        // The near plane stands inboard of the pane (see HULL_REACH), and
        // the edges scale with it — a frustum's shape is a ratio, not a
        // distance.
        let near = (depth - HULL_REACH).max(0.02);
        let scale = near / depth;
        let (du, dv) = (half_u.length(), half_v.length());
        let centre_u = -toward.dot(u);
        let centre_v = -toward.dot(v);
        Some((
            Self {
                left: (centre_u - du) * scale,
                right: (centre_u + du) * scale,
                bottom: (centre_v - dv) * scale,
                top: (centre_v + dv) * scale,
                near,
                far: VOID_FAR,
            },
            rot,
        ))
    }

    /// The aperture's own span at the near plane — what the flood quad is
    /// cut to, and what the target's texel count is derived from.
    const fn span(self) -> Vec2 {
        Vec2::new(self.right - self.left, self.top - self.bottom)
    }

    /// The aperture's centre at the near plane, in view space.
    const fn mid(self) -> Vec2 {
        Vec2::new(
            f32::midpoint(self.left, self.right),
            f32::midpoint(self.bottom, self.top),
        )
    }
}

impl CameraProjection for Aperture {
    fn get_clip_from_view(&self) -> Mat4 {
        // Kooima's generalized perspective, in the pipeline's own
        // conventions: right-handed, reverse-Z, finite far.
        let wide = self.right - self.left;
        let high = self.top - self.bottom;
        let (near, far) = (self.near, self.far);
        let depth = far - near;
        Mat4::from_cols(
            Vec4::new(2.0 * near / wide, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 * near / high, 0.0, 0.0),
            Vec4::new(
                (self.right + self.left) / wide,
                (self.top + self.bottom) / high,
                near / depth,
                -1.0,
            ),
            Vec4::new(0.0, 0.0, near * far / depth, 0.0),
        )
    }

    fn get_clip_from_view_for_sub(&self, _sub_view: &SubCameraView) -> Mat4 {
        // The porthole owns its whole target; there is no sub-view to cut.
        self.get_clip_from_view()
    }

    fn update(&mut self, _width: f32, _height: f32) {
        // The aperture is the window's, not the target's: a resized
        // texture must not re-shape the hole in the hull.
    }

    fn far(&self) -> f32 {
        self.far
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let a = z_near / self.near;
        let b = z_far / self.near;
        [
            Vec3A::new(self.right * a, self.bottom * a, -z_near),
            Vec3A::new(self.right * a, self.top * a, -z_near),
            Vec3A::new(self.left * a, self.top * a, -z_near),
            Vec3A::new(self.left * a, self.bottom * a, -z_near),
            Vec3A::new(self.right * b, self.bottom * b, -z_far),
            Vec3A::new(self.right * b, self.top * b, -z_far),
            Vec3A::new(self.left * b, self.top * b, -z_far),
            Vec3A::new(self.left * b, self.bottom * b, -z_far),
        ]
    }
}

// ------------------------------------------------------------ one wall, one sky --

/// A pane, reduced to the four facts a sky needs about it: where its
/// glass is, how big, which way it faces, and whose material to dress.
#[derive(Clone)]
struct Pane {
    glass: Handle<StandardMaterial>,
    centre: Vec3,
    /// World half-axes of the quad — the rig scaled a unit rectangle, so
    /// the transform's own axes ARE the extents.
    half_u: Vec3,
    half_v: Vec3,
    /// `half_u × half_v`, normalized: the way the glass looks, which for
    /// the pane the rig hangs is into the room, where the crew is.
    facing: Vec3,
}

impl Pane {
    /// The quad's four world corners, for measuring a wall's glass.
    fn corners(&self) -> [Vec3; 4] {
        [
            self.centre - self.half_u - self.half_v,
            self.centre + self.half_u - self.half_v,
            self.centre + self.half_u + self.half_v,
            self.centre - self.half_u + self.half_v,
        ]
    }
}

/// Which wall a pane belongs to, quantized to [`PLANE_GRAIN`] so two
/// panes bolted to the same chart key identically.
///
/// Ordered, and the grouping walk sorts on it: a stable order is what
/// keeps a sky slot on the same wall from frame to frame, and a slot that
/// wandered would swap two windows' skies for one frame every time the
/// crew walked past. Nobody would report it as a bug; everybody would
/// feel it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Wall([i32; 4], u64);

impl Wall {
    /// The wall a pane hangs on. `lone` splits every pane onto its own
    /// key — the control arm ([`Grouping::Pane`]), and the only thing
    /// that distinguishes it.
    fn of(pane: &Pane, lone: Option<u64>) -> Self {
        let grain = |v: f32| (v * PLANE_GRAIN).round() as i32;
        Self(
            [
                grain(pane.facing.x),
                grain(pane.facing.y),
                grain(pane.facing.z),
                grain(pane.centre.dot(pane.facing)),
            ],
            lone.unwrap_or(0),
        )
    }
}

/// A wall plane's own in-plane axes, derived from the plane and nothing
/// else — never from a pane, because a pane's frame is a fact about
/// which window happened to be first in a query and the sky must not be.
///
/// `u × v == facing` by construction, which is the handedness
/// [`Aperture::through`] reads. On any upright wall this comes out as
/// "`v` is up, `u` runs along the wall", which is also the least
/// surprising thing for the texel density to be measured along.
fn plane_axes(facing: Vec3) -> (Vec3, Vec3) {
    let up = if facing.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let u = up.cross(facing).normalize();
    (u, facing.cross(u))
}

/// The affine remap from a pane's own texture coordinates into the sky
/// its wall shares — **the one piece of arithmetic the whole pass rests
/// on**, so it is written out rather than fitted.
///
/// Two co-planar apertures seen from one eye have the same near plane
/// and the same view axis, so their projections differ by an affine map
/// of the near plane, and the wall's render therefore CONTAINS each
/// pane's render exactly. `(u, v)` are the plane's own axes and
/// `(lo, hi)` the wall rectangle's bounds in them; the result takes a
/// pane-local `uv` (Bevy's rectangle: `u` along local +X, `v` DOWN from
/// local +Y) to the same point in the sky's image (`u` along the
/// aperture's +u, `v` down from its +v, because a render target's first
/// row is the top of the frame).
fn sub_uv(pane: &Pane, u: Vec3, v: Vec3, lo: Vec2, hi: Vec2) -> Affine2 {
    let span = (hi - lo).max(Vec2::splat(f32::EPSILON));
    let (cu, cv) = (pane.centre.dot(u), pane.centre.dot(v));
    let (uu, uv) = (pane.half_u.dot(u), pane.half_u.dot(v));
    let (vu, vv) = (pane.half_v.dot(u), pane.half_v.dot(v));
    // x(a, b) = (cu + (2a-1)·uu + (1-2b)·vu - lo.x) / span.x
    // y(a, b) = (hi.y - cv - (2a-1)·uv - (1-2b)·vv) / span.y
    Affine2::from_mat2_translation(
        Mat2::from_cols(
            Vec2::new(2.0 * uu / span.x, -2.0 * uv / span.y),
            Vec2::new(-2.0 * vu / span.x, 2.0 * vv / span.y),
        ),
        Vec2::new(
            (cu - uu + vu - lo.x) / span.x,
            (hi.y - cv + uv - vv) / span.y,
        ),
    )
}

/// One wall's sky: the aperture the outside is drawn through, the camera
/// pose that goes with it, the target's texel count, and the panes that
/// will read their own rectangles back out of it.
struct SkyPlan {
    aperture: Aperture,
    rot: Quat,
    size: UVec2,
    panes: Vec<(Handle<StandardMaterial>, Affine2)>,
}

/// Gather the standing panes into skies: one per wall, in wall order,
/// each drawn through the rectangle that bounds its own glass.
///
/// Everything that cannot be served comes back in the second half of the
/// pair, to be hung as dark glass — a pane the eye is behind, a pane
/// whose wall has no slot left, a pane scaled to nothing by a sold
/// window. There is no third state: a window either shows the actual
/// outside or it shows nothing at all.
fn plan_skies(panes: Vec<(u64, Pane)>, eye: Vec3, how: Grouping) -> (Vec<SkyPlan>, Vec<Pane>) {
    let mut keyed: Vec<(Wall, Pane)> = panes
        .into_iter()
        .map(|(id, pane)| {
            let lone = matches!(how, Grouping::Pane).then_some(id);
            (Wall::of(&pane, lone), pane)
        })
        .collect();
    keyed.sort_by_key(|(key, _)| *key);

    let mut plans: Vec<SkyPlan> = Vec::new();
    let mut dark: Vec<Pane> = Vec::new();
    let mut rest = keyed.as_slice();
    while let Some((key, _)) = rest.first() {
        let key = *key;
        let end = rest.partition_point(|(k, _)| *k <= key);
        let (wall, tail) = rest.split_at(end);
        rest = tail;
        if plans.len() >= MAX_SKIES {
            dark.extend(wall.iter().map(|(_, pane)| pane.clone()));
            continue;
        }
        let facing = wall[0].1.facing;
        let (u, v) = plane_axes(facing);
        // The wall's glass, measured in its own plane: the rectangle that
        // holds every pane on it. One window and this IS the window.
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        for (_, pane) in wall {
            for corner in pane.corners() {
                let at = Vec2::new(corner.dot(u), corner.dot(v));
                lo = lo.min(at);
                hi = hi.max(at);
            }
        }
        let mid = (lo + hi) * 0.5;
        let seed = wall[0].1.centre;
        let centre = seed + u * (mid.x - seed.dot(u)) + v * (mid.y - seed.dot(v));
        let half = (hi - lo) * 0.5;
        let Some((aperture, rot)) = Aperture::through(eye, centre, u * half.x, v * half.y) else {
            dark.extend(wall.iter().map(|(_, pane)| pane.clone()));
            continue;
        };
        plans.push(SkyPlan {
            aperture,
            rot,
            // Cut to the GLASS, not to the frustum: the void keeps one
            // texel density whatever wall it ends up on, and a rehung
            // window re-cuts its own texture instead of stretching it.
            size: UVec2::new(
                pane_texels(2.0 * half.x * PANE_DENSITY),
                pane_texels(2.0 * half.y * PANE_DENSITY),
            ),
            panes: wall
                .iter()
                .map(|(_, pane)| (pane.glass.clone(), sub_uv(pane, u, v, lo, hi)))
                .collect(),
        });
    }
    (plans, dark)
}

// ---------------------------------------------------------------- the void --

/// A blank porthole target: small, unsmoothed, the same recipe the crunch
/// itself uses because it is the same discipline.
fn void_target(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// Build the outside: the target, the porthole camera, the deep field,
/// the stream, the three world slots' rigs (empty until a leg needs
/// them), the dock's mast, the company, the flood, and the one light out
/// there. The ship's own shells arrive with the graph ([`hull_outside`]).
#[allow(clippy::too_many_lines)] // one paragraph per body out there; splitting scatters the census
fn spawn_void(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // The sky pool, allocated ONCE and never grown: [`MAX_SKIES`]
    // cameras, [`MAX_SKIES`] targets, and that is the whole exterior
    // budget however many windows the crew buys. Each draws BEFORE the
    // cabin camera (order −2 against its −1) so the glass it feeds is
    // this frame's, not last frame's, and each clears to the void because
    // that is what is behind everything.
    let size = UVec2::new(PANE_MIN * 4, PANE_MIN * 2);
    let targets: [Handle<Image>; MAX_SKIES] =
        std::array::from_fn(|_| images.add(void_target(size)));
    // One flood mesh and one flood material for the whole pool: the jump
    // is the same violet in every window at once.
    let sheet = meshes.add(Rectangle::new(1.0, 1.0));
    let flood = materials.add(StandardMaterial {
        base_color: palette::EERIE_BRIGHT.with_alpha(0.0),
        emissive: LinearRgba::BLACK,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.insert_resource(Skies {
        images: targets.clone(),
        sizes: [size; MAX_SKIES],
        flood: flood.clone(),
        lit: 0,
    });
    for (slot, image) in targets.into_iter().enumerate() {
        let camera = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    order: -2,
                    clear_color: ClearColorConfig::Custom(palette::VOID),
                    is_active: false,
                    ..default()
                },
                Projection::custom(Aperture {
                    left: -1.0,
                    right: 1.0,
                    bottom: -1.0,
                    top: 1.0,
                    near: 0.1,
                    far: VOID_FAR,
                }),
                RenderTarget::Image(image.into()),
                Hdr,
                Bloom::NATURAL,
                Msaa::Off,
                Transform::default(),
                outside(),
                Sky(slot),
            ))
            .id();

        // The jump flood: a quad riding this sky's own near plane, cut to
        // the aperture every frame. It is the last thing between the eye
        // and space, which is exactly what a jump feels like.
        commands.spawn((
            Mesh3d(sheet.clone()),
            MeshMaterial3d(flood.clone()),
            Transform::default(),
            Visibility::Hidden,
            outside(),
            Flood,
            Sky(slot),
            ChildOf(camera),
        ));
    }

    // The deep field: one mesh per tint bucket, pinned to the eye.
    let shell = commands
        .spawn((Transform::default(), Visibility::default(), outside(), Deep))
        .id();
    for bucket in 0..STAR_BUCKETS {
        let tint = star_tint(bucket);
        commands.spawn((
            Mesh3d(meshes.add(star_mesh(bucket))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: palette::SHADOW,
                emissive: tint.to_linear() * STAR_GLOW,
                ..default()
            })),
            Transform::default(),
            outside(),
            ChildOf(shell),
        ));
    }

    // The stream: the ship's own passage, made of things it passes.
    let fleck = meshes.add(Cuboid::new(FLECK, FLECK, FLECK_LONG));
    let fleck_mat = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::GLINT.to_linear() * FLECK_GLOW,
        ..default()
    });
    for i in 0..STREAM_COUNT {
        let h = splitmix(STREAM_SEED, i);
        let angle = ((h & 0xFFFF) as f32 / 65_535.0) * TAU;
        let reach =
            (((h >> 16) & 0xFFFF) as f32 / 65_535.0).mul_add(STREAM_FAR - STREAM_NEAR, STREAM_NEAR);
        let along = (((h >> 32) & 0xFFFF) as f32 / 65_535.0) * STREAM_SPAN;
        commands.spawn((
            Mesh3d(fleck.clone()),
            MeshMaterial3d(fleck_mat.clone()),
            Transform::default(),
            outside(),
            Fleck {
                lane: Vec3::new(angle.cos() * reach, angle.sin() * reach * 0.6, 0.0),
                along,
            },
        ));
    }

    // The dock alongside: a real mast with real arms and real lamps, so a
    // docked crew's window has near-field to parallax against.
    berth_mast(&mut commands, &mut meshes, &mut materials);

    // The company, standing by dark until their encounter opens.
    callers(&mut commands, &mut meshes, &mut materials);

    // Starlight. The void owns exactly one source, it casts no shadows
    // (art direction: light volumes, never simulated occlusion), and the
    // omen dims it like everything else aboard.
    commands.spawn((
        DirectionalLight {
            color: palette::PLATE_LIT,
            illuminance: STARLIGHT,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(SHIP_MID).looking_to(Vec3::new(-0.5, -0.35, 0.8), Vec3::Y),
        outside(),
        Starlight,
    ));
}

/// A star bucket's hue: mostly glint-white in two values, with two
/// buckets carrying a faint tint of somewhere you could go. The only
/// green out there is still a whale.
fn star_tint(bucket: u64) -> Color {
    match bucket {
        0 => palette::GLINT,
        1 => palette::ICON_LIT,
        2 => palette::mix(palette::GLINT, palette::ICON_LIT, 0.5),
        3 => palette::mix(palette::GLINT, palette::POI_URANUS, 0.35),
        4 => palette::mix(palette::GLINT, palette::POI_MARS, 0.30),
        _ => palette::mix(palette::ICON_LIT, palette::POI_EARTH, 0.35),
    }
}

/// One bucket of the deep field, as a single mesh of camera-facing quads
/// on the shell. Hashed off [`STAR_SEED`] like every other decoration, so
/// the sky is the same sky every boot.
fn star_mesh(bucket: u64) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(STAR_PER_BUCKET as usize * 4);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(STAR_PER_BUCKET as usize * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(STAR_PER_BUCKET as usize * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(STAR_PER_BUCKET as usize * 6);
    for i in 0..STAR_PER_BUCKET {
        let h = splitmix(STAR_SEED ^ bucket.wrapping_mul(0x9E37_79B9), i);
        // An even scatter over the sphere: the z is uniform, the angle is
        // uniform, which is the one distribution that does not clot at
        // the poles.
        let z = ((h & 0xFFFF) as f32 / 65_535.0).mul_add(2.0, -1.0);
        let angle = (((h >> 16) & 0xFFFF) as f32 / 65_535.0) * TAU;
        let ring = (1.0 - z * z).max(0.0).sqrt();
        let dir = Vec3::new(ring * angle.cos(), z, ring * angle.sin());
        let size = if (h >> 40).trailing_zeros() >= 3 {
            STAR_BIG
        } else {
            STAR_SMALL
        };
        // A quad squared to the shell: the eye sits at the centre, so
        // facing the middle is facing the camera.
        let up = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let right = up.cross(dir).normalize() * size;
        let over = dir.cross(right).normalize() * size;
        let at = dir * STAR_SHELL;
        let base = positions.len() as u32;
        for (dx, dy, uv) in [
            (-1.0, -1.0, [0.0, 1.0]),
            (1.0, -1.0, [1.0, 1.0]),
            (1.0, 1.0, [1.0, 0.0]),
            (-1.0, 1.0, [0.0, 0.0]),
        ] {
            let corner = at + right * dx + over * dy;
            positions.push([corner.x, corner.y, corner.z]);
            normals.push([-dir.x, -dir.y, -dir.z]);
            uvs.push(uv);
        }
        // Wound to face the shell's middle, where the eye is: a star
        // squared the other way is a star the culler eats.
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// The berth's own hardware alongside: a mast, two arms reaching back at
/// the hull, and the amber lamps every dock in the game keeps burning.
/// Hidden until the ship is actually parked at one.
fn berth_mast(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let plate = materials.add(StandardMaterial {
        base_color: palette::PLATE,
        perceptual_roughness: 0.94,
        metallic: 0.18,
        ..default()
    });
    let lamp = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::AMBER.to_linear() * 5.0,
        ..default()
    });
    let root = commands
        .spawn((
            Transform::from_translation(SHIP_MID + MAST_AT),
            Visibility::Hidden,
            outside(),
            Mast,
        ))
        .id();
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.7, MAST_H, 0.7))),
        MeshMaterial3d(plate.clone()),
        Transform::default(),
        outside(),
        ChildOf(root),
    ));
    let arm = meshes.add(Cuboid::new(MAST_ARM, 0.28, 0.28));
    let bulb = meshes.add(Sphere::new(0.16).mesh().ico(1).expect("ico in range"));
    for sy in [-1.0_f32, 1.0] {
        commands.spawn((
            Mesh3d(arm.clone()),
            MeshMaterial3d(plate.clone()),
            Transform::from_xyz(MAST_ARM * 0.5, sy * MAST_H * 0.3, 0.0),
            outside(),
            ChildOf(root),
        ));
        commands.spawn((
            Mesh3d(bulb.clone()),
            MeshMaterial3d(lamp.clone()),
            Transform::from_xyz(MAST_ARM, sy * MAST_H * 0.3, 0.0),
            outside(),
            ChildOf(root),
        ));
    }
    // The gantry beam overhead, which is the bit that says "somebody
    // built this" rather than "a pole is here".
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.4, 0.4, 7.0))),
        MeshMaterial3d(plate),
        Transform::from_xyz(0.0, MAST_H * 0.46, 0.0),
        outside(),
        ChildOf(root),
    ));
}

/// Everything that calls on a leg, spawned dark: the whale, three meteor
/// lanes, five of the Grand Parade, and the ad drone. Each is real
/// geometry out there now rather than a silhouette painted on glass —
/// which means the whale passes BEHIND the ship's own hull when it does,
/// and nobody had to write that down.
#[allow(clippy::too_many_lines)] // one caller per paragraph; splitting scatters the census
fn callers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // The whale: vast, patient, deep green rising to a lit back.
    let hide = materials.add(StandardMaterial {
        base_color: palette::mix(palette::PHOSPHOR_DIM, palette::ICON_LIT, 0.22),
        perceptual_roughness: 0.95,
        ..default()
    });
    let back = materials.add(StandardMaterial {
        base_color: palette::mix(palette::PHOSPHOR_DIM, palette::ICON_LIT, 0.55),
        perceptual_roughness: 0.9,
        emissive: palette::PHOSPHOR_DIM.to_linear() * 0.4,
        ..default()
    });
    let glint = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::GLINT.to_linear() * 4.0,
        ..default()
    });
    let whale = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            outside(),
            Caller::Whale,
        ))
        .id();
    let body = meshes.add(Sphere::new(1.0).mesh().ico(2).expect("ico in range"));
    commands.spawn((
        Mesh3d(body.clone()),
        MeshMaterial3d(hide.clone()),
        Transform::default().with_scale(Vec3::new(15.0, 4.2, 4.6)),
        outside(),
        ChildOf(whale),
    ));
    commands.spawn((
        Mesh3d(body.clone()),
        MeshMaterial3d(back),
        Transform::from_xyz(-1.2, 1.7, 0.0).with_scale(Vec3::new(11.0, 1.6, 3.0)),
        outside(),
        ChildOf(whale),
    ));
    for (at, scale) in [
        (Vec3::new(-17.5, 2.4, 0.0), Vec3::new(4.0, 3.2, 0.5)),
        (Vec3::new(-17.0, -2.0, 0.0), Vec3::new(3.6, 2.6, 0.5)),
        (Vec3::new(1.5, -3.0, 2.6), Vec3::new(4.0, 0.7, 2.4)),
    ] {
        commands.spawn((
            Mesh3d(body.clone()),
            MeshMaterial3d(hide.clone()),
            Transform::from_translation(at).with_scale(scale),
            outside(),
            ChildOf(whale),
        ));
    }
    commands.spawn((
        Mesh3d(body.clone()),
        MeshMaterial3d(glint.clone()),
        Transform::from_xyz(11.0, 1.0, 2.0).with_scale(Vec3::splat(0.42)),
        outside(),
        ChildOf(whale),
    ));

    // Meteors: three lanes of gravel weather, each a lit rod.
    let rod = meshes.add(Cuboid::new(0.5, 0.5, 9.0));
    for lane in 0..3_u8 {
        commands.spawn((
            Mesh3d(rod.clone()),
            MeshMaterial3d(glint.clone()),
            Transform::default(),
            Visibility::Hidden,
            outside(),
            Caller::Meteor(lane),
        ));
    }

    // The Grand Parade: eerie heptagons, at distance, explaining nothing.
    let eerie = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::EERIE.to_linear() * 2.2,
        ..default()
    });
    let hept = meshes.add(Cylinder::new(1.0, 0.35).mesh().resolution(7).build());
    for i in 0..5_u8 {
        commands.spawn((
            Mesh3d(hept.clone()),
            MeshMaterial3d(eerie.clone()),
            Transform::default(),
            Visibility::Hidden,
            outside(),
            Caller::Parade(i),
        ));
    }

    // The ad drone: an amber dot lugging a billboard nobody can read.
    let amber = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::AMBER.to_linear() * 4.0,
        ..default()
    });
    let drone = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            outside(),
            Caller::Drone,
        ))
        .id();
    commands.spawn((
        Mesh3d(body),
        MeshMaterial3d(amber),
        Transform::default().with_scale(Vec3::splat(0.30)),
        outside(),
        ChildOf(drone),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.6, 0.9, 0.05))),
        MeshMaterial3d(glint),
        Transform::from_xyz(0.0, 1.1, 0.0),
        outside(),
        ChildOf(drone),
    ));
}

// ------------------------------------------------------- the ship, outside --

/// The exterior kit for one room: the plate its shell wears and the
/// running lights it burns.
///
/// **This is one of the two seams a per-station design agent works
/// through**, and it is no longer keyed by room kind — one `Trade` kind
/// serves twelve stations, and twelve identical shells was the defect
/// this pass exists to fix. The ship's own two rooms keep their kind's
/// kit; everything alongside wears its station's
/// ([`crate::poi::Character::outfit`]). The other seam is [`dress`].
#[must_use]
pub const fn outfit(kind: RoomKind, host: Option<crate::poi::Host>) -> crate::poi::Outfit {
    crate::poi::outfit(kind, host)
}

/// The second seam: whatever a station bolts to its shell BEYOND the
/// neutral plate — a mast, a hangar maw, a refinery's flare stack, a
/// planet-side POI's space-elevator ribbon.
///
/// The hardware is **data** ([`crate::poi::Character::dress`]) rather than
/// a spawn callback, which is what lets the containment laws be arithmetic
/// a test can run instead of a habit a reviewer has to notice. It is
/// measured off the shell's own half-extents in the room's own axes, so a
/// retuned lattice carries the dressing with it — and **the dressing may
/// not move the room**, because the shell's box is derived from the pose
/// and the pose is the sim's.
fn dress(
    commands: &mut Commands,
    shapes: &crate::poi::Shapes,
    materials: &mut Assets<StandardMaterial>,
    fittings: &[crate::poi::Fitting],
    root: Entity,
    frame: crate::poi::Frame,
) {
    for fitting in fittings {
        commands.spawn((
            Mesh3d(shapes.of(fitting.shape)),
            MeshMaterial3d(fitting.coat.material(materials, None)),
            // The root already stands at the shell's centre, so the
            // fitting's placement is taken relative to a frame at the
            // origin and inherited from there.
            crate::poi::Frame {
                mid: Vec3::ZERO,
                ..frame
            }
            .place(fitting),
            outside(),
            ChildOf(root),
        ));
    }
}

/// Grow the ship's outside from the plan, whenever the graph changes
/// shape. One shell per attached room, straight off `room::hull_box` —
/// the same pose its interior is built from, which is the whole point:
/// the trade room is bolted where you walked out of it, and nothing here
/// gets to have an opinion about where that is.
fn hull_outside(
    mut commands: Commands,
    plan: Res<Plan>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    standing: Query<Entity, With<Outside>>,
) {
    if !plan.is_changed() {
        return;
    }
    for entity in &standing {
        commands.entity(entity).despawn();
    }
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let bulb = meshes.add(Sphere::new(LAMP_R).mesh().ico(1).expect("ico in range"));
    let shapes = crate::poi::Shapes::new(&mut meshes);
    for placed in &plan.rooms {
        let (lo, hi) = crate::room::hull_box(placed);
        let mid = (lo + hi) * 0.5;
        let span = hi - lo;
        let kit = outfit(placed.kind, placed.host);
        let root = commands
            .spawn((
                Transform::from_translation(mid),
                Visibility::default(),
                outside(),
                Outside,
            ))
            .id();
        // The plate itself, DOUBLE-SIDED on purpose: a room mated flush
        // to the wall you are looking out of has its shell inside your
        // own hull's thickness, and a single-sided box seen from inside
        // is a room that vanished. Plate you cannot see past is the
        // honest reading of "there is a room bolted here".
        let plate = materials.add(StandardMaterial {
            base_color: kit.plate,
            perceptual_roughness: 0.95,
            metallic: 0.12,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(plate),
            Transform::default().with_scale(span),
            outside(),
            Plate { lo, hi },
            ChildOf(root),
        ));
        // Seams: a belt around the middle and a post at every corner. The
        // whole vocabulary, and deliberately the whole vocabulary.
        let seam = materials.add(StandardMaterial {
            base_color: palette::PLATE_SHADE,
            perceptual_roughness: 0.9,
            metallic: 0.25,
            ..default()
        });
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(seam.clone()),
            Transform::from_xyz(0.0, span.y * -0.12, 0.0).with_scale(Vec3::new(
                span.x + RAIL,
                RAIL,
                span.z + RAIL,
            )),
            outside(),
            ChildOf(root),
        ));
        for sx in [-1.0_f32, 1.0] {
            for sz in [-1.0_f32, 1.0] {
                commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(seam.clone()),
                    Transform::from_xyz(sx * span.x * 0.5, 0.0, sz * span.z * 0.5)
                        .with_scale(Vec3::new(RAIL, span.y + RAIL, RAIL)),
                    outside(),
                    ChildOf(root),
                ));
            }
        }
        // Running lights, at the top corners the kit pays for.
        let lamp = materials.add(StandardMaterial {
            base_color: palette::GLASS,
            emissive: kit.lamp.to_linear() * 6.0,
            ..default()
        });
        for i in 0..kit.lamps {
            let sx = if i % 2 == 0 { -1.0 } else { 1.0 };
            commands.spawn((
                Mesh3d(bulb.clone()),
                MeshMaterial3d(lamp.clone()),
                Transform::from_xyz(sx * span.x * 0.5, span.y * 0.5, sx * -span.z * 0.5),
                outside(),
                ChildOf(root),
            ));
        }
        dress(
            &mut commands,
            &shapes,
            &mut materials,
            crate::poi::character_of(placed.host).dress,
            root,
            crate::poi::Frame::of(lo, hi, placed.yaw),
        );
    }
}

/// Drop the one plate the eye is standing inside.
///
/// The room you are in is a box around you out there as much as in here,
/// and a double-sided box around the camera hides the entire universe.
/// Every OTHER room's plate stays: a neighbour's shell filling the window
/// is the honest reading of "there is a room bolted to this wall".
fn mind_the_hull(
    eye: Query<&GlobalTransform, With<CabinCamera>>,
    mut plates: Query<(&Plate, &mut Visibility)>,
) {
    let Ok(eye) = eye.single() else {
        return;
    };
    let at = eye.translation();
    for (plate, mut vis) in &mut plates {
        vis.set_if_neq(if plate.holds(at) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}

// ---------------------------------------------------------- worlds outside --

/// Which world stands in which slot this frame, straight off the sim.
/// `None` where the slot has nothing to show.
fn slots(sim: &Sim) -> [(Slot, Option<(PoiId, f32)>); 3] {
    match sim.ship().state {
        ShipState::Docked(at) => [
            (Slot::Berth, Some((at, 1.0))),
            (Slot::Ahead, None),
            (Slot::Astern, None),
        ],
        ShipState::Traveling {
            from,
            to,
            progress,
            leg_ticks,
        } => {
            let frac = progress as f32 / leg_ticks.max(1) as f32;
            if frac < HANDOFF {
                [
                    (Slot::Berth, None),
                    (Slot::Ahead, None),
                    (Slot::Astern, Some((from, frac / HANDOFF))),
                ]
            } else {
                [
                    (Slot::Berth, None),
                    (Slot::Ahead, Some((to, (frac - HANDOFF) / (1.0 - HANDOFF)))),
                    (Slot::Astern, None),
                ]
            }
        }
    }
}

/// Where a slot's world stands and how big it reads, given its own
/// growth `g`. The destination cubes in — the middle of a long haul stays
/// honestly empty and the world then arrives all at once — and the origin
/// squares away as it is cast off.
fn world_pose(slot: Slot, g: f32) -> (Vec3, f32) {
    match slot {
        Slot::Berth => (SHIP_MID + BERTH_AT, BERTH_R),
        Slot::Ahead => (
            SHIP_MID + AHEAD_AT,
            (DEST_R1 - DEST_R0).mul_add(g.powi(3), DEST_R0),
        ),
        Slot::Astern => (
            SHIP_MID + ASTERN_AT.lerp(ASTERN_AT * 2.4, g),
            (1.0 - g).powi(2) * HOME_R,
        ),
    }
}

/// One world, as the naked eye sees it: its identity hue and one cheap
/// accent, in unit radius so the slot's own scale sizes it. The 2D
/// glyph vocabulary, kept — Jupiter's bands, the two ring systems, the
/// Guild's heptagon, the comet's thrown tail, and whatever ??? is.
#[allow(clippy::too_many_lines)] // one world per arm; the vocabulary is the point
fn world_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    slot: Slot,
    poi: PoiId,
) -> Entity {
    let hue = palette::poi_color(poi);
    let skin = materials.add(StandardMaterial {
        base_color: hue,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        // Worlds carry a breath of their own light, so a lampless void
        // still shows one. Enamel, not phosphor.
        emissive: hue.to_linear() * 0.22,
        ..default()
    });
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            outside(),
            World { slot, poi },
        ))
        .id();
    let ball = meshes.add(Sphere::new(1.0).mesh().ico(3).expect("ico in range"));
    let accent = |materials: &mut Assets<StandardMaterial>, color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.9,
            emissive: color.to_linear() * 0.18,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };
    match poi {
        // Jupiter: two darker bands across the disc.
        3 => {
            commands.spawn((
                Mesh3d(ball),
                MeshMaterial3d(skin),
                Transform::default(),
                outside(),
                ChildOf(root),
            ));
            let band = accent(materials, palette::mix(hue, palette::SHADOW, 0.45));
            for fy in [-0.30_f32, 0.24] {
                let chord = fy.mul_add(-fy, 1.0).sqrt();
                commands.spawn((
                    Mesh3d(meshes.add(Torus::new(chord * 0.97, chord * 1.02))),
                    MeshMaterial3d(band.clone()),
                    Transform::from_xyz(0.0, fy, 0.0),
                    outside(),
                    ChildOf(root),
                ));
            }
        }
        // Uranus and Saturn: a thin tilted ring, and the broad graveyard.
        4 | SATURN => {
            let (body_r, ring_in, ring_out, tilt, tone) = if poi == SATURN {
                (
                    0.85,
                    1.25,
                    1.95,
                    -0.28,
                    accent(materials, palette::accent::SATURN_RING),
                )
            } else {
                (
                    0.9,
                    1.35,
                    1.7,
                    0.32,
                    accent(materials, palette::accent::URANUS_RING),
                )
            };
            commands.spawn((
                Mesh3d(ball),
                MeshMaterial3d(skin),
                Transform::default().with_scale(Vec3::splat(body_r)),
                outside(),
                ChildOf(root),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Torus::new(ring_in, ring_out))),
                MeshMaterial3d(tone),
                Transform::default()
                    .with_rotation(Quat::from_rotation_z(tilt))
                    .with_scale(Vec3::new(1.0, 0.04, 1.0)),
                outside(),
                ChildOf(root),
            ));
        }
        // The Guild: a heptagon on edge, its rim picked out.
        GUILD => {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.0, 0.45).mesh().resolution(7).build())),
                MeshMaterial3d(skin),
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                outside(),
                ChildOf(root),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.08, 0.18).mesh().resolution(7).build())),
                MeshMaterial3d(accent(materials, palette::accent::GUILD_EDGE)),
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                outside(),
                ChildOf(root),
            ));
        }
        // The comet: an icy head with its tail thrown up and away.
        COMET => {
            commands.spawn((
                Mesh3d(ball),
                MeshMaterial3d(skin),
                Transform::default().with_scale(Vec3::splat(0.7)),
                outside(),
                ChildOf(root),
            ));
            let tail = materials.add(StandardMaterial {
                base_color: hue.with_alpha(0.35),
                emissive: hue.to_linear() * 0.5,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let cone = meshes.add(Cone {
                radius: 0.75,
                height: 4.2,
            });
            for (spread, lean) in [(0.0_f32, 0.0_f32), (0.5, 0.24), (-0.45, -0.2)] {
                commands.spawn((
                    Mesh3d(cone.clone()),
                    MeshMaterial3d(tail.clone()),
                    Transform::from_xyz(spread.mul_add(0.4, 1.4), 1.1, spread * 0.6)
                        .with_rotation(Quat::from_rotation_z(lean - 2.2)),
                    outside(),
                    ChildOf(root),
                ));
            }
        }
        // ???: a diamond that is not entirely committed to existing.
        WANDERER => {
            let ghost = materials.add(StandardMaterial {
                base_color: hue.with_alpha(0.6),
                emissive: hue.to_linear() * 1.2,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.0, 1.9).mesh().resolution(4).build())),
                MeshMaterial3d(ghost),
                Transform::from_rotation(Quat::from_rotation_z(PI * 0.25)),
                outside(),
                ChildOf(root),
            ));
        }
        // Everybody else is a world, and a world is a ball.
        _ => {
            commands.spawn((
                Mesh3d(ball),
                MeshMaterial3d(skin),
                Transform::default(),
                outside(),
                ChildOf(root),
            ));
        }
    }
    root
}

// ------------------------------------------------------------ every frame --

/// Advance the clocks and read the whole exterior off the sim: which
/// worlds stand where, how hard the stream runs, who is calling, how far
/// the omen has leaned on the starlight, and how hot the jump is.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn drive_void(
    time: Res<Time>,
    shell: Res<Shell>,
    mut commands: Commands,
    mut clock: ResMut<SkyClock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    eye: Query<&GlobalTransform, With<CabinCamera>>,
    mut deep: Query<&mut Transform, (With<Deep>, Without<Fleck>)>,
    mut flecks: Query<(&Fleck, &mut Transform), Without<Deep>>,
    mut worlds: WorldQuery,
    mut mast: Query<&mut Visibility, (With<Mast>, Without<World>)>,
    mut callers: CallerQuery,
    mut starlight: Query<&mut DirectionalLight, With<Starlight>>,
) {
    let sim = &shell.bridge.sim;
    clock.update(time.delta_secs(), sim);
    let t = clock.t;

    // The deep field rides the eye and turns on the ambient drift. It
    // hangs on the CREW, not on a sky: every window aboard shares one
    // field, because the stars are at infinity and a freighter is not
    // wide enough to disagree with itself about where infinity is.
    if let Ok(mut shellt) = deep.single_mut() {
        let eye = eye.single().map_or(
            SHIP_MID,
            bevy::transform::components::GlobalTransform::translation,
        );
        shellt.translation = eye;
        shellt.rotation = Quat::from_euler(EulerRot::YXZ, t * DRIFT, t * DRIFT * 0.37, 0.0);
    }

    // The stream: real motion, aft, stretched by the smoothed speed.
    let stretch = clock.speed.mul_add(STREAK, 1.0);
    let run = clock.run * STREAM_GAIN;
    for (fleck, mut transform) in &mut flecks {
        let along = STREAM_SPAN.mul_add(-0.5, (fleck.along + run).rem_euclid(STREAM_SPAN));
        transform.translation = SHIP_MID + fleck.lane + Vec3::Z * along;
        transform.scale = Vec3::new(1.0, 1.0, stretch);
    }

    // The worlds. A slot whose POI changed is rebuilt, because a world's
    // accent is its identity and identities do not morph.
    let want = slots(sim);
    let mut standing = [false; 3];
    for (entity, world, mut transform, mut vis) in &mut worlds {
        let index = match world.slot {
            Slot::Berth => 0,
            Slot::Ahead => 1,
            Slot::Astern => 2,
        };
        match want[index].1 {
            Some((poi, g)) if poi == world.poi => {
                standing[index] = true;
                let (at, r) = world_pose(world.slot, g);
                transform.translation = at;
                transform.scale = Vec3::splat(r.max(0.01));
                transform.rotation = Quat::from_rotation_y(t * 0.05);
                vis.set_if_neq(Visibility::Visible);
            }
            Some(_) => commands.entity(entity).despawn(),
            None => {
                vis.set_if_neq(Visibility::Hidden);
                standing[index] = true;
            }
        }
    }
    for (index, (slot, wanted)) in want.into_iter().enumerate() {
        if standing[index] {
            continue;
        }
        if let Some((poi, _)) = wanted {
            world_rig(&mut commands, &mut meshes, &mut materials, slot, poi);
        }
    }

    // The dock's own structure stands only where a dock is.
    let docked = matches!(sim.ship().state, ShipState::Docked(_));
    if let Ok(mut vis) = mast.single_mut() {
        vis.set_if_neq(if docked {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }

    // The company.
    for (caller, mut transform, mut vis) in &mut callers {
        let shown = match *caller {
            Caller::Whale => whale_pose(sim, t, &mut transform),
            Caller::Meteor(lane) => meteor_pose(sim, t, lane, &mut transform),
            Caller::Parade(i) => parade_pose(sim, t, i, &mut transform),
            Caller::Drone => drone_pose(sim, t, &mut transform),
        };
        vis.set_if_neq(if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }

    // The omen leans on space itself: the one light out there dims with
    // the room's and takes the violet, exactly as `fx::dim_cabin` does
    // inside. Nothing gets to opt out of the hum.
    if let Ok(mut light) = starlight.single_mut() {
        light.illuminance = STARLIGHT * sim.light().max(0.25);
        light.color = palette::mix(palette::PLATE_LIT, palette::EERIE, sim.omen() * 0.8);
    }
}

/// The whale, crossing the whole view over its encounter and gone.
fn whale_pose(sim: &Sim, t: f32, transform: &mut Transform) -> bool {
    let ShipState::Traveling { progress, .. } = sim.ship().state else {
        return false;
    };
    let Some(enc) = sim.encounter() else {
        return false;
    };
    if !enc.open() || enc.kind != EncounterKind::Whale {
        return false;
    }
    let frac = progress.saturating_sub(enc.start) as f32 / (enc.end - enc.start).max(1) as f32;
    transform.translation = SHIP_MID
        + Vec3::new(
            frac.mul_add(180.0, -90.0),
            (t * 0.35).sin().mul_add(3.0, -6.0),
            -74.0,
        );
    transform.rotation = Quat::from_rotation_y((t * 0.18).sin() * 0.12);
    true
}

/// Gravel weather: three lanes, each throwing a rod past the hull.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn meteor_pose(sim: &Sim, t: f32, lane: u8, transform: &mut Transform) -> bool {
    let Some(enc) = sim.encounter() else {
        return false;
    };
    if !enc.open() || enc.kind != EncounterKind::MeteorShower {
        return false;
    }
    let cyc = t.mul_add(1.0 / 2.6, f32::from(lane) * 0.37);
    let ph = cyc.fract();
    if ph > 0.28 {
        return false;
    }
    let f = ph / 0.28;
    let h = splitmix(METEOR_SEED.wrapping_add(u64::from(lane)), cyc as u64);
    let start = SHIP_MID
        + Vec3::new(
            ((h & 0xFFFF) as f32 / 65_535.0).mul_add(90.0, -45.0),
            (((h >> 16) & 0xFFFF) as f32 / 65_535.0).mul_add(40.0, 6.0),
            -46.0,
        );
    let along = Vec3::new(26.0, -30.0, 12.0);
    transform.translation = start + along * f;
    transform.rotation = Quat::from_rotation_arc(Vec3::Z, along.normalize());
    true
}

/// The Grand Parade at distance, high and unbothered.
fn parade_pose(sim: &Sim, t: f32, i: u8, transform: &mut Transform) -> bool {
    let Some(frac) = sim.parade() else {
        return false;
    };
    let lead = frac.mul_add(300.0, -150.0);
    let x = f32::from(i).mul_add(-11.0, lead);
    if !(-160.0..=160.0).contains(&x) {
        return false;
    }
    let r = if i == 0 {
        3.4
    } else {
        f32::from(i).mul_add(-0.28, 2.4)
    };
    transform.translation = SHIP_MID
        + Vec3::new(
            x,
            t.mul_add(1.3, f32::from(i) * 1.1)
                .sin()
                .mul_add(1.6, f32::from(i).mul_add(2.2, 34.0)),
            -128.0,
        );
    transform.rotation = Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_y(t * 2.0);
    transform.scale = Vec3::splat(r);
    true
}

/// The ad drone, zipping past every seven seconds while attached.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn drone_pose(sim: &Sim, t: f32, transform: &mut Transform) -> bool {
    if !sim.advertising() {
        return false;
    }
    let cycle = t / 7.0;
    let ph = cycle.fract();
    if ph > 0.30 {
        return false;
    }
    let f = ph / 0.30;
    let h = splitmix(DRONE_SEED, cycle as u64);
    let lane = ((h & 0xFFFF) as f32 / 65_535.0).mul_add(9.0, -3.0);
    transform.translation = SHIP_MID
        + Vec3::new(
            f.mul_add(52.0, -26.0),
            (f * 14.0).sin().mul_add(0.8, lane),
            -13.0,
        );
    transform.rotation = Quat::from_rotation_y((f * 6.0).sin() * 0.2);
    true
}

// ------------------------------------------------------------- the porthole --

/// Aim every sky the standing glass calls for, and dress each pane with
/// its own rectangle of the one its wall shares.
///
/// This is the whole rendering contract in one system:
///
/// 1. **Read the glass.** Every `SkyPane`'s live world pose — its piece
///    rig's, wherever the crew rehung it. A pane the eye stands behind,
///    a pane scaled to nothing, and a pane outside the eye's own frustum
///    are all set aside here: a window nobody can see is a window that
///    costs nothing, which is the only reason the bound in
///    [`MAX_SKIES`] is generous rather than tight.
/// 2. **Gather by wall.** Co-planar panes share one sky ([`plan_skies`]).
/// 3. **Aim.** Park each pooled camera at the eye in its aperture's
///    frame, hand it the off-axis frustum, and re-cut its target to the
///    wall's own glass at [`PANE_DENSITY`].
/// 4. **Dress.** Each pane's material takes the sky as its emissive and
///    the affine remap ([`sub_uv`]) that picks its rectangle out of it.
///    A pane with no sky takes black: no window, no aperture, no camera,
///    no view, and the hull is solid.
#[allow(clippy::too_many_arguments)]
fn aim_skies(
    mut skies: ResMut<Skies>,
    how: Res<Grouping>,
    clock: Res<SkyClock>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    glass: GlassQuery,
    eye: EyeQuery,
    mut cameras: SkyQuery,
    mut floods: FloodQuery,
) {
    let Ok((eye, seen)) = eye.single() else {
        // No eye aboard: nothing to be a hole for.
        skies.lit = 0;
        for (_, mut camera, _, _) in &mut cameras {
            camera.is_active = false;
        }
        for (_, _, glass) in &glass {
            unlit(&mut materials, &glass.0);
        }
        return;
    };
    let at = eye.translation();

    let mut standing: Vec<(u64, Pane)> = Vec::new();
    for (entity, pose, glass) in &glass {
        // The pane's own frame: the glass is a unit rectangle the rig
        // scaled to its size, so the transform's own axes ARE its extents.
        let axes = pose.affine().matrix3;
        let half_u = Vec3::from(axes.x_axis) * 0.5;
        let half_v = Vec3::from(axes.y_axis) * 0.5;
        let centre = pose.translation();
        // A window sold, scaled to nothing, is not a hole.
        let (Some(u), Some(v)) = (half_u.try_normalize(), half_v.try_normalize()) else {
            unlit(&mut materials, &glass.0);
            continue;
        };
        let facing = u.cross(v);
        // A window has a front, and the eye must be in front of it — and
        // it must be somewhere the eye could actually look. A sphere
        // around the glass is the conservative test: it never hides a
        // pane that is really on screen, and it catches every pane on
        // the wall behind the crew's back, which is where the saving is.
        let showing = (at - centre).dot(facing) > 1e-3
            && seen.intersects_sphere(
                &Bounds {
                    center: centre.into(),
                    radius: (half_u + half_v).length(),
                },
                true,
            );
        if showing {
            standing.push((
                entity.to_bits(),
                Pane {
                    glass: glass.0.clone(),
                    centre,
                    half_u,
                    half_v,
                    facing,
                },
            ));
        } else {
            unlit(&mut materials, &glass.0);
        }
    }

    let (plans, dark) = plan_skies(standing, at, *how);
    for pane in &dark {
        unlit(&mut materials, &pane.glass);
    }
    skies.lit = plans.len();

    for (sky, mut camera, mut projection, mut transform) in &mut cameras {
        let Some(plan) = plans.get(sky.0) else {
            camera.is_active = false;
            continue;
        };
        camera.is_active = true;
        transform.translation = at;
        transform.rotation = plan.rot;
        if let Projection::Custom(custom) = &mut *projection
            && let Some(slot) = custom.get_mut::<Aperture>()
        {
            *slot = plan.aperture;
        }
        if plan.size != skies.sizes[sky.0] {
            skies.sizes[sky.0] = plan.size;
            // A dropped handle would mean a sky that no longer exists,
            // which is a slot this loop has already declined to serve.
            drop(images.insert(&skies.images[sky.0], void_target(plan.size)));
        }
        for (glass, uv) in &plan.panes {
            lit(&mut materials, glass, &skies.images[sky.0], *uv);
        }
    }

    // The floods ride their own skies' near planes, cut to the aperture
    // exactly. The jump floods every window at once — it is the space
    // between them that lights up, not the glass.
    let heat = clock.heat();
    for (sky, mut quad, mut vis) in &mut floods {
        let Some(plan) = plans.get(sky.0) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(if heat > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        let mid = plan.aperture.mid();
        let span = plan.aperture.span();
        quad.translation = Vec3::new(mid.x, mid.y, -plan.aperture.near * 1.001);
        quad.scale = Vec3::new(span.x * 1.02, span.y * 1.02, 1.0);
    }
    let level = heat * heat;
    let (tint, burn) = (
        palette::EERIE_BRIGHT.with_alpha(level * 0.95),
        palette::EERIE_BRIGHT.to_linear() * (level * 3.0),
    );
    // Read before write, like the glass: a jump is a fifth of a second
    // every few legs, and the other 99% of frames have nothing to say.
    let settled = materials
        .get(&skies.flood)
        .is_some_and(|mat| mat.base_color == tint && mat.emissive == burn);
    if !settled && let Some(mut flood) = materials.get_mut(&skies.flood) {
        flood.base_color = tint;
        flood.emissive = burn;
    }
}

/// Hang a sky on a pane: the shared render as emissive, and the affine
/// remap that makes it this pane's rectangle of it.
///
/// The read-before-write is not fussiness — an `Assets` handle taken
/// mutably re-uploads the material, and a window is a thing you stand
/// still and look at, so most frames have nothing to say.
fn lit(
    materials: &mut Assets<StandardMaterial>,
    glass: &Handle<StandardMaterial>,
    sky: &Handle<Image>,
    uv: Affine2,
) {
    let settled = materials.get(glass).is_some_and(|mat| {
        mat.emissive_texture.as_ref() == Some(sky)
            && mat.uv_transform == uv
            && mat.emissive == LinearRgba::WHITE * PANE_GLOW
    });
    if settled {
        return;
    }
    if let Some(mut mat) = materials.get_mut(glass) {
        mat.emissive_texture = Some(sky.clone());
        mat.emissive = LinearRgba::WHITE * PANE_GLOW;
        mat.uv_transform = uv;
    }
}

/// Put a pane out: dark glass, no sky at all. What a sold window, a
/// window on the wall behind you, and a window past the bound all look
/// like, because they are all the same fact.
fn unlit(materials: &mut Assets<StandardMaterial>, glass: &Handle<StandardMaterial>) {
    if materials
        .get(glass)
        .is_some_and(|mat| mat.emissive_texture.is_none())
    {
        return;
    }
    if let Some(mut mat) = materials.get_mut(glass) {
        mat.emissive_texture = None;
        mat.emissive = LinearRgba::BLACK;
        mat.uv_transform = Affine2::IDENTITY;
    }
}

/// One axis of the porthole target, clamped and quantized to a step the
/// texture allocator will not thrash over.
#[allow(clippy::cast_sign_loss)] // the aperture's span is positive by construction
fn pane_texels(raw: f32) -> u32 {
    let stepped = (raw.max(0.0) as u32).div_ceil(8) * 8;
    stepped.clamp(PANE_MIN, PANE_MAX)
}

// ----------------------------------------------------------------- presets --

/// Every standing pane's chart and where on it the glass sits, cabin
/// first and then by piece id. The presets read the BOARD rather than
/// remembering where a window was last time, which is what makes a
/// rehung window take its own screenshots with it.
type GlassChart = (
    space_trucking::sim::room::RoomId,
    crate::surface::Station,
    crate::surface::SimSurface,
    SimVec2,
);

fn glass_charts(sim: &Sim) -> Vec<GlassChart> {
    use space_trucking::sim::room::CABIN;
    let pieces = sim.pieces();
    let mut glass: Vec<_> = pieces
        .iter()
        .filter(|piece| {
            piece.kind.window() && matches!(piece.loc, space_trucking::sim::Loc::Hold { .. })
        })
        .collect();
    glass.sort_by_key(|piece| (piece.loc.room(pieces) != Some(CABIN), piece.id));
    glass
        .into_iter()
        .filter_map(|piece| {
            let home = piece.loc.room(pieces)?;
            let at =
                crate::canvas::rect_center(space_trucking::sim::layout::piece_rect(pieces, piece));
            sim.rooms().iter().find_map(|(id, room)| {
                crate::room::charts(id, room)
                    .into_iter()
                    .find(|(_, surface)| surface.rect.contains(at))
                    .map(|(station, surface)| (home, station, surface, at))
            })
        })
        .collect()
}

/// A `--view` preset standing at whatever wall the window is actually
/// hung on: `pane` square on to the glass, `pane-port` and `pane-stbd`
/// a stride to either side of it, and `panes` back far enough to hold
/// every window aboard in one frame.
///
/// Dev tooling, derived like every other preset in the game (`room::preset`):
/// it asks the board where the glass is rather than remembering where it
/// was last time, so a rehung window takes its own screenshots with it.
/// The two off-centre poses exist to be *compared* — the same pane, two
/// eyes, two cones of space, which is the pass's whole claim.
#[must_use]
pub fn preset(sim: &Sim, name: &str) -> Option<(Vec3, f32, f32)> {
    // `panes` is the multi-window view, and it is derived the same way
    // the rest are: it stands back from the MEAN of one room's standing
    // glass along the mean of the walls those panes are set in, so a
    // ship with two windows on two walls gets the corner shot that shows
    // both at once, and a ship with eight on one wall gets that wall
    // square on. Which is the pass's whole claim, in one frame. One
    // room, because the shot is a shot from somewhere and a crew cannot
    // stand in two rooms at once.
    if name == "panes" {
        let charts = glass_charts(sim);
        let room = charts.first().map(|(room, ..)| *room)?;
        let mut mid = Vec3::ZERO;
        let mut out = Vec3::ZERO;
        let mut count = 0.0_f32;
        for (home, station, surface, at) in charts {
            if home != room {
                continue;
            }
            mid += surface.to_world(at);
            out += station.inward(&surface);
            count += 1.0;
        }
        if count == 0.0 {
            return None;
        }
        let mid = mid / count;
        let out = out.normalize_or_zero();
        let stand = mid + out * 2.15;
        let eye = Vec3::new(stand.x, crate::rig::EYE_HEIGHT, stand.z);
        let d = mid - eye;
        return Some((eye, (-d.x).atan2(-d.z), d.y.atan2(d.xz().length())));
    }
    let aside = match name {
        "pane" => 0.0,
        "pane-port" => -0.95,
        "pane-stbd" => 0.95,
        _ => return None,
    };
    let (_, station, surface, at) = glass_charts(sim).into_iter().next()?;
    let centre = surface.to_world(at);
    let along = surface.half_u.normalize_or_zero();
    let stand = centre + station.inward(&surface) * 1.75 + along * aside;
    let eye = Vec3::new(stand.x, crate::rig::EYE_HEIGHT, stand.z);
    // Look ALONGSIDE the glass rather than at it. Where the eye stands is
    // what decides the view out (that is the whole pass); where the head
    // is turned only decides the framing — and a crosshair resting on
    // cargo lights that piece's own footprint frame
    // (`pieces::hover_glint`), which is a glowing wireframe box standing
    // in front of the thing these shots are of.
    let d = along.mul_add(Vec3::splat(0.62), centre) - eye;
    Some((eye, (-d.x).atan2(-d.z), d.y.atan2(d.xz().length())))
}

// ------------------------------------------------------------------ tests --

#[cfg(test)]
mod tests {
    use super::*;
    use space_trucking::sim::room::{CABIN, RoomKind};
    use space_trucking::sim::{InputFrame, TICK_DT, Vec2 as SimVec2, layout};

    /// A pane one metre wide and half a metre tall, hung on a wall and
    /// looking `out` — the geometry any window ever has. Its own facing
    /// (`u × v`) is into the room, exactly as the rig hangs it.
    fn pane_at(centre: Vec3, out: Vec3, up: Vec3) -> (Vec3, Vec3, Vec3) {
        let facing = -out.normalize();
        let v = up.normalize();
        (centre, v.cross(facing) * 0.5, v * 0.25)
    }

    /// Where a world point lands in the porthole's clip space, as NDC.
    fn ndc(aperture: Aperture, rot: Quat, eye: Vec3, at: Vec3) -> Vec3 {
        let view = rot.inverse() * (at - eye);
        let clip = aperture.get_clip_from_view() * view.extend(1.0);
        clip.truncate() / clip.w
    }

    /// The whole point of the pass: the same pane, two eye positions, two
    /// different cones of space. A painted sky cannot do this, which is
    /// why the painted sky is gone.
    ///
    /// Note which way it goes, because it is the way holes work and not
    /// the way pictures do: standing to STARBOARD of the glass shows you
    /// the space off the PORT bow, and stepping across the cabin swaps
    /// the sky for a different one.
    #[test]
    fn moving_the_head_frames_different_space() {
        let (centre, half_u, half_v) = pane_at(Vec3::new(0.0, 1.4, -1.9), Vec3::NEG_Z, Vec3::Y);
        let port = Vec3::new(-0.9, 1.5, 0.4);
        let stbd = Vec3::new(0.9, 1.5, 0.4);
        let (a, arot) = Aperture::through(port, centre, half_u, half_v).expect("a view out");
        let (b, brot) = Aperture::through(stbd, centre, half_u, half_v).expect("a view out");
        // A star far off the port bow.
        let star = Vec3::new(-60.0, 1.4, -120.0);
        let from_port = ndc(a, arot, port, star);
        let from_stbd = ndc(b, brot, stbd, star);
        assert!(
            from_stbd.x.abs() <= 1.0,
            "the starboard eye should frame the port star, got {from_stbd}"
        );
        assert!(
            from_port.x.abs() > 1.0,
            "the port eye must NOT frame it, got {from_port}"
        );
        // And the frames genuinely differ — not two crops of one picture.
        assert!(
            (from_port.x - from_stbd.x).abs() > 1.0,
            "two eyes, one pane, one sky: {from_port} vs {from_stbd}"
        );
    }

    /// A point dead ahead through the middle of the glass lands dead
    /// centre, from wherever the eye stands. That is what "the projection
    /// is actually right" means, stated as an assertion.
    #[test]
    fn the_glass_centre_is_the_frame_centre() {
        let (centre, half_u, half_v) = pane_at(Vec3::new(0.3, 1.4, -1.9), Vec3::NEG_Z, Vec3::Y);
        for eye in [
            Vec3::new(0.3, 1.4, 0.5),
            Vec3::new(-1.4, 1.1, 1.2),
            Vec3::new(1.5, 1.8, 0.2),
        ] {
            let (aperture, rot) = Aperture::through(eye, centre, half_u, half_v).expect("a view");
            // Straight out through the middle of the pane, and on.
            let beyond = eye + (centre - eye) * 40.0;
            let hit = ndc(aperture, rot, eye, beyond);
            assert!(
                hit.x.abs() < 1e-3 && hit.y.abs() < 1e-3,
                "the pane's middle must be the frame's middle, got {hit} from {eye}"
            );
            assert!(
                (0.0..=1.0).contains(&hit.z),
                "reverse-Z depth must stay in clip, got {hit}"
            );
        }
    }

    /// Rehang the window and the void follows: the same eye, the same
    /// room, two walls, two directions of space.
    #[test]
    fn a_rehung_window_turns_the_void_with_it() {
        let eye = Vec3::new(0.0, 1.5, 0.3);
        let (fc, fu, fv) = pane_at(Vec3::new(0.0, 1.4, -1.9), Vec3::NEG_Z, Vec3::Y);
        let (pc, pu, pv) = pane_at(Vec3::new(-2.1, 1.4, 0.3), Vec3::NEG_X, Vec3::Y);
        let (_, front) = Aperture::through(eye, fc, fu, fv).expect("a view forward");
        let (_, port) = Aperture::through(eye, pc, pu, pv).expect("a view to port");
        let ahead = front * Vec3::NEG_Z;
        let aside = port * Vec3::NEG_Z;
        assert!(ahead.z < -0.9, "the front pane must look forward: {ahead}");
        assert!(aside.x < -0.9, "the port pane must look to port: {aside}");
    }

    /// The hull is solid where the window is not: a window has a front,
    /// and standing behind it (or in its plane) is not a view.
    #[test]
    fn there_is_no_view_from_behind_the_glass() {
        let (centre, half_u, half_v) = pane_at(Vec3::new(0.0, 1.4, -1.9), Vec3::NEG_Z, Vec3::Y);
        // Out in the void, looking back at the pane from the wrong side.
        assert!(Aperture::through(Vec3::new(0.0, 1.4, -4.0), centre, half_u, half_v).is_none());
        // Exactly in the plane of the glass: no aperture at all.
        assert!(Aperture::through(Vec3::new(1.0, 1.4, -1.9), centre, half_u, half_v).is_none());
        // And a pane with no extent — a window sold, scaled to nothing —
        // is likewise not a hole.
        assert!(Aperture::through(Vec3::new(0.0, 1.4, 0.4), centre, Vec3::ZERO, half_v).is_none());
    }

    /// Every attached room's outside is grown from the very pose its
    /// inside is: the shell holds the whole interior box, and no two
    /// shells are the same box.
    #[test]
    fn the_ship_wears_its_own_lattice_outside() {
        let sim = Sim::from_save(crate::fixture::SAVE).expect("the fixture parses");
        let rooms = sim.rooms();
        let placed: Vec<_> = rooms
            .iter()
            .map(|(id, room)| crate::room::placed(id, room))
            .collect();
        assert!(placed.len() >= 3, "the fixture flies three rooms");
        for room in &placed {
            let (lo, hi) = crate::room::hull_box(room);
            assert!(
                lo.cmple(room.lo).all() && hi.cmpge(room.hi).all(),
                "{:?}'s shell must hold its own interior: {lo}..{hi} vs {}..{}",
                room.kind,
                room.lo,
                room.hi
            );
        }
        for (a, b) in placed.iter().zip(placed.iter().skip(1)) {
            assert!(
                crate::room::hull_box(a).0 != crate::room::hull_box(b).0,
                "two rooms cannot share one shell"
            );
        }
        // The cabin is the root, so its shell is the one the whole
        // lattice is pinned to.
        assert!(
            placed.iter().any(|room| room.id == CABIN),
            "the cabin flies with the ship"
        );
    }

    /// The shell you are standing in is dropped and every other one
    /// stands. A double-sided box around the eye is a blindfold; a
    /// neighbour's plate filling the window is the point.
    #[test]
    fn the_shell_you_stand_in_is_the_one_that_goes() {
        let sim = Sim::from_save(crate::fixture::SAVE).expect("the fixture parses");
        let rooms = sim.rooms();
        let plates: Vec<Plate> = rooms
            .iter()
            .map(|(id, room)| {
                let (lo, hi) = crate::room::hull_box(&crate::room::placed(id, room));
                Plate { lo, hi }
            })
            .collect();
        // The cabin's own boot pose: standing mid-cabin at eye height.
        let eye = Vec3::new(0.0, crate::rig::EYE_HEIGHT, 0.9);
        let dropped = plates.iter().filter(|plate| plate.holds(eye)).count();
        assert_eq!(dropped, 1, "exactly one shell holds a standing crew");
        // And out in the void, none of them do.
        let outside = Vec3::new(0.0, 4.0, -40.0);
        assert!(
            plates.iter().all(|plate| !plate.holds(outside)),
            "a shell must not claim the void"
        );
    }

    /// Every room kind has an exterior kit **at every station that could
    /// wear it**, and the derelict is the one that burns nothing — the
    /// tell a design agent must not delete by accident.
    #[test]
    fn every_room_kind_is_dressed_for_the_void() {
        for kind in space_trucking::sim::room::ROOM_KINDS {
            for host in std::iter::once(None).chain(crate::poi::HOSTS.map(Some)) {
                let kit = outfit(kind, host);
                assert!(
                    kit.lamps <= 2,
                    "{kind:?} at {host:?} carries a lighting rig"
                );
            }
            if kind == RoomKind::Wreck {
                assert_eq!(
                    outfit(kind, Some(crate::poi::Host::Wreck)).lamps,
                    0,
                    "a derelict burns nothing"
                );
            }
        }
    }

    /// A press-and-hold input frame at a world position.
    fn press_at(pos: SimVec2) -> InputFrame {
        InputFrame {
            pointer: pos,
            press: true,
            held: true,
            ..InputFrame::default()
        }
    }

    /// A sim freshly cast off: chart the first POI the dock allows, pull
    /// the launch lever, tick once.
    fn launched(seed: u64) -> Sim {
        let mut sim = Sim::new(seed);
        let ShipState::Docked(at) = sim.ship().state else {
            panic!("a fresh sim starts docked");
        };
        let target = (0..12_u8)
            .find(|&id| id != at && sim.poi_chartable(id))
            .expect("somewhere must be chartable from the starting dock");
        sim.advance(0.0, &press_at(sim.poi_pos(target)));
        sim.advance(
            0.0,
            &press_at(crate::canvas::rect_center(layout::LAUNCH_LEVER)),
        );
        sim.advance(TICK_DT, &InputFrame::default());
        assert!(
            matches!(sim.ship().state, ShipState::Traveling { .. }),
            "the lever should cast off"
        );
        sim
    }

    /// The sim stays the authority: docked hangs a world alongside, the
    /// early leg leaves one astern, and past the handoff one grows ahead.
    #[test]
    fn the_sim_says_which_world_stands_where() {
        let sim = Sim::new(7);
        let parked = slots(&sim);
        assert!(parked[0].1.is_some(), "docked hangs the berth alongside");
        assert!(parked[1].1.is_none() && parked[2].1.is_none());

        let mut sim = launched(7);
        let cast_off = slots(&sim);
        assert!(cast_off[2].1.is_some(), "the origin shrinks astern");
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            unreachable!("launched() travels");
        };
        sim.fast_forward(leg_ticks / 2);
        let midway = slots(&sim);
        assert!(midway[1].1.is_some(), "midway the destination grows ahead");
        assert!(midway[2].1.is_none(), "and the origin is gone");
    }

    /// The destination cubes in and the origin squares away — the painted
    /// sky's growth curves, kept, because they carried the leg's shape.
    #[test]
    fn the_destination_arrives_all_at_once() {
        let (_, early) = world_pose(Slot::Ahead, 0.5);
        let (_, late) = world_pose(Slot::Ahead, 1.0);
        assert!(
            early < late * 0.2,
            "half a leg past the handoff must still read empty: {early} vs {late}"
        );
        let (_, cast) = world_pose(Slot::Astern, 0.0);
        let (_, gone) = world_pose(Slot::Astern, 1.0);
        assert!(
            cast > 40.0 && gone < 0.01,
            "the origin must leave: {cast} -> {gone}"
        );
    }

    /// A paused world looks paused: the clocks latch cues and hold.
    #[test]
    fn a_paused_world_holds_its_sky_still() {
        let mut sim = launched(7);
        let mut clock = SkyClock::default();
        clock.update(0.5, &sim);
        let running = (clock.t, clock.run, clock.speed);
        assert!(
            running.0 > 0.0 && running.2 > 0.0,
            "under way, the sky moves"
        );
        sim.advance(
            0.0,
            &InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        assert!(sim.is_paused(), "the toggle should pause the sim");
        clock.update(0.5, &sim);
        assert_eq!(
            (clock.t, clock.run, clock.speed),
            running,
            "a paused world must hold every phase"
        );
    }

    /// The jump floods and fades squared, inside the half-second law.
    #[test]
    fn the_jump_floods_and_fades() {
        let mut clock = SkyClock {
            jump: JUMP_LEN,
            ..SkyClock::default()
        };
        assert!((clock.heat() - 1.0).abs() < 1e-6);
        let sim = Sim::new(7);
        clock.update(JUMP_LEN * 0.5, &sim);
        assert!(
            (0.4..0.6).contains(&clock.heat()),
            "half spent: {}",
            clock.heat()
        );
        clock.update(JUMP_LEN, &sim);
        assert!(clock.heat() <= 0.0, "the flood must end inside the law");
    }

    /// The deep field is the same field every boot, and it is a field:
    /// hashed, even over the sphere, two size classes.
    #[test]
    fn the_deep_field_is_deterministic_and_whole() {
        let a = star_mesh(0);
        let b = star_mesh(0);
        let count = a.count_vertices();
        assert_eq!(count, STAR_PER_BUCKET as usize * 4, "four corners a star");
        assert_eq!(count, b.count_vertices(), "the same sky every boot");
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pa)) =
            a.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("the field carries positions");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pb)) =
            b.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("the field carries positions");
        };
        assert_eq!(pa, pb, "the deep field must be the same every boot");
        // Every star sits on the shell, within its own half-size.
        for corner in pa {
            let r = Vec3::from(*corner).length();
            assert!(
                (STAR_BIG.mul_add(-2.0, STAR_SHELL)..=STAR_BIG.mul_add(2.0, STAR_SHELL))
                    .contains(&r),
                "a star left the shell at {r}"
            );
        }
        // And the buckets are different skies, not one sky six times.
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(other)) =
            star_mesh(1).attribute(Mesh::ATTRIBUTE_POSITION).cloned()
        else {
            panic!("the field carries positions");
        };
        assert_ne!(*pa, other, "each bucket must be its own scatter");
    }

    /// The target is cut to the aperture, quantized, and never allowed to
    /// become a second render budget.
    #[test]
    fn the_porthole_target_stays_inside_its_bounds() {
        assert_eq!(pane_texels(0.0), PANE_MIN);
        assert_eq!(pane_texels(1e9), PANE_MAX);
        assert_eq!(pane_texels(100.0), 104, "quantized up to the step");
        assert!(pane_texels(163.0).is_multiple_of(8));
    }

    // ------------------------------------------------- one wall, one sky --

    /// A pane of the given size, centred at `centre`, hung on a wall
    /// facing `out` — with a real material handle, because the plan
    /// hands one back per pane and the tests count them.
    fn hung(
        materials: &mut Assets<StandardMaterial>,
        centre: Vec3,
        out: Vec3,
        (w, h): (f32, f32),
    ) -> Pane {
        let facing = -out.normalize();
        let v = Vec3::Y;
        let u = v.cross(facing);
        Pane {
            glass: materials.add(StandardMaterial::default()),
            centre,
            half_u: u * (w * 0.5),
            half_v: v * (h * 0.5),
            facing,
        }
    }

    /// A row of `n` panes down one wall, evenly spaced.
    fn row(materials: &mut Assets<StandardMaterial>, n: usize) -> Vec<(u64, Pane)> {
        (0..n)
            .map(|i| {
                let x = (i as f32).mul_add(0.42, -1.4);
                (
                    i as u64,
                    hung(
                        materials,
                        Vec3::new(x, 1.4, -1.9),
                        Vec3::NEG_Z,
                        (0.32, 0.30),
                    ),
                )
            })
            .collect()
    }

    /// Where a world point lands in a plan's own image, as texture uv.
    fn sky_uv(plan: &SkyPlan, eye: Vec3, at: Vec3) -> Vec2 {
        let n = ndc(plan.aperture, plan.rot, eye, at);
        Vec2::new(n.x.mul_add(0.5, 0.5), 0.5f32.mul_add(-n.y, 0.5))
    }

    /// **The claim, as an assertion.** N panes on one wall cost ONE sky,
    /// however many N is — and the control arm (one sky per pane, which
    /// is what the first exterior pass did) costs N, which is the thing
    /// being replaced. Measured cost lives in `--gauge`; what is checked
    /// here is the shape of it, which is what the measurement is a
    /// measurement OF.
    #[test]
    fn a_wall_of_windows_costs_one_sky() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.0, 1.5, 0.4);
        for n in [1_usize, 2, 4, 8] {
            let (plans, dark) = plan_skies(row(&mut materials, n), eye, Grouping::Wall);
            assert_eq!(plans.len(), 1, "{n} panes on one wall must share one sky");
            assert_eq!(plans[0].panes.len(), n, "and every one of them reads it");
            assert!(dark.is_empty(), "none of them goes dark");
            let (control, _) = plan_skies(row(&mut materials, n), eye, Grouping::Pane);
            assert_eq!(control.len(), n, "the control arm pays per pane");
        }
    }

    /// And the bound holds whatever the crew does: past [`MAX_SKIES`]
    /// distinct walls the remaining panes go DARK rather than stale.
    /// Black glass is a truthful answer; last frame's sky is not.
    #[test]
    fn the_sky_pool_is_a_hard_ceiling() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.0, 1.5, 0.4);
        // Twenty panes, every one on its own plane (stepped away from
        // the eye along the same normal, so no two share an offset).
        let panes: Vec<(u64, Pane)> = (0..20_usize)
            .map(|i| {
                let z = (i as f32).mul_add(-0.31, -1.9);
                (
                    i as u64,
                    hung(
                        &mut materials,
                        Vec3::new(0.0, 1.4, z),
                        Vec3::NEG_Z,
                        (0.30, 0.30),
                    ),
                )
            })
            .collect();
        let (plans, dark) = plan_skies(panes, eye, Grouping::Wall);
        assert_eq!(plans.len(), MAX_SKIES, "the pool is the pool");
        assert_eq!(dark.len(), 20 - MAX_SKIES, "the rest are honestly black");
    }

    /// **The correctness of the sharing**, which is the whole reason it
    /// is allowed to be this cheap: a pane reading its rectangle out of
    /// its wall's sky lands on exactly the texel its OWN aperture would
    /// have drawn there. Not close — the same affine map, checked
    /// against the independent projection, for panes of different sizes
    /// at different places on the wall, and for points all over the
    /// void including behind the eye's shoulder.
    #[test]
    fn a_shared_sky_is_the_pane_s_own_sky() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(-0.35, 1.52, 0.6);
        let panes = vec![
            (
                0,
                hung(
                    &mut materials,
                    Vec3::new(-1.1, 1.35, -1.9),
                    Vec3::NEG_Z,
                    (0.6, 0.3),
                ),
            ),
            (
                1,
                hung(
                    &mut materials,
                    Vec3::new(0.7, 1.6, -1.9),
                    Vec3::NEG_Z,
                    (0.3, 0.75),
                ),
            ),
            (
                2,
                hung(
                    &mut materials,
                    Vec3::new(1.4, 1.1, -1.9),
                    Vec3::NEG_Z,
                    (0.25, 0.25),
                ),
            ),
        ];
        let alone: Vec<(Aperture, Quat)> = panes
            .iter()
            .map(|(_, pane)| {
                Aperture::through(eye, pane.centre, pane.half_u, pane.half_v)
                    .expect("each pane is a hole on its own")
            })
            .collect();
        let (plans, dark) = plan_skies(panes, eye, Grouping::Wall);
        assert!(dark.is_empty() && plans.len() == 1, "one wall, one sky");
        let plan = &plans[0];
        for (i, (_, uv)) in plan.panes.iter().enumerate() {
            let (aperture, rot) = alone[i];
            for at in [
                Vec3::new(0.0, 1.4, -60.0),
                Vec3::new(-40.0, 20.0, -90.0),
                Vec3::new(55.0, -18.0, -140.0),
                Vec3::new(-4.0, 1.0, -2.4),
                Vec3::new(300.0, 300.0, -900.0),
            ] {
                let n = ndc(aperture, rot, eye, at);
                let own = Vec2::new(n.x.mul_add(0.5, 0.5), 0.5f32.mul_add(-n.y, 0.5));
                let shared = sky_uv(plan, eye, at);
                let remapped = uv.transform_point2(own);
                assert!(
                    (remapped - shared).length() < 1e-4,
                    "pane {i} reads {remapped} where its own sky is {shared} (point {at})"
                );
            }
        }
    }

    /// The whimsy rule survives the sharing: two panes on DIFFERENT
    /// walls get different skies, aimed different ways, because the
    /// gathering is by plane and a plane is a fact about the wall. Hang
    /// a window anywhere and it shows what is out THAT wall.
    #[test]
    fn two_walls_are_two_skies_pointed_two_ways() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.0, 1.5, 0.3);
        let panes = vec![
            (
                0,
                hung(
                    &mut materials,
                    Vec3::new(0.0, 1.4, -1.9),
                    Vec3::NEG_Z,
                    (0.6, 0.3),
                ),
            ),
            (
                1,
                hung(
                    &mut materials,
                    Vec3::new(-2.1, 1.4, 0.3),
                    Vec3::NEG_X,
                    (0.6, 0.3),
                ),
            ),
        ];
        let (plans, _) = plan_skies(panes, eye, Grouping::Wall);
        assert_eq!(plans.len(), 2, "two walls cannot share one hole");
        let looks: Vec<Vec3> = plans.iter().map(|plan| plan.rot * Vec3::NEG_Z).collect();
        assert!(looks[0].z < -0.9 || looks[1].z < -0.9, "one looks forward");
        assert!(looks[0].x < -0.9 || looks[1].x < -0.9, "one looks to port");
        assert!(
            looks[0].dot(looks[1]) < 0.2,
            "and they are not the same sky twice: {looks:?}"
        );
    }

    /// A wall's sky is cut to that wall's GLASS and stays inside the
    /// texel bounds — so a crew that hangs eight windows buys one target
    /// no bigger than the one window bought, and the crunch stays the
    /// crunch (`docs/ART_DIRECTION_3D.md`).
    #[test]
    fn the_sky_pool_never_becomes_a_second_render_budget() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.0, 1.5, 0.4);
        let mut widest = 0;
        for n in [1_usize, 8] {
            let (plans, _) = plan_skies(row(&mut materials, n), eye, Grouping::Wall);
            for plan in &plans {
                assert!(
                    plan.size.x >= PANE_MIN
                        && plan.size.x <= PANE_MAX
                        && plan.size.y >= PANE_MIN
                        && plan.size.y <= PANE_MAX,
                    "{n} panes cut a {} sky",
                    plan.size
                );
                widest = widest.max(plan.size.x);
            }
        }
        assert!(widest <= PANE_MAX, "the ceiling is the ceiling");
    }

    /// A sold window is not a hole, and neither is one hung with no
    /// extent at all: the plan hands it back dark and the pool stays
    /// unlit. Solid hull, no view — the property the whole feature is
    /// allowed to have taken away.
    #[test]
    fn a_sold_window_lights_nothing() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.0, 1.5, 0.4);
        let (plans, dark) = plan_skies(Vec::new(), eye, Grouping::Wall);
        assert!(plans.is_empty() && dark.is_empty(), "no glass, no sky");
        // And a pane the eye stands behind never reaches the plan at all
        // — `aim_skies` sets it aside — which the aperture itself says:
        let pane = hung(
            &mut materials,
            Vec3::new(0.0, 1.4, -1.9),
            Vec3::NEG_Z,
            (0.6, 0.3),
        );
        assert!(
            Aperture::through(
                Vec3::new(0.0, 1.4, -4.0),
                pane.centre,
                pane.half_u,
                pane.half_v
            )
            .is_none(),
            "there is no view from behind the glass"
        );
    }

    /// The whole pass, run headless, with the resource counts read off
    /// the world afterwards.
    ///
    /// **This is the stress test the architecture exists to pass.** It
    /// builds the real pool, hangs N panes on one wall in front of a
    /// real eye with a real frustum, runs [`aim_skies`] itself, and then
    /// counts what the frame actually costs: cameras, targets, and lit
    /// glass. Nothing here is a stand-in — the only thing missing is the
    /// GPU, and the GPU is not what scales badly.
    fn stress(n: usize, how: Grouping) -> (usize, usize, usize) {
        use bevy::ecs::system::RunSystemOnce as _;

        let mut world = bevy::ecs::world::World::new();
        let mut images = Assets::<Image>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let targets: [Handle<Image>; MAX_SKIES] =
            std::array::from_fn(|_| images.add(void_target(UVec2::splat(PANE_MIN))));
        let flood = materials.add(StandardMaterial::default());

        // The eye: mid-cabin, looking at the wall the glass is on.
        let eye = Vec3::new(0.0, 1.5, 0.9);
        let view = Mat4::look_to_rh(eye, Vec3::NEG_Z, Vec3::Y);
        let clip = Mat4::perspective_infinite_reverse_rh(1.0, 16.0 / 9.0, 0.1);
        world.spawn((
            GlobalTransform::from_translation(eye),
            Frustum(bevy::math::primitives::ViewFrustum::from_clip_from_world(
                &(clip * view),
            )),
            CabinCamera,
        ));

        // The pool, exactly as `spawn_void` lays it out.
        for slot in 0..MAX_SKIES {
            world.spawn((
                Camera {
                    is_active: false,
                    ..default()
                },
                Projection::custom(Aperture {
                    left: -1.0,
                    right: 1.0,
                    bottom: -1.0,
                    top: 1.0,
                    near: 0.1,
                    far: VOID_FAR,
                }),
                Transform::default(),
                Sky(slot),
            ));
            world.spawn((Transform::default(), Visibility::Hidden, Flood, Sky(slot)));
        }

        // The glass: a row of panes down the front wall, each with its
        // own material exactly as a piece rig gives it one.
        let glass: Vec<Handle<StandardMaterial>> = (0..n)
            .map(|i| {
                let x = (i as f32).mul_add(0.42, -1.4);
                let handle = materials.add(StandardMaterial::default());
                world.spawn((
                    GlobalTransform::from(
                        Transform::from_xyz(x, 1.4, -1.9).with_scale(Vec3::new(0.32, 0.30, 1.0)),
                    ),
                    MeshMaterial3d(handle.clone()),
                    SkyPane,
                ));
                handle
            })
            .collect();

        world.insert_resource(Skies {
            images: targets,
            sizes: [UVec2::splat(PANE_MIN); MAX_SKIES],
            flood,
            lit: 0,
        });
        world.insert_resource(how);
        world.insert_resource(SkyClock::default());
        world.insert_resource(images);
        world.insert_resource(materials);
        world.run_system_once(aim_skies).expect("the aim runs");

        let drawing = world
            .query::<(&Camera, &Sky)>()
            .iter(&world)
            .filter(|(camera, _)| camera.is_active)
            .count();
        let materials = world.resource::<Assets<StandardMaterial>>();
        let lit = glass
            .iter()
            .filter(|handle| {
                materials
                    .get(*handle)
                    .is_some_and(|mat| mat.emissive_texture.is_some())
            })
            .count();
        (drawing, world.resource::<Skies>().images.len(), lit)
    }

    /// **Layer isolation, checked on the actual spawn.** One number
    /// keeps the hull solid from inside and stops starlight leaking
    /// through a wall ([`VOID_LAYER`]), and the way it fails is not
    /// subtle reasoning — it is somebody adding a body out there and
    /// forgetting `outside()`. So the void is built for real and every
    /// last thing it spawned is asked which layer it is on.
    #[test]
    fn nothing_out_there_is_on_the_cabin_s_layer() {
        use bevy::ecs::system::RunSystemOnce as _;

        let mut world = bevy::ecs::world::World::new();
        world.insert_resource(Assets::<Mesh>::default());
        world.insert_resource(Assets::<StandardMaterial>::default());
        world.insert_resource(Assets::<Image>::default());
        world.run_system_once(spawn_void).expect("the void builds");

        let void = outside();
        let cabin = RenderLayers::default();
        assert!(
            !void.intersects(&cabin),
            "the void's layer must not be the cabin's"
        );
        // Everything the void spawns is POSED — a body, a lamp, a
        // camera. Bevy keeps its own bookkeeping in entities too, and
        // those are not the void's.
        let mut drawn = world.query_filtered::<(Entity, Option<&RenderLayers>), With<Transform>>();
        let mut counted = 0;
        for (entity, layers) in drawn.iter(&world) {
            // The flood quads and the bodies alike: everything the void
            // spawned wears the void's layer, or the cabin camera would
            // draw a star through a wall.
            let Some(layers) = layers else {
                panic!("{entity} left the void with no layer at all");
            };
            assert!(
                !layers.intersects(&cabin),
                "{entity} is visible to the cabin camera"
            );
            counted += 1;
        }
        assert!(counted > MAX_SKIES, "the void is more than its cameras");
        // The one deliberate exception, and it is dev tooling: the
        // drydock view lets the cabin camera see BOTH (`rig`).
        let drydock = RenderLayers::from_layers(&[0, VOID_LAYER]);
        assert!(drydock.intersects(&void) && drydock.intersects(&cabin));
    }

    /// N panes, one wall: the cameras and the targets do not move, and
    /// every pane is lit. The control arm pays a camera apiece, which is
    /// the bill this pass was written to stop paying.
    #[test]
    fn the_frame_costs_the_same_however_many_windows_hang() {
        for n in [1_usize, 2, 4, 8] {
            let (cameras, targets, lit) = stress(n, Grouping::Wall);
            assert_eq!(cameras, 1, "{n} panes on one wall lit {cameras} cameras");
            assert_eq!(targets, MAX_SKIES, "the pool is allocated once, at boot");
            assert_eq!(lit, n, "every pane must show the sky it shares");
            let (control, _, control_lit) = stress(n, Grouping::Pane);
            assert_eq!(control, n, "the control arm pays per pane");
            assert_eq!(control_lit, n, "and lights the same glass, dearer");
        }
    }

    /// The gathering is STABLE: the same glass in any query order plans
    /// the same skies in the same slots. A slot that wandered would swap
    /// two windows' views for one frame every time the crew walked past,
    /// which nobody would file and everybody would feel.
    #[test]
    fn the_same_glass_plans_the_same_skies() {
        let mut materials = Assets::<StandardMaterial>::default();
        let eye = Vec3::new(0.1, 1.5, 0.4);
        let panes = vec![
            (
                7,
                hung(
                    &mut materials,
                    Vec3::new(0.0, 1.4, -1.9),
                    Vec3::NEG_Z,
                    (0.6, 0.3),
                ),
            ),
            (
                2,
                hung(
                    &mut materials,
                    Vec3::new(-2.1, 1.4, 0.3),
                    Vec3::NEG_X,
                    (0.4, 0.4),
                ),
            ),
            (
                5,
                hung(
                    &mut materials,
                    Vec3::new(-2.1, 1.4, 1.1),
                    Vec3::NEG_X,
                    (0.4, 0.4),
                ),
            ),
        ];
        let mut shuffled = panes.clone();
        shuffled.reverse();
        let (a, _) = plan_skies(panes, eye, Grouping::Wall);
        let (b, _) = plan_skies(shuffled, eye, Grouping::Wall);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert!(
                (x.rot.to_array()[0] - y.rot.to_array()[0]).abs() < 1e-6
                    && (x.aperture.left - y.aperture.left).abs() < 1e-6
                    && x.size == y.size,
                "slot order must not depend on query order"
            );
        }
    }
}
