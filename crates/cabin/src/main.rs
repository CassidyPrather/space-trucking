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
mod bridge;
mod canvas;
mod console;
mod crt;
mod fixture;
mod fx;
mod gesture;
mod glow;
mod menu;
mod palette;
mod pieces;
mod rig;
mod room;
mod surface;
mod viewport;
mod wear;

use bevy::prelude::*;
use bevy::window::PresentMode;

use bridge::{Bridge, FrameInput, FrameOutcome};
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

/// Dev tooling: `--gauge <frames>` lets the scene settle, times that many
/// frames, prints one line, and exits.
///
/// It measures the thing the exterior pass actually claims — that the
/// cost of a wall of windows does not track the number of windows — and
/// it measures it the only way a claim like that can be honest, which is
/// with a control arm on the same code path (`--grouping pane`). The
/// numbers are meaningless in absolute terms wherever this runs (the
/// container's GPU is llvmpipe, in software); the CURVE across
/// `--panes 1 2 4 8` is the whole reading.
///
/// It reads `Time<Real>` and not the game clock, deliberately: the
/// virtual clock CLAMPS a long frame (Bevy's `max_delta`, a quarter of
/// a second) so that a stalled frame cannot throw the sim's catch-up.
/// That is exactly right for the game and exactly wrong for a gauge —
/// it silently floors every measurement worse than the clamp, which is
/// to say every measurement the gauge exists to take.
#[derive(Resource)]
struct Gauge {
    want: u32,
    frames: u32,
    /// Seconds accumulated over the measured window.
    took: f32,
    panes: usize,
    grouping: viewport::Grouping,
}

/// Frames burned before the gauge starts counting: the same settle the
/// screenshot path takes, for the same reason — a cold pipeline is not
/// what anybody is asking about.
const GAUGE_SETTLE: u32 = 60;

/// The cabin's own `--view` roam poses. Rooms derive theirs from the
/// graph (`room::preset`); these three are the starter cabin's, and they
/// stand back one cell further than they once did — the 8×7 floor put a
/// cell of room between every hull plane and where it was, and a
/// viewpoint that did not follow ends up nose-first on the wall it is
/// meant to frame.
fn cabin_preset(name: &str, rig: &mut rig::CameraRig) {
    match name {
        "bay" => {
            rig.pos.z = -0.85;
            rig.yaw = std::f32::consts::PI;
            rig.pitch = -0.22;
        }
        // The front wall — bare metal since the console face came off,
        // and worth a preset precisely so it can be checked.
        "front" => {
            rig.pos = Vec3::new(0.0, 1.35, 0.85);
            rig.yaw = 0.0;
            rig.pitch = -0.06;
        }
        // The starboard wall by the doorway — the starter chart tank berth.
        "starboard" => {
            rig.pos = Vec3::new(-0.60, 1.40, 0.76);
            rig.yaw = -std::f32::consts::FRAC_PI_2;
            rig.pitch = -0.10;
        }
        // Outside, in dry dock: the one view that is not from aboard.
        // Dev tooling for the ship's own exterior shells (`viewport`),
        // which are otherwise only ever seen through a window. It stands
        // off the port bow, high enough to read the whole graph of rooms.
        "drydock" => {
            let eye = Vec3::new(7.6, 4.2, -8.2);
            let at = Vec3::new(0.6, 1.1, 1.9);
            let d = at - eye;
            rig.pos = eye;
            rig.yaw = (-d.x).atan2(-d.z);
            rig.pitch = d.y.atan2(d.xz().length());
            rig.drydock = true;
        }
        _ => {}
    }
}

/// A fresh world, already `along` of the way through its first leg, as a
/// save string (`--underway`).
///
/// Dev tooling, and it cheats at nothing: it charts a POI and pulls the
/// launch handle through the sim's own `InputFrame` interface, exactly as
/// a player would, then runs the leg on. Whatever the gates say, they say
/// — a world that refuses to launch simply boots docked.
#[allow(clippy::cast_sign_loss)] // the fraction is clamped non-negative below
fn cast_off(seed: u64, along: f32) -> String {
    use space_trucking::sim::{InputFrame, ShipState, Sim, TICK_DT, layout};
    let mut sim = Sim::new(seed);
    let press = |at| InputFrame {
        pointer: at,
        press: true,
        held: true,
        ..InputFrame::default()
    };
    if let ShipState::Docked(here) = sim.ship().state
        && let Some(there) = (0..12_u8).find(|&id| id != here && sim.poi_chartable(id))
    {
        sim.advance(0.0, &press(sim.poi_pos(there)));
        sim.advance(0.0, &press(canvas::rect_center(layout::LAUNCH_LEVER)));
        sim.advance(TICK_DT, &InputFrame::default());
    }
    if let ShipState::Traveling { leg_ticks, .. } = sim.ship().state {
        sim.fast_forward((leg_ticks as f32 * along.clamp(0.0, 1.0)) as u64);
    }
    sim.save_string()
}

// One paragraph per dev flag, each argued where it stands; splitting the
// boot in half would only put half the arguments somewhere else.
#[allow(clippy::too_many_lines)]
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
    // `--menu`: boot with the `Esc` menu standing open. Dev tooling for
    // screenshot runs (the menu is a click away otherwise), kept because
    // a shot of the meta-controls is exactly the kind of thing that
    // wants capturing without a hand on the keyboard.
    let open_menu = args.iter().any(|arg| arg == "--menu");
    // `--view tank|lever|bay` boots parked at that viewpoint — mostly
    // for screenshot runs, harmless interactively. The bay has no focus
    // pose; its view is a roam pose facing aft. Both instrument
    // viewpoints find their pieces on the first frame, wherever the
    // board hangs them: there is no fixed station left to name.
    let view_name = flag_value("--view");
    let view = view_name.as_deref().and_then(|name| match name {
        "tank" => Some(rig::Focus::Tank),
        "lever" => Some(rig::Focus::Lever),
        _ => None,
    });
    let mut boot_rig = rig::CameraRig::boot(view);
    if let Some(name) = view_name.as_deref() {
        cabin_preset(name, &mut boot_rig);
    }

    // `--panes n`: the stress board — the starter ship with every window
    // stripped and `n` hung on one wall (`fixture::panes_board`). The
    // scaling measurement's own fixture, and at `n = 0` the sold-window
    // case: no pane, no aperture, no view, solid hull.
    let panes = flag_value("--panes").and_then(|n| n.parse::<usize>().ok());
    // `--grouping wall|pane`: which law the exterior gathers panes by.
    // `wall` is what ships; `pane` is the control arm the curve is read
    // against (see `viewport::Grouping`).
    let grouping = match flag_value("--grouping").as_deref() {
        Some("pane") => viewport::Grouping::Pane,
        _ => viewport::Grouping::Wall,
    };

    // `--underway`: a world that has already cast off, so the transit sky
    // — star streaks, the destination growing off the bow — can be looked
    // at without waiting out a leg. Sandboxed exactly like `--fixture`.
    let underway = args.iter().any(|arg| arg == "--underway");
    let bridge = panes.map_or_else(
        || {
            if underway {
                Bridge::boot_fixture(&cast_off(7, 0.75))
            } else if fixture {
                Bridge::boot_fixture(fixture::SAVE)
            } else {
                Bridge::boot(dev)
            }
        },
        |n| Bridge::boot_fixture(&fixture::panes_board(7, n)),
    );
    // The room presets are DERIVED, like everything else about a room:
    // they ask the graph where the room is and stand in the middle of it
    // facing the wall the view is named for. Attach the trade room
    // somewhere else and `--view trade` follows it.
    // The window's own presets are derived the same way, off the board
    // rather than off the graph: `--view pane|pane-port|pane-stbd` stands
    // at whatever wall the glass is actually hung on (`viewport::preset`).
    if let Some(name) = view_name.as_deref()
        && let Some(pose) =
            room::preset(bridge.sim.rooms(), name).or_else(|| viewport::preset(&bridge.sim, name))
    {
        boot_rig.pos = pose.0;
        boot_rig.yaw = pose.1;
        boot_rig.pitch = pose.2;
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
        bridge,
        outcome: FrameOutcome::default(),
        muted: false,
    })
    .insert_resource(boot_rig)
    .init_resource::<VirtualPointer>()
    .configure_sets(Update, (Phase::Input, Phase::Advance, Phase::View).chain())
    .insert_resource(menu::Menu::boot(open_menu))
    .insert_resource(grouping)
    .add_plugins((
        airlock::AirlockPlugin,
        audio::AudioPlugin,
        crt::CrtPlugin,
        fx::FxPlugin,
        gesture::GesturePlugin,
        menu::MenuPlugin,
        pieces::PiecesPlugin,
        room::RoomsPlugin,
        viewport::ViewportPlugin,
    ))
    .add_systems(Startup, rig::spawn)
    .add_systems(
        Update,
        (
            // The menu takes the keyboard first: an `Esc` it answers is
            // an `Esc` the camera must never also act on.
            menu::keys,
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
    if let Some(want) = flag_value("--gauge").and_then(|n| n.parse::<u32>().ok()) {
        app.insert_resource(Gauge {
            want,
            frames: 0,
            took: 0.0,
            panes: panes.unwrap_or(0),
            grouping,
        })
        .add_systems(Update, gauge.in_set(Phase::View));
    }
    app.run();
}

/// Time the settled frame loop and report once. One line, parseable,
/// carrying the two facts that make the number mean anything: how many
/// panes were hanging and how many skies the exterior actually drew for
/// them. See [`Gauge`].
fn gauge(
    time: Res<Time<bevy::time::Real>>,
    skies: Option<Res<viewport::Skies>>,
    mut mode: ResMut<Gauge>,
    mut exit: MessageWriter<AppExit>,
) {
    mode.frames += 1;
    if mode.frames <= GAUGE_SETTLE {
        return;
    }
    mode.took += time.delta_secs();
    if mode.frames < GAUGE_SETTLE + mode.want {
        return;
    }
    let lit = skies.map_or(0, |skies| skies.lit());
    let mean = mode.took * 1000.0 / mode.want as f32;
    println!(
        "gauge panes={} grouping={:?} skies={lit} frames={} mean_ms={mean:.2}",
        mode.panes, mode.grouping, mode.want
    );
    exit.write(AppExit::Success);
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
///
/// **The world keeps turning while the menu stands.** The menu parks the
/// pointer (nothing of it reaches the sim as a click) but never freezes
/// the frame: the only honest pause is the sim's own, folded below like
/// any other toggle, so a paused game is paused because the sim says so
/// and a menu left open overnight still arrives somewhere.
#[allow(clippy::too_many_arguments)]
fn advance(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<VirtualPointer>,
    camera: Res<rig::CameraRig>,
    grips: Res<gesture::Grips>,
    occupancy: Res<room::Occupancy>,
    latch: Res<room::AimedLatch>,
    mut menu: ResMut<menu::Menu>,
    mut shell: ResMut<Shell>,
) {
    let live = camera.interactive();
    let holding = shell.bridge.sim.held(0).is_some();
    // The detach gesture: a roam click on a door's amber latch asks the
    // input schedule to part that seam, and consumes the click so it
    // never doubles as a grab. Empty-handed only — a hand full of cargo
    // is exactly the hand the gangway law refuses.
    let parting = (!holding && camera.roaming() && buttons.just_pressed(MouseButton::Left))
        .then_some(latch.0)
        .flatten();
    let (at, press, held, release) = if parting.is_some() {
        (bridge::POINTER_PARKED, false, false, false)
    } else if camera.roaming() {
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
        // focus arrives and the drag continues at the station.
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
    // The menu's controls are worked with the mouse, but they arrive
    // here as plain edges — exactly where the console face's icon rects
    // used to fold in. One toggle path, whatever threw it: the sim is
    // still the only thing that decides what pausing means.
    let worked = menu.take();
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
        menu_pause: worked.pause,
        menu_warp: worked.warp,
        menu_mute: worked.mute,
        occupied: occupancy.0,
        detach: parting,
    };
    let outcome = shell.bridge.frame(time.delta_secs(), &input);
    if outcome.toggle_mute {
        shell.muted = !shell.muted;
    }
    shell.outcome = outcome;
}
