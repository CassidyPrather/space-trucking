//! The ship console: every pixel of the game, drawn from sim accessors.
//!
//! No text, no numbers, no labels — the one string the game ever renders
//! lives in `main.rs`. Everything here communicates by shape, colour, and
//! motion, and every colour is a role from [`crate::palette`] (a purity test
//! there enforces it). All geometry comes from [`layout`], the same
//! constants the sim hit-tests against, so things are always drawn exactly
//! where they can be clicked.
//!
//! The look follows `docs/ART_DIRECTION.md`: the screen is the instrument
//! panel of an old freighter. Two material families — worn metal (plates,
//! sockets, levers, lamps) and phosphor CRTs (the star map and destination
//! preview, the only screens) — and one light source at the top-left.
//!
//! The whole fiction renders into a low-res target ([`crate::CRUNCH`] world
//! units per pixel) and upscales nearest-neighbour into the letterbox, so
//! the pixels are honest. The [`Canvas`] wrapper applies the console light
//! level ([`Sim::light`]) plus the omen's violet cast to every draw call,
//! so the whole palette dims as one when the event wants it dark.

use std::f32::consts::{PI, TAU};

use macroquad::camera::{Camera2D, set_camera, set_default_camera};
use macroquad::color::Color;
use macroquad::math::{Rect as ScreenRect, vec2};
use macroquad::shapes::{
    draw_arc, draw_circle, draw_circle_lines, draw_ellipse, draw_ellipse_lines, draw_line,
    draw_poly, draw_poly_lines, draw_rectangle, draw_rectangle_lines, draw_triangle,
    draw_triangle_lines,
};
use macroquad::texture::{DrawTextureParams, RenderTarget, draw_texture_ex};
use macroquad::window::clear_background;

use space_trucking::sim::{
    Barter, EAGER_MAX, EncounterKind, GUILD, Kind, Loc, POIS, Piece, PoiId, SUN, ShipState, Sim,
    Track, Vec2, Violation, WORLD_H, WORLD_W, layout, leg_endpoints, placement_check, player_owned,
    splitmix,
};

use crate::View;
use crate::juice::{DELIVERY_LAMPS, Juice};
use crate::palette::{
    AMBER, BLIT, BRASS, EERIE, EERIE_BRIGHT, GLASS, GLINT, HULL, ICON, ICON_LIT, LAMP_NO, LAMP_OK,
    PHOSPHOR, PHOSPHOR_DIM, PHOSPHOR_HOT, PLATE, PLATE_LIT, PLATE_SHADE, POI_COMET, POI_EARTH,
    POI_GUILD, POI_GUILD_EDGE, POI_HERMITAGE, POI_JUPITER, POI_MARS, POI_MARS_PATCH, POI_NEPTUNE,
    POI_SATURN, POI_SATURN_RING, POI_SMOG, POI_UMBRA, POI_URANUS, POI_URANUS_RING, POI_VENUS,
    POI_VENUS_HALO, POI_WANDERER, RIVET, SCREEN, SHADOW, SOCKET, TRIM_GIVE, TRIM_RECEIVED,
    TRIM_SHELF, TRIM_TAKE, VOID, dim, fade, kind_color, lerp, mix, omen_tint, phosphorize,
    variant_tint,
};
use crate::tutor::{Echo, Ghost};

/// Everything one frame of drawing needs, gathered by `main`.
// Independent presentation flags, not a state machine in disguise — the
// same plea `InputFrame` makes.
#[allow(clippy::struct_excessive_bools)]
pub struct Scene<'a> {
    pub sim: &'a Sim,
    pub juice: &'a Juice,
    /// Pointer position in world coordinates, for ghosts and hover glows.
    pub pointer: Vec2,
    /// The onboarding tutor's ghost hand mid-demonstration, if any.
    pub ghost: Option<Ghost>,
    /// Audio still waits on the browser's first-gesture rule.
    pub audio_waiting: bool,
    /// The player has muted.
    pub audio_muted: bool,
    /// The OS asked for reduced motion (the web shell mirrors the media
    /// query into storage; absent natively). Decoration freezes to its calm
    /// static pose; feedback and the instructional ghost still run. See
    /// `docs/ART_DIRECTION.md`, Motion.
    pub reduced_motion: bool,
    /// Developer mode: the only state in which the warp button exists.
    pub dev: bool,
}

impl Scene<'_> {
    /// The decoration clock: the free-running cosmetic clock normally,
    /// frozen at zero under reduced motion — every idle loop here is a sine
    /// of `t` times a frequency (plus a per-element phase for variety), so
    /// zero parks each one exactly at its calm midpoint. Feedback tweens
    /// read [`Juice`]'s clocks directly and never come through here.
    const fn idle_clock(&self) -> f32 {
        if self.reduced_motion {
            0.0
        } else {
            self.juice.clock()
        }
    }
}

// ------------------------------------------------------------- crunch grid --

/// World units per crunched target pixel. Anything thinner than this
/// vanishes at the low resolution, so strokes, bevels, and dots are sized
/// in multiples of it.
const PX: f32 = crate::CRUNCH;

/// `v` snapped down to the target pixel grid, for things (stars, scanlines)
/// that must land on whole pixels or shimmer.
fn snap(v: f32) -> f32 {
    (v / PX).floor() * PX
}

// ------------------------------------------------------------- small math --

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

/// `p` clamped inside `r`, for wear marks that must not leave their plate.
fn clamp_into(p: Vec2, r: layout::Rect) -> Vec2 {
    Vec2::new(p.x.clamp(r.x, r.x + r.w), p.y.clamp(r.y, r.y + r.h))
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

/// World-space draw helper: applies a cosmetic offset (shakes), the console
/// light level, and the omen cast to every call. The camera set up in
/// [`draw`] maps world coordinates onto the crunch target, so no scaling
/// happens here.
#[derive(Clone, Copy)]
struct Canvas {
    light: f32,
    omen: f32,
    offset: Vec2,
}

impl Canvas {
    fn shifted(&self, by: Vec2) -> Self {
        Self {
            offset: self.offset + by,
            ..*self
        }
    }

    fn point(&self, p: Vec2) -> Vec2 {
        p + self.offset
    }

    /// Line width quantized to whole target pixels, never below one, so
    /// strokes survive the crunch instead of shimmering at half-coverage.
    fn stroke(w: f32) -> f32 {
        (w / PX).round().max(1.0) * PX
    }

    /// The whole palette runs through here: light level times omen cast.
    fn tint(&self, col: Color) -> Color {
        omen_tint(col, self.light, self.omen)
    }

    fn fill(&self, r: layout::Rect, col: Color) {
        let p = self.point(Vec2::new(r.x, r.y));
        draw_rectangle(p.x, p.y, r.w, r.h, self.tint(col));
    }

    fn frame(&self, r: layout::Rect, col: Color) {
        self.frame_thick(r, 1.0, col);
    }

    fn frame_thick(&self, r: layout::Rect, w: f32, col: Color) {
        let p = self.point(Vec2::new(r.x, r.y));
        draw_rectangle_lines(p.x, p.y, r.w, r.h, Self::stroke(w), self.tint(col));
    }

    fn dot(&self, at: Vec2, r: f32, col: Color) {
        let p = self.point(at);
        // Sub-pixel dots vanish at the crunch resolution; keep a floor.
        draw_circle(p.x, p.y, r.max(PX * 0.5), self.tint(col));
    }

    fn ring(&self, at: Vec2, r: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_circle_lines(p.x, p.y, r, Self::stroke(w), self.tint(col));
    }

    fn seg(&self, a: Vec2, b: Vec2, w: f32, col: Color) {
        let pa = self.point(a);
        let pb = self.point(b);
        draw_line(pa.x, pa.y, pb.x, pb.y, Self::stroke(w), self.tint(col));
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
            Self::stroke(w),
            self.tint(col),
        );
    }

    fn poly(&self, at: Vec2, sides: u8, r: f32, rot: f32, col: Color) {
        let p = self.point(at);
        draw_poly(p.x, p.y, sides, r, rot, self.tint(col));
    }

    fn poly_ring(&self, at: Vec2, sides: u8, r: f32, rot: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_poly_lines(p.x, p.y, sides, r, rot, Self::stroke(w), self.tint(col));
    }

    /// Arc from `rot` degrees, sweeping `sweep` degrees clockwise, drawn
    /// outward from `r`.
    fn arc(&self, at: Vec2, r: f32, rot: f32, sweep: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_arc(p.x, p.y, 48, r, rot, Self::stroke(w), sweep, self.tint(col));
    }

    fn oval(&self, at: Vec2, rx: f32, ry: f32, rot: f32, col: Color) {
        let p = self.point(at);
        draw_ellipse(p.x, p.y, rx, ry, rot, self.tint(col));
    }

    fn oval_ring(&self, at: Vec2, rx: f32, ry: f32, rot: f32, w: f32, col: Color) {
        let p = self.point(at);
        draw_ellipse_lines(p.x, p.y, rx, ry, rot, Self::stroke(w), self.tint(col));
    }
}

// ------------------------------------------------------------------- entry --

/// Draw the whole console for one frame: the fiction into the crunch
/// target, then one nearest-neighbour blit into the letterbox.
pub fn draw(view: &View, target: &RenderTarget, scene: &Scene) {
    let canvas = Canvas {
        light: scene.sim.light(),
        omen: scene.sim.omen(),
        offset: Vec2::default(),
    };

    // The camera maps the full logical world onto the low-res target.
    // `from_display_rect` keeps world y pointing down, which lands upside
    // down in the target's texture; the blit below flips it back.
    let camera = Camera2D {
        render_target: Some(target.clone()),
        ..Camera2D::from_display_rect(ScreenRect::new(0.0, 0.0, WORLD_W, WORLD_H))
    };
    set_camera(&camera);

    clear_background(canvas.tint(HULL));
    draw_map(&canvas, scene);
    draw_console(&canvas, scene);
    draw_hold(&canvas, scene);
    draw_barter(&canvas, scene);
    draw_pieces(&canvas, scene);
    draw_rat(&canvas, scene);
    draw_violation_flash(&canvas, scene);
    draw_held(&canvas, scene);
    draw_tutor_ghost(&canvas, scene);
    draw_omen_fx(scene);

    // Blit the crunched frame into the letterbox, hard pixels intact. The
    // destination is exactly the View's rect, so what you see is where you
    // click. The version string draws after this, outside the fiction.
    set_default_camera();
    clear_background(VOID);
    let corner = view.to_screen(Vec2::default());
    draw_texture_ex(
        &target.texture,
        corner.x,
        corner.y,
        BLIT,
        DrawTextureParams {
            dest_size: Some(vec2(WORLD_W * view.scale(), WORLD_H * view.scale())),
            flip_y: true,
            ..Default::default()
        },
    );
}

// -------------------------------------------------------------- worn metal --

/// Fixed render-side seed for wear marks; cosmetic only, never the sim's.
const WEAR_SEED: u64 = 0x5C2A_7C4E;

/// One wear mark per this much plate area, world units². The single
/// density knob the art doc asks for.
const WEAR_PER_MARK: f32 = 4500.0;

/// Wear alpha ceiling — above this it reads as dirt on the player's own
/// monitor, says the art doc.
const WEAR_ALPHA: f32 = 0.12;

// One wear stream per plate, so plates never share scratches.
const SALT_MAP: u64 = 1;
const SALT_PREVIEW: u64 = 2;
const SALT_CONSOLE: u64 = 3;
const SALT_HOLD: u64 = 4;
const SALT_BARTER: u64 = 5;
const SALT_LAUNCH: u64 = 6;
const SALT_ACCEPT: u64 = 7;
const SALT_HANGAR: u64 = 8;

/// One-pixel bevel edges: `top_left` along the edges facing the light,
/// `bottom_right` along the ones facing away. Light is always top-left.
fn bevel(c: &Canvas, r: layout::Rect, top_left: Color, bottom_right: Color) {
    c.fill(layout::Rect::new(r.x, r.y, r.w, PX), top_left);
    c.fill(layout::Rect::new(r.x, r.y, PX, r.h), top_left);
    c.fill(
        layout::Rect::new(r.x, r.y + r.h - PX, r.w, PX),
        bottom_right,
    );
    c.fill(
        layout::Rect::new(r.x + r.w - PX, r.y, PX, r.h),
        bottom_right,
    );
}

/// A riveted plate face: raised bevel, deterministic wear, corner rivets.
fn plate(c: &Canvas, r: layout::Rect, wear_salt: u64) {
    c.fill(r, PLATE);
    bevel(c, r, PLATE_LIT, PLATE_SHADE);
    wear(c, r, wear_salt);
    rivets(c, r, 4.0 * PX);
}

fn rivets(c: &Canvas, r: layout::Rect, inset: f32) {
    for (x, y) in [
        (r.x + inset, r.y + inset),
        (r.x + r.w - inset, r.y + inset),
        (r.x + inset, r.y + r.h - inset),
        (r.x + r.w - inset, r.y + r.h - inset),
    ] {
        rivet(c, Vec2::new(snap(x), snap(y)));
    }
}

/// A pixel rivet: head, a glint toward the light, a cast shadow away.
fn rivet(c: &Canvas, at: Vec2) {
    c.fill(
        layout::Rect::new(at.x - PX, at.y - PX, 2.0 * PX, 2.0 * PX),
        RIVET,
    );
    c.fill(
        layout::Rect::new(at.x - PX, at.y - PX, PX, PX),
        fade(GLINT, 0.25),
    );
    c.fill(layout::Rect::new(at.x + PX, at.y, PX, PX), PLATE_SHADE);
    c.fill(layout::Rect::new(at.x, at.y + PX, PX, PX), PLATE_SHADE);
}

/// An inset well: light falls away at the top/left lip and catches the
/// bottom/right one — the reverse of a raised plate.
fn socket_well(c: &Canvas, r: layout::Rect) {
    c.fill(r, SOCKET);
    bevel(c, r, PLATE_SHADE, fade(PLATE_LIT, 0.6));
}

/// Deterministic wear: scratches, scuffs, and smudges hashed from
/// [`WEAR_SEED`], so every ship is scuffed the same way every boot.
fn wear(c: &Canvas, r: layout::Rect, salt: u64) {
    let count = ((r.w * r.h / WEAR_PER_MARK) as i64)
        .clamp(2, 24)
        .unsigned_abs();
    let inset = 2.0 * PX;
    for i in 0..count {
        let h = splitmix(WEAR_SEED ^ salt, i);
        let fx = (h & 0xFFFF) as f32 / 65_535.0;
        let fy = ((h >> 16) & 0xFFFF) as f32 / 65_535.0;
        let at = Vec2::new(
            fx.mul_add(2.0f32.mul_add(-inset, r.w), r.x + inset),
            fy.mul_add(2.0f32.mul_add(-inset, r.h), r.y + inset),
        );
        match (h >> 32) % 5 {
            // Scratches: short strokes, light or dark by coin flip.
            0..=2 => {
                let len = (((h >> 40) & 0xF) as f32).mul_add(1.0, 6.0);
                let angle = ((h >> 44) & 0xF) as f32 / 16.0 * TAU;
                let col = if h & (1 << 60) == 0 {
                    fade(GLINT, WEAR_ALPHA * 0.8)
                } else {
                    fade(SHADOW, WEAR_ALPHA)
                };
                c.seg(at, clamp_into(polar(at, len, angle), r), 1.0, col);
            }
            // Scuffs: a couple of chipped pixels.
            3 => c.fill(
                layout::Rect::new(at.x, at.y, 2.0 * PX, PX),
                fade(SHADOW, WEAR_ALPHA),
            ),
            // Smudges: someone's decade-old glove print.
            _ => {
                let side = (((h >> 48) & 0xF) as f32).mul_add(1.0, 10.0);
                c.fill(
                    layout::Rect::new(at.x - side * 0.5, at.y - side * 0.5, side, side),
                    fade(SHADOW, 0.05),
                );
            }
        }
    }
}

/// A lamp behind glass: bright core with a soft halo when lit, dark glass
/// when off. `level` in `0..=1` eases the whole treatment for pulses.
fn lamp(c: &Canvas, at: Vec2, r: f32, col: Color, level: f32) {
    c.dot(at, r, GLASS);
    if level > 0.01 {
        c.dot(at, r * 2.2, fade(col, 0.18 * level));
        c.dot(at, r * 0.95, fade(col, level));
        c.dot(at, r * 0.4, fade(GLINT, 0.75 * level));
    } else {
        c.dot(
            at + Vec2::new(-r * 0.35, -r * 0.35),
            r * 0.3,
            fade(GLINT, 0.10),
        );
    }
}

/// A brass lever handle: beveled block with grip grooves.
fn brass_handle(c: &Canvas, r: layout::Rect) {
    c.fill(r, BRASS);
    bevel(c, r, fade(GLINT, 0.35), fade(SHADOW, 0.45));
    for fy in [0.50, 0.68, 0.86] {
        c.fill(
            layout::Rect::new(r.x + PX, r.h.mul_add(fy, r.y), 2.0f32.mul_add(-PX, r.w), PX),
            fade(SHADOW, 0.30),
        );
    }
}

// ------------------------------------------------------- phosphor screens --

/// How far the map's CRT leans POI colours toward phosphor (identity by
/// silhouette); the preview is the "big screen" and keeps a richer tint.
const MAP_PH: f32 = 0.85;
const PREVIEW_PH: f32 = 0.35;

/// Seconds per sonar-sweep revolution. Ambient, not attention-seeking.
const SWEEP_PERIOD: f32 = 20.0;

/// A CRT set into the panel: raised bezel ring, then the inset glass.
/// Returns the glass rect the caller draws its picture inside.
fn crt_screen(c: &Canvas, r: layout::Rect, wear_salt: u64) -> layout::Rect {
    c.fill(r, PLATE);
    bevel(c, r, PLATE_LIT, PLATE_SHADE);
    // Wear lands across the whole rect; the glass fill below keeps only
    // the bezel ring's share, which is exactly right.
    wear(c, r, wear_salt);
    let glass = inflate(r, -4.0 * PX);
    c.fill(glass, SCREEN);
    bevel(c, glass, PLATE_SHADE, fade(PLATE_LIT, 0.5));
    rivets(c, r, 2.0 * PX);
    glass
}

/// Scanlines, vignette, and the pre-jump flicker — drawn last inside a
/// screen so the picture shows through them.
fn finish_screen(c: &Canvas, glass: layout::Rect, scene: &Scene) {
    scanlines(c, glass);
    screen_vignette(c, glass);
    let omen = scene.sim.omen();
    if omen > 0.0 {
        // The flicker loops for as long as the omen holds, so it gates as
        // decoration; frozen it is a steady half-strength dimming, and the
        // omen still reads through the whole-panel light drop and cast.
        let flicker = (scene.idle_clock() * 9.0).sin().mul_add(0.5, 0.5);
        c.fill(glass, fade(SHADOW, omen * 0.12 * flicker));
    }
}

/// Every other target-pixel row, very low alpha.
fn scanlines(c: &Canvas, r: layout::Rect) {
    let step = 2.0 * PX;
    let y0 = (r.y / step).ceil() * step;
    let rows = ((r.y + r.h - y0 - PX) / step) as i32;
    for i in 0..=rows.max(0) {
        let y = (i as f32).mul_add(step, y0);
        c.fill(
            layout::Rect::new(r.x, y, r.w, PX),
            fade(dim(PHOSPHOR_DIM, 0.3), 0.4),
        );
    }
}

/// The corners darken: the map is a tube, not a viewport.
fn screen_vignette(c: &Canvas, r: layout::Rect) {
    let s = r.w.min(r.h) * 0.18;
    let corners = [
        (Vec2::new(r.x, r.y), 1.0, 1.0),
        (Vec2::new(r.x + r.w, r.y), -1.0, 1.0),
        (Vec2::new(r.x, r.y + r.h), 1.0, -1.0),
        (Vec2::new(r.x + r.w, r.y + r.h), -1.0, -1.0),
    ];
    for (corner, sx, sy) in corners {
        for reach in [s, s * 0.55] {
            c.tri(
                corner,
                corner + Vec2::new(sx * reach, 0.0),
                corner + Vec2::new(0.0, sy * reach),
                fade(SHADOW, 0.12),
            );
        }
    }
    c.frame_thick(r, 1.0, fade(SHADOW, 0.30));
}

/// Sweep bearing at cosmetic time `t`, radians.
fn sweep_angle(t: f32) -> f32 {
    (t * (TAU / SWEEP_PERIOD)).rem_euclid(TAU)
}

/// How lit `pos` still is by the sweep that last passed it, `0..=1` —
/// a slow phosphor afterglow trailing the beam by up to ~5 seconds.
fn sweep_glow(center: Vec2, angle: f32, pos: Vec2) -> f32 {
    let d = pos - center;
    let behind = (angle - d.y.atan2(d.x)).rem_euclid(TAU);
    (behind / 1.7).mul_add(-1.0, 1.0).max(0.0)
}

/// One slow sonar line about the map centre with a short fading trail.
fn draw_sweep(c: &Canvas, glass: layout::Rect, t: f32) {
    let center = rect_center(glass);
    let radius = glass.w.min(glass.h).mul_add(0.5, -2.0 * PX);
    let angle = sweep_angle(t);
    for k in 0..5_u8 {
        let a0 = f32::from(k).mul_add(-0.04, angle);
        let alpha = f32::from(k).mul_add(-0.012, 0.065);
        c.tri(
            center,
            polar(center, radius, a0),
            polar(center, radius, a0 - 0.04),
            fade(PHOSPHOR, alpha),
        );
    }
    c.seg(
        center,
        polar(center, radius, angle),
        1.0,
        fade(PHOSPHOR, 0.28),
    );
    c.dot(center, PX, fade(PHOSPHOR, 0.5));
}

// --------------------------------------------------------------------- map --

fn draw_map(c: &Canvas, scene: &Scene) {
    // Star twinkle and the sweep are decoration: under reduced motion the
    // idle clock holds at zero, so the stars sit steady and the sweep parks
    // at its zero bearing — still visibly a radar, just not a moving one.
    let t = scene.idle_clock();
    let glass = crt_screen(c, layout::MAP_PANEL, SALT_MAP);
    draw_starfield(c, glass, t);
    draw_orbits(c, scene, t);
    draw_route_and_ship(c, scene);
    draw_pois(c, scene, glass);
    draw_parade(c, scene, glass);
    draw_travel_company(c, scene, glass);
    draw_sweep(c, glass, t);
    finish_screen(c, glass, scene);
    draw_ad_static(c, scene, glass);
}

/// Fixed render-side seed for the starfield hash; cosmetic only.
const STAR_SEED: u64 = 0x57A2_F1E1;
const STAR_COUNT: u64 = 110;

fn draw_starfield(c: &Canvas, glass: layout::Rect, t: f32) {
    for i in 0..STAR_COUNT {
        let hash = splitmix(STAR_SEED, i);
        let fx = (hash & 0xFFFF) as f32 / 65_535.0;
        let fy = ((hash >> 16) & 0xFFFF) as f32 / 65_535.0;
        let base = ((hash >> 32) & 0xFF) as f32 / 255.0;
        let phase = ((hash >> 40) & 0xFF) as f32 / 255.0 * TAU;
        let speed = (((hash >> 48) & 0x3) as f32).mul_add(0.6, 0.4);
        // Snapped to the pixel grid: a half-covered star is an off star.
        let star_x = snap(fx.mul_add(3.0f32.mul_add(-PX, glass.w), glass.x + PX));
        let star_y = snap(fy.mul_add(3.0f32.mul_add(-PX, glass.h), glass.y + PX));
        let twinkle = 0.15 * speed.mul_add(t, phase).sin();
        let alpha = (base.mul_add(0.45, 0.2) + twinkle).clamp(0.05, 0.8);
        let size = if hash & 0x300 == 0 { 2.0 * PX } else { PX };
        c.fill(
            layout::Rect::new(star_x, star_y, size, size),
            fade(mix(PHOSPHOR_DIM, PHOSPHOR, base), alpha),
        );
    }
}

/// Faint radar range rings around the sweep centre: CRT furniture.
fn draw_orbits(c: &Canvas, scene: &Scene, t: f32) {
    // The sun, and the orbit each world rides: the new range rings. The
    // comet's path stays undrawn — a comet should feel like a visitor.
    let flicker = (t * 3.0).sin().mul_add(0.06, 0.9);
    c.dot(SUN, 7.0, fade(AMBER, 0.85 * flicker));
    c.dot(SUN, 3.5, fade(GLINT, 0.9));
    c.ring(SUN, 10.0, 1.0, fade(AMBER, 0.25 * flicker));
    for (i, poi) in scene.sim.pois().iter().enumerate() {
        let Track::Circle { orbit, .. } = poi.track else {
            continue;
        };
        // The Umbra Market's little orbit stays secret in daylight.
        if !scene.sim.poi_visible(i as PoiId) {
            continue;
        }
        c.ring(SUN, orbit, 1.0, fade(PHOSPHOR_DIM, 0.4));
    }
}

fn draw_route_and_ship(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let ShipState::Traveling {
        from,
        to,
        progress,
        leg_ticks,
    } = sim.ship().state
    else {
        return;
    };
    // The charted line: cast-off point to where the destination will be at
    // the arrival tick. Same derivation the sim flies, so the dashes and
    // the freighter agree — and a jump re-aims both at once.
    let (a, b) = leg_endpoints(from, to, progress, leg_ticks, sim.tick());
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
            fade(PHOSPHOR, 0.45),
        );
    }

    // The freighter: a small triangle nosing along the route, with an
    // engine-flicker triangle behind it (bigger and faster under warp).
    let pos = sim.ship().interpolated(sim.alpha());
    let perp = Vec2::new(-dir.y, dir.x);
    // Engine flicker is decoration (it loops the whole leg); frozen it is a
    // steady flame, still longer under warp. The ship's travel is the sim's.
    let t = scene.idle_clock();
    let (freq, boost): (f32, f32) = if sim.is_warp() {
        (26.0, 1.9)
    } else {
        (9.0, 1.0)
    };
    let flick = 0.3f32.mul_add((t * freq).sin(), 0.7);
    let tail = (3.5 * boost).mul_add(flick, 3.0);
    let stern = pos - dir * 4.0;
    c.tri(
        stern + perp * 2.2,
        stern - perp * 2.2,
        stern - dir * tail,
        fade(PHOSPHOR, 0.75),
    );
    c.tri(
        pos + dir * 8.0,
        stern + perp * 4.5,
        stern - perp * 4.5,
        PHOSPHOR_HOT,
    );
}

fn draw_pois(c: &Canvas, scene: &Scene, glass: layout::Rect) {
    let sim = scene.sim;
    // Two clocks: `idle` for the decorative loops (glyph sparkles, the
    // armed-selection breathing), the real one for feedback tweens (the
    // catch-up dock pulse), which run under reduced motion.
    let t = scene.juice.clock();
    let idle = scene.idle_clock();
    let sweep = sweep_angle(idle);
    let center = rect_center(glass);
    let docked = match sim.ship().state {
        ShipState::Docked(at) => Some(at),
        ShipState::Traveling { .. } => None,
    };
    for (i, poi) in sim.pois().iter().enumerate() {
        let id = i as PoiId;
        if !sim.poi_visible(id) {
            continue;
        }
        let pos = sim.poi_pos(id);
        poi_glyph(c, i, pos, poi.radius, idle, MAP_PH);

        // Inner-to-inner courses check transit papers at charting time:
        // docked at one inner world without a chit, its rivals wear a
        // barred ring — visible, not chartable.
        if docked.is_some() && sim.inner_ring_locked(id) {
            let r = poi.radius + 4.0;
            c.ring(pos, r, 1.2, fade(LAMP_NO, 0.55));
            let bar = Vec2::new(r * 0.8, -r * 0.8);
            c.seg(pos - bar, pos + bar, 1.4, fade(LAMP_NO, 0.55));
        }

        // The sweep's afterglow brightens a POI it just passed. A parked
        // sweep passes nothing, so under reduced motion the afterglow stays
        // off instead of freezing into a phantom highlight.
        if !scene.reduced_motion {
            let glow = sweep_glow(center, sweep, pos);
            if glow > 0.0 {
                c.ring(pos, poi.radius + PX, 1.0, fade(PHOSPHOR, 0.30 * glow));
            }
        }

        // A comet already picked clean this pass sits under a haze: still
        // there, nothing left to chip.
        if id == space_trucking::sim::COMET && sim.comet_spent() {
            c.dot(pos, poi.radius, fade(SCREEN, 0.45));
        }

        // Hover: a faint ring over any POI the player could actually
        // chart right now — the invite comes from the same predicate the
        // press consults, so the glass never invites a refusal silently.
        if let Some(at) = docked {
            if id != at && sim.poi_chartable(id) && (scene.pointer - pos).length() <= poi.radius {
                c.ring(pos, poi.radius + 3.0, 1.0, fade(PHOSPHOR, 0.3));
            }
        }

        // Selected: a pulsing ring (the pulse is decoration — frozen it is
        // a steady ring that still says "armed"), plus a blip right when
        // picked (feedback).
        if sim.ship().selected == Some(id) {
            let wave = (idle * 4.0).sin();
            c.ring(
                pos,
                wave.mul_add(1.5, poi.radius + 5.0),
                1.4,
                fade(PHOSPHOR, wave.mul_add(0.15, 0.7)),
            );
            let blip = scene.juice.select_blip();
            if blip > 0.0 {
                c.ring(
                    pos,
                    (1.0 - blip).mul_add(9.0, poi.radius + 4.0),
                    1.2,
                    fade(PHOSPHOR, blip * 0.8),
                );
            }
        }

        // The hangar swallow: as `Delivered` fires, the Guild station
        // blooms violet for a moment. Hangar business, not a phosphor
        // reading — the same licence the omen cast and jump flash hold to
        // put the suspicious colour on a screen.
        if id == GUILD {
            let flash = scene.juice.delivered_flash();
            if flash > 0.0 {
                c.dot(pos, poi.radius + 2.0, fade(EERIE, 0.30 * flash));
                c.ring(
                    pos,
                    (1.0 - flash).mul_add(10.0, poi.radius + 3.0),
                    1.4,
                    fade(EERIE_BRIGHT, 0.7 * flash),
                );
            }
        }

        // Docked: a solid ring; pop on arrival; a long pulse after an
        // offline catch-up so the returning player spots where they landed.
        if docked == Some(id) {
            c.ring(pos, poi.radius + 4.0, 1.6, fade(PHOSPHOR, 0.85));
            let pop = scene.juice.dock_pop();
            if pop > 0.0 {
                c.ring(
                    pos,
                    (1.0 - pop).mul_add(12.0, poi.radius + 4.0),
                    1.5,
                    fade(PHOSPHOR, pop * 0.9),
                );
            }
            let pulse = scene.juice.dock_pulse();
            if pulse > 0.0 {
                let wave = (t * 5.0).sin();
                c.ring(
                    pos,
                    wave.mul_add(2.5, poi.radius + 7.0),
                    1.5,
                    fade(
                        PHOSPHOR_HOT,
                        wave.mul_add(0.25, 0.45) * (pulse * 3.0).min(1.0),
                    ),
                );
            }
        }
    }
}

// -------------------------------------------------------------- POI glyphs --

/// One hand-drawn glyph per POI index, shared by the map, the preview, and
/// the dial badge. `ph` is how far its colours lean toward phosphor: the
/// CRT shows a *reading* of Venus, not Venus.
fn poi_glyph(c: &Canvas, id: usize, pos: Vec2, r: f32, t: f32, ph: f32) {
    match id {
        0 => venus_glyph(c, pos, r, t, ph),
        1 => earth_glyph(c, pos, r, ph),
        2 => mars_glyph(c, pos, r, ph),
        3 => jupiter_glyph(c, pos, r, ph),
        4 => uranus_glyph(c, pos, r, ph),
        5 => neptune_glyph(c, pos, r, ph),
        7 => saturn_glyph(c, pos, r, ph),
        8 => umbra_glyph(c, pos, r, ph),
        9 => hermitage_glyph(c, pos, r, t, ph),
        10 => comet_glyph(c, pos, r, ph),
        11 => wanderer_glyph(c, pos, r, t, ph),
        _ => guild_glyph(c, pos, r, t, ph),
    }
}

/// Venus: pink-gold, with a glittery halo the rich paid extra for.
fn venus_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32, ph: f32) {
    c.dot(pos, r, phosphorize(POI_VENUS, ph));
    c.ring(
        pos,
        r * 1.45,
        1.0,
        fade(phosphorize(POI_VENUS_HALO, ph), 0.5),
    );
    for i in 0..3_u8 {
        let angle = f32::from(i).mul_add(2.1, t * 0.5);
        let sparkle = polar(pos, r * 1.45, angle);
        let glint = (f32::from(i).mul_add(1.7, t * 3.0)).sin();
        c.dot(
            sparkle,
            r.mul_add(0.10, 0.6),
            fade(phosphorize(GLINT, ph), glint.mul_add(0.3, 0.65)),
        );
    }
}

/// Earth: blue-gray under a brown smog ring.
fn earth_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r, phosphorize(POI_EARTH, ph));
    c.arc(
        pos,
        r * 1.1,
        -160.0,
        240.0,
        (r * 0.16).max(1.2),
        fade(phosphorize(POI_SMOG, ph), 0.85),
    );
}

/// Mars: rust-red with a patchwork wedge of the scrappy republic.
fn mars_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r, phosphorize(POI_MARS, ph));
    c.tri(
        pos,
        polar(pos, r * 0.96, -0.7),
        polar(pos, r * 0.96, 0.45),
        phosphorize(POI_MARS_PATCH, ph),
    );
}

/// Jupiter: a banded orange ellipse.
fn jupiter_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    let rx = r * 1.12;
    let ry = r * 0.82;
    let col = phosphorize(POI_JUPITER, ph);
    c.oval(pos, rx, ry, 0.0, col);
    for (fy, shade) in [(-0.42_f32, 0.78), (0.05, 0.66), (0.5, 0.8)] {
        let chord = rx * fy.mul_add(-fy, 1.0_f32).sqrt() * 0.94;
        c.oval(
            pos + Vec2::new(0.0, fy * ry),
            chord,
            ry * 0.12,
            0.0,
            dim(col, shade),
        );
    }
}

/// Uranus: pale cyan with a thin tilted ring.
fn uranus_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r * 0.92, phosphorize(POI_URANUS, ph));
    c.oval_ring(
        pos,
        r * 1.7,
        r * 0.5,
        20.0,
        1.0,
        fade(phosphorize(POI_URANUS_RING, ph), 0.7),
    );
}

/// Neptune: deep blue with a faint pale streak of storm.
fn neptune_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r, phosphorize(POI_NEPTUNE, ph));
    c.seg(
        pos + Vec2::new(-r * 0.5, -r * 0.3),
        pos + Vec2::new(r * 0.6, -r * 0.15),
        (r * 0.16).max(1.0),
        fade(phosphorize(GLINT, ph), 0.5),
    );
}

/// The Grand Parade: once the hangar counter fills, a procession of
/// heptagons crosses the whole sky, once, slowly, and does not explain.
fn draw_parade(c: &Canvas, scene: &Scene, glass: layout::Rect) {
    let Some(frac) = scene.sim.parade() else {
        return;
    };
    let t = scene.idle_clock();
    let across = frac.mul_add(glass.w + 120.0, glass.x - 60.0);
    let lead = Vec2::new(across, frac.mul_add(-60.0, glass.h.mul_add(0.4, glass.y)));
    for i in 0..5_u32 {
        let back = lead - Vec2::new(22.0f32.mul_add(i as f32, 26.0), (i as f32) * -6.0);
        let r = if i == 0 { 14.0 } else { 7.0 - i as f32 };
        if back.x < glass.x + 8.0 || back.x > glass.x + glass.w - 8.0 {
            continue;
        }
        let pulse = t.mul_add(2.0, i as f32).sin().mul_add(0.08, 0.8);
        c.poly(back, 7, r.max(2.5), t * 6.0, fade(EERIE, 0.5 * pulse));
        c.poly_ring(
            back,
            7,
            r.max(2.5),
            t * 6.0,
            1.0,
            fade(EERIE_BRIGHT, 0.8 * pulse),
        );
    }
}

/// Whatever is alongside mid-leg: its badge on the encounter plate, its
/// spectacle on the glass, the flotsam nets, and the ad drone.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn draw_travel_company(c: &Canvas, scene: &Scene, glass: layout::Rect) {
    let sim = scene.sim;
    if !matches!(sim.ship().state, ShipState::Traveling { .. }) {
        return;
    }
    let t = scene.idle_clock();

    // Flotsam nets: wells appear when there is (or could be) drift.
    let drifting = sim
        .pieces()
        .iter()
        .any(|p| matches!(p.loc, Loc::Flotsam { .. }));
    let open = sim
        .encounter()
        .is_some_and(space_trucking::sim::Encounter::open);
    if drifting || open {
        for &slot in &layout::FLOTSAM_SLOTS {
            c.frame(inflate(slot, PX), fade(PHOSPHOR_DIM, 0.7));
            socket_well(c, slot);
        }
    }

    if let Some(enc) = sim.encounter() {
        if enc.open() {
            // The whale itself, vast and patient, crossing under the sky.
            // (Its badge, like every encounter's, sits on the barter
            // panel's dial housing — the console's one context-dependent
            // corner.)
            if enc.kind == EncounterKind::Whale {
                if let ShipState::Traveling { progress, .. } = sim.ship().state {
                    let frac = (progress.saturating_sub(enc.start)) as f32
                        / (enc.end - enc.start).max(1) as f32;
                    let at = Vec2::new(
                        frac.mul_add(glass.w * 0.8, glass.w.mul_add(0.1, glass.x)),
                        (t * 0.5).sin().mul_add(4.0, glass.h.mul_add(0.78, glass.y)),
                    );
                    c.oval(at, 26.0, 7.0, -4.0, fade(PHOSPHOR_DIM, 0.5));
                    let tail = at + Vec2::new(-30.0, 0.0);
                    c.tri(
                        tail,
                        tail + Vec2::new(-8.0, -7.0),
                        tail + Vec2::new(-8.0, 7.0),
                        fade(PHOSPHOR_DIM, 0.5),
                    );
                    c.dot(at + Vec2::new(18.0, -2.0), 1.2, fade(PHOSPHOR, 0.8));
                }
            }
        }
    }

    // The ad drone: a tiny billboard on a tight orbit, wobbling per swat.
    if let Some(at) = sim.drone_pos() {
        let wobble = f32::from(sim.drone_swats()) * (t * 21.0).sin() * 2.0;
        let at = at + Vec2::new(wobble, 0.0);
        c.dot(at, 2.2, fade(AMBER, 0.9));
        let board = layout::Rect::new(at.x - 7.0, at.y - 12.0, 14.0, 8.0);
        c.fill(board, fade(GLINT, 0.85));
        for (i, ch) in [3_u8, 11, 7].into_iter().enumerate() {
            ad_rune(
                c,
                ch.wrapping_add((t * 2.0) as u8),
                Vec2::new((i as f32).mul_add(4.4, board.x + 2.0), board.y + 1.5),
                4.0,
                SHADOW,
            );
        }
        c.seg(at, at + Vec2::new(0.0, -4.0), 1.0, fade(AMBER, 0.7));
    }
}

/// A dead ship, keel up, one porthole dark.
fn derelict_badge(c: &Canvas, mid: Vec2, t: f32) {
    let list = (t * 0.8).sin() * 0.1;
    c.tri(
        mid + Vec2::new(-16.0, 8.0),
        mid + Vec2::new(18.0, list.mul_add(10.0, 4.0)),
        mid + Vec2::new(-10.0, -8.0),
        fade(PHOSPHOR_DIM, 0.9),
    );
    c.dot(mid + Vec2::new(-4.0, 0.0), 2.2, SCREEN);
    c.seg(
        mid + Vec2::new(6.0, -2.0),
        mid + Vec2::new(12.0, -10.0),
        1.0,
        fade(PHOSPHOR_DIM, 0.6),
    );
}

/// The inexplicable gas station: pump, hose, one honest drip. Dimmed
/// once its one top-up is spent.
fn gas_badge(c: &Canvas, mid: Vec2, used: bool, t: f32) {
    let a = if used { 0.35 } else { 0.9 };
    c.fill(
        layout::Rect::new(mid.x - 8.0, mid.y - 10.0, 10.0, 20.0),
        fade(PHOSPHOR, a),
    );
    c.fill(
        layout::Rect::new(mid.x - 6.0, mid.y - 7.0, 6.0, 5.0),
        fade(SCREEN, 0.9),
    );
    c.arc(
        mid + Vec2::new(6.0, -6.0),
        6.0,
        -90.0,
        180.0,
        1.2,
        fade(PHOSPHOR, a),
    );
    let drip = (t * 2.0).fract() * 6.0;
    c.dot(mid + Vec2::new(12.0, drip), 1.0, fade(PHOSPHOR, a * 0.8));
}

/// The casino: a neon heptagram with running lights. No visible doors.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn casino_badge(c: &Canvas, mid: Vec2, t: f32) {
    c.poly_ring(mid, 7, 15.0, t * 14.0, 1.4, fade(AMBER, 0.9));
    c.poly_ring(mid, 7, 10.0, t * -20.0, 1.0, fade(EERIE_BRIGHT, 0.8));
    for i in 0..7_u32 {
        let angle = (i as f32).mul_add(std::f32::consts::TAU / 7.0, t * 1.5);
        let on = ((t * 6.0) as u32 + i) % 3 == 0;
        c.dot(
            polar(mid, 18.0, angle),
            1.2,
            fade(GLINT, if on { 0.9 } else { 0.2 }),
        );
    }
}

/// Gravel weather, incoming.
fn meteor_badge(c: &Canvas, mid: Vec2, t: f32) {
    for (i, off) in [-8.0_f32, 0.0, 9.0].into_iter().enumerate() {
        let jitter = t.mul_add(3.0, i as f32).fract() * 5.0;
        let head = mid + Vec2::new(off + jitter, -6.0 + jitter);
        c.seg(head + Vec2::new(-8.0, -8.0), head, 1.2, fade(AMBER, 0.7));
        c.dot(head, 1.5, fade(GLINT, 0.9));
    }
}

/// A fluke, mid-dive.
fn whale_badge(c: &Canvas, mid: Vec2, t: f32) {
    let sway = (t * 0.9).sin() * 2.0;
    c.tri(
        mid + Vec2::new(sway, 8.0),
        mid + Vec2::new(-13.0, -8.0),
        mid + Vec2::new(-2.0, -2.0),
        fade(PHOSPHOR, 0.85),
    );
    c.tri(
        mid + Vec2::new(sway, 8.0),
        mid + Vec2::new(13.0, -8.0),
        mid + Vec2::new(2.0, -2.0),
        fade(PHOSPHOR, 0.85),
    );
    c.arc(
        mid + Vec2::new(0.0, 12.0),
        8.0,
        180.0,
        180.0,
        1.0,
        fade(PHOSPHOR_DIM, 0.6),
    );
}

/// One rune of the ad tongue: an angular scrawl derived from the byte, in
/// a language nobody aboard reads. (It is trying very hard to sell you
/// something; the glyphs are procedural so it never accidentally succeeds.)
fn ad_rune(c: &Canvas, ch: u8, at: Vec2, size: f32, col: Color) {
    let h = splitmix(0xAD_51_11, u64::from(ch));
    let point = |n: u64| {
        Vec2::new(
            (((h >> n) % 4) as f32 / 3.0).mul_add(size, at.x),
            (((h >> (n + 8)) % 5) as f32 / 4.0 * size).mul_add(1.4, at.y),
        )
    };
    let mut prev = point(0);
    for leg_bits in [16_u64, 32, 48] {
        let next = point(leg_bits);
        c.seg(prev, next, 1.0, col);
        prev = next;
    }
}

/// The ad ticker: while the drone is attached, every screen in the shop
/// runs its incomprehensible pitch. Swat the drone to make it stop.
fn draw_ad_static(c: &Canvas, scene: &Scene, glass: layout::Rect) {
    if !scene.sim.advertising() {
        return;
    }
    let t = scene.juice.clock();
    // The pitch, in lojban, transliterated into the rune alphabet. It
    // says something like "buy the great shining thing"; nobody asked.
    let pitch = b"ko te vecnu lo banli je carmi dacti .i e'osai";
    let band = layout::Rect::new(glass.x, glass.y + 6.0, glass.w, 12.0);
    c.fill(band, fade(SHADOW, 0.55));
    let step = 9.0;
    let scroll = if scene.reduced_motion {
        0.0
    } else {
        (t * 30.0) % (pitch.len() as f32 * step)
    };
    for (i, &ch) in pitch.iter().enumerate() {
        let x = (i as f32).mul_add(step, band.x + band.w - scroll);
        if x < band.x + 2.0 || x > band.x + band.w - step {
            continue;
        }
        ad_rune(c, ch, Vec2::new(x, band.y + 2.0), 6.0, fade(AMBER, 0.85));
    }
}

/// The Guild Station: a gray-violet heptagon, pulsing like it knows
/// things. Seven sides; the officially published schematics show six.
fn guild_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32, ph: f32) {
    let pulse = (t * 2.4).sin().mul_add(0.05, 1.0);
    let rot = t * 4.0;
    c.poly(pos, 7, r * pulse, rot, phosphorize(POI_GUILD, ph));
    c.poly_ring(pos, 7, r * pulse, rot, 1.0, phosphorize(POI_GUILD_EDGE, ph));
}

/// Saturn: pale gold under a broad debris ring — the ring-barons'
/// graveyard, a thousand failed hauling companies ground to gravel.
fn saturn_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r * 0.85, phosphorize(POI_SATURN, ph));
    c.oval_ring(
        pos,
        r * 1.8,
        r * 0.55,
        -18.0,
        1.4,
        phosphorize(POI_SATURN_RING, ph),
    );
    // Gravel in the ring: three coarse grains along its long axis.
    for f in [-1.35_f32, -0.6, 1.1] {
        let a = (-18.0_f32).to_radians();
        let at = pos + Vec2::new(a.cos(), a.sin()) * (r * f);
        c.dot(at, 1.2, phosphorize(POI_SATURN_RING, ph));
    }
}

/// The Umbra Market: a crescent sliver in Mercury's shadow. Only drawn at
/// all while somebody's clock reads deep night.
fn umbra_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    c.dot(pos, r, phosphorize(POI_UMBRA, ph));
    // The shadowed face: a screen-dark bite that leaves a crescent.
    c.dot(pos + Vec2::new(r * 0.45, -r * 0.2), r * 0.85, SCREEN);
    c.dot(
        pos + Vec2::new(-r * 0.45, r * 0.3),
        1.3,
        phosphorize(GLINT, ph),
    );
}

/// The Hermitage: a lumpy rock in the belt with one warm lit window.
fn hermitage_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32, ph: f32) {
    c.poly(pos, 5, r, 23.0, phosphorize(POI_HERMITAGE, ph));
    c.poly_ring(
        pos,
        5,
        r,
        23.0,
        1.0,
        phosphorize(dim(POI_HERMITAGE, 0.35), ph),
    );
    // The window: a lamp that breathes very slowly. Hermits keep odd hours.
    let warmth = (t * 0.7).sin().mul_add(0.2, 0.75);
    c.dot(
        pos + Vec2::new(r * 0.3, -r * 0.15),
        1.6,
        fade(AMBER, warmth),
    );
}

/// The comet: an icy head with its tail thrown away from the sun. The
/// tail rides a capped radius so the destination preview's enlarged
/// glyph keeps its plume on the glass instead of across the console.
fn comet_glyph(c: &Canvas, pos: Vec2, r: f32, ph: f32) {
    let away = pos - SUN;
    let len = away.length().max(f32::EPSILON);
    let dir = away * len.recip();
    let perp = Vec2::new(-dir.y, dir.x);
    let stretch = r.min(12.0);
    for (spread, reach, a) in [(0.0, 3.2, 0.6), (0.5, 2.4, 0.4), (-0.5, 2.4, 0.4)] {
        c.seg(
            pos + dir * r * 0.4,
            pos + dir * r * 0.4 + (dir + perp * spread * 0.3) * stretch * reach,
            1.2,
            fade(phosphorize(POI_COMET, ph), a),
        );
    }
    c.dot(pos, r * 0.75, phosphorize(POI_COMET, ph));
    c.dot(pos + dir * -0.2 * r, r * 0.35, phosphorize(GLINT, ph));
}

/// ???: a diamond that is not entirely committed to existing.
fn wanderer_glyph(c: &Canvas, pos: Vec2, r: f32, t: f32, ph: f32) {
    let there = (t * 6.3).sin().mul_add(0.25, 0.65);
    c.poly_ring(
        pos,
        4,
        r,
        45.0,
        1.4,
        fade(phosphorize(POI_WANDERER, ph), there),
    );
    c.poly_ring(
        pos,
        4,
        r * 0.55,
        45.0,
        1.0,
        fade(phosphorize(POI_WANDERER, ph), 1.0 - there),
    );
    c.dot(pos, 1.5, fade(phosphorize(GLINT, ph), there));
}

// ----------------------------------------------------------------- console --

fn draw_console(c: &Canvas, scene: &Scene) {
    plate(c, layout::CONSOLE, SALT_CONSOLE);
    draw_preview(c, scene);
    draw_eta(c, scene);
    draw_launch_lever(c, scene);
    draw_toggle_buttons(c, scene);
    draw_hangar_plate(c, scene);
}

// ------------------------------------------------------- hangar lamp plate --

/// The hangar tally plate, filling the bare console metal right of the
/// toggle buttons. Display-only — nothing hit-tests it — so like the wants
/// row its geometry is render-chosen rather than `layout`'s business.
const HANGAR_PLATE: layout::Rect = layout::Rect::new(682.0, 380.0, 100.0, 40.0);

/// The recessed glass strip the six lamps sit behind.
const HANGAR_WELL: layout::Rect = layout::Rect::new(690.0, 392.0, 84.0, 16.0);

/// Left-most lamp centre x, and the spacing between lamp centres.
const HANGAR_LAMP_X0: f32 = 699.0;
const HANGAR_LAMP_STEP: f32 = 13.2;

/// Six unlabeled lamps behind one glass strip, filling as the Guild's
/// hangar swallows crates — one per [`DELIVERY_LAMPS`] threshold. Violet,
/// not a status green: this is hangar business, and the plate says only
/// that something is being counted, never what.
fn draw_hangar_plate(c: &Canvas, scene: &Scene) {
    plate(c, HANGAR_PLATE, SALT_HANGAR);
    socket_well(c, HANGAR_WELL);
    let deliveries = scene.sim.deliveries();
    let wake = scene.juice.lamp_wake();
    // The shimmer is decoration: frozen, each lit lamp holds a steady
    // brightness near the shimmer's midpoint. The wake blink is feedback
    // and still runs off its juice timer.
    let t = scene.idle_clock();
    let mid_y = HANGAR_WELL.h.mul_add(0.5, HANGAR_WELL.y);
    for (i, &threshold) in DELIVERY_LAMPS.iter().enumerate() {
        let at = Vec2::new((i as f32).mul_add(HANGAR_LAMP_STEP, HANGAR_LAMP_X0), mid_y);
        // Lit lamps idle on a faint out-of-step shimmer: counting,
        // not signalling.
        let shimmer = (i as f32).mul_add(1.7, t * 1.1).sin().mul_add(0.06, 0.82);
        let level = match wake {
            // The newest lamp blinks awake: a couple of stutters easing
            // into the steady shimmer.
            Some((lamp, rise)) if lamp == i => {
                let stutter = (rise * TAU * 2.5).sin().mul_add(0.5, 0.5);
                shimmer * ease_out(rise) * lerp(stutter, 1.0, rise)
            }
            _ if deliveries >= threshold => shimmer,
            _ => 0.0,
        };
        lamp(c, at, 2.0 * PX, EERIE, level);
    }
}

/// Destination preview: the console's second CRT, showing where you are
/// going — or, dimmed, where you already are.
fn draw_preview(c: &Canvas, scene: &Scene) {
    // (The ad overlay lands at the end of this function, over the glass.)
    let sim = scene.sim;
    let glass = crt_screen(c, layout::DEST_PREVIEW, SALT_PREVIEW);
    preview_grid(c, glass);
    let (id, bright) = match sim.ship().state {
        ShipState::Traveling { to, .. } => (to, true),
        ShipState::Docked(at) => sim.ship().selected.map_or((at, false), |sel| (sel, true)),
    };
    // Idle shows the tube's green *reading* of where you already are; the
    // richer enamel tint is reserved for an armed destination, so the
    // screen visibly wakes up when there is somewhere to go.
    let ph = if bright { PREVIEW_PH } else { MAP_PH };
    poi_glyph(
        c,
        usize::from(id),
        rect_center(glass),
        48.0,
        scene.idle_clock(),
        ph,
    );
    if !bright {
        c.fill(glass, fade(SHADOW, 0.35));
    }
    finish_screen(c, glass, scene);
    draw_ad_static(c, scene, glass);
}

/// Faint alignment grid on the preview glass: the tube is powered even
/// with nothing armed.
fn preview_grid(c: &Canvas, glass: layout::Rect) {
    let step = 12.0 * PX;
    let x0 = (glass.x / step).ceil() * step;
    for i in 0..=((glass.x + glass.w - x0) / step) as i32 {
        c.fill(
            layout::Rect::new((i as f32).mul_add(step, x0), glass.y, PX, glass.h),
            fade(PHOSPHOR_DIM, 0.40),
        );
    }
    let y0 = (glass.y / step).ceil() * step;
    for i in 0..=((glass.y + glass.h - y0) / step) as i32 {
        c.fill(
            layout::Rect::new(glass.x, (i as f32).mul_add(step, y0), glass.w, PX),
            fade(PHOSPHOR_DIM, 0.40),
        );
    }
}

/// ETA gauge: a recessed ring that drains as the leg completes; a full dim
/// ring while docked with a destination armed. Metal, not a screen.
fn draw_eta(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let mid = layout::ETA_ARC_CENTER;
    let radius = layout::ETA_ARC_RADIUS;
    // At rest this is still an instrument, not a hole in the panel: dark
    // glass with a reflection, a ticked track, and a standby ember.
    c.dot(mid, 3.0f32.mul_add(PX, radius), GLASS);
    c.arc(
        mid,
        3.0f32.mul_add(PX, radius),
        135.0,
        180.0,
        1.0,
        PLATE_SHADE,
    );
    c.arc(
        mid,
        3.0f32.mul_add(PX, radius),
        -45.0,
        180.0,
        1.0,
        fade(PLATE_LIT, 0.7),
    );
    c.dot(
        mid + Vec2::new(-radius * 0.4, -radius * 0.4),
        radius * 0.3,
        fade(GLINT, 0.08),
    );
    c.ring(mid, radius, 1.0, fade(GLINT, 0.16));
    // Tick marks around the track; twelve o'clock, where the arc starts
    // and ends, reads brightest.
    for i in 0..8_u8 {
        let angle = f32::from(i).mul_add(TAU / 8.0, -PI / 2.0);
        let alpha = if i == 0 { 0.32 } else { 0.18 };
        c.seg(
            polar(mid, radius - 3.0, angle),
            polar(mid, radius + 1.0, angle),
            1.0,
            fade(GLINT, alpha),
        );
    }
    c.dot(mid, PX, fade(AMBER, 0.45));
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

/// The launch lever: a brass handle on a riveted plate, its go-lamp lit
/// when a pull would work, with a thunk slide on departure and a rattle on
/// a refused pull.
fn draw_launch_lever(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let juice = scene.juice;
    let rect = layout::LAUNCH_LEVER;
    plate(c, rect, SALT_LAUNCH);
    let mid_y = rect.h.mul_add(0.5, rect.y);
    socket_well(
        c,
        layout::Rect::new(
            rect.x + 16.0,
            2.0f32.mul_add(-PX, mid_y),
            rect.w - 32.0,
            4.0 * PX,
        ),
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
    // The go-glow's breathing is decoration; frozen it holds its midpoint,
    // and pullable still reads from the lit lamp. The shake above is
    // feedback and keeps the real clock.
    let glow = (scene.idle_clock() * 2.2).sin().mul_add(0.12, 0.78);
    if pullable {
        c.fill(inflate(handle, 2.0 * PX), fade(LAMP_OK, 0.12 * glow));
    }
    brass_handle(c, handle);
    lamp(
        c,
        Vec2::new(rect_center(handle).x, 4.5f32.mul_add(PX, handle.y)),
        2.2 * PX,
        LAMP_OK,
        if pullable { glow } else { 0.0 },
    );
}

fn draw_toggle_buttons(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    for r in [layout::PAUSE_BTN, layout::SPEAKER] {
        // A raised cap on the console plate; too small for rivets or wear.
        c.fill(r, PLATE);
        bevel(c, r, PLATE_LIT, PLATE_SHADE);
    }
    if scene.dev {
        // Fast-forward exists only for developers who said pretty-please;
        // everyone else gets a blank patch of console and real time.
        c.fill(layout::WARP_BTN, PLATE);
        bevel(c, layout::WARP_BTN, PLATE_LIT, PLATE_SHADE);
    }

    // Pause: two bars, and its lamp.
    let mid = icon_center(layout::PAUSE_BTN);
    let bars = if sim.is_paused() { AMBER } else { ICON };
    c.fill(layout::Rect::new(mid.x - 7.0, mid.y - 9.0, 5.0, 18.0), bars);
    c.fill(layout::Rect::new(mid.x + 2.0, mid.y - 9.0, 5.0, 18.0), bars);
    button_lamp(
        c,
        layout::PAUSE_BTN,
        AMBER,
        if sim.is_paused() { 1.0 } else { 0.0 },
    );

    // Warp: a double chevron, and its lamp. Dev-only furniture.
    if scene.dev {
        let warp_col = if sim.is_warp() { AMBER } else { ICON };
        chevrons(c, icon_center(layout::WARP_BTN), 7.0, warp_col);
        button_lamp(
            c,
            layout::WARP_BTN,
            AMBER,
            if sim.is_warp() { 1.0 } else { 0.0 },
        );
    }

    draw_speaker(c, scene);
}

/// Icon anchor inside a toggle button, nudged up to clear the lamp.
fn icon_center(r: layout::Rect) -> Vec2 {
    rect_center(r) - Vec2::new(0.0, 2.5 * PX)
}

/// The state lamp under a toggle button's icon.
fn button_lamp(c: &Canvas, r: layout::Rect, col: Color, level: f32) {
    lamp(
        c,
        Vec2::new(rect_center(r).x, 4.5f32.mul_add(-PX, r.y + r.h)),
        2.0 * PX,
        col,
        level,
    );
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

/// Speaker: its lamp pulses amber until the browser lets audio start,
/// holds a calm green while sound runs, and goes dark glass when muted.
fn draw_speaker(c: &Canvas, scene: &Scene) {
    let mid = icon_center(layout::SPEAKER);
    // The waiting pulse is decoration; frozen, the lamp holds mid-bright
    // amber, which still reads against the running state's calm green.
    let t = scene.idle_clock();
    let (icon, lamp_col, level) = if scene.audio_waiting {
        (AMBER, AMBER, (t * 5.0).sin().mul_add(0.3, 0.65))
    } else if scene.audio_muted {
        (ICON, AMBER, 0.0)
    } else {
        (ICON_LIT, LAMP_OK, 0.45)
    };
    c.tri(
        Vec2::new(mid.x - 4.0, mid.y),
        Vec2::new(mid.x + 7.0, mid.y - 9.0),
        Vec2::new(mid.x + 7.0, mid.y + 9.0),
        icon,
    );
    c.fill(
        layout::Rect::new(mid.x - 10.0, mid.y - 5.0, 7.0, 10.0),
        icon,
    );
    if scene.audio_muted {
        c.seg(
            Vec2::new(mid.x - 12.0, mid.y + 11.0),
            Vec2::new(mid.x + 12.0, mid.y - 11.0),
            2.0,
            fade(LAMP_NO, 0.9),
        );
    }
    button_lamp(c, layout::SPEAKER, lamp_col, level);
}

// -------------------------------------------------------------------- hold --

fn draw_hold(c: &Canvas, scene: &Scene) {
    let cs = c.shifted(grid_shake_offset(scene.juice));
    plate(&cs, inflate(grid_rect(), 10.0), SALT_HOLD);
    for y in 0..layout::GRID_ROWS {
        for x in 0..layout::GRID_COLS {
            socket_well(&cs, inflate(layout::cell_rect(x, y), -2.0));
        }
    }
    draw_drop_hints(&cs, scene);
}

/// Per-cell legality tint under a held player piece: green footprint where
/// the drop would land, red where a rule refuses it.
fn draw_drop_hints(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let Some(held) = sim.held(0) else {
        return;
    };
    if !player_owned(held.origin) {
        return;
    }
    let Some(piece) = piece_by_id(sim, held.piece) else {
        return;
    };
    let Some((anchor_x, anchor_y)) = layout::cell_at(scene.pointer) else {
        return;
    };
    let legal = placement_check(sim.pieces(), piece.id, piece.kind, anchor_x, anchor_y).is_ok();
    let col = if legal { LAMP_OK } else { LAMP_NO };
    let (foot_w, foot_h) = piece.kind.cells();
    for dy in 0..foot_h {
        for dx in 0..foot_w {
            let (cx, cy) = (anchor_x + dx, anchor_y + dy);
            if cx < layout::GRID_COLS && cy < layout::GRID_ROWS {
                let cell = inflate(layout::cell_rect(cx, cy), -2.0);
                c.fill(cell, fade(col, 0.22));
                if !legal {
                    // No hue alone: every refused footprint cell wears a
                    // diagonal slash (the mute slash's angle), so illegal
                    // reads by shape where legal stays a plain fill.
                    c.seg(
                        Vec2::new(cell.x + PX, cell.y + cell.h - PX),
                        Vec2::new(cell.x + cell.w - PX, cell.y + PX),
                        1.5,
                        fade(LAMP_NO, 0.65),
                    );
                }
            }
        }
    }
}

// ------------------------------------------------------------------ barter --

/// The barter surface. Its furniture is bolted to the panel, so it stays
/// drawn mid-flight — dormant, lamps dark — and lights up when the sim
/// opens a trade.
fn draw_barter(c: &Canvas, scene: &Scene) {
    plate(c, layout::BARTER_PANEL, SALT_BARTER);
    let barter = scene.sim.barter();
    draw_slot_rows(c, scene);
    if let Some(barter) = barter {
        draw_wants(c, scene, barter);
    } else {
        draw_wayside(c, scene);
    }
    draw_dial(c, scene, barter);
    draw_accept_lever(c, scene, barter);
}

/// The panel's other lives, whenever no trade is open. Underway, the dial
/// housing wears the encounter's badge and the shelf row is the outboard
/// rail; docked at ???, the housing wears the diamond and the shelf row
/// shows the toll — three sockets, filling as crates come aboard; docked
/// at the comet, the housing acknowledges the berth, dimly.
fn draw_wayside(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let t = scene.idle_clock();
    let mid = layout::DIAL_CENTER;
    match sim.ship().state {
        ShipState::Traveling { .. } => {
            if let Some(enc) = sim.encounter().filter(|enc| enc.open()) {
                let badge = layout::ENCOUNTER_BADGE;
                c.frame(inflate(badge, PX), fade(PHOSPHOR_DIM, 0.8));
                let bmid = rect_center(badge);
                match enc.kind {
                    EncounterKind::Derelict => derelict_badge(c, bmid, t),
                    EncounterKind::GasStation => gas_badge(c, bmid, enc.used, t),
                    EncounterKind::Casino => casino_badge(c, bmid, t),
                    EncounterKind::MeteorShower => meteor_badge(c, bmid, t),
                    EncounterKind::Whale => whale_badge(c, bmid, t),
                }
            }
        }
        ShipState::Docked(at) => {
            // A barterless berth: say where we are moored, quietly.
            poi_glyph(c, usize::from(at), mid, 11.0, t, 0.0);
            if at == space_trucking::sim::WANDERER {
                draw_wanderer_toll(c, scene, t);
            }
        }
    }
}

/// ???'s donation ledger on the shelf row: three sockets — the toll — each
/// glowing violet once a mysterious crate is stowed against it. Wordless:
/// three holes, your parcels, do the arithmetic.
fn draw_wanderer_toll(c: &Canvas, scene: &Scene, t: f32) {
    let aboard = scene.sim.mysterious_aboard();
    for (i, &slot) in layout::FLOTSAM_SLOTS.iter().take(3).enumerate() {
        let lit = (i as u32) < aboard;
        let mid = rect_center(slot);
        let breath = t.mul_add(0.8, i as f32).sin().mul_add(0.1, 0.55);
        c.poly_ring(
            mid,
            4,
            slot.w * 0.3,
            45.0,
            1.2,
            fade(EERIE_BRIGHT, if lit { breath } else { 0.18 }),
        );
        if lit {
            c.dot(mid, 2.0, fade(EERIE, breath));
        }
    }
}

fn draw_slot_rows(c: &Canvas, scene: &Scene) {
    let trading = scene.sim.barter().is_some();
    let shuttered = scene
        .sim
        .barter()
        .is_some_and(|barter| barter.patience == 0);
    // No trade open: the shelf row is the outboard rail — same sockets,
    // phosphor trim instead of shelf paint — and the trading rows go dark.
    let shelf_trim = if trading { TRIM_SHELF } else { PHOSPHOR_DIM };
    for (slots, trim) in [
        (&layout::SHELF_SLOTS, shelf_trim),
        (
            &layout::RECEIVED_SLOTS,
            if trading { TRIM_RECEIVED } else { GLASS },
        ),
        (&layout::GIVE_SLOTS, if trading { TRIM_GIVE } else { GLASS }),
        (&layout::TAKE_SLOTS, if trading { TRIM_TAKE } else { GLASS }),
    ] {
        for &slot in slots {
            // Painted trim on the plate names the row; the well is inset.
            c.frame(inflate(slot, PX), fade(trim, 0.8));
            socket_well(c, slot);
        }
    }

    // Out of patience: corrugated shutters over the station's own row.
    // Gifts still move through the give pads — and one may reopen this.
    if shuttered {
        for &slot in &layout::SHELF_SLOTS {
            c.fill(inflate(slot, PX), PLATE_SHADE);
            let slats = ((slot.h - 5.0) / 5.0) as i32;
            for i in 0..slats {
                let y = (i as f32).mul_add(5.0, slot.y + 3.0);
                c.seg(
                    Vec2::new(slot.x + 2.0, y),
                    Vec2::new(slot.x + slot.w - 2.0, y),
                    1.0,
                    fade(PLATE_LIT, 0.35),
                );
            }
        }
    }

    // While a piece is held, glow the rows that would actually accept it.
    // The set comes from the sim's own drop matrix, never re-derived here:
    // an affordance the rules would refuse cannot be drawn.
    if let Some(targets) = scene.sim.drop_targets(0) {
        for (slots, invited) in [
            (&layout::SHELF_SLOTS, targets.shelf || targets.net),
            (&layout::RECEIVED_SLOTS, targets.received),
            (&layout::GIVE_SLOTS, targets.give),
            (&layout::TAKE_SLOTS, targets.take),
        ] {
            if !invited {
                continue;
            }
            // The invite glow's breathing is decoration; frozen at its
            // midpoint the amber frame still reads as an invitation.
            let pulse = (scene.idle_clock() * 2.0).sin().mul_add(0.08, 0.55);
            for &slot in slots {
                c.fill(inflate(slot, 2.0), fade(AMBER, 0.10));
                c.frame(inflate(slot, 2.0), fade(AMBER, pulse));
            }
        }
    }
}

/// The station's top-three wants: small kind glyphs with 1-3 pips under
/// them, tucked between the shelf and the received row.
fn draw_wants(c: &Canvas, scene: &Scene, barter: &Barter) {
    let t = scene.idle_clock();
    for (i, &(kind, pips)) in barter.wants.iter().enumerate() {
        let x = (i as f32).mul_add(56.0, 303.0);
        piece_glyph(
            c,
            kind,
            0,
            false,
            layout::Rect::new(x - 11.0, 500.0, 22.0, 22.0),
            t,
        );
        for p in 0..pips {
            let px = (f32::from(pips) - 1.0)
                .mul_add(-0.5, f32::from(p))
                .mul_add(7.0, x);
            c.dot(Vec2::new(px, 528.0), 2.5, fade(AMBER, 0.85));
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
        mix(LAMP_NO, AMBER, value)
    } else {
        mix(AMBER, LAMP_OK, (value - 1.0).clamp(0.0, 1.0))
    }
}

/// The eagerness dial: a physical gauge in a raised round housing, needle
/// eased by the sim, notch at break-even, the station's badge in the
/// middle. Metal, never a screen.
// The gauge is one instrument: housing, badge, track, fog, needle, pips,
// and flashes belong together even past the line lint's comfort.
#[allow(clippy::too_many_lines)]
fn draw_dial(c: &Canvas, scene: &Scene, barter: Option<&Barter>) {
    let juice = scene.juice;
    let mid = layout::DIAL_CENTER;
    let t = juice.clock();

    // The housing: a raised disc, rim lit toward the light.
    c.dot(mid, 34.0, PLATE);
    c.arc(mid, 32.0, 135.0, 180.0, 1.0, PLATE_LIT);
    c.arc(mid, 32.0, -45.0, 180.0, 1.0, PLATE_SHADE);

    // The station badge: enamel colours while docked, dark glass in flight.
    // The refusal wobble is feedback (real clock); the glyph's own sparkle
    // loop is decoration (idle clock).
    if let Some(barter) = barter {
        let shake = juice.station_shake();
        let wobble = Vec2::new(
            (t * 67.0).sin() * 2.2 * shake,
            (t * 51.0).sin() * 1.6 * shake,
        );
        poi_glyph(
            c,
            usize::from(barter.station),
            mid + wobble,
            11.0,
            scene.idle_clock(),
            0.0,
        );
    } else {
        c.dot(mid, 11.0, GLASS);
        c.dot(mid + Vec2::new(-3.5, -3.5), 3.0, fade(GLINT, 0.08));
    }

    // Track groove, then the gradient fill up to the eased needle.
    c.arc(mid, 23.0, DIAL_START, DIAL_SWEEP, 5.0, fade(SHADOW, 0.5));
    let value = barter.map_or(0.0, |barter| {
        lerp(barter.prev_eagerness, barter.eagerness, scene.sim.alpha())
    });
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
        fade(GLINT, 0.8),
    );
    // Unfamiliar goods fog the reading: the needle wanders inside a hazy
    // band whose width is the guesswork fraction. What the station would
    // actually say stays hidden until these kinds have been traded here —
    // or until somebody gambles a pull.
    let fog = barter.map_or(0.0, |barter| barter.fog);
    let needle = DIAL_SWEEP.mul_add(frac, DIAL_START).to_radians();
    if fog > 0.0 {
        let half = fog * 0.55;
        c.arc(
            mid,
            18.0,
            needle.to_degrees() - half.to_degrees(),
            2.0 * half.to_degrees(),
            10.0,
            fade(GLASS, 0.75),
        );
        let wobble = (scene.idle_clock() * 9.0).sin() * half * 0.6;
        c.seg(
            polar(mid, 8.0, needle + wobble),
            polar(mid, 29.0, needle + wobble),
            2.0,
            fade(GLINT, 0.55),
        );
    } else {
        c.seg(
            polar(mid, 8.0, needle),
            polar(mid, 29.0, needle),
            2.0,
            GLINT,
        );
    }

    // Patience pips under the housing: how many refused pulls this visit
    // will still tolerate. All dark means the shutters are down.
    if let Some(barter) = barter {
        for i in 0..space_trucking::sim::PATIENCE {
            let at = Vec2::new(f32::from(i).mul_add(9.0, mid.x - 9.0), mid.y + 40.0);
            let left = i < barter.patience;
            lamp(
                c,
                at,
                1.6,
                if barter.patience == 0 { LAMP_NO } else { AMBER },
                if left { 0.8 } else { 0.0 },
            );
        }
    }

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
            fade(LAMP_NO, flash * 0.5),
        );
    }
    if let Some((heat, value)) = juice.accept_flash() {
        let grow = (1.0 - heat) * 45.0 * value.mul_add(1.2, 1.0);
        c.ring(mid, 12.0 + grow, 2.5, fade(LAMP_OK, heat * 0.9));
        c.ring(mid, (12.0 + grow) * 0.65, 1.5, fade(AMBER, heat * 0.7));
    }
}

/// The accept lever: brass on its own small plate, go-lamp lit the instant
/// the station would say yes.
fn draw_accept_lever(c: &Canvas, scene: &Scene, barter: Option<&Barter>) {
    let rect = layout::ACCEPT_LEVER;
    plate(c, rect, SALT_ACCEPT);
    let mid_y = rect.h.mul_add(0.5, rect.y);
    socket_well(
        c,
        layout::Rect::new(
            rect.x + 12.0,
            1.5f32.mul_add(-PX, mid_y),
            rect.w - 24.0,
            3.0 * PX,
        ),
    );
    let handle = layout::Rect::new(rect.x + 20.0, rect.y + 6.0, 14.0, rect.h - 12.0);
    let ready = barter.is_some_and(|barter| barter.ready);
    let certain = barter.is_some_and(|barter| barter.fog <= 0.0);
    // Decoration, like the launch lever's glow: frozen at its midpoint.
    let glow = (scene.idle_clock() * 2.8).sin().mul_add(0.12, 0.78);
    if ready && certain {
        c.fill(inflate(handle, 2.0 * PX), fade(LAMP_OK, 0.12 * glow));
    }
    brass_handle(c, handle);
    // The go-lamp answers only for trades made of known quantities. Fogged
    // pads get an amber shimmer instead: pull if you like, it says, and
    // find out — that is the price of knowing.
    let lamp_at = Vec2::new(rect_center(handle).x, 3.5f32.mul_add(PX, handle.y));
    if certain {
        lamp(
            c,
            lamp_at,
            1.8 * PX,
            LAMP_OK,
            if ready { glow } else { 0.0 },
        );
    } else {
        let shimmer = (scene.idle_clock() * 5.0).sin().mul_add(0.2, 0.45);
        lamp(c, lamp_at, 1.8 * PX, AMBER, shimmer);
    }
}

// ------------------------------------------------------------------ pieces --

/// The discovery haze: a shimmer over pieces whose kind this station has
/// never traded with the player. Their worth is a rumor until they cross
/// the pads in a concluded deal.
fn unfamiliar_veil(c: &Canvas, scene: &Scene, piece: &Piece, rect: layout::Rect) {
    if scene.sim.barter().is_none()
        || !matches!(
            piece.loc,
            Loc::StationShelf { .. } | Loc::GivePad { .. } | Loc::TakePad { .. }
        )
        || scene.sim.kind_familiar(piece.kind)
    {
        return;
    }
    let t = scene.idle_clock();
    c.fill(rect, fade(GLASS, 0.4));
    let mid = rect_center(rect);
    let drift = Vec2::new((t * 1.7).sin() * 3.0, (t * 1.3).cos() * 2.0);
    c.dot(mid + drift, 1.2, fade(GLINT, 0.5));
    c.ring(mid - drift, 3.5, 1.0, fade(GLINT, 0.25));
}

fn draw_pieces(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let juice = scene.juice;
    // The clock here only feeds decoration — the crate's breathing and the
    // crew ghost's breathing frame; slides and settles are juice heats.
    let t = scene.idle_clock();
    let shaken = c.shifted(grid_shake_offset(juice));
    for piece in sim.pieces() {
        let held_by = sim
            .all_held()
            .find(|(_, held)| held.piece == piece.id)
            .map(|(player, _)| player);
        if held_by == Some(0) {
            // The local player's drag: drawn glued to the pointer by
            // `draw_held` instead.
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
        if held_by.is_some() {
            crew_ghost(canvas, piece, rect, t);
        } else {
            piece_glyph(canvas, piece.kind, piece.variant, piece.gnawed, rect, t);
            unfamiliar_veil(canvas, scene, piece, rect);
        }
    }
}

/// Another crew member's drag, seen from this seat. There is no remote
/// pointer to follow yet, so the honest rendering is a dim ghost at the
/// piece's home (the sim parks `loc` at the origin until the drop): the
/// glyph sunk toward the socket under a faint slow-breathing frame —
/// someone is holding this, and it is not you.
fn crew_ghost(c: &Canvas, piece: &Piece, rect: layout::Rect, t: f32) {
    piece_glyph(c, piece.kind, piece.variant, piece.gnawed, rect, t);
    c.fill(rect, fade(SOCKET, 0.55));
    c.frame_thick(
        inflate(rect, -PX),
        1.0,
        fade(GLINT, (t * 2.0).sin().mul_add(0.05, 0.25)),
    );
}

/// The held piece: enlarged, shadowed, glued to the pointer, tinted by
/// whether letting go would work.
fn draw_held(c: &Canvas, scene: &Scene) {
    let sim = scene.sim;
    let Some(held) = sim.held(0) else {
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
    c.fill(shifted_rect(rect, 3.0, 4.0), fade(SHADOW, 0.5));
    piece_glyph(
        c,
        piece.kind,
        piece.variant,
        piece.gnawed,
        rect,
        scene.idle_clock(),
    );
    let col = if held.legal { LAMP_OK } else { LAMP_NO };
    c.fill(rect, fade(col, 0.10));
    c.frame_thick(rect, 1.5, fade(col, 0.7));
    if !held.legal {
        // No hue alone: a refused drop wears a diagonal slash as well as
        // the red tint, so the state survives without colour.
        c.seg(
            Vec2::new(rect.x + PX, rect.y + rect.h - PX),
            Vec2::new(rect.x + rect.w - PX, rect.y + PX),
            1.5,
            fade(LAMP_NO, 0.7),
        );
    }
}

// ----------------------------------------------------------- cargo glyphs --

/// One silhouette per cargo kind, fitted into `rect` (a hold footprint or a
/// slot). Variants shift the hue and scale the inner detail — same kind,
/// sibling crates. A gnawed piece wears the rat's bite on top.
fn piece_glyph(c: &Canvas, kind: Kind, variant: u8, gnawed: bool, rect: layout::Rect, t: f32) {
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
        Kind::MysteriousCrate => mysterious_glyph(c, b, col, vs, t),
        Kind::VeryMysteriousCrate => very_mysterious_glyph(c, b, col, vs, t),
        Kind::CometIce => ice_glyph(c, b, col, vs),
        Kind::BottledMidnight => midnight_glyph(c, b, col, vs),
        Kind::Fluff => fluff_glyph(c, b, col, vs, t),
        Kind::TransitChit => chit_glyph(c, b, col, vs),
        Kind::CasinoChip => chip_glyph(c, b, col, vs),
    }
    if gnawed {
        bite_mark(c, b);
    }
}

/// A small parcel in dun paper, lashed with twine, addressed to no one.
/// Deliberately nothing like the matte-black crates: muted, domestic,
/// almost boring — until you catch the faint violet breath under the
/// wrapping, on a slow cycle you have to be watching to see.
fn mysterious_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32, t: f32) {
    let parcel = inflate(b, -b.w * 0.08);
    c.fill(parcel, col);
    bevel(c, parcel, dim(col, -0.08), dim(col, 0.18));
    // Twine, crossed, with a knot slightly off-centre (hand-tied).
    let twine = dim(col, 0.35);
    let knot = Vec2::new(
        parcel.w.mul_add(0.42 * vs, parcel.x),
        parcel.h.mul_add(0.5, parcel.y),
    );
    c.seg(
        Vec2::new(knot.x, parcel.y),
        Vec2::new(knot.x, parcel.y + parcel.h),
        1.2,
        twine,
    );
    c.seg(
        Vec2::new(parcel.x, knot.y),
        Vec2::new(parcel.x + parcel.w, knot.y),
        1.2,
        twine,
    );
    c.dot(knot, 1.4, dim(col, 0.5));
    // The breath: a slow, easily-missed violet seep at the paper's seam.
    let breath = (t * 0.8).sin().max(0.0) * 0.14;
    if breath > 0.01 {
        c.frame(inflate(parcel, -PX), fade(EERIE, breath));
    }
}

/// The big one. It hums a chord.
fn very_mysterious_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32, t: f32) {
    c.fill(b, col);
    bevel(c, b, dim(col, -0.1), SHADOW);
    let hum = (t * 2.2).sin().mul_add(0.18, 0.45);
    c.ring(
        rect_center(b),
        b.w * 0.28 * vs,
        1.4,
        fade(EERIE_BRIGHT, hum),
    );
    c.dot(rect_center(b), b.w * 0.1, fade(EERIE_BRIGHT, hum * 0.8));
}

/// A shard chipped off the comet, still cold enough to demand the hull.
fn ice_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let mid = rect_center(b);
    c.tri(
        Vec2::new(mid.x, (b.h * (1.0 - vs)).mul_add(0.5, b.y)),
        Vec2::new(b.w.mul_add(0.2, b.x), b.h.mul_add(0.85, b.y)),
        Vec2::new(b.w.mul_add(0.85, b.x), b.h.mul_add(0.75, b.y)),
        col,
    );
    c.seg(
        Vec2::new(mid.x, b.h.mul_add(0.25, b.y)),
        Vec2::new(b.w.mul_add(0.35, b.x), b.h.mul_add(0.7, b.y)),
        1.0,
        fade(GLINT, 0.7),
    );
}

/// A bottle of the dark between stars, corked.
fn midnight_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let body = layout::Rect::new(
        b.w.mul_add(0.28, b.x),
        b.h.mul_add(0.35, b.y),
        b.w * 0.44,
        b.h * 0.6,
    );
    c.fill(body, col);
    c.fill(
        layout::Rect::new(
            b.w.mul_add(0.42, b.x),
            b.h.mul_add(0.12, b.y),
            b.w * 0.16,
            b.h * 0.26,
        ),
        col,
    );
    c.fill(
        layout::Rect::new(
            b.w.mul_add(0.40, b.x),
            b.h.mul_add(0.06, b.y),
            b.w * 0.2,
            b.h * 0.08,
        ),
        BRASS,
    );
    // One star, somewhere inside the bottle.
    c.dot(
        Vec2::new(
            (body.w * 0.6).mul_add(vs, body.x),
            body.h.mul_add(0.4, body.y),
        ),
        1.0,
        fade(GLINT, 0.9),
    );
}

/// A legally distinct ball of fur. Do not feed after midnight; do not
/// feed at all, actually — see what happened to the hold.
fn fluff_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32, t: f32) {
    let mid = rect_center(b);
    let breathe = (t * 2.8).sin().mul_add(0.04, 1.0) * vs;
    let r = b.w * 0.34 * breathe;
    c.dot(mid + Vec2::new(-r * 0.4, r * 0.2), r * 0.8, dim(col, 0.12));
    c.dot(mid + Vec2::new(r * 0.45, r * 0.25), r * 0.7, dim(col, 0.06));
    c.dot(mid + Vec2::new(0.0, -r * 0.2), r, col);
    // Two dark bead eyes. It is looking at you. It is multiplying.
    c.dot(mid + Vec2::new(-r * 0.25, -r * 0.25), 1.0, SHADOW);
    c.dot(mid + Vec2::new(r * 0.25, -r * 0.25), 1.0, SHADOW);
}

/// Inner-ring transit papers: a punch-card with the Guild's stripe.
fn chit_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let card = layout::Rect::new(
        b.w.mul_add(0.15, b.x),
        b.h.mul_add(0.28, b.y),
        b.w * 0.7,
        b.h * 0.46,
    );
    c.fill(card, col);
    bevel(c, card, dim(col, -0.1), dim(col, 0.25));
    c.fill(
        layout::Rect::new(card.w.mul_add(0.12, card.x), card.y, card.w * 0.14, card.h),
        POI_GUILD,
    );
    for i in 0..3_u8 {
        c.dot(
            Vec2::new(
                (card.w * f32::from(i + 1).mul_add(0.2, 0.25)).mul_add(vs, card.x),
                card.h.mul_add(0.5, card.y),
            ),
            0.9,
            SOCKET,
        );
    }
}

/// One casino chip. The house assures you it is priceless.
fn chip_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32) {
    let mid = rect_center(b);
    let r = b.w * 0.36 * vs;
    c.dot(mid, r, col);
    c.ring(mid, r * 0.94, 1.2, dim(col, 0.3));
    c.ring(mid, r * 0.55, 1.0, fade(GLINT, 0.65));
    for i in 0..4_u8 {
        let a = f32::from(i).mul_add(std::f32::consts::FRAC_PI_2, 0.4);
        c.dot(polar(mid, r * 0.8, a), 1.0, fade(GLINT, 0.8));
    }
}

/// The rat's mark: two socket-coloured teeth scalloped out of the glyph's
/// upper-right shoulder, angled toward the middle so every kind's
/// silhouette gets visibly cut. Geometry, not hue, per the accessibility
/// direction — it survives any palette and any variant tint.
fn bite_mark(c: &Canvas, b: layout::Rect) {
    let edge = b.x + b.w;
    let depth = b.w * 0.38;
    let tooth = b.h * 0.19;
    for i in 0..2_u8 {
        let top = (f32::from(i)).mul_add(tooth, b.h.mul_add(0.08, b.y));
        c.tri(
            Vec2::new(edge, top),
            Vec2::new(edge, top + tooth),
            Vec2::new(edge - depth, tooth.mul_add(0.5, top)),
            SOCKET,
        );
    }
}

/// Inner-detail scale per variant, `0.85..=1.15`.
fn variant_scale(variant: u8) -> f32 {
    f32::from(variant % 4).mul_add(0.1, 0.85)
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
        GLINT,
    );
    c.seg(
        sparkle - Vec2::new(0.0, arm),
        sparkle + Vec2::new(0.0, arm),
        1.0,
        GLINT,
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
    c.poly_ring(mid, 6, b.w * 0.46, 90.0, 1.0, fade(GLINT, 0.55));
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
            fade(GLINT, 0.75),
        );
    }
}

/// The suspicious crate: matte black, faintly humming violet at ~1 Hz —
/// the same beat the audio hum keeps.
fn crate_glyph(c: &Canvas, b: layout::Rect, col: Color, vs: f32, t: f32) {
    let body = scaled(b, 0.86);
    let pulse = (TAU * t).sin().mul_add(0.5, 0.5);
    c.fill(inflate(body, 3.0), fade(EERIE, pulse.mul_add(0.22, 0.08)));
    c.fill(body, col);
    c.frame(body, dim(EERIE, 0.4));
    c.frame_thick(
        inflate(body, 1.5),
        1.0,
        fade(EERIE, pulse.mul_add(0.35, 0.15)),
    );
    c.dot(
        rect_center(b),
        b.w * 0.05 * vs,
        fade(EERIE_BRIGHT, pulse.mul_add(0.5, 0.3)),
    );
}

// ----------------------------------------------------------------- the rat --

/// Hop tween length in ticks: 0.35 s, inside the juice conventions'
/// half-second cap.
const RAT_HOP_TICKS: f32 = 21.0;

/// The stowaway: a few pixels of gray-brown metal-family paint tucked into
/// its cell's lower corner, drawn over the cargo (it perches on pieces when
/// the hold is full). The hop tween derives from sim state — `prev_cell`,
/// `moved_at`, the tick count, and the interpolation alpha — so it needs no
/// renderer clock of its own, replays exactly, and a skitter cue and its
/// hop can never disagree.
fn draw_rat(c: &Canvas, scene: &Scene) {
    let Some(rat) = scene.sim.rat() else {
        return;
    };
    let cs = c.shifted(grid_shake_offset(scene.juice));
    let age = scene.sim.tick().saturating_sub(rat.moved_at) as f32 + scene.sim.alpha();
    let t = (age / RAT_HOP_TICKS).clamp(0.0, 1.0);
    let from = rat_perch(rat.prev_cell);
    let to = rat_perch(rat.cell);
    let mut at = from.lerp(to, ease_out(t));
    // A shallow arc while mid-hop.
    at.y = (PI * t).sin().mul_add(-5.0, at.y);
    // The hop is sim-driven feedback and always runs; the tail sway inside
    // the glyph is decoration and takes the idle clock.
    rat_glyph(&cs, at, to.x <= from.x, scene.idle_clock());
}

/// Where the rat sits in cell `(x, y)`: low and left of centre, out of the
/// way of the piece silhouette above it.
fn rat_perch((x, y): (u8, u8)) -> Vec2 {
    let cell = layout::cell_rect(x, y);
    Vec2::new(cell.w.mul_add(0.42, cell.x), cell.h.mul_add(0.74, cell.y))
}

/// Body, head, ear, eye, tail — all metal-family grays (`RIVET` and
/// friends), nothing hue-coded, per the art doc. `facing_left` keeps its
/// nose pointed the way it last hopped; the tail sways on the cosmetic
/// clock.
fn rat_glyph(c: &Canvas, at: Vec2, facing_left: bool, t: f32) {
    let flip = if facing_left { -1.0 } else { 1.0 };
    let sway = (t * 3.0).sin() * 1.4;
    let tail_root = at + Vec2::new(-4.0 * flip, 0.5);
    let tail_tip = at + Vec2::new(-9.5 * flip, sway - 1.0);
    c.seg(tail_root, tail_tip, 1.0, dim(RIVET, 0.75));
    c.oval(at, 5.0, 3.0, 0.0, RIVET);
    let head = at + Vec2::new(5.0 * flip, -1.4);
    c.dot(head, 2.4, RIVET);
    c.dot(head + Vec2::new(-0.4 * flip, -2.2), 1.2, dim(RIVET, 0.85));
    c.dot(head + Vec2::new(1.3 * flip, -0.5), 0.7, SHADOW);
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
            c.frame_thick(grid_rect(), 2.0, fade(LAMP_NO, heat));
            c.frame_thick(rect, 2.0, fade(LAMP_NO, heat * 0.9));
        }
        Violation::Heavy => {
            c.fill(rect, fade(LAMP_NO, heat * 0.18));
            weight_glyph(c, mid, 12.0, fade(GLINT, heat));
        }
        Violation::Volatile => {
            c.fill(rect, fade(LAMP_NO, heat * 0.18));
            hazard_glyph(c, mid, 12.0, fade(AMBER, heat));
        }
        Violation::Cryo => {
            c.fill(rect, fade(LAMP_NO, heat * 0.18));
            snowflake_glyph(c, mid, 12.0, fade(kind_color(Kind::CryoCore), heat));
        }
        // A second crate aboard: the hold itself objects in violet.
        Violation::Suspicious => {
            c.fill(rect, fade(EERIE, heat * 0.45));
            c.frame_thick(rect, 2.0, fade(EERIE_BRIGHT, heat));
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

// ------------------------------------------------------------ ghost tutor --

/// The previous contractor's hand: the onboarding ghost from
/// `crate::tutor`, drawn last inside the fiction so it floats over the
/// furniture it points at, through the [`Canvas`] so it crunches and dims
/// with everything else. Overlay only — it consumes tutor output and
/// touches nothing. Every stroke is GLINT-family at low alpha, translucent
/// and calm and visibly not the player's own cursor, and its echoes borrow
/// dim versions of the glows the real interactions would light.
fn draw_tutor_ghost(c: &Canvas, scene: &Scene) {
    let Some(ghost) = scene.ghost else {
        return;
    };
    let a = ghost.alpha;
    if a <= 0.0 {
        return;
    }
    draw_ghost_echo(c, scene, ghost.echo, a);
    // The hand itself: a soft halo around a ring that sinks toward its dot
    // as the ghost "presses", with a bloom while the press holds. Bigger
    // and steadier than the juice conventions' flashes on purpose — it has
    // to read as a hand, not a spark.
    let press = ghost.press;
    let radius = press.mul_add(-3.0, 9.0);
    c.dot(ghost.pos, radius + 3.0, fade(GLINT, 0.10 * a));
    c.ring(ghost.pos, radius, 1.0, fade(GLINT, 0.75 * a));
    c.dot(ghost.pos, 3.0, fade(GLINT, 0.85 * a));
    if press > 0.01 {
        c.dot(ghost.pos, radius, fade(GLINT, 0.22 * press * a));
        c.ring(
            ghost.pos,
            press.mul_add(8.0, radius),
            1.0,
            fade(GLINT, 0.40 * press * a),
        );
    }
}

/// The ghost's dim echo of what a real press would light up. The console
/// cannot react to a fake hand, so the demonstration carries its own faint
/// consequences instead of lying loudly.
fn draw_ghost_echo(c: &Canvas, scene: &Scene, echo: Echo, a: f32) {
    match echo {
        Echo::None => {}
        Echo::Poi(id) => {
            // A dim cousin of the real selection ring, kept glued to the
            // moving planet rather than where it was at lesson start.
            let radius = POIS[usize::from(id)].radius;
            let at = scene.sim.poi_pos(id);
            c.ring(at, radius + 5.0, 1.2, fade(PHOSPHOR, 0.30 * a));
        }
        Echo::LaunchLever => ghost_lever_glow(c, layout::LAUNCH_LEVER, a),
        Echo::AcceptLever => ghost_lever_glow(c, layout::ACCEPT_LEVER, a),
        Echo::GiveSlot(slot) => {
            // A dim cousin of the drop-target invite.
            let r = inflate(layout::GIVE_SLOTS[usize::from(slot)], 2.0);
            c.fill(r, fade(AMBER, 0.06 * a));
            c.frame(r, fade(AMBER, 0.30 * a));
        }
        Echo::TakeSlot(slot) => {
            let r = inflate(layout::TAKE_SLOTS[usize::from(slot)], 2.0);
            c.fill(r, fade(AMBER, 0.06 * a));
            c.frame(r, fade(AMBER, 0.30 * a));
        }
        Echo::HoldPiece(id) => {
            if let Some(piece) = piece_by_id(scene.sim, id) {
                c.frame_thick(
                    inflate(layout::piece_rect(piece), PX),
                    1.0,
                    fade(GLINT, 0.35 * a),
                );
            }
        }
    }
}

/// A faint go-glow over a lever plate: the dim echo of its ready lamp.
fn ghost_lever_glow(c: &Canvas, r: layout::Rect, a: f32) {
    let inner = scaled(r, 0.9);
    c.fill(inner, fade(LAMP_OK, 0.10 * a));
    c.frame(inner, fade(LAMP_OK, 0.40 * a));
}

/// The omen's whole-panel cast and the jump's flash, drawn raw in world
/// space (inside the crunch target, so they pixelate with everything else)
/// but outside the canvas tint: they *are* the light change, and tinting
/// them would apply it twice.
fn draw_omen_fx(scene: &Scene) {
    let omen = scene.sim.omen();
    if omen > 0.0 {
        overlay(
            layout::Rect::new(0.0, 0.0, WORLD_W, WORLD_H),
            fade(EERIE, omen * 0.14),
        );
        let band = WORLD_H * 0.12;
        overlay(
            layout::Rect::new(0.0, 0.0, WORLD_W, band),
            fade(SHADOW, omen * 0.45),
        );
        overlay(
            layout::Rect::new(0.0, WORLD_H - band, WORLD_W, band),
            fade(SHADOW, omen * 0.45),
        );
    }
    let jump = scene.juice.jump_flash();
    if jump > 0.0 {
        overlay(
            layout::Rect::new(0.0, 0.0, WORLD_W, WORLD_H),
            fade(EERIE_BRIGHT, jump * jump * 0.55),
        );
    }
}

/// An untinted world-space fill, for effects that skip the palette tint.
fn overlay(r: layout::Rect, col: Color) {
    draw_rectangle(r.x, r.y, r.w, r.h, col);
}
