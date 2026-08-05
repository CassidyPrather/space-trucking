//! Space Trucking's 3D cabin: the Bevy first-person frontend.
//!
//! Everything that decides what happens lives in `space_trucking::sim` —
//! the same deterministic library the 2D console runs, saves and all. This
//! binary is a different window onto it: a cramped freighter cabin where
//! the console's regions are physical panels. `surface` maps the cursor
//! ray onto sim coordinates, `bridge` owns the sim/save/tape, and the view
//! modules read sim state back onto cabin geometry. The sim never learns
//! it grew a third dimension.

// Bevy systems take `Res`/`Query` by value; fighting pedantic over it
// per-function is noise.
#![allow(clippy::needless_pass_by_value)]

mod audio;
mod bridge;
mod console;
mod fx;
mod glow;
mod nav;
mod palette;
mod rig;
mod surface;

use bevy::prelude::*;
use bevy::window::PresentMode;

use bridge::{Bridge, FrameInput, FrameOutcome};
use space_trucking::sim::layout;
use surface::VirtualPointer;

/// The shell's one resource: the bridge (sim, save, tape) plus the two
/// bits of frontend-only state the sim refuses to own.
#[derive(Resource)]
pub struct Shell {
    pub bridge: Bridge,
    /// What this frame's advance concluded, for systems downstream.
    pub outcome: FrameOutcome,
    /// Mute is presentation; the audio systems consume it.
    pub muted: bool,
}

/// Frame phases: gather the pointer, advance the sim once, then let every
/// view read the fresh state (cues included) in peace.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Input,
    Advance,
    View,
}

fn main() {
    let dev = std::env::args().any(|arg| arg == "--dev");
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        // The one place the game says its own name.
                        title: "space trucking".into(),
                        resolution: (1280, 720).into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                // Nearest sampling everywhere: small textures, hard edges.
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(Shell {
            bridge: Bridge::boot(dev),
            outcome: FrameOutcome::default(),
            muted: false,
        })
        .init_resource::<VirtualPointer>()
        .configure_sets(Update, (Phase::Input, Phase::Advance, Phase::View).chain())
        .add_plugins((
            audio::AudioPlugin,
            console::ConsolePlugin,
            fx::FxPlugin,
            nav::NavPlugin,
        ))
        .add_systems(Startup, rig::spawn)
        .add_systems(
            Update,
            (rig::glance, surface::track_pointer).in_set(Phase::Input),
        )
        .add_systems(Update, advance.in_set(Phase::Advance))
        .run();
}

/// Gather this frame's input in sim terms and advance the world exactly
/// once — the sim drains pointer edges per call, so once is the law.
fn advance(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<VirtualPointer>,
    mut shell: ResMut<Shell>,
) {
    let at = pointer.sim;
    let input = FrameInput {
        pointer: at,
        press: buttons.just_pressed(MouseButton::Left),
        held: buttons.pressed(MouseButton::Left),
        release: buttons.just_released(MouseButton::Left),
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        key_pause: keys.just_pressed(KeyCode::Space),
        key_warp: keys.just_pressed(KeyCode::KeyF),
        key_mute: keys.just_pressed(KeyCode::KeyM),
        key_reseed: keys.just_pressed(KeyCode::KeyR),
        // The console icons fold into shell toggles, same as 2D.
        icon_pause: layout::PAUSE_BTN.contains(at),
        icon_warp: layout::WARP_BTN.contains(at),
        icon_mute: layout::SPEAKER.contains(at),
    };
    let outcome = shell.bridge.frame(time.delta_secs(), &input);
    if outcome.toggle_mute {
        shell.muted = !shell.muted;
    }
    shell.outcome = outcome;
}
