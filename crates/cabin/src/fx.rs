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

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JumpFlash>()
            .add_systems(Startup, spawn)
            .add_systems(Update, (dim_cabin, omen_swell).in_set(Phase::View));
    }
}

fn spawn(mut commands: Commands) {
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
