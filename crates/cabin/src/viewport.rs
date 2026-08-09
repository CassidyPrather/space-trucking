//! The cabin's front window: a plate-framed pane of actual space over the
//! spot the star tank vacated. Most of a trucker's life is transit, and the
//! void owed them better than ink — so the glass shows a slow parallax
//! starfield, the berth filling a flank while docked, star-streaks and a
//! growing destination while under way, and whatever pulls alongside
//! (whales, ad drones, parades, meteors) as silhouettes instead of sensor
//! readings.
//!
//! The picture is painted by the shared [`Canvas`] rasterizer into an
//! emissive texture, exactly the plumbing the CRTs use — but this is a
//! window, not a tube: no scanlines, no vignette, no phosphor ramp. Stars
//! are glint-white with faint POI-hue tints; the only green out there is a
//! whale. Colours enter through palette roles, decoration phases ride
//! `splitmix` offsets over a held clock, and the omen dims and
//! violet-casts space itself through the canvas mood.

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use space_trucking::sim::layout::Rect;
use space_trucking::sim::{
    COMET, Cue, EncounterKind, GUILD, PoiId, SATURN, ShipState, Sim, Vec2 as SimVec2, WANDERER,
    WARP_FACTOR, splitmix,
};

use crate::canvas::{Canvas, PX, dim, fade, inflate, ink, lerp, mix, snap};
use crate::palette;
use crate::{Phase, Shell};

// -------------------------------------------------------------- placement --
// The window's furniture — pane quad, frame bars, rivets — lives on the
// `Kind::Window` piece rig now (`pieces::build_kind`); this module owns
// only the sky itself.

// ----------------------------------------------------------------- canvas --

/// The window's own logical rect: 500×420 sim units → 250×210 texels at
/// the shared crunch density. Its coordinate space, nobody else's.
const VIEW_W: f32 = 500.0;
const VIEW_H: f32 = 420.0;
const VIEW: Rect = Rect::new(0.0, 0.0, VIEW_W, VIEW_H);

// ------------------------------------------------------------------- tune --

/// Render-side seeds; cosmetic only, never the sim's.
const STAR_SEED: u64 = 0x0BE7_57A6;
const DUST_SEED: u64 = 0xD057_0001;
const SPARK_SEED: u64 = 0x5AA7_C1E5;
const DRONE_SEED: u64 = 0xAD_2222;
const METEOR_SEED: u64 = 0x3E7E_0C0C;

/// Deep-field census. Two size classes fall out of the hash.
const STAR_COUNT: u64 = 90;
/// Ambient parallax drift, sim units per second at full depth — alive
/// even docked, calm enough to watch with coffee.
const DRIFT: f32 = 2.6;
/// Streaming gain: sim units of star travel per odometer unit.
const STREAM: f32 = 22.0;
/// Streak length per unit of smoothed speed at full depth. Subtle dashes
/// at cruise; 16× warp draws honest comet tails.
const STREAK: f32 = 3.4;
/// Seconds for the smoothed speed to close most of a gap (feedback fast).
const SPEED_EASE: f32 = 0.22;
/// Stream-speed multiplier while the burner is stoked. The sim makes two
/// ticks of way per tick of fire — at cruise and under warp alike — so
/// the glass doubles its streaks honestly rather than by vibes.
const STOKED_FACTOR: f32 = 2.0;

/// Where the berth hangs while docked: half-cropped off the left edge,
/// filling a flank of the window.
const BERTH: SimVec2 = SimVec2::new(28.0, 252.0);
const BERTH_R: f32 = 142.0;
/// Berth bob: ±3 units at ~0.1 Hz. Station-keeping, not weather.
const BOB_HZ: f32 = 0.1;
/// Seconds between dust-mote glint sparkles while parked.
const SPARK_EVERY: f32 = 5.0;

/// Leg fraction where the view stops looking back and starts looking
/// ahead. Below it the origin shrinks astern; above it the destination
/// grows — slowly, then all at once. Long hauls should feel long.
const HANDOFF: f32 = 0.15;
/// Where the destination swells: a little off-centre, like every horizon.
const DEST_AT: SimVec2 = SimVec2::new(VIEW_W * 0.60, VIEW_H * 0.45);
const DEST_R0: f32 = 3.0;
const DEST_R1: f32 = 85.0;

/// The jump seen from inside: the glass floods violet and fades squared.
const JUMP_LEN: f32 = 0.45;

// ----------------------------------------------------------------- plugin --

/// The glass texture handle plus its reusable paint buffer. The handle
/// is crate-visible: the transit window is CARGO now, and its piece rig
/// wears this sky on its glass wherever it is rehung — the paint stays
/// here, the furniture rides the piece.
#[derive(Resource)]
pub struct Pane {
    pub image: Handle<Image>,
    canvas: Canvas,
}

/// Motion clocks the sim refuses to own: a held phase clock, a streaming
/// odometer, the smoothed stream speed, and the jump-flood timer. Frozen
/// wholesale while paused — a paused world should look paused.
#[derive(Resource, Default)]
struct SkyClock {
    t: f32,
    run: f32,
    speed: f32,
    jump: f32,
}

/// One frame's sample of [`SkyClock`], handed to the painter so headless
/// tests can paint any moment they like. `jump` is normalized heat.
#[derive(Clone, Copy)]
struct Motion {
    t: f32,
    run: f32,
    speed: f32,
    jump: f32,
}

impl SkyClock {
    /// Latch cues, then advance every phase — unless the world is paused,
    /// in which case only the latch happens and all motion holds.
    fn update(&mut self, dt: f32, sim: &Sim) {
        if sim.cues().iter().any(|cue| matches!(cue, Cue::Jump)) {
            self.jump = JUMP_LEN;
        }
        if sim.is_paused() {
            return;
        }
        self.jump = (self.jump - dt).max(0.0);
        let target = match sim.ship().state {
            ShipState::Traveling { .. } => {
                let base = if sim.is_warp() { WARP_FACTOR } else { 1.0 };
                if sim.stoked() {
                    base * STOKED_FACTOR
                } else {
                    base
                }
            }
            ShipState::Docked(_) => 0.0,
        };
        let ease = 1.0 - (-dt / SPEED_EASE).exp();
        self.speed = (target - self.speed).mul_add(ease, self.speed);
        self.t += dt;
        self.run = dt.mul_add(self.speed, self.run);
    }

    const fn motion(&self) -> Motion {
        Motion {
            t: self.t,
            run: self.run,
            speed: self.speed,
            jump: self.jump / JUMP_LEN,
        }
    }
}

/// The viewport plugin: hang the window once the rig stands, then repaint
/// the sky from the sim every view frame.
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyClock>()
            .add_systems(PostStartup, spawn)
            .add_systems(Update, repaint.in_set(Phase::View));
    }
}

/// A blank glass texture the canvas will overwrite every frame — the same
/// recipe as the CRT screens, because it is the same plumbing.
fn screen_image(canvas: &Canvas) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: canvas.w,
            height: canvas.h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// Ready the sky glass: the painted texture and its canvas. No quad, no
/// frame, no rivets — the window is CARGO (`Kind::Window`), and its
/// piece rig hangs the glass wherever the player rehangs it. A ship
/// whose window was sold has a painted sky and nothing to show it
/// through, which is legal, foolish, and the crew's own doing.
fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let canvas = Canvas::new(VIEW);
    let image = images.add(screen_image(&canvas));
    commands.insert_resource(Pane { image, canvas });
}

/// Advance the sky clocks, repaint the pane, push texels to the GPU.
fn repaint(
    time: Res<Time>,
    shell: Res<Shell>,
    mut clock: ResMut<SkyClock>,
    pane: Option<ResMut<Pane>>,
    mut images: ResMut<Assets<Image>>,
) {
    let sim = &shell.bridge.sim;
    clock.update(time.delta_secs(), sim);
    let Some(mut pane) = pane else {
        return;
    };
    let pane = &mut *pane;
    pane.canvas.mood(sim.light(), sim.omen());
    paint_sky(&mut pane.canvas, sim, clock.motion());
    if let Some(mut image) = images.get_mut(&pane.image)
        && let Some(data) = image.data.as_mut()
    {
        data.copy_from_slice(&pane.canvas.px);
    }
}

// ------------------------------------------------------------------- sky --

/// One frame of window: void, stars, whatever the leg has for us, company,
/// the jump flood, and the glass's own finish. No scanlines, no vignette —
/// that is the difference between a window and a tube.
fn paint_sky(cv: &mut Canvas, sim: &Sim, m: Motion) {
    cv.fill(VIEW, ink(palette::VOID));
    starfield(cv, m);
    match sim.ship().state {
        ShipState::Docked(at) => berth_flank(cv, at, m),
        ShipState::Traveling {
            from,
            to,
            progress,
            leg_ticks,
        } => {
            let frac = progress as f32 / leg_ticks.max(1) as f32;
            if frac < HANDOFF {
                origin_astern(cv, from, frac / HANDOFF, m.t);
            } else {
                destination_ahead(cv, to, (frac - HANDOFF) / (1.0 - HANDOFF), m.t);
            }
        }
    }
    whale_alongside(cv, sim, m);
    meteor_shower(cv, sim, m);
    grand_parade(cv, sim, m);
    ad_drone(cv, sim, m);
    if m.jump > 0.0 {
        cv.fill(
            VIEW,
            fade(ink(palette::EERIE_BRIGHT), m.jump * m.jump * 0.95),
        );
    }
    glass_finish(cv);
}

/// The deep field: ~90 hashed stars in two size classes, glint-white with
/// the odd faraway POI tint. They drift on parallax even when parked, and
/// stream into streaks as the smoothed speed climbs — the streak rides +x
/// while the stars run −x, tails trailing their motion.
fn starfield(cv: &mut Canvas, m: Motion) {
    for i in 0..STAR_COUNT {
        let h = splitmix(STAR_SEED, i);
        let fx = (h & 0xFFFF) as f32 / 65_535.0;
        let fy = ((h >> 16) & 0xFFFF) as f32 / 65_535.0;
        let depth = (((h >> 32) & 0xFF) as f32 / 255.0).mul_add(0.65, 0.35);
        let phase = ((h >> 40) & 0xFF) as f32 / 255.0 * TAU;
        let warmth = ((h >> 48) & 0xF) as f32 / 15.0;
        let mut col = mix(ink(palette::ICON_LIT), ink(palette::GLINT), warmth);
        if (h >> 55).trailing_zeros() >= 2 {
            // A few stars carry a faint tint of somewhere you could go.
            let hue = ink(palette::poi_color(((h >> 57) % 12) as u8));
            col = mix(col, hue, 0.35);
        }
        let twinkle = 0.12 * m.t.mul_add(depth + 0.4, phase).sin();
        let alpha = (depth.mul_add(0.5, 0.22) + twinkle).clamp(0.06, 0.85);
        let travel = DRIFT.mul_add(m.t, STREAM * m.run) * depth;
        // Snapped to the texel grid: a half-covered star is an off star.
        let x = snap(fx.mul_add(VIEW_W, -travel).rem_euclid(VIEW_W));
        let y = snap(fy.mul_add(VIEW_H - 4.0, 2.0));
        let size = if (h >> 52).trailing_zeros() >= 3 {
            2.0 * PX
        } else {
            PX
        };
        let len = m.speed * depth * STREAK;
        if len > PX {
            cv.seg(
                SimVec2::new(x + size, size.mul_add(0.5, y)),
                SimVec2::new(x + size + len, size.mul_add(0.5, y)),
                size,
                fade(col, alpha * 0.4),
            );
        }
        cv.fill(Rect::new(x, y, size, size), fade(col, alpha));
    }
}

// -------------------------------------------------------- worlds outside --

/// A world seen with the naked eye: its identity hue plus one cheap
/// accent, no phosphor anywhere. The same body draws the berth, the
/// origin astern, and the destination ahead.
fn body(cv: &mut Canvas, id: PoiId, at: SimVec2, r: f32, t: f32) {
    let col = ink(palette::poi_color(id));
    match id {
        // Jupiter: two darker bands across the disc.
        3 => {
            cv.dot(at, r, col);
            for fy in [-0.30_f32, 0.24] {
                let chord = r * fy.mul_add(-fy, 1.0).sqrt() * 0.92;
                cv.oval(
                    at + SimVec2::new(0.0, fy * r),
                    chord,
                    (r * 0.11).max(1.5),
                    0.0,
                    dim(col, 0.6),
                );
            }
        }
        // Uranus: a thin tilted ring in its accent hue.
        4 => {
            cv.dot(at, r * 0.9, col);
            cv.oval_ring(
                at,
                r * 1.55,
                r * 0.42,
                18.0,
                (r * 0.06).max(1.2),
                fade(ink(palette::accent::URANUS_RING), 0.85),
            );
        }
        // Saturn: the broad graveyard ring.
        SATURN => {
            cv.dot(at, r * 0.85, col);
            cv.oval_ring(
                at,
                r * 1.65,
                r * 0.5,
                -16.0,
                (r * 0.10).max(1.6),
                ink(palette::accent::SATURN_RING),
            );
        }
        // The Guild: a heptagon, slowly turning, edges picked out.
        GUILD => {
            let rot = t * 2.0;
            cv.poly(at, 7, r, rot, col);
            cv.poly_ring(
                at,
                7,
                r,
                rot,
                (r * 0.05).max(1.2),
                ink(palette::accent::GUILD_EDGE),
            );
        }
        // The comet: icy head, tail thrown up and away.
        COMET => {
            let dir = SimVec2::new(0.80, -0.60);
            let perp = SimVec2::new(-dir.y, dir.x);
            for (spread, reach, alpha) in [(0.0, 2.8, 0.5), (0.4, 2.1, 0.3), (-0.4, 2.1, 0.3)] {
                let root = at + dir * (r * 0.3);
                cv.seg(
                    root,
                    root + (dir + perp * (spread * 0.4)) * (r * reach),
                    (r * 0.22).max(1.4),
                    fade(col, alpha),
                );
            }
            cv.dot(at, r * 0.75, col);
            cv.dot(at, r * 0.35, ink(palette::GLINT));
        }
        // ???: a diamond that is not entirely committed to existing.
        WANDERER => {
            let there = (t * 1.9).sin().mul_add(0.30, 0.62);
            cv.poly(at, 4, r, 45.0, fade(col, there));
            cv.poly_ring(
                at,
                4,
                r * 1.2,
                45.0,
                (r * 0.06).max(1.2),
                fade(col, (1.15 - there).min(1.0)),
            );
        }
        _ => cv.dot(at, r, col),
    }
}

/// Docked: the berth fills the left flank, bobbing gently, half-cropped by
/// the frame. Dust motes drift by; now and then one catches the light.
fn berth_flank(cv: &mut Canvas, at: PoiId, m: Motion) {
    let bob = (m.t * (BOB_HZ * TAU)).sin() * 3.0;
    body(cv, at, SimVec2::new(BERTH.x, BERTH.y + bob), BERTH_R, m.t);
    dust_and_glints(cv, m.t);
}

/// Near-field texture while parked: three one-texel motes on slow hashed
/// drifts, and a four-point sparkle every few seconds somewhere new.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn dust_and_glints(cv: &mut Canvas, t: f32) {
    for k in 0..3_u64 {
        let h = splitmix(DUST_SEED, k);
        let fx = (h & 0xFFFF) as f32 / 65_535.0;
        let fy = ((h >> 16) & 0xFFFF) as f32 / 65_535.0;
        let vx = (((h >> 32) & 0xFF) as f32 / 255.0).mul_add(2.0, 1.2);
        let vy = (((h >> 40) & 0xFF) as f32 / 255.0).mul_add(1.2, -0.6);
        let x = snap(vx.mul_add(t, fx * VIEW_W).rem_euclid(VIEW_W));
        let y = snap(vy.mul_add(t, fy * VIEW_H).rem_euclid(VIEW_H));
        cv.fill(Rect::new(x, y, PX, PX), fade(ink(palette::GLINT), 0.55));
    }
    let cycle = t / SPARK_EVERY;
    let ph = cycle.fract();
    if ph < 0.12 {
        let h = splitmix(SPARK_SEED, cycle as u64);
        let at = SimVec2::new(
            ((h & 0xFFFF) as f32 / 65_535.0).mul_add(VIEW_W - 60.0, 30.0),
            (((h >> 16) & 0xFFFF) as f32 / 65_535.0).mul_add(VIEW_H - 60.0, 30.0),
        );
        let flare = (ph * (std::f32::consts::PI / 0.12)).sin();
        cv.dot(at, 1.0, fade(ink(palette::GLINT), flare * 0.9));
        for arm in [SimVec2::new(5.0, 0.0), SimVec2::new(0.0, 5.0)] {
            cv.seg(
                at - arm,
                at + arm,
                1.0,
                fade(ink(palette::GLINT), flare * 0.45),
            );
        }
    }
}

/// Early leg: the origin slides for a lower corner and shrinks — cast off,
/// pulling away, gone by the handoff.
fn origin_astern(cv: &mut Canvas, from: PoiId, g: f32, t: f32) {
    let r = (1.0 - g).powi(2) * BERTH_R;
    if r < 1.5 {
        return;
    }
    let at = SimVec2::new(lerp(BERTH.x, -70.0, g), lerp(BERTH.y, VIEW_H + 90.0, g));
    body(cv, from, at, r, t);
}

/// Past the handoff: the destination grows ahead, cubed-in so the middle
/// of a long haul stays honestly empty, then the world arrives all at once.
fn destination_ahead(cv: &mut Canvas, to: PoiId, g: f32, t: f32) {
    let r = lerp(DEST_R0, DEST_R1, g.powi(3));
    body(cv, to, DEST_AT, r, t);
}

// ---------------------------------------------------------------- company --

/// The whale, vast and patient, crossing the whole pane over its
/// encounter and gone — silhouette in deep greens rising to a lit back,
/// the one green thing out a real window.
fn whale_alongside(cv: &mut Canvas, sim: &Sim, m: Motion) {
    let ShipState::Traveling { progress, .. } = sim.ship().state else {
        return;
    };
    let Some(enc) = sim.encounter() else {
        return;
    };
    if !enc.open() || enc.kind != EncounterKind::Whale {
        return;
    }
    let frac = progress.saturating_sub(enc.start) as f32 / (enc.end - enc.start).max(1) as f32;
    let at = SimVec2::new(
        lerp(-140.0, VIEW_W + 140.0, frac),
        (m.t * 0.35).sin().mul_add(10.0, VIEW_H * 0.64),
    );
    let hide = mix(ink(palette::PHOSPHOR_DIM), ink(palette::ICON_LIT), 0.30);
    let back = mix(ink(palette::PHOSPHOR_DIM), ink(palette::ICON_LIT), 0.65);
    cv.oval(at, 72.0, 22.0, -4.0, fade(hide, 0.85));
    cv.oval(
        at + SimVec2::new(-6.0, -9.0),
        58.0,
        10.0,
        -4.0,
        fade(back, 0.45),
    );
    let tail = at + SimVec2::new(-74.0, 2.0);
    let sway = (m.t * 0.8).sin() * 6.0;
    cv.tri(
        tail,
        tail + SimVec2::new(-26.0, sway - 16.0),
        tail + SimVec2::new(-20.0, 2.0),
        fade(hide, 0.8),
    );
    cv.tri(
        tail,
        tail + SimVec2::new(-24.0, sway.mul_add(0.5, 16.0)),
        tail + SimVec2::new(-20.0, -2.0),
        fade(hide, 0.8),
    );
    cv.tri(
        at + SimVec2::new(8.0, 10.0),
        at + SimVec2::new(28.0, 26.0),
        at + SimVec2::new(26.0, 8.0),
        fade(hide, 0.7),
    );
    cv.dot(
        at + SimVec2::new(52.0, -4.0),
        1.6,
        fade(ink(palette::GLINT), 0.9),
    );
}

/// Meteor shower: three staggered lanes, each throwing an occasional
/// glint streak diagonally down the pane.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn meteor_shower(cv: &mut Canvas, sim: &Sim, m: Motion) {
    let Some(enc) = sim.encounter() else {
        return;
    };
    if !enc.open() || enc.kind != EncounterKind::MeteorShower {
        return;
    }
    for lane in 0..3_u64 {
        let cyc = m.t.mul_add(1.0 / 2.6, lane as f32 * 0.37);
        let ph = cyc.fract();
        if ph > 0.22 {
            continue;
        }
        let f = ph / 0.22;
        let h = splitmix(METEOR_SEED.wrapping_add(lane), cyc as u64);
        let start = SimVec2::new(
            ((h & 0xFFFF) as f32 / 65_535.0).mul_add(VIEW_W + 200.0, -100.0),
            (((h >> 16) & 0xFFFF) as f32 / 65_535.0) * VIEW_H * 0.5,
        );
        let along = SimVec2::new(120.0, 84.0);
        let head = start + along * f;
        cv.seg(
            head,
            head - along * 0.35,
            1.0,
            fade(ink(palette::GLINT), (1.0 - f) * 0.85),
        );
        cv.dot(head, 1.0, fade(ink(palette::GLINT), 1.0 - f));
    }
}

/// The Grand Parade at distance: a short procession of small eerie
/// heptagons crossing high on the glass, explaining nothing.
fn grand_parade(cv: &mut Canvas, sim: &Sim, m: Motion) {
    let Some(frac) = sim.parade() else {
        return;
    };
    let lead_x = frac.mul_add(VIEW_W + 260.0, -130.0);
    for i in 0..5_u32 {
        let x = (i as f32).mul_add(-34.0, lead_x);
        if !(-20.0..=VIEW_W + 20.0).contains(&x) {
            continue;
        }
        let y =
            m.t.mul_add(1.3, i as f32 * 1.1)
                .sin()
                .mul_add(4.0, (i as f32).mul_add(7.0, VIEW_H * 0.20));
        let r = if i == 0 {
            10.0
        } else {
            (i as f32).mul_add(-0.8, 7.0)
        };
        let at = SimVec2::new(x, y);
        cv.poly(at, 7, r, m.t * 5.0, fade(ink(palette::EERIE), 0.55));
        cv.poly_ring(
            at,
            7,
            r,
            m.t * 5.0,
            1.0,
            fade(ink(palette::EERIE_BRIGHT), 0.8),
        );
    }
}

/// The ad drone, zipping across every ~7 seconds while attached: an amber
/// dot lugging a tiny glint billboard nobody can read at this range.
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn ad_drone(cv: &mut Canvas, sim: &Sim, m: Motion) {
    if !sim.advertising() {
        return;
    }
    let cycle = m.t / 7.0;
    let ph = cycle.fract();
    if ph > 0.18 {
        return;
    }
    let f = ph / 0.18;
    let h = splitmix(DRONE_SEED, cycle as u64);
    let lane = ((h & 0xFFFF) as f32 / 65_535.0).mul_add(VIEW_H - 160.0, 60.0);
    let at = SimVec2::new(
        lerp(-30.0, VIEW_W + 30.0, f),
        (f * 14.0).sin().mul_add(5.0, lane),
    );
    cv.dot(at, 2.5, ink(palette::AMBER));
    let board = Rect::new(at.x - 8.0, at.y - 15.0, 16.0, 9.0);
    cv.fill(board, fade(ink(palette::GLINT), 0.9));
    cv.seg(
        SimVec2::new(board.x + 3.0, board.y + 3.0),
        SimVec2::new(board.x + 13.0, board.y + 6.0),
        1.0,
        ink(palette::SHADOW),
    );
    cv.seg(
        at,
        SimVec2::new(at.x, board.y + board.h),
        1.0,
        fade(ink(palette::AMBER), 0.7),
    );
}

// ------------------------------------------------------------------ glass --

/// The window's own finish: a barely-there reflection gleam across the
/// top-right corner and a one-texel inner shade line where glass meets
/// frame. Deliberately not a screen finish — no scanlines, no vignette.
fn glass_finish(cv: &mut Canvas) {
    let a = SimVec2::new(VIEW_W * 0.58, -20.0);
    let b = SimVec2::new(VIEW_W + 20.0, VIEW_H * 0.42);
    cv.seg(a, b, 30.0, fade(ink(palette::GLASS), 0.03));
    let off = SimVec2::new(30.0, -22.0);
    cv.seg(a + off, b + off, 10.0, fade(ink(palette::GLINT), 0.02));
    cv.frame_thick(inflate(VIEW, -PX), 1.0, ink(palette::PLATE_SHADE));
}

// ------------------------------------------------------------------ tests --

#[cfg(test)]
#[allow(clippy::cast_sign_loss)] // test coordinates are positive
mod tests {
    use super::*;
    use crate::canvas::rect_center;
    use space_trucking::sim::{InputFrame, TICK_DT, layout};

    /// A motion sample at rest: phase zero, no streaming, no flood.
    const fn still() -> Motion {
        Motion {
            t: 0.0,
            run: 0.0,
            speed: 0.0,
            jump: 0.0,
        }
    }

    /// Texel RGBA at canvas coordinates.
    fn texel(cv: &Canvas, column: u32, row: u32) -> [u8; 4] {
        let at = ((row * cv.w + column) * 4) as usize;
        [cv.px[at], cv.px[at + 1], cv.px[at + 2], cv.px[at + 3]]
    }

    /// Summed RGB of one texel — the void backdrop reads ~39.
    fn glow(px: [u8; 4]) -> u32 {
        u32::from(px[0]) + u32::from(px[1]) + u32::from(px[2])
    }

    /// Texels meaningfully brighter than the void backdrop.
    fn lit(cv: &Canvas) -> usize {
        cv.px
            .chunks_exact(4)
            .filter(|px| glow([px[0], px[1], px[2], px[3]]) > 72)
            .count()
    }

    /// A press-and-hold input frame at a world position — the bench's
    /// `press_at`, verbatim.
    fn press_at(pos: SimVec2) -> InputFrame {
        InputFrame {
            pointer: pos,
            press: true,
            held: true,
            ..InputFrame::default()
        }
    }

    /// A sim freshly cast off: chart the first POI the dock allows, pull
    /// the launch lever, tick once.
    fn launched(seed: u64) -> Sim {
        let mut sim = Sim::new(seed);
        let ShipState::Docked(at) = sim.ship().state else {
            panic!("a fresh sim starts docked");
        };
        let target = (0..12_u8)
            .find(|&id| id != at && sim.poi_chartable(id))
            .expect("somewhere must be chartable from the starting dock");
        sim.advance(0.0, &press_at(sim.poi_pos(target)));
        assert_eq!(sim.ship().selected, Some(target), "map press should chart");
        sim.advance(0.0, &press_at(rect_center(layout::LAUNCH_LEVER)));
        sim.advance(TICK_DT, &InputFrame::default());
        assert!(
            matches!(sim.ship().state, ShipState::Traveling { .. }),
            "the lever should cast off"
        );
        sim
    }

    #[test]
    fn a_docked_berth_lights_the_glass() {
        let sim = Sim::new(7);
        assert!(
            matches!(sim.ship().state, ShipState::Docked(_)),
            "a fresh sim starts docked"
        );
        let mut full = Canvas::new(VIEW);
        full.mood(sim.light(), sim.omen());
        paint_sky(&mut full, &sim, still());
        // Empty space for comparison: backdrop, stars, and glass finish —
        // everything except the berth and its motes.
        let mut empty = Canvas::new(VIEW);
        empty.mood(sim.light(), sim.omen());
        empty.fill(VIEW, ink(palette::VOID));
        starfield(&mut empty, still());
        glass_finish(&mut empty);
        let (lit_full, lit_empty) = (lit(&full), lit(&empty));
        assert!(
            lit_full > lit_empty + 1500,
            "the berth should fill a flank: {lit_full} vs {lit_empty} lit texels"
        );
    }

    #[test]
    fn the_destination_grows_past_the_handoff() {
        let mut sim = launched(7);
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            unreachable!("launched() travels");
        };
        sim.fast_forward(leg_ticks / 2);
        let ShipState::Traveling {
            progress,
            leg_ticks,
            ..
        } = sim.ship().state
        else {
            panic!("half a leg should still be traveling");
        };
        let frac = progress as f32 / leg_ticks as f32;
        assert!(frac > 0.2, "midway should be past the handoff, got {frac}");
        let mut cv = Canvas::new(VIEW);
        cv.mood(sim.light(), sim.omen());
        paint_sky(&mut cv, &sim, still());
        let hit = texel(&cv, (DEST_AT.x / PX) as u32, (DEST_AT.y / PX) as u32);
        assert!(
            glow(hit) > 90,
            "the destination disc should light its texel, got {hit:?}"
        );
    }

    #[test]
    fn identical_inputs_paint_identical_glass() {
        let sim = launched(7);
        let m = Motion {
            t: 42.5,
            run: 6.25,
            speed: WARP_FACTOR,
            jump: 0.4,
        };
        let mut a = Canvas::new(VIEW);
        a.mood(sim.light(), sim.omen());
        paint_sky(&mut a, &sim, m);
        let mut b = Canvas::new(VIEW);
        b.mood(sim.light(), sim.omen());
        paint_sky(&mut b, &sim, m);
        assert_eq!(a.px, b.px, "the window must repaint byte-identically");
    }
}
