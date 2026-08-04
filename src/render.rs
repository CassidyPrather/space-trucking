//! The ship console: every pixel of the game, drawn from sim accessors.
//!
//! No text, no numbers, no labels — the one string the game ever renders
//! lives in `main.rs`. Everything here communicates by shape, colour, and
//! motion: hand-built planet glyphs, cargo silhouettes from primitives,
//! levers that glow when they will work and shake when they will not. All
//! geometry comes from [`layout`], the same constants the sim hit-tests
//! against, so things are always drawn exactly where they can be clicked.
//!
//! The [`Canvas`] wrapper applies the letterbox transform and the console
//! light level ([`Sim::light`]) plus the omen's violet cast to every draw
//! call, so the whole palette dims as one when the event wants it dark.

use std::f32::consts::{PI, TAU};

use macroquad::color::Color;
use macroquad::math::vec2;
use macroquad::shapes::{
    draw_arc, draw_circle, draw_circle_lines, draw_ellipse, draw_ellipse_lines, draw_hexagon,
    draw_line, draw_poly, draw_poly_lines, draw_rectangle, draw_rectangle_lines, draw_triangle,
    draw_triangle_lines,
};
use macroquad::window::{clear_background, screen_height, screen_width};

use space_trucking::sim::{
    Barter, EAGER_MAX, Kind, Loc, POIS, Piece, PoiId, ShipState, Sim, Vec2, Violation, layout,
    placement_check, splitmix,
};

use crate::View;
use crate::juice::Juice;

/// Everything one frame of drawing needs, gathered by `main`.
pub struct Scene<'a> {
    pub sim: &'a Sim,
    pub juice: &'a Juice,
    /// Pointer position in world coordinates, for ghosts and hover glows.
    pub pointer: Vec2,
    /// Audio still waits on the browser's first-gesture rule.
    pub audio_waiting: bool,
    /// The player has muted.
    pub audio_muted: bool,
}

// ---------------------------------------------------------------- palette --

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

/// Alpha-scaled copy of `col`.
const fn fade(col: Color, alpha: f32) -> Color {
    Color::new(col.r, col.g, col.b, col.a * alpha)
}

/// Brightness-scaled copy of `col`.
const fn dim(col: Color, by: f32) -> Color {
    Color::new(col.r * by, col.g * by, col.b * by, col.a)
}

const BG: Color = rgb(0.030, 0.034, 0.050);
const MAP_PLATE: Color = rgb(0.043, 0.048, 0.066);
const PLATE: Color = rgb(0.075, 0.080, 0.104);
const PLATE_DEEP: Color = rgb(0.058, 0.062, 0.082);
const PLATE_EDGE: Color = rgb(0.210, 0.230, 0.300);
const SOCKET: Color = rgb(0.045, 0.049, 0.068);
const SOCKET_EDGE: Color = rgb(0.150, 0.160, 0.215);
const STAR: Color = rgb(0.80, 0.85, 0.95);
const ROUTE: Color = rgb(0.70, 0.76, 0.92);
const RING: Color = rgb(0.75, 0.80, 0.95);
const SHIP: Color = rgb(0.95, 0.86, 0.55);
const ENGINE: Color = rgb(1.00, 0.62, 0.25);
const AMBER: Color = rgb(1.00, 0.72, 0.33);
const AMBER_EDGE: Color = rgb(1.00, 0.85, 0.55);
const HANDLE_DIM: Color = rgb(0.30, 0.31, 0.38);
const ICON_DIM: Color = rgb(0.42, 0.44, 0.54);
const ICON_LIT: Color = rgb(0.78, 0.81, 0.92);
const GOOD: Color = rgb(0.42, 0.85, 0.48);
const BAD: Color = rgb(0.92, 0.30, 0.28);
const VIOLET: Color = rgb(0.55, 0.32, 0.75);
const VIOLET_FLASH: Color = rgb(0.70, 0.45, 0.95);
const WHITE: Color = rgb(0.95, 0.96, 1.00);
const INK: Color = rgb(0.0, 0.0, 0.0);

const SHELF_EDGE: Color = rgb(0.34, 0.28, 0.42);
const GIVE_EDGE: Color = rgb(0.42, 0.34, 0.22);
const TAKE_EDGE: Color = rgb(0.22, 0.36, 0.42);
const RECEIVED_EDGE: Color = rgb(0.24, 0.38, 0.28);

const VENUS: Color = rgb(0.94, 0.70, 0.55);
const VENUS_HALO: Color = rgb(1.00, 0.86, 0.60);
const EARTH: Color = rgb(0.45, 0.55, 0.65);
const SMOG: Color = rgb(0.48, 0.36, 0.24);
const MARS: Color = rgb(0.76, 0.34, 0.22);
const MARS_PATCH: Color = rgb(0.58, 0.24, 0.17);
const JUPITER: Color = rgb(0.86, 0.60, 0.34);
const URANUS: Color = rgb(0.62, 0.85, 0.88);
const URANUS_RING: Color = rgb(0.72, 0.84, 0.90);
const NEPTUNE: Color = rgb(0.26, 0.40, 0.85);
const GUILD_FILL: Color = rgb(0.42, 0.38, 0.55);
const GUILD_EDGE: Color = rgb(0.62, 0.56, 0.78);

/// The colour the omen leans everything toward.
const CAST: (f32, f32, f32) = (0.42, 0.22, 0.58);

/// Flat base colour per cargo kind; variants tint it.
const fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::PerfumeVial => rgb(0.90, 0.55, 0.75),
        Kind::GildedIdol => rgb(0.92, 0.76, 0.30),
        Kind::RationBricks => rgb(0.55, 0.55, 0.30),
        Kind::ScrapAlloy => rgb(0.70, 0.42, 0.28),
        Kind::Seedlings => rgb(0.45, 0.78, 0.40),
        Kind::GasCanister => rgb(0.90, 0.52, 0.22),
        Kind::CryoCore => rgb(0.45, 0.80, 0.92),
        Kind::BrinePearls => rgb(0.60, 0.70, 0.88),
        Kind::SuspiciousCrate => rgb(0.10, 0.09, 0.12),
    }
}

/// Sibling-crate hue shifts, one per cosmetic variant roll.
const VARIANT_SHIFT: [(f32, f32, f32); 4] = [
    (0.0, 0.0, 0.0),
    (0.06, -0.03, -0.02),
    (-0.05, 0.04, 0.02),
    (0.02, 0.02, -0.06),
];

fn variant_tint(col: Color, variant: u8) -> Color {
    let (dr, dg, db) = VARIANT_SHIFT[usize::from(variant % 4)];
    Color::new(
        (col.r + dr).clamp(0.0, 1.0),
        (col.g + dg).clamp(0.0, 1.0),
        (col.b + db).clamp(0.0, 1.0),
        col.a,
    )
}

/// Inner-detail scale per variant, `0.85..=1.15`.
fn variant_scale(variant: u8) -> f32 {
    f32::from(variant % 4).mul_add(0.1, 0.85)
}

// ------------------------------------------------------------- small math --

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        lerp(a.r, b.r, t),
        lerp(a.g, b.g, t),
        lerp(a.b, b.b, t),
        lerp(a.a, b.a, t),
    )
}

/// Cubic ease-out over `0..=1`.
fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t;
    u.mul_add(-u * u, 1.0)
}

/// Point at `radius` along `angle` (radians) from `from`.
fn polar(from: Vec2, radius: f32, angle: f32) -> Vec2 {
    Vec2::new(
        radius.mul_add(angle.cos(), from.x),
        radius.mul_add(angle.sin(), from.y),
    )
}

fn rect_center(r: layout::Rect) -> Vec2 {
    Vec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y))
}

fn inflate(r: layout::Rect, by: f32) -> layout::Rect {
    layout::Rect::new(
        r.x - by,
        r.y - by,
        2.0f32.mul_add(by, r.w),
        2.0f32.mul_add(by, r.h),
    )
}

/// `r` scaled by `s` around its centre.
fn scaled(r: layout::Rect, s: f32) -> layout::Rect {
    let w = r.w * s;
    let h = r.h * s;
    layout::Rect::new(
        (r.w - w).mul_add(0.5, r.x),
        (r.h - h).mul_add(0.5, r.y),
        w,
        h,
    )
}

fn shifted_rect(r: layout::Rect, dx: f32, dy: f32) -> layout::Rect {
    layout::Rect::new(r.x + dx, r.y + dy, r.w, r.h)
}

fn rect_lerp(a: layout::Rect, b: layout::Rect, t: f32) -> layout::Rect {
    layout::Rect::new(
        lerp(a.x, b.x, t),
        lerp(a.y, b.y, t),
        lerp(a.w, b.w, t),
        lerp(a.h, b.h, t),
    )
}

/// The whole hold grid as one world rect.
fn grid_rect() -> layout::Rect {
    layout::Rect::new(
        layout::GRID_ORIGIN.x,
        layout::GRID_ORIGIN.y,
        f32::from(layout::GRID_COLS) * layout::CELL,
        f32::from(layout::GRID_ROWS) * layout::CELL,
    )
}

/// Whether a piece at `loc` belongs to the player (may enter the hold, may
/// be abandoned to the shelf) rather than to the station.
const fn ours(loc: Loc) -> bool {
    matches!(
        loc,
        Loc::Hold { .. } | Loc::GivePad { .. } | Loc::ReceivedShelf { .. }
    )
}

/// Whether departure would strand pieces: the launch lever's dim condition.
fn pads_occupied(sim: &Sim) -> bool {
    sim.pieces().iter().any(|piece| {
        matches!(
            piece.loc,
            Loc::GivePad { .. } | Loc::TakePad { .. } | Loc::ReceivedShelf { .. }
        )
    })
}

fn piece_by_id(sim: &Sim, id: u32) -> Option<&Piece> {
    sim.pieces().iter().find(|piece| piece.id == id)
}

/// Hold-grid shake offset during a hard-reject rattle.
fn grid_shake_offset(juice: &Juice) -> Vec2 {
    let heat = juice.grid_shake();
    let t = juice.clock();
    Vec2::new((t * 61.0).sin() * 2.6 * heat, (t * 83.0).sin() * 1.8 * heat)
}

// ------------------------------------------------------------------ canvas --

/// World-to-screen draw helper: applies the letterbox transform, a cosmetic
/// world-space offset (shakes), the console light level, and the omen cast
/// to every call.
#[derive(Clone, Copy)]
struct Canvas<'a> {
    view: &'a View,
    light: f32,
    omen: f32,
    offset: Vec2,
}

impl Canvas<'_> {
    fn shifted(&self, by: Vec2) -> Self {
        Self {
            offset: self.offset + by,
            ..*self
        }
    }

    fn point(&self, p: Vec2) -> Vec2 {
        self.view.to_screen(p + self.offset)
    }

    fn s(&self, v: f32) -> f32 {
        v * self.view.scale()
    }

    /// Line width in pixels: scaled, but never below one.
    fn stroke(&self, w: f32) -> f32 {
        self.s(w).max(1.0)
    }

    /// The whole palette runs through here: light level times omen cast.
    fn tint(&self, col: Color) -> Color {
        let cast = self.omen * 0.35;
        Color::new(
            (CAST.0 - col.r).mul_add(cast, col.r) * self.light,
            (CAST.1 - col.g).mul_add(cast, col.g) * self.light,
            (CAST.2 - col.b).mul_add(cast, col.b) * self.light,
            col.a,
        )
    }

    fn fill(&self, r: layout::Rect, col: Color) {
        let p = self.point(Vec2::new(r.x, r.y));
        draw_rectangle(p.x, p.y, self.s(r.w), self.s(r.h), self.tint(col));
    }

    fn frame(&self, r: layout::Rect, col: Color) {
        self.frame_thick(r, 1.0, col);
    }

    fn frame_thick(&self, r: layout::Rect, w: f32, col: Color) {
        let p = self.point(Vec2::new(r.x, r.y));
        draw_rectangle_lines(
            p.x,
            p.y,
            self.s(r.w),
            self.s(r.h),
            self.stroke(w),
            self.tint(col),
        );
    }

    fn dot(&self, at: Vec2, r: f32, col: Color) {
        let p = self.point(at);
        draw_circle(p.x, p.y, self.s(r), self.tint(col));
    }

    fn ring(&self, at: Vec2, r: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_circle_lines(p.x, p.y, self.s(r), self.stroke(w), self.tint(col));
    }

    fn seg(&self, a: Vec2, b: Vec2, w: f32, col: Color) {
        let pa = self.point(a);
        let pb = self.point(b);
        draw_line(pa.x, pa.y, pb.x, pb.y, self.stroke(w), self.tint(col));
    }

    fn tri(&self, a: Vec2, b: Vec2, d: Vec2, col: Color) {
        let (pa, pb, pd) = (self.point(a), self.point(b), self.point(d));
        draw_triangle(
            vec2(pa.x, pa.y),
            vec2(pb.x, pb.y),
            vec2(pd.x, pd.y),
            self.tint(col),
        );
    }

    fn tri_ring(&self, a: Vec2, b: Vec2, d: Vec2, w: f32, col: Color) {
        let (pa, pb, pd) = (self.point(a), self.point(b), self.point(d));
        draw_triangle_lines(
            vec2(pa.x, pa.y),
            vec2(pb.x, pb.y),
            vec2(pd.x, pd.y),
            self.stroke(w),
            self.tint(col),
        );
    }

    fn poly(&self, at: Vec2, sides: u8, r: f32, rot: f32, col: Color) {
        let p = self.point(at);
        draw_poly(p.x, p.y, sides, self.s(r), rot, self.tint(col));
    }

    fn poly_ring(&self, at: Vec2, sides: u8, r: f32, rot: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_poly_lines(
            p.x,
            p.y,
            sides,
            self.s(r),
            rot,
            self.stroke(w),
            self.tint(col),
        );
    }

    /// Arc from `rot` degrees, sweeping `sweep` degrees clockwise, drawn
    /// outward from `r`.
    fn arc(&self, at: Vec2, r: f32, rot: f32, sweep: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_arc(
            p.x,
            p.y,
            48,
            self.s(r),
            rot,
            self.stroke(w),
            sweep,
            self.tint(col),
        );
    }

    fn oval(&self, at: Vec2, rx: f32, ry: f32, rot: f32, col: Color) {
        let p = self.point(at);
        draw_ellipse(p.x, p.y, self.s(rx), self.s(ry), rot, self.tint(col));
    }

    fn oval_ring(&self, at: Vec2, rx: f32, ry: f32, rot: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_ellipse_lines(
            p.x,
            p.y,
            self.s(rx),
            self.s(ry),
            rot,
            self.stroke(w),
            self.tint(col),
        );
    }

    fn hexagon(&self, at: Vec2, r: f32, edge: Color, fill: Color) {
        let p = self.point(at);
        draw_hexagon(
            p.x,
            p.y,
            self.s(r),
            self.stroke(1.0),
            true,
            self.tint(edge),
            self.tint(fill),
        );
    }
}

// ------------------------------------------------------------------- entry --

/// Draw the whole console for one frame.
pub fn draw(view: &View, scene: &Scene) {
    let canvas = Canvas {
        view,
        light: scene.sim.light(),
        omen: scene.sim.omen(),
        offset: Vec2::default(),
    };
    clear_background(canvas.tint(BG));
    draw_map(&canvas, scene);
    draw_console(&canvas, scene);
    draw_hold(&canvas, scene);
    if let Some(barter) = scene.sim.barter() {
        draw_barter(&canvas, scene, barter);
    }
    draw_pieces(&canvas, scene);
    draw_violation_flash(&canvas, scene);
    draw_held(&canvas, scene);
    draw_screen_fx(scene);
}

// --------------------------------------------------------------------- map --

fn draw_map(c: &Canvas, scene: &Scene) {
    c.fill(layout::MAP_PANEL, MAP_PLATE);
    c.frame(layout::MAP_PANEL, PLATE_EDGE);
    draw_starfield(c, scene.juice.clock());
    draw_route_and_ship(c, scene);
    draw_pois(c, scene);
}

/// Fixed render-side seed for the starfield hash; cosmetic only.
const STAR_SEED: u64 = 0x57A2_F1E1;
const STAR_COUNT: u64 = 110;

fn draw_starfield(c: &Canvas, t: f32) {
    let area = layout::MAP_PANEL;
    for i in 0..STAR_COUNT {
        let hash = splitmix(STAR_SEED, i);
        let fx = (hash & 0xFFFF) as f32 / 65_535.0;
        let fy = ((hash >> 16) & 0xFFFF) as f32 / 65_535.0;
        let base = ((hash >> 32) & 0xFF) as f32 / 255.0;
        let phase = ((hash >> 40) & 0xFF) as f32 / 255.0 * TAU;
        let speed = (((hash >> 48) & 0x3) as f32).mul_add(0.6, 0.4);
        let star_x = fx.mul_add(area.w - 8.0, area.x + 4.0);
        let star_y = fy.mul_add(area.h - 8.0, area.y + 4.0);
        let twinkle = 0.15 * speed.mul_add(t, phase).sin();
        let alpha = (base.mul_add(0.45, 0.2) + twinkle).clamp(0.05, 0.8);
        let size = if hash & 0x300 == 0 { 2.0 } else { 1.3 };
        c.fill(
            layout::Rect::new(star_x, star_y, size, size),
            fade(STAR, alpha),
        );
    }
}

fn draw_route_and_ship(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let ShipState::Traveling { from, to, .. } = sim.ship().state else {
        return;
    };
    let a = POIS[usize::from(from)].pos;
    let b = POIS[usize::from(to)].pos;
    let span = b - a;
    let length = span.length();
    if length <= f32::EPSILON {
        return;
    }
    let dir = span * length.recip();

    // Dotted route: short dashes from origin to destination.
    let step = 13.0;
    let dash = 6.0;
    let count = ((length / step) as i32).max(0);
    for i in 0..count {
        let at = (i as f32) * step;
        c.seg(
            a + dir * at,
            a + dir * (at + dash).min(length),
            1.0,
            fade(ROUTE, 0.35),
        );
    }

    // The freighter: a small triangle nosing along the route, with an
    // engine-flicker triangle behind it (bigger and faster under warp).
    let pos = sim.ship().interpolated(sim.alpha());
    let perp = Vec2::new(-dir.y, dir.x);
    let t = scene.juice.clock();
    let (freq, boost): (f32, f32) = if sim.is_warp() {
        (26.0, 1.9)
    } else {
        (9.0, 1.0)
    };
    let flick = 0.3f32.mul_add((t * freq).sin(), 0.7);
    let tail = (3.5 * boost).mul_add(flick, 3.0);
    let stern = pos - dir * 3.0;
    c.tri(
        stern + perp * 1.7,
        stern - perp * 1.7,
        stern - dir * tail,
        fade(ENGINE, 0.85),
    );
    c.tri(
        pos + dir * 6.0,
        stern + perp * 3.5,
        stern - perp * 3.5,
        SHIP,
    );
}

fn draw_pois(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let t = scene.juice.clock();
    let docked = match sim.ship().state {
        ShipState::Docked(at) => Some(at),
        ShipState::Traveling { .. } => None,
    };
    for (i, poi) in sim.pois().iter().enumerate() {
        poi_glyph(c, i, poi.pos, poi.radius, t);
        let id = i as PoiId;

        // Hover: a faint ring over any POI the player could pick right now.
        if let Some(at) = docked {
            if id != at && (scene.pointer - poi.pos).length() <= poi.radius {
                c.ring(poi.pos, poi.radius + 3.0, 1.0, fade(RING, 0.3));
            }
        }

        // Selected: a pulsing ring, plus a blip right when picked.
        if sim.ship().selected == Some(id) {
            let wave = (t * 4.0).sin();
            c.ring(
                poi.pos,
                wave.mul_add(1.5, poi.radius + 5.0),
                1.4,
                fade(RING, wave.mul_add(0.15, 0.7)),
            );
            let blip = scene.juice.select_blip();
            if blip > 0.0 {
                c.ring(
                    poi.pos,
                    (1.0 - blip).mul_add(9.0, poi.radius + 4.0),
                    1.2,
                    fade(RING, blip * 0.8),
                );
            }
        }

        // Docked: a solid ring; pop on arrival; a long pulse after an
        // offline catch-up so the returning player spots where they landed.
        if docked == Some(id) {
            c.ring(poi.pos, poi.radius + 4.0, 1.6, fade(RING, 0.85));
            let pop = scene.juice.dock_pop();
            if pop > 0.0 {
                c.ring(
                    poi.pos,
                    (1.0 - pop).mul_add(12.0, poi.radius + 4.0),
                    1.5,
                    fade(RING, pop * 0.9),
                );
            }
            let pulse = scene.juice.dock_pulse();
            if pulse > 0.0 {
                let wave = (t * 5.0).sin();
                c.ring(
                    poi.pos,
                    wave.mul_add(2.5, poi.radius + 7.0),
                    1.5,
                    fade(AMBER, wave.mul_add(0.25, 0.45) * (pulse * 3.0).min(1.0)),
                );
            }
        }
    }
}

// -------------------------------------------------------------- POI glyphs --

/// One hand-drawn glyph per POI index, shared by the map and the preview.
fn poi_glyph(c: &Canvas, id: usize, pos: Vec2, r: f32, t: f32) {
    match id {
        0 => venus_glyph(c, pos, r, t),
        1 => earth_glyph(c, pos, r),
        2 => mars_glyph(c, pos, r),
        3 => jupiter_glyph(c, pos, r),
        4 => uranus_glyph(c, pos, r),
        5 => neptune_glyph(c, pos, r),
        _ => guild_glyph(c, pos, r, t),
    }
}

/// Venus: pink-gold, with a glittery halo the rich paid extra for.
fn venus_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32) {
    c.dot(pos, r, VENUS);
    c.ring(pos, r * 1.45, 1.0, fade(VENUS_HALO, 0.5));
    for i in 0..3_u8 {
        let angle = f32::from(i).mul_add(2.1, t * 0.5);
        let sparkle = polar(pos, r * 1.45, angle);
        let glint = (f32::from(i).mul_add(1.7, t * 3.0)).sin();
        c.dot(
            sparkle,
            r.mul_add(0.10, 0.6),
            fade(WHITE, glint.mul_add(0.3, 0.65)),
        );
    }
}

/// Earth: blue-gray under a brown smog ring.
fn earth_glyph(c: &Canvas, pos: Vec2, r: f32) {
    c.dot(pos, r, EARTH);
    c.arc(
        pos,
        r * 1.1,
        -160.0,
        240.0,
        (r * 0.16).max(1.2),
        fade(SMOG, 0.85),
    );
}

/// Mars: rust-red with a patchwork wedge of the scrappy republic.
fn mars_glyph(c: &Canvas, pos: Vec2, r: f32) {
    c.dot(pos, r, MARS);
    c.tri(
        pos,
        polar(pos, r * 0.96, -0.7),
        polar(pos, r * 0.96, 0.45),
        MARS_PATCH,
    );
}

/// Jupiter: a banded orange ellipse.
fn jupiter_glyph(c: &Canvas, pos: Vec2, r: f32) {
    let rx = r * 1.12;
    let ry = r * 0.82;
    c.oval(pos, rx, ry, 0.0, JUPITER);
    for (fy, shade) in [(-0.42_f32, 0.78), (0.05, 0.66), (0.5, 0.8)] {
        let chord = rx * fy.mul_add(-fy, 1.0_f32).sqrt() * 0.94;
        c.oval(
            pos + Vec2::new(0.0, fy * ry),
            chord,
            ry * 0.12,
            0.0,
            dim(JUPITER, shade),
        );
    }
}

/// Uranus: pale cyan with a thin tilted ring.
fn uranus_glyph(c: &Canvas, pos: Vec2, r: f32) {
    c.dot(pos, r * 0.92, URANUS);
    c.oval_ring(pos, r * 1.7, r * 0.5, 20.0, 1.0, fade(URANUS_RING, 0.7));
}

/// Neptune: deep blue with a faint white streak of storm.
fn neptune_glyph(c: &Canvas, pos: Vec2, r: f32) {
    c.dot(pos, r, NEPTUNE);
    c.seg(
        pos + Vec2::new(-r * 0.5, -r * 0.3),
        pos + Vec2::new(r * 0.6, -r * 0.15),
        (r * 0.16).max(1.0),
        fade(WHITE, 0.5),
    );
}

/// The Guild Station: a gray-violet hexagon, pulsing like it knows things.
fn guild_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32) {
    let pulse = (t * 2.4).sin().mul_add(0.05, 1.0);
    c.hexagon(pos, r * pulse, GUILD_EDGE, GUILD_FILL);
}

// ----------------------------------------------------------------- console --

fn draw_console(c: &Canvas, scene: &Scene) {
    c.fill(layout::CONSOLE, PLATE);
    c.frame(layout::CONSOLE, PLATE_EDGE);
    draw_preview(c, scene);
    draw_eta(c, scene);
    draw_launch_lever(c, scene);
    draw_toggle_buttons(c, scene);
}

/// Destination preview: the big glyph of where you are going — or, dimmed,
/// where you already are.
fn draw_preview(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    c.fill(layout::DEST_PREVIEW, SOCKET);
    c.frame(layout::DEST_PREVIEW, SOCKET_EDGE);
    let (id, bright) = match sim.ship().state {
        ShipState::Traveling { to, .. } => (to, true),
        ShipState::Docked(at) => sim.ship().selected.map_or((at, false), |sel| (sel, true)),
    };
    poi_glyph(
        c,
        usize::from(id),
        rect_center(layout::DEST_PREVIEW),
        52.0,
        scene.juice.clock(),
    );
    if !bright {
        c.fill(layout::DEST_PREVIEW, fade(BG, 0.55));
    }
}

/// ETA arc: a ring around nothing that drains as the leg completes; a full
/// dim ring while docked with a destination armed.
fn draw_eta(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let mid = layout::ETA_ARC_CENTER;
    let radius = layout::ETA_ARC_RADIUS;
    c.ring(mid, radius, 1.0, fade(RING, 0.18));
    match sim.ship().state {
        ShipState::Traveling {
            progress,
            leg_ticks,
            ..
        } => {
            let frac = ((progress as f32 + sim.alpha()) / leg_ticks as f32).clamp(0.0, 1.0);
            let sweep = (1.0 - frac) * 360.0;
            if sweep > 0.5 {
                c.arc(mid, radius - 3.0, -90.0, sweep, 5.0, AMBER);
            }
        }
        ShipState::Docked(_) => {
            if sim.ship().selected.is_some() {
                c.arc(mid, radius - 3.0, -90.0, 360.0, 5.0, fade(AMBER, 0.35));
            }
        }
    }
}

/// The launch lever: glowing when a pull would work, dim otherwise, with a
/// thunk slide on departure and a rattle on a refused pull.
fn draw_launch_lever(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let juice = scene.juice;
    let rect = layout::LAUNCH_LEVER;
    c.fill(rect, PLATE_DEEP);
    c.frame(rect, PLATE_EDGE);
    let mid_y = rect.h.mul_add(0.5, rect.y);
    c.seg(
        Vec2::new(rect.x + 16.0, mid_y),
        Vec2::new(rect.x + rect.w - 16.0, mid_y),
        4.0,
        SOCKET,
    );

    let pullable = matches!(sim.ship().state, ShipState::Docked(_))
        && sim.ship().selected.is_some()
        && !pads_occupied(sim);
    let thunk = juice.lever_thunk();
    let pull = if thunk > 0.65 {
        (1.0 - thunk) / 0.35
    } else {
        thunk / 0.65
    };
    let shake = (juice.clock() * 70.0).sin() * 3.0 * juice.lever_shake();
    let x = pull.mul_add(rect.w - 68.0, rect.x + 34.0) + shake;
    let handle = layout::Rect::new(x - 9.0, rect.y + 8.0, 18.0, rect.h - 16.0);
    if pullable {
        let glow = (juice.clock() * 2.2).sin().mul_add(0.1, 0.28);
        c.fill(inflate(handle, 6.0), fade(AMBER, glow));
        c.fill(handle, AMBER);
        c.frame(handle, AMBER_EDGE);
    } else {
        c.fill(handle, HANDLE_DIM);
        c.frame(handle, PLATE_EDGE);
    }
}

fn draw_toggle_buttons(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    button_plate(c, layout::PAUSE_BTN, sim.is_paused());
    button_plate(c, layout::WARP_BTN, sim.is_warp());
    button_plate(c, layout::SPEAKER, scene.audio_waiting);

    // Pause: two bars.
    let mid = rect_center(layout::PAUSE_BTN);
    let bars = if sim.is_paused() { AMBER } else { ICON_DIM };
    c.fill(layout::Rect::new(mid.x - 7.0, mid.y - 9.0, 5.0, 18.0), bars);
    c.fill(layout::Rect::new(mid.x + 2.0, mid.y - 9.0, 5.0, 18.0), bars);

    // Warp: a double chevron.
    let warp_col = if sim.is_warp() { AMBER } else { ICON_DIM };
    chevrons(c, rect_center(layout::WARP_BTN), 7.0, warp_col);

    draw_speaker(c, scene);
}

fn button_plate(c: &Canvas, rect: layout::Rect, lit: bool) {
    c.fill(rect, if lit { fade(AMBER, 0.16) } else { SOCKET });
    c.frame(rect, SOCKET_EDGE);
}

/// Two chevrons pointing right, centred on `mid`.
fn chevrons(c: &Canvas, mid: Vec2, size: f32, col: Color) {
    for off in [-0.75, 0.55] {
        let x = size.mul_add(off, mid.x);
        let nose = Vec2::new(size.mul_add(0.55, x), mid.y);
        let back = size.mul_add(-0.35, x);
        c.seg(Vec2::new(back, mid.y - size), nose, 2.2, col);
        c.seg(nose, Vec2::new(back, mid.y + size), 2.2, col);
    }
}

/// Speaker: pulses amber until the browser lets audio start, slashed while
/// muted.
fn draw_speaker(c: &Canvas, scene: &Scene) {
    let mid = rect_center(layout::SPEAKER);
    let t = scene.juice.clock();
    let col = if scene.audio_waiting {
        fade(AMBER, (t * 5.0).sin().mul_add(0.3, 0.7))
    } else if scene.audio_muted {
        ICON_DIM
    } else {
        ICON_LIT
    };
    c.tri(
        Vec2::new(mid.x - 4.0, mid.y),
        Vec2::new(mid.x + 7.0, mid.y - 9.0),
        Vec2::new(mid.x + 7.0, mid.y + 9.0),
        col,
    );
    c.fill(layout::Rect::new(mid.x - 10.0, mid.y - 5.0, 7.0, 10.0), col);
    if scene.audio_muted {
        c.seg(
            Vec2::new(mid.x - 12.0, mid.y + 11.0),
            Vec2::new(mid.x + 12.0, mid.y - 11.0),
            2.0,
            fade(BAD, 0.9),
        );
    }
}

// -------------------------------------------------------------------- hold --

fn draw_hold(c: &Canvas, scene: &Scene) {
    let cs = c.shifted(grid_shake_offset(scene.juice));
    let plate = inflate(grid_rect(), 10.0);
    cs.fill(plate, PLATE);
    cs.frame(plate, PLATE_EDGE);
    for y in 0..layout::GRID_ROWS {
        for x in 0..layout::GRID_COLS {
            let cell = inflate(layout::cell_rect(x, y), -2.0);
            cs.fill(cell, SOCKET);
            cs.frame(cell, SOCKET_EDGE);
        }
    }
    draw_drop_hints(&cs, scene);
}

/// Per-cell legality tint under a held player piece: green footprint where
/// the drop would land, red where a rule refuses it.
fn draw_drop_hints(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let Some(held) = sim.held() else {
        return;
    };
    if !ours(held.origin) {
        return;
    }
    let Some(piece) = piece_by_id(sim, held.piece) else {
        return;
    };
    let Some((anchor_x, anchor_y)) = layout::cell_at(scene.pointer) else {
        return;
    };
    let legal = placement_check(sim.pieces(), piece.id, piece.kind, anchor_x, anchor_y).is_ok();
    let col = if legal { GOOD } else { BAD };
    let (foot_w, foot_h) = piece.kind.cells();
    for dy in 0..foot_h {
        for dx in 0..foot_w {
            let (cx, cy) = (anchor_x + dx, anchor_y + dy);
            if cx < layout::GRID_COLS && cy < layout::GRID_ROWS {
                c.fill(inflate(layout::cell_rect(cx, cy), -2.0), fade(col, 0.22));
            }
        }
    }
}

// ------------------------------------------------------------------ barter --

fn draw_barter(c: &Canvas, scene: &Scene, barter: &Barter) {
    c.fill(layout::BARTER_PANEL, PLATE);
    c.frame(layout::BARTER_PANEL, PLATE_EDGE);
    draw_slot_rows(c, scene);
    draw_wants(c, scene, barter);
    draw_dial(c, scene, barter);
    draw_accept_lever(c, barter, scene.juice);
}

fn draw_slot_rows(c: &Canvas, scene: &Scene) {
    for (slots, edge) in [
        (&layout::SHELF_SLOTS, SHELF_EDGE),
        (&layout::RECEIVED_SLOTS, RECEIVED_EDGE),
        (&layout::GIVE_SLOTS, GIVE_EDGE),
        (&layout::TAKE_SLOTS, TAKE_EDGE),
    ] {
        for &slot in slots {
            c.fill(slot, SOCKET);
            c.frame(slot, edge);
        }
    }

    // The station shelf is also the abandon target: while the player holds
    // one of their own pieces it glows faintly hungry, brighter when the
    // piece hovers over it.
    if let Some(held) = scene.sim.held() {
        if ours(held.origin) {
            let over = layout::SHELF_AREA.contains(scene.pointer);
            let base = if over { 0.20 } else { 0.06 };
            let alpha = (scene.juice.clock() * 3.0).sin().mul_add(0.04, base);
            c.fill(inflate(layout::SHELF_AREA, 4.0), fade(AMBER, alpha));
        }
    }
}

/// The station's top-three wants: small kind glyphs with 1-3 pips under
/// them, tucked between the shelf and the received row.
fn draw_wants(c: &Canvas, scene: &Scene, barter: &Barter) {
    let t = scene.juice.clock();
    for (i, &(kind, pips)) in barter.wants.iter().enumerate() {
        let x = (i as f32).mul_add(56.0, 303.0);
        piece_glyph(
            c,
            kind,
            0,
            layout::Rect::new(x - 11.0, 500.0, 22.0, 22.0),
            t,
        );
        for p in 0..pips {
            let px = (f32::from(pips) - 1.0)
                .mul_add(-0.5, f32::from(p))
                .mul_add(7.0, x);
            c.dot(Vec2::new(px, 528.0), 1.8, fade(AMBER, 0.85));
        }
    }
}

/// Dial sweep start, degrees (down-left on screen).
const DIAL_START: f32 = 135.0;

/// Dial sweep, degrees (clockwise through up to down-right).
const DIAL_SWEEP: f32 = 270.0;

/// Gauge colour along the sweep: red short of break-even, amber at it,
/// green into generosity.
fn dial_color(value: f32) -> Color {
    if value <= 1.0 {
        mix(BAD, AMBER, value)
    } else {
        mix(AMBER, GOOD, (value - 1.0).clamp(0.0, 1.0))
    }
}

/// The eagerness dial: an arc gauge around the station's own glyph, needle
/// eased by the sim, notch at break-even.
fn draw_dial(c: &Canvas, scene: &Scene, barter: &Barter) {
    let juice = scene.juice;
    let mid = layout::DIAL_CENTER;
    let t = juice.clock();

    // The station glyph sits in the middle and shakes off refused trades.
    let shake = juice.station_shake();
    let wobble = Vec2::new(
        (t * 67.0).sin() * 2.2 * shake,
        (t * 51.0).sin() * 1.6 * shake,
    );
    poi_glyph(c, usize::from(barter.station), mid + wobble, 11.0, t);

    // Track, then the gradient fill up to the eased needle.
    c.arc(mid, 23.0, DIAL_START, DIAL_SWEEP, 5.0, fade(RING, 0.15));
    let value = lerp(barter.prev_eagerness, barter.eagerness, scene.sim.alpha());
    let frac = (value / EAGER_MAX).clamp(0.0, 1.0);
    let segments = 20_u32;
    let seg_sweep = DIAL_SWEEP / segments as f32;
    for i in 0..segments {
        let seg_frac = (i as f32 + 0.5) / segments as f32;
        if seg_frac > frac {
            break;
        }
        c.arc(
            mid,
            23.0,
            (i as f32).mul_add(seg_sweep, DIAL_START),
            seg_sweep + 0.6,
            5.0,
            dial_color(seg_frac * EAGER_MAX),
        );
    }

    // Break-even notch and the needle itself.
    let notch = (DIAL_SWEEP / EAGER_MAX)
        .mul_add(1.0, DIAL_START)
        .to_radians();
    c.seg(
        polar(mid, 20.0, notch),
        polar(mid, 31.0, notch),
        1.4,
        fade(WHITE, 0.8),
    );
    let needle = DIAL_SWEEP.mul_add(frac, DIAL_START).to_radians();
    c.seg(
        polar(mid, 8.0, needle),
        polar(mid, 29.0, needle),
        2.0,
        WHITE,
    );

    // Refused: the gauge flashes red. Accepted: a radial celebration,
    // scaled by how generous the trade was.
    let flash = juice.dial_flash();
    if flash > 0.0 {
        c.arc(
            mid,
            21.0,
            DIAL_START,
            DIAL_SWEEP,
            9.0,
            fade(BAD, flash * 0.5),
        );
    }
    if let Some((heat, value)) = juice.accept_flash() {
        let grow = (1.0 - heat) * 45.0 * value.mul_add(1.2, 1.0);
        c.ring(mid, 12.0 + grow, 2.5, fade(GOOD, heat * 0.9));
        c.ring(mid, (12.0 + grow) * 0.65, 1.5, fade(AMBER, heat * 0.7));
    }
}

/// The accept lever: glowing green the instant the station would say yes.
fn draw_accept_lever(c: &Canvas, barter: &Barter, juice: &Juice) {
    let rect = layout::ACCEPT_LEVER;
    c.fill(rect, PLATE_DEEP);
    c.frame(rect, PLATE_EDGE);
    let mid_y = rect.h.mul_add(0.5, rect.y);
    c.seg(
        Vec2::new(rect.x + 12.0, mid_y),
        Vec2::new(rect.x + rect.w - 12.0, mid_y),
        3.0,
        SOCKET,
    );
    let handle = layout::Rect::new(rect.x + 20.0, rect.y + 6.0, 14.0, rect.h - 12.0);
    if barter.ready {
        let glow = (juice.clock() * 2.8).sin().mul_add(0.1, 0.3);
        c.fill(inflate(handle, 5.0), fade(GOOD, glow));
        c.fill(handle, GOOD);
    } else {
        c.fill(handle, HANDLE_DIM);
    }
    c.frame(handle, PLATE_EDGE);
}

// ------------------------------------------------------------------ pieces --

fn draw_pieces(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let juice = scene.juice;
    let t = juice.clock();
    let shaken = c.shifted(grid_shake_offset(juice));
    let held_id = sim.held().map(|held| held.piece);
    for piece in sim.pieces() {
        if held_id == Some(piece.id) {
            continue;
        }
        let mut rect = layout::piece_rect(piece);
        // Freshly received goods arc from the take pad to their shelf slot.
        if let (Some(slide), Loc::ReceivedShelf { slot }) =
            (juice.received_slide(piece.id), piece.loc)
        {
            rect = rect_lerp(layout::TAKE_SLOTS[usize::from(slot)], rect, ease_out(slide));
        }
        // A just-placed piece lands large and snap-settles down.
        if let Some(settle) = juice.settling(piece.id) {
            rect = scaled(rect, (settle * settle).mul_add(0.1, 1.0));
        }
        let on_grid = matches!(piece.loc, Loc::Hold { .. });
        let canvas = if on_grid { &shaken } else { c };
        piece_glyph(canvas, piece.kind, piece.variant, rect, t);
    }
}

/// The held piece: enlarged, shadowed, glued to the pointer, tinted by
/// whether letting go would work.
fn draw_held(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let Some(held) = sim.held() else {
        return;
    };
    let Some(piece) = piece_by_id(sim, held.piece) else {
        return;
    };
    let (w, h) = piece.kind.cells();
    let scale = scene.juice.pickup_pop().mul_add(-0.1, 1.1);
    let gw = f32::from(w) * layout::CELL * scale;
    let gh = f32::from(h) * layout::CELL * scale;
    let rect = layout::Rect::new(
        gw.mul_add(-0.5, scene.pointer.x),
        gh.mul_add(-0.5, scene.pointer.y),
        gw,
        gh,
    );
    c.fill(shifted_rect(rect, 3.0, 4.0), fade(INK, 0.35));
    piece_glyph(c, piece.kind, piece.variant, rect, scene.juice.clock());
    let col = if held.legal { GOOD } else { BAD };
    c.fill(rect, fade(col, 0.10));
    c.frame_thick(rect, 1.5, fade(col, 0.7));
}

// ----------------------------------------------------------- cargo glyphs --

/// One silhouette per cargo kind, fitted into `rect` (a hold footprint or a
/// slot). Variants shift the hue and scale the inner detail — same kind,
/// sibling crates.
fn piece_glyph(c: &Canvas, kind: Kind, variant: u8, rect: layout::Rect, t: f32) {
    let b = glyph_box(kind, rect);
    let col = variant_tint(kind_color(kind), variant);
    let vs = variant_scale(variant);
    match kind {
        Kind::PerfumeVial => vial_glyph(c, b, col, vs),
        Kind::GildedIdol => idol_glyph(c, b, col, vs),
        Kind::RationBricks => rations_glyph(c, b, col, vs),
        Kind::ScrapAlloy => scrap_glyph(c, b, col, vs),
        Kind::Seedlings => seedlings_glyph(c, b, col, vs),
        Kind::GasCanister => canister_glyph(c, b, col, vs),
        Kind::CryoCore => cryo_glyph(c, b, col, vs),
        Kind::BrinePearls => pearls_glyph(c, b, col, vs),
        Kind::SuspiciousCrate => crate_glyph(c, b, col, vs, t),
    }
}

/// The kind's footprint aspect fitted inside `rect` with padding, so a wide
/// piece stays wide even when drawn in a square slot.
fn glyph_box(kind: Kind, rect: layout::Rect) -> layout::Rect {
    let (cw, ch) = kind.cells();
    let aspect = f32::from(cw) / f32::from(ch);
    let pad = 0.84;
    let (bw, bh) = if rect.w / rect.h > aspect {
        (rect.h * aspect * pad, rect.h * pad)
    } else {
        (rect.w * pad, rect.w / aspect * pad)
    };
    layout::Rect::new(
        (rect.w - bw).mul_add(0.5, rect.x),
        (rect.h - bh).mul_add(0.5, rect.y),
        bw,
        bh,
    )
}

/// Perfume vial: a pink rhombus with a sparkle.
fn vial_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let mid = rect_center(b);
    c.poly(mid, 4, b.w * 0.42, 0.0, col);
    c.poly(mid, 4, b.w * 0.20, 0.0, dim(col, 0.75));
    let sparkle = Vec2::new(b.w.mul_add(0.82, b.x), b.h.mul_add(0.18, b.y));
    let arm = b.w * 0.13 * vs;
    c.seg(
        sparkle - Vec2::new(arm, 0.0),
        sparkle + Vec2::new(arm, 0.0),
        1.0,
        WHITE,
    );
    c.seg(
        sparkle - Vec2::new(0.0, arm),
        sparkle + Vec2::new(0.0, arm),
        1.0,
        WHITE,
    );
}

/// Gilded idol: a gold slab with a circle head. Unimaginably tacky.
fn idol_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let cx = b.w.mul_add(0.5, b.x);
    c.fill(
        layout::Rect::new(
            b.w.mul_add(-0.28, cx),
            b.h.mul_add(0.34, b.y),
            b.w * 0.56,
            b.h * 0.58,
        ),
        col,
    );
    c.fill(
        layout::Rect::new(
            b.w.mul_add(-0.28, cx),
            b.h.mul_add(0.56, b.y),
            b.w * 0.56,
            b.h * 0.07,
        ),
        dim(col, 0.6),
    );
    c.dot(Vec2::new(cx, b.h.mul_add(0.18, b.y)), b.w * 0.24 * vs, col);
}

/// Ration bricks: an olive 2x2 sub-grid of identical government flavour.
fn rations_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let brick = b.w * 0.42 * vs;
    let gap = b.w * 0.07;
    let total = 2.0f32.mul_add(brick, gap);
    let x0 = (b.w - total).mul_add(0.5, b.x);
    let y0 = (b.h - total).mul_add(0.5, b.y);
    for iy in 0..2_u8 {
        for ix in 0..2_u8 {
            let cell = layout::Rect::new(
                f32::from(ix).mul_add(brick + gap, x0),
                f32::from(iy).mul_add(brick + gap, y0),
                brick,
                brick,
            );
            c.fill(cell, col);
            c.frame(cell, dim(col, 0.55));
        }
    }
}

/// Scrap alloy: rust bars with a jagged bite missing.
fn scrap_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let bar_h = b.h * 0.30;
    let top = layout::Rect::new(
        b.w.mul_add(0.04, b.x),
        b.h.mul_add(0.12, b.y),
        b.w * 0.92,
        bar_h,
    );
    let bottom = layout::Rect::new(b.x, b.h.mul_add(0.56, b.y), b.w * 0.88, bar_h);
    c.fill(top, col);
    c.fill(bottom, dim(col, 0.8));
    // The bite: a socket-coloured triangle notched out of the top bar.
    let bite = bar_h * 0.95 * vs;
    let x1 = top.x + top.w;
    c.tri(
        Vec2::new(x1, top.y),
        Vec2::new(x1, top.y + bar_h),
        Vec2::new(x1 - bite, bar_h.mul_add(0.5, top.y)),
        SOCKET,
    );
}

/// Seedlings: a pot-round base with a sprout on top.
fn seedlings_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let cx = b.w.mul_add(0.5, b.x);
    c.dot(
        Vec2::new(cx, b.h.mul_add(0.64, b.y)),
        b.w * 0.29,
        dim(col, 0.75),
    );
    let spread = b.w * 0.19 * vs;
    c.tri(
        Vec2::new(cx, b.h.mul_add(0.06, b.y)),
        Vec2::new(cx - spread, b.h.mul_add(0.46, b.y)),
        Vec2::new(cx + spread, b.h.mul_add(0.46, b.y)),
        col,
    );
}

/// Gas canister: an orange capsule wearing hazard chevrons.
fn canister_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let mid = rect_center(b);
    let body = layout::Rect::new(
        b.w.mul_add(0.10, b.x),
        b.h.mul_add(0.20, b.y),
        b.w * 0.80,
        b.h * 0.60,
    );
    let cap = b.h * 0.30;
    c.dot(Vec2::new(body.x, mid.y), cap, col);
    c.dot(Vec2::new(body.x + body.w, mid.y), cap, col);
    c.fill(body, col);
    let s = b.h * 0.20 * vs;
    for off in [-0.6, 0.8] {
        let x = s.mul_add(off, mid.x);
        c.tri(
            Vec2::new(x - s, mid.y - s),
            Vec2::new(x - s, mid.y + s),
            Vec2::new(x, mid.y),
            dim(col, 0.4),
        );
    }
}

/// Cryo core: a cyan hexagon in a frost outline.
fn cryo_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let mid = rect_center(b);
    c.poly(mid, 6, b.w * 0.34 * vs, 90.0, col);
    c.poly(mid, 6, b.w * 0.16 * vs, 90.0, dim(col, 0.7));
    c.poly_ring(mid, 6, b.w * 0.46, 90.0, 1.0, fade(WHITE, 0.55));
}

/// Brine pearls: a stack of blue spheres, each with a wet glint.
fn pearls_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let cx = b.w.mul_add(0.5, b.x);
    let r = b.w * 0.27;
    for (i, fy) in [0.2, 0.5, 0.8].into_iter().enumerate() {
        let center = Vec2::new(cx, b.h.mul_add(fy, b.y));
        c.dot(center, r, if i == 1 { dim(col, 0.88) } else { col });
        c.dot(
            center + Vec2::new(-r * 0.3, -r * 0.3),
            r * 0.24 * vs,
            fade(WHITE, 0.75),
        );
    }
}

/// The suspicious crate: matte black, faintly humming violet at ~1 Hz —
/// the same beat the audio hum keeps.
fn crate_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32, t: f32) {
    let body = scaled(b, 0.86);
    let pulse = (TAU * t).sin().mul_add(0.5, 0.5);
    c.fill(inflate(body, 3.0), fade(VIOLET, pulse.mul_add(0.22, 0.08)));
    c.fill(body, col);
    c.frame(body, rgb(0.20, 0.18, 0.26));
    c.frame_thick(
        inflate(body, 1.5),
        1.0,
        fade(VIOLET, pulse.mul_add(0.35, 0.15)),
    );
    c.dot(
        rect_center(b),
        b.w * 0.05 * vs,
        fade(VIOLET_FLASH, pulse.mul_add(0.5, 0.3)),
    );
}

// ---------------------------------------------------------------- flashes --

/// The rule glyph that flashes over the offending cells on a hard reject.
fn draw_violation_flash(c: &Canvas, scene: &Scene) {
    let Some((rule, rect, heat)) = scene.juice.violation() else {
        return;
    };
    let mid = rect_center(rect);
    match rule {
        // Off the grid or onto another piece: a plain red edge flash.
        Violation::Bounds | Violation::Overlap => {
            c.frame_thick(grid_rect(), 2.0, fade(BAD, heat));
            c.frame_thick(rect, 2.0, fade(BAD, heat * 0.9));
        }
        Violation::Heavy => {
            c.fill(rect, fade(BAD, heat * 0.18));
            weight_glyph(c, mid, 12.0, fade(WHITE, heat));
        }
        Violation::Volatile => {
            c.fill(rect, fade(BAD, heat * 0.18));
            hazard_glyph(c, mid, 12.0, fade(AMBER_EDGE, heat));
        }
        Violation::Cryo => {
            c.fill(rect, fade(BAD, heat * 0.18));
            snowflake_glyph(c, mid, 12.0, fade(URANUS, heat));
        }
        // A second crate aboard: the hold itself objects in violet.
        Violation::Suspicious => {
            c.fill(rect, fade(VIOLET, heat * 0.45));
            c.frame_thick(rect, 2.0, fade(VIOLET_FLASH, heat));
        }
    }
}

/// A kettlebell-ish weight: trapezoid body, loop handle.
fn weight_glyph(c: &Canvas, mid: Vec2, s: f32, col: Color) {
    let bl = Vec2::new(mid.x - s, s.mul_add(0.65, mid.y));
    let br = Vec2::new(mid.x + s, s.mul_add(0.65, mid.y));
    let tl = Vec2::new(s.mul_add(-0.55, mid.x), s.mul_add(-0.3, mid.y));
    let tr = Vec2::new(s.mul_add(0.55, mid.x), s.mul_add(-0.3, mid.y));
    c.tri(bl, br, tr, col);
    c.tri(bl, tr, tl, col);
    c.frame_thick(
        layout::Rect::new(
            s.mul_add(-0.35, mid.x),
            s.mul_add(-0.75, mid.y),
            s * 0.7,
            s * 0.45,
        ),
        1.5,
        col,
    );
}

/// A hazard triangle with an inner chevron.
fn hazard_glyph(c: &Canvas, mid: Vec2, s: f32, col: Color) {
    c.tri_ring(
        Vec2::new(mid.x, mid.y - s),
        Vec2::new(s.mul_add(-0.9, mid.x), s.mul_add(0.7, mid.y)),
        Vec2::new(s.mul_add(0.9, mid.x), s.mul_add(0.7, mid.y)),
        2.0,
        col,
    );
    let nose = Vec2::new(mid.x, s.mul_add(-0.15, mid.y));
    c.seg(
        Vec2::new(s.mul_add(-0.35, mid.x), s.mul_add(0.35, mid.y)),
        nose,
        2.0,
        col,
    );
    c.seg(
        nose,
        Vec2::new(s.mul_add(0.35, mid.x), s.mul_add(0.35, mid.y)),
        2.0,
        col,
    );
}

/// A six-armed snowflake.
fn snowflake_glyph(c: &Canvas, mid: Vec2, s: f32, col: Color) {
    for i in 0..3_u8 {
        let angle = f32::from(i) * (PI / 3.0);
        c.seg(polar(mid, s, angle), polar(mid, s, angle + PI), 1.8, col);
    }
    c.ring(mid, s * 0.45, 1.2, col);
}

/// Screen-space effects, drawn raw (outside the palette tint): the omen's
/// violet vignette and the jump's flash.
fn draw_screen_fx(scene: &Scene) {
    let sw = screen_width();
    let sh = screen_height();
    let omen = scene.sim.omen();
    if omen > 0.0 {
        draw_rectangle(0.0, 0.0, sw, sh, fade(VIOLET, omen * 0.14));
        let band = sh * 0.12;
        draw_rectangle(0.0, 0.0, sw, band, fade(INK, omen * 0.35));
        draw_rectangle(0.0, sh - band, sw, band, fade(INK, omen * 0.35));
    }
    let jump = scene.juice.jump_flash();
    if jump > 0.0 {
        draw_rectangle(0.0, 0.0, sw, sh, fade(VIOLET_FLASH, jump * jump * 0.55));
    }
}
