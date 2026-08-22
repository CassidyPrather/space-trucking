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
mod gauntlet;
mod gesture;
mod glow;
mod menu;
mod palette;
mod pieces;
mod poi;
mod rig;
mod room;
mod surface;
mod viewport;
mod wear;

use std::time::Duration;

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::time::{TimeSystems, Virtual};
use bevy::window::PresentMode;
use space_trucking::sim::room::RoomKind;

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

/// **A judged run counts its clock instead of measuring it.**
///
/// Everything that moves in the cabin reads `Time::elapsed_secs` — the
/// breathing emissives, the CRT sweep, the console sway, the refusal
/// strobe, the drifting motes, the stars — and the dev modes that judge a
/// picture shoot at a frame NUMBER. Those two facts disagree: on a
/// machine whose startup, shader build and pipeline warm-up vary by
/// seconds (this one rasterises in software), frame 46 lands at a
/// different instant every run, every effect is sampled at a different
/// phase, and one view shot twice is two different pictures.
///
/// So a judged run takes the wall clock away from the game. The game
/// clock is paused and advanced by exactly one [`FRAME_STEP`] per frame,
/// which puts frame N at N × step whatever the machine did to get there.
/// No animation has to change: they all read the clock they always read,
/// and it now says the same thing at the same frame on every run.
///
/// `Time<Real>` is left alone, because the gauge measures with it — see
/// [`Gauge`], which is the one dev mode this is deliberately not applied
/// to.
///
/// The step is the sim's own tick, `TICK_DT`, to the last bit an `f32`
/// has (`tests::a_pinned_frame_is_one_tick_of_the_sim`). One rendered
/// frame is then exactly one simulated tick — the sim's accumulator
/// never drops a tick nor doubles one — and the settle counts below read
/// in world seconds as well as in frames.
const FRAME_STEP: Duration = Duration::from_nanos(16_666_667);

/// Pin the clock for a run that has to reproduce. Called by `--shot` and
/// `--gauntlet-walk`, and by nothing else.
fn pin_clock(app: &mut App) {
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.add_systems(First, step_clock.after(TimeSystems));
}

/// One frame of the pinned clock. The generic `Time` every view reads is
/// a copy of the game clock, so it is stepped in the same breath — Bevy
/// took its copy while the clock was still paused.
fn step_clock(mut game: ResMut<Time<Virtual>>, mut time: ResMut<Time>) {
    game.advance_by(FRAME_STEP);
    *time = game.as_generic();
}

/// Dev tooling: `--shot <path>` renders a settling period, saves one
/// screenshot of the window, and exits — the visual-verification loop.
///
/// It runs on the pinned clock ([`FRAME_STEP`]), so the same view shot
/// twice is the same file, byte for byte — which is the guard
/// `gauntlet::tests::the_same_view_shot_twice_is_the_same_bytes` holds
/// it to.
#[derive(Resource)]
struct ShotMode {
    path: String,
    frames: u32,
    fired: bool,
}

/// Frames burned before the shutter: long enough for a glide to finish
/// and the room to be lit, and on the pinned clock exactly three quarters
/// of a world second, every run.
const SHOT_SETTLE: u32 = 45;

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
///
/// The counted clock the judged modes run on ([`FRAME_STEP`]) is the
/// same argument a second time, and louder: it would hand the gauge
/// back the step it was given. So the gauge is not on it — and because
/// the pin stops at the game clock, it could not reach this measurement
/// even if some future flag put the two modes in one process.
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

/// Dev tooling: `--gauntlet-walk <dir>` drives the scripted room walk
/// (`gauntlet::walk`), captures a frame at every waypoint, holds still
/// for [`gauntlet::FLICKER_FRAMES`] frames to see whether the picture
/// does, then backs off along an approach sampling the room's own
/// brightness. The three things a still cannot see, in one pass.
///
/// It reads the WINDOW, so it needs a rasteriser: run it under `xvfb`
/// with the software Vulkan device (`gauntlet::tests::the_pixel_half_is_opt_in`
/// carries the invocation). The pure half of the same walk runs in
/// `cargo test` and needs none of that.
#[derive(Resource, Default)]
struct WalkMode {
    /// Where the filmstrip is written.
    dir: String,
    /// Which room is being walked, for the file names and the report.
    room: String,
    /// The walk, filled in on the first frame from the live plan.
    steps: Vec<gauntlet::Step>,
    /// The stand-offs the light-pop pass samples from.
    approach: Vec<(Vec3, Vec3)>,
    /// Which waypoint is being shot, then which flicker frame, then
    /// which approach sample.
    at: usize,
    /// How many shutters have fired in all — the film's own index.
    shot: usize,
    phase: WalkPhase,
    /// Frames burned at the current pose before the shutter.
    settle: u32,
    /// Whether this pose's shutter is already in flight.
    fired: bool,
    /// Mean luminance per stand-off of the approach.
    lit: Vec<f32>,
    /// Whatever went wrong, for the exit code.
    faults: Vec<String>,
}

/// Which of the three passes `--gauntlet-walk` is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum WalkPhase {
    /// One frame per waypoint: the filmstrip.
    #[default]
    Strip,
    /// The camera holds still and the frames are compared: the flicker.
    Still,
    /// The camera backs away along the approach: the light pop.
    Approach,
    Done,
}

/// The flag the pixel half is reached by. Named here so the doc comment
/// that documents the invocation cannot drift from the argument that
/// answers it.
#[must_use]
const fn gauntlet_walk_flag() -> &'static str {
    "--gauntlet-walk"
}

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
        // Outside, off a caller's own outboard face. The pose is derived
        // from the graph (`room::preset`); all this arm does is let the
        // cabin camera see the void layer, which is the half of `drydock`
        // that is not a position.
        "berth" => rig.drydock = true,
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
    // `--gauntlet`: the adversarial sweep's pure half, printed. Every
    // room in the game against every geometric rule the harness has, with
    // no window, no GPU, and no clock — which is why it can also run in
    // `cargo test`, and does. It exits non-zero only on something the
    // docket does not already carry, because a defect somebody has
    // already written down is not news.
    // `--gauntlet-docket`: the same sweep, printed as the docket's own
    // `room | rule | offender` lines. How the work order is regenerated
    // after a fixing pass, so nobody transcribes 600 lines by hand.
    if args.iter().any(|arg| arg == "--gauntlet-docket") {
        print!("{}", gauntlet::as_docket(&gauntlet::sweep()));
        std::process::exit(0);
    }
    if args.iter().any(|arg| arg == "--gauntlet") {
        let found = gauntlet::sweep();
        print!("{}", gauntlet::report(&found));
        let fresh = gauntlet::undocketed(&found);
        if !fresh.is_empty() {
            eprintln!("gauntlet: {} finding(s) off the docket", fresh.len());
        }
        std::process::exit(i32::from(!fresh.is_empty()));
    }
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
    // `--docked n`: the fixture board, moored at POI `n` instead of the
    // Guild. Every run starts at the Guild, so this is the only way to
    // *look* at the other eleven stations' rooms — which is exactly what
    // a per-station design agent has to do before it can judge anything
    // it wrote (`crate::poi`).
    let docked = flag_value("--docked").and_then(|n| n.parse::<u8>().ok());
    // `--alongside wreck|parlor|pump`: a leg with that event room already
    // attached. `--docked n` berths any of the twelve stations, but the
    // three rooms nobody keeps are met in transit and gone by the next
    // dock, so this is the only way to stand in one — or to photograph
    // its shell with `--view berth` (`fixture::alongside`).
    let met = flag_value("--alongside")
        .and_then(|name| match name.as_str() {
            "wreck" => Some(RoomKind::Wreck),
            "parlor" => Some(RoomKind::Parlor),
            "pump" => Some(RoomKind::Pump),
            _ => None,
        })
        .and_then(fixture::alongside);
    let bridge = panes.map_or_else(
        || {
            met.as_deref().map_or_else(
                || {
                    if underway {
                        Bridge::boot_fixture(&cast_off(7, 0.75))
                    } else if let Some(poi) = docked {
                        Bridge::boot_fixture(&fixture::docked_at(poi))
                    } else if fixture {
                        Bridge::boot_fixture(fixture::SAVE)
                    } else {
                        Bridge::boot(dev)
                    }
                },
                Bridge::boot_fixture,
            )
        },
        |n| Bridge::boot_fixture(&fixture::panes_board(7, n)),
    );
    // `--gauntlet-walk <dir>`: the pixel half. It boots whatever board
    // the other flags asked for and then LOADS it — cargo in every legal
    // berth of every room aboard — because a room photographed empty is
    // exactly the hole this harness exists to close (`gauntlet::load`).
    let walk_dir = flag_value(gauntlet_walk_flag());
    let mut bridge = if walk_dir.is_some() {
        gauntlet::loaded_save(&bridge.sim.save_string())
            .map_or(bridge, |save| Bridge::boot_fixture(&save))
    } else {
        bridge
    };
    // A run that judges a picture — the screenshot, the walk — must give
    // the same answer twice, so it counts its clock instead of measuring
    // it: the frames come off a fixed step ([`FRAME_STEP`]) and the world
    // stops reading the wall (`Bridge::steady`).
    let judged = shot.is_some() || walk_dir.is_some();
    if judged {
        bridge.steady();
    }
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
            .set(ImagePlugin::default_nearest())
            // **A judged run waits for its shaders.** Bevy builds
            // pipelines off the frame loop and DRAWS WITHOUT THE MESHES
            // whose pipelines are not built yet, which is the right
            // trade for a game — a stutter beats a freeze — and the
            // wrong one for a shutter: on a busy machine the settle runs
            // out first and the picture is the clear colour with the
            // scene missing. Compiling in the frame that needs it costs
            // a judged run a slower start and nothing else.
            .set(RenderPlugin {
                synchronous_pipeline_compilation: judged,
                ..default()
            }),
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
            // After the survey, so the body is put back aboard against
            // the envelope this frame's graph actually has, and before
            // anything walks, aims or glides from where it stands.
            keep_aboard.after(room::survey),
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
    if judged {
        pin_clock(&mut app);
    }
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
    if let Some(dir) = walk_dir {
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|why| panic!("the filmstrip needs somewhere to land: {dir} ({why})"));
        app.insert_resource(WalkMode {
            dir,
            ..WalkMode::default()
        })
        .init_resource::<WalkFilm>()
        .add_systems(Update, walk_drive.in_set(Phase::View));
    }
    // The dev tools that JUDGE — the gauntlet's walk today — answer with
    // an exit code, and an exit code the process throws away is a check
    // that always passes.
    if let AppExit::Error(code) = app.run() {
        std::process::exit(i32::from(code.get()));
    }
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

/// Frames burned at each pose before the shutter: enough for a glide to
/// finish, a lamp to wake, and the pipeline to stop being cold.
const WALK_SETTLE: u32 = 24;

/// And between two flicker samples — one frame, because the whole point
/// is to look at consecutive frames from one pose.
const STILL_SETTLE: u32 = 1;

/// One captured frame's readings: how bright it was, and how much of it
/// moved since the last one.
#[derive(Resource, Default)]
struct WalkFilm {
    /// Mean luminance of the last frame captured, 0..=1.
    lum: Vec<f32>,
    /// Fraction of pixels that moved since the previous capture.
    moved: Vec<f32>,
    /// The previous frame's bytes, for the difference.
    last: Option<Vec<u8>>,
    /// How many captures have landed.
    landed: usize,
}

/// **What a captured frame amounts to**: how bright it is on average,
/// and how much of it stands clear of the dark.
///
/// The second number is the one no other check in the game asks for. The
/// gauntlet measures shapes, and a shape is exactly as present in a
/// black frame as in a lit one; mean brightness will not stand in for it
/// either, because pure black is banned and the darkest frame the game
/// can draw already means out well above zero. What separates a room you
/// can see from a room that drew nothing is the *fraction* of the
/// picture over [`gauntlet::READ_FLOOR`].
///
/// Raw bytes rather than a decode — the window target is 8-bit RGBA and
/// both numbers are one pass over it.
struct FrameRead {
    /// Rec. 601 luma, meaned over every texel, `0..=1`.
    lum: f32,
    /// Fraction of texels standing clear of the ground, `0..=1`.
    read: f32,
}

fn frame_read(frame: &Image) -> FrameRead {
    let Some(data) = frame.data.as_ref() else {
        return FrameRead {
            lum: 0.0,
            read: 0.0,
        };
    };
    let mut sum = 0.0f64;
    let mut clear = 0usize;
    for texel in data.as_chunks::<4>().0 {
        let luma = 0.299f64.mul_add(
            f64::from(texel[0]),
            0.587f64.mul_add(f64::from(texel[1]), 0.114 * f64::from(texel[2])),
        ) / f64::from(255.0_f32);
        sum += luma;
        if luma >= f64::from(gauntlet::READ_FLOOR) {
            clear += 1;
        }
    }
    let pixels = (data.len() / 4).max(1);
    FrameRead {
        lum: (sum / pixels as f64) as f32,
        read: (clear as f64 / pixels as f64) as f32,
    }
}

/// Read one captured frame: [`frame_read`]'s brightness, and the fraction
/// of it that moved since the last capture.
fn read_film(frame: &Image, film: &mut WalkFilm) {
    let Some(data) = frame.data.as_ref() else {
        return;
    };
    let mut moved = 0usize;
    let pixels = (data.len() / 4).max(1);
    for (i, texel) in data.as_chunks::<4>().0.iter().enumerate() {
        if let Some(last) = film.last.as_ref()
            && last.len() == data.len()
            && (0..3).any(|c| texel[c].abs_diff(last[i * 4 + c]) > gauntlet::FLICKER_STEP)
        {
            moved += 1;
        }
    }
    film.lum.push(frame_read(frame).lum);
    film.moved.push(moved as f32 / pixels as f32);
    film.last = Some(data.clone());
    film.landed += 1;
}

/// Drive the scripted walk: pose, settle, shoot, advance. Three passes —
/// the filmstrip, the still, and the approach — then the verdict.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn walk_drive(
    mut commands: Commands,
    plan: Res<room::Plan>,
    mut mode: ResMut<WalkMode>,
    mut film: ResMut<WalkFilm>,
    mut rig: ResMut<rig::CameraRig>,
    capturing: Query<(), With<bevy::render::view::screenshot::Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    // The room under judgement is whatever came alongside — a station's,
    // an event's — falling back to the cabin on a ship with nothing
    // attached. Read off the plan, so `--docked n` and `--alongside`
    // aim this without it learning either flag.
    if mode.steps.is_empty() {
        let Some(placed) = plan
            .rooms
            .iter()
            .find(|placed| !placed.kind.riding())
            .or_else(|| plan.rooms.first())
        else {
            return;
        };
        mode.steps = gauntlet::walk(placed);
        mode.approach = gauntlet::approach(placed);
        mode.room = format!("{:?}", placed.kind);
        if mode.steps.is_empty() {
            exit.write(AppExit::Success);
            return;
        }
    }
    let (eye, at) = match mode.phase {
        WalkPhase::Done => {
            let code = u8::from(!mode.faults.is_empty());
            for fault in &mode.faults {
                eprintln!("gauntlet-walk: {fault}");
            }
            exit.write(if code == 0 {
                AppExit::Success
            } else {
                AppExit::from_code(code)
            });
            return;
        }
        WalkPhase::Strip => {
            let step = mode.steps[mode.at.min(mode.steps.len() - 1)];
            let ahead = Quat::from_euler(EulerRot::YXZ, step.yaw, step.pitch, 0.0) * Vec3::NEG_Z;
            (step.eye, step.eye + ahead)
        }
        // The still holds exactly the pose the filmstrip's middle shot
        // used: a flicker is a thing the picture does while nothing else
        // does anything.
        WalkPhase::Still => {
            let step = mode
                .steps
                .iter()
                .find(|step| step.label == "middle")
                .copied()
                .unwrap_or(mode.steps[0]);
            let ahead = Quat::from_euler(EulerRot::YXZ, step.yaw, step.pitch, 0.0) * Vec3::NEG_Z;
            (step.eye, step.eye + ahead)
        }
        WalkPhase::Approach => mode.approach[mode.at.min(mode.approach.len() - 1)],
    };
    rig.pos = eye;
    let d = at - eye;
    rig.yaw = (-d.x).atan2(-d.z);
    rig.pitch = d.y.atan2(d.xz().length().max(1e-4));
    rig.parked = true;
    let want = if mode.phase == WalkPhase::Still {
        STILL_SETTLE
    } else {
        WALK_SETTLE
    };
    if mode.settle < want {
        mode.settle += 1;
        return;
    }
    if !capturing.is_empty() {
        return;
    }
    if film.landed <= mode.shot {
        if mode.fired {
            return;
        }
        mode.fired = true;
        let label = match mode.phase {
            WalkPhase::Strip => mode.steps[mode.at].label.to_owned(),
            WalkPhase::Still => format!("still-{:02}", mode.at),
            WalkPhase::Approach | WalkPhase::Done => format!("approach-{:02}", mode.at),
        };
        let path = format!("{}/{}-{:02}-{label}.png", mode.dir, mode.room, mode.at);
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path))
            .observe(
                move |captured: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                      mut film: ResMut<WalkFilm>| {
                    read_film(&captured.image, &mut film);
                },
            );
        return;
    }
    // A sample landed. Read it, judge it, and step on.
    mode.fired = false;
    mode.shot += 1;
    mode.settle = 0;
    let lum = film.lum.last().copied().unwrap_or(0.0);
    let moved = film.moved.last().copied().unwrap_or(0.0);
    println!(
        "gauntlet-walk room={} phase={:?} at={} eye=({:.2},{:.2},{:.2}) lum={lum:.5} moved={moved:.5}",
        mode.room, mode.phase, mode.at, eye.x, eye.y, eye.z
    );
    match mode.phase {
        WalkPhase::Strip => {
            mode.at += 1;
            if mode.at >= mode.steps.len() {
                mode.at = 0;
                mode.phase = WalkPhase::Still;
                film.last = None;
            }
        }
        WalkPhase::Still => {
            // The first sample of the still has nothing to be compared
            // with; every one after it does, and a lamp on an
            // every-other-frame cycle cannot hide from a run of ten.
            if mode.at > 0 && moved > gauntlet::FLICKER_TOL {
                let fault = format!(
                    "{}: {:.2}% of the picture moved between still frames {} and {} \
                     from one pose — something is flickering",
                    mode.room,
                    moved * 100.0,
                    mode.at - 1,
                    mode.at
                );
                mode.faults.push(fault);
            }
            mode.at += 1;
            if mode.at >= gauntlet::FLICKER_FRAMES {
                mode.at = 0;
                mode.phase = if mode.approach.is_empty() {
                    WalkPhase::Done
                } else {
                    WalkPhase::Approach
                };
                film.last = None;
            }
        }
        WalkPhase::Approach => {
            mode.lit.push(lum);
            mode.at += 1;
            if mode.at >= mode.approach.len() {
                let peak = mode.lit.iter().copied().fold(0.0f32, f32::max).max(1e-6);
                let room = mode.room.clone();
                let pops: Vec<String> = mode
                    .lit
                    .windows(2)
                    .enumerate()
                    .filter_map(|(i, step)| {
                        let jump = (step[1] - step[0]).abs() / peak;
                        (jump > gauntlet::POP_TOL).then(|| {
                            format!(
                                "{room}: the room's brightness jumped {:.0}% between \
                                 stand-off {i} and {} — a light is switching with \
                                 distance, not fading",
                                jump * 100.0,
                                i + 1
                            )
                        })
                    })
                    .collect();
                mode.faults.extend(pops);
                mode.phase = WalkPhase::Done;
            }
        }
        WalkPhase::Done => {}
    }
}

/// Let the scene settle, capture the window once, exit when the write
/// lands. Drives the in-container visual verification loop.
///
/// **Every shot prints what it came out as** ([`frame_read`]), because
/// the one thing a reviewer cannot get out of a PNG by looking at a
/// thumbnail is whether the frame is dark on purpose or dark because
/// nothing drew. A `shot` line is the number a legibility claim can be
/// argued from, and the one an automated guard can read back.
fn shoot(
    mut commands: Commands,
    mut mode: ResMut<ShotMode>,
    capturing: Query<(), With<bevy::render::view::screenshot::Capturing>>,
    mut exit: MessageWriter<AppExit>,
) {
    mode.frames += 1;
    if !mode.fired && mode.frames > SHOT_SETTLE {
        mode.fired = true;
        let path = mode.path.clone();
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                mode.path.clone(),
            ))
            .observe(
                move |captured: On<bevy::render::view::screenshot::ScreenshotCaptured>| {
                    let read = frame_read(&captured.image);
                    println!("shot path={path} lum={:.5} read={:.5}", read.lum, read.read);
                },
            );
    } else if mode.fired && mode.frames > SHOT_SETTLE + 5 && capturing.is_empty() {
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

/// **The body stands where a body can stand.**
///
/// `rig::pose_is_aboard` already says this about the camera, and it says
/// it because a camera in the hull is a view the player cannot read
/// their way out of. The body needs the same sentence, and for a
/// stronger reason: the crosshair reaches [`rig::REACH`] and no further,
/// so a body standing where the ship is not can work nothing at all —
/// every left click in the cabin falls on empty space, which is what a
/// lockup looks like from the seat.
///
/// A doorway is where it happens. The walk envelope joins two rooms with
/// a connector across their shared seam, and that connector belongs to
/// neither room's own box: a body in it is, to `room::occupy`, still in
/// the room it came from (docs/ROOMS.md, "The one new input field"). So
/// the gangway law's "nothing detaches while it holds you" gate passes
/// for a body standing in the very gangway, the seam shuts, and the
/// connector the body was standing in stops existing.
///
/// Nothing here refuses that detach — shutting the door behind you is
/// the whole gesture. The body simply comes back inside with it, to the
/// nearest place the ship still offers, which is the same answer
/// `rig::steer` gives a walk that runs out of floor.
///
/// It is stated as a standing property rather than as a detach handler
/// because the graph can change for reasons the cabin never asked for —
/// a departure dismisses every calling room, an arrival brings one — and
/// a law that only guarded the press would be a law with a back door.
///
/// **It is a transition, not a fence.** What is wrong is the ship moving
/// out from under a body that was aboard; a camera that was never aboard
/// in the first place is not a body at all. That is the two dev views
/// that stand outside the hull on purpose (`--view drydock|berth`) and
/// the gauntlet's walk, which poses the camera off a room's outboard
/// face to judge it — none of them is somebody who has to be able to
/// click something, and a fence would drag all three back inside.
fn keep_aboard(
    envelope: Res<room::Envelope>,
    walk: Option<Res<WalkMode>>,
    mut rig: ResMut<rig::CameraRig>,
    mut was_aboard: Local<bool>,
) {
    if walk.is_some() || rig.drydock || envelope.rooms.is_empty() {
        return;
    }
    if !envelope.holds(rig.pos) && *was_aboard {
        rig.pos = envelope.nearest(rig.pos);
    }
    *was_aboard = envelope.holds(rig.pos);
}

#[cfg(test)]
mod tests {
    use space_trucking::sim::TICK_DT;

    use super::*;

    /// An app with nothing in it but a clock, pinned. Enough to state
    /// what the pin promises without a window anywhere near it.
    fn pinned() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        pin_clock(&mut app);
        app
    }

    /// **Frame N of a pinned run is N steps old, and one step is one
    /// tick.** The first half is why a screenshot reproduces: every
    /// animation in the cabin reads this clock, so a shot fired at a
    /// frame number is a shot fired at an instant. The second half is
    /// why nothing downstream stutters: the sim accumulates the same
    /// `f32` it spends, so one frame buys exactly one tick.
    #[test]
    fn a_pinned_frame_is_one_tick_of_the_sim() {
        let mut app = pinned();
        for frame in 1..=10u32 {
            app.update();
            let time = app.world().resource::<Time>();
            assert_eq!(time.delta(), FRAME_STEP);
            assert_eq!(time.elapsed(), FRAME_STEP * frame);
            // To the last bit: the sim spends an `f32`, and the step
            // and the tick are the same `f32`, so the accumulator that
            // buys ticks with frames comes out even.
            assert_eq!(time.delta_secs().to_bits(), TICK_DT.to_bits());
        }
    }

    /// **The pin never reaches the clock the gauge reads.** `--gauge`
    /// measures with `Time<Real>` on purpose ([`Gauge`]), and a
    /// measurement taken off a counted clock would only ever return the
    /// number it was told to. Bevy's own manual real-time strategy
    /// stands in for a machine here, so the two clocks can be watched
    /// disagreeing on purpose.
    #[test]
    fn a_pinned_run_leaves_the_measured_clock_alone() {
        use bevy::time::{Real, TimeUpdateStrategy};

        const MACHINE: Duration = Duration::from_millis(100);
        let mut app = pinned();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(MACHINE));
        for frame in 1..=4u32 {
            app.update();
            // The real clock spends its first update learning where it
            // is, so it is one frame behind the count — which is the
            // point: it is measured, not counted.
            assert_eq!(
                app.world().resource::<Time<Real>>().elapsed(),
                MACHINE * (frame - 1)
            );
            assert_eq!(
                app.world().resource::<Time<Virtual>>().elapsed(),
                FRAME_STEP * frame
            );
        }
    }
}

/// **A cabin with no screen**, and the laws it exists to state.
///
/// Every lockup this module guards against is one shape — a left click
/// that reaches nothing — and a click reaches nothing through the whole
/// input schedule, never through one system in it. So the schedule runs
/// here for real, in its real order, over a real [`Sim`]: `room::survey`
/// reads the graph, the charts and the amber latches stand where the
/// plan says, the instruments ride their cargo, `rig::steer` and
/// `rig::pose` fly the camera, the pointer is `surface::pick`'s own
/// answer, and [`advance`] routes the frame exactly as it does in the
/// window. A whole scripted session runs in well under a second, which
/// is why the monkey below can afford a hundred of them.
///
/// Three things stand in, and each is a window rather than a rule:
///
/// - **The meshes.** Nothing in the input path casts a ray at a mesh;
///   the charts and the latches carry their own quads.
/// - **The cursor pixel.** A focused cursor rests on a sim point, and
///   working out which one is the whole of what the window's viewport
///   arithmetic does. The script names the sim point instead.
/// - **`pieces::ride_pieces` and `menu::click`**, both private, both
///   re-said here through the public halves they are built from.
#[cfg(test)]
mod session {
    use bevy::input::InputPlugin;
    use bevy::time::TimeUpdateStrategy;
    use space_trucking::sim::cargo::Loc;
    use space_trucking::sim::room::{CABIN, RoomId, Tile};
    use space_trucking::sim::{Cue, ShipState, TICK_DT, Vec2 as SimVec2, layout, splitmix};

    use super::*;
    use crate::pieces::Riding;
    use crate::rig::{CabinCamera, CameraRig, EYE_HEIGHT, Focus, Mode, REACH};
    use crate::room::{Dress, Envelope, InRoom, Latch, Occupancy, Plan};
    use crate::surface::{Aimable, SimSurface, Station};

    /// Where the freed cursor rests while a station is focused, in sim
    /// coordinates. Roaming ignores it: there the crosshair is the
    /// camera, and the camera is the body.
    #[derive(Resource, Default)]
    struct Cursor(Option<SimVec2>);

    /// The buttons and keys the script is holding down. Edges are
    /// derived from it rather than injected, so a press that is never
    /// let go stays down exactly as a real one does.
    #[derive(Resource, Default)]
    struct Hands {
        left: bool,
        right: bool,
        keys: Vec<KeyCode>,
    }

    /// Turn the script's hands into this frame's edges. Runs first, after
    /// Bevy's own input pass has already cleared last frame's.
    fn hands(
        hands: Res<Hands>,
        mut mouse: ResMut<ButtonInput<MouseButton>>,
        mut keys: ResMut<ButtonInput<KeyCode>>,
    ) {
        for (want, button) in [
            (hands.left, MouseButton::Left),
            (hands.right, MouseButton::Right),
        ] {
            match (want, mouse.pressed(button)) {
                (true, false) => mouse.press(button),
                (false, true) => mouse.release(button),
                _ => {}
            }
        }
        let down: Vec<KeyCode> = keys.get_pressed().copied().collect();
        for key in down {
            if !hands.keys.contains(&key) {
                keys.release(key);
            }
        }
        for key in &hands.keys {
            if !keys.pressed(*key) {
                keys.press(*key);
            }
        }
    }

    /// What `room::rebuild` puts in the world that the input path reads:
    /// every room's six charts, and every doorway's amber latch.
    fn stage(mut commands: Commands, plan: Res<Plan>, standing: Query<Entity, With<InRoom>>) {
        if !plan.is_changed() {
            return;
        }
        for entity in &standing {
            commands.entity(entity).despawn();
        }
        for placed in &plan.rooms {
            let tag = InRoom {
                room: placed.id,
                kind: placed.kind,
            };
            for (station, surface) in placed.charts {
                commands.spawn((station, surface, tag));
            }
            for part in crate::room::seam_parts(placed) {
                if let Dress::Grab(room, face) = part.dress {
                    commands.spawn((Latch { room, face }, tag));
                }
            }
        }
    }

    /// The menu's scrim, which a screenless cabin grows no UI for: while
    /// the menu stands it covers the window, so a click on it puts the
    /// menu away and hands the cursor back (`menu::click`).
    fn scrim(
        mouse: Res<ButtonInput<MouseButton>>,
        mut menu: ResMut<menu::Menu>,
        mut rig: ResMut<CameraRig>,
    ) {
        if menu.open && mouse.just_pressed(MouseButton::Left) {
            menu.open = false;
            rig.parked = false;
        }
    }

    /// `pieces::ride_pieces`, through its two public halves: hang, move
    /// and retire the surfaces that ride the cargo.
    fn ride(
        mut commands: Commands,
        shell: Res<Shell>,
        charts: Query<(&Station, &SimSurface), Without<Riding>>,
        mut riders: Query<(Entity, &Riding, &Station, &mut SimSurface)>,
    ) {
        let charts: Vec<(Station, SimSurface)> = charts.iter().map(|(s, q)| (*s, *q)).collect();
        let sim = &shell.bridge.sim;
        let in_hand = sim.held(0).map(|held| held.piece);
        let mut live: Vec<(u32, Station, SimSurface)> = Vec::new();
        for piece in sim.pieces() {
            if in_hand == Some(piece.id) || !matches!(piece.loc, Loc::Hold { .. }) {
                continue;
            }
            let rect = layout::piece_rect(sim.rooms(), sim.pieces(), piece);
            if let Some((station, surface)) =
                crate::pieces::instrument_surface(&charts, piece.kind, rect)
            {
                live.push((piece.id, station, surface));
            }
            if let Some(face) = crate::pieces::standing_surface(&charts, piece.kind, rect) {
                live.push((piece.id, Station::Standing, face));
            }
        }
        for (entity, riding, station, mut surface) in &mut riders {
            if let Some(at) = live
                .iter()
                .position(|(id, tag, _)| *id == riding.0 && tag == station)
            {
                *surface = live.swap_remove(at).2;
            } else {
                commands.entity(entity).despawn();
            }
        }
        for (id, station, surface) in live {
            commands.spawn((station, surface, Riding(id)));
        }
    }

    /// `surface::track_pointer` without a window: the same two regimes,
    /// the same `pick`, aimed at a sim point instead of at a screen
    /// pixel.
    fn aim(
        rig: Res<CameraRig>,
        cursor: Res<Cursor>,
        camera: Single<&Transform, With<CabinCamera>>,
        surfaces: Query<(&Station, &SimSurface, Option<&Riding>, Option<&InRoom>)>,
        mut pointer: ResMut<VirtualPointer>,
    ) {
        *pointer = VirtualPointer::default();
        let aimables = || {
            surfaces
                .iter()
                .map(|(station, surface, riding, in_room)| Aimable {
                    station: *station,
                    surface: *surface,
                    riding: riding.is_some(),
                    in_room: in_room.copied(),
                })
        };
        let (ray, roam_only, reach) = if rig.interactive() {
            let Some(at) = cursor.0 else { return };
            let Some(world) = aimables()
                .filter(|aim| !aim.riding || !aim.station.roamable())
                .find(|aim| aim.surface.rect.contains(at))
                .map(|aim| aim.surface.to_world(at))
            else {
                return;
            };
            let Ok(dir) = Dir3::new(world - camera.translation) else {
                return;
            };
            (Ray3d::new(camera.translation, dir), false, f32::INFINITY)
        } else if rig.roaming() {
            let Ok(dir) = Dir3::new(camera.forward().into()) else {
                return;
            };
            (Ray3d::new(camera.translation, dir), true, REACH)
        } else {
            return;
        };
        *pointer = crate::surface::pick(ray, roam_only, reach, aimables());
    }

    /// A cabin a test can play.
    struct Cabin {
        app: App,
    }

    impl Cabin {
        fn new(save: &str) -> Self {
            let mut bridge = Bridge::boot_fixture(save);
            bridge.steady();
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, InputPlugin))
                .insert_resource(TimeUpdateStrategy::ManualDuration(FRAME_STEP))
                .insert_resource(Shell {
                    bridge,
                    outcome: FrameOutcome::default(),
                    muted: false,
                })
                .insert_resource(CameraRig::boot(None))
                .insert_resource(menu::Menu::boot(false))
                .init_resource::<VirtualPointer>()
                .init_resource::<Cursor>()
                .init_resource::<Hands>()
                .init_resource::<gesture::Grips>()
                .init_resource::<Plan>()
                .init_resource::<Envelope>()
                .init_resource::<Occupancy>()
                .init_resource::<crate::room::AimedLatch>()
                .configure_sets(Update, (Phase::Input, Phase::Advance).chain())
                .add_systems(
                    Update,
                    (
                        hands,
                        crate::room::survey,
                        stage,
                        keep_aboard,
                        crate::room::occupy,
                        crate::room::aim_latch,
                        ride,
                        menu::keys,
                        scrim,
                        crate::rig::steer,
                        crate::rig::pose,
                        aim,
                        gesture::grip,
                    )
                        .chain()
                        .in_set(Phase::Input),
                )
                .add_systems(Update, advance.in_set(Phase::Advance));
            app.world_mut().spawn((CabinCamera, Transform::default()));
            let mut cabin = Self { app };
            cabin.steps(3);
            cabin
        }

        /// The developer fixture, moored at Venus, with the player's own
        /// goods walked home out of the market — the board the gangway
        /// law will actually let you part.
        fn at_venus() -> Self {
            let mut cabin = Self::new(&crate::fixture::docked_at(0));
            let rooms: Vec<RoomId> = cabin.latches().iter().map(|(room, _)| *room).collect();
            for room in rooms {
                cabin.send_home(room);
            }
            cabin.steps(3);
            cabin
        }

        fn step(&mut self) {
            self.app.update();
        }

        fn steps(&mut self, n: u32) {
            for _ in 0..n {
                self.step();
            }
        }

        fn sim(&self) -> &space_trucking::sim::Sim {
            &self.app.world().resource::<Shell>().bridge.sim
        }

        fn rig(&mut self) -> Mut<'_, CameraRig> {
            self.app.world_mut().resource_mut::<CameraRig>()
        }

        fn pos(&self) -> Vec3 {
            self.app.world().resource::<CameraRig>().pos
        }

        fn roaming(&self) -> bool {
            self.app.world().resource::<CameraRig>().roaming()
        }

        /// Stand the body somewhere, looking at a point.
        fn stand(&mut self, at: Vec3, toward: Vec3) {
            self.rig().pos = at;
            self.look(toward);
        }

        /// Look toward a world point without moving. The mouse aims
        /// anywhere; this is the only thing it does.
        fn look(&mut self, toward: Vec3) {
            let from = self.pos();
            let d = toward - from;
            let mut rig = self.rig();
            rig.yaw = (-d.x).atan2(-d.z);
            rig.pitch = d.y.atan2(d.xz().length()).clamp(-1.2, 1.2);
        }

        fn hold_left(&mut self, down: bool) {
            self.app.world_mut().resource_mut::<Hands>().left = down;
        }

        fn hold_right(&mut self, down: bool) {
            self.app.world_mut().resource_mut::<Hands>().right = down;
        }

        fn hold_keys(&mut self, keys: &[KeyCode]) {
            self.app.world_mut().resource_mut::<Hands>().keys = keys.to_vec();
        }

        fn rest_cursor(&mut self, at: Option<SimVec2>) {
            self.app.world_mut().resource_mut::<Cursor>().0 = at;
        }

        /// One click: down for a frame, up the next, with everything the
        /// sim said across the two.
        fn click(&mut self) -> Vec<Cue> {
            let mut said = Vec::new();
            self.hold_left(true);
            self.step();
            said.extend_from_slice(self.sim().cues());
            self.hold_left(false);
            self.step();
            said.extend_from_slice(self.sim().cues());
            said
        }

        /// Every amber latch standing this frame: the room it asks to
        /// part, and where its face is.
        fn latches(&mut self) -> Vec<(RoomId, Vec3)> {
            self.app
                .world_mut()
                .query::<&Latch>()
                .iter(self.app.world())
                .map(|latch| (latch.room, latch.face.center))
                .collect()
        }

        /// Whichever surface answers as `want` this frame.
        fn face(&mut self, want: Station) -> Option<SimSurface> {
            self.app
                .world_mut()
                .query::<(&Station, &SimSurface)>()
                .iter(self.app.world())
                .find(|(station, _)| **station == want)
                .map(|(_, surface)| *surface)
        }

        /// Walk everything of the player's out of `room` the way a
        /// shift-click quick-move does. Board setup, straight through
        /// the bridge: not the path under test.
        fn send_home(&mut self, room: RoomId) {
            for _ in 0..40 {
                let Some(at) = self.stray_in(room) else {
                    return;
                };
                let mut shell = self.app.world_mut().resource_mut::<Shell>();
                shell.bridge.frame(
                    TICK_DT,
                    &FrameInput {
                        pointer: at,
                        press: true,
                        held: true,
                        shift: true,
                        ..FrameInput::default()
                    },
                );
                shell.bridge.frame(TICK_DT, &FrameInput::default());
            }
        }

        /// The middle of some piece in `room` that is not the room's own
        /// stock, if one is left.
        fn stray_in(&self, room: RoomId) -> Option<SimVec2> {
            let sim = self.sim();
            let rect = sim
                .pieces()
                .iter()
                .find(|piece| {
                    matches!(piece.loc, Loc::Hold { room: at, x, y }
                        if at == room && sim.rooms().tile(at, x, y) != Some(Tile::Stock))
                })
                .map(|piece| layout::piece_rect(sim.rooms(), sim.pieces(), piece))?;
            Some(SimVec2::new(
                rect.w.mul_add(0.5, rect.x),
                rect.h.mul_add(0.5, rect.y),
            ))
        }

        /// **Can the player still act?** Let go of everything, step out
        /// of whatever the camera is in, walk up to the chart tank,
        /// focus it, and chart a course. Every step is something a
        /// player does with the hardware they have; the mouse alone is
        /// enough for all of it, which is why no key is pressed here.
        fn can_still_chart(&mut self) -> Result<(), String> {
            self.hold_left(false);
            self.hold_right(false);
            self.hold_keys(&[]);
            self.rest_cursor(None);
            self.steps(4);
            for _ in 0..10 {
                if self.roaming() {
                    break;
                }
                // A left click reclaims a parked cursor and dismisses the
                // menu; a right click steps out of a station.
                self.click();
                self.hold_right(true);
                self.step();
                self.hold_right(false);
                self.steps(30);
            }
            if !self.roaming() {
                return Err(format!(
                    "the camera never came back: {:?}",
                    self.app.world().resource::<CameraRig>().mode
                ));
            }
            let Some(map) = self.face(Station::Map) else {
                return Err("no chart tank aboard".into());
            };
            let stand = map.center + map.normal() * 0.75;
            let goal = Vec3::new(stand.x, EYE_HEIGHT, stand.z);
            for _ in 0..1200 {
                let here = self.pos();
                if here.with_y(0.0).distance(goal.with_y(0.0)) < 0.25 {
                    break;
                }
                self.look(goal.with_y(here.y));
                self.hold_keys(&[KeyCode::KeyW]);
                self.step();
            }
            self.hold_keys(&[]);
            self.look(map.center);
            self.steps(3);
            self.click();
            self.steps(60);
            if !matches!(
                self.app.world().resource::<CameraRig>().mode,
                Mode::Focused { focus: Focus::Tank }
            ) {
                return Err(format!(
                    "a click on the tank did not focus it, from {:?}",
                    self.pos()
                ));
            }
            let ShipState::Docked(here) = self.sim().ship().state else {
                return Err("the ship left the dock".into());
            };
            let Some(target) = (0..12u8).find(|&id| id != here && self.sim().poi_chartable(id))
            else {
                return Err("nothing is chartable".into());
            };
            let at = self.sim().poi_pos(target);
            self.rest_cursor(Some(at));
            self.steps(2);
            self.click();
            if self.sim().ship().selected == Some(target) {
                Ok(())
            } else {
                Err(format!(
                    "a press on the tank's glass selected {:?}, not {target}",
                    self.sim().ship().selected
                ))
            }
        }
    }

    /// **A seam never shuts on the body.**
    ///
    /// The gangway law refuses to part a room that holds you, and it
    /// asks one question to find out: which room is the body in
    /// (docs/ROOMS.md, "The one new input field"). A body in a doorway
    /// is in neither room's box, so `room::occupy` answers with the room
    /// it came from — and the seam it is standing in is not that room.
    /// The gate passes, the connector stops existing, and the body is
    /// left in the vacuum where the gangway was, out of
    /// [`REACH`] of every surface in the ship: mouse look
    /// still works, walking still works, and every left click in the
    /// cabin lands on nothing at all.
    ///
    /// So the law is about where the body ENDS UP, not about which press
    /// is allowed: whatever a seam does, the body is still standing
    /// somewhere the ship offers ([`keep_aboard`]). This is asserted from
    /// every threshold a body can click a latch from, because the one
    /// that strands you is the one nobody thought to stand on.
    #[test]
    fn a_seam_never_shuts_on_the_body() {
        let mut cabin = Cabin::at_venus();
        let (room, latch) = cabin.latches()[0];
        // Every point of the connector across that seam that lies in no
        // room's own box, and is close enough to work the latch from.
        let thresholds: Vec<Vec3> = {
            let world = cabin.app.world();
            let plan = world.resource::<Plan>();
            let envelope = world.resource::<Envelope>();
            envelope
                .seams
                .iter()
                .flat_map(|(lo, hi)| {
                    (0..=20u8).map(move |k| {
                        let t = f32::from(k) / 20.0;
                        Vec3::new(
                            f32::midpoint(lo.x, hi.x),
                            EYE_HEIGHT,
                            (hi.z - lo.z).mul_add(t, lo.z),
                        )
                    })
                })
                .filter(|p| plan.room_at(*p).is_none() && p.distance(latch) < REACH - 0.4)
                .collect()
        };
        assert!(
            !thresholds.is_empty(),
            "a doorway a body can work the latch from is the whole case"
        );
        let mut parted = 0;
        for spot in thresholds {
            let mut cabin = Cabin::at_venus();
            cabin.stand(spot, latch);
            // The eye ducks under a doorway's lintel, so aim again from
            // wherever it settles rather than from where it started.
            cabin.steps(8);
            cabin.look(latch);
            cabin.steps(2);
            let said = cabin.click();
            if !said.iter().any(|cue| matches!(cue, Cue::Parted)) {
                continue;
            }
            parted += 1;
            cabin.steps(2);
            let inside = {
                let world = cabin.app.world();
                world
                    .resource::<Envelope>()
                    .holds(world.resource::<CameraRig>().pos)
            };
            assert!(
                inside,
                "parting {room} from {spot:?} left the body at {:?}, which is not aboard",
                cabin.pos()
            );
            assert!(
                cabin.can_still_chart().is_ok(),
                "parting {room} from {spot:?} left the player unable to chart"
            );
        }
        assert!(parted > 0, "no threshold click ever parted the seam");
    }

    /// **Nothing a pair of hands can do leaves the player unable to
    /// act.**
    ///
    /// The cabin monkey, per the drag-monkey tradition the 2D prototype
    /// started and `gesture::tests::gesture_monkey_mask_integrity` keeps:
    /// seeded pseudo-random hands on the real hardware — look, walk,
    /// click, right-click, `E`, `Esc`, and a standing bias toward
    /// whatever amber latch is in the room, so seams really do part
    /// under it. However the session ends, the player can still walk up
    /// to the chart tank and chart a course with the mouse alone.
    ///
    /// The claim is deliberately end to end rather than per-system.
    /// Every lockup this file has had was a state no single system was
    /// wrong about: a grip nobody released, an aim that outlived its
    /// room, a pose in a wall. What they share is the sentence below.
    #[test]
    fn no_pair_of_hands_leaves_the_player_unable_to_act() {
        let mut parted = 0;
        for run in 0..48u64 {
            let seed = splitmix(0xBADD_C0DE, run);
            let mut cabin = Cabin::at_venus();
            for i in 0..400u64 {
                let h = splitmix(seed, i);
                let bit = |n: u32| (h >> n) & 1 == 1;
                cabin.hold_left(bit(0) || bit(1));
                cabin.hold_right(bit(2) && bit(3) && bit(4));
                let mut keys = Vec::new();
                for (n, key) in [
                    (5, KeyCode::KeyW),
                    (7, KeyCode::KeyA),
                    (9, KeyCode::KeyS),
                    (11, KeyCode::KeyD),
                ] {
                    if bit(n) && bit(n + 1) {
                        keys.push(key);
                    }
                }
                if bit(13) && bit(14) && bit(15) {
                    keys.push(KeyCode::KeyE);
                }
                if bit(16) && bit(17) && bit(18) && bit(19) {
                    keys.push(KeyCode::Escape);
                }
                cabin.hold_keys(&keys);
                // Where the eyes go: mostly a wander, sometimes straight
                // at a latch from arm's length, which is the only way a
                // seam ever parts.
                let latches = cabin.latches();
                if !latches.is_empty() && bit(20) && bit(21) {
                    let at = latches[(h >> 32) as usize % latches.len()].1;
                    let step = (at - cabin.pos()).normalize_or_zero() * 0.6;
                    cabin.stand(Vec3::new(at.x - step.x, EYE_HEIGHT, at.z - step.z), at);
                } else {
                    let mut rig = cabin.rig();
                    rig.yaw = ((h >> 40) & 0xFF) as f32 / 255.0 * std::f32::consts::TAU;
                    rig.pitch = (((h >> 48) & 0xFF) as f32 / 255.0 - 0.5) * 2.0;
                }
                cabin.rest_cursor(Some(SimVec2::new(
                    ((h >> 24) & 0x3FF) as f32,
                    ((h >> 34) & 0x1FF) as f32,
                )));
                cabin.step();
                parted += u32::from(cabin.sim().cues().iter().any(|c| matches!(c, Cue::Parted)));
            }
            if let Err(why) = cabin.can_still_chart() {
                panic!("run {run}: {why}");
            }
        }
        assert!(
            parted > 0,
            "the monkey never parted a seam, so it never tested one"
        );
    }

    /// **A detached room takes nothing of the cabin's with it.** The
    /// owner's own report, scripted: docked at Venus, work the seam's
    /// amber latch from inside the cabin, and then chart a course.
    #[test]
    fn the_map_still_charts_after_the_market_is_sent_away() {
        let mut cabin = Cabin::at_venus();
        let (room, latch) = cabin.latches()[0];
        // Arm's length off the latch, on the cabin's side of it.
        let inboard = {
            let placed = cabin
                .app
                .world()
                .resource::<Plan>()
                .get(CABIN)
                .expect("the cabin")
                .clone();
            let middle = (placed.lo + placed.hi) * 0.5;
            let toward = (middle - latch).normalize_or_zero() * 0.9;
            Vec3::new(latch.x + toward.x, EYE_HEIGHT, latch.z + toward.z)
        };
        cabin.stand(inboard, latch);
        cabin.steps(3);
        cabin.look(latch);
        cabin.steps(2);
        let said = cabin.click();
        assert!(
            said.iter().any(|cue| matches!(cue, Cue::Parted)),
            "the latch did not part room {room}: {said:?}"
        );
        assert!(cabin.sim().rooms().get(room).is_none());
        assert_eq!(cabin.app.world().resource::<Occupancy>().0, CABIN);
        cabin
            .can_still_chart()
            .expect("the market left with the map");
    }
}
