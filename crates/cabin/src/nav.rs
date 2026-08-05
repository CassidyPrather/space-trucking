//! The navigation tank: the star map as a shallow shadow-box orrery
//! behind the [`Station::Map`] surface. A SCREEN backing quad sits a few
//! millimetres behind the mapped plane inside a PLATE frame; the readings
//! — starfield, sun, orbit rings, POI markers, the freighter, the sweep —
//! float just proud of it, phosphor on glass, exactly the semantics of the
//! 2D console's `draw_map` translated into shallow depth. The sim stays
//! the only authority: every position is `sim.poi_pos`/`ship.interpolated`
//! mapped through [`SimSurface::to_world`], every state ring reads the
//! same predicate the click consults.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

use space_trucking::sim::{
    COMET, Cue, EncounterKind, GUILD, POIS, PoiId, SATURN, SUN, ShipState, Sim, Track,
    Vec2 as SimVec2, WANDERER, leg_endpoints, splitmix,
};

use crate::rig::Skin;
use crate::surface::{SimSurface, Station, VirtualPointer};
use crate::{Phase, Shell, glow, palette};

// --- Depth ladder, metres along the tank normal. The backing recesses;
// --- readings float 5–15 mm proud of it, nearest-the-glass last.
const LIFT_BACKING: f32 = -0.003;
const LIFT_STARS: f32 = -0.001;
const LIFT_ORBITS: f32 = 0.002;
const LIFT_SWEEP: f32 = 0.003;
const LIFT_WHALE: f32 = 0.005;
const LIFT_MARKERS: f32 = 0.006;
const LIFT_ROUTE: f32 = 0.007;
const LIFT_STATES: f32 = 0.008;
const LIFT_HAZE: f32 = 0.009;
const LIFT_PARADE: f32 = 0.009;
const LIFT_SHIP: f32 = 0.010;
const LIFT_DRONE: f32 = 0.011;

/// Render-side starfield seed, the same idea as the 2D `STAR_SEED`;
/// cosmetic only, never fed back to the sim.
const STAR_SEED: u64 = 0x57A2_F1E1;
const STAR_COUNT: u64 = 60;

/// Seconds per sonar-sweep revolution, matching the 2D tank.
const SWEEP_PERIOD: f32 = 20.0;

/// Route dash cadence, sim units, matching the 2D dotted route.
const DASH_STEP: f32 = 13.0;
const DASH_LEN: f32 = 6.0;

/// Feedback tweens finish inside half a second — the one exception is the
/// sanctioned long catch-up pulse.
const FEEDBACK_LEN: f32 = 0.5;
const STALL_LEN: f32 = 3.5;

// --- Emissive intensities: 1.0 reads lit, 4.0+ blooms.
const STAR_GLOW: f32 = 2.2;
const SUN_GLOW: f32 = 3.0;
const MARKER_GLOW: f32 = 2.0;
const SHIP_GLOW: f32 = 4.0;
const EXHAUST_GLOW: f32 = 2.5;
const DRONE_GLOW: f32 = 2.5;
const WHALE_GLOW: f32 = 1.1;
const PARADE_GLOW: f32 = 2.2;

/// The Map surface, copied once at spawn so every tank system maps
/// sim → world without re-finding the panel entity.
#[derive(Resource)]
struct Tank {
    surface: SimSurface,
}

/// A twinkling star: its mixed color and breathe parameters, all derived
/// from the splitmix hash of its index.
#[derive(Component)]
struct Star {
    color: Color,
    base: f32,
    speed: f32,
    phase: f32,
}

/// The amber sun sphere (own material; it flickers gently).
#[derive(Component)]
struct SunDot;

/// One POI marker root; children carry the silhouette meshes.
#[derive(Component)]
struct PoiMarker {
    id: PoiId,
}

/// The ??? diamond facet whose emissive phases in and out of existing.
#[derive(Component)]
struct WandererFacet;

/// The dim haze disc over a comet already picked clean this apparition.
#[derive(Component)]
struct CometHaze;

/// The freighter dart.
#[derive(Component)]
struct ShipDart;

/// The dart's exhaust triangle (own material; it flickers).
#[derive(Component)]
struct Exhaust;

/// The ad drone's billboard dot.
#[derive(Component)]
struct DroneDot;

/// The whale crossing the lower tank while its encounter is open.
#[derive(Component)]
struct WhaleShade;

/// One shape in the Grand Parade's file; `index` 0 is the lead.
#[derive(Component)]
struct ParadeShape {
    index: u32,
    /// Draw radius, sim units.
    radius: f32,
}

/// The comet haze's query, disjoint from the marker roots by filter.
type HazeQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (With<CometHaze>, Without<PoiMarker>),
>;

/// The exhaust plume's query, disjoint from the dart by filter.
type ExhaustQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static MeshMaterial3d<StandardMaterial>,
    ),
    (With<Exhaust>, Without<ShipDart>),
>;

/// Feedback latches: cues are only valid during their frame's View phase,
/// so the tween timers live here.
#[derive(Default)]
struct Feedback {
    booted: bool,
    select: Option<(PoiId, f32)>,
    arrive: Option<(PoiId, f32)>,
    delivered: f32,
    stall: f32,
}

pub struct NavPlugin;

impl Plugin for NavPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn).add_systems(
            Update,
            (
                place_pois,
                shimmer,
                animate_ship,
                animate_drone,
                animate_whale,
                animate_parade,
                draw_gizmos,
            )
                .in_set(Phase::View),
        );
    }
}

/// Sim position → world, floated `lift` metres along the tank normal.
fn lifted(surface: &SimSurface, at: SimVec2, lift: f32) -> Vec3 {
    surface.to_world(at) + surface.normal() * lift
}

/// One flat triangle in the local XY plane, facing +Z.
fn tri_mesh(a: Vec2, b: Vec2, c: Vec2) -> Mesh {
    let positions = vec![[a.x, a.y, 0.0], [b.x, b.y, 0.0], [c.x, c.y, 0.0]];
    let normals = vec![[0.0, 0.0, 1.0]; 3];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

/// A thin phosphor ring in the tank plane. Gizmos render into the crunch
/// camera and pixelate agreeably.
fn ring(
    gizmos: &mut Gizmos,
    surface: &SimSurface,
    at: SimVec2,
    radius: f32,
    lift: f32,
    color: Color,
) {
    gizmos.circle(
        Isometry3d::new(lifted(surface, at, lift), surface.orientation()),
        radius * surface.scale_u(),
        color,
    );
}

/// Spawn the whole tank: backing, frame, starfield, sun, POI markers,
/// ship, drone, whale, parade. Everything mapped, nothing random.
#[allow(clippy::too_many_lines)]
fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
) {
    let Some(surface) = surfaces
        .iter()
        .find_map(|(station, s)| (*station == Station::Map).then_some(*s))
    else {
        return;
    };
    let rot = surface.orientation();
    let normal = surface.normal();
    let scale = surface.scale_u();
    let rect = surface.rect;
    let width = surface.half_u.length() * 2.0;
    let height = surface.half_v.length() * 2.0;

    // --- The recessed screen: SCREEN glass a few mm behind the plane,
    // --- bordered by a PLATE frame just proud of it.
    commands.spawn((
        Mesh3d(skin.cube.clone()),
        MeshMaterial3d(skin.screen.clone()),
        Transform::from_translation(surface.center + normal * LIFT_BACKING)
            .with_rotation(rot)
            .with_scale(Vec3::new(width, height, 0.002)),
    ));
    let half_w = width * 0.5 + 0.006;
    let half_h = height * 0.5 + 0.006;
    let across = Vec3::new(width + 0.024, 0.012, 0.008);
    let upright = Vec3::new(0.012, height + 0.024, 0.008);
    let frame = [
        (Vec3::new(0.0, half_h, 0.001), across),
        (Vec3::new(0.0, -half_h, 0.001), across),
        (Vec3::new(-half_w, 0.0, 0.001), upright),
        (Vec3::new(half_w, 0.0, 0.001), upright),
    ];
    for (local, size) in frame {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.plate.clone()),
            Transform::from_translation(surface.center + rot * local)
                .with_rotation(rot)
                .with_scale(size),
        ));
    }

    // --- Shared low-poly meshes.
    let dot = meshes.add(
        Sphere::new(1.0)
            .mesh()
            .ico(2)
            .expect("2 subdivisions is well under the icosphere cap"),
    );
    let hepta = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(7));
    let disc = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16));

    // --- Starfield: deterministic splitmix scatter, a few of them alive.
    for i in 0..STAR_COUNT {
        let hash = splitmix(STAR_SEED, i);
        let u_frac = (hash & 0xFFFF) as f32 / 65_535.0;
        let v_frac = ((hash >> 16) & 0xFFFF) as f32 / 65_535.0;
        let base = ((hash >> 32) & 0xFF) as f32 / 255.0;
        let phase = ((hash >> 40) & 0xFF) as f32 / 255.0 * std::f32::consts::TAU;
        let speed = (((hash >> 48) & 0x3) as f32).mul_add(0.6, 0.4);
        let at = SimVec2::new(
            u_frac.mul_add(rect.w - 8.0, rect.x + 4.0),
            v_frac.mul_add(rect.h - 8.0, rect.y + 4.0),
        );
        let size = if hash & 0x300 == 0 { 0.0024 } else { 0.0014 };
        let color = palette::mix(palette::PHOSPHOR_DIM, palette::PHOSPHOR, base);
        let level = base.mul_add(0.45, 0.2);
        let material = glow::phosphor(&mut materials, color, level * STAR_GLOW);
        let mut star = commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(lifted(&surface, at, LIFT_STARS))
                .with_rotation(rot)
                .with_scale(Vec3::new(size, size, 0.0008)),
        ));
        // Every fifth star twinkles; the rest hold their spawn level.
        if i % 5 == 0 {
            star.insert(Star {
                color,
                base,
                speed,
                phase,
            });
        }
    }

    // --- The sun: amber sphere with a GLINT core, gentle flicker.
    let sun_mat = glow::phosphor(&mut materials, palette::AMBER, SUN_GLOW);
    let glint_mat = glow::phosphor(&mut materials, palette::GLINT, 2.5);
    commands
        .spawn((
            SunDot,
            Mesh3d(dot.clone()),
            MeshMaterial3d(sun_mat),
            Transform::from_translation(lifted(&surface, SUN, LIFT_MARKERS))
                .with_rotation(rot)
                .with_scale(Vec3::splat(7.0 * scale)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(dot.clone()),
                MeshMaterial3d(glint_mat),
                Transform::from_translation(Vec3::new(0.0, 0.0, 0.5)).with_scale(Vec3::splat(0.5)),
            ));
        });

    // --- POI markers: phosphor readings of the worlds, one root each,
    // --- silhouette children for the special ones.
    for (i, poi) in POIS.iter().enumerate() {
        let id = i as PoiId;
        let r = poi.radius * scale;
        // A CRT reading of Venus, not Venus: identity leans hard phosphor.
        let color = palette::mix(palette::poi_color(id), palette::PHOSPHOR, 0.7);
        let material = glow::phosphor(&mut materials, color, MARKER_GLOW);
        // Extra materials for the silhouettes, made before the children
        // close over the borrow.
        let comet_core = (id == COMET).then(|| {
            glow::phosphor(
                &mut materials,
                palette::mix(palette::GLINT, palette::PHOSPHOR, 0.5),
                MARKER_GLOW,
            )
        });
        let mut anchor = commands.spawn((
            PoiMarker { id },
            Transform::from_translation(lifted(&surface, SUN, LIFT_MARKERS)).with_rotation(rot),
            Visibility::Hidden,
        ));
        anchor.with_children(|parent| match id {
            // The Guild: a seven-sided station; schematics show six.
            GUILD => {
                parent.spawn((
                    Mesh3d(hepta.clone()),
                    MeshMaterial3d(material),
                    Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(r, 0.004, r)),
                ));
            }
            // Saturn: a globe under a flat salvage ring, tilted a touch
            // out of the tank plane so it reads as a ring in 3D.
            // (Simplified from 2D: no gravel grains along the ring.)
            SATURN => {
                parent.spawn((
                    Mesh3d(dot.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(r * 0.85)),
                ));
                let torus = meshes.add(Torus::new(1.3, 1.6));
                parent.spawn((
                    Mesh3d(torus),
                    MeshMaterial3d(material),
                    Transform::from_rotation(Quat::from_rotation_x(
                        std::f32::consts::FRAC_PI_2 - 0.35,
                    ))
                    .with_scale(Vec3::splat(r)),
                ));
            }
            // The comet: icy head, GLINT core, tail dots thrown away from
            // the sun (the root rotates to aim local +X sunward-away).
            COMET => {
                parent.spawn((
                    Mesh3d(dot.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_scale(Vec3::splat(r * 0.75)),
                ));
                parent.spawn((
                    Mesh3d(dot.clone()),
                    MeshMaterial3d(comet_core.clone().unwrap_or_else(|| material.clone())),
                    Transform::from_translation(Vec3::new(-0.2 * r, 0.0, 0.0005))
                        .with_scale(Vec3::splat(r * 0.35)),
                ));
                for k in 0..3_u8 {
                    let reach = f32::from(k).mul_add(0.9, 1.4);
                    parent.spawn((
                        Mesh3d(dot.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(Vec3::new(r * reach, 0.0, 0.0))
                            .with_scale(Vec3::splat(f32::from(k).mul_add(-0.07, 0.35) * r)),
                    ));
                }
            }
            // ???: a diamond not entirely committed to existing — its
            // facet's emissive phases via breathe.
            WANDERER => {
                parent.spawn((
                    WandererFacet,
                    Mesh3d(skin.cube.clone()),
                    MeshMaterial3d(material),
                    Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
                        .with_scale(Vec3::new(r * 1.3, r * 1.3, 0.004)),
                ));
            }
            // Everyone else: a plain low-poly sphere. (Simplified from the
            // 2D glyph set — smog arcs, storm streaks, crescents and lit
            // windows are dropped; identity survives in the tinted hue.)
            _ => {
                parent.spawn((
                    Mesh3d(dot.clone()),
                    MeshMaterial3d(material),
                    Transform::from_scale(Vec3::splat(r)),
                ));
            }
        });
    }

    // --- Comet-spent haze: a dark translucent veil over the marker.
    let haze_mat = materials.add(StandardMaterial {
        base_color: palette::SCREEN.with_alpha(0.55),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let comet_r = POIS[usize::from(COMET)].radius * scale;
    commands.spawn((
        CometHaze,
        Mesh3d(disc),
        MeshMaterial3d(haze_mat),
        Transform::from_translation(lifted(&surface, SUN, LIFT_HAZE))
            .with_rotation(rot * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::new(comet_r * 1.6, 0.001, comet_r * 1.6)),
        Visibility::Hidden,
    ));

    // --- The freighter: a hot dart nosing +X, exhaust triangle astern.
    let dart = meshes.add(tri_mesh(
        Vec2::new(8.0 * scale, 0.0),
        Vec2::new(-4.0 * scale, 4.5 * scale),
        Vec2::new(-4.0 * scale, -4.5 * scale),
    ));
    // Unit-length exhaust; its x-scale becomes the flickering tail.
    let plume = meshes.add(tri_mesh(
        Vec2::new(0.0, 2.2 * scale),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, -2.2 * scale),
    ));
    let ship_mat = glow::phosphor(&mut materials, palette::PHOSPHOR_HOT, SHIP_GLOW);
    let plume_mat = glow::phosphor(&mut materials, palette::PHOSPHOR, EXHAUST_GLOW);
    commands
        .spawn((
            ShipDart,
            Mesh3d(dart),
            MeshMaterial3d(ship_mat),
            Transform::from_translation(lifted(&surface, SUN, LIFT_SHIP)).with_rotation(rot),
        ))
        .with_children(|parent| {
            parent.spawn((
                Exhaust,
                Mesh3d(plume),
                MeshMaterial3d(plume_mat),
                Transform::from_translation(Vec3::new(-4.0 * scale, 0.0, -0.0003))
                    .with_scale(Vec3::new(6.0 * scale, 1.0, 1.0)),
            ));
        });

    // --- The ad drone: a tiny amber billboard dot on its tight orbit.
    let drone_mat = glow::phosphor(&mut materials, palette::AMBER, DRONE_GLOW);
    commands.spawn((
        DroneDot,
        Mesh3d(dot.clone()),
        MeshMaterial3d(drone_mat),
        Transform::from_translation(lifted(&surface, SUN, LIFT_DRONE))
            .with_scale(Vec3::splat(2.2 * scale)),
        Visibility::Hidden,
    ));

    // --- The whale: dim oval, fluke, one bright eye, per the 2D company.
    let whale_mat = glow::phosphor(&mut materials, palette::PHOSPHOR_DIM, WHALE_GLOW);
    let eye_mat = glow::phosphor(&mut materials, palette::PHOSPHOR, MARKER_GLOW);
    let fluke = meshes.add(tri_mesh(
        Vec2::new(-30.0 * scale, 0.0),
        Vec2::new(-38.0 * scale, 7.0 * scale),
        Vec2::new(-38.0 * scale, -7.0 * scale),
    ));
    commands
        .spawn((
            WhaleShade,
            Transform::from_translation(lifted(&surface, SUN, LIFT_WHALE)).with_rotation(rot),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            // The body: a flattened sphere with the 2D oval's slight list.
            parent.spawn((
                Mesh3d(dot.clone()),
                MeshMaterial3d(whale_mat.clone()),
                Transform::from_rotation(Quat::from_rotation_z(0.07)).with_scale(Vec3::new(
                    26.0 * scale,
                    7.0 * scale,
                    0.004,
                )),
            ));
            parent.spawn((
                Mesh3d(fluke),
                MeshMaterial3d(whale_mat.clone()),
                Transform::IDENTITY,
            ));
            parent.spawn((
                Mesh3d(dot.clone()),
                MeshMaterial3d(eye_mat),
                Transform::from_translation(Vec3::new(18.0 * scale, 2.0 * scale, 0.0005))
                    .with_scale(Vec3::splat(1.2 * scale)),
            ));
        });

    // --- The Grand Parade: five eerie heptagons in file. (Simplified
    // --- from 2D: flat EERIE discs with a scale pulse stand in for the
    // --- fill + EERIE_BRIGHT edge-ring pair and its alpha pulse.)
    let parade_mat = glow::phosphor(&mut materials, palette::EERIE, PARADE_GLOW);
    for index in 0..5_u32 {
        let radius = if index == 0 { 14.0 } else { 7.0 - index as f32 };
        commands.spawn((
            ParadeShape { index, radius },
            Mesh3d(hepta.clone()),
            MeshMaterial3d(parade_mat.clone()),
            Transform::from_translation(lifted(&surface, SUN, LIFT_PARADE)),
            Visibility::Hidden,
        ));
    }

    commands.insert_resource(Tank { surface });
}

/// Decorative emissive loops: star twinkle, sun flicker, the ??? phase.
/// Each mutated material is its own instance, made at spawn.
fn shimmer(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stars: Query<(&MeshMaterial3d<StandardMaterial>, &Star)>,
    sun: Query<&MeshMaterial3d<StandardMaterial>, With<SunDot>>,
    wanderer: Query<&MeshMaterial3d<StandardMaterial>, With<WandererFacet>>,
) {
    let t = time.elapsed_secs();
    for (handle, star) in &stars {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let level = glow::breathe(t, star.speed, star.phase)
                .mul_add(0.3, star.base.mul_add(0.45, 0.15));
            material.emissive = star.color.to_linear() * (level * STAR_GLOW);
        }
    }
    for handle in &sun {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let flicker = glow::breathe(t, 3.0, 0.0).mul_add(0.12, 0.88);
            material.emissive = palette::AMBER.to_linear() * (flicker * SUN_GLOW);
        }
    }
    for handle in &wanderer {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let there = glow::breathe(t, 6.3, 0.0).mul_add(0.5, 0.4);
            let color = palette::mix(palette::poi_color(WANDERER), palette::PHOSPHOR, 0.7);
            material.emissive = color.to_linear() * (there * MARKER_GLOW);
        }
    }
}

/// Every visible POI marker rides its live sim position; the comet aims
/// its tail away from the sun, the Guild turns slowly, the spent comet
/// gains its haze.
fn place_pois(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    mut markers: Query<(&PoiMarker, &mut Transform, &mut Visibility)>,
    mut haze: HazeQuery,
) {
    let sim = &shell.bridge.sim;
    let surface = tank.surface;
    let rot = surface.orientation();
    let t = time.elapsed_secs();
    for (marker, mut transform, mut visibility) in &mut markers {
        if !sim.poi_visible(marker.id) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos = sim.poi_pos(marker.id);
        transform.translation = lifted(&surface, pos, LIFT_MARKERS);
        transform.rotation = match marker.id {
            GUILD => rot * Quat::from_rotation_z(t * 0.4),
            COMET => {
                // Sim y runs down the panel: a sim direction (x, y) is the
                // panel-local direction (x, -y).
                let away = pos - SUN;
                let angle = if away.length() < f32::EPSILON {
                    0.0
                } else {
                    (-away.y).atan2(away.x)
                };
                rot * Quat::from_rotation_z(angle)
            }
            _ => rot,
        };
    }
    let comet_shrouded = sim.poi_visible(COMET) && sim.comet_spent();
    for (mut transform, mut visibility) in &mut haze {
        if comet_shrouded {
            *visibility = Visibility::Visible;
            transform.translation = lifted(&surface, sim.poi_pos(COMET), LIFT_HAZE);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

/// The freighter dart noses along its travel direction; the exhaust
/// flickers, faster and longer under warp — same numbers as the 2D flame.
fn animate_ship(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dart: Query<&mut Transform, With<ShipDart>>,
    mut exhaust: ExhaustQuery,
) {
    let sim = &shell.bridge.sim;
    let surface = tank.surface;
    let ship = sim.ship();
    let pos = ship.interpolated(sim.alpha());
    let heading = ship.pos - ship.prev_pos;
    let angle = if heading.length() < 1e-6 {
        0.0 // Parked or freshly loaded: nose along sim +x.
    } else {
        (-heading.y).atan2(heading.x)
    };
    for mut transform in &mut dart {
        transform.translation = lifted(&surface, pos, LIFT_SHIP);
        transform.rotation = surface.orientation() * Quat::from_rotation_z(angle);
    }
    let (freq, boost): (f32, f32) = if sim.is_warp() {
        (26.0, 1.9)
    } else {
        (9.0, 1.0)
    };
    let flick = glow::breathe(time.elapsed_secs(), freq, 0.0).mul_add(0.6, 0.4);
    let tail = (3.5 * boost).mul_add(flick, 3.0) * surface.scale_u();
    for (mut transform, handle) in &mut exhaust {
        transform.scale.x = tail;
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.emissive = palette::PHOSPHOR.to_linear() * (flick * EXHAUST_GLOW);
        }
    }
}

/// The ad drone dot rides its sim orbit; each landed swat widens the
/// wobble, same as the 2D billboard (whose rune board is not ported —
/// no text in the cabin, and the ad ticker belongs to another station).
fn animate_drone(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    mut drones: Query<(&mut Transform, &mut Visibility), With<DroneDot>>,
) {
    let sim = &shell.bridge.sim;
    let at = sim.drone_pos();
    for (mut transform, mut visibility) in &mut drones {
        let Some(at) = at else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        let wobble = f32::from(sim.drone_swats()) * (time.elapsed_secs() * 21.0).sin() * 2.0;
        let at = SimVec2::new(at.x + wobble, at.y);
        transform.translation = lifted(&tank.surface, at, LIFT_DRONE);
    }
}

/// While an open Whale rides alongside, its shade crosses the lower tank
/// on the encounter-window fraction, bobbing on the idle clock — the same
/// fraction math as the 2D spectacle.
fn animate_whale(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    mut whales: Query<(&mut Transform, &mut Visibility), With<WhaleShade>>,
) {
    let sim = &shell.bridge.sim;
    let rect = tank.surface.rect;
    let t = time.elapsed_secs();
    let spot = if let Some(enc) = sim.encounter()
        && enc.kind == EncounterKind::Whale
        && enc.open()
        && let ShipState::Traveling { progress, .. } = sim.ship().state
    {
        let frac = progress.saturating_sub(enc.start) as f32 / (enc.end - enc.start).max(1) as f32;
        Some(SimVec2::new(
            frac.mul_add(rect.w * 0.8, rect.w.mul_add(0.1, rect.x)),
            (t * 0.5).sin().mul_add(4.0, rect.h.mul_add(0.78, rect.y)),
        ))
    } else {
        None
    };
    for (mut transform, mut visibility) in &mut whales {
        if let Some(at) = spot {
            *visibility = Visibility::Visible;
            transform.translation = lifted(&tank.surface, at, LIFT_WHALE);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

/// The Grand Parade: a file of eerie heptagons crossing the tank at the
/// sim's fraction, spinning and breathing, explaining nothing.
fn animate_parade(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    mut shapes: Query<(&ParadeShape, &mut Transform, &mut Visibility)>,
) {
    let sim = &shell.bridge.sim;
    let surface = tank.surface;
    let rect = surface.rect;
    let frac = sim.parade();
    let t = time.elapsed_secs();
    for (shape, mut transform, mut visibility) in &mut shapes {
        let Some(frac) = frac else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let lead = SimVec2::new(
            frac.mul_add(rect.w + 120.0, rect.x - 60.0),
            frac.mul_add(-60.0, rect.h.mul_add(0.4, rect.y)),
        );
        let step = shape.index as f32;
        let back = lead - SimVec2::new(step.mul_add(22.0, 26.0), step * -6.0);
        if back.x < rect.x + 8.0 || back.x > rect.x + rect.w - 8.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pulse = glow::breathe(t, 2.0, step).mul_add(0.16, 0.88);
        let r = shape.radius * surface.scale_u() * pulse;
        *transform = Transform::from_translation(lifted(&surface, back, LIFT_PARADE))
            .with_rotation(
                surface.orientation()
                    * Quat::from_rotation_z(t * 0.6)
                    * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            )
            .with_scale(Vec3::new(r, 0.003, r));
    }
}

/// Everything drawn as phosphor lines: orbit rings, selection and state
/// rings, the dashed route, the sweep, and the latched feedback tweens.
fn draw_gizmos(
    time: Res<Time>,
    shell: Res<Shell>,
    tank: Res<Tank>,
    pointer: Res<VirtualPointer>,
    mut feedback: Local<Feedback>,
    mut gizmos: Gizmos,
) {
    let sim = &shell.bridge.sim;
    let surface = tank.surface;
    let t = time.elapsed_secs();

    // Decay first, then latch, so a fresh cue draws at full strength.
    let dt = time.delta_secs();
    feedback.select = feedback
        .select
        .and_then(|(id, left)| (left > dt).then_some((id, left - dt)));
    feedback.arrive = feedback
        .arrive
        .and_then(|(id, left)| (left > dt).then_some((id, left - dt)));
    feedback.delivered = (feedback.delivered - dt).max(0.0);
    feedback.stall = (feedback.stall - dt).max(0.0);
    if !feedback.booted {
        feedback.booted = true;
        // The ship docked somewhere during the boot catch-up: mark where.
        if shell.bridge.arrived_while_away {
            feedback.stall = STALL_LEN;
        }
    }
    if shell.outcome.stall_arrived {
        feedback.stall = STALL_LEN;
    }
    for cue in sim.cues() {
        match cue {
            Cue::Select => {
                feedback.select = sim.ship().selected.map(|id| (id, FEEDBACK_LEN));
            }
            Cue::Arrive => {
                if let ShipState::Docked(id) = sim.ship().state {
                    feedback.arrive = Some((id, FEEDBACK_LEN));
                }
            }
            Cue::Delivered => feedback.delivered = FEEDBACK_LEN,
            _ => {}
        }
    }

    draw_orbits(&mut gizmos, sim, &surface, t);
    draw_states(&mut gizmos, sim, &surface, &pointer, t);
    draw_route(&mut gizmos, sim, &surface);
    draw_sweep(&mut gizmos, &surface, t);
    draw_feedback(&mut gizmos, sim, &surface, &feedback, t);
}

/// The sun's amber halo and one dim ring per visible circular orbit.
/// Ellipse and fixed tracks draw none — a comet should feel like a
/// visitor, and ??? was never invited.
fn draw_orbits(gizmos: &mut Gizmos, sim: &Sim, surface: &SimSurface, t: f32) {
    let flicker = glow::breathe(t, 3.0, 0.0).mul_add(0.12, 0.88);
    ring(
        gizmos,
        surface,
        SUN,
        10.0,
        LIFT_ORBITS,
        palette::AMBER.with_alpha(0.25 * flicker),
    );
    for (i, poi) in sim.pois().iter().enumerate() {
        let Track::Circle { orbit, .. } = poi.track else {
            continue;
        };
        if !sim.poi_visible(i as PoiId) {
            continue;
        }
        gizmos
            .circle(
                Isometry3d::new(lifted(surface, SUN, LIFT_ORBITS), surface.orientation()),
                orbit * surface.scale_u(),
                palette::PHOSPHOR_DIM.with_alpha(0.4),
            )
            .resolution(48);
    }
}

/// Selection and state rings around each visible POI: the hover invite,
/// the armed breathing ring, the docked ring, and the papers refusal —
/// which carries a diagonal bar, because no signal rides on hue alone.
fn draw_states(
    gizmos: &mut Gizmos,
    sim: &Sim,
    surface: &SimSurface,
    pointer: &VirtualPointer,
    t: f32,
) {
    let docked = match sim.ship().state {
        ShipState::Docked(at) => Some(at),
        ShipState::Traveling { .. } => None,
    };
    for (i, poi) in sim.pois().iter().enumerate() {
        let id = i as PoiId;
        if !sim.poi_visible(id) {
            continue;
        }
        let pos = sim.poi_pos(id);

        // Inner-ring papers refusal: barred, visible, not chartable.
        if docked.is_some() && sim.inner_ring_locked(id) {
            let r = poi.radius + 4.0;
            let no = palette::LAMP_NO.with_alpha(0.55);
            ring(gizmos, surface, pos, r, LIFT_STATES, no);
            let bar = SimVec2::new(r * 0.8, -r * 0.8);
            gizmos.line(
                lifted(surface, pos - bar, LIFT_STATES),
                lifted(surface, pos + bar, LIFT_STATES),
                no,
            );
        }

        // Hover invite: same predicate the press consults, so the glass
        // never invites a refusal silently.
        if let Some(at) = docked
            && id != at
            && sim.poi_chartable(id)
            && (pointer.sim - pos).length() <= poi.radius
        {
            ring(
                gizmos,
                surface,
                pos,
                poi.radius + 3.0,
                LIFT_STATES,
                palette::PHOSPHOR.with_alpha(0.3),
            );
        }

        // Armed selection: a breathing ring, slightly larger.
        if sim.ship().selected == Some(id) {
            let wave = (t * 4.0).sin();
            ring(
                gizmos,
                surface,
                pos,
                wave.mul_add(1.5, poi.radius + 5.0),
                LIFT_STATES,
                palette::PHOSPHOR.with_alpha(wave.mul_add(0.15, 0.7)),
            );
        }

        // Docked: a steady bright ring marks home.
        if docked == Some(id) {
            ring(
                gizmos,
                surface,
                pos,
                poi.radius + 4.0,
                LIFT_STATES,
                palette::PHOSPHOR.with_alpha(0.85),
            );
        }

        // A comet already picked clean sits under its haze disc, which is
        // a mesh (see `CometHaze`), not a gizmo — nothing to draw here.
    }
}

/// The dashed charted line, cast-off point to arrival point — the same
/// derivation the sim flies, so the dashes and the freighter agree.
fn draw_route(gizmos: &mut Gizmos, sim: &Sim, surface: &SimSurface) {
    let ShipState::Traveling {
        from,
        to,
        progress,
        leg_ticks,
    } = sim.ship().state
    else {
        return;
    };
    let (a, b) = leg_endpoints(from, to, progress, leg_ticks, sim.tick());
    let span = b - a;
    let length = span.length();
    if length <= f32::EPSILON {
        return;
    }
    let dir = span * length.recip();
    let count = ((length / DASH_STEP) as i32).max(0);
    let color = palette::PHOSPHOR.with_alpha(0.45);
    for i in 0..count {
        let at = (i as f32) * DASH_STEP;
        gizmos.line(
            lifted(surface, a + dir * at, LIFT_ROUTE),
            lifted(surface, a + dir * (at + DASH_LEN).min(length), LIFT_ROUTE),
            color,
        );
    }
}

/// One slow sonar line about the tank centre, a full turn every twenty
/// seconds, with a two-step fading trail. Subtle on purpose.
fn draw_sweep(gizmos: &mut Gizmos, surface: &SimSurface, t: f32) {
    let rect = surface.rect;
    let center = SimVec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y));
    let radius = rect.w.min(rect.h).mul_add(0.5, -4.0);
    let angle = (t * std::f32::consts::TAU / SWEEP_PERIOD).rem_euclid(std::f32::consts::TAU);
    for (trail, alpha) in [(0.0, 0.22), (0.05, 0.10), (0.10, 0.05)] {
        let a = angle - trail;
        let tip = SimVec2::new(
            a.cos().mul_add(radius, center.x),
            a.sin().mul_add(radius, center.y),
        );
        gizmos.line(
            lifted(surface, center, LIFT_SWEEP),
            lifted(surface, tip, LIFT_SWEEP),
            palette::PHOSPHOR.with_alpha(alpha),
        );
    }
}

/// The latched feedback tweens: select blip, arrival pop, the Guild's
/// violet delivery bloom, and the long catch-up pulse at the dock.
fn draw_feedback(gizmos: &mut Gizmos, sim: &Sim, surface: &SimSurface, fb: &Feedback, t: f32) {
    if let Some((id, left)) = fb.select
        && sim.poi_visible(id)
    {
        let blip = left / FEEDBACK_LEN;
        let r = POIS[usize::from(id)].radius;
        ring(
            gizmos,
            surface,
            sim.poi_pos(id),
            (1.0 - blip).mul_add(9.0, r + 4.0),
            LIFT_STATES,
            palette::PHOSPHOR.with_alpha(blip * 0.8),
        );
    }
    if let Some((id, left)) = fb.arrive {
        let pop = left / FEEDBACK_LEN;
        let r = POIS[usize::from(id)].radius;
        ring(
            gizmos,
            surface,
            sim.poi_pos(id),
            (1.0 - pop).mul_add(12.0, r + 4.0),
            LIFT_STATES,
            palette::PHOSPHOR.with_alpha(pop * 0.9),
        );
    }
    if fb.delivered > 0.0 {
        // The hangar swallow: the Guild blooms violet for a moment. The
        // 2D dot becomes an inner ring — a filled gizmo disc is not worth
        // a mesh for a half-second flash.
        let flash = fb.delivered / FEEDBACK_LEN;
        let pos = sim.poi_pos(GUILD);
        let r = POIS[usize::from(GUILD)].radius;
        ring(
            gizmos,
            surface,
            pos,
            (1.0 - flash).mul_add(10.0, r + 3.0),
            LIFT_STATES,
            palette::EERIE_BRIGHT.with_alpha(0.7 * flash),
        );
        ring(
            gizmos,
            surface,
            pos,
            r + 2.0,
            LIFT_STATES,
            palette::EERIE.with_alpha(0.3 * flash),
        );
    }
    if fb.stall > 0.0
        && let ShipState::Docked(id) = sim.ship().state
    {
        // The sanctioned long tween: the returning player spots where
        // they landed while away.
        let wave = (t * 5.0).sin();
        let r = POIS[usize::from(id)].radius;
        ring(
            gizmos,
            surface,
            sim.poi_pos(id),
            wave.mul_add(2.5, r + 7.0),
            LIFT_STATES,
            palette::PHOSPHOR_HOT
                .with_alpha(wave.mul_add(0.25, 0.45) * (fb.stall / STALL_LEN * 3.0).min(1.0)),
        );
    }
}
