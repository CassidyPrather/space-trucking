//! The cabin itself: an enclosed box with flavor, per DESIGN.md's first
//! pass. Four panels in a cramped wraparound, echoing the 2D console's
//! layout so muscle memory transfers: star tank upper left, console upper
//! right, hold tray low left, barter counter low right.
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

use space_trucking::sim::layout;

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
/// clear of every slab by construction, asserted by test).
const EYE_HEIGHT: f32 = 1.5;
const WALK_MIN: Vec3 = Vec3::new(-1.30, EYE_HEIGHT, -0.30);
const WALK_MAX: Vec3 = Vec3::new(1.30, EYE_HEIGHT, 1.55);
const WALK_SPEED: f32 = 1.3;
const LOOK_SPEED: f32 = 0.0026;
const PITCH_LIMIT: f32 = 1.35;

/// Focus glide length, seconds. A camera move is feedback: it answers a
/// click and finishes fast.
const GLIDE: f32 = 0.38;

/// Margin factor when fitting a panel into the focused view.
const FIT_MARGIN: f32 = 1.14;

/// How far the physical panel plate extends past its mapped quad.
const PLATE_MARGIN: f32 = 0.03;

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
pub fn structure(panels: &[(Station, SimSurface); 4]) -> Vec<Slab> {
    let mut slabs = vec![
        // The box: floor, ceiling, four walls.
        Slab::new(
            Vec3::new(0.0, -0.05, 0.2),
            Vec3::new(3.4, 0.1, 3.4),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 2.32, 0.2),
            Vec3::new(3.4, 0.1, 3.4),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 1.15, -1.42),
            Vec3::new(3.4, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(0.0, 1.15, 1.92),
            Vec3::new(3.4, 2.5, 0.1),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(-1.72, 1.15, 0.2),
            Vec3::new(0.1, 2.5, 3.4),
            Finish::Hull,
        ),
        Slab::new(
            Vec3::new(1.72, 1.15, 0.2),
            Vec3::new(0.1, 2.5, 3.4),
            Finish::Hull,
        ),
    ];
    // Wall ribs: the junk that says somebody built this hull in a hurry.
    for i in 0..5 {
        let z = 0.7f32.mul_add(i as f32, -1.2);
        slabs.push(Slab::new(
            Vec3::new(-1.66, 1.15, z),
            Vec3::new(0.06, 2.3, 0.08),
            Finish::Hull,
        ));
        slabs.push(Slab::new(
            Vec3::new(1.66, 1.15, z),
            Vec3::new(0.06, 2.3, 0.08),
            Finish::Hull,
        ));
    }
    // Desk supports, derived: the slab's top stops just under the lowest
    // corner of the panel's *plate* (quad + margin), its front face just
    // shy of the plate's forward reach.
    for (station, surface) in panels {
        if !matches!(station, Station::Hold | Station::Barter) {
            continue;
        }
        let (lo, hi) = plate_bounds(surface);
        let top = lo.y - 0.008;
        let front = hi.z + 0.02;
        let back = -1.37;
        slabs.push(Slab::new(
            Vec3::new(surface.center.x, top * 0.5, f32::midpoint(front, back)),
            Vec3::new(hi.x - lo.x + 0.10, top, front - back),
            Finish::Plate,
        ));
    }
    slabs
}

/// World-space AABB of a panel's physical plate: the mapped quad grown by
/// the plate margin on both axes, plus its thickness behind the plane.
fn plate_bounds(surface: &SimSurface) -> (Vec3, Vec3) {
    let u = surface.half_u + surface.half_u.normalize() * PLATE_MARGIN;
    let v = surface.half_v + surface.half_v.normalize() * PLATE_MARGIN;
    let n = surface.normal();
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for su in [-1.0, 1.0] {
        for sv in [-1.0, 1.0] {
            for d in [0.0, -0.055] {
                let p = surface.center + u * su + v * sv + n * d;
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    (lo, hi)
}

// ---- The camera rig ----

/// A focused viewpoint. The two desk panels share one so cargo drags can
/// cross between hold and counter without leaving focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Tank,
    Console,
    Desk,
}

impl Focus {
    /// Which focus a station belongs to.
    #[must_use]
    pub const fn of(station: Station) -> Self {
        match station {
            Station::Map => Self::Tank,
            Station::Console => Self::Console,
            Station::Hold | Station::Barter => Self::Desk,
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
        }
    }

    /// Whether the sim should receive pointer interaction this frame.
    #[must_use]
    pub const fn interactive(&self) -> bool {
        matches!(self.mode, Mode::Focused { .. })
    }

    fn roam_rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0)
    }
}

/// The pose a focus parks at: panel extents fitted to the camera FOV,
/// eyed along the panel normal, up running up-panel.
#[must_use]
pub fn focus_pose(focus: Focus, panels: &[(Station, SimSurface); 4]) -> (Vec3, Quat) {
    let group: Vec<&SimSurface> = panels
        .iter()
        .filter(|(station, _)| Focus::of(*station) == focus)
        .map(|(_, s)| s)
        .collect();
    // Combined center and planar extents, measured in the first panel's
    // frame (desk panels share a tilt by construction).
    let lead = group[0];
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
    (eye, look.rotation)
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
pub struct CabinCamera;

/// Cabin lights the omen may dim, remembering their honest brightness.
#[derive(Component)]
pub struct Dimmable {
    pub intensity: f32,
}

/// The roaming crosshair dot (UI, hidden while focused).
#[derive(Component)]
pub struct Crosshair;

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
    let skin = Skin::build(&mut meshes, &mut materials);
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

    let (pos, rot) = match rig.mode {
        Mode::Focused { focus } => focus_pose(focus, &panels),
        _ => (rig.pos, rig.roam_rotation()),
    };
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
            Finish::Plate => skin.plate.clone(),
        };
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(slab.center).with_scale(slab.size),
        ));
    }
    // Ceiling pipes: oriented decor, outside the slab list on purpose.
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

        // The hold's 6×4 socket wells are structural furniture, not view
        // state — they live here with the rest of the metal.
        if matches!(station, Station::Hold) {
            for y in 0..layout::GRID_ROWS {
                for x in 0..layout::GRID_COLS {
                    let cell = layout::cell_rect(x, y);
                    let mid = space_trucking::sim::Vec2::new(
                        cell.w.mul_add(0.5, cell.x),
                        cell.h.mul_add(0.5, cell.y),
                    );
                    commands.spawn((
                        Mesh3d(skin.cube.clone()),
                        MeshMaterial3d(skin.socket.clone()),
                        Transform::from_translation(surface.to_world(mid) + n * 0.0015)
                            .with_rotation(surface.orientation())
                            .with_scale(Vec3::new(
                                (cell.w - 4.0) * surface.scale_u(),
                                (cell.h - 4.0) * surface.scale_v(),
                                0.003,
                            )),
                    ));
                }
            }
        }
    }

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

/// The station the roaming crosshair rests on, if any: a ray straight
/// out of the camera against the panel quads.
fn aimed_station(
    camera: &Transform,
    surfaces: &Query<(&Station, &SimSurface), Without<CabinCamera>>,
) -> Option<Station> {
    let ray = Ray3d::new(camera.translation, Dir3::new(camera.forward().into()).ok()?);
    let mut best: Option<(f32, Station)> = None;
    for (station, surface) in surfaces {
        if let Some((t, _, _)) = surface.project(ray)
            && best.is_none_or(|(bt, _)| t < bt)
        {
            best = Some((t, *station));
        }
    }
    best.map(|(_, station)| station)
}

/// Mode transitions and roaming movement, from this frame's input.
pub fn steer(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    surfaces: Query<(&Station, &SimSurface), Without<CabinCamera>>,
    mut rig: ResMut<CameraRig>,
    camera: Single<&Transform, With<CabinCamera>>,
) {
    let toggle = keys.just_pressed(KeyCode::KeyE);
    match rig.mode {
        Mode::Roam => {
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
            // Focus what the crosshair rests on.
            if (buttons.just_pressed(MouseButton::Left) || toggle)
                && let Some(station) = aimed_station(&camera, &surfaces)
            {
                rig.mode = Mode::ToFocus {
                    focus: Focus::of(station),
                    from: (camera.translation, camera.rotation),
                    t: 0.0,
                };
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
pub fn pose(
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
    mut camera: Single<&mut Transform, With<CabinCamera>>,
) {
    let panels = panels();
    let dt = time.delta_secs();
    let (roam_pos, roam_rot) = (rig.pos, rig.roam_rotation());
    let (pos, rot) = match &mut rig.mode {
        Mode::Roam => (roam_pos, roam_rot),
        Mode::Focused { focus } => focus_pose(*focus, &panels),
        Mode::ToFocus { focus, from, t } => {
            *t = (*t + dt / GLIDE).min(1.0);
            let s = smooth(*t);
            let (to_pos, to_rot) = focus_pose(*focus, &panels);
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
    if roaming && window.focused {
        // Windows' winit cannot Lock the cursor, only Confine it — and a
        // confined, hidden cursor still wanders (to the taskbar, where a
        // click steals the window), so it gets pinned to center every
        // frame instead. Look input reads raw deltas and never notices.
        if cfg!(target_os = "windows") {
            cursor.grab_mode = CursorGrabMode::Confined;
            let center = window.size() * 0.5;
            window.set_cursor_position(Some(center));
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
        aimed_station(&camera, &surfaces).map(Focus::of)
    } else {
        None
    };
    for (frame, mut visibility) in &mut frames {
        *visibility = if aimed == Some(Focus::of(frame.0)) {
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
        panels: &[(Station, SimSurface); 4],
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
            Station::Console => {
                spots.push(mid(layout::LAUNCH_LEVER));
                spots.push(mid(layout::PAUSE_BTN));
                spots.push(mid(layout::WARP_BTN));
                spots.push(mid(layout::SPEAKER));
                spots.push(mid(layout::DEST_PREVIEW));
                spots.push(layout::ETA_ARC_CENTER);
            }
            Station::Hold => {
                for (x, y) in [(0, 0), (5, 0), (0, 3), (5, 3), (2, 1)] {
                    spots.push(mid(layout::cell_rect(x, y)));
                }
            }
            Station::Barter => {
                spots.push(mid(layout::ACCEPT_LEVER));
                spots.push(layout::DIAL_CENTER);
                spots.push(mid(layout::ENCOUNTER_BADGE));
                for row in [
                    &layout::SHELF_SLOTS,
                    &layout::RECEIVED_SLOTS,
                    &layout::GIVE_SLOTS,
                    &layout::TAKE_SLOTS,
                ] {
                    spots.push(mid(row[0]));
                    spots.push(mid(row[3]));
                }
            }
        }
        spots.into_iter().map(|s| surface.to_world(s)).collect()
    }

    /// The sightline contract: from a station's own focus viewpoint,
    /// every panel corner and every control it carries must be visible —
    /// framed and unoccluded. This is the "corner must be visible from
    /// the perspective" rule, enforced at build time.
    #[test]
    fn every_control_is_visible_from_its_focus() {
        let panels = panels();
        let slabs = structure(&panels);
        for (station, surface) in &panels {
            let (eye, rot) = focus_pose(Focus::of(*station), &panels);
            let mut points = corner_points(surface);
            points.extend(control_points(*station, surface));
            for point in points {
                // Lift each point a hair off the face so the ray test
                // asks about the air in front of it, not the face itself.
                let probe = point + surface.normal() * 0.004;
                if let Err(reason) = visible_from(eye, rot, probe, &slabs, &panels) {
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
    /// inside no slab, looking at their panels.
    #[test]
    fn focus_poses_are_legal_camera_positions() {
        let panels = panels();
        let slabs = structure(&panels);
        for focus in [Focus::Tank, Focus::Console, Focus::Desk] {
            let (eye, rot) = focus_pose(focus, &panels);
            assert!(
                eye.y > 0.2 && eye.y < 2.2 && eye.x.abs() < 1.6 && eye.z > -1.3 && eye.z < 1.8,
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
            for (station, surface) in &panels {
                if Focus::of(*station) == focus {
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

    #[test]
    fn desk_supports_sit_under_their_panels() {
        let panels = panels();
        let slabs = structure(&panels);
        // Two derived supports exist and their tops sit below both desk
        // panels' lowest plate corners.
        let supports: Vec<&Slab> = slabs
            .iter()
            .filter(|s| matches!(s.finish, Finish::Plate))
            .collect();
        assert_eq!(supports.len(), 2);
        for (station, surface) in &panels {
            if matches!(station, Station::Hold | Station::Barter) {
                let (lo, _) = plate_bounds(surface);
                let support = supports
                    .iter()
                    .find(|s| (s.center.x - surface.center.x).abs() < 0.2)
                    .expect("a support under each desk panel");
                let top = support.size.y.mul_add(0.5, support.center.y);
                assert!(top < lo.y, "{station:?} support top {top} reaches {}", lo.y);
            }
        }
    }
}
