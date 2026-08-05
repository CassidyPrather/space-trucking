//! The cabin itself: an enclosed box with flavor, per DESIGN.md's first
//! pass. One seat, no locomotion — four panels arranged in a cramped
//! wraparound within a glance's reach, echoing the 2D console's layout so
//! muscle memory transfers: star tank upper left, console upper right,
//! hold tray low left, barter counter low right. The camera stays in the
//! seat and leans gently toward the cursor.
//!
//! Also home to the pixel crunch: the 3D view renders into a small texture
//! upscaled nearest-neighbour — the design doc's "smoothing off", applied
//! to the whole world — and to the shared low-poly material [`Skin`].

use bevy::camera::{Hdr, RenderTarget};
use bevy::image::ImageSampler;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;

use space_trucking::sim::layout;

use crate::palette;
use crate::surface::{SimSurface, Station};

/// The crunch target, in pixels. The window upscales this without
/// smoothing; hard pixel edges everywhere. One knob, like the 2D CRUNCH.
pub const CRUNCH_W: u32 = 480;
pub const CRUNCH_H: u32 = 270;

/// The seat: eye position, and where an unglanced gaze rests.
pub const EYE: Vec3 = Vec3::new(0.0, 1.24, 0.62);
const GAZE_AT: Vec3 = Vec3::new(0.0, 1.18, -1.2);

/// Glance envelope: how far the head turns toward the cursor, and how
/// eagerly. Decoration-adjacent but driven by input, so it always runs.
const GLANCE_YAW: f32 = 0.42;
const GLANCE_PITCH: f32 = 0.26;
const GLANCE_RATE: f32 = 7.0;

/// The four panels: where each sim region lives in the cabin.
/// Width/height keep each rect's aspect; scales differ per panel on
/// purpose (the hold tray is generous — it is the main play surface).
#[must_use]
pub fn panels() -> [(Station, SimSurface); 4] {
    let grid = layout::Rect::new(
        layout::GRID_ORIGIN.x,
        layout::GRID_ORIGIN.y,
        f32::from(layout::GRID_COLS) * layout::CELL,
        f32::from(layout::GRID_ROWS) * layout::CELL,
    );
    [
        (
            Station::Map,
            SimSurface::panel(
                Vec3::new(-0.56, 1.52, -1.28),
                1.00,
                0.84,
                0.10,
                layout::MAP_PANEL,
            ),
        ),
        (
            Station::Console,
            SimSurface::panel(
                Vec3::new(0.50, 1.52, -1.28),
                0.54,
                0.84,
                0.10,
                layout::CONSOLE,
            ),
        ),
        (
            Station::Hold,
            SimSurface::panel(Vec3::new(-0.64, 0.86, -0.94), 0.78, 0.52, 0.96, grid),
        ),
        (
            Station::Barter,
            SimSurface::panel(
                Vec3::new(0.42, 0.88, -0.96),
                1.12,
                0.32,
                0.96,
                layout::BARTER_PANEL,
            ),
        ),
    ]
}

/// Shared meshes and materials for the worn-metal family. Views make
/// their own phosphors; the metal is communal.
#[derive(Resource)]
pub struct Skin {
    pub hull: Handle<StandardMaterial>,
    pub plate: Handle<StandardMaterial>,
    pub plate_lit: Handle<StandardMaterial>,
    pub plate_shade: Handle<StandardMaterial>,
    pub socket: Handle<StandardMaterial>,
    pub screen: Handle<StandardMaterial>,
    pub brass: Handle<StandardMaterial>,
    pub rivet: Handle<StandardMaterial>,
    pub glass: Handle<StandardMaterial>,
    pub cube: Handle<Mesh>,
}

impl Skin {
    fn build(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let metal = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.92,
            metallic: 0.15,
            ..default()
        };
        Self {
            hull: materials.add(metal(palette::HULL)),
            plate: materials.add(metal(palette::PLATE)),
            plate_lit: materials.add(metal(palette::PLATE_LIT)),
            plate_shade: materials.add(metal(palette::PLATE_SHADE)),
            socket: materials.add(StandardMaterial {
                base_color: palette::SOCKET,
                perceptual_roughness: 1.0,
                metallic: 0.0,
                ..default()
            }),
            screen: materials.add(StandardMaterial {
                base_color: palette::SCREEN,
                perceptual_roughness: 0.35,
                metallic: 0.0,
                ..default()
            }),
            brass: materials.add(StandardMaterial {
                base_color: palette::BRASS,
                perceptual_roughness: 0.45,
                metallic: 0.8,
                ..default()
            }),
            rivet: materials.add(metal(palette::RIVET)),
            glass: materials.add(StandardMaterial {
                base_color: palette::GLASS,
                perceptual_roughness: 0.3,
                metallic: 0.0,
                ..default()
            }),
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        }
    }
}

/// The in-cabin camera (renders to the crunch target).
#[derive(Component)]
pub struct CabinCamera {
    base: Quat,
}

/// Cabin lights the omen may dim, remembering their honest brightness.
#[derive(Component)]
pub struct Dimmable {
    pub intensity: f32,
}

/// Marker for the fullscreen node showing the crunch target.
#[derive(Component)]
pub struct CrunchView;

/// Spawn the whole static cabin: crunch pipeline, cameras, box, panels,
/// lights, version text.
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let skin = Skin::build(&mut meshes, &mut materials);

    // --- The crunch: a small render target shown fullscreen, unsmoothed.
    let mut target = Image::new_target_texture(
        CRUNCH_W,
        CRUNCH_H,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    target.sampler = ImageSampler::nearest();
    let target = images.add(target);

    let base = Transform::from_translation(EYE)
        .looking_at(GAZE_AT, Vec3::Y)
        .rotation;
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(palette::VOID),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
        Hdr,
        Bloom::NATURAL,
        Msaa::Off,
        Transform::from_translation(EYE).with_rotation(base),
        CabinCamera { base },
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
        CrunchView,
    ));

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
    ));

    // --- The box. Inward faces only matter; slabs are cheap.
    let wall = |commands: &mut Commands, center: Vec3, size: Vec3| {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.hull.clone()),
            Transform::from_translation(center).with_scale(size),
        ));
    };
    wall(
        &mut commands,
        Vec3::new(0.0, -0.05, 0.2),
        Vec3::new(3.4, 0.1, 3.4),
    ); // floor
    wall(
        &mut commands,
        Vec3::new(0.0, 2.32, 0.2),
        Vec3::new(3.4, 0.1, 3.4),
    ); // ceiling
    wall(
        &mut commands,
        Vec3::new(0.0, 1.15, -1.42),
        Vec3::new(3.4, 2.5, 0.1),
    ); // front
    wall(
        &mut commands,
        Vec3::new(0.0, 1.15, 1.92),
        Vec3::new(3.4, 2.5, 0.1),
    ); // back
    wall(
        &mut commands,
        Vec3::new(-1.72, 1.15, 0.2),
        Vec3::new(0.1, 2.5, 3.4),
    ); // left
    wall(
        &mut commands,
        Vec3::new(1.72, 1.15, 0.2),
        Vec3::new(0.1, 2.5, 3.4),
    ); // right

    // Wall ribs and a ceiling duct: the junk that says somebody built
    // this hull in a hurry, decades ago. Deterministic, hand-placed.
    for i in 0..5 {
        let z = 0.7f32.mul_add(i as f32, -1.2);
        wall(
            &mut commands,
            Vec3::new(-1.66, 1.15, z),
            Vec3::new(0.06, 2.3, 0.08),
        );
        wall(
            &mut commands,
            Vec3::new(1.66, 1.15, z),
            Vec3::new(0.06, 2.3, 0.08),
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.09, 3.2))),
        MeshMaterial3d(skin.plate_shade.clone()),
        Transform::from_xyz(-1.35, 2.18, 0.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.05, 3.2))),
        MeshMaterial3d(skin.rivet.clone()),
        Transform::from_xyz(1.42, 2.2, 0.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // --- Panels: each SimSurface entity carries its station tag; a PLATE
    // slab sits just behind each mapped quad as the physical panel.
    for (station, surface) in panels() {
        let n = surface.normal();
        let size = Vec3::new(
            surface.half_u.length().mul_add(2.0, 0.06),
            surface.half_v.length().mul_add(2.0, 0.06),
            0.05,
        );
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.plate.clone()),
            Transform::from_translation(surface.center - n * 0.028)
                .with_rotation(surface.orientation())
                .with_scale(size),
        ));
        commands.spawn((station, surface));
    }

    // Desk mass under the two lower panels, and a console pedestal.
    wall(
        &mut commands,
        Vec3::new(-0.62, 0.42, -0.98),
        Vec3::new(0.95, 0.84, 0.5),
    );
    wall(
        &mut commands,
        Vec3::new(0.44, 0.42, -0.98),
        Vec3::new(1.25, 0.84, 0.5),
    );

    // --- Light: one warm overhead, one floor fill, one phosphor spill
    // by the tank. The omen reaches all three through `Dimmable`.
    commands.spawn((
        PointLight {
            color: palette::GLINT,
            intensity: 220_000.0,
            range: 7.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.25, 2.1, 0.35),
        Dimmable {
            intensity: 220_000.0,
        },
    ));
    commands.spawn((
        PointLight {
            color: palette::PLATE_LIT,
            intensity: 40_000.0,
            range: 5.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.4, 0.4, 0.9),
        Dimmable {
            intensity: 40_000.0,
        },
    ));
    commands.spawn((
        PointLight {
            color: palette::PHOSPHOR,
            intensity: 12_000.0,
            range: 2.4,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.56, 1.5, -1.05),
        Dimmable {
            intensity: 12_000.0,
        },
    ));

    commands.insert_resource(skin);
}

/// Lean the seat's gaze gently toward the cursor. The cursor stays free
/// (no pointer lock); the head follows attention, eased, within a small
/// envelope — cramped on purpose.
#[allow(clippy::needless_pass_by_value)]
pub fn glance(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera: Single<(&mut Transform, &CabinCamera)>,
) {
    let (transform, rig) = &mut *camera;
    let target = window.cursor_position().map_or(rig.base, |cursor| {
        let size = window.size();
        let u = (cursor.x / size.x - 0.5) * 2.0;
        let v = (cursor.y / size.y - 0.5) * 2.0;
        rig.base * Quat::from_euler(EulerRot::YXZ, -u * GLANCE_YAW, -v * GLANCE_PITCH, 0.0)
    });
    let rate = (time.delta_secs() * GLANCE_RATE).min(1.0);
    transform.rotation = transform.rotation.slerp(target, rate);
}
