//! macroquad frontend: input in, the ship console out.
//!
//! Everything that decides what happens lives in [`space_trucking::sim`].
//! This file only translates between the window and the sim's logical world:
//! it gathers an [`InputFrame`] each frame (folding the console's icon
//! buttons into the pause/warp/mute toggles), advances the sim, and hands
//! the result to [`render`] and [`audio`]. It also owns the save slot —
//! load-and-catch-up on startup, cue-driven autosave forever after — the
//! flight recorder's black box, the opt-in telemetry buffer behind the web
//! shell's consent card (`docs/TELEMETRY.md`), and the game's one and only
//! piece of text, the version string. Run natively with `--replay <file>`
//! to watch a recorded session play itself back.

mod audio;
mod juice;
mod palette;
mod render;
mod storage;
mod tutor;

use audio::Audio;
use juice::Juice;
use macroquad::color::Color;
use macroquad::input::{
    KeyCode, MouseButton, is_key_down, is_key_pressed, is_mouse_button_down,
    is_mouse_button_pressed, is_mouse_button_released, mouse_position,
};
use macroquad::text::{draw_text, measure_text};
use macroquad::texture::{FilterMode, RenderTarget, RenderTargetParams, render_target_ex};
use macroquad::time::get_frame_time;
use macroquad::window::{Conf, next_frame, screen_height, screen_width};
use tutor::Tutor;

use space_trucking::VERSION;
use space_trucking::replay::Recording;
use space_trucking::sim::{
    Cue, InputFrame, Sim, TICK_DT, Vec2, WARP_FACTOR, WORLD_H, WORLD_W, layout,
};
use space_trucking::telemetry::{Aggregate, BUFFER_KEY, CONSENT_KEY, Snapshot};

const TEXT_SIZE: u16 = 16;
const TEXT_MARGIN: f32 = 8.0;

/// Pixel crunch: world units per rendered pixel.
///
/// The fiction draws into a target this many times smaller than the logical
/// world and upscales nearest-neighbour — hard pixel edges everywhere, per
/// the art doc. Set to 1.0 and everything still draws, just uncrunched.
pub const CRUNCH: f32 = 2.0;

/// Seconds between wall-clock autosaves; cue-driven saves come sooner.
const SAVE_EVERY: f64 = 10.0;

/// Storage key for the flight recorder's black box, persisted on the same
/// cadence as the save.
const REPLAY_KEY: &str = "space-trucking/replay";

/// Storage key the web shell mirrors `prefers-reduced-motion` into: `"1"`
/// while reduced, absent otherwise (see `web/index.html`). Read once at
/// boot — a mid-session OS toggle applies on the next load. Native builds
/// have no shell writing it, so the key stays absent and motion stays full.
const REDUCED_MOTION_KEY: &str = "space-trucking/reduced-motion";

/// Storage key for developer mode, which is the only thing that unlocks
/// fast-forward. The web shell sets it after the pretty-please dialogue
/// (see `web/index.html`); natively, `--dev` on the command line sets it
/// for good. Players never need it: the game is meant to run at 1x.
const DEV_KEY: &str = "space-trucking/dev";

/// Storage key the web shell mirrors the local wall clock's deep-night
/// window into (23:30–06:00): `"1"` inside it, `"0"` outside. Native
/// builds fall back to the same window in UTC — close enough for a
/// mystery.
const NIGHT_KEY: &str = "space-trucking/night";

/// A frame gap larger than this means the tab was backgrounded or the
/// machine slept: real time kept passing, so the missing ticks are
/// replayed through `fast_forward` instead of being clamped away.
const STALL_SECONDS: f64 = 1.0;

/// Whether the player's OS asked for reduced motion, per the mirrored key.
fn reduced_motion() -> bool {
    storage::get(REDUCED_MOTION_KEY).as_deref() == Some("1")
}

/// Whether developer mode is unlocked (the pretty-please was said).
fn dev_mode() -> bool {
    storage::get(DEV_KEY).as_deref() == Some("1")
}

/// Whether it is deep night (23:30–06:00) on the player's clock: the web
/// shell's mirrored local answer when present, otherwise the same window
/// read off the UTC wall clock.
fn night_now() -> bool {
    match storage::get(NIGHT_KEY).as_deref() {
        Some("1") => true,
        Some(_) => false,
        None => {
            let day = macroquad::miniquad::date::now().rem_euclid(86_400.0);
            let hour = day / 3600.0;
            !(6.0..23.5).contains(&hour)
        }
    }
}

/// Longest frame the replay pacer banks, matching the sim's own clamp so a
/// backgrounded playback does not spiral catching up.
const REPLAY_FRAME_CLAMP: f32 = 0.25;

/// Longest absence the startup catch-up replays, in seconds (six hours).
const MAX_CATCH_UP: f64 = 6.0 * 3600.0;

/// Sim ticks per wall-clock second of absence.
const CATCH_UP_RATE: f64 = 60.0;

fn window_conf() -> Conf {
    Conf {
        // The one place the game says its own name: lowercase, one space,
        // matching the browser tab.
        window_title: "space trucking".to_owned(),
        window_width: WORLD_W as i32,
        window_height: WORLD_H as i32,
        high_dpi: true,
        ..Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // `--replay <file>`: the native black-box playback mode. On the web
    // there are no args, so this never triggers there. (`Args` is scoped so
    // the non-Send iterator never lives across an await.)
    let (replay_mode, replay_path) = {
        let mut args = std::env::args();
        (args.nth(1).as_deref() == Some("--replay"), args.next())
    };
    if replay_mode {
        replay_session(replay_path).await;
        return;
    }
    if std::env::args().any(|arg| arg == "--dev") {
        // The native pretty-please. Saying it once is saying it forever.
        storage::set(DEV_KEY, "1");
    }

    let (mut sim, arrived_while_away, caught_up_seconds) = restore();
    let mut recording = Recording::new(sim.save_string());
    // On a first visit the consent card's answer necessarily arrives AFTER
    // boot — the card floats over the already-running game — so while no
    // choice is recorded yet, keep asking at the persist cadence and let a
    // late "yes" start counting mid-session.
    let mut consent_pending = storage::get(CONSENT_KEY).is_none();
    let mut telemetry = boot_telemetry(caught_up_seconds);
    let mut audio = Audio::load().await;
    let mut juice = Juice::default();
    let mut tutor = Tutor::default();
    let reduced_motion = reduced_motion();
    let mut dev = dev_mode();
    let mut night = night_now();
    // Wall-clock idle for the onboarding ghost: seconds since the player
    // last pressed, keyed, or toggled anything. Pointer motion deliberately
    // does not count — watching must not hold the tutor at bay.
    let mut idle_seconds: f32 = 0.0;
    let target = pixel_target();
    if arrived_while_away {
        juice.catch_up_arrival();
    }
    let mut last_save = macroquad::miniquad::date::now();
    let mut last_frame = last_save;

    loop {
        let view = View::fit(screen_width(), screen_height());
        let input = gather_input(&view, dev, night);
        let toggle_mute =
            is_key_pressed(KeyCode::M) || (input.press && layout::SPEAKER.contains(input.pointer));
        let dt = get_frame_time();

        // A backgrounded tab or a sleeping laptop does not pause the world:
        // when the wall clock says far more passed than the frame did,
        // replay the difference silently, exactly like the boot catch-up.
        let wall_now = macroquad::miniquad::date::now();
        let wall_gap = wall_now - last_frame;
        last_frame = wall_now;
        if wall_gap > STALL_SECONDS {
            let missed = (wall_gap - f64::from(dt)).clamp(0.0, MAX_CATCH_UP);
            let ticks = u64::try_from((missed * CATCH_UP_RATE) as i64).unwrap_or(0);
            if sim.fast_forward(ticks).arrived {
                juice.catch_up_arrival();
            }
        }

        recording.record_frame(sim.tick(), &input);
        sim.advance(dt, &input);
        juice.update(dt, &sim, input.pointer, input.press);
        audio.update(dt, &sim, toggle_mute);

        // Any real input resets the idle clock, which is also how the tutor
        // learns to snuff its ghost mid-demonstration.
        let interacted = input.press
            || input.held
            || input.release
            || input.toggle_pause
            || input.toggle_warp
            || input.reseed.is_some()
            || toggle_mute;
        idle_seconds = if interacted { 0.0 } else { idle_seconds + dt };
        tutor.update(dt, idle_seconds, &sim);

        if sim.cues().iter().any(|cue| matches!(cue, Cue::Reseed)) {
            // The black box tells one run's story: a new world, a new tape.
            recording = Recording::new(sim.save_string());
        }

        if let Some(aggregate) = &mut telemetry {
            // Cues live until the next advance, so the aggregator sees this
            // frame's — the reseed just counted included — with the coarse
            // state as the advance left it.
            aggregate.observe(sim.cues(), dt, Snapshot::of(&sim));
        }

        let now = macroquad::miniquad::date::now();
        if save_worthy(&sim) || now - last_save >= SAVE_EVERY {
            storage::store(&sim.save_string(), now);
            if recording.is_full() && sim.held(0).is_none() {
                // Roll the tape. Saves drop drags, so only cut between them.
                recording.rebase(sim.save_string(), sim.tick());
            }
            recording.seal(sim.tick());
            storage::set(REPLAY_KEY, &recording.serialize());
            if consent_pending {
                match storage::get(CONSENT_KEY).as_deref() {
                    Some("yes") => {
                        telemetry = Some(consented_aggregate(caught_up_seconds));
                        consent_pending = false;
                    }
                    Some(_) => consent_pending = false,
                    None => {}
                }
            }
            if let Some(aggregate) = &telemetry {
                // Boot merged this aggregate with its stored predecessor,
                // so the write is the running total across sessions.
                storage::set(BUFFER_KEY, &aggregate.serialize());
            }
            // Cheap clock work rides the save cadence: the night window
            // creeps, and the shell may have heard a pretty-please.
            night = night_now();
            dev = dev_mode();
            last_save = now;
        }

        render::draw(
            &view,
            &target,
            &render::Scene {
                sim: &sim,
                juice: &juice,
                pointer: input.pointer,
                ghost: tutor.ghost(),
                audio_waiting: audio.needs_gesture(),
                audio_muted: audio.muted(),
                reduced_motion,
                dev,
            },
        );
        draw_version(palette::VERSION_TEXT);
        next_frame().await;
    }
}

/// Play a recording back at real time: one sim tick per elapsed [`TICK_DT`]
/// (times [`WARP_FACTOR`] while the recorded session warped), rendering
/// normally with the recorded pointer as a ghost cursor. Local input is
/// ignored — close the window to leave. The version string turns amber as
/// the "this is a replay" tell.
// macroquad's future runs on the main thread; Send is not on the menu.
#[allow(clippy::future_not_send)]
async fn replay_session(path: Option<String>) {
    let Some(path) = path else {
        eprintln!("usage: space-trucking --replay <file>");
        return;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("cannot read {path}: {err}");
            return;
        }
    };
    let recording = match Recording::parse(&text) {
        Ok(recording) => recording,
        Err(err) => {
            eprintln!("cannot replay {path}: {err}");
            return;
        }
    };
    let Ok(mut sim) = recording.base_sim() else {
        // Unreachable in practice: parse already validated the base.
        eprintln!("cannot replay {path}: base save refused");
        return;
    };
    let mut cursor = recording.cursor();
    let mut audio = Audio::load().await;
    let mut juice = Juice::default();
    // The viewer's own preference, not the recorded session's: reduced
    // motion is presentation, and the tape carries only inputs.
    let reduced_motion = reduced_motion();
    let target = pixel_target();
    let mut accumulator: f32 = 0.0;
    let mut rolling = true;

    loop {
        let view = View::fit(screen_width(), screen_height());
        let dt = get_frame_time();
        if rolling {
            let scale = if sim.is_warp() { WARP_FACTOR } else { 1.0 };
            accumulator += (dt * scale).clamp(0.0, REPLAY_FRAME_CLAMP * scale);
            // The float condition is the fixed-timestep idiom; the clamp
            // above bounds the loop.
            #[allow(clippy::while_float)]
            while rolling && accumulator >= TICK_DT {
                accumulator -= TICK_DT;
                // A refused tick (corrupt tape) simply freezes the frame:
                // the box shows as much of the story as it holds.
                rolling = cursor.next_tick(&mut sim).unwrap_or(false);
            }
        }
        let pointer = cursor.pointer();
        juice.update(dt, &sim, pointer, false);
        audio.update(dt, &sim, false);
        render::draw(
            &view,
            &target,
            &render::Scene {
                sim: &sim,
                juice: &juice,
                pointer,
                // Replays show the recorded hand, never the tutor's.
                ghost: None,
                audio_waiting: audio.needs_gesture(),
                audio_muted: audio.muted(),
                reduced_motion,
                dev: true,
            },
        );
        draw_version(palette::fade(palette::AMBER, 0.75));
        next_frame().await;
    }
}

/// The low-res target the whole fiction renders into. Nearest filtering is
/// what makes the upscale pixels instead of blur. `sample_count` must be 0:
/// even 1 makes macroquad allocate an MSAA-resolve pass whose blit needs
/// WebGL2, and the vendored `gl.js` context is WebGL 1.
#[allow(clippy::cast_sign_loss)] // Both operands are positive constants.
fn pixel_target() -> RenderTarget {
    let target = render_target_ex(
        (WORLD_W / CRUNCH) as u32,
        (WORLD_H / CRUNCH) as u32,
        RenderTargetParams {
            sample_count: 0,
            depth: false,
        },
    );
    target.texture.set_filter(FilterMode::Nearest);
    target
}

/// Load the save and replay the absence, or start fresh. The second value
/// reports whether the ship docked somewhere while the player was away, so
/// the renderer can pulse the dock ring about it; the third is how many
/// seconds of absence were actually replayed, for the telemetry catch-up
/// row.
fn restore() -> (Sim, bool, f64) {
    let Some((save, saved_at)) = storage::load() else {
        return (Sim::new(fresh_seed()), false, 0.0);
    };
    let Ok(mut sim) = Sim::from_save(&save) else {
        return (Sim::new(fresh_seed()), false, 0.0);
    };
    let elapsed = (macroquad::miniquad::date::now() - saved_at).clamp(0.0, MAX_CATCH_UP);
    let ticks = u64::try_from((elapsed * CATCH_UP_RATE) as i64).unwrap_or(0);
    let caught_up = sim.fast_forward(ticks);
    (
        sim,
        caught_up.arrived,
        caught_up.ticks as f64 / CATCH_UP_RATE,
    )
}

/// Construct the telemetry aggregator iff consent is recorded as `"yes"`
/// (docs/TELEMETRY.md): load the stored buffer if it parses — the merge
/// across sessions — count the session, and count the replayed absence.
/// Anything else — declined, or no consent recorded, which is every native
/// build since only the web shell's consent card writes the key — means
/// opt-in never happened: nothing is constructed, nothing will be written,
/// and any previously stored buffer is deleted. Declining cleans up.
fn boot_telemetry(catch_up_seconds: f64) -> Option<Aggregate> {
    if storage::get(CONSENT_KEY).as_deref() == Some("yes") {
        Some(consented_aggregate(catch_up_seconds))
    } else {
        storage::remove(BUFFER_KEY);
        None
    }
}

/// The consented aggregator: the stored buffer if it parses — the merge
/// across sessions — with this session and its replayed absence counted.
/// Shared by boot and by a first visit's late "yes" from the consent card.
fn consented_aggregate(catch_up_seconds: f64) -> Aggregate {
    let mut aggregate = storage::get(BUFFER_KEY)
        .and_then(|text| Aggregate::parse(&text).ok())
        .unwrap_or_default();
    aggregate.note_session();
    aggregate.note_catch_up(catch_up_seconds);
    aggregate
}

/// Whether this frame produced a cue worth writing the save for.
fn save_worthy(sim: &Sim) -> bool {
    sim.cues().iter().any(|cue| {
        matches!(
            cue,
            Cue::Arrive
                | Cue::Depart
                | Cue::Accept { .. }
                | Cue::Place
                | Cue::Pause { .. }
                | Cue::Reseed
        )
    })
}

/// Wall-clock seed for fresh runs; determinism starts once the sim owns it.
fn fresh_seed() -> u64 {
    macroquad::miniquad::date::now().to_bits()
}

/// Letterboxed mapping between the sim's fixed logical world and the window.
pub struct View {
    scale: f32,
    origin: Vec2,
}

impl View {
    #[must_use]
    fn fit(screen_w: f32, screen_h: f32) -> Self {
        // A minimised window or hidden tab reports zero size; the floor keeps
        // the inverse mapping from producing infinities.
        let scale = (screen_w / WORLD_W)
            .min(screen_h / WORLD_H)
            .max(f32::EPSILON);
        Self {
            scale,
            origin: Vec2::new(
                WORLD_W.mul_add(-scale, screen_w) * 0.5,
                WORLD_H.mul_add(-scale, screen_h) * 0.5,
            ),
        }
    }

    #[must_use]
    pub fn to_screen(&self, world: Vec2) -> Vec2 {
        world * self.scale + self.origin
    }

    #[must_use]
    fn to_world(&self, screen: Vec2) -> Vec2 {
        (screen - self.origin) * self.scale.recip()
    }

    /// World-to-screen length factor.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }
}

fn gather_input(view: &View, dev_mode: bool, night: bool) -> InputFrame {
    let (mouse_x, mouse_y) = mouse_position();
    let pointer = view.to_world(Vec2::new(mouse_x, mouse_y));
    let press = is_mouse_button_pressed(MouseButton::Left);
    InputFrame {
        pointer,
        press,
        held: is_mouse_button_down(MouseButton::Left),
        release: is_mouse_button_released(MouseButton::Left),
        // The icon buttons fold into the same toggles as the keys; the sim
        // deliberately ignores presses on those rects.
        toggle_pause: is_key_pressed(KeyCode::Space)
            || (press && layout::PAUSE_BTN.contains(pointer)),
        toggle_warp: dev_mode
            && (is_key_pressed(KeyCode::F) || (press && layout::WARP_BTN.contains(pointer))),
        shift: is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift),
        night,
        reseed: is_key_pressed(KeyCode::R).then(fresh_seed),
    }
}

/// The one permitted piece of text: the version, bottom-right. The colour is
/// a palette role — live play uses [`palette::VERSION_TEXT`], replay tints
/// it amber as its only tell.
fn draw_version(color: Color) {
    let version = format!("{} {VERSION}", env!("CARGO_PKG_NAME"));
    let size = measure_text(&version, None, TEXT_SIZE, 1.0);
    draw_text(
        &version,
        screen_width() - size.width - TEXT_MARGIN,
        screen_height() - TEXT_MARGIN,
        f32::from(TEXT_SIZE),
        color,
    );
}
