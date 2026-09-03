//! **The console face, retired — and kept on the shelf.**
//!
//! Nothing in this module is bolted to the ship any more. The face's
//! readings walked off first (the preview CRT, the ETA gauge, the launch
//! handle became cargo — `Kind::DestPreview`, `Kind::EtaGauge`,
//! `Kind::LaunchLever` in `pieces`), and this pass took the rest: the
//! pause/warp/speaker plate and the hangar tally strip were the last
//! fixed UI screwed to a wall, and a wall panel that only carries
//! meta-controls is furniture pretending to be a station. The controls
//! themselves moved to the `Esc` menu (`crate::menu`), where the sim
//! stays the authority and the room stays diegetic.
//!
//! What is left here is **recipes**, not furniture: the icon geometry,
//! the lamp feel, and the hangar strip's delivery blink, each written
//! against a plain [`SimSurface`] so any face can wear them. Every one
//! of them **wants to come back as a cargo kind** — a toggle block you
//! bolt to a wall, a tally plaque the Guild ships you when the hangar
//! starts counting — and the whole point of leaving them compiled is
//! that the day somebody appends that `Kind`, the hardware is already
//! drawn and already breathes. Deleting them would have thrown away the
//! tactile half of the work and kept only the boring half.
//!
//! Semantics still mirror what the 2D console's `draw_console` family
//! did: the sim is the only authority, and this module only ever read
//! it back onto metal, glass, and phosphor.

// **The dormancy allow, argued.** Everything below is unreferenced on
// purpose — see the module docs. Compiled-but-unused beats a comment
// block full of code: the borrow checker keeps the recipes honest, a
// palette or `glow` rename breaks the build here instead of rotting
// quietly, and resurrection is a call site rather than an archaeology
// dig. The lint would otherwise flag the whole file.
#![allow(dead_code)]

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use space_trucking::sim::{Cue, Vec2 as SimVec2, layout};

use crate::rig::Skin;
use crate::surface::{SimSurface, VirtualPointer};
use crate::{Shell, glow, palette};

// ---- Feedback lengths (all inside the half-second law) ----

/// A hangar lamp blinking awake after a delivery crosses its threshold.
const WAKE_LEN: f32 = 0.40;

/// Lifetime-delivery thresholds for the hangar plate's six lamps. The
/// ladder itself did NOT retire with the plate — `crate::menu` counts
/// the same rungs, because the ladder is the reading and the plate was
/// only ever one way to wear it.
pub const DELIVERY_LAMPS: [u32; 6] = [1, 2, 4, 8, 16, 32];

// ---- Sim-unit furniture numbers, mirrored from the 2D console ----

/// The hangar tally plaque and its recessed glass strip (display-only, so
/// like the 2D renderer its rects are furniture, not `layout`'s business).
const HANGAR_PLATE: (f32, f32, f32, f32) = (682.0, 380.0, 100.0, 40.0);
const HANGAR_WELL: (f32, f32, f32, f32) = (690.0, 392.0, 84.0, 16.0);
const HANGAR_LAMP_X0: f32 = 699.0;
const HANGAR_LAMP_STEP: f32 = 13.2;

// ---- Markers, timers ----

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

/// The hangar lamp mid-blink after a threshold crossing.
#[derive(Resource, Default)]
struct HangarWake {
    lamp: Option<usize>,
    left: f32,
}

// ------------------------------------------------------------------ spawn --

/// Everything a spawn helper needs to bolt furniture onto a face.
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

/// **The resurrection entry point.** Bolt the whole toggle-and-tally
/// face onto any surface — hand it the face a piece's rig is drawing on
/// and the hardware grows there, exactly as it once grew on the wall.
///
/// Wants to come back as a cargo kind: a bolt-on toggle block, or the
/// Guild's tally plaque. Nothing in here knows about `Station::Console`
/// any more, which is the whole reason it survived the sweep.
fn bolt_on(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    panel: SimSurface,
    dev: bool,
) {
    let puck = meshes.add(Cylinder::new(1.0, 1.0));
    let mut build = Build {
        commands,
        meshes,
        materials,
        skin,
        panel,
        puck,
    };
    spawn_buttons(&mut build, dev);
    spawn_hangar(&mut build);
}

/// The three toggle buttons: raised plate caps, icons built from strokes,
/// a state lamp under each. Warp is dev-only furniture, same as 2D — and
/// the `Esc` menu inherited exactly that gate.
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
    let bars = glow::etched(b.materials, palette::ICON);
    for dx in [-4.5, 4.5] {
        let bar = b.slab((mid.0 + dx, mid.1), (5.0, 18.0), 0.004, 0.012, &bars);
        b.commands.entity(bar).insert((Toggle::Pause, IconStroke));
    }
}

/// Warp: a double chevron pointing right. Dev-only.
fn spawn_warp(b: &mut Build<'_, '_, '_>) {
    let mid = spawn_button_base(b, layout::WARP_BTN, Toggle::Warp);
    let strokes = glow::etched(b.materials, palette::ICON);
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
    let icon = glow::etched(b.materials, palette::ICON_LIT);
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

/// Repaint an icon stroke: etched metal while the function sleeps, the
/// live color (optionally emissive) while it is awake.
fn set_stroke(mat: &mut StandardMaterial, live: Color, lit: bool, glow_up: f32) {
    mat.base_color = if lit { live } else { palette::ICON };
    // Asleep, the stroke keeps the etched self-glow floor — the icons
    // must survive a lampless cabin (glow::etched's contract).
    mat.emissive = if lit {
        live.to_linear() * glow_up
    } else {
        palette::ICON.to_linear() * 0.35
    };
}

/// **The button feel.** Icon strokes, state lamps, and the mute slash,
/// read off the sim every frame: a live function lights its own icon, a
/// sleeping one keeps only the etched floor, and a hovered lamp wakes
/// faintly — interactable, not active. That last touch is the tell that
/// makes a metal button feel like hardware, and it is the reason this
/// system is on the shelf instead of in the bin.
fn buttons(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    icons: Query<(&Toggle, &MeshMaterial3d<StandardMaterial>), With<IconStroke>>,
    lamps: Query<(&Toggle, &MeshMaterial3d<StandardMaterial>), With<ButtonLamp>>,
    mut slashes: Query<&mut Visibility, With<MuteSlash>>,
) {
    let sim = &shell.bridge.sim;
    let paused = sim.is_paused();
    let warping = sim.is_warp();
    let muted = shell.muted;
    let hover = |which: &Toggle| {
        match which {
            Toggle::Pause => layout::PAUSE_BTN,
            Toggle::Warp => layout::WARP_BTN,
            Toggle::Speaker => layout::SPEAKER,
        }
        .contains(pointer.sim)
    };
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
        let (color, level): (Color, f32) = match which {
            Toggle::Pause => (palette::AMBER, if paused { 1.0 } else { 0.0 }),
            Toggle::Warp => (palette::AMBER, if warping { 1.0 } else { 0.0 }),
            Toggle::Speaker => (palette::LAMP_OK, if muted { 0.0 } else { 0.45 }),
        };
        // Hover wakes a sleeping lamp faintly — interactable, not active.
        let level = if hover(which) { level.max(0.18) } else { level };
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

/// **The delivery blink.** Each tally lamp shimmers slightly out of
/// phase once its threshold is passed; the lamp whose threshold a
/// delivery just crossed blinks awake — a couple of stutters easing into
/// the steady shimmer, all inside the half-second law.
///
/// Wants to come back as a cargo kind: the Guild's tally plaque, hung
/// wherever you like, counting the same ladder. The `Esc` menu carries
/// the *reading* now ([`DELIVERY_LAMPS`]) but deliberately not this —
/// a menu that stutters at you is a menu arguing, and the blink belongs
/// to hardware in a room.
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
