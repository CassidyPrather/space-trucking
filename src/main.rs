//! macroquad frontend: input in, the ship console out.
//!
//! Everything that decides what happens lives in [`space_trucking::sim`].
//! This file only translates between the window and the sim's logical world:
//! it gathers an [`InputFrame`] each frame (folding the console's icon
//! buttons into the pause/warp/mute toggles), advances the sim, and hands
//! the result to [`render`] and [`audio`]. It also owns the save slot —
//! load-and-catch-up on startup, cue-driven autosave forever after — and the
//! game's one and only piece of text, the version string.

mod audio;
mod juice;
mod render;
mod storage;

use audio::Audio;
use juice::Juice;
use macroquad::color::Color;
use macroquad::input::{
    KeyCode, MouseButton, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed,
    is_mouse_button_released, mouse_position,
};
use macroquad::text::{draw_text, measure_text};
use macroquad::time::get_frame_time;
use macroquad::window::{Conf, next_frame, screen_height, screen_width};

use space_trucking::VERSION;
use space_trucking::sim::{Cue, InputFrame, Sim, Vec2, WORLD_H, WORLD_W, layout};

const OVERLAY: Color = Color::new(0.85, 0.85, 0.90, 0.75);
const TEXT_SIZE: u16 = 16;
const TEXT_MARGIN: f32 = 8.0;

/// Seconds between wall-clock autosaves; cue-driven saves come sooner.
const SAVE_EVERY: f64 = 10.0;

/// Longest absence the startup catch-up replays, in seconds (six hours).
const MAX_CATCH_UP: f64 = 6.0 * 3600.0;

/// Sim ticks per wall-clock second of absence.
const CATCH_UP_RATE: f64 = 60.0;

fn window_conf() -> Conf {
    Conf {
        window_title: env!("CARGO_PKG_NAME").to_owned(),
        window_width: WORLD_W as i32,
        window_height: WORLD_H as i32,
        high_dpi: true,
        ..Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let (mut sim, arrived_while_away) = restore();
    let mut audio = Audio::load().await;
    let mut juice = Juice::default();
    if arrived_while_away {
        juice.catch_up_arrival();
    }
    let mut last_save = macroquad::miniquad::date::now();

    loop {
        let view = View::fit(screen_width(), screen_height());
        let input = gather_input(&view);
        let toggle_mute =
            is_key_pressed(KeyCode::M) || (input.press && layout::SPEAKER.contains(input.pointer));
        let dt = get_frame_time();

        sim.advance(dt, &input);
        juice.update(dt, &sim, input.pointer, input.press);
        audio.update(dt, &sim, toggle_mute);

        let now = macroquad::miniquad::date::now();
        if save_worthy(&sim) || now - last_save >= SAVE_EVERY {
            storage::store(&sim.save_string(), now);
            last_save = now;
        }

        render::draw(
            &view,
            &render::Scene {
                sim: &sim,
                juice: &juice,
                pointer: input.pointer,
                audio_waiting: audio.needs_gesture(),
                audio_muted: audio.muted(),
            },
        );
        draw_version();
        next_frame().await;
    }
}

/// Load the save and replay the absence, or start fresh. The second value
/// reports whether the ship docked somewhere while the player was away, so
/// the renderer can pulse the dock ring about it.
fn restore() -> (Sim, bool) {
    let Some((save, saved_at)) = storage::load() else {
        return (Sim::new(fresh_seed()), false);
    };
    let Ok(mut sim) = Sim::from_save(&save) else {
        return (Sim::new(fresh_seed()), false);
    };
    let elapsed = (macroquad::miniquad::date::now() - saved_at).clamp(0.0, MAX_CATCH_UP);
    let ticks = u64::try_from((elapsed * CATCH_UP_RATE) as i64).unwrap_or(0);
    let caught_up = sim.fast_forward(ticks);
    (sim, caught_up.arrived)
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

fn gather_input(view: &View) -> InputFrame {
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
        toggle_warp: is_key_pressed(KeyCode::F) || (press && layout::WARP_BTN.contains(pointer)),
        reseed: is_key_pressed(KeyCode::R).then(fresh_seed),
    }
}

/// The one permitted piece of text: the version, bottom-right.
fn draw_version() {
    let version = format!("{} {VERSION}", env!("CARGO_PKG_NAME"));
    let size = measure_text(&version, None, TEXT_SIZE, 1.0);
    draw_text(
        &version,
        screen_width() - size.width - TEXT_MARGIN,
        screen_height() - TEXT_MARGIN,
        f32::from(TEXT_SIZE),
        OVERLAY,
    );
}
