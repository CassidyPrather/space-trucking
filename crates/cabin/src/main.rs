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
mod art;
mod audio;
mod bridge;
mod canvas;
mod console;
mod crt;
mod fixture;
mod fx;
mod gesture;
mod glow;
mod keys;
mod menu;
mod outline;
mod palette;
mod pieces;
mod poi;
mod rig;
mod room;
mod surface;
mod viewport;
mod wear;

use std::time::Duration;

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
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

/// Pin the clock for a run that has to reproduce. Called by `--shot`, and
/// by nothing else.
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
/// twice is the same picture.
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
    // wants capturing without a hand on the keyboard. `--menu keys` opens
    // it on the carry's keys page, whose layout is a thing a screenshot is
    // for (`menu::Page::Keys`).
    let open_menu = args.iter().any(|arg| arg == "--menu");
    let menu_page = if flag_value("--menu").as_deref() == Some("keys") {
        menu::Page::Keys
    } else {
        menu::Page::Reading
    };
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
    let mut bridge = panes.map_or_else(
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
    // A run that judges a picture — the screenshot — must give
    // the same answer twice, so it counts its clock instead of measuring
    // it: the frames come off a fixed step ([`FRAME_STEP`]) and the world
    // stops reading the wall (`Bridge::steady`).
    let judged = shot.is_some();
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
            })
            // **The art cache is the asset root**, and only under the
            // feature that has an asset to load. Nothing else in this
            // game reads a file through the asset server — every mesh and
            // texture is cut in code — so there is no `assets/` directory
            // to share, and pointing the server straight at
            // `cargo xtask art resolve`'s output means a path out of the
            // index is a path the server takes verbatim.
            .set({
                #[cfg(not(feature = "whitebox"))]
                {
                    AssetPlugin {
                        file_path: art::cache_root().display().to_string(),
                        ..default()
                    }
                }
                #[cfg(feature = "whitebox")]
                {
                    AssetPlugin::default()
                }
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
    .insert_resource(menu::Menu::boot(open_menu).on(menu_page))
    // The carry's keys, as the player last left them: read from the file
    // beside the save, defaults where there is none (`crate::keys`).
    .insert_resource(keys::Bindings::boot())
    .insert_resource(grouping)
    .add_plugins((
        airlock::AirlockPlugin,
        audio::AudioPlugin,
        crt::CrtPlugin,
        fx::FxPlugin,
        gesture::GesturePlugin,
        menu::MenuPlugin,
        outline::OutlinePlugin,
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
    .add_systems(Update, advance.in_set(Phase::Advance));
    // **The seam.** Off in the default build and not merely inert there:
    // the half of `art` that reads a cache does not exist without the
    // feature, so a whitebox build has no line of code that could open a
    // file the licence keeps out of this repository.
    #[cfg(not(feature = "whitebox"))]
    art::plugin(&mut app);
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

/// Let the scene settle, capture the window once, exit when the write
/// lands. Drives the in-container visual verification loop.
fn shoot(
    mut commands: Commands,
    mut mode: ResMut<ShotMode>,
    capturing: Query<(), With<bevy::render::view::screenshot::Capturing>>,
    mut exit: MessageWriter<AppExit>,
    #[cfg(not(feature = "whitebox"))] dressed: Option<Res<art::Dressed>>,
    #[cfg(not(feature = "whitebox"))] assets: Res<AssetServer>,
) {
    mode.frames += 1;
    // **A dressed build waits for its art.** The settle is forty-five
    // pinned frames, and a purchased scene lands whenever the loader has
    // read its file, which on a slow disk or a small machine is later
    // than that — and a shot of a room whose walls are still on their
    // way in is a picture of the void. The whitebox build has nothing to
    // wait for, and a scene that failed stops nobody: it is already a
    // sentence on stderr and a whitebox in its place.
    #[cfg(not(feature = "whitebox"))]
    let settled = dressed.as_deref().is_none_or(|dressed| {
        dressed.scenes().all(|scene| {
            !matches!(
                assets.recursive_dependency_load_state(scene),
                bevy::asset::RecursiveDependencyLoadState::NotLoaded
                    | bevy::asset::RecursiveDependencyLoadState::Loading
            )
        })
    });
    #[cfg(feature = "whitebox")]
    let settled = true;
    if !mode.fired && mode.frames > SHOT_SETTLE && settled {
        mode.fired = true;
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                mode.path.clone(),
            ));
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
    scroll: Res<AccumulatedMouseScroll>,
    pointer: Res<VirtualPointer>,
    camera: Res<rig::CameraRig>,
    grips: Res<gesture::Grips>,
    occupancy: Res<room::Occupancy>,
    latch: Res<room::AimedLatch>,
    bindings: Res<keys::Bindings>,
    mut menu: ResMut<menu::Menu>,
    mut shell: ResMut<Shell>,
) {
    let live = camera.interactive();
    let holding = shell.bridge.sim.held(0).is_some();
    // The detach gesture: a roam click on a door's amber latch asks the
    // input schedule to part that seam, and consumes the click so it
    // never doubles as a grab. Empty-handed only — a hand full of cargo
    // is exactly the hand the gangway law refuses. The latch has already
    // lost to anything standing nearer along the same ray
    // (`room::aim_latch`), so the click spent here is a click that was
    // aimed at the latch and at nothing else.
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
        // Cargo in hand and no crosshair to carry it by: the menu or the
        // desktop has the cursor, or the camera is walking back out of a
        // focus a press there lifted something in (`rig::steer`). A
        // frame without a held signal would snap the piece home (the
        // sim's phantom-pointer guard), so the grip keeps synthesizing
        // until the body roams again and the carry goes on.
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
    // still the only thing that decides what pausing means. The new run
    // arrives the same way and has no key beside it: the labelled bar on
    // the menu is the only thing that starts one (`crate::menu`).
    let worked = menu.take();
    // The carry turns and lifts while the body roams with it, and at no
    // other time: an open menu parks the roam and a focus leaves it, and
    // there the wheel and the carry's keys do what they always did, which
    // is nothing.
    let hands = if holding && camera.roaming() {
        hands(&keys, &scroll, &bindings)
    } else {
        bridge::Hands::default()
    };
    // **The piece the press means**, resolved where the hover and the
    // outline resolve it (`VirtualPointer::aimed`): with pieces sharing
    // ground the point alone cannot say, and the sim takes this aim only
    // among what the pointer is on. Off the roam there is no body to have
    // met — a focus works panels, a glide and a latch press nothing.
    let aim = (press && camera.roaming())
        .then(|| pointer.aimed(&shell.bridge.sim).map(|piece| piece.id))
        .flatten();
    let input = FrameInput {
        pointer: at,
        press,
        aim,
        held,
        release,
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        key_pause: keys.just_pressed(KeyCode::Space),
        key_warp: keys.just_pressed(KeyCode::KeyF),
        key_mute: keys.just_pressed(KeyCode::KeyM),
        menu_pause: worked.pause,
        menu_warp: worked.warp,
        menu_mute: worked.mute,
        menu_reseed: worked.reseed,
        occupied: occupancy.0,
        detach: parting,
        hands,
    };
    let outcome = shell.bridge.frame(time.delta_secs(), &input);
    if outcome.toggle_mute {
        shell.muted = !shell.muted;
    }
    shell.outcome = outcome;
}

/// **The hands on the carry**, as the bridge takes them (`bridge::Hands`;
/// docs/BAY.md, "Cargo turns" and "Lift, and the keys").
///
/// - **The wheel**, rolled up and away from you, turns the carry
///   counter-clockwise as seen from the room — as you face a wall, for a
///   piece bound for one — and down turns it back. Each notch turns to
///   the next multiple of fifteen degrees that way: the convenient
///   angles, offered by the tool and never by the sim. A wheel that
///   reports pixels (a touchpad, a smooth wheel) is read at
///   `MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR` to the notch.
/// - **`Ctrl` + wheel** turns one degree a notch from wherever the carry
///   is, with nothing to snap to, so every angle is a few notches away.
/// - **`Shift` + wheel** lifts instead: up away from the surface for a
///   notch rolled up, back toward it rolled down, a sixteenth of a cell
///   a notch, and a single fine unit with `Ctrl` too. `Shift` is
///   quick-move, and quick-move is read on a press — a press that lifts —
///   so a hand already carrying has no other use for it until the drop.
/// - **The carry's keys** are read off the binding table
///   (`crate::keys`), never by name: by default `Q` turns as the wheel
///   rolled up does and `E` the other way, and `X` raises and `Z` lowers
///   a sixteenth of a cell a press. Two keys of one pair pressed together
///   cancel.
fn hands(
    keys: &ButtonInput<KeyCode>,
    scroll: &AccumulatedMouseScroll,
    bindings: &keys::Bindings,
) -> bridge::Hands {
    use keys::Action;

    let held = |pair: [KeyCode; 2]| pair.into_iter().any(|key| keys.pressed(key));
    let step = |up: Action, down: Action| {
        i8::from(bindings.pressed(keys, up)) - i8::from(bindings.pressed(keys, down))
    };
    bridge::Hands {
        wheel: match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => {
                scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
            }
        },
        fine: held([KeyCode::ControlLeft, KeyCode::ControlRight]),
        lifting: held([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        turn: step(Action::TurnCcw, Action::TurnCw),
        lift: step(Action::Raise, Action::Lower),
    }
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
/// that stand outside the hull on purpose (`--view drydock|berth`) —
/// neither is somebody who has to be able to click something, and a
/// fence would drag both back inside.
fn keep_aboard(
    envelope: Res<room::Envelope>,
    mut rig: ResMut<rig::CameraRig>,
    mut was_aboard: Local<bool>,
) {
    if rig.drydock || envelope.rooms.is_empty() {
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
    use bevy::input::mouse::MouseWheel;
    use bevy::input::touch::TouchPhase;
    use bevy::time::TimeUpdateStrategy;
    use space_trucking::sim::Turn;
    use space_trucking::sim::cargo::Loc;
    use space_trucking::sim::room::{CABIN, RoomId, Tile};
    use space_trucking::sim::{
        Cue, Kind, ShipState, Surf, TICK_DT, Vec2 as SimVec2, cargo, layout, splitmix,
    };

    use super::*;
    use crate::bridge::LIFT_STEP;
    use crate::pieces::Riding;
    use crate::rig::{CabinCamera, CameraRig, EYE_HEIGHT, Focus, Mode, REACH};
    use crate::room::{Dress, Envelope, InRoom, Latch, Occupancy, Plan};
    use crate::surface::{Aimable, SimSurface, Station};

    /// Where the freed cursor rests while a station is focused, in sim
    /// coordinates. Roaming ignores it: there the crosshair is the
    /// camera, and the camera is the body.
    #[derive(Resource, Default)]
    struct Cursor(Option<SimVec2>);

    /// The fine centre of the cabin deck's middle cell, `across` cells to
    /// its right on the net.
    fn deck_middle(across: u8) -> (u16, u16) {
        let (fx, fy, fw, fh) = RoomKind::Cabin.floor_rect();
        let fine = |cell: u8| cargo::fine(cell) + cargo::FINE / 2;
        (fine(fx + fw / 2 + across), fine(fy + fh / 2))
    }

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
            let rect = layout::piece_rect(sim.rooms(), piece);
            if let Some((station, surface)) = crate::pieces::instrument_surface(
                &charts,
                piece.kind,
                rect,
                crate::pieces::berth_at(piece),
            ) {
                live.push((piece.id, station, surface));
            }
            if let Some(face) = crate::pieces::standing_surface(
                &charts,
                piece.kind,
                rect,
                crate::pieces::berth_at(piece),
            ) {
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
    /// the same `pick` told the same about the hand, aimed at a sim point
    /// instead of at a screen pixel.
    fn aim(
        rig: Res<CameraRig>,
        cursor: Res<Cursor>,
        shell: Res<Shell>,
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
                    riding: riding.map(|riding| riding.0),
                    in_room: in_room.copied(),
                })
        };
        let (ray, roam_only, reach) = if rig.interactive() {
            let Some(at) = cursor.0 else { return };
            let Some(world) = aimables()
                .filter(|aim| aim.riding.is_none() || !aim.station.roamable())
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
        let holding = shell.bridge.sim.held(0).is_some();
        *pointer = crate::surface::pick(ray, roam_only, reach, holding, aimables());
    }

    /// Two places to stand and one latch: the spot where a piece
    /// eclipses it, the piece doing the eclipsing, and a spot where the
    /// same latch is clear.
    #[derive(Clone, Copy)]
    struct Eclipse {
        clear: Vec3,
        eye: Vec3,
        latch: Latch,
        piece: u32,
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
                .insert_resource(crate::keys::Bindings::default())
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
                        ride,
                        menu::keys,
                        scrim,
                        crate::rig::steer,
                        crate::rig::pose,
                        aim,
                        crate::room::aim_latch,
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

        /// Every mapped quad standing this frame, as the pick sees them
        /// — the same list `aim` hands [`crate::surface::pick`].
        fn aimables(&mut self) -> Vec<Aimable> {
            self.app
                .world_mut()
                .query::<(&Station, &SimSurface, Option<&Riding>, Option<&InRoom>)>()
                .iter(self.app.world())
                .map(|(station, surface, riding, in_room)| Aimable {
                    station: *station,
                    surface: *surface,
                    riding: riding.map(|riding| riding.0),
                    in_room: in_room.copied(),
                })
                .collect()
        }

        /// **Somewhere a body can stand where a piece eclipses a latch**,
        /// and somewhere it can stand where the same latch is clear.
        /// Both are eye positions in the cabin, inside the walk envelope,
        /// with the latch within [`REACH`] straight ahead; at the
        /// eclipsed one the crosshair rests on a piece's own body nearer
        /// along that very line, at the clear one nothing is in the way.
        ///
        /// Searched over the board rather than written down as
        /// coordinates: the fixture is re-dressed whenever the cargo
        /// tables change, and spots spelled out here would quietly stop
        /// being the spots.
        fn eclipsed_latch(&mut self) -> Option<Eclipse> {
            let aims = self.aimables();
            let latches: Vec<Latch> = self
                .app
                .world_mut()
                .query::<&Latch>()
                .iter(self.app.world())
                .copied()
                .collect();
            let spots: Vec<Vec3> = {
                let world = self.app.world();
                let envelope = world.resource::<Envelope>();
                let plan = world.resource::<Plan>();
                let placed = plan.get(CABIN)?;
                let (lo, hi) = (placed.lo, placed.hi);
                let step =
                    |a: f32, b: f32, n: u8| (b - a - 0.7).mul_add(f32::from(n) / 32.0, a + 0.35);
                (0..=32u8)
                    .flat_map(|i| (0..=32u8).map(move |k| (i, k)))
                    .map(|(i, k)| Vec3::new(step(lo.x, hi.x, i), EYE_HEIGHT, step(lo.z, hi.z, k)))
                    .filter(|eye| envelope.holds(*eye) && plan.room_at(*eye) == Some(CABIN))
                    .collect()
            };
            let sim = self.sim();
            let mut eclipsed: Option<(Vec3, Latch, u32)> = None;
            let mut clear: Option<Vec3> = None;
            for eye in spots {
                for latch in &latches {
                    let d = latch.face.center - eye;
                    // Inside the neck's own pitch, so the aim below is an
                    // aim the body can actually take.
                    if d.y.atan2(d.xz().length()).abs() > crate::rig::PITCH_LIMIT - 0.3 {
                        continue;
                    }
                    let Ok(dir) = Dir3::new(d) else { continue };
                    let ray = Ray3d::new(eye, dir);
                    // The latch is in reach — so it is a latch this press
                    // could have gone to.
                    let Some((behind, _, _)) = latch.face.project(ray) else {
                        continue;
                    };
                    if behind > REACH {
                        continue;
                    }
                    let pointer =
                        crate::surface::pick(ray, true, REACH, false, aims.iter().copied());
                    let nearer = pointer.depth < behind;
                    if !nearer {
                        clear.get_or_insert(eye);
                        continue;
                    }
                    // The crosshair is resting on a piece's own body in
                    // front of the latch.
                    if eclipsed.is_none()
                        && pointer.station == Some(Station::Standing)
                        && let Some(piece) = pointer.aimed(sim)
                    {
                        eclipsed = Some((eye, *latch, piece.id));
                    }
                }
                if let (Some((eye, latch, piece)), Some(clear)) = (eclipsed, clear) {
                    return Some(Eclipse {
                        clear,
                        eye,
                        latch,
                        piece,
                    });
                }
            }
            None
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

        /// Roll the wheel `notches` this frame, the way a mouse reports
        /// it: in lines, positive up and away.
        fn wheel(&mut self, notches: f32) {
            self.app.world_mut().write_message(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: notches,
                window: Entity::PLACEHOLDER,
                phase: TouchPhase::Moved,
            });
        }

        /// The facing the carry is sent at.
        fn facing(&self) -> Turn {
            self.app.world().resource::<Shell>().bridge.facing()
        }

        /// **A cabin with one crate aboard**, standing in the middle of the
        /// deck facing the front, and a body standing over it close enough
        /// to reach it, looking at it: the crate's id, and the deck point it
        /// stands on.
        fn over_a_crate() -> (Self, u32, SimVec2) {
            let (x, y) = deck_middle(0);
            let mut cabin = Self::furnished(&[(Kind::ScrapAlloy, x, y)], &[]);
            let at = cabin.stand_over(0);
            (cabin, 0, at)
        }

        /// **A cabin furnished with exactly `pieces`**, each `(kind, x, y)`
        /// standing on the deck centred on fine `(x, y)` at no turn and no
        /// lift, ids in order from nought — and after them whatever of the
        /// starting board's own `kept` kinds it stands, where it stands
        /// them.
        fn furnished(pieces: &[(Kind, u16, u16)], kept: &[Kind]) -> Self {
            use std::fmt::Write as _;

            let starter = space_trucking::sim::Sim::new(1).save_string();
            let mut save = String::new();
            let mut next = 0;
            for line in starter.lines() {
                if line.starts_with("piece ") {
                    continue;
                }
                if line.starts_with("next_piece") {
                    // Writing into a String cannot fail.
                    for (kind, x, y) in pieces {
                        let _ = writeln!(
                            save,
                            "piece {next} {} 0 hold {CABIN} {x} {y} 0 0",
                            kind.index()
                        );
                        next += 1;
                    }
                    for line in starter.lines().filter(|line| line.starts_with("piece ")) {
                        let mut words = line.split_whitespace().skip(2);
                        let kind = words.next().and_then(|word| word.parse::<usize>().ok());
                        if kept.iter().any(|keep| Some(keep.index()) == kind) {
                            let rest = line.split_whitespace().skip(2).collect::<Vec<_>>();
                            let _ = writeln!(save, "piece {next} {}", rest.join(" "));
                            next += 1;
                        }
                    }
                    let _ = writeln!(save, "next_piece {next}");
                    continue;
                }
                save.push_str(line);
                save.push('\n');
            }
            Self::new(&save)
        }

        /// Stand beside piece `id`, close enough to reach it, looking at
        /// the middle of its ground: that point, on the deck.
        fn stand_over(&mut self, id: u32) -> SimVec2 {
            let piece = *self
                .sim()
                .pieces()
                .iter()
                .find(|piece| piece.id == id)
                .expect("aboard");
            let at = layout::piece_rect(self.sim().rooms(), &piece);
            let at = SimVec2::new(at.w.mul_add(0.5, at.x), at.h.mul_add(0.5, at.y));
            let floor = self.face(Station::BayFloor).expect("the cabin's deck");
            let target = floor.to_world(at);
            let eye = [Vec3::Z, Vec3::NEG_Z, Vec3::X, Vec3::NEG_X]
                .into_iter()
                .map(|way| Vec3::new(target.x, EYE_HEIGHT, target.z) + way * 0.9)
                .find(|eye| self.app.world().resource::<Envelope>().holds(*eye))
                .expect("somewhere to stand beside the piece");
            self.stand(eye, target);
            self.steps(3);
            at
        }

        /// Stand arm's length in front of piece `id`'s own pick face,
        /// looking at its middle.
        fn stand_before(&mut self, id: u32) {
            let face = self
                .riding(id, Station::Standing)
                .expect("the piece carries its own pick face");
            let eye = face.center + Station::Standing.inward(&face) * 0.75;
            self.stand(Vec3::new(eye.x, EYE_HEIGHT, eye.z), face.center);
            self.steps(3);
        }

        /// **A line through piece `id`'s body that comes down on bare deck
        /// beyond it**: an eye a body can stand at, beside the piece, and
        /// the deck point past the piece's far side the line from it ends
        /// on, inside [`REACH`]. Half way along, the line passes over the
        /// middle of the piece's ground at half the eye's height.
        fn through(&mut self, id: u32) -> (Vec3, Vec3) {
            let piece = *self
                .sim()
                .pieces()
                .iter()
                .find(|piece| piece.id == id)
                .expect("aboard");
            let at = layout::piece_rect(self.sim().rooms(), &piece);
            let at = SimVec2::new(at.w.mul_add(0.5, at.x), at.h.mul_add(0.5, at.y));
            let middle = self
                .face(Station::BayFloor)
                .expect("the cabin's deck")
                .to_world(at);
            [Vec3::Z, Vec3::NEG_Z, Vec3::X, Vec3::NEG_X]
                .into_iter()
                .map(|way| {
                    (
                        Vec3::new(middle.x, EYE_HEIGHT, middle.z) + way * 0.6,
                        middle - way * 0.6,
                    )
                })
                .find(|(eye, _)| self.app.world().resource::<Envelope>().holds(*eye))
                .expect("somewhere to stand beside the piece")
        }

        /// This frame's pointer.
        fn pointer(&self) -> VirtualPointer {
            *self.app.world().resource::<VirtualPointer>()
        }

        /// What the room alone reads along `ray`: the pick with every
        /// surface that rides a piece taken away.
        fn room_alone(&mut self, ray: Ray3d) -> VirtualPointer {
            let bare: Vec<Aimable> = self
                .aimables()
                .into_iter()
                .filter(|aim| aim.riding.is_none())
                .collect();
            crate::surface::pick(ray, true, REACH, false, bare)
        }

        /// Where the held piece's drop at this frame's pointer would land,
        /// and the sim's ruling on it: the ghost's own question.
        fn preview(&self) -> (Loc, Result<(), Option<space_trucking::sim::Violation>>) {
            let pointer = self.pointer().sim;
            let bridge = &self.app.world().resource::<Shell>().bridge;
            bridge
                .sim
                .drop_preview(0, pointer, bridge.facing(), bridge.lift())
                .expect("the aim is on a net")
        }

        /// The surface piece `id` carries as `station` this frame, if it
        /// carries one: a standing body's pick face rides its own pose.
        fn riding(&mut self, id: u32, want: Station) -> Option<SimSurface> {
            self.app
                .world_mut()
                .query::<(&Station, &SimSurface, &Riding)>()
                .iter(self.app.world())
                .find(|(station, _, riding)| **station == want && riding.0 == id)
                .map(|(_, surface, _)| *surface)
        }

        /// Press `key` for a frame, and let it back up for one.
        fn tap(&mut self, key: KeyCode) {
            self.hold_keys(&[key]);
            self.step();
            self.hold_keys(&[]);
            self.step();
        }

        /// The lift the carry is sent at.
        fn lift(&self) -> u16 {
            self.app.world().resource::<Shell>().bridge.lift()
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
                    matches!(piece.loc, Loc::Hold { room: at, .. } if at == room)
                        && space_trucking::sim::cargo::berth_tile(
                            sim.rooms(),
                            piece.kind,
                            piece.loc,
                        ) != Some(Tile::Stock)
                })
                .map(|piece| layout::piece_rect(sim.rooms(), piece))?;
            Some(SimVec2::new(
                rect.w.mul_add(0.5, rect.x),
                rect.h.mul_add(0.5, rect.y),
            ))
        }

        /// **Can the player still act?** Let go of everything, step out
        /// of whatever the camera is in, send home whatever is still in
        /// hand, walk up to the chart tank, focus it, and chart a course.
        /// Every step is something a player does with the hardware they
        /// have; the mouse alone is enough for all of it, which is why no
        /// key is pressed here.
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
            // A carry outlives the buttons — it is hands-free — and a
            // click with a piece in hand is the drop, never a focus. A
            // right click sends the piece home.
            if self.sim().held(0).is_some() {
                self.hold_right(true);
                self.step();
                self.hold_right(false);
                self.steps(2);
            }
            if let Some(held) = self.sim().held(0) {
                return Err(format!("a right click left piece {} in hand", held.piece));
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

    /// **A press reaches the nearest thing the player is actually looking
    /// at.**
    ///
    /// A doorway's amber latch is hardware on a wall, and the room in
    /// front of it is full of cargo. Stand so that a crate crosses the
    /// line between the eye and the latch and the crosshair is on the
    /// crate: the press belongs to the crate, and the seam is not the
    /// player's answer to a click they aimed at something else.
    ///
    /// The latch used to answer anyway. It cast its own ray, took the
    /// nearest latch within [`REACH`], and `advance` spent the whole
    /// frame's pointer on it — so a latch behind a piece ate every click
    /// on that piece, which can then be neither lifted nor focused, and
    /// the click looks like it did nothing at all.
    #[test]
    fn a_piece_in_front_of_a_latch_takes_the_press() {
        let mut cabin = Cabin::at_venus();
        let spot = cabin
            .eclipsed_latch()
            .expect("a latch with cargo standing in front of it");
        let Eclipse {
            clear,
            eye,
            latch,
            piece,
        } = spot;
        cabin.stand(eye, latch.face.center);
        cabin.steps(8);
        cabin.look(latch.face.center);
        cabin.steps(2);
        let said = cabin.click();
        assert!(
            !said.iter().any(|cue| matches!(cue, Cue::Parted)),
            "a press aimed at piece {piece} parted room {}: {said:?}",
            latch.room
        );
        assert_eq!(
            cabin.sim().held(0).map(|held| held.piece),
            Some(piece),
            "a press on piece {piece} from {eye:?} lifted nothing"
        );

        // **And the answer is this frame's.** Stand where the latch is
        // clear — where the press really does part the seam — then step
        // across to the eclipsed spot and press on the frame the body
        // arrives, which is what a flick of the mouse and a click is. A
        // latch aimed from where the body USED to stand is a latch that
        // eats a press aimed at something else.
        let mut cabin = Cabin::at_venus();
        cabin.stand(clear, latch.face.center);
        cabin.steps(8);
        cabin.look(latch.face.center);
        cabin.steps(2);
        cabin.stand(eye, latch.face.center);
        let said = cabin.click();
        assert!(
            !said.iter().any(|cue| matches!(cue, Cue::Parted)),
            "a press on the frame the eye reached piece {piece} parted room {}: {said:?}",
            latch.room
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

    /// **`R` is only a letter now.** It used to start a new run, which
    /// throws the old one away for good, and it sat one key over from
    /// focus and one row up from the walk. A run ends only from the `Esc`
    /// menu now, on a bar that says so in words; that half of the law is
    /// `menu::tests::the_new_run_bar_starts_a_new_run`, on the same road
    /// into the sim. This half hands the old key to the whole input
    /// schedule and asks whether the world it had is the world it kept.
    #[test]
    fn r_is_only_a_letter_now() {
        let mut cabin = Cabin::new(crate::fixture::SAVE);
        let seed = cabin.sim().seed();
        let tick = cabin.sim().tick();
        cabin.hold_keys(&[KeyCode::KeyR]);
        cabin.step();
        let said = cabin.sim().cues().to_vec();
        cabin.hold_keys(&[]);
        cabin.steps(2);
        assert!(
            !said.contains(&Cue::Reseed),
            "R threw the run away: {said:?}"
        );
        assert_eq!(cabin.sim().seed(), seed, "R replaced the world");
        assert!(
            cabin.sim().tick() > tick,
            "the world stopped instead of going on"
        );
    }

    /// The `Turn` nearest `d` degrees, the way the carry is sent at one.
    fn degrees(d: i32) -> Turn {
        // A degree is 65,536 / 360 of a Turn unit: nearest, halves up.
        let units = (i64::from(d.rem_euclid(360)) * 65_536 * 2 + 360) / 720;
        Turn(u16::try_from(units % 65_536).expect("a turn"))
    }

    /// **The hands turn the carry, through the whole input schedule, and
    /// the drop keeps the turn.**
    ///
    /// A crate lifted off the deck carries at its own turn. `Ctrl` and
    /// the wheel turn it a degree a notch, to seven; a plain notch up
    /// from seven lands on fifteen and not twenty-two, and another on
    /// thirty; a notch down is fifteen again. `Q` and `E` are the plain
    /// notch either way for a hand with no wheel, read off the binding
    /// table (`crate::keys`). Each turn is the turn the sim previews the
    /// drop at, the patch's own question, and the release lands the crate
    /// at it — a twenty-second of a turn off square, nothing snapped.
    /// Lifted again, it carries on from there.
    #[test]
    fn the_wheel_and_the_turn_keys_turn_the_carry_and_the_drop_keeps_the_turn() {
        let (mut cabin, id, at) = Cabin::over_a_crate();
        cabin.click();
        assert_eq!(
            cabin.sim().held(0).map(|held| held.piece),
            Some(id),
            "the crate under the crosshair did not lift"
        );
        assert_eq!(
            cabin.facing(),
            Turn::ZERO,
            "a carry starts at the crate's own turn"
        );
        let turned = |cabin: &mut Cabin, keys: &[KeyCode], notches: f32| {
            cabin.hold_keys(keys);
            cabin.wheel(notches);
            cabin.step();
            cabin.hold_keys(&[]);
            cabin.step();
            cabin.facing()
        };
        let ctrl = [KeyCode::ControlLeft];
        assert_eq!(
            turned(&mut cabin, &ctrl, 7.0),
            degrees(7),
            "Ctrl turns degrees"
        );
        assert_eq!(
            turned(&mut cabin, &[], 1.0),
            degrees(15),
            "up from seven is fifteen"
        );
        assert_eq!(turned(&mut cabin, &[], 1.0), degrees(30));
        assert_eq!(
            turned(&mut cabin, &[], -1.0),
            degrees(15),
            "and down is the way back"
        );
        assert_eq!(turned(&mut cabin, &ctrl, -1.0), degrees(14));
        assert_eq!(
            turned(&mut cabin, &[KeyCode::KeyQ], 0.0),
            degrees(15),
            "Q is a notch"
        );
        assert_eq!(
            turned(&mut cabin, &[KeyCode::KeyE], 0.0),
            degrees(0),
            "E is a notch back"
        );
        assert_eq!(
            turned(&mut cabin, &[KeyCode::ShiftLeft, KeyCode::KeyQ], 0.0),
            degrees(15),
            "Shift reverses nothing any more"
        );
        assert_eq!(
            turned(&mut cabin, &[KeyCode::KeyQ, KeyCode::KeyE], 0.0),
            degrees(15),
            "both turn keys at once cancel"
        );
        let facing = turned(&mut cabin, &ctrl, -31.0);
        assert_eq!(facing, degrees(-16));
        assert!(!facing.square(), "the carry was meant to end up off square");

        // The preview the ghost and the patch are drawn from is asked at
        // that turn, and the release lands exactly where it said.
        let pointer = cabin.app.world().resource::<VirtualPointer>().sim;
        let shell = &cabin.app.world().resource::<Shell>().bridge;
        let (berth, verdict) = shell
            .sim
            .drop_preview(0, pointer, shell.facing(), shell.lift())
            .expect("the aim is on the deck");
        assert_eq!(verdict, Ok(()), "open deck refused the turned crate");
        assert_eq!(berth.spot().turn, facing);
        cabin.click();
        let landed = cabin.sim().pieces()[0].loc;
        assert!(cabin.sim().held(0).is_none(), "the release did not land");
        assert_eq!(
            landed, berth,
            "the crate landed somewhere the preview never said"
        );
        assert_eq!(
            landed.spot().turn,
            facing,
            "the drop forgot the turn it was carried at"
        );

        // Lifted again, the carry starts at the turn the crate stands at.
        let floor = cabin.face(Station::BayFloor).expect("the deck");
        cabin.look(floor.to_world(at));
        cabin.steps(2);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(id));
        assert_eq!(cabin.facing(), facing, "a crate lifted again lost its turn");
    }

    /// **`E` turns a full hand and focuses an empty one**, through the
    /// whole input schedule (docs/BAY.md, "Lift, and the keys"). Standing
    /// at the chart tank with nothing in hand, `E` focuses it and `E`
    /// steps back out. With a crate in hand, at the same tank, `E` turns
    /// the crate a notch clockwise and focuses nothing — and so does a
    /// click, which is the drop with a piece in hand and nothing else
    /// (docs/BAY.md, "The carry sees the room"): the wall behind the glass
    /// takes no crate, so the crate goes home, and the camera stays in
    /// the room.
    #[test]
    fn e_turns_a_full_hand_and_focuses_an_empty_one() {
        let (x, y) = deck_middle(0);
        let mut cabin = Cabin::furnished(&[(Kind::ScrapAlloy, x, y)], &[Kind::ChartTank]);
        let tank = cabin.face(Station::Map).expect("the starting board's tank");
        let stand = tank.center + tank.normal() * 0.75;
        let at_the_tank = |cabin: &mut Cabin| {
            cabin.stand(Vec3::new(stand.x, EYE_HEIGHT, stand.z), tank.center);
            cabin.steps(3);
        };
        let focused = |cabin: &Cabin| {
            matches!(
                cabin.app.world().resource::<CameraRig>().mode,
                Mode::ToFocus {
                    focus: Focus::Tank,
                    ..
                } | Mode::Focused { focus: Focus::Tank }
            )
        };

        at_the_tank(&mut cabin);
        cabin.tap(KeyCode::KeyE);
        assert!(
            focused(&cabin),
            "E with an empty hand did not focus the tank"
        );
        cabin.steps(60);
        cabin.tap(KeyCode::KeyE);
        cabin.steps(60);
        assert!(cabin.roaming(), "E did not step back out of the tank");

        let home = cabin.sim().pieces()[0].loc;
        cabin.stand_over(0);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(0));
        at_the_tank(&mut cabin);
        cabin.tap(KeyCode::KeyE);
        assert!(
            cabin.roaming() && !focused(&cabin),
            "E focused the tank with a crate in hand"
        );
        assert_eq!(cabin.facing(), degrees(-15), "E did not turn the crate");
        cabin.click();
        cabin.steps(2);
        assert!(
            cabin.roaming() && !focused(&cabin),
            "a click with a crate in hand flew to the tank"
        );
        assert!(
            cabin.sim().held(0).is_none(),
            "a click with a crate in hand did not end the carry"
        );
        assert_eq!(
            cabin.sim().pieces()[0].loc,
            home,
            "the wall behind the tank took a crate"
        );
    }

    /// **A piece lifted at a station takes the camera back to the room.**
    /// Nothing in the room starts a focus with a full hand, but a focused
    /// cursor works the room's charts as well as the glass, and a press on
    /// a crate down on the deck lifts it. A carry is worked in the room:
    /// the camera walks back out with the crate in hand, the grip holds
    /// through the glide, and the next click drops it.
    #[test]
    fn a_piece_lifted_at_a_station_takes_the_camera_back_to_the_room() {
        let (x, y) = deck_middle(0);
        let mut cabin = Cabin::furnished(&[(Kind::ScrapAlloy, x, y)], &[Kind::ChartTank]);
        let tank = cabin.face(Station::Map).expect("the starting board's tank");
        let stand = tank.center + tank.normal() * 0.75;
        cabin.stand(Vec3::new(stand.x, EYE_HEIGHT, stand.z), tank.center);
        cabin.steps(3);
        cabin.click();
        cabin.steps(60);
        assert!(
            matches!(
                cabin.app.world().resource::<CameraRig>().mode,
                Mode::Focused { focus: Focus::Tank }
            ),
            "a click on the tank did not focus it"
        );

        let crate_at = layout::piece_rect(cabin.sim().rooms(), &cabin.sim().pieces()[0]);
        cabin.rest_cursor(Some(SimVec2::new(
            crate_at.w.mul_add(0.5, crate_at.x),
            crate_at.h.mul_add(0.5, crate_at.y),
        )));
        cabin.steps(2);
        cabin.click();
        assert_eq!(
            cabin.sim().held(0).map(|held| held.piece),
            Some(0),
            "the focused cursor did not lift the crate"
        );
        assert!(
            !matches!(
                cabin.app.world().resource::<CameraRig>().mode,
                Mode::Focused { .. } | Mode::ToFocus { .. }
            ),
            "the camera kept its focus with a crate in hand"
        );
        cabin.rest_cursor(None);
        cabin.steps(60);
        assert!(cabin.roaming(), "the camera never came back to the room");
        assert_eq!(
            cabin.sim().held(0).map(|held| held.piece),
            Some(0),
            "the glide dropped the crate"
        );
        cabin.stand_over(0);
        cabin.click();
        assert!(
            cabin.sim().held(0).is_none(),
            "a click in the room did not drop the crate"
        );
    }

    /// **The lift keys and `Shift` with the wheel lift the carry**,
    /// through the whole input schedule (docs/BAY.md, "Lift, and the
    /// keys"). `X` raises a sixteenth of a cell and `Z` lowers one, and
    /// nothing lowers it under the deck; `Shift` with the wheel lifts a
    /// sixteenth a notch and `Ctrl` with them a fine unit, and turns
    /// nothing; the deckhead is the cap, however far past it the hand
    /// goes; and the release lands the crate at the lift it was carried
    /// at.
    #[test]
    fn the_lift_keys_and_shift_with_the_wheel_lift_the_carry() {
        let (mut cabin, id, _) = Cabin::over_a_crate();
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(id));
        assert_eq!(cabin.lift(), 0, "a carry starts at the crate's own lift");
        cabin.tap(KeyCode::KeyX);
        assert_eq!(cabin.lift(), LIFT_STEP, "X is a sixteenth up");
        cabin.tap(KeyCode::KeyZ);
        cabin.tap(KeyCode::KeyZ);
        assert_eq!(cabin.lift(), 0, "Z lowered the crate through the deck");
        let lifted = |cabin: &mut Cabin, keys: &[KeyCode], notches: f32| {
            cabin.hold_keys(keys);
            cabin.wheel(notches);
            cabin.step();
            cabin.hold_keys(&[]);
            cabin.step();
            cabin.lift()
        };
        let shift = [KeyCode::ShiftLeft];
        let fine = [KeyCode::ShiftLeft, KeyCode::ControlLeft];
        assert_eq!(lifted(&mut cabin, &shift, 2.0), 2 * LIFT_STEP);
        assert_eq!(
            lifted(&mut cabin, &fine, 1.0),
            2 * LIFT_STEP + 1,
            "Ctrl and Shift lift a fine unit"
        );
        assert_eq!(lifted(&mut cabin, &shift, -1.0), LIFT_STEP + 1);
        assert_eq!(cabin.facing(), Turn::ZERO, "a lift turned the crate");
        let cap = cargo::lift_cap(RoomKind::Cabin, Kind::ScrapAlloy, Surf::Floor);
        assert_eq!(
            lifted(&mut cabin, &shift, 1000.0),
            cap,
            "the deckhead did not stop the crate"
        );
        cabin.tap(KeyCode::KeyZ);
        assert_eq!(cabin.lift(), cap - LIFT_STEP, "the cap saved a lift up");
        cabin.click();
        assert!(cabin.sim().held(0).is_none(), "the release did not land");
        assert_eq!(
            cabin.sim().pieces()[0].loc.lift(),
            cap - LIFT_STEP,
            "the drop forgot its lift"
        );
    }

    /// **A carry aims through cargo, at the room behind it** (docs/BAY.md,
    /// "The carry sees the room"), through the whole input schedule.
    ///
    /// A line from a standing eye down through a cabinet's body to bare
    /// deck beyond it: with an empty hand the cabinet answers, the nearest
    /// body along the ray, and it is the piece a press would lift. With a
    /// vial in hand the same line reads the deck it comes down on, exactly
    /// what the room would read with no cargo in it; the drop is centred
    /// there, out past the cabinet's back and not on its ground; and the
    /// release lands it where the preview said.
    #[test]
    fn a_carry_aims_through_a_cabinet_at_the_deck_beyond_it() {
        let (cx, cy) = deck_middle(0);
        let (vx, vy) = deck_middle(2);
        let mut cabin =
            Cabin::furnished(&[(Kind::Cabinet, cx, cy), (Kind::PerfumeVial, vx, vy)], &[]);
        let ground = cargo::Foot::at(cabin.sim().rooms(), &cabin.sim().pieces()[0])
            .expect("the cabinet stands on the deck")
            .1;
        let (eye, beyond) = cabin.through(0);
        let look = |cabin: &mut Cabin| {
            cabin.stand(eye, beyond);
            cabin.steps(2);
            cabin.pointer()
        };

        let pointer = look(&mut cabin);
        assert_eq!(
            pointer.piece,
            Some(0),
            "an empty hand aimed through the cabinet met {:?}",
            pointer.station
        );
        assert_eq!(
            pointer.aimed(cabin.sim()).map(|piece| piece.id),
            Some(0),
            "an empty hand aimed at the cabinet would lift something else"
        );

        cabin.stand_over(1);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(1));
        let pointer = look(&mut cabin);
        let ray = pointer.ray.expect("a roaming crosshair casts");
        let room = cabin.room_alone(ray);
        assert_eq!(
            (pointer.piece, pointer.station),
            (None, Some(Station::BayFloor)),
            "the carry was answered by a body, not by the deck behind it"
        );
        assert_eq!(
            pointer.sim, room.sim,
            "the carry read a point the room does not"
        );
        let met = ray.get_point(pointer.depth);
        assert!(
            met.distance(beyond) < 1e-3,
            "the carry met the line at {met:?}, not on the deck at {beyond:?}"
        );
        let (berth, verdict) = cabin.preview();
        assert_eq!(verdict, Ok(()), "bare deck refused the vial");
        let spot = berth.spot();
        let centre = (i32::from(spot.x), i32::from(spot.y));
        assert_eq!(
            centre,
            layout::fine_at(CABIN, pointer.sim),
            "the drop is not centred where the line meets the deck"
        );
        assert!(
            !ground.contains(centre),
            "the drop was drawn into the cabinet's footprint, at {centre:?}"
        );
        cabin.click();
        assert!(cabin.sim().held(0).is_none(), "the vial did not land");
        assert_eq!(
            cabin.sim().pieces()[1].loc,
            berth,
            "the vial landed somewhere the preview never said"
        );
    }

    /// **A carry aims through an instrument's glass, and a click with a
    /// piece in hand drops it there.** The starting board's chart tank
    /// hangs on the starboard flank. With an empty hand, a crosshair on
    /// the middle of its glass names the tank and routes the click to its
    /// focus, and a click focuses it. With the sconce in hand, the same
    /// aim reads the wall behind the glass, as the bare wall would; the
    /// drop is centred there; and the click hangs the sconce on that wall
    /// and leaves the camera roaming.
    #[test]
    fn a_carry_aims_through_an_instruments_glass_and_a_click_drops_it_there() {
        let mut cabin = Cabin::furnished(&[], &[Kind::ChartTank, Kind::WallLamp]);
        let id = |cabin: &Cabin, kind: Kind| {
            cabin
                .sim()
                .pieces()
                .iter()
                .find(|piece| piece.kind == kind)
                .map(|piece| piece.id)
                .expect("the starting board's")
        };
        let (tank, lamp) = (id(&cabin, Kind::ChartTank), id(&cabin, Kind::WallLamp));
        let glass = cabin.face(Station::Map).expect("the tank's glass");
        let stand = glass.center + glass.normal() * 0.75;
        let at_the_tank = |cabin: &mut Cabin| {
            cabin.stand(Vec3::new(stand.x, EYE_HEIGHT, stand.z), glass.center);
            cabin.steps(3);
            cabin.pointer()
        };
        let mode = |cabin: &Cabin| cabin.app.world().resource::<CameraRig>().mode;

        let pointer = at_the_tank(&mut cabin);
        let over = *pointer
            .aimed(cabin.sim())
            .expect("an empty hand at the glass names the tank");
        assert_eq!(over.id, tank);
        assert_eq!(
            crate::rig::handle_route(cabin.sim().rooms(), &over, pointer.sim),
            Some(Focus::Tank),
            "the glass no longer routes to the tank's focus"
        );

        cabin.stand_before(lamp);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(lamp));
        let pointer = at_the_tank(&mut cabin);
        let ray = pointer.ray.expect("a roaming crosshair casts");
        let room = cabin.room_alone(ray);
        assert_eq!(
            (pointer.piece, pointer.station),
            (None, Some(Station::BayStarboard)),
            "the carry was answered by the tank, not by the wall behind it"
        );
        assert_eq!(
            (pointer.sim, pointer.depth),
            (room.sim, room.depth),
            "the carry read a point the bare wall does not"
        );
        let (berth, verdict) = cabin.preview();
        assert_eq!(
            verdict,
            Ok(()),
            "the wall behind the tank refused the sconce"
        );
        let spot = berth.spot();
        assert_eq!(
            (i32::from(spot.x), i32::from(spot.y)),
            layout::fine_at(CABIN, pointer.sim),
            "the drop is not centred where the line meets the wall"
        );
        cabin.click();
        assert!(
            matches!(mode(&cabin), Mode::Roam),
            "a click with the sconce in hand flew to {:?}",
            mode(&cabin)
        );
        assert!(
            cabin.sim().held(0).is_none(),
            "the click did not drop the sconce"
        );
        let landed = cabin
            .sim()
            .pieces()
            .iter()
            .find(|piece| piece.id == lamp)
            .expect("aboard")
            .loc;
        assert_eq!(
            landed, berth,
            "the sconce landed somewhere the preview never said"
        );

        at_the_tank(&mut cabin);
        cabin.click();
        cabin.steps(60);
        assert!(
            matches!(mode(&cabin), Mode::Focused { focus: Focus::Tank }),
            "an empty hand's click on the glass no longer focuses the tank: {:?}",
            mode(&cabin)
        );
    }

    /// **A piece left at a height stays there, and is lifted again at
    /// it** (docs/BAY.md, "Lift"), through the whole input schedule. A
    /// vial raised off the deck with `Shift` and the wheel and set down a
    /// cell over stands at that height, centred where the crosshair met
    /// the deck; its pick face rides it up there by exactly the lift, so
    /// the crosshair finds it by its own raised body; lifted again, the
    /// carry starts at its height; and set down again with no touch of
    /// the lift keys, it keeps it.
    #[test]
    fn a_lifted_piece_keeps_its_lift_through_a_drop_and_a_regrab() {
        let (x, y) = deck_middle(0);
        let mut cabin = Cabin::furnished(&[(Kind::PerfumeVial, x, y)], &[]);
        let low = cabin
            .riding(0, Station::Standing)
            .expect("the vial carries its own pick face")
            .center;
        let home = cabin.stand_over(0);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(0));
        let raised = 20 * LIFT_STEP;
        cabin.hold_keys(&[KeyCode::ShiftLeft]);
        cabin.wheel(20.0);
        cabin.step();
        cabin.hold_keys(&[]);
        cabin.step();
        assert_eq!(cabin.lift(), raised, "the vial was not raised");

        let floor = cabin.face(Station::BayFloor).expect("the cabin's deck");
        let over = SimVec2::new(home.x + layout::CELL, home.y);
        cabin.look(floor.to_world(over));
        cabin.steps(2);
        let (berth, verdict) = cabin.preview();
        assert_eq!(verdict, Ok(()), "open deck refused the raised vial");
        cabin.click();
        let landed = cabin.sim().pieces()[0].loc;
        assert!(cabin.sim().held(0).is_none(), "the vial did not land");
        assert_eq!(
            landed, berth,
            "the vial landed somewhere the preview never said"
        );
        assert_eq!(landed.lift(), raised, "the drop forgot the vial's height");
        let spot = landed.spot();
        assert_eq!(
            (i32::from(spot.x), i32::from(spot.y)),
            layout::fine_at(CABIN, over),
            "the raised vial is not centred where the crosshair met the deck"
        );

        let face = cabin
            .riding(0, Station::Standing)
            .expect("the raised vial carries its own pick face");
        let up = f32::from(raised) / f32::from(cargo::FINE) * crate::rig::BAY_CELL;
        assert!(
            (face.center.y - low.y - up).abs() < 1e-3,
            "the vial's pick face stands {} m up, not the {up} m it was raised",
            face.center.y - low.y
        );
        cabin.look(face.center);
        cabin.steps(2);
        cabin.click();
        assert_eq!(
            cabin.sim().held(0).map(|held| held.piece),
            Some(0),
            "the crosshair on the raised vial lifted something else"
        );
        assert_eq!(
            cabin.lift(),
            raised,
            "lifted again, the vial forgot its height"
        );

        cabin.look(floor.to_world(home));
        cabin.steps(2);
        cabin.click();
        let landed = cabin.sim().pieces()[0].loc;
        assert!(cabin.sim().held(0).is_none(), "the vial did not land again");
        assert_eq!(
            landed.lift(),
            raised,
            "set down again, the vial forgot its height"
        );
    }

    /// **Nothing but a carry turns, and only while the body roams.** The
    /// wheel and `Q` with an empty hand turn nothing — the carry that
    /// follows starts at its crate's own turn, with no stray notch saved
    /// up for it — and with the `Esc` menu standing over a carry they
    /// are the menu's, which reads neither. Close the menu and the same
    /// notch turns the carry.
    #[test]
    fn the_wheel_turns_only_a_carry_and_only_in_the_room() {
        let (mut cabin, id, _) = Cabin::over_a_crate();
        cabin.hold_keys(&[KeyCode::KeyQ]);
        cabin.wheel(3.0);
        cabin.step();
        cabin.hold_keys(&[]);
        cabin.wheel(0.5);
        cabin.step();
        assert_eq!(cabin.facing(), Turn::ZERO);
        cabin.click();
        assert_eq!(cabin.sim().held(0).map(|held| held.piece), Some(id));
        cabin.wheel(0.5);
        cabin.step();
        assert_eq!(
            cabin.facing(),
            Turn::ZERO,
            "a carry began with wheel saved up from an empty hand"
        );

        cabin.hold_keys(&[KeyCode::Escape]);
        cabin.step();
        cabin.hold_keys(&[]);
        cabin.step();
        assert!(
            cabin.app.world().resource::<menu::Menu>().open,
            "Esc opens the menu"
        );
        assert!(cabin.sim().held(0).is_some(), "the menu dropped the carry");
        cabin.wheel(2.0);
        cabin.step();
        cabin.hold_keys(&[KeyCode::KeyQ]);
        cabin.step();
        cabin.hold_keys(&[]);
        cabin.step();
        assert_eq!(
            cabin.facing(),
            Turn::ZERO,
            "the wheel turned a carry under the menu"
        );

        cabin.hold_keys(&[KeyCode::Escape]);
        cabin.step();
        cabin.hold_keys(&[]);
        cabin.step();
        assert!(!cabin.app.world().resource::<menu::Menu>().open);
        cabin.wheel(1.0);
        cabin.step();
        assert_eq!(
            cabin.facing(),
            degrees(15),
            "back in the room, the wheel turns it"
        );
    }
}
