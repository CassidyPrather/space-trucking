//! The console face: the 2D console's right-hand panel made physical.
//! One `SimSurface` (`Station::Console`, sim rect `layout::CONSOLE`)
//! carries five instruments — the destination preview CRT, the ETA gauge,
//! the launch lever, the pause/warp/speaker toggle buttons, and the hangar
//! tally plate. Semantics mirror `src/render.rs`'s `draw_console` family:
//! the sim stays the only authority, this module only reads it back onto
//! metal, glass, and phosphor.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use space_trucking::sim::{Cue, Loc, ShipState, Vec2 as SimVec2, layout};

use crate::rig::Skin;
use crate::surface::{SimSurface, Station, VirtualPointer};
use crate::{Phase, Shell, glow, palette};

// ---- Feedback lengths (all inside the half-second law) ----

/// Launch-lever thunk travel time after a departure.
const THUNK_LEN: f32 = 0.35;

/// Launch-lever rattle after a refused pull.
const SHAKE_LEN: f32 = 0.30;

/// A hangar lamp blinking awake after a delivery crosses its threshold.
const WAKE_LEN: f32 = 0.40;

/// Lifetime-delivery thresholds for the hangar plate's six lamps, matching
/// the 2D console's `DELIVERY_LAMPS` ladder.
const DELIVERY_LAMPS: [u32; 6] = [1, 2, 4, 8, 16, 32];

// ---- Sim-unit furniture numbers, mirrored from `src/render.rs` ----

/// The hangar tally plaque and its recessed glass strip (display-only, so
/// like the 2D renderer its rects are furniture, not `layout`'s business).
const HANGAR_PLATE: (f32, f32, f32, f32) = (682.0, 380.0, 100.0, 40.0);
const HANGAR_WELL: (f32, f32, f32, f32) = (690.0, 392.0, 84.0, 16.0);
const HANGAR_LAMP_X0: f32 = 699.0;
const HANGAR_LAMP_STEP: f32 = 13.2;

/// How far off the panel plane the lever handle's centre rides, metres.
const HANDLE_LIFT: f32 = 0.014;

/// How far off the panel the ETA arc gizmo draws, metres.
const ARC_LIFT: f32 = 0.012;

// ---- Markers, timers ----

/// The console's `SimSurface`, copied at spawn so view systems can map sim
/// coordinates back onto the cabin without re-querying.
#[derive(Resource, Clone, Copy)]
struct ConsolePanel(SimSurface);

/// The brass launch handle (its children: go-lamp and glow halo).
#[derive(Component)]
struct LeverHandle;

/// The go-lamp riding on the handle.
#[derive(Component)]
struct LeverLamp;

/// The soft glow plate peeking around the handle while a pull would work.
#[derive(Component)]
struct LeverHalo;

/// Which toggle button an icon stroke or lamp belongs to.
#[derive(Component, Clone, Copy, Debug)]
enum Toggle {
    Pause,
    Warp,
    Speaker,
}

/// Marks an icon stroke whose material repaints with the toggle's state.
#[derive(Component)]
struct IconStroke;

/// Marks the state lamp under a toggle button.
#[derive(Component)]
struct ButtonLamp;

/// The diagonal refusal bar over the speaker icon — mute carries shape,
/// never hue alone.
#[derive(Component)]
struct MuteSlash;

/// One of the six hangar tally lamps, by ladder index.
#[derive(Component)]
struct HangarLamp(usize);

/// The launch lever's two feedback clocks, wound by cues.
#[derive(Resource, Default)]
struct LeverJuice {
    thunk: f32,
    shake: f32,
}

/// The hangar lamp mid-blink after a threshold crossing.
#[derive(Resource, Default)]
struct HangarWake {
    lamp: Option<usize>,
    left: f32,
}

/// The console face's plugin: spawn the furniture after the rig stands,
/// then read the sim back onto it every view frame.
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LeverJuice>()
            .init_resource::<HangarWake>()
            .add_systems(PostStartup, spawn)
            .add_systems(
                Update,
                (eta, lever_motion, lever_lamp, buttons, hangar).in_set(Phase::View),
            );
    }
}

// ------------------------------------------------------------------ spawn --

/// Everything a spawn helper needs to bolt furniture onto the panel.
struct Build<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    skin: &'a Skin,
    panel: SimSurface,
    /// A unit cylinder, shared by every lamp cap and disc.
    puck: Handle<Mesh>,
}

impl Build<'_, '_, '_> {
    /// World position `lift` metres off the panel plane at sim `(x, y)`.
    fn at(&self, x: f32, y: f32, lift: f32) -> Vec3 {
        self.panel.to_world(SimVec2::new(x, y)) + self.panel.normal() * lift
    }

    /// An axis-aligned box on the panel: centre and size in sim units,
    /// `depth` metres thick, its centre `lift` metres off the plane.
    fn slab(
        &mut self,
        (x, y): (f32, f32),
        (w, h): (f32, f32),
        depth: f32,
        lift: f32,
        material: &Handle<StandardMaterial>,
    ) -> Entity {
        self.commands
            .spawn((
                Mesh3d(self.skin.cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(self.at(x, y, lift))
                    .with_rotation(self.panel.orientation())
                    .with_scale(Vec3::new(
                        w * self.panel.scale_u(),
                        h * self.panel.scale_v(),
                        depth,
                    )),
            ))
            .id()
    }

    /// A thin stroke: length along its own axis, rotated `angle` radians
    /// counterclockwise in the panel plane (seen from the seat), centred
    /// at `anchor` plus an up-frame offset — all in sim units.
    fn stroke(
        &mut self,
        anchor: (f32, f32),
        off: (f32, f32),
        (len, wide): (f32, f32),
        angle: f32,
        lift: f32,
        material: &Handle<StandardMaterial>,
    ) -> Entity {
        // Up-frame +y is panel-up, which is sim -y.
        let (x, y) = (anchor.0 + off.0, anchor.1 - off.1);
        self.commands
            .spawn((
                Mesh3d(self.skin.cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(self.at(x, y, lift))
                    .with_rotation(self.panel.orientation() * Quat::from_rotation_z(angle))
                    .with_scale(Vec3::new(
                        len * self.panel.scale_u(),
                        wide * self.panel.scale_v(),
                        0.004,
                    )),
            ))
            .id()
    }

    /// A flat disc facing out of the panel: radius in sim units.
    fn disc(
        &mut self,
        (x, y): (f32, f32),
        radius: f32,
        depth: f32,
        lift: f32,
        material: &Handle<StandardMaterial>,
    ) -> Entity {
        self.commands
            .spawn((
                Mesh3d(self.puck.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(self.at(x, y, lift))
                    .with_rotation(self.panel.orientation() * Quat::from_rotation_x(FRAC_PI_2))
                    .with_scale(Vec3::new(
                        radius * self.panel.scale_u(),
                        depth,
                        radius * self.panel.scale_v(),
                    )),
            ))
            .id()
    }
}

/// Bolt the whole console face onto the `Station::Console` surface.
fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
    shell: Res<Shell>,
    surfaces: Query<(&Station, &SimSurface)>,
) {
    let Some(panel) = surfaces
        .iter()
        .find_map(|(station, surface)| (*station == Station::Console).then_some(*surface))
    else {
        return;
    };
    commands.insert_resource(ConsolePanel(panel));
    let puck = meshes.add(Cylinder::new(1.0, 1.0));
    let mut build = Build {
        commands: &mut commands,
        meshes: &mut meshes,
        materials: &mut materials,
        skin: &skin,
        panel,
        puck,
    };
    spawn_eta(&mut build);
    spawn_lever(&mut build);
    spawn_buttons(&mut build, shell.bridge.dev());
    spawn_hangar(&mut build);
}

// The destination preview surface itself is painted by `crt` — the
// faithful software-rasterized port of the 2D tube. The console keeps
// only the instruments around it.

/// The ETA gauge: a physical metal instrument — plate housing, dark glass
/// face, eight GLINT ticks, an amber standby ember. The draining arc
/// itself is a per-frame gizmo in [`eta`].
fn spawn_eta(b: &mut Build<'_, '_, '_>) {
    let mid = layout::ETA_ARC_CENTER;
    let plate = b.skin.plate.clone();
    b.disc((mid.x, mid.y), 32.0, 0.006, 0.003, &plate);
    let glass = b.skin.glass.clone();
    b.disc(
        (mid.x, mid.y),
        layout::ETA_ARC_RADIUS + 3.0,
        0.004,
        0.007,
        &glass,
    );

    // Ticks straddle the arc track; twelve o'clock, where the arc starts
    // and ends, reads brightest — same hierarchy as the 2D gauge.
    let tick = glow::enamel(
        b.materials,
        palette::mix(palette::GLINT, palette::PLATE, 0.45),
    );
    let tick_hot = glow::enamel(b.materials, palette::GLINT);
    for i in 0..8_u8 {
        let a = f32::from(i) * (TAU / 8.0);
        let off = (24.0 * a.sin(), 24.0 * a.cos());
        let material = if i == 0 { &tick_hot } else { &tick };
        b.stroke(
            (mid.x, mid.y),
            off,
            (5.0, 1.5),
            FRAC_PI_2 - a,
            0.010,
            material,
        );
    }

    // The standby ember: always faintly alive, so the gauge never reads
    // as a hole in the panel. Static, but still its own instance.
    let ember = glow::phosphor(b.materials, palette::AMBER, 1.3);
    b.disc((mid.x, mid.y), 2.0, 0.003, 0.010, &ember);
}

/// The launch lever: a SOCKET track slot and a brass handle resting left,
/// carrying its own go-lamp and glow halo so feedback rides along.
fn spawn_lever(b: &mut Build<'_, '_, '_>) {
    let r = layout::LAUNCH_LEVER;
    let mid_y = r.h.mul_add(0.5, r.y);
    let socket = b.skin.socket.clone();
    b.slab(
        (r.w.mul_add(0.5, r.x), mid_y),
        (r.w - 32.0, 8.0),
        0.006,
        0.003,
        &socket,
    );

    let su = b.panel.scale_u();
    let sv = b.panel.scale_v();
    let handle_mesh = b.meshes.add(Cuboid::new(18.0 * su, 44.0 * sv, 0.026));
    let halo_mesh = b.meshes.add(Cuboid::new(26.0 * su, 52.0 * sv, 0.002));
    let halo_mat = glow::phosphor(b.materials, palette::LAMP_OK, 0.0);
    let lamp_mat = glow::phosphor(b.materials, palette::LAMP_OK, 0.0);
    let puck = b.puck.clone();
    b.commands
        .spawn((
            Mesh3d(handle_mesh),
            MeshMaterial3d(b.skin.brass.clone()),
            Transform::from_translation(b.at(r.x + 34.0, mid_y, HANDLE_LIFT))
                .with_rotation(b.panel.orientation()),
            LeverHandle,
        ))
        .with_children(|kids| {
            kids.spawn((
                Mesh3d(halo_mesh),
                MeshMaterial3d(halo_mat),
                Transform::from_xyz(0.0, 0.0, -0.006),
                LeverHalo,
            ));
            kids.spawn((
                Mesh3d(puck),
                MeshMaterial3d(lamp_mat),
                Transform::from_xyz(0.0, 13.0 * sv, 0.015)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                    .with_scale(Vec3::new(4.4 * su, 0.003, 4.4 * sv)),
                LeverLamp,
            ));
        });
}

/// The three toggle buttons: raised plate caps, icons built from strokes,
/// a state lamp under each. Warp is dev-only furniture, same as 2D.
fn spawn_buttons(b: &mut Build<'_, '_, '_>, dev: bool) {
    spawn_pause(b);
    if dev {
        spawn_warp(b);
    }
    spawn_speaker(b);
}

/// A button's shared base: the raised cap and the lamp near its lower
/// edge. Returns the icon anchor (sim coordinates, nudged up to clear the
/// lamp, mirroring the 2D `icon_center`).
fn spawn_button_base(b: &mut Build<'_, '_, '_>, rect: layout::Rect, which: Toggle) -> (f32, f32) {
    let cx = rect.w.mul_add(0.5, rect.x);
    let cy = rect.h.mul_add(0.5, rect.y);
    let plate = b.skin.plate.clone();
    b.slab((cx, cy), (36.0, 36.0), 0.010, 0.005, &plate);
    let lamp_mat = glow::phosphor(b.materials, palette::AMBER, 0.0);
    let lamp = b.disc((cx, rect.y + rect.h - 9.0), 4.0, 0.003, 0.012, &lamp_mat);
    b.commands.entity(lamp).insert((which, ButtonLamp));
    (cx, cy - 5.0)
}

/// Pause: two upright bars.
fn spawn_pause(b: &mut Build<'_, '_, '_>) {
    let mid = spawn_button_base(b, layout::PAUSE_BTN, Toggle::Pause);
    let bars = glow::enamel(b.materials, palette::ICON);
    for dx in [-4.5, 4.5] {
        let bar = b.slab((mid.0 + dx, mid.1), (5.0, 18.0), 0.004, 0.012, &bars);
        b.commands.entity(bar).insert((Toggle::Pause, IconStroke));
    }
}

/// Warp: a double chevron pointing right. Dev-only.
fn spawn_warp(b: &mut Build<'_, '_, '_>) {
    let mid = spawn_button_base(b, layout::WARP_BTN, Toggle::Warp);
    let strokes = glow::enamel(b.materials, palette::ICON);
    for chevron in [-5.25, 3.85] {
        for (dy, angle) in [(3.5, -0.838), (-3.5, 0.838)] {
            let arm = b.stroke(mid, (chevron + 0.7, dy), (9.4, 2.2), angle, 0.012, &strokes);
            b.commands.entity(arm).insert((Toggle::Warp, IconStroke));
        }
    }
}

/// Speaker: a box body and a flattened horn wedge, plus the hidden mute
/// slash that carries the refusal by shape.
fn spawn_speaker(b: &mut Build<'_, '_, '_>) {
    let mid = spawn_button_base(b, layout::SPEAKER, Toggle::Speaker);
    let icon = glow::enamel(b.materials, palette::ICON_LIT);
    let body = b.slab((mid.0 - 6.5, mid.1), (7.0, 10.0), 0.004, 0.012, &icon);
    b.commands
        .entity(body)
        .insert((Toggle::Speaker, IconStroke));

    let horn_mesh = b.meshes.add(Cone {
        radius: 1.0,
        height: 1.0,
    });
    let su = b.panel.scale_u();
    b.commands.spawn((
        Mesh3d(horn_mesh),
        MeshMaterial3d(icon),
        Transform::from_translation(b.at(mid.0 + 1.5, mid.1, 0.012))
            .with_rotation(b.panel.orientation() * Quat::from_rotation_z(FRAC_PI_2))
            .with_scale(Vec3::new(9.0 * su, 11.0 * su, 0.004)),
        Toggle::Speaker,
        IconStroke,
    ));

    let slash_mat = glow::phosphor(b.materials, palette::LAMP_NO, 2.2);
    let slash = b.stroke(mid, (0.0, 0.0), (32.6, 3.0), 0.742, 0.017, &slash_mat);
    b.commands
        .entity(slash)
        .insert((MuteSlash, Visibility::Hidden));
}

/// The hangar tally plate: a plaque, a recessed well strip, and six
/// violet lamps — hangar business, not status green.
fn spawn_hangar(b: &mut Build<'_, '_, '_>) {
    let (px, py, pw, ph) = HANGAR_PLATE;
    let plate = b.skin.plate.clone();
    b.slab(
        (pw.mul_add(0.5, px), ph.mul_add(0.5, py)),
        (pw, ph),
        0.008,
        0.004,
        &plate,
    );
    let (wx, wy, ww, wh) = HANGAR_WELL;
    let socket = b.skin.socket.clone();
    let well_mid = wh.mul_add(0.5, wy);
    b.slab(
        (ww.mul_add(0.5, wx), well_mid),
        (ww, wh),
        0.003,
        0.009,
        &socket,
    );
    for i in 0..DELIVERY_LAMPS.len() {
        let x = (i as f32).mul_add(HANGAR_LAMP_STEP, HANGAR_LAMP_X0);
        let mat = glow::phosphor(b.materials, palette::EERIE, 0.0);
        let lamp = b.disc((x, well_mid), 4.0, 0.003, 0.012, &mat);
        b.commands.entity(lamp).insert(HangarLamp(i));
    }
}

// ------------------------------------------------------------------- view --

/// The ETA arc: amber, from twelve o'clock, draining clockwise as the leg
/// completes; a full dim ring while docked with a destination armed.
fn eta(shell: Res<Shell>, panel: Option<Res<ConsolePanel>>, mut gizmos: Gizmos) {
    let Some(panel) = panel else {
        return;
    };
    let sim = &shell.bridge.sim;
    let surface = panel.0;
    let mid = layout::ETA_ARC_CENTER;
    let center = surface.to_world(mid) + surface.normal() * ARC_LIFT;
    let radius = (layout::ETA_ARC_RADIUS - 3.0) * surface.scale_u();
    match sim.ship().state {
        ShipState::Traveling {
            progress,
            leg_ticks,
            ..
        } => {
            let frac = ((progress as f32 + sim.alpha()) / leg_ticks as f32).clamp(0.0, 1.0);
            let sweep = (1.0 - frac) * TAU;
            if sweep > 0.01 {
                // Local X starts the arc at panel-up (twelve o'clock);
                // local Y is the plane normal; negative angle sweeps
                // clockwise as seen from the seat.
                let rot = surface.orientation()
                    * Quat::from_mat3(&Mat3::from_cols(Vec3::Y, Vec3::Z, Vec3::X));
                gizmos.arc_3d(-sweep, radius, Isometry3d::new(center, rot), palette::AMBER);
            }
        }
        ShipState::Docked(_) if sim.ship().selected.is_some() => {
            gizmos.circle(
                Isometry3d::new(center, surface.orientation()),
                radius,
                palette::AMBER.with_alpha(0.35),
            );
        }
        ShipState::Docked(_) => {}
    }
}

/// The launch handle's ride: the thunk throw on departure, the rattle on
/// a refused pull, rest at the track's left end otherwise.
fn lever_motion(
    time: Res<Time>,
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    panel: Option<Res<ConsolePanel>>,
    grips: Res<crate::gesture::Grips>,
    mut juice: ResMut<LeverJuice>,
    mut handle: Single<&mut Transform, With<LeverHandle>>,
) {
    let Some(panel) = panel else {
        return;
    };
    let sim = &shell.bridge.sim;
    let dt = time.delta_secs();
    juice.thunk = (juice.thunk - dt).max(0.0);
    juice.shake = (juice.shake - dt).max(0.0);
    for cue in sim.cues() {
        match cue {
            Cue::Depart => juice.thunk = THUNK_LEN,
            Cue::Reject { hard: false } if layout::LAUNCH_LEVER.contains(pointer.sim) => {
                juice.shake = SHAKE_LEN;
            }
            _ => {}
        }
    }

    // The thunk throws the handle right fast, then eases it home; the
    // rattle jitters it around its rest. Same envelope as the 2D lever.
    // A live pull gesture overrides both: the handle is in the hand.
    let heat = juice.thunk / THUNK_LEN;
    let pull = if heat > 0.65 {
        (1.0 - heat) / 0.35
    } else {
        heat / 0.65
    }
    .max(grips.launch.travel);
    let shake = (time.elapsed_secs() * 70.0).sin() * 3.0 * (juice.shake / SHAKE_LEN);
    let rect = layout::LAUNCH_LEVER;
    let x = pull.mul_add(rect.w - 68.0, rect.x + 34.0) + shake;
    let mid_y = rect.h.mul_add(0.5, rect.y);
    let surface = panel.0;
    handle.translation = surface.to_world(SimVec2::new(x, mid_y)) + surface.normal() * HANDLE_LIFT;
}

/// The go-lamp and its halo: lit and breathing while a pull would depart,
/// dark glass while the sim would refuse one.
fn lever_lamp(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lamp: Single<&MeshMaterial3d<StandardMaterial>, With<LeverLamp>>,
    halo: Single<&MeshMaterial3d<StandardMaterial>, With<LeverHalo>>,
) {
    let sim = &shell.bridge.sim;
    let ship = sim.ship();
    let pullable = matches!(ship.state, ShipState::Docked(_))
        && ship.selected.is_some()
        && !sim.pieces().iter().any(|piece| {
            matches!(
                piece.loc,
                Loc::GivePad { .. } | Loc::TakePad { .. } | Loc::ReceivedShelf { .. }
            )
        });
    // Decoration: the go-glow breathes gently while a pull would work.
    let breath = glow::breathe(time.elapsed_secs(), 2.2, 0.0).mul_add(0.24, 0.66);
    if let Some(mut mat) = materials.get_mut(&lamp.0) {
        glow::set_lamp(
            &mut mat,
            palette::LAMP_OK,
            if pullable { breath } else { 0.0 },
        );
    }
    if let Some(mut mat) = materials.get_mut(&halo.0) {
        let strength = if pullable { breath * 0.55 } else { 0.0 };
        mat.emissive = palette::LAMP_OK.to_linear() * strength;
    }
}

/// Repaint an icon stroke: etched metal while the function sleeps, the
/// live color (optionally emissive) while it is awake.
fn set_stroke(mat: &mut StandardMaterial, live: Color, lit: bool, glow_up: f32) {
    mat.base_color = if lit { live } else { palette::ICON };
    mat.emissive = live.to_linear() * if lit { glow_up } else { 0.0 };
}

/// The toggle buttons: icon strokes, state lamps, and the mute slash.
fn buttons(
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    icons: Query<(&Toggle, &MeshMaterial3d<StandardMaterial>), With<IconStroke>>,
    lamps: Query<(&Toggle, &MeshMaterial3d<StandardMaterial>), With<ButtonLamp>>,
    mut slashes: Query<&mut Visibility, With<MuteSlash>>,
) {
    let sim = &shell.bridge.sim;
    let paused = sim.is_paused();
    let warping = sim.is_warp();
    let muted = shell.muted;
    for (which, material) in &icons {
        let Some(mut mat) = materials.get_mut(&material.0) else {
            continue;
        };
        match which {
            Toggle::Pause => set_stroke(&mut mat, palette::AMBER, paused, 1.6),
            Toggle::Warp => set_stroke(&mut mat, palette::AMBER, warping, 1.6),
            // The speaker icon is never lamp-hot: live is merely brighter
            // etch; the lamp below carries the state.
            Toggle::Speaker => set_stroke(&mut mat, palette::ICON_LIT, !muted, 0.0),
        }
    }
    for (which, material) in &lamps {
        let Some(mut mat) = materials.get_mut(&material.0) else {
            continue;
        };
        let (color, level) = match which {
            Toggle::Pause => (palette::AMBER, if paused { 1.0 } else { 0.0 }),
            Toggle::Warp => (palette::AMBER, if warping { 1.0 } else { 0.0 }),
            Toggle::Speaker => (palette::LAMP_OK, if muted { 0.0 } else { 0.45 }),
        };
        glow::set_lamp(&mut mat, color, level);
    }
    for mut visibility in &mut slashes {
        *visibility = if muted {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// The hangar tally: each lamp shimmers slightly out of phase once its
/// threshold is passed; the lamp whose threshold a delivery just crossed
/// blinks awake — a couple of stutters easing into the steady shimmer.
fn hangar(
    time: Res<Time>,
    shell: Res<Shell>,
    mut wake: ResMut<HangarWake>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lamps: Query<(&HangarLamp, &MeshMaterial3d<StandardMaterial>)>,
) {
    let sim = &shell.bridge.sim;
    wake.left = (wake.left - time.delta_secs()).max(0.0);
    // The tally has already counted the crate when the cue fires, so a
    // lamp that just crossed its threshold matches the ladder exactly.
    if sim.cues().iter().any(|cue| matches!(cue, Cue::Delivered))
        && let Some(lamp) = DELIVERY_LAMPS
            .iter()
            .position(|&threshold| threshold == sim.deliveries())
    {
        wake.lamp = Some(lamp);
        wake.left = WAKE_LEN;
    }

    let t = time.elapsed_secs();
    let deliveries = sim.deliveries();
    for (HangarLamp(i), material) in &lamps {
        let Some(mut mat) = materials.get_mut(&material.0) else {
            continue;
        };
        let shimmer = glow::breathe(t, 1.1, (*i as f32) * 1.7).mul_add(0.12, 0.76);
        let level = if wake.left > 0.0 && wake.lamp == Some(*i) {
            let rise = 1.0 - wake.left / WAKE_LEN;
            let stutter = glow::breathe(rise, TAU * 2.5, 0.0);
            let eased = {
                let u = 1.0 - rise;
                u.mul_add(-u * u, 1.0)
            };
            shimmer * eased * (1.0 - stutter).mul_add(rise, stutter)
        } else if deliveries >= DELIVERY_LAMPS[*i] {
            shimmer
        } else {
            0.0
        };
        glow::set_lamp(&mut mat, palette::EERIE, level);
    }
}
