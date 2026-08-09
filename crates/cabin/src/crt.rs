//! The two CRT screens, painted the 2D way: a near-1:1 port of
//! `src/render.rs`'s star map (`draw_map`) and destination preview
//! (`draw_preview`) rasterized in software into `Image` textures every
//! frame, exactly as the 2D console painted its render target. The glass
//! quads on the [`Station::Map`] and [`Station::Console`] panels show the
//! textures as emissives, so the phosphor blooms in the room.
//!
//! The [`Canvas`] here is the 2D `Canvas` reborn as a scanline rasterizer:
//! draw calls speak sim world units, texels sit on the 2D prototype's own
//! crunch grid ([`PX`] world units per texel), edges are hard (a texel is
//! in or out by its centre), and every colour runs through the console
//! light level and omen cast — the whole palette dims as one, no element
//! opts out. Colours come from [`crate::palette`] roles; decoration phases
//! ride `Time::elapsed_secs`; feedback rides latched cue timers; the only
//! randomness is `splitmix`.

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use space_trucking::sim::layout::{self, Rect};
use space_trucking::sim::{
    COMET, Cue, EncounterKind, GUILD, PoiId, SUN, ShipState, Sim, Track, Vec2 as SimVec2,
    leg_endpoints, splitmix,
};

use crate::canvas::{
    Canvas, PX, Rgba, dim, fade, inflate, ink, mix, phosphorize, polar, rect_center, snap,
};
use crate::rig::Skin;
use crate::surface::{SimSurface, Station, VirtualPointer};
use crate::{Phase, Shell, palette};

// ------------------------------------------------------------ crunch grid --

/// silhouette); the preview is the "big screen" and keeps a richer tint.
const MAP_PH: f32 = 0.85;
const PREVIEW_PH: f32 = 0.35;

/// Seconds per sonar-sweep revolution. Ambient, not attention-seeking.
const SWEEP_PERIOD: f32 = 20.0;

/// Fixed render-side seed for the starfield hash; cosmetic only.
const STAR_SEED: u64 = 0x57A2_F1E1;
const STAR_COUNT: u64 = 110;

// ---- Feedback lengths: the 2D juice's clocks, verbatim.

/// Ring blip on the freshly selected destination.
const BLIP_LEN: f32 = 0.35;
/// Dock-ring pop after an arrival.
const POP_LEN: f32 = 0.40;
/// Violet flash on the Guild station as its hangar swallows a delivery.
const DELIVERED_LEN: f32 = 0.45;
/// Long dock-ring pulse after an offline catch-up arrival: the one tween
/// allowed past half a second, so a returning player cannot miss it.
const PULSE_LEN: f32 = 3.5;

// ---- The physical screens.

/// Emissive multiplier on the glass material; the texture already carries
/// the picture, this just pushes the phosphor into bloom.
const GLOW: f32 = 2.0;

/// Glass lift off the panel plane, metres.
const LIFT_GLASS: f32 = 0.003;

/// Bezel slab centre lift and thickness, metres.
const LIFT_FRAME: f32 = 0.006;
const FRAME_DEPTH: f32 = 0.016;

/// Bezel width in sim units, straddling the panel rect's edge — half over
/// the texture's outer ring (where the 2D drew its PLATE bezel), half out.
const BEZEL: f32 = 16.0;

// --------------------------------------------------------------- feedback --

/// Feedback latches: cues are only valid during their frame's View phase,
/// so the tween timers live here — lengths are the 2D juice's, verbatim.
#[derive(Resource, Default)]
struct Feedback {
    /// Boot handled: the catch-up dock pulse winds at most once from it.
    booted: bool,
    select: f32,
    pop: f32,
    delivered: f32,
    pulse: f32,
}

/// Normalized cooldown: 1.0 the instant a timer is wound, 0.0 once spent.
const fn heat(remaining: f32, length: f32) -> f32 {
    (remaining / length).clamp(0.0, 1.0)
}

impl Feedback {
    /// Cool every clock by `dt`, then wind whatever this frame calls for.
    fn update(&mut self, dt: f32, shell: &Shell) {
        for timer in [
            &mut self.select,
            &mut self.pop,
            &mut self.delivered,
            &mut self.pulse,
        ] {
            *timer = (*timer - dt).max(0.0);
        }
        if !self.booted {
            self.booted = true;
            if shell.bridge.arrived_while_away {
                self.pulse = PULSE_LEN;
            }
        }
        if shell.outcome.stall_arrived {
            self.pulse = PULSE_LEN;
        }
        for cue in shell.bridge.sim.cues() {
            match cue {
                Cue::Select => self.select = BLIP_LEN,
                Cue::Arrive => self.pop = POP_LEN,
                Cue::Delivered => self.delivered = DELIVERED_LEN,
                _ => {}
            }
        }
    }

    const fn select_blip(&self) -> f32 {
        heat(self.select, BLIP_LEN)
    }

    const fn dock_pop(&self) -> f32 {
        heat(self.pop, POP_LEN)
    }

    const fn delivered_flash(&self) -> f32 {
        heat(self.delivered, DELIVERED_LEN)
    }

    const fn dock_pulse(&self) -> f32 {
        heat(self.pulse, PULSE_LEN)
    }
}

// ----------------------------------------------------------------- plugin --

/// The map and preview handles plus their reusable paint buffers. The
/// handles are crate-visible: the chart tank and destination preview
/// PIECES show these same textures on their glass (instruments are
/// cargo; the paint stays here, the furniture rides the pieces).
#[derive(Resource)]
pub struct Screens {
    pub map: Handle<Image>,
    pub preview: Handle<Image>,
    map_canvas: Canvas,
    preview_canvas: Canvas,
}

/// The CRT plugin: spawn the physical screens once the rig stands, then
/// repaint both tubes from the sim every view frame.
pub struct CrtPlugin;

impl Plugin for CrtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Feedback>()
            .add_systems(PostStartup, spawn)
            .add_systems(Update, repaint.in_set(Phase::View));
    }
}

/// A blank screen texture the canvas will overwrite every frame.
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

/// Spawn both physical screens: a glass quad showing the painted texture
/// as an emissive over each mapped rect, inside a PLATE bezel frame.
fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
) {
    let find = |want: Station| {
        surfaces
            .iter()
            .find_map(|(station, surface)| (*station == want).then_some(*surface))
    };
    let Some(map) = find(Station::Map) else {
        return;
    };
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let map_canvas = Canvas::new(layout::MAP_PANEL);
    let preview_canvas = Canvas::new(layout::DEST_PREVIEW);
    let map_image = images.add(screen_image(&map_canvas));
    let preview_image = images.add(screen_image(&preview_canvas));
    spawn_screen(
        &mut commands,
        &mut materials,
        &skin,
        &quad,
        &map,
        layout::MAP_PANEL,
        &map_image,
    );
    // The destination preview no longer hangs on the console face: the
    // DestPreview PIECE is its glass now (instruments are cargo), fed
    // from the same painted texture through `Screens`.
    commands.insert_resource(Screens {
        map: map_image,
        preview: preview_image,
        map_canvas,
        preview_canvas,
    });
}

/// The tube-glass material: a painted texture as emissive over
/// near-black glass. The recipe is crate-visible because the instrument
/// pieces (chart tank, destination preview, window) wear the same glass
/// on their rigs.
pub fn tube_glass(
    materials: &mut Assets<StandardMaterial>,
    image: &Handle<Image>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: palette::SHADOW,
        emissive: LinearRgba::WHITE * GLOW,
        emissive_texture: Some(image.clone()),
        perceptual_roughness: 0.85,
        metallic: 0.0,
        ..default()
    })
}

/// One screen's furniture: the emissive glass quad sized to exactly cover
/// `rect` on the panel plane, and four PLATE bezel slabs straddling its
/// edges. The near-black base keeps the tube glass in the room; the
/// texture supplies the phosphor and bloom does the halo.
fn spawn_screen(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    quad: &Handle<Mesh>,
    surface: &SimSurface,
    rect: Rect,
    image: &Handle<Image>,
) {
    let normal = surface.normal();
    let center = rect_center(rect);
    let glass = tube_glass(materials, image);
    commands.spawn((
        Mesh3d(quad.clone()),
        MeshMaterial3d(glass),
        Transform::from_translation(surface.to_world(center) + normal * LIFT_GLASS)
            .with_rotation(surface.orientation())
            .with_scale(Vec3::new(
                rect.w * surface.scale_u(),
                rect.h * surface.scale_v(),
                1.0,
            )),
    ));
    for (at, size) in [
        (SimVec2::new(center.x, rect.y), (rect.w + BEZEL, BEZEL)),
        (
            SimVec2::new(center.x, rect.y + rect.h),
            (rect.w + BEZEL, BEZEL),
        ),
        (SimVec2::new(rect.x, center.y), (BEZEL, rect.h - BEZEL)),
        (
            SimVec2::new(rect.x + rect.w, center.y),
            (BEZEL, rect.h - BEZEL),
        ),
    ] {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(skin.plate.clone()),
            Transform::from_translation(surface.to_world(at) + normal * LIFT_FRAME)
                .with_rotation(surface.orientation())
                .with_scale(Vec3::new(
                    size.0 * surface.scale_u(),
                    size.1 * surface.scale_v(),
                    FRAME_DEPTH,
                )),
        ));
    }
}

/// Repaint both tubes from the sim and push the texels to the GPU.
fn repaint(
    time: Res<Time>,
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    mut feedback: ResMut<Feedback>,
    screens: Option<ResMut<Screens>>,
    mut images: ResMut<Assets<Image>>,
) {
    feedback.update(time.delta_secs(), &shell);
    let Some(mut screens) = screens else {
        return;
    };
    let screens = &mut *screens;
    let sim = &shell.bridge.sim;
    let t = time.elapsed_secs();
    screens.map_canvas.mood(sim.light(), sim.omen());
    screens.preview_canvas.mood(sim.light(), sim.omen());
    paint_map(&mut screens.map_canvas, sim, t, pointer.sim, &feedback);
    paint_preview(&mut screens.preview_canvas, sim, t);
    for (handle, canvas) in [
        (&screens.map, &screens.map_canvas),
        (&screens.preview, &screens.preview_canvas),
    ] {
        if let Some(mut image) = images.get_mut(handle)
            && let Some(data) = image.data.as_mut()
        {
            data.copy_from_slice(&canvas.px);
        }
    }
}

// -------------------------------------------------------------------- map --

/// One frame of the star map — `draw_map`'s order, verbatim: stars,
/// orbits, route and ship, POIs, parade, travel company, sweep, screen
/// finish, ad ticker.
fn paint_map(cv: &mut Canvas, sim: &Sim, t: f32, pointer: SimVec2, fb: &Feedback) {
    let glass = crt_face(cv, layout::MAP_PANEL);
    draw_starfield(cv, glass, t);
    draw_orbits(cv, sim, t);
    draw_route_and_ship(cv, sim, t);
    draw_pois(cv, sim, glass, t, pointer, fb);
    draw_parade(cv, sim, glass, t);
    draw_travel_company(cv, sim, glass, t);
    draw_sweep(cv, glass, t);
    finish_screen(cv, glass, sim.omen(), t);
    draw_ad_static(cv, sim, glass, t);
}

/// The tube face: SCREEN glass over the whole panel (the raised PLATE
/// bezel is physical now), with the 2D's one-texel inner bevel around the
/// glass inset. Returns the glass rect the picture draws inside.
fn crt_face(cv: &mut Canvas, panel: Rect) -> Rect {
    cv.fill(panel, ink(palette::SCREEN));
    let glass = inflate(panel, -4.0 * PX);
    bevel(
        cv,
        glass,
        ink(palette::PLATE_SHADE),
        fade(ink(palette::PLATE_LIT), 0.5),
    );
    glass
}

/// One-texel bevel edges: `top_left` along the edges facing the light,
/// `bottom_right` along the ones facing away.
fn bevel(cv: &mut Canvas, r: Rect, top_left: Rgba, bottom_right: Rgba) {
    cv.fill(Rect::new(r.x, r.y, r.w, PX), top_left);
    cv.fill(Rect::new(r.x, r.y, PX, r.h), top_left);
    cv.fill(Rect::new(r.x, r.y + r.h - PX, r.w, PX), bottom_right);
    cv.fill(Rect::new(r.x + r.w - PX, r.y, PX, r.h), bottom_right);
}

fn draw_starfield(cv: &mut Canvas, glass: Rect, t: f32) {
    for i in 0..STAR_COUNT {
        let hash = splitmix(STAR_SEED, i);
        let fx = (hash & 0xFFFF) as f32 / 65_535.0;
        let fy = ((hash >> 16) & 0xFFFF) as f32 / 65_535.0;
        let base = ((hash >> 32) & 0xFF) as f32 / 255.0;
        let phase = ((hash >> 40) & 0xFF) as f32 / 255.0 * TAU;
        let speed = (((hash >> 48) & 0x3) as f32).mul_add(0.6, 0.4);
        // Snapped to the texel grid: a half-covered star is an off star.
        let star_x = snap(fx.mul_add(3.0f32.mul_add(-PX, glass.w), glass.x + PX));
        let star_y = snap(fy.mul_add(3.0f32.mul_add(-PX, glass.h), glass.y + PX));
        let twinkle = 0.15 * speed.mul_add(t, phase).sin();
        let alpha = (base.mul_add(0.45, 0.2) + twinkle).clamp(0.05, 0.8);
        let size = if hash & 0x300 == 0 { 2.0 * PX } else { PX };
        cv.fill(
            Rect::new(star_x, star_y, size, size),
            fade(
                mix(ink(palette::PHOSPHOR_DIM), ink(palette::PHOSPHOR), base),
                alpha,
            ),
        );
    }
}

/// The sun, and the orbit each world rides. The comet's path stays
/// undrawn — a comet should feel like a visitor.
fn draw_orbits(cv: &mut Canvas, sim: &Sim, t: f32) {
    let flicker = (t * 3.0).sin().mul_add(0.06, 0.9);
    cv.dot(SUN, 7.0, fade(ink(palette::AMBER), 0.85 * flicker));
    cv.dot(SUN, 3.5, fade(ink(palette::GLINT), 0.9));
    cv.ring(SUN, 10.0, 1.0, fade(ink(palette::AMBER), 0.25 * flicker));
    for (i, poi) in sim.pois().iter().enumerate() {
        let Track::Circle { orbit, .. } = poi.track else {
            continue;
        };
        // The Umbra Market's little orbit stays secret in daylight.
        if !sim.poi_visible(i as PoiId) {
            continue;
        }
        cv.ring(SUN, orbit, 1.0, fade(ink(palette::PHOSPHOR_DIM), 0.4));
    }
}

/// The dashed course and the freighter nosing along it, engine flicker
/// bigger and faster under warp.
fn draw_route_and_ship(cv: &mut Canvas, sim: &Sim, t: f32) {
    let ShipState::Traveling {
        from,
        to,
        progress,
        leg_ticks,
    } = sim.ship().state
    else {
        return;
    };
    // The charted line: cast-off point to where the destination will be
    // at the arrival tick — the same derivation the sim flies.
    let (a, b) = leg_endpoints(from, to, progress, leg_ticks, sim.tick());
    let span = b - a;
    let length = span.length();
    if length <= f32::EPSILON {
        return;
    }
    let dir = span * length.recip();

    let step = 13.0;
    let dash = 6.0;
    let count = ((length / step) as i32).max(0);
    for i in 0..count {
        let along = (i as f32) * step;
        cv.seg(
            a + dir * along,
            a + dir * (along + dash).min(length),
            1.0,
            fade(ink(palette::PHOSPHOR), 0.45),
        );
    }

    let pos = sim.ship().interpolated(sim.alpha());
    let perp = SimVec2::new(-dir.y, dir.x);
    let (freq, boost): (f32, f32) = if sim.is_warp() {
        (26.0, 1.9)
    } else {
        (9.0, 1.0)
    };
    let flick = 0.3f32.mul_add((t * freq).sin(), 0.7);
    let tail = (3.5 * boost).mul_add(flick, 3.0);
    let stern = pos - dir * 4.0;
    cv.tri(
        stern + perp * 2.2,
        stern - perp * 2.2,
        stern - dir * tail,
        fade(ink(palette::PHOSPHOR), 0.75),
    );
    cv.tri(
        pos + dir * 8.0,
        stern + perp * 4.5,
        stern - perp * 4.5,
        ink(palette::PHOSPHOR_HOT),
    );
}

/// Every visible POI: glyph, then its state dressing in the 2D's order —
/// transit bar, sweep afterglow, spent-comet haze, hover invite, armed
/// selection, the Guild's delivery bloom, and the dock rings.
#[allow(clippy::too_many_lines)] // the 2D state ladder, kept in one place
fn draw_pois(cv: &mut Canvas, sim: &Sim, glass: Rect, t: f32, pointer: SimVec2, fb: &Feedback) {
    let sweep = sweep_angle(t);
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
        poi_glyph(cv, i, pos, poi.radius, t, MAP_PH);

        // Inner-to-inner courses check transit papers at charting time:
        // docked at one inner world without a chit, its rivals wear a
        // barred ring — visible, not chartable.
        if docked.is_some() && sim.inner_ring_locked(id) {
            let radius = poi.radius + 4.0;
            cv.ring(pos, radius, 1.2, fade(ink(palette::LAMP_NO), 0.55));
            let bar = SimVec2::new(radius * 0.8, -radius * 0.8);
            cv.seg(pos - bar, pos + bar, 1.4, fade(ink(palette::LAMP_NO), 0.55));
        }

        // The sweep's afterglow brightens a POI it just passed.
        let glow = sweep_glow(center, sweep, pos);
        if glow > 0.0 {
            cv.ring(
                pos,
                poi.radius + PX,
                1.0,
                fade(ink(palette::PHOSPHOR), 0.30 * glow),
            );
        }

        // A comet already picked clean this pass sits under a haze.
        if id == COMET && sim.comet_spent() {
            cv.dot(pos, poi.radius, fade(ink(palette::SCREEN), 0.45));
        }

        // Hover: a faint ring over any POI the player could actually
        // chart right now — the same predicate the press consults.
        if let Some(at) = docked
            && id != at
            && sim.poi_chartable(id)
            && (pointer - pos).length() <= poi.radius
        {
            cv.ring(
                pos,
                poi.radius + 3.0,
                1.0,
                fade(ink(palette::PHOSPHOR), 0.3),
            );
        }

        // Selected: a breathing ring, plus a blip right when picked.
        if sim.ship().selected == Some(id) {
            let wave = (t * 4.0).sin();
            cv.ring(
                pos,
                wave.mul_add(1.5, poi.radius + 5.0),
                1.4,
                fade(ink(palette::PHOSPHOR), wave.mul_add(0.15, 0.7)),
            );
            let blip = fb.select_blip();
            if blip > 0.0 {
                cv.ring(
                    pos,
                    (1.0 - blip).mul_add(9.0, poi.radius + 4.0),
                    1.2,
                    fade(ink(palette::PHOSPHOR), blip * 0.8),
                );
            }
        }

        // The hangar swallow: as `Delivered` fires, the Guild station
        // blooms violet for a moment — hangar business, not a phosphor
        // reading, the same licence the omen cast holds.
        if id == GUILD {
            let flash = fb.delivered_flash();
            if flash > 0.0 {
                cv.dot(
                    pos,
                    poi.radius + 2.0,
                    fade(ink(palette::EERIE), 0.30 * flash),
                );
                cv.ring(
                    pos,
                    (1.0 - flash).mul_add(10.0, poi.radius + 3.0),
                    1.4,
                    fade(ink(palette::EERIE_BRIGHT), 0.7 * flash),
                );
            }
        }

        // Docked: a solid ring; pop on arrival; a long pulse after an
        // offline catch-up so the returning player spots where they are.
        if docked == Some(id) {
            cv.ring(
                pos,
                poi.radius + 4.0,
                1.6,
                fade(ink(palette::PHOSPHOR), 0.85),
            );
            let pop = fb.dock_pop();
            if pop > 0.0 {
                cv.ring(
                    pos,
                    (1.0 - pop).mul_add(12.0, poi.radius + 4.0),
                    1.5,
                    fade(ink(palette::PHOSPHOR), pop * 0.9),
                );
            }
            let pulse = fb.dock_pulse();
            if pulse > 0.0 {
                let wave = (t * 5.0).sin();
                cv.ring(
                    pos,
                    wave.mul_add(2.5, poi.radius + 7.0),
                    1.5,
                    fade(
                        ink(palette::PHOSPHOR_HOT),
                        wave.mul_add(0.25, 0.45) * (pulse * 3.0).min(1.0),
                    ),
                );
            }
        }
    }
}

// ------------------------------------------------------------- POI glyphs --

/// One hand-drawn glyph per POI index, shared by the map and the preview.
/// `ph` is how far its colours lean toward phosphor: the CRT shows a
/// *reading* of Venus, not Venus.
fn poi_glyph(cv: &mut Canvas, id: usize, pos: SimVec2, r: f32, t: f32, ph: f32) {
    match id {
        0 => venus_glyph(cv, pos, r, t, ph),
        1 => earth_glyph(cv, pos, r, ph),
        2 => mars_glyph(cv, pos, r, ph),
        3 => jupiter_glyph(cv, pos, r, ph),
        4 => uranus_glyph(cv, pos, r, ph),
        5 => neptune_glyph(cv, pos, r, ph),
        7 => saturn_glyph(cv, pos, r, ph),
        8 => umbra_glyph(cv, pos, r, ph),
        9 => hermitage_glyph(cv, pos, r, t, ph),
        10 => comet_glyph(cv, pos, r, ph),
        11 => wanderer_glyph(cv, pos, r, t, ph),
        _ => guild_glyph(cv, pos, r, t, ph),
    }
}

/// Venus: pink-gold, with a glittery halo the rich paid extra for.
fn venus_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, t: f32, ph: f32) {
    cv.dot(pos, r, phosphorize(ink(palette::POI_VENUS), ph));
    cv.ring(
        pos,
        r * 1.45,
        1.0,
        fade(phosphorize(ink(palette::accent::VENUS_HALO), ph), 0.5),
    );
    for i in 0..3_u8 {
        let angle = f32::from(i).mul_add(2.1, t * 0.5);
        let sparkle = polar(pos, r * 1.45, angle);
        let glint = (f32::from(i).mul_add(1.7, t * 3.0)).sin();
        cv.dot(
            sparkle,
            r.mul_add(0.10, 0.6),
            fade(
                phosphorize(ink(palette::GLINT), ph),
                glint.mul_add(0.3, 0.65),
            ),
        );
    }
}

/// Earth: blue-gray under a brown smog ring.
fn earth_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r, phosphorize(ink(palette::POI_EARTH), ph));
    cv.arc(
        pos,
        r * 1.1,
        -160.0,
        240.0,
        (r * 0.16).max(1.2),
        fade(phosphorize(ink(palette::accent::SMOG), ph), 0.85),
    );
}

/// Mars: rust-red with a patchwork wedge of the scrappy republic.
fn mars_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r, phosphorize(ink(palette::POI_MARS), ph));
    cv.tri(
        pos,
        polar(pos, r * 0.96, -0.7),
        polar(pos, r * 0.96, 0.45),
        phosphorize(ink(palette::accent::MARS_PATCH), ph),
    );
}

/// Jupiter: a banded orange ellipse.
fn jupiter_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    let rx = r * 1.12;
    let ry = r * 0.82;
    let col = phosphorize(ink(palette::POI_JUPITER), ph);
    cv.oval(pos, rx, ry, 0.0, col);
    for (fy, shade) in [(-0.42_f32, 0.78), (0.05, 0.66), (0.5, 0.8)] {
        let chord = rx * fy.mul_add(-fy, 1.0_f32).sqrt() * 0.94;
        cv.oval(
            pos + SimVec2::new(0.0, fy * ry),
            chord,
            ry * 0.12,
            0.0,
            dim(col, shade),
        );
    }
}

/// Uranus: pale cyan with a thin tilted ring.
fn uranus_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r * 0.92, phosphorize(ink(palette::POI_URANUS), ph));
    cv.oval_ring(
        pos,
        r * 1.7,
        r * 0.5,
        20.0,
        1.0,
        fade(phosphorize(ink(palette::accent::URANUS_RING), ph), 0.7),
    );
}

/// Neptune: deep blue with a faint pale streak of storm.
fn neptune_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r, phosphorize(ink(palette::POI_NEPTUNE), ph));
    cv.seg(
        pos + SimVec2::new(-r * 0.5, -r * 0.3),
        pos + SimVec2::new(r * 0.6, -r * 0.15),
        (r * 0.16).max(1.0),
        fade(phosphorize(ink(palette::GLINT), ph), 0.5),
    );
}

/// The Guild Station: a gray-violet heptagon, pulsing like it knows
/// things. Seven sides; the officially published schematics show six.
fn guild_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, t: f32, ph: f32) {
    let pulse = (t * 2.4).sin().mul_add(0.05, 1.0);
    let rot = t * 4.0;
    cv.poly(
        pos,
        7,
        r * pulse,
        rot,
        phosphorize(ink(palette::POI_GUILD), ph),
    );
    cv.poly_ring(
        pos,
        7,
        r * pulse,
        rot,
        1.0,
        phosphorize(ink(palette::accent::GUILD_EDGE), ph),
    );
}

/// Saturn: pale gold under a broad debris ring — the ring-barons'
/// graveyard, a thousand failed hauling companies ground to gravel.
fn saturn_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r * 0.85, phosphorize(ink(palette::POI_SATURN), ph));
    cv.oval_ring(
        pos,
        r * 1.8,
        r * 0.55,
        -18.0,
        1.4,
        phosphorize(ink(palette::accent::SATURN_RING), ph),
    );
    // Gravel in the ring: three coarse grains along its long axis.
    for f in [-1.35_f32, -0.6, 1.1] {
        let tilt = (-18.0_f32).to_radians();
        let at = pos + SimVec2::new(tilt.cos(), tilt.sin()) * (r * f);
        cv.dot(at, 1.2, phosphorize(ink(palette::accent::SATURN_RING), ph));
    }
}

/// The Umbra Market: a crescent sliver in Mercury's shadow. Only drawn at
/// all while somebody's clock reads deep night.
fn umbra_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    cv.dot(pos, r, phosphorize(ink(palette::POI_UMBRA), ph));
    // The shadowed face: a screen-dark bite that leaves a crescent.
    cv.dot(
        pos + SimVec2::new(r * 0.45, -r * 0.2),
        r * 0.85,
        ink(palette::SCREEN),
    );
    cv.dot(
        pos + SimVec2::new(-r * 0.45, r * 0.3),
        1.3,
        phosphorize(ink(palette::GLINT), ph),
    );
}

/// The Hermitage: a lumpy rock in the belt with one warm lit window.
fn hermitage_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, t: f32, ph: f32) {
    cv.poly(
        pos,
        5,
        r,
        23.0,
        phosphorize(ink(palette::POI_HERMITAGE), ph),
    );
    cv.poly_ring(
        pos,
        5,
        r,
        23.0,
        1.0,
        phosphorize(dim(ink(palette::POI_HERMITAGE), 0.35), ph),
    );
    // The window: a lamp that breathes very slowly. Hermits keep odd hours.
    let warmth = (t * 0.7).sin().mul_add(0.2, 0.75);
    cv.dot(
        pos + SimVec2::new(r * 0.3, -r * 0.15),
        1.6,
        fade(ink(palette::AMBER), warmth),
    );
}

/// The comet: an icy head with its tail thrown away from the sun. The
/// tail rides a capped radius so the preview's enlarged glyph keeps its
/// plume on the glass.
fn comet_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, ph: f32) {
    let away = pos - SUN;
    let len = away.length().max(f32::EPSILON);
    let dir = away * len.recip();
    let perp = SimVec2::new(-dir.y, dir.x);
    let stretch = r.min(12.0);
    for (spread, reach, alpha) in [(0.0, 3.2, 0.6), (0.5, 2.4, 0.4), (-0.5, 2.4, 0.4)] {
        cv.seg(
            pos + dir * r * 0.4,
            pos + dir * r * 0.4 + (dir + perp * spread * 0.3) * stretch * reach,
            1.2,
            fade(phosphorize(ink(palette::POI_COMET), ph), alpha),
        );
    }
    cv.dot(pos, r * 0.75, phosphorize(ink(palette::POI_COMET), ph));
    cv.dot(
        pos + dir * -0.2 * r,
        r * 0.35,
        phosphorize(ink(palette::GLINT), ph),
    );
}

/// ???: a diamond that is not entirely committed to existing.
fn wanderer_glyph(cv: &mut Canvas, pos: SimVec2, r: f32, t: f32, ph: f32) {
    let there = (t * 6.3).sin().mul_add(0.25, 0.65);
    cv.poly_ring(
        pos,
        4,
        r,
        45.0,
        1.4,
        fade(phosphorize(ink(palette::POI_WANDERER), ph), there),
    );
    cv.poly_ring(
        pos,
        4,
        r * 0.55,
        45.0,
        1.0,
        fade(phosphorize(ink(palette::POI_WANDERER), ph), 1.0 - there),
    );
    cv.dot(pos, 1.5, fade(phosphorize(ink(palette::GLINT), ph), there));
}

// ------------------------------------------------------------- decoration --

/// The Grand Parade: once the hangar counter fills, a procession of
/// heptagons crosses the whole sky, once, slowly, and does not explain.
fn draw_parade(cv: &mut Canvas, sim: &Sim, glass: Rect, t: f32) {
    let Some(frac) = sim.parade() else {
        return;
    };
    let across = frac.mul_add(glass.w + 120.0, glass.x - 60.0);
    let lead = SimVec2::new(across, frac.mul_add(-60.0, glass.h.mul_add(0.4, glass.y)));
    for i in 0..5_u32 {
        let back = lead - SimVec2::new(22.0f32.mul_add(i as f32, 26.0), (i as f32) * -6.0);
        let radius = if i == 0 { 14.0 } else { 7.0 - i as f32 };
        if back.x < glass.x + 8.0 || back.x > glass.x + glass.w - 8.0 {
            continue;
        }
        let pulse = t.mul_add(2.0, i as f32).sin().mul_add(0.08, 0.8);
        cv.poly(
            back,
            7,
            radius.max(2.5),
            t * 6.0,
            fade(ink(palette::EERIE), 0.5 * pulse),
        );
        cv.poly_ring(
            back,
            7,
            radius.max(2.5),
            t * 6.0,
            1.0,
            fade(ink(palette::EERIE_BRIGHT), 0.8 * pulse),
        );
    }
}

/// Whatever is alongside mid-leg: the whale's spectacle on the glass and
/// the ad drone's tiny billboard. (The flotsam net wells are console
/// furniture in 3D, not a map reading.)
#[allow(clippy::cast_sign_loss)] // cosmetic clocks are non-negative
fn draw_travel_company(cv: &mut Canvas, sim: &Sim, glass: Rect, t: f32) {
    if !matches!(sim.ship().state, ShipState::Traveling { .. }) {
        return;
    }

    // The whale itself, vast and patient, crossing under the sky.
    if let Some(enc) = sim.encounter()
        && enc.open()
        && enc.kind == EncounterKind::Whale
        && let ShipState::Traveling { progress, .. } = sim.ship().state
    {
        let frac =
            (progress.saturating_sub(enc.start)) as f32 / (enc.end - enc.start).max(1) as f32;
        let at = SimVec2::new(
            frac.mul_add(glass.w * 0.8, glass.w.mul_add(0.1, glass.x)),
            (t * 0.5).sin().mul_add(4.0, glass.h.mul_add(0.78, glass.y)),
        );
        cv.oval(at, 26.0, 7.0, -4.0, fade(ink(palette::PHOSPHOR_DIM), 0.5));
        let tail = at + SimVec2::new(-30.0, 0.0);
        cv.tri(
            tail,
            tail + SimVec2::new(-8.0, -7.0),
            tail + SimVec2::new(-8.0, 7.0),
            fade(ink(palette::PHOSPHOR_DIM), 0.5),
        );
        cv.dot(
            at + SimVec2::new(18.0, -2.0),
            1.2,
            fade(ink(palette::PHOSPHOR), 0.8),
        );
    }

    // The ad drone: a tiny billboard on a tight orbit, wobbling per swat.
    if let Some(at) = sim.drone_pos() {
        let wobble = f32::from(sim.drone_swats()) * (t * 21.0).sin() * 2.0;
        let at = at + SimVec2::new(wobble, 0.0);
        cv.dot(at, 2.2, fade(ink(palette::AMBER), 0.9));
        let board = Rect::new(at.x - 7.0, at.y - 12.0, 14.0, 8.0);
        cv.fill(board, fade(ink(palette::GLINT), 0.85));
        for (i, ch) in [3_u8, 11, 7].into_iter().enumerate() {
            ad_rune(
                cv,
                ch.wrapping_add((t * 2.0) as u8),
                SimVec2::new((i as f32).mul_add(4.4, board.x + 2.0), board.y + 1.5),
                4.0,
                ink(palette::SHADOW),
            );
        }
        cv.seg(
            at,
            at + SimVec2::new(0.0, -4.0),
            1.0,
            fade(ink(palette::AMBER), 0.7),
        );
    }
}

/// One rune of the ad tongue: an angular scrawl derived from the byte, in
/// a language nobody aboard reads.
fn ad_rune(cv: &mut Canvas, ch: u8, at: SimVec2, size: f32, col: Rgba) {
    let h = splitmix(0xAD_51_11, u64::from(ch));
    let point = |n: u64| {
        SimVec2::new(
            (((h >> n) % 4) as f32 / 3.0).mul_add(size, at.x),
            (((h >> (n + 8)) % 5) as f32 / 4.0 * size).mul_add(1.4, at.y),
        )
    };
    let mut prev = point(0);
    for leg_bits in [16_u64, 32, 48] {
        let next = point(leg_bits);
        cv.seg(prev, next, 1.0, col);
        prev = next;
    }
}

/// The ad ticker: while the drone is attached, every screen in the shop
/// runs its incomprehensible pitch. Swat the drone to make it stop.
fn draw_ad_static(cv: &mut Canvas, sim: &Sim, glass: Rect, t: f32) {
    if !sim.advertising() {
        return;
    }
    // The pitch, in lojban, transliterated into the rune alphabet. It
    // says something like "buy the great shining thing"; nobody asked.
    let pitch = b"ko te vecnu lo banli je carmi dacti .i e'osai";
    let band = Rect::new(glass.x, glass.y + 6.0, glass.w, 12.0);
    cv.fill(band, fade(ink(palette::SHADOW), 0.55));
    let step = 9.0;
    let scroll = (t * 30.0) % (pitch.len() as f32 * step);
    for (i, &ch) in pitch.iter().enumerate() {
        let x = (i as f32).mul_add(step, band.x + band.w - scroll);
        if x < band.x + 2.0 || x > band.x + band.w - step {
            continue;
        }
        ad_rune(
            cv,
            ch,
            SimVec2::new(x, band.y + 2.0),
            6.0,
            fade(ink(palette::AMBER), 0.85),
        );
    }
}

// ------------------------------------------------------------------ sweep --

/// Sweep bearing at cosmetic time `t`, radians.
fn sweep_angle(t: f32) -> f32 {
    (t * (TAU / SWEEP_PERIOD)).rem_euclid(TAU)
}

/// How lit `pos` still is by the sweep that last passed it, `0..=1` —
/// a slow phosphor afterglow trailing the beam.
fn sweep_glow(center: SimVec2, angle: f32, pos: SimVec2) -> f32 {
    let d = pos - center;
    let behind = (angle - d.y.atan2(d.x)).rem_euclid(TAU);
    (behind / 1.7).mul_add(-1.0, 1.0).max(0.0)
}

/// One slow sonar line about the map centre with a short fading trail.
fn draw_sweep(cv: &mut Canvas, glass: Rect, t: f32) {
    let center = rect_center(glass);
    let radius = glass.w.min(glass.h).mul_add(0.5, -2.0 * PX);
    let angle = sweep_angle(t);
    for k in 0..5_u8 {
        let a0 = f32::from(k).mul_add(-0.04, angle);
        let alpha = f32::from(k).mul_add(-0.012, 0.065);
        cv.tri(
            center,
            polar(center, radius, a0),
            polar(center, radius, a0 - 0.04),
            fade(ink(palette::PHOSPHOR), alpha),
        );
    }
    cv.seg(
        center,
        polar(center, radius, angle),
        1.0,
        fade(ink(palette::PHOSPHOR), 0.28),
    );
    cv.dot(center, PX, fade(ink(palette::PHOSPHOR), 0.5));
}

// ----------------------------------------------------------- screen finish --

/// Scanlines, vignette, and the omen flicker — drawn last inside a screen
/// so the picture shows through them.
fn finish_screen(cv: &mut Canvas, glass: Rect, omen: f32, t: f32) {
    scanlines(cv, glass);
    screen_vignette(cv, glass);
    if omen > 0.0 {
        let flicker = (t * 9.0).sin().mul_add(0.5, 0.5);
        cv.fill(glass, fade(ink(palette::SHADOW), omen * 0.12 * flicker));
    }
}

/// Every other texel row, very low alpha.
fn scanlines(cv: &mut Canvas, r: Rect) {
    let step = 2.0 * PX;
    let y0 = (r.y / step).ceil() * step;
    let rows = ((r.y + r.h - y0 - PX) / step) as i32;
    let shade = fade(dim(ink(palette::PHOSPHOR_DIM), 0.3), 0.4);
    for i in 0..=rows.max(0) {
        let y = (i as f32).mul_add(step, y0);
        cv.fill(Rect::new(r.x, y, r.w, PX), shade);
    }
}

/// The corners darken: the map is a tube, not a viewport.
fn screen_vignette(cv: &mut Canvas, r: Rect) {
    let s = r.w.min(r.h) * 0.18;
    let corners = [
        (SimVec2::new(r.x, r.y), 1.0, 1.0),
        (SimVec2::new(r.x + r.w, r.y), -1.0, 1.0),
        (SimVec2::new(r.x, r.y + r.h), 1.0, -1.0),
        (SimVec2::new(r.x + r.w, r.y + r.h), -1.0, -1.0),
    ];
    for (corner, along_x, along_y) in corners {
        for reach in [s, s * 0.55] {
            cv.tri(
                corner,
                corner + SimVec2::new(along_x * reach, 0.0),
                corner + SimVec2::new(0.0, along_y * reach),
                fade(ink(palette::SHADOW), 0.12),
            );
        }
    }
    cv.frame_thick(r, 1.0, fade(ink(palette::SHADOW), 0.30));
}

// ---------------------------------------------------------------- preview --

/// Destination preview: the console's second CRT, showing where you are
/// going — or, dimmed, where you already are.
fn paint_preview(cv: &mut Canvas, sim: &Sim, t: f32) {
    let glass = crt_face(cv, layout::DEST_PREVIEW);
    preview_grid(cv, glass);
    let (id, bright) = match sim.ship().state {
        ShipState::Traveling { to, .. } => (to, true),
        ShipState::Docked(at) => sim.ship().selected.map_or((at, false), |sel| (sel, true)),
    };
    // Idle shows the tube's green *reading* of where you already are; the
    // richer enamel tint is reserved for an armed destination, so the
    // screen visibly wakes up when there is somewhere to go.
    let ph = if bright { PREVIEW_PH } else { MAP_PH };
    poi_glyph(cv, usize::from(id), rect_center(glass), 48.0, t, ph);
    if !bright {
        cv.fill(glass, fade(ink(palette::SHADOW), 0.35));
    }
    finish_screen(cv, glass, sim.omen(), t);
    draw_ad_static(cv, sim, glass, t);
}

/// Faint alignment grid on the preview glass: the tube is powered even
/// with nothing armed.
fn preview_grid(cv: &mut Canvas, glass: Rect) {
    let step = 12.0 * PX;
    let shade = fade(ink(palette::PHOSPHOR_DIM), 0.40);
    let x0 = (glass.x / step).ceil() * step;
    for i in 0..=((glass.x + glass.w - x0) / step) as i32 {
        cv.fill(
            Rect::new((i as f32).mul_add(step, x0), glass.y, PX, glass.h),
            shade,
        );
    }
    let y0 = (glass.y / step).ceil() * step;
    for i in 0..=((glass.y + glass.h - y0) / step) as i32 {
        cv.fill(
            Rect::new(glass.x, (i as f32).mul_add(step, y0), glass.w, PX),
            shade,
        );
    }
}

// ------------------------------------------------------------------ tests --

#[cfg(test)]
#[allow(clippy::cast_sign_loss)] // test coordinates are positive
mod tests {
    use super::*;
    use crate::canvas::lerp;

    /// Texel RGBA at canvas coordinates.
    fn texel(cv: &Canvas, column: u32, row: u32) -> [u8; 4] {
        let at = ((row * cv.w + column) * 4) as usize;
        [cv.px[at], cv.px[at + 1], cv.px[at + 2], cv.px[at + 3]]
    }

    #[test]
    fn a_dot_lands_where_aimed() {
        let mut cv = Canvas::new(layout::MAP_PANEL);
        cv.fill(layout::MAP_PANEL, ink(palette::SCREEN));
        cv.dot(SimVec2::new(100.0, 100.0), 3.0, ink(palette::PHOSPHOR));
        // Sim (100, 100) is texel (45, 45): (100 - panel origin 10) / PX.
        let hit = texel(&cv, 45, 45);
        assert!(hit[1] > 150, "expected phosphor green, got {hit:?}");
        let miss = texel(&cv, 60, 45);
        assert!(
            miss[1] < 40,
            "far texel should stay dark glass, got {miss:?}"
        );
    }

    #[test]
    fn blending_is_src_over() {
        let mut cv = Canvas::new(layout::DEST_PREVIEW);
        cv.fill(layout::DEST_PREVIEW, ink(palette::SCREEN));
        cv.fill(layout::DEST_PREVIEW, fade(ink(palette::GLINT), 0.5));
        let got = texel(&cv, 10, 10);
        let under = ink(palette::SCREEN);
        let over = ink(palette::GLINT);
        let want = [
            lerp(under.r, over.r, 0.5),
            lerp(under.g, over.g, 0.5),
            lerp(under.b, over.b, 0.5),
        ];
        for (have, want) in got.iter().take(3).zip(want) {
            let have = f32::from(*have) / 255.0;
            assert!(
                (have - want).abs() < 0.02,
                "src-over should halve toward glint: {have} vs {want}"
            );
        }
        assert_eq!(got[3], 255, "opaque base stays opaque");
    }

    #[test]
    fn strokes_survive_the_crunch() {
        let mut cv = Canvas::new(layout::MAP_PANEL);
        // A hairline ring must still quantize up to a whole-texel stroke.
        cv.ring(
            SimVec2::new(260.0, 220.0),
            20.0,
            0.2,
            ink(palette::PHOSPHOR),
        );
        let row = ((220.0_f32 - 10.0) / PX) as u32;
        let lit = (0..cv.w).any(|column| texel(&cv, column, row)[1] > 100);
        assert!(lit, "the ring's row should contain lit texels");
    }

    #[test]
    fn the_map_lights_the_docked_station() {
        let sim = Sim::new(7);
        let ShipState::Docked(at) = sim.ship().state else {
            panic!("a fresh sim starts docked");
        };
        let mut cv = Canvas::new(layout::MAP_PANEL);
        cv.mood(sim.light(), sim.omen());
        paint_map(
            &mut cv,
            &sim,
            0.0,
            crate::bridge::POINTER_PARKED,
            &Feedback::default(),
        );
        let pos = sim.poi_pos(at);
        let column = ((pos.x - layout::MAP_PANEL.x) / PX) as u32;
        let row = ((pos.y - layout::MAP_PANEL.y) / PX) as u32;
        let hit = texel(&cv, column, row);
        let glass = ink(palette::SCREEN);
        assert!(
            f32::from(hit[1]) / 255.0 > glass.g + 0.15,
            "the docked station should read on the glass, got {hit:?}"
        );
    }
}
