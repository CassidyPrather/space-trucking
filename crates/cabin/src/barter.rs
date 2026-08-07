//! The barter counter, made furniture: the 3D face of `layout::BARTER_PANEL`.
//!
//! The 2D console draws two mutually exclusive faces on this rect
//! (`draw_barter` vs `draw_wayside` in the root `src/render.rs`); here the
//! counter is one piece of bolted-down hardware whose *dressing* switches.
//! The wells, dial housing, lever, and lamps are always physically present
//! — metal does not despawn — while the trading dressing (wants chips,
//! shutter) and the wayside dressing (voyage strip, encounter plaque, ???
//! toll) toggle visibility on `sim.barter()`. Cargo pieces and drop-glow
//! affordances are deliberately absent: the pieces module owns those.
//!
//! Every element reads sim accessors only; nothing here re-derives a rule.
//! Feedback (refusal flash, badge wobble, accept ring, shutter slide) runs
//! well inside half a second; decoration breathes off `Res<Time>` elapsed.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

use space_trucking::sim::Vec2 as SimVec2;
use space_trucking::sim::{
    Cue, EAGER_MAX, EncounterKind, Kind, Loc, PATIENCE, ShipState, Sim, WANDERER, layout,
};

use crate::glow;
use crate::palette;
use crate::rig::Skin;
use crate::surface::{SimSurface, Station};
use crate::{Phase, Shell};

// ---- Feedback clocks (all inside the half-second law) ----

/// The refusal flash arc over the dial.
const REFUSE_LEN: f32 = 0.35;
/// The station badge's insulted wobble.
const WOBBLE_LEN: f32 = 0.3;
/// The accept celebration ring's expansion.
const ACCEPT_LEN: f32 = 0.45;
/// The shutter's slide, either direction.
const SHUTTER_LEN: f32 = 0.3;

// ---- Dial geometry, in sim units and sim screen angles ----
// Sim screen angles run y-down: the sweep starts down-left (135 deg),
// passes straight up the panel at break-even (270 deg), ends down-right.
// `dial_dir` converts to the panel's local y-up frame in one place.

const DIAL_START: f32 = 135.0 * PI / 180.0;
const DIAL_SWEEP: f32 = 270.0 * PI / 180.0;
/// Fill segments along the sweep, mirroring the 2D gauge's 20 arcs.
const SEGMENTS: usize = 20;

const HOUSING_R: f32 = 34.0;
const TRACK_R: f32 = 23.0;
const BADGE_R: f32 = 11.0;
const BADGE_RIM_R: f32 = 13.0;
const NEEDLE_MID_R: f32 = 18.5;
const NEEDLE_LEN: f32 = 21.0;
const HAZE_R: f32 = 18.0;
const NOTCH_MID_R: f32 = 25.5;
const NOTCH_LEN: f32 = 11.0;
const FLASH_R: f32 = 21.0;

// ---- Z layers above the panel plane, in meters ----

const Z_WELL: f32 = 0.0005;
const Z_LIP: f32 = 0.0025;
const Z_TRIM: f32 = 0.004;
const Z_HOUSING: f32 = 0.004;
const Z_GROOVE: f32 = 0.0085;
const Z_FILL: f32 = 0.0095;
const Z_NOTCH: f32 = 0.0105;
const Z_BADGE_RIM: f32 = 0.009;
const Z_BADGE: f32 = 0.0115;
const Z_HAZE: f32 = 0.0135;
const Z_NEEDLE: f32 = 0.0145;
const Z_FLASH: f32 = 0.016;
const Z_RING: f32 = 0.017;
const Z_LAMP: f32 = 0.0035;
const Z_DISC: f32 = 0.004;
const Z_DART: f32 = 0.008;
const Z_SHUTTER: f32 = 0.011;
const Z_TOLL: f32 = 0.01;
const Z_PLAQUE: f32 = 0.02;

/// How far gizmo dashes float off the panel face.
const GIZMO_LIFT: f32 = 0.006;

// ---- Wayside strip landmarks (sim units, from the 2D voyage strip) ----

const ROW_Y: f32 = 556.0;
const FROM_X: f32 = 310.0;
const TO_X: f32 = 620.0;
const LINE_X0: f32 = 332.0;
const LINE_X1: f32 = 598.0;

// ---- Emissive levels ----

const SEG_GLOW: f32 = 3.0;
const GAS_GLOW: f32 = 2.6;

pub struct BarterPlugin;

impl Plugin for BarterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BarterFx>()
            .add_systems(PostStartup, spawn)
            .add_systems(
                Update,
                (
                    latch,
                    (
                        switch_faces,
                        update_dial_fill,
                        update_needle,
                        update_badge,
                        update_patience,
                        update_flashes,
                        update_lever,
                        update_shutter,
                        slide_accept_handle,
                        update_wants,
                        update_voyage,
                        update_encounter,
                        update_toll,
                        spin_decor,
                        sway_decor,
                        streak_decor,
                    ),
                )
                    .chain()
                    .in_set(Phase::View),
            );
    }
}

// ---- Resources ----

/// The barter surface's mapping, captured at spawn: the panel quad plus
/// meters-per-sim-unit along each axis, so systems can place things.
#[derive(Resource, Clone, Copy)]
struct BarterFrame {
    surface: SimSurface,
    su: f32,
    sv: f32,
}

/// The accept lever's brass handle — slides with the pull gesture.
#[derive(Component)]
struct AcceptHandle;

impl BarterFrame {
    /// A sim-rect position as a translation local to the counter root
    /// (panel-right +x, up-panel +y — sim y runs down, so it flips here).
    fn local(&self, x: f32, y: f32, z: f32) -> Vec3 {
        let rect = self.surface.rect;
        let cx = rect.w.mul_add(0.5, rect.x);
        let cy = rect.h.mul_add(0.5, rect.y);
        Vec3::new((x - cx) * self.su, (cy - y) * self.sv, z)
    }
}

/// Latched feedback timers — cues are only valid the frame they fire, so
/// the view keeps its own short clocks.
#[derive(Resource, Default)]
struct BarterFx {
    /// Seconds left of the red refusal arc.
    refuse: f32,
    /// Seconds left of the badge wobble.
    wobble: f32,
    /// Seconds left of the accept ring.
    accept: f32,
    /// The concluded trade's generosity, scaling the ring's growth.
    accept_value: f32,
    /// Shutter deployment, `0..=1`, eased toward the sim's patience state.
    shutter: f32,
}

/// The one material every refusal-arc quad shares — they flash as one.
#[derive(Resource)]
struct RefuseMat(Handle<StandardMaterial>);

// ---- Components ----

/// Root of the trading dressing (wants chips, shutter).
#[derive(Component)]
struct TradingFace;

/// Root of the wayside dressing (voyage strip, plaque, toll).
#[derive(Component)]
struct WaysideFace;

/// One eagerness-fill wedge: lit while the eased value covers it.
#[derive(Component)]
struct DialSeg {
    /// `(i + 0.5) / SEGMENTS`, compared against `value / EAGER_MAX`.
    threshold: f32,
    /// Its slice of the red-amber-green ramp, from `palette::dial_color`.
    color: Color,
}

/// The dial needle, swung and dimmed per frame.
#[derive(Component)]
struct DialNeedle;

/// The fog haze wedge over the needle's uncertainty span.
#[derive(Component)]
struct FogHaze;

/// The enamel station badge at the dial hub.
#[derive(Component)]
struct StationBadge {
    /// Rest translation, local to the dial; the wobble offsets from here.
    home: Vec3,
}

/// One of the three patience lamps below the dial.
#[derive(Component)]
struct PatienceLamp(u8);

/// The accept lever's go-lamp.
#[derive(Component)]
struct GoLamp;

/// Group node holding the refusal flash arc quads.
#[derive(Component)]
struct RefuseArc;

/// The expanding accept celebration ring.
#[derive(Component)]
struct AcceptRing;

/// The corrugated shutter over the shelf row; scales down from its top
/// edge, so `scale.y` is the deployment.
#[derive(Component)]
struct Shutter;

/// One wants chip; the index picks `barter.wants[i]`.
#[derive(Component)]
struct WantChip(usize);

/// One amber pip under a wants chip.
#[derive(Component)]
struct WantPip {
    chip: usize,
    index: u8,
}

/// The voyage strip's moving parts.
#[derive(Component, PartialEq, Eq)]
enum VoyagePart {
    From,
    To,
    Dart,
}

/// The encounter plaque root (slab + border + emblems).
#[derive(Component)]
struct EncounterPlaque;

/// One emblem group, shown while its encounter kind is alongside.
#[derive(Component)]
struct Emblem(EncounterKind);

/// The gas pump's shared glow material lives on this entity; it dims to
/// 35% once the one top-up is spent.
#[derive(Component)]
struct GasBody;

/// Decoration: constant spin about the panel normal, radians per second.
#[derive(Component)]
struct Spin(f32);

/// Decoration: gentle rocking about the panel normal.
#[derive(Component)]
struct Sway {
    freq: f32,
    amp: f32,
}

/// Decoration: a meteor streak sliding along its own axis on a loop.
#[derive(Component)]
struct MeteorStreak {
    index: usize,
    home: Vec3,
}

/// Root of the ??? toll sockets.
#[derive(Component)]
struct TollGroup;

/// One violet toll diamond; the four bars share `mat` so it breathes whole.
#[derive(Component)]
struct TollRing {
    slot: u8,
    mat: Handle<StandardMaterial>,
}

/// The faint dot inside an unpaid toll socket.
#[derive(Component)]
struct TollDot(u8);

// ---- Small shared helpers ----

/// Visibility from a bool, `Inherited` so parent face-switching still wins.
const fn shown(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// A sim screen angle (y down) as a local panel offset (y up) at `radius`
/// sim units and `z` meters — the one place the flip happens for the dial.
fn dial_dir(su: f32, angle: f32, radius: f32, z: f32) -> Vec3 {
    Vec3::new(angle.cos() * radius * su, -angle.sin() * radius * su, z)
}

/// The dial reading in eagerness units, eased exactly as the 2D gauge:
/// last tick's value toward this tick's by the sub-tick alpha. Zero with
/// no barter open — the dormant gauge rests.
fn dial_value(sim: &Sim) -> f32 {
    sim.barter().map_or(0.0, |barter| {
        (barter.eagerness - barter.prev_eagerness).mul_add(sim.alpha(), barter.prev_eagerness)
    })
}

/// A flat one-sided triangle mesh in the local XY plane, facing +Z.
/// Points are in meters, wound counter-clockwise.
fn flat_tri(meshes: &mut Assets<Mesh>, pts: [Vec2; 3]) -> Handle<Mesh> {
    let positions: Vec<[f32; 3]> = pts.iter().map(|p| [p.x, p.y, 0.0]).collect();
    let normals = vec![[0.0, 0.0, 1.0]; 3];
    meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(vec![0, 1, 2])),
    )
}

/// The spawn-time carpenter: commands plus the panel frame and the shared
/// unit cube, so furniture helpers stay within argument budgets.
struct Builder<'w, 's, 'a> {
    commands: &'a mut Commands<'w, 's>,
    frame: BarterFrame,
    cube: Handle<Mesh>,
}

impl Builder<'_, '_, '_> {
    /// An empty grouping node at a local translation.
    fn node(&mut self, parent: Entity, at: Vec3) -> Entity {
        self.commands
            .spawn((
                Transform::from_translation(at),
                Visibility::Inherited,
                ChildOf(parent),
            ))
            .id()
    }

    /// A scaled unit cube — the low-poly workhorse.
    fn slab(
        &mut self,
        parent: Entity,
        mat: &Handle<StandardMaterial>,
        at: Vec3,
        size: Vec3,
        rot: Quat,
    ) -> Entity {
        self.commands
            .spawn((
                Mesh3d(self.cube.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(at)
                    .with_rotation(rot)
                    .with_scale(size),
                ChildOf(parent),
            ))
            .id()
    }

    /// An axis-aligned slab.
    fn flat(&mut self, parent: Entity, mat: &Handle<StandardMaterial>, at: Vec3, size: Vec3) {
        self.slab(parent, mat, at, size, Quat::IDENTITY);
    }

    /// Four thin bars rimming a sim rect (inflated by `margin`), at `z`.
    #[allow(clippy::suboptimal_flops)] // one-shot layout math; readability wins
    fn rim(
        &mut self,
        parent: Entity,
        mat: &Handle<StandardMaterial>,
        rect: layout::Rect,
        margin: f32,
        bar: f32,
        z: f32,
    ) {
        let cx = rect.x + rect.w * 0.5;
        let cy = rect.y + rect.h * 0.5;
        let (su, sv) = (self.frame.su, self.frame.sv);
        let w = (rect.w + 2.0 * margin + 2.0 * bar) * su;
        let h = (rect.h + 2.0 * margin) * sv;
        let dx = (rect.w * 0.5 + margin + bar * 0.5) * su;
        let dy = (rect.h * 0.5 + margin + bar * 0.5) * sv;
        let base = self.frame.local(cx, cy, z);
        let th = 0.003;
        for side in [-1.0, 1.0] {
            self.flat(
                parent,
                mat,
                base + Vec3::new(0.0, side * dy, 0.0),
                Vec3::new(w, bar * sv, th),
            );
            self.flat(
                parent,
                mat,
                base + Vec3::new(side * dx, 0.0, 0.0),
                Vec3::new(bar * su, h, th),
            );
        }
    }
}

// ---- Spawn ----

/// Build the whole counter under one root aligned to the barter surface.
fn spawn(
    mut commands: Commands,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(surface) = surfaces
        .iter()
        .find(|(station, _)| **station == Station::Barter)
        .map(|(_, surface)| *surface)
    else {
        return;
    };
    let frame = BarterFrame {
        surface,
        su: surface.scale_u(),
        sv: surface.scale_v(),
    };
    let root = commands
        .spawn((
            Transform::from_translation(surface.center).with_rotation(surface.orientation()),
            Visibility::Inherited,
        ))
        .id();

    let mut shop = Builder {
        commands: &mut commands,
        frame,
        cube: skin.cube.clone(),
    };

    spawn_wells(&mut shop, &skin, &mut materials, root);
    let refuse = spawn_dial(&mut shop, &skin, &mut meshes, &mut materials, root);
    spawn_lever(&mut shop, &skin, &mut meshes, &mut materials, root);

    let trading = shop
        .commands
        .spawn((
            TradingFace,
            Transform::IDENTITY,
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    spawn_wants(&mut shop, &mut meshes, &mut materials, trading);
    spawn_shutter(&mut shop, &skin, trading);

    let wayside = shop
        .commands
        .spawn((
            WaysideFace,
            Transform::IDENTITY,
            Visibility::Hidden,
            ChildOf(root),
        ))
        .id();
    spawn_voyage(&mut shop, &mut meshes, &mut materials, wayside);
    spawn_plaque(&mut shop, &skin, &mut meshes, &mut materials, wayside);
    spawn_toll(&mut shop, &mut meshes, &mut materials, wayside);

    commands.insert_resource(frame);
    commands.insert_resource(RefuseMat(refuse));
}

/// The sixteen cubbies: SOCKET wells with PLATE lips, each row rimmed in
/// its enamel trim. Always present — on the wayside face the shelf row's
/// trim reads as the outboard rail (`FLOTSAM_SLOTS` shares those rects).
fn spawn_wells(
    shop: &mut Builder,
    skin: &Skin,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
) {
    let rows: [(&[layout::Rect; 4], Color); 4] = [
        (&layout::SHELF_SLOTS, palette::TRIM_SHELF),
        (&layout::RECEIVED_SLOTS, palette::TRIM_RECEIVED),
        (&layout::GIVE_SLOTS, palette::TRIM_GIVE),
        (&layout::TAKE_SLOTS, palette::TRIM_TAKE),
    ];
    let (su, sv) = (shop.frame.su, shop.frame.sv);
    for (slots, trim) in rows {
        let trim_mat = glow::enamel(materials, trim);
        for slot in slots {
            let center = shop.frame.local(
                slot.w.mul_add(0.5, slot.x),
                slot.h.mul_add(0.5, slot.y),
                Z_WELL,
            );
            shop.flat(
                root,
                &skin.socket,
                center,
                Vec3::new(slot.w * su, slot.h * sv, 0.003),
            );
            shop.rim(root, &skin.plate.clone(), *slot, 0.0, 2.5, Z_LIP);
        }
        // The row's bounding rect carries the identity trim.
        let row = layout::Rect::new(
            slots[0].x,
            slots[0].y,
            slots[3].x + slots[3].w - slots[0].x,
            slots[0].h,
        );
        shop.rim(root, &trim_mat, row, 3.0, 2.0, Z_TRIM);
    }
}

/// The eagerness gauge: housing, groove, fill wedges, notch, needle, fog
/// haze, station badge, patience lamps, and both feedback flashes. One
/// instrument; it stays assembled past the line lint's comfort.
#[allow(clippy::too_many_lines)]
fn spawn_dial(
    shop: &mut Builder,
    skin: &Skin,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
) -> Handle<StandardMaterial> {
    let su = shop.frame.su;
    let hub = shop
        .frame
        .local(layout::DIAL_CENTER.x, layout::DIAL_CENTER.y, 0.0);
    let dial = shop.node(root, hub);
    let face_up = Quat::from_rotation_x(FRAC_PI_2);

    // Housing: a raised PLATE disc.
    let housing = meshes.add(Cylinder::new(HOUSING_R * su, 0.006));
    shop.commands.spawn((
        Mesh3d(housing),
        MeshMaterial3d(skin.plate.clone()),
        Transform::from_xyz(0.0, 0.0, Z_HOUSING).with_rotation(face_up),
        ChildOf(dial),
    ));

    // Track groove (dark) and the twenty fill wedges over it.
    let groove_mat = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    });
    let seg_sweep = DIAL_SWEEP / SEGMENTS as f32;
    for i in 0..SEGMENTS {
        let mid = ((i as f32) + 0.5).mul_add(seg_sweep, DIAL_START);
        let tangent = Quat::from_rotation_z(-mid - FRAC_PI_2);
        shop.slab(
            dial,
            &groove_mat,
            dial_dir(su, mid, TRACK_R, Z_GROOVE),
            Vec3::new(6.0 * su, 5.6 * su, 0.0008),
            tangent,
        );
        let threshold = ((i as f32) + 0.5) / SEGMENTS as f32;
        let color = palette::dial_color(threshold * EAGER_MAX);
        let fill = glow::phosphor(materials, color, 0.0);
        let id = shop.slab(
            dial,
            &fill,
            dial_dir(su, mid, TRACK_R, Z_FILL),
            Vec3::new(5.5 * su, 4.6 * su, 0.0008),
            tangent,
        );
        shop.commands
            .entity(id)
            .insert(DialSeg { threshold, color });
    }

    // Break-even notch: straight up the panel, a glint of honest metal.
    let notch_angle = DIAL_SWEEP.mul_add(0.5, DIAL_START);
    let notch = glow::phosphor(materials, palette::GLINT, 2.5);
    shop.slab(
        dial,
        &notch,
        dial_dir(su, notch_angle, NOTCH_MID_R, Z_NOTCH),
        Vec3::new(NOTCH_LEN * su, 1.8 * su, 0.001),
        Quat::from_rotation_z(-notch_angle),
    );

    // Needle and its fog haze; both repositioned per frame.
    let needle_mat = glow::phosphor(materials, palette::GLINT, 4.5);
    let id = shop.slab(
        dial,
        &needle_mat,
        dial_dir(su, DIAL_START, NEEDLE_MID_R, Z_NEEDLE),
        Vec3::new(NEEDLE_LEN * su, 1.6 * su, 0.0015),
        Quat::from_rotation_z(-DIAL_START),
    );
    shop.commands.entity(id).insert(DialNeedle);
    let haze_mat = glow::phosphor(materials, palette::GLASS, 8.0);
    let id = shop.slab(
        dial,
        &haze_mat,
        dial_dir(su, DIAL_START, HAZE_R, Z_HAZE),
        Vec3::new(0.001, 10.0 * su, 0.0008),
        Quat::IDENTITY,
    );
    shop.commands
        .entity(id)
        .insert((FogHaze, Visibility::Hidden));

    // Station badge: enamel on metal (never a screen), glint-rimmed.
    let rim = meshes.add(Cylinder::new(BADGE_RIM_R * su, 0.003));
    shop.commands.spawn((
        Mesh3d(rim),
        MeshMaterial3d(glow::enamel(materials, palette::GLINT)),
        Transform::from_xyz(0.0, 0.0, Z_BADGE_RIM).with_rotation(face_up),
        ChildOf(dial),
    ));
    let badge = meshes.add(Cylinder::new(BADGE_R * su, 0.003));
    let home = Vec3::new(0.0, 0.0, Z_BADGE);
    shop.commands.spawn((
        Mesh3d(badge),
        MeshMaterial3d(glow::enamel(materials, palette::GLASS)),
        Transform::from_translation(home).with_rotation(face_up),
        StationBadge { home },
        ChildOf(dial),
    ));

    // Patience lamps: three small glass eyes below the housing.
    let lamp = meshes.add(Cylinder::new(2.6 * su, 0.005));
    for i in 0..PATIENCE {
        let at = shop
            .frame
            .local(f32::from(i).mul_add(9.0, 691.0), 525.0, Z_LAMP);
        shop.commands.spawn((
            Mesh3d(lamp.clone()),
            MeshMaterial3d(glow::phosphor(materials, palette::AMBER, 0.0)),
            Transform::from_translation(at).with_rotation(face_up),
            PatienceLamp(i),
            ChildOf(root),
        ));
    }

    // Refusal flash: an arc of quads over the gauge, one shared material.
    let refuse_mat = glow::phosphor(materials, palette::LAMP_NO, 0.0);
    let arc = shop.node(dial, Vec3::ZERO);
    shop.commands
        .entity(arc)
        .insert((RefuseArc, Visibility::Hidden));
    for i in 0..SEGMENTS {
        let mid = ((i as f32) + 0.5).mul_add(seg_sweep, DIAL_START);
        shop.slab(
            arc,
            &refuse_mat,
            dial_dir(su, mid, FLASH_R, Z_FLASH),
            Vec3::new(5.0 * su, 9.0 * su, 0.0008),
            Quat::from_rotation_z(-mid - FRAC_PI_2),
        );
    }

    // Accept celebration: a flat ring scaled outward per frame.
    let ring = meshes.add(Torus::new(0.996, 1.004));
    shop.commands.spawn((
        Mesh3d(ring),
        MeshMaterial3d(glow::phosphor(materials, palette::LAMP_OK, 0.0)),
        Transform::from_xyz(0.0, 0.0, Z_RING)
            .with_rotation(face_up)
            .with_scale(Vec3::new(0.001, 1.0, 0.001)),
        AcceptRing,
        Visibility::Hidden,
        ChildOf(dial),
    ));

    refuse_mat
}

/// The accept lever: SOCKET track, static BRASS handle, and the go-lamp
/// that answers only for trades made of known quantities.
fn spawn_lever(
    shop: &mut Builder,
    skin: &Skin,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
) {
    let rect = layout::ACCEPT_LEVER;
    let (su, sv) = (shop.frame.su, shop.frame.sv);
    let mid_y = rect.h.mul_add(0.5, rect.y);
    // Track: the inset groove the handle rides.
    shop.flat(
        root,
        &skin.socket.clone(),
        shop.frame.local(rect.w.mul_add(0.5, rect.x), mid_y, 0.002),
        Vec3::new((rect.w - 24.0) * su, 4.0 * sv, 0.004),
    );
    // Handle: brass, chunky — rides the gesture layer's pull and springs
    // home when a timid pull lets go. The sim owns whether pulls land.
    let handle = shop.slab(
        root,
        &skin.brass.clone(),
        shop.frame.local(rect.x + 27.0, mid_y, 0.01),
        Vec3::new(14.0 * su, (rect.h - 12.0) * sv, 0.02),
        Quat::IDENTITY,
    );
    shop.commands.entity(handle).insert(AcceptHandle);
    // Go-lamp above the handle.
    let lamp = meshes.add(Cylinder::new(3.0 * su, 0.004));
    shop.commands.spawn((
        Mesh3d(lamp),
        MeshMaterial3d(glow::phosphor(materials, palette::LAMP_OK, 0.0)),
        Transform::from_translation(shop.frame.local(rect.x + 27.0, rect.y + 11.0, 0.022))
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        GoLamp,
        ChildOf(root),
    ));
}

/// The wants row: one enamel chip per wanted kind with amber pips below.
/// A colored chip is a deliberate simplification of the 2D's full cargo
/// glyphs — identity rides on the kind hue plus the pip count.
fn spawn_wants(
    shop: &mut Builder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    trading: Entity,
) {
    let su = shop.frame.su;
    let chip = meshes.add(Cylinder::new(9.0 * su, 0.004));
    let pip = meshes.add(Cylinder::new(2.0 * su, 0.003));
    let face_up = Quat::from_rotation_x(FRAC_PI_2);
    for i in 0..3_usize {
        let x = (i as f32).mul_add(56.0, 303.0);
        shop.commands.spawn((
            Mesh3d(chip.clone()),
            MeshMaterial3d(glow::enamel(materials, palette::ICON)),
            Transform::from_translation(shop.frame.local(x, 506.0, 0.005)).with_rotation(face_up),
            WantChip(i),
            ChildOf(trading),
        ));
        for p in 0..3_u8 {
            let px = f32::from(p).mul_add(7.0, x - 7.0);
            shop.commands.spawn((
                Mesh3d(pip.clone()),
                MeshMaterial3d(glow::phosphor(materials, palette::AMBER, 0.0)),
                Transform::from_translation(shop.frame.local(px, 528.0, 0.003))
                    .with_rotation(face_up),
                WantPip { chip: i, index: p },
                ChildOf(trading),
            ));
        }
    }
}

/// The out-of-patience shutter: a corrugated `PLATE_SHADE` panel anchored at
/// the shelf row's top edge, unrolled by scaling `y` — gifts still move on
/// the give pads while it is down.
fn spawn_shutter(shop: &mut Builder, skin: &Skin, trading: Entity) {
    let (su, sv) = (shop.frame.su, shop.frame.sv);
    let row = &layout::SHELF_SLOTS;
    let x0 = row[0].x - 2.0;
    let x1 = row[3].x + row[3].w + 2.0;
    let cx = f32::midpoint(x0, x1);
    let top = row[0].y - 2.0;
    let drop = row[0].h + 4.0;
    let anchor = shop.frame.local(cx, top, 0.0);
    let shutter = shop
        .commands
        .spawn((
            Shutter,
            Transform::from_translation(anchor).with_scale(Vec3::new(1.0, 0.001, 1.0)),
            Visibility::Hidden,
            ChildOf(trading),
        ))
        .id();
    // The slat panel hangs below the anchor so scaling reads as sliding.
    shop.flat(
        shutter,
        &skin.plate_shade.clone(),
        Vec3::new(0.0, drop * -0.5 * sv, Z_SHUTTER),
        Vec3::new((x1 - x0) * su, drop * sv, 0.004),
    );
    for k in 0..5_usize {
        let y = (k as f32).mul_add(-8.0, -6.0);
        shop.flat(
            shutter,
            &skin.plate_lit.clone(),
            Vec3::new(0.0, y * sv, Z_SHUTTER + 0.0025),
            Vec3::new((x1 - x0 - 6.0) * su, 1.5 * sv, 0.0015),
        );
    }
}

/// The voyage strip's hardware: two enamel POI discs and the phosphor
/// dart that rides the dashed course (the dashes are gizmos, drawn live).
fn spawn_voyage(
    shop: &mut Builder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wayside: Entity,
) {
    let su = shop.frame.su;
    let disc = meshes.add(Cylinder::new(14.0 * su, 0.005));
    let face_up = Quat::from_rotation_x(FRAC_PI_2);
    for (part, x) in [(VoyagePart::From, FROM_X), (VoyagePart::To, TO_X)] {
        shop.commands.spawn((
            Mesh3d(disc.clone()),
            MeshMaterial3d(glow::enamel(materials, palette::GLASS)),
            Transform::from_translation(shop.frame.local(x, ROW_Y, Z_DISC)).with_rotation(face_up),
            part,
            Visibility::Hidden,
            ChildOf(wayside),
        ));
    }
    let dart = flat_tri(
        meshes,
        [
            Vec2::new(6.0 * su, 0.0),
            Vec2::new(-4.0 * su, 4.5 * su),
            Vec2::new(-4.0 * su, -4.5 * su),
        ],
    );
    shop.commands.spawn((
        Mesh3d(dart),
        MeshMaterial3d(glow::phosphor(materials, palette::PHOSPHOR_HOT, 4.0)),
        Transform::from_translation(shop.frame.local(LINE_X0, ROW_Y, Z_DART)),
        VoyagePart::Dart,
        Visibility::Hidden,
        ChildOf(wayside),
    ));
}

/// The encounter plaque: a small PLATE tablet hung over the dormant dial
/// corner, carrying one emblem per encounter kind. Simple builds, per the
/// spec — silhouettes, not portraits.
#[allow(clippy::too_many_lines)]
fn spawn_plaque(
    shop: &mut Builder,
    skin: &Skin,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wayside: Entity,
) {
    let (su, sv) = (shop.frame.su, shop.frame.sv);
    let rect = layout::ENCOUNTER_BADGE;
    let at = shop.frame.local(
        rect.w.mul_add(0.5, rect.x),
        rect.h.mul_add(0.5, rect.y),
        Z_PLAQUE,
    );
    let plaque = shop.node(wayside, at);
    shop.commands
        .entity(plaque)
        .insert((EncounterPlaque, Visibility::Hidden));
    shop.flat(
        plaque,
        &skin.plate.clone(),
        Vec3::ZERO,
        Vec3::new(rect.w * su, rect.h * sv, 0.004),
    );
    let border = glow::phosphor(materials, palette::PHOSPHOR_DIM, 2.0);
    for side in [-1.0, 1.0] {
        shop.flat(
            plaque,
            &border,
            Vec3::new(0.0, side * 35.0 * sv, 0.003),
            Vec3::new(72.0 * su, 2.0 * sv, 0.001),
        );
        shop.flat(
            plaque,
            &border,
            Vec3::new(side * 35.0 * su, 0.0, 0.003),
            Vec3::new(2.0 * su, 68.0 * sv, 0.001),
        );
    }
    let z = 0.004;

    // Derelict: a listing hull, one porthole dark.
    let derelict = shop.node(plaque, Vec3::new(0.0, 0.0, z));
    shop.commands.entity(derelict).insert((
        Emblem(EncounterKind::Derelict),
        Sway {
            freq: 0.8,
            amp: 0.1,
        },
        Visibility::Hidden,
    ));
    let hull = flat_tri(
        meshes,
        [
            Vec2::new(-16.0 * su, -8.0 * su),
            Vec2::new(18.0 * su, -4.0 * su),
            Vec2::new(-10.0 * su, 8.0 * su),
        ],
    );
    let hull_mat = glow::phosphor(materials, palette::PHOSPHOR_DIM, 2.4);
    shop.commands.spawn((
        Mesh3d(hull),
        MeshMaterial3d(hull_mat),
        Transform::IDENTITY,
        ChildOf(derelict),
    ));
    let porthole = meshes.add(Cylinder::new(2.2 * su, 0.002));
    shop.commands.spawn((
        Mesh3d(porthole),
        MeshMaterial3d(skin.screen.clone()),
        Transform::from_xyz(-4.0 * su, 0.0, 0.001).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
        ChildOf(derelict),
    ));

    // Gas station: pump body, dark window, nozzle arm. Dims once used.
    let gas = shop.node(plaque, Vec3::new(0.0, 0.0, z));
    shop.commands
        .entity(gas)
        .insert((Emblem(EncounterKind::GasStation), Visibility::Hidden));
    let gas_mat = glow::phosphor(materials, palette::PHOSPHOR, GAS_GLOW);
    let body = shop.slab(
        gas,
        &gas_mat,
        Vec3::new(-3.0 * su, 0.0, 0.0),
        Vec3::new(10.0 * su, 20.0 * sv, 0.001),
        Quat::IDENTITY,
    );
    shop.commands.entity(body).insert(GasBody);
    shop.flat(
        gas,
        &skin.screen.clone(),
        Vec3::new(-3.0 * su, 4.5 * sv, 0.0012),
        Vec3::new(6.0 * su, 5.0 * sv, 0.0008),
    );
    shop.flat(
        gas,
        &gas_mat,
        Vec3::new(9.0 * su, 4.0 * sv, 0.0),
        Vec3::new(1.8 * su, 8.0 * sv, 0.001),
    );
    shop.flat(
        gas,
        &gas_mat,
        Vec3::new(5.5 * su, 8.0 * sv, 0.0),
        Vec3::new(9.0 * su, 1.8 * sv, 0.001),
    );

    // Casino: two counter-rotating rings of bars, amber over eerie.
    let casino = shop.node(plaque, Vec3::new(0.0, 0.0, z));
    shop.commands
        .entity(casino)
        .insert((Emblem(EncounterKind::Casino), Visibility::Hidden));
    let rings = [
        (15.0, 10.4, palette::AMBER, 0.9, 0.001),
        (10.0, 6.9, palette::EERIE_BRIGHT, -1.3, 0.002),
    ];
    for (radius, bar_len, color, rate, dz) in rings {
        let spinner = shop.node(casino, Vec3::new(0.0, 0.0, dz));
        shop.commands.entity(spinner).insert(Spin(rate));
        let mat = glow::phosphor(materials, color, 3.0);
        for i in 0..7_usize {
            let angle = (i as f32) * TAU / 7.0;
            shop.slab(
                spinner,
                &mat,
                Vec3::new(angle.cos() * radius * su, angle.sin() * radius * su, 0.0),
                Vec3::new(bar_len * su, 1.6 * su, 0.0008),
                Quat::from_rotation_z(angle + FRAC_PI_2),
            );
        }
    }

    // Meteor shower: three amber streaks on a sliding loop.
    let meteor = shop.node(plaque, Vec3::new(0.0, 0.0, z));
    shop.commands
        .entity(meteor)
        .insert((Emblem(EncounterKind::MeteorShower), Visibility::Hidden));
    let streak_mat = glow::phosphor(materials, palette::AMBER, 2.8);
    for (index, off) in [-8.0_f32, 0.0, 9.0].into_iter().enumerate() {
        let home = Vec3::new(off * su, 6.0 * sv, 0.001);
        let id = shop.slab(
            meteor,
            &streak_mat,
            home,
            Vec3::new(11.0 * su, 1.5 * su, 0.0008),
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4),
        );
        shop.commands
            .entity(id)
            .insert(MeteorStreak { index, home });
    }

    // Whale: two flukes, mid-dive, swaying.
    let whale = shop.node(plaque, Vec3::new(0.0, 0.0, z));
    shop.commands.entity(whale).insert((
        Emblem(EncounterKind::Whale),
        Sway {
            freq: 0.9,
            amp: 0.12,
        },
        Visibility::Hidden,
    ));
    let fluke_mat = glow::phosphor(materials, palette::PHOSPHOR, 2.4);
    let flukes = [
        [
            Vec2::new(0.0, -8.0 * su),
            Vec2::new(-2.0 * su, 2.0 * su),
            Vec2::new(-13.0 * su, 8.0 * su),
        ],
        [
            Vec2::new(0.0, -8.0 * su),
            Vec2::new(13.0 * su, 8.0 * su),
            Vec2::new(2.0 * su, 2.0 * su),
        ],
    ];
    for pts in flukes {
        let mesh = flat_tri(meshes, pts);
        shop.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(fluke_mat.clone()),
            Transform::IDENTITY,
            ChildOf(whale),
        ));
    }
}

/// ???'s toll sockets: a violet diamond over each of the first three rail
/// wells, breathing once a mysterious crate fills it.
fn spawn_toll(
    shop: &mut Builder,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wayside: Entity,
) {
    let su = shop.frame.su;
    let group = shop.node(wayside, Vec3::ZERO);
    shop.commands
        .entity(group)
        .insert((TollGroup, Visibility::Hidden));
    let dot_mesh = meshes.add(Cylinder::new(2.0 * su, 0.003));
    for (i, slot) in layout::FLOTSAM_SLOTS.iter().take(3).enumerate() {
        let slot_u8 = u8::try_from(i).unwrap_or(0);
        let center = shop.frame.local(
            slot.w.mul_add(0.5, slot.x),
            slot.h.mul_add(0.5, slot.y),
            Z_TOLL,
        );
        let mat = glow::phosphor(materials, palette::EERIE_BRIGHT, 1.0);
        let ring = shop.node(group, center);
        shop.commands.entity(ring).insert(TollRing {
            slot: slot_u8,
            mat: mat.clone(),
        });
        let radius = slot.w * 0.42;
        let half = radius * 0.5 * su;
        for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            // Bars at +-45 degrees close the diamond.
            let angle = if sx * sy > 0.0 {
                -std::f32::consts::FRAC_PI_4
            } else {
                std::f32::consts::FRAC_PI_4
            };
            shop.slab(
                ring,
                &mat,
                Vec3::new(sx * half, sy * half, 0.0),
                Vec3::new(radius * 1.48 * su, 1.7 * su, 0.0008),
                Quat::from_rotation_z(angle),
            );
        }
        shop.commands.spawn((
            Mesh3d(dot_mesh.clone()),
            MeshMaterial3d(glow::phosphor(materials, palette::EERIE, 0.0)),
            Transform::from_translation(center).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            TollDot(slot_u8),
            ChildOf(group),
        ));
    }
}

// ---- Per-frame systems (Phase::View) ----

/// Latch this frame's cues into short view-side clocks and run them down;
/// also ease the shutter toward the sim's patience state.
fn latch(time: Res<Time>, shell: Res<Shell>, mut fx: ResMut<BarterFx>) {
    let dt = time.delta_secs();
    fx.refuse = (fx.refuse - dt).max(0.0);
    fx.wobble = (fx.wobble - dt).max(0.0);
    fx.accept = (fx.accept - dt).max(0.0);
    for cue in shell.bridge.sim.cues() {
        match cue {
            Cue::Refuse => {
                fx.refuse = REFUSE_LEN;
                fx.wobble = WOBBLE_LEN;
            }
            Cue::Accept { value } => {
                fx.accept = ACCEPT_LEN;
                fx.accept_value = *value;
            }
            _ => {}
        }
    }
    // The broker's hand hovers over the shutter cord: the corrugated
    // panel creeps a little with every spent pull, then drops fully at
    // zero. State-reading presentation — patience itself is the sim's.
    let target = match shell.bridge.sim.barter().map(|barter| barter.patience) {
        Some(0) => 1.0,
        Some(1) => 0.16,
        Some(2) => 0.05,
        _ => 0.0,
    };
    let step = dt / SHUTTER_LEN;
    fx.shutter = if fx.shutter < target {
        (fx.shutter + step).min(target)
    } else {
        (fx.shutter - step).max(target)
    };
}

/// One panel, two faces, never both: trading furniture exists only while
/// a counterparty does — same contract as the 2D `draw_barter`.
fn switch_faces(
    shell: Res<Shell>,
    mut trading: Query<&mut Visibility, (With<TradingFace>, Without<WaysideFace>)>,
    mut wayside: Query<&mut Visibility, With<WaysideFace>>,
) {
    let open = shell.bridge.sim.barter().is_some();
    for mut vis in &mut trading {
        vis.set_if_neq(shown(open));
    }
    for mut vis in &mut wayside {
        vis.set_if_neq(shown(!open));
    }
}

/// Light the fill wedges up to the eased eagerness; each keeps its own
/// slice of the red-amber-green ramp.
fn update_dial_fill(
    shell: Res<Shell>,
    segs: Query<(&DialSeg, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let frac = (dial_value(&shell.bridge.sim) / EAGER_MAX).clamp(0.0, 1.0);
    for (seg, handle) in &segs {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.emissive = if seg.threshold <= frac {
                seg.color.to_linear() * SEG_GLOW
            } else {
                LinearRgba::NONE
            };
        }
    }
}

/// Swing the needle to the eased reading. Unfamiliar goods fog it: the
/// needle dims and wanders inside the guesswork band while a glassy haze
/// wedge covers the span — what the station would say stays hidden.
#[allow(clippy::type_complexity)] // bevy query filters are what they are
fn update_needle(
    time: Res<Time>,
    shell: Res<Shell>,
    frame: Option<Res<BarterFrame>>,
    mut needles: Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        (With<DialNeedle>, Without<FogHaze>),
    >,
    mut hazes: Query<(&mut Transform, &mut Visibility), With<FogHaze>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(frame) = frame else { return };
    let sim = &shell.bridge.sim;
    let frac = (dial_value(sim) / EAGER_MAX).clamp(0.0, 1.0);
    let fog = sim.barter().map_or(0.0, |barter| barter.fog);
    let aim = DIAL_SWEEP.mul_add(frac, DIAL_START);
    let half = fog * 0.55;
    let wander = if fog > 0.0 {
        (time.elapsed_secs() * 9.0).sin() * half * 0.6
    } else {
        0.0
    };
    // Near break-even on a clear reading, the needle carries a faint
    // anticipatory tremor — zero-mean and far under a segment's width,
    // so the reading stays the sim's verbatim; it just looks like the
    // instrument is holding its breath.
    let poise = ((0.12 - (dial_value(sim) - 1.0).abs()) / 0.12).clamp(0.0, 1.0);
    let tremor = if fog <= 0.0 && sim.barter().is_some() {
        (time.elapsed_secs() * 31.0).sin() * 0.009 * poise
    } else {
        0.0
    };
    let angle = aim + wander + tremor;
    for (mut transform, handle) in &mut needles {
        transform.translation = dial_dir(frame.su, angle, NEEDLE_MID_R, Z_NEEDLE);
        transform.rotation = Quat::from_rotation_z(-angle);
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let level = if sim.barter().is_none() {
                1.2
            } else if fog > 0.0 {
                2.2
            } else {
                4.5
            };
            material.emissive = palette::GLINT.to_linear() * level;
        }
    }
    let hazy = fog > 0.0 && sim.barter().is_some();
    for (mut transform, mut vis) in &mut hazes {
        vis.set_if_neq(shown(hazy));
        if hazy {
            transform.translation = dial_dir(frame.su, aim, HAZE_R, Z_HAZE);
            transform.rotation = Quat::from_rotation_z(-aim - FRAC_PI_2);
            transform.scale = Vec3::new(2.0 * half * HAZE_R * frame.su, 10.0 * frame.su, 0.0008);
        }
    }
}

/// The station badge wears its counterparty's enamel while trading and
/// dark glass otherwise; a refusal rattles it for a moment — feedback
/// carried by motion, not hue.
fn update_badge(
    time: Res<Time>,
    shell: Res<Shell>,
    fx: Res<BarterFx>,
    frame: Option<Res<BarterFrame>>,
    mut badges: Query<(
        &mut Transform,
        &StationBadge,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(frame) = frame else { return };
    let barter = shell.bridge.sim.barter();
    let color = barter.map_or(palette::GLASS, |barter| palette::poi_color(barter.station));
    // A viable trade warms the broker's sigil and leans it a hair off
    // the face — the counterparty perking up, in enamel.
    let ready = barter.is_some_and(|barter| barter.ready);
    let shake = (fx.wobble / WOBBLE_LEN).clamp(0.0, 1.0);
    let t = time.elapsed_secs();
    for (mut transform, badge, handle) in &mut badges {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color = color;
            material.emissive = if ready {
                color.to_linear() * glow::breathe(t, 2.8, 0.0).mul_add(0.35, 0.85)
            } else {
                LinearRgba::BLACK
            };
        }
        let lean = if ready {
            Vec3::Z * (1.1 * frame.su)
        } else {
            Vec3::ZERO
        };
        let offset = if shake > 0.0 {
            Vec3::new((t * 67.0).sin() * 2.2, -(t * 51.0).sin() * 1.6, 0.0) * (shake * frame.su)
        } else {
            Vec3::ZERO
        };
        transform.translation = badge.home + lean + offset;
    }
}

/// Patience lamps: amber for pulls the station will still tolerate; the
/// dead state reads as red glass, not just darkness.
fn update_patience(
    time: Res<Time>,
    shell: Res<Shell>,
    lamps: Query<(&PatienceLamp, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let patience = shell.bridge.sim.barter().map(|barter| barter.patience);
    // The last tolerated pull gets a heartbeat: the lone lamp swells
    // gently so "one refusal from shuttered" reads at a glance, in
    // brightness, without a new signal color.
    let pulse = glow::breathe(time.elapsed_secs(), 4.6, 0.0).mul_add(0.30, 0.62);
    for (lamp, handle) in &lamps {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            match patience {
                Some(0) => glow::set_lamp(&mut material, palette::LAMP_NO, 0.15),
                Some(1) => glow::set_lamp(
                    &mut material,
                    palette::AMBER,
                    if lamp.0 == 0 { pulse } else { 0.0 },
                ),
                Some(left) => glow::set_lamp(
                    &mut material,
                    palette::AMBER,
                    if lamp.0 < left { 0.85 } else { 0.0 },
                ),
                None => glow::set_lamp(&mut material, palette::AMBER, 0.0),
            }
        }
    }
}

/// Refused: the gauge flashes red. Accepted: a green ring swells from the
/// hub, scaled by how generous the trade was. Both die inside half a
/// second — the latched clocks in [`BarterFx`] own the timing.
fn update_flashes(
    fx: Res<BarterFx>,
    frame: Option<Res<BarterFrame>>,
    refuse_mat: Option<Res<RefuseMat>>,
    mut arcs: Query<&mut Visibility, (With<RefuseArc>, Without<AcceptRing>)>,
    mut rings: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<AcceptRing>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(frame), Some(refuse_mat)) = (frame, refuse_mat) else {
        return;
    };
    let heat = fx.refuse / REFUSE_LEN;
    for mut vis in &mut arcs {
        vis.set_if_neq(shown(heat > 0.0));
    }
    if let Some(mut material) = materials.get_mut(&refuse_mat.0) {
        material.emissive = palette::LAMP_NO.to_linear() * (heat * 3.5);
    }
    let ring_heat = fx.accept / ACCEPT_LEN;
    for (mut transform, mut vis, handle) in &mut rings {
        vis.set_if_neq(shown(ring_heat > 0.0));
        if ring_heat > 0.0 {
            let grow = (1.0 - ring_heat) * 45.0 * fx.accept_value.mul_add(1.2, 1.0);
            let scale = (grow + 12.0) * frame.su;
            transform.scale = Vec3::new(scale, 1.0, scale);
            if let Some(mut material) = materials.get_mut(&handle.0) {
                material.emissive = palette::LAMP_OK.to_linear() * (ring_heat * 4.0);
            }
        }
    }
}

/// The go-lamp: green breathing when the station would say yes to a trade
/// of known quantities; amber shimmer over fogged pads — pull and find
/// The brass handle follows the pull gesture along its track and springs
/// home on a timid release — the gesture layer owns the throw math.
fn slide_accept_handle(
    grips: Res<crate::gesture::Grips>,
    frame: Option<Res<BarterFrame>>,
    mut handle: Single<&mut Transform, (With<AcceptHandle>, Without<GoLamp>)>,
    mut lamp: Single<&mut Transform, (With<GoLamp>, Without<AcceptHandle>)>,
) {
    let Some(frame) = frame else { return };
    let rect = layout::ACCEPT_LEVER;
    let mid_y = rect.h.mul_add(0.5, rect.y);
    let x = grips.accept.travel.mul_add(rect.w - 54.0, rect.x + 27.0);
    handle.translation = frame.local(x, mid_y, 0.01);
    // The go-lamp rides its handle — light and hand travel together.
    lamp.translation = frame.local(x, rect.y + 11.0, 0.022);
}

/// out, it says; dark glass otherwise.
fn update_lever(
    time: Res<Time>,
    shell: Res<Shell>,
    pointer: Res<crate::surface::VirtualPointer>,
    lamps: Query<&MeshMaterial3d<StandardMaterial>, With<GoLamp>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let (color, level) = shell
        .bridge
        .sim
        .barter()
        .map_or((palette::LAMP_OK, 0.0), |barter| {
            if barter.fog > 0.0 {
                (
                    palette::AMBER,
                    glow::breathe(t, 5.0, 0.0).mul_add(0.4, 0.25),
                )
            } else if barter.ready {
                (
                    palette::LAMP_OK,
                    glow::breathe(t, 2.8, 0.0).mul_add(0.24, 0.66),
                )
            } else {
                (palette::LAMP_OK, 0.0)
            }
        });
    // Hover wakes the lamp faintly even when a pull would refuse —
    // interactable, not ready. Lamps are the affordance language.
    let level = if layout::ACCEPT_LEVER.contains(pointer.sim) {
        level.max(0.18)
    } else {
        level
    };
    for handle in &lamps {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            glow::set_lamp(&mut material, color, level);
        }
    }
}

/// Unroll or retract the shutter from its latched progress.
fn update_shutter(
    fx: Res<BarterFx>,
    mut shutters: Query<(&mut Transform, &mut Visibility), With<Shutter>>,
) {
    let p = fx.shutter;
    let eased = p * p * 2.0f32.mul_add(-p, 3.0);
    for (mut transform, mut vis) in &mut shutters {
        transform.scale.y = eased.max(0.001);
        vis.set_if_neq(shown(p > 0.01));
    }
}

/// Repaint the wants chips and their pips from this visit's list.
fn update_wants(
    shell: Res<Shell>,
    chips: Query<(&WantChip, &MeshMaterial3d<StandardMaterial>)>,
    pips: Query<(&WantPip, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(barter) = shell.bridge.sim.barter() else {
        return;
    };
    for (chip, handle) in &chips {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color = palette::kind_color(barter.wants[chip.0].0);
        }
    }
    for (pip, handle) in &pips {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let lit = pip.index < barter.wants[pip.chip].1;
            material.emissive = if lit {
                palette::AMBER.to_linear() * 3.0
            } else {
                LinearRgba::NONE
            };
        }
    }
}

/// The voyage strip: origin and destination enamel, a dashed phosphor
/// course between, the dart exactly as far along as the sky says. Docked
/// at a barterless berth, the strip collapses to the berth's own disc.
fn update_voyage(
    shell: Res<Shell>,
    frame: Option<Res<BarterFrame>>,
    mut gizmos: Gizmos,
    mut parts: Query<(
        &VoyagePart,
        &mut Visibility,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(frame) = frame else { return };
    let sim = &shell.bridge.sim;
    if sim.barter().is_some() {
        return; // The wayside face is hidden; leave its parts be.
    }
    let paint = |handle: &MeshMaterial3d<StandardMaterial>,
                 materials: &mut Assets<StandardMaterial>,
                 id: u8| {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color = palette::poi_color(id);
        }
    };
    match sim.ship().state {
        ShipState::Traveling {
            from,
            to,
            progress,
            leg_ticks,
        } => {
            let frac = ((progress as f32 + sim.alpha()) / leg_ticks.max(1) as f32).clamp(0.0, 1.0);
            for (part, mut vis, mut transform, handle) in &mut parts {
                vis.set_if_neq(Visibility::Inherited);
                match part {
                    VoyagePart::From => paint(handle, &mut materials, from),
                    VoyagePart::To => paint(handle, &mut materials, to),
                    VoyagePart::Dart => {
                        let x = frac.mul_add(LINE_X1 - LINE_X0, LINE_X0);
                        transform.translation = frame.local(x, ROW_Y, Z_DART);
                    }
                }
            }
            // The dashed course, redrawn live: gizmos suit phosphor lines.
            let normal = frame.surface.normal();
            for i in 0..20_u8 {
                let x0 = f32::from(i).mul_add(13.0, LINE_X0);
                let x1 = (x0 + 6.0).min(LINE_X1);
                let a = frame.surface.to_world(SimVec2::new(x0, ROW_Y)) + normal * GIZMO_LIFT;
                let b = frame.surface.to_world(SimVec2::new(x1, ROW_Y)) + normal * GIZMO_LIFT;
                gizmos.line(a, b, palette::PHOSPHOR_DIM);
            }
        }
        ShipState::Docked(at) => {
            for (part, mut vis, _, handle) in &mut parts {
                if *part == VoyagePart::From {
                    vis.set_if_neq(Visibility::Inherited);
                    paint(handle, &mut materials, at);
                } else {
                    vis.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}

/// Hang the plaque while something is alongside; show only the emblem of
/// what it is. The gas pump dims to 35% once its one top-up is spent.
fn update_encounter(
    shell: Res<Shell>,
    mut plaques: Query<&mut Visibility, (With<EncounterPlaque>, Without<Emblem>)>,
    mut emblems: Query<(&Emblem, &mut Visibility)>,
    pumps: Query<&MeshMaterial3d<StandardMaterial>, With<GasBody>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sim = &shell.bridge.sim;
    let underway =
        sim.barter().is_none() && matches!(sim.ship().state, ShipState::Traveling { .. });
    let alongside = if underway {
        sim.encounter().filter(|enc| enc.open())
    } else {
        None
    };
    for mut vis in &mut plaques {
        vis.set_if_neq(shown(alongside.is_some()));
    }
    for (emblem, mut vis) in &mut emblems {
        vis.set_if_neq(shown(alongside.is_some_and(|enc| enc.kind == emblem.0)));
    }
    let spent = sim.encounter().is_some_and(|enc| enc.used);
    for handle in &pumps {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let glow = if spent { GAS_GLOW * 0.35 } else { GAS_GLOW };
            material.emissive = palette::PHOSPHOR.to_linear() * glow;
        }
    }
}

/// ???'s toll: three violet diamonds over the rail's first wells, each
/// breathing once a mysterious crate sits in its socket, a faint eerie
/// dot marking the empties. Wordless arithmetic, same as the 2D ledger.
fn update_toll(
    time: Res<Time>,
    shell: Res<Shell>,
    mut groups: Query<&mut Visibility, With<TollGroup>>,
    rings: Query<&TollRing>,
    dots: Query<(&TollDot, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sim = &shell.bridge.sim;
    let at_wanderer = sim.barter().is_none()
        && matches!(sim.ship().state, ShipState::Docked(at) if at == WANDERER);
    for mut vis in &mut groups {
        vis.set_if_neq(shown(at_wanderer));
    }
    if !at_wanderer {
        return;
    }
    let t = time.elapsed_secs();
    let filled = |slot: u8| {
        sim.pieces()
            .iter()
            .any(|p| p.kind == Kind::MysteriousCrate && p.loc == Loc::Flotsam { slot })
    };
    for ring in &rings {
        if let Some(mut material) = materials.get_mut(&ring.mat) {
            let level = if filled(ring.slot) {
                glow::breathe(t, 0.8, f32::from(ring.slot)).mul_add(0.8, 2.2)
            } else {
                1.0
            };
            material.emissive = palette::EERIE_BRIGHT.to_linear() * level;
        }
    }
    for (dot, handle) in &dots {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let level = if filled(dot.0) { 0.0 } else { 1.4 };
            material.emissive = palette::EERIE.to_linear() * level;
        }
    }
}

// ---- Decoration (idle loops; phases from indices, time from Res<Time>) ----

/// Constant spin — the casino's running lights.
fn spin_decor(time: Res<Time>, mut spinners: Query<(&Spin, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (spin, mut transform) in &mut spinners {
        transform.rotation = Quat::from_rotation_z(t * spin.0);
    }
}

/// Gentle rocking — the derelict's list, the whale's sway.
fn sway_decor(time: Res<Time>, mut swayers: Query<(&Sway, &mut Transform), Without<Spin>>) {
    let t = time.elapsed_secs();
    for (sway, mut transform) in &mut swayers {
        transform.rotation = Quat::from_rotation_z((t * sway.freq).sin() * sway.amp);
    }
}

/// Meteor streaks sliding along their own axis on a short loop.
#[allow(clippy::type_complexity)] // bevy query filters are what they are
fn streak_decor(
    time: Res<Time>,
    frame: Option<Res<BarterFrame>>,
    mut streaks: Query<(&MeteorStreak, &mut Transform), (Without<Spin>, Without<Sway>)>,
) {
    let Some(frame) = frame else { return };
    let t = time.elapsed_secs();
    let dir = Vec3::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2, 0.0);
    for (streak, mut transform) in &mut streaks {
        let phase = t.mul_add(3.0, streak.index as f32).fract();
        let slide = phase.mul_add(5.0, -2.5) * frame.su;
        transform.translation = streak.home + dir * slide;
    }
}
