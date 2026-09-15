//! The shared software rasterizer: RGBA8 texels over a sim rect, drawn
//! with the 2D prototype's Canvas discipline — texel-centre sampling, no
//! antialiasing, stored light/omen tinting every stroke. Extracted from
//! `crt` so every painted surface (the map, the preview, the viewport)
//! speaks the same brush.

use std::f32::consts::TAU;

use bevy::prelude::*;
use space_trucking::sim::Vec2 as SimVec2;
use space_trucking::sim::layout::Rect;

use crate::palette;

/// Sim world units per texel — the 2D prototype's `CRUNCH`, verbatim, so
/// painted surfaces keep exactly the pixel density they were designed at.
pub const PX: f32 = 2.0;

// ------------------------------------------------------------------ color --

/// One rasterizer colour: sRGB channels plus alpha, `0..=1`. The texture
/// is `Rgba8UnormSrgb`, so bytes written from here read back as the same
/// sRGB values the 2D palette speaks.
#[derive(Clone, Copy, Debug)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// The hue the omen leans the whole picture toward. The one value the
/// cabin palette does not export (the 2D keeps its `CAST` private too);
/// hex from the root `src/palette.rs`, restated here in rasterizer terms.
const CAST: Rgba = Rgba {
    r: 107.0 / 255.0,
    g: 56.0 / 255.0,
    b: 148.0 / 255.0,
    a: 1.0,
};

/// A palette role as rasterizer ink. Every colour drawn below enters
/// through here, so the palette stays the one place hues live.
pub fn ink(col: Color) -> Rgba {
    let srgba = col.to_srgba();
    Rgba {
        r: srgba.red,
        g: srgba.green,
        b: srgba.blue,
        a: srgba.alpha,
    }
}

/// Alpha-scaled copy of `col`.
pub const fn fade(col: Rgba, alpha: f32) -> Rgba {
    Rgba {
        a: col.a * alpha,
        ..col
    }
}

/// Brightness-scaled copy of `col`.
pub const fn dim(col: Rgba, by: f32) -> Rgba {
    Rgba {
        r: col.r * by,
        g: col.g * by,
        b: col.b * by,
        a: col.a,
    }
}

/// Linear interpolation, shared by every colour ramp.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}

/// Channel-wise blend from `a` to `b`.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    Rgba {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
}

/// A CRT's reading of `col`: brightness mapped onto the phosphor ramp,
/// then leaned toward that reading by `amount` — the 2D `phosphorize`.
pub fn phosphorize(col: Rgba, amount: f32) -> Rgba {
    let luma = col.r.mul_add(0.299, col.g.mul_add(0.587, col.b * 0.114));
    let ramp = if luma > 0.75 {
        mix(
            ink(palette::PHOSPHOR),
            ink(palette::PHOSPHOR_HOT),
            (luma - 0.75) * 4.0,
        )
    } else {
        mix(
            ink(palette::PHOSPHOR_DIM),
            ink(palette::PHOSPHOR),
            luma / 0.75,
        )
    };
    // Keep the caller's alpha: phosphor changes hue, not translucency.
    Rgba {
        a: col.a,
        ..mix(col, ramp, amount)
    }
}

/// The console light level times the omen's violet cast — the 2D
/// `omen_tint`, applied to every draw call by [`Canvas::paint`].
pub fn omen_tint(col: Rgba, light: f32, omen: f32) -> Rgba {
    let cast = omen * 0.35;
    Rgba {
        r: (CAST.r - col.r).mul_add(cast, col.r) * light,
        g: (CAST.g - col.g).mul_add(cast, col.g) * light,
        b: (CAST.b - col.b).mul_add(cast, col.b) * light,
        a: col.a,
    }
}

// ----------------------------------------------------------------- canvas --

/// A software render target: RGBA8 texels over one sim rect. Draw calls
/// take sim world coordinates and hard-rasterize by texel-centre sample —
/// no antialiasing, the chunky edge is the aesthetic. The stored light and
/// omen levels tint every call, exactly like the 2D `Canvas`.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    /// Sim world position of texel (0, 0)'s top-left corner.
    pub origin: SimVec2,
    pub light: f32,
    pub omen: f32,
    /// RGBA8 rows, top to bottom — sim y points down, so no flip anywhere.
    pub px: Vec<u8>,
}

impl Canvas {
    /// A canvas covering `rect` at the 2D crunch density.
    #[allow(clippy::cast_sign_loss)] // layout sizes are positive
    pub fn new(rect: Rect) -> Self {
        let w = (rect.w / PX).round() as u32;
        let h = (rect.h / PX).round() as u32;
        Self {
            w,
            h,
            origin: SimVec2::new(rect.x, rect.y),
            light: 1.0,
            omen: 0.0,
            px: vec![0; (w * h * 4) as usize],
        }
    }

    /// Set this frame's console light level and omen cast.
    pub const fn mood(&mut self, light: f32, omen: f32) {
        self.light = light;
        self.omen = omen;
    }

    /// Line width quantized to whole texels, never below one, so strokes
    /// survive the crunch instead of vanishing at half-coverage.
    pub fn stroke(w: f32) -> f32 {
        (w / PX).round().max(1.0) * PX
    }

    /// Rasterize one primitive: every texel whose centre passes `inside`
    /// (in sim coordinates) takes `col`, tinted, blended src-over. The
    /// bounding box keeps the scan tight; no per-pixel allocation.
    #[allow(clippy::cast_sign_loss)] // texel bounds are clamped non-negative
    pub fn paint(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        col: Rgba,
        inside: impl Fn(f32, f32) -> bool,
    ) {
        let col = omen_tint(col, self.light, self.omen);
        let alpha = col.a.clamp(0.0, 1.0);
        if alpha <= 0.002 {
            return;
        }
        let col0 = (((x0 - self.origin.x) / PX).floor().max(0.0)) as u32;
        let col1 = ((((x1 - self.origin.x) / PX).ceil()).clamp(0.0, self.w as f32)) as u32;
        let row0 = (((y0 - self.origin.y) / PX).floor().max(0.0)) as u32;
        let row1 = ((((y1 - self.origin.y) / PX).ceil()).clamp(0.0, self.h as f32)) as u32;
        for row in row0..row1 {
            let sy = (row as f32 + 0.5).mul_add(PX, self.origin.y);
            for column in col0..col1 {
                let sx = (column as f32 + 0.5).mul_add(PX, self.origin.x);
                if inside(sx, sy) {
                    self.blend(column, row, col, alpha);
                }
            }
        }
    }

    /// Source-over one texel: `out = src·a + dst·(1−a)`, alpha composited.
    #[allow(clippy::cast_sign_loss)] // channel math stays inside 0..=1
    pub fn blend(&mut self, column: u32, row: u32, col: Rgba, alpha: f32) {
        let at = ((row * self.w + column) * 4) as usize;
        let inv = 1.0 - alpha;
        for (offset, channel) in [col.r, col.g, col.b].into_iter().enumerate() {
            let dst = f32::from(self.px[at + offset]) / 255.0;
            self.px[at + offset] = (channel.mul_add(alpha, dst * inv) * 255.0).round() as u8;
        }
        let dst = f32::from(self.px[at + 3]) / 255.0;
        self.px[at + 3] = (dst.mul_add(inv, alpha) * 255.0).round() as u8;
    }

    pub fn fill(&mut self, r: Rect, col: Rgba) {
        self.paint(r.x, r.y, r.x + r.w, r.y + r.h, col, |sx, sy| {
            sx >= r.x && sx < r.x + r.w && sy >= r.y && sy < r.y + r.h
        });
    }

    pub fn frame_thick(&mut self, r: Rect, w: f32, col: Rgba) {
        let s = Self::stroke(w);
        self.fill(Rect::new(r.x, r.y, r.w, s), col);
        self.fill(Rect::new(r.x, r.y + r.h - s, r.w, s), col);
        self.fill(Rect::new(r.x, r.y + s, s, 2.0f32.mul_add(-s, r.h)), col);
        self.fill(
            Rect::new(r.x + r.w - s, r.y + s, s, 2.0f32.mul_add(-s, r.h)),
            col,
        );
    }

    pub fn dot(&mut self, at: SimVec2, r: f32, col: Rgba) {
        // Sub-texel dots vanish at the crunch resolution; keep a floor.
        let radius = r.max(PX * 0.5);
        let r2 = radius * radius;
        self.paint(
            at.x - radius,
            at.y - radius,
            at.x + radius,
            at.y + radius,
            col,
            |sx, sy| {
                let dx = sx - at.x;
                let dy = sy - at.y;
                dx.mul_add(dx, dy * dy) <= r2
            },
        );
    }

    pub fn ring(&mut self, at: SimVec2, r: f32, w: f32, col: Rgba) {
        let half = Self::stroke(w) * 0.5;
        let reach = r + half;
        self.paint(
            at.x - reach,
            at.y - reach,
            at.x + reach,
            at.y + reach,
            col,
            |sx, sy| ((sx - at.x).hypot(sy - at.y) - r).abs() <= half,
        );
    }

    pub fn seg(&mut self, a: SimVec2, b: SimVec2, w: f32, col: Rgba) {
        let half = Self::stroke(w) * 0.5;
        let h2 = half * half;
        self.paint(
            a.x.min(b.x) - half,
            a.y.min(b.y) - half,
            a.x.max(b.x) + half,
            a.y.max(b.y) + half,
            col,
            |sx, sy| seg_dist2(a, b, SimVec2::new(sx, sy)) <= h2,
        );
    }

    pub fn tri(&mut self, a: SimVec2, b: SimVec2, d: SimVec2, col: Rgba) {
        self.paint(
            a.x.min(b.x).min(d.x),
            a.y.min(b.y).min(d.y),
            a.x.max(b.x).max(d.x),
            a.y.max(b.y).max(d.y),
            col,
            |sx, sy| {
                let p = SimVec2::new(sx, sy);
                let s0 = side(a, b, p);
                let s1 = side(b, d, p);
                let s2 = side(d, a, p);
                (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0)
            },
        );
    }

    /// Stroked triangle: three segments. Ported with the rest of the 2D
    /// primitive set; the map itself has no current call site.
    #[allow(dead_code)]
    pub fn tri_ring(&mut self, a: SimVec2, b: SimVec2, d: SimVec2, w: f32, col: Rgba) {
        self.seg(a, b, w, col);
        self.seg(b, d, w, col);
        self.seg(d, a, w, col);
    }

    /// Filled regular n-gon; `rot` in degrees, matching the 2D call sites.
    pub fn poly(&mut self, at: SimVec2, sides: u8, r: f32, rot: f32, col: Rgba) {
        let corners = ngon(at, sides, r, rot);
        let n = usize::from(sides).min(MAX_SIDES);
        self.paint(at.x - r, at.y - r, at.x + r, at.y + r, col, |sx, sy| {
            let p = SimVec2::new(sx, sy);
            let mut has_pos = false;
            let mut has_neg = false;
            for i in 0..n {
                let cross = side(corners[i], corners[(i + 1) % n], p);
                has_pos |= cross > 0.0;
                has_neg |= cross < 0.0;
            }
            !(has_pos && has_neg)
        });
    }

    /// Regular n-gon outline; `rot` in degrees.
    pub fn poly_ring(&mut self, at: SimVec2, sides: u8, r: f32, rot: f32, w: f32, col: Rgba) {
        let corners = ngon(at, sides, r, rot);
        let n = usize::from(sides).min(MAX_SIDES);
        for i in 0..n {
            self.seg(corners[i], corners[(i + 1) % n], w, col);
        }
    }

    /// Arc from `rot` degrees sweeping `sweep` degrees clockwise (sim y
    /// points down), drawn outward from `r` — the 2D `draw_arc` band.
    pub fn arc(&mut self, at: SimVec2, r: f32, rot: f32, sweep: f32, w: f32, col: Rgba) {
        let band = Self::stroke(w);
        let reach = r + band;
        self.paint(
            at.x - reach,
            at.y - reach,
            at.x + reach,
            at.y + reach,
            col,
            |sx, sy| {
                let dx = sx - at.x;
                let dy = sy - at.y;
                let dist = dx.hypot(dy);
                if dist < r || dist > r + band {
                    return false;
                }
                (dy.atan2(dx).to_degrees() - rot).rem_euclid(360.0) <= sweep
            },
        );
    }

    /// Filled ellipse; `rot` in degrees.
    pub fn oval(&mut self, at: SimVec2, rx: f32, ry: f32, rot: f32, col: Rgba) {
        let reach = rx.abs().max(ry.abs());
        let (sin, cos) = rot.to_radians().sin_cos();
        self.paint(
            at.x - reach,
            at.y - reach,
            at.x + reach,
            at.y + reach,
            col,
            |sx, sy| ellipse_level(at, rx, ry, sin, cos, sx, sy) <= 1.0,
        );
    }

    /// Stroked ellipse: between the ellipse grown and shrunk by half the
    /// stroke — the chunky reading of a constant-width ellipse outline.
    pub fn oval_ring(&mut self, at: SimVec2, rx: f32, ry: f32, rot: f32, w: f32, col: Rgba) {
        let half = Self::stroke(w) * 0.5;
        let reach = rx.abs().max(ry.abs()) + half;
        let (sin, cos) = rot.to_radians().sin_cos();
        let (inner_x, inner_y) = ((rx - half).max(0.1), (ry - half).max(0.1));
        self.paint(
            at.x - reach,
            at.y - reach,
            at.x + reach,
            at.y + reach,
            col,
            |sx, sy| {
                ellipse_level(at, rx + half, ry + half, sin, cos, sx, sy) <= 1.0
                    && ellipse_level(at, inner_x, inner_y, sin, cos, sx, sy) > 1.0
            },
        );
    }
}

// ------------------------------------------------------------- small math --

/// The largest n-gon the map draws is the parade's heptagon; a fixed
/// array keeps [`Canvas::poly`] allocation-free.
pub const MAX_SIDES: usize = 12;

/// Vertices of a regular n-gon, first corner at `rot` degrees — the same
/// spelling macroquad's `draw_poly` used, so glyph poses carry over.
pub fn ngon(at: SimVec2, sides: u8, r: f32, rot: f32) -> [SimVec2; MAX_SIDES] {
    let mut corners = [at; MAX_SIDES];
    let n = usize::from(sides).min(MAX_SIDES);
    let phase = rot.to_radians();
    for (i, corner) in corners.iter_mut().enumerate().take(n) {
        let angle = (i as f32 / n as f32).mul_add(TAU, phase);
        *corner = polar(at, r, angle);
    }
    corners
}

/// Twice the signed area of triangle `a b p`: the edge-function test.
pub fn side(a: SimVec2, b: SimVec2, p: SimVec2) -> f32 {
    (b.x - a.x).mul_add(p.y - a.y, -((b.y - a.y) * (p.x - a.x)))
}

/// Squared distance from `p` to segment `a b`.
pub fn seg_dist2(a: SimVec2, b: SimVec2, p: SimVec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len2 = ab.x.mul_add(ab.x, ab.y * ab.y);
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (ap.x.mul_add(ab.x, ap.y * ab.y) / len2).clamp(0.0, 1.0)
    };
    let dx = ab.x.mul_add(-t, ap.x);
    let dy = ab.y.mul_add(-t, ap.y);
    dx.mul_add(dx, dy * dy)
}

/// Rotated-frame ellipse membership: `<= 1` is inside.
pub fn ellipse_level(at: SimVec2, rx: f32, ry: f32, sin: f32, cos: f32, sx: f32, sy: f32) -> f32 {
    let dx = sx - at.x;
    let dy = sy - at.y;
    let lx = dx.mul_add(cos, dy * sin) / rx;
    let ly = dy.mul_add(cos, -(dx * sin)) / ry;
    lx.mul_add(lx, ly * ly)
}

/// Point at `radius` along `angle` (radians) from `from`.
pub fn polar(from: SimVec2, radius: f32, angle: f32) -> SimVec2 {
    SimVec2::new(
        radius.mul_add(angle.cos(), from.x),
        radius.mul_add(angle.sin(), from.y),
    )
}

pub const fn rect_center(r: Rect) -> SimVec2 {
    SimVec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y))
}

pub fn inflate(r: Rect, by: f32) -> Rect {
    Rect::new(
        r.x - by,
        r.y - by,
        2.0f32.mul_add(by, r.w),
        2.0f32.mul_add(by, r.h),
    )
}

/// `v` snapped down to the texel grid, for things (stars, scanlines) that
/// must land on whole texels or shimmer.
pub fn snap(v: f32) -> f32 {
    (v / PX).floor() * PX
}
