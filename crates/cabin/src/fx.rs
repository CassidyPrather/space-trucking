//! The omen, made physical. In 2D the whole palette multiplies toward
//! dark violet; in a real 3D room the honest translation is the lights
//! themselves: every [`Dimmable`] cabin light follows `sim.light()`, an
//! eerie violet source swells with `sim.omen()`, and the jump lands as a
//! bright violet flash that decays inside half a second. Screens keep
//! their own glow in the dark — phosphor does not care about the room,
//! which is exactly why a dimmed cabin feels wrong in the right way.

use bevy::prelude::*;

use space_trucking::sim::Cue;

use crate::palette;
use crate::rig::Dimmable;
use crate::{Phase, Shell};

/// Peak intensity of the omen's violet source at full swell.
const OMEN_LUMENS: f32 = 90_000.0;

/// Peak intensity of the jump flash, gone in [`JUMP_LEN`] seconds.
const JUMP_LUMENS: f32 = 900_000.0;
const JUMP_LEN: f32 = 0.45;

/// The eerie light that answers the omen, dark until the hum swells.
#[derive(Component)]
pub struct OmenLight;

/// The jump flash timer, wound by `Cue::Jump`.
#[derive(Resource, Default)]
pub struct JumpFlash {
    left: f32,
}

/// One drifting dust mote: hull air made visible where the light pools.
#[derive(Component)]
struct Mote {
    base: Vec3,
    phase: f32,
}

/// The one material every mote shares — the air changes mood as one.
#[derive(Resource)]
struct MoteMat(Handle<StandardMaterial>);

/// Motes per light pool; a few more wander free.
const MOTES: u64 = 44;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JumpFlash>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (dim_cabin, omen_swell, drift_motes).in_set(Phase::View),
            );
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PointLight {
            color: palette::EERIE,
            intensity: 0.0,
            range: 6.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.7, 0.1),
        OmenLight,
    ));

    // The particulate: tiny catch-the-light flecks clustered under the
    // cabin's light pools (denser air where light lives), a few adrift.
    // Deterministic like all decoration — the same air every boot.
    let fleck = meshes.add(Cuboid::new(0.006, 0.006, 0.006));
    let mat = materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: palette::GLINT.to_linear() * 0.6,
        perceptual_roughness: 1.0,
        ..default()
    });
    let pools = [
        (Vec3::new(0.25, 1.6, 0.35), 0.8), // under the warm overhead
        (Vec3::new(-1.2, 1.4, -0.2), 0.5), // the tank's phosphor spill
        (Vec3::new(0.0, 1.3, 0.6), 1.2),   // free air mid-cabin
    ];
    for i in 0..MOTES {
        let h = space_trucking::sim::splitmix(0x0D05_7A1E, i);
        let (pool, spread) = pools[(h % 3) as usize];
        let offset = Vec3::new(
            (((h >> 8) & 0xFF) as f32 / 255.0 - 0.5) * 2.0 * spread,
            (((h >> 16) & 0xFF) as f32 / 255.0 - 0.5) * 1.4 * spread,
            (((h >> 24) & 0xFF) as f32 / 255.0 - 0.5) * 2.0 * spread,
        );
        commands.spawn((
            Mesh3d(fleck.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pool + offset),
            Mote {
                base: pool + offset,
                phase: ((h >> 32) & 0xFFFF) as f32 / 65535.0 * std::f32::consts::TAU,
            },
        ));
    }
    commands.insert_resource(MoteMat(mat));
}

/// The air drifts — slow, unhurried, the pace of a parked freighter —
/// and takes the omen's color the way the lights do: violet creeps in,
/// brightness follows the console light level.
fn drift_motes(
    time: Res<Time>,
    shell: Res<Shell>,
    mote_mat: Res<MoteMat>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut motes: Query<(&Mote, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (mote, mut transform) in &mut motes {
        transform.translation = mote.base
            + Vec3::new(
                t.mul_add(0.11, mote.phase).sin() * 0.09,
                mote.phase.mul_add(2.0, t * 0.07).sin() * 0.06,
                mote.phase.mul_add(3.0, t * 0.09).cos() * 0.09,
            );
    }
    let sim = &shell.bridge.sim;
    if let Some(mut mat) = materials.get_mut(&mote_mat.0) {
        let tone = palette::mix(palette::GLINT, palette::EERIE_BRIGHT, sim.omen());
        mat.emissive = tone.to_linear() * (0.6 * sim.light().max(0.35));
    }
}

/// Cabin lights obey the sim's light level — the omen dims the room, and
/// nothing opts out.
fn dim_cabin(shell: Res<Shell>, mut lights: Query<(&mut PointLight, &Dimmable)>) {
    let light = shell.bridge.sim.light();
    for (mut lamp, dimmable) in &mut lights {
        lamp.intensity = dimmable.intensity * light;
    }
}

/// The violet source swells with the omen and spikes on the jump.
fn omen_swell(
    time: Res<Time>,
    shell: Res<Shell>,
    mut flash: ResMut<JumpFlash>,
    mut light: Single<&mut PointLight, With<OmenLight>>,
) {
    if shell
        .bridge
        .sim
        .cues()
        .iter()
        .any(|c| matches!(c, Cue::Jump))
    {
        flash.left = JUMP_LEN;
    }
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    let heat = flash.left / JUMP_LEN;
    // The flash squares in, matching the 2D full-screen bloom's falloff.
    light.intensity = shell
        .bridge
        .sim
        .omen()
        .mul_add(OMEN_LUMENS, JUMP_LUMENS * heat * heat);
    light.color = if heat > 0.0 {
        palette::EERIE_BRIGHT
    } else {
        palette::EERIE
    };
}
