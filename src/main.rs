//! macroquad frontend: input in, placeholder shapes out.
//!
//! Everything that decides what happens lives in [`space_trucking::sim`].
//! This file only translates between the window and the sim's logical world;
//! the drawing below is throwaway scaffolding for the renderer stage — POIs
//! as circles, the ship as a dot, cargo as coloured rects — with the version
//! string as the game's one and only piece of text.

mod audio;
#[allow(dead_code)] // Wired into the loop by the persistence stage.
mod storage;

use audio::Audio;
use macroquad::color::Color;
use macroquad::input::{
    KeyCode, MouseButton, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed,
    is_mouse_button_released, mouse_position,
};
use macroquad::shapes::{draw_circle, draw_rectangle, draw_rectangle_lines};
use macroquad::text::{draw_text, measure_text};
use macroquad::time::get_frame_time;
use macroquad::window::{Conf, clear_background, next_frame, screen_height, screen_width};

use space_trucking::VERSION;
use space_trucking::sim::{InputFrame, Kind, Sim, Vec2, WORLD_H, WORLD_W, layout};

const BACKGROUND: Color = Color::new(0.05, 0.05, 0.07, 1.0);
const POI_COLOR: Color = Color::new(0.55, 0.60, 0.75, 1.0);
const SHIP_COLOR: Color = Color::new(0.95, 0.85, 0.55, 1.0);
const GRID_COLOR: Color = Color::new(0.25, 0.27, 0.33, 1.0);
const OVERLAY: Color = Color::new(0.85, 0.85, 0.90, 0.75);
const MUTED_OVERLAY: Color = Color::new(0.55, 0.55, 0.60, 0.55);
const HIGHLIGHT: Color = Color::new(1.0, 0.83, 0.42, 0.95);
const TEXT_SIZE: u16 = 16;
const TEXT_MARGIN: f32 = 8.0;

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
    let mut sim = Sim::new(fresh_seed());
    let mut audio = Audio::load().await;

    loop {
        let view = View::fit(screen_width(), screen_height());
        let input = gather_input(&view);
        let toggle_mute = is_key_pressed(KeyCode::M);
        let dt = get_frame_time();

        sim.advance(dt, &input);
        audio.update(dt, &sim, toggle_mute);
        draw(&sim, &view, &audio);
        next_frame().await;
    }
}

/// Wall-clock seed for fresh runs; determinism starts once the sim owns it.
fn fresh_seed() -> u64 {
    macroquad::miniquad::date::now().to_bits()
}

/// Letterboxed mapping between the sim's fixed logical world and the window.
struct View {
    scale: f32,
    origin: Vec2,
}

impl View {
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

    fn to_screen(&self, world: Vec2) -> Vec2 {
        world * self.scale + self.origin
    }

    fn to_world(&self, screen: Vec2) -> Vec2 {
        (screen - self.origin) * self.scale.recip()
    }
}

fn gather_input(view: &View) -> InputFrame {
    let (mouse_x, mouse_y) = mouse_position();
    InputFrame {
        pointer: view.to_world(Vec2::new(mouse_x, mouse_y)),
        press: is_mouse_button_pressed(MouseButton::Left),
        held: is_mouse_button_down(MouseButton::Left),
        release: is_mouse_button_released(MouseButton::Left),
        toggle_pause: is_key_pressed(KeyCode::Space),
        toggle_warp: is_key_pressed(KeyCode::F),
        reseed: is_key_pressed(KeyCode::R).then(fresh_seed),
    }
}

/// Flat placeholder colour per cargo kind; sprites are the renderer stage's.
const fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::PerfumeVial => Color::new(0.85, 0.55, 0.75, 1.0),
        Kind::GildedIdol => Color::new(0.90, 0.75, 0.30, 1.0),
        Kind::RationBricks => Color::new(0.60, 0.45, 0.30, 1.0),
        Kind::ScrapAlloy => Color::new(0.55, 0.55, 0.60, 1.0),
        Kind::Seedlings => Color::new(0.45, 0.75, 0.40, 1.0),
        Kind::GasCanister => Color::new(0.80, 0.45, 0.25, 1.0),
        Kind::CryoCore => Color::new(0.45, 0.75, 0.90, 1.0),
        Kind::BrinePearls => Color::new(0.75, 0.80, 0.85, 1.0),
        Kind::SuspiciousCrate => Color::new(0.50, 0.30, 0.55, 1.0),
    }
}

fn draw_rect(view: &View, rect: layout::Rect, color: Color, filled: bool) {
    let pos = view.to_screen(Vec2::new(rect.x, rect.y));
    let (w, h) = (rect.w * view.scale, rect.h * view.scale);
    if filled {
        draw_rectangle(pos.x, pos.y, w, h, color);
    } else {
        draw_rectangle_lines(pos.x, pos.y, w, h, 1.0, color);
    }
}

fn draw(sim: &Sim, view: &View, audio: &Audio) {
    clear_background(BACKGROUND);

    for poi in sim.pois() {
        let pos = view.to_screen(poi.pos);
        draw_circle(pos.x, pos.y, poi.radius * view.scale, POI_COLOR);
    }

    for y in 0..layout::GRID_ROWS {
        for x in 0..layout::GRID_COLS {
            draw_rect(view, layout::cell_rect(x, y), GRID_COLOR, false);
        }
    }

    for piece in sim.pieces() {
        draw_rect(
            view,
            layout::piece_rect(piece),
            kind_color(piece.kind),
            true,
        );
    }

    let ship = view.to_screen(sim.ship().interpolated(sim.alpha()));
    draw_circle(ship.x, ship.y, 5.0 * view.scale, SHIP_COLOR);

    // The one permitted piece of text: the version, bottom-right.
    let version = format!("{} {VERSION}", env!("CARGO_PKG_NAME"));
    let size = measure_text(&version, None, TEXT_SIZE, 1.0);
    let color = if audio.needs_gesture() {
        HIGHLIGHT
    } else if audio.muted() {
        MUTED_OVERLAY
    } else {
        OVERLAY
    };
    draw_text(
        &version,
        screen_width() - size.width - TEXT_MARGIN,
        screen_height() - TEXT_MARGIN,
        f32::from(TEXT_SIZE),
        color,
    );
}
