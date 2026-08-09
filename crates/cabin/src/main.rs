//! Space Trucking's 3D cabin: the Bevy first-person frontend.
//!
//! Everything that decides what happens lives in `space_trucking::sim` —
//! the deterministic library the retired 2D console ran, saves and all.
//! This binary is a window onto it: a cramped freighter cabin where the
//! console's regions are physical stations and the hold is a walkable
//! bay. `surface` maps cursor and crosshair rays onto sim coordinates,
//! `bridge` owns the sim/save/tape, and the view modules read sim state
//! back onto cabin geometry. The sim never learns it grew a third
//! dimension.

// Bevy systems take `Res`/`Query` by value; fighting pedantic over it
// per-function is noise.
#![allow(clippy::needless_pass_by_value)]

mod airlock;
mod audio;
mod barter;
mod bridge;
mod canvas;
mod console;
mod crt;
mod fixture;
mod fx;
mod gesture;
mod glow;
mod palette;
mod pieces;
mod rig;
mod surface;
mod viewport;
mod wear;

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

/// Dev tooling: `--shot <path>` renders a settling period, saves one
/// screenshot of the window, and exits — the visual-verification loop.
#[derive(Resource)]
struct ShotMode {
    path: String,
    frames: u32,
    fired: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dev = args.iter().any(|arg| arg == "--dev");
    // `--fixture`: boot the developer showcase save (one of everything,
    // actuated off defaults; see `fixture`) in a sandbox that never
    // writes over the real save. For sweeping the attachment surface.
    let fixture = args.iter().any(|arg| arg == "--fixture");
    let flag_value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|at| args.get(at + 1).cloned())
    };
    let shot = flag_value("--shot");
    // `--view tank|lever|console|desk|bay` boots parked at that
    // viewpoint — mostly for screenshot runs, harmless interactively.
    // The bay has no focus pose; its view is a roam pose facing aft.
    // The instrument viewpoints (tank, lever) find their pieces on the
    // first frame, wherever the board hangs them.
    let view_name = flag_value("--view");
    let view = view_name.as_deref().and_then(|name| match name {
        "tank" => Some(rig::Focus::Tank),
        "lever" => Some(rig::Focus::Lever),
        "console" => Some(rig::Focus::Console),
        "desk" => Some(rig::Focus::Desk),
        _ => None,
    });
    let mut boot_rig = rig::CameraRig::boot(view);
    if view_name.as_deref() == Some("bay") {
        boot_rig.pos.z = -0.30;
        boot_rig.yaw = std::f32::consts::PI;
        boot_rig.pitch = -0.30;
    }
    if view_name.as_deref() == Some("airlock") {
        boot_rig.pos = Vec3::new(0.30, 1.5, 1.05);
        boot_rig.yaw = -std::f32::consts::FRAC_PI_2;
        boot_rig.pitch = -0.30;
    }
    // The front wall, where the instrument cluster hangs.
    if view_name.as_deref() == Some("front") {
        boot_rig.pos = Vec3::new(0.10, 1.35, 0.55);
        boot_rig.yaw = 0.0;
        boot_rig.pitch = 0.12;
    }
    // The starboard wall by the doorway — the starter chart tank berth.
    if view_name.as_deref() == Some("starboard") {
        boot_rig.pos = Vec3::new(-0.40, 1.35, 0.20);
        boot_rig.yaw = -std::f32::consts::FRAC_PI_2;
        boot_rig.pitch = 0.05;
    }

    let mut app = App::new();
    app.add_plugins(
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
        bridge: if fixture {
            Bridge::boot_fixture(fixture::SAVE)
        } else {
            Bridge::boot(dev)
        },
        outcome: FrameOutcome::default(),
        muted: false,
    })
    .insert_resource(boot_rig)
    .init_resource::<VirtualPointer>()
    .configure_sets(Update, (Phase::Input, Phase::Advance, Phase::View).chain())
    .add_plugins((
        airlock::AirlockPlugin,
        audio::AudioPlugin,
        barter::BarterPlugin,
        console::ConsolePlugin,
        crt::CrtPlugin,
        fx::FxPlugin,
        gesture::GesturePlugin,
        pieces::PiecesPlugin,
        viewport::ViewportPlugin,
    ))
    .add_systems(Startup, rig::spawn)
    .add_systems(
        Update,
        (
            rig::steer,
            rig::pose,
            rig::present_mode,
            surface::track_pointer,
        )
            .chain()
            .in_set(Phase::Input),
    )
    .add_systems(Update, advance.in_set(Phase::Advance))
    .add_systems(Update, rig::fade_tiles.in_set(Phase::View));
    if let Some(path) = shot {
        app.insert_resource(ShotMode {
            path,
            frames: 0,
            fired: false,
        })
        .add_systems(Update, shoot.in_set(Phase::View));
    }
    app.run();
}

/// Let the scene settle, capture the window once, exit when the write
/// lands. Drives the in-container visual verification loop.
fn shoot(
    mut commands: Commands,
    mut mode: ResMut<ShotMode>,
    capturing: Query<(), With<bevy::render::view::screenshot::Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    mode.frames += 1;
    if !mode.fired && mode.frames > 45 {
        mode.fired = true;
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                mode.path.clone(),
            ));
    } else if mode.fired && mode.frames > 50 && capturing.is_empty() {
        exit.write(AppExit::Success);
    }
}

/// Gather this frame's input in sim terms and advance the world exactly
/// once — the sim drains pointer edges per call, so once is the law.
/// Focused stations get the freed cursor; roaming gets the crosshair
/// carry over the bay (and clicks on stations glide the camera instead,
/// empty-handed). Keys stay live in every mode.
fn advance(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<VirtualPointer>,
    camera: Res<rig::CameraRig>,
    grips: Res<gesture::Grips>,
    mut shell: ResMut<Shell>,
) {
    let live = camera.interactive();
    let holding = shell.bridge.sim.held(0).is_some();
    let (at, press, held, release) = if camera.roaming() {
        // The carry: a roam click grabs what the crosshair rests on; the
        // drag then persists hands-free (`held` synthesized every frame,
        // the pointer tracking the aim, parked off the bay); the next
        // click places — or, aimed at nothing, snaps the piece home.
        // Right-click is the explicit cancel. The sim sees ordinary drag
        // frames; every rule, cue, and conservation test applies as-is.
        let clicked = buttons.just_pressed(MouseButton::Left);
        let cancel = holding && buttons.just_pressed(MouseButton::Right);
        let place = holding && clicked;
        let grab = !holding && clicked && pointer.station.is_some();
        (
            if cancel {
                bridge::POINTER_PARKED
            } else {
                pointer.sim
            },
            grab,
            grab || (holding && !place && !cancel),
            place || cancel,
        )
    } else if !live && holding {
        // Mid-glide with cargo in hand — the player clicked a station
        // while carrying, and the camera is on its way. A frame without
        // a held signal would snap the piece home (the sim's phantom-
        // pointer guard), so the grip keeps synthesizing until the
        // focus arrives and the drag continues on the counter.
        (bridge::POINTER_PARKED, false, true, false)
    } else {
        // The gesture layer merges with raw input in one place
        // (`synthesize`, property-tested by the gesture monkey): lever
        // rects are withheld while hands are empty, and a completed pull
        // arrives as one plain press at the lever's center — what the 2D
        // console would have sent.
        gesture::synthesize(
            &grips,
            pointer.sim,
            holding,
            live,
            buttons.just_pressed(MouseButton::Left),
            buttons.pressed(MouseButton::Left),
            buttons.just_released(MouseButton::Left),
        )
    };
    let input = FrameInput {
        pointer: at,
        press,
        held,
        release,
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
