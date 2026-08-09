//! Cargo pieces and the rat, made physical: every [`Piece`] the sim knows
//! becomes a low-poly rig — hold pieces at furniture scale in the walkable
//! bay (rows 0–2 hung on the aft wall band, row 3 standing on the deck,
//! the fold-straddling 1×2 kinds rising across both), everything else a
//! scale model on the barter counter: the broker's diorama, the deliberate
//! scale conceit `docs/BAY.md` records. Plus the carried piece riding the
//! crosshair, per-cell placement hints, drop-target invitations, the
//! hard-reject flash with its rule glyphs, and the stowaway.
//!
//! Semantics keep the retired 2D console's law: the sim stays the only
//! arbiter — footprints come from `layout::piece_rect` and `cubby_rect`,
//! legality from `placement_check`, invites from `drop_targets` — and no
//! refusal rides on hue alone: illegality always carries a slash, gnawing
//! carries a wedge, shapes over colors.
//!
//! The fixture kinds go further, per `docs/FIXTURES.md`: every lamp rig
//! owns a real `PointLight` gated by the sim's `lamp_lit` and dimmed by
//! the omen through `rig::Dimmable`, seedlings bloom in `lit_adjacent`
//! lamplight, paintings carry one seeded artwork painted through the
//! shared `canvas`, a couch under the rat settles it into a nap pose, and
//! the cabinet is furniture that stores: an open-fronted wardrobe whose
//! 2×2 cubby rack renders its `Loc::Stow` cargo in miniature.
//!
//! The dressing layer (`docs/BAY.md`) adds a fourth regime: a covering
//! rig owns two bodies — laid flat into the bay surface versus rolled or
//! canned everywhere else — and the sim's berth class picks which one
//! shows, with luminous coats waking and dimming exactly as lamp glass
//! does.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use space_trucking::sim::cargo::CABINET_SLOTS;
use space_trucking::sim::layout::{self, Rect, Surf};
use space_trucking::sim::{
    Cue, Kind, Loc, Mount, Piece, ShipState, Vec2 as SimVec2, Violation, lamp_lit, lit_adjacent,
    placement_check, player_owned, splitmix,
};

use crate::rig::{Dimmable, Skin};
use crate::surface::{SimSurface, Station, VirtualPointer};
use crate::{Phase, Shell, canvas, glow, palette};

/// How long a piece takes to glide to a new berth, seconds.
const EASE_LEN: f32 = 0.15;

/// Scale-settle (1.1 → 1.0) after `Cue::Place`, seconds.
const SETTLE_LEN: f32 = 0.18;

/// Violation flash length — the 2D juice's clock, kept.
const FLASH_LEN: f32 = 0.45;

/// How far a carried piece hovers off the struck surface, meters.
const CARRY_LIFT: f32 = 0.05;

/// Where the carried piece floats while the crosshair aims at nothing
/// placeable: ahead of and below the eye, nudged off center, like a box
/// hitched on one arm — carried, never dropped, and never a blindfold.
/// Compacted to a fraction of berth scale so the room stays visible
/// through a couch-sized carry (the occlusion defect class, BAY.md);
/// the aim-anchored hover keeps full scale because it stands at the
/// *destination*, not at the face.
const CARRY_AHEAD: f32 = 0.5;
const CARRY_DOWN: f32 = 0.40;
const CARRY_SIDE: f32 = 0.15;
const CARRY_COMPACT: f32 = 0.45;

/// Extra reach of the focus x-ray test past a piece's half-diagonal,
/// metres: catches a rig's depth off its berth plane.
const XRAY_MARGIN: f32 = 0.10;

/// The x-ray outline's lamp level: present, legible, unmistakably not a
/// legality ruling (those are the carry's green/red at full glow).
const XRAY_GLOW: f32 = 0.35;

/// Fraction of its rect a desk rig fills, so tray neighbours never touch.
const FIT: f32 = 0.88;

/// Fraction of its cells a bay rig fills. Roomier than the desk's [`FIT`]:
/// furniture nearly fills its berth — a couch reads ~1.06 world units
/// wide over its two 0.55 cells.
const BAY_FIT: f32 = 0.96;

/// A stowed piece's scale relative to its host cabinet's: shrunk until
/// the widest 1×1 rig (~34 sim units across) reads ~0.18 world units —
/// small enough to sit visibly *inside* a cubby, doors or no doors.
const STOW_FIT: f32 = 0.34;

/// The rat's per-sim-unit scale relative to the bay's. Nose to tail the
/// rig spans ~17 sim units, so this reads ~0.12 world units of ship rat.
const RAT_FIT: f32 = 0.45;

/// Rat hop tween length in ticks (0.35 s), same as the 2D renderer.
const RAT_HOP_TICKS: f32 = 21.0;

/// Salt for emissive pulse phases, off every sim stream.
const SALT_PULSE: u64 = 0x91EC_E501;

/// Salt for the bite wedge's spin.
const SALT_BITE: u64 = 0x91EC_B17E;

/// Salt for the painting's one artwork roll.
const SALT_ART: u64 = 0x91EC_0A27;

/// A lit wall or floor lamp's honest brightness; the omen scales it
/// via `Dimmable`. Since the ship owns no light of its own (lights are
/// cargo; BAY.md), lamplight is most of what a room ever gets.
const LAMP_LUMENS: f32 = 36_000.0;

/// A wall or floor lamp's reach, metres: a local pool, about one bay
/// cell past its own.
const LAMP_RANGE: f32 = 1.9;

/// The ceiling lamp is the room's key light — the one the ship starts
/// with — so it burns brighter and reaches the whole floor from its
/// pendant height instead of pooling.
const CEILING_LUMENS: f32 = 120_000.0;
const CEILING_RANGE: f32 = 4.6;

/// Lamp wake/sleep fade, seconds. Placement feedback, so it finishes
/// well inside the half-second law.
const LAMP_WAKE: f32 = 0.3;

/// How far a laid covering sits proud of its chart, metres: a rung of
/// the decal ladder (`rig::layer`), over the tiles and the doormat,
/// still well under anything standing on the same cells.
const LAID_LIFT: f32 = crate::rig::layer::LAID;

/// A laid rug's pile, metres — the one covering with real body; the
/// paints are coats, millimetres of enamel.
const RUG_THICK: f32 = 0.012;

/// The hint quads' rung on the decal ladder (`rig::layer`): above the
/// dressing layer's thickest covering (a laid rug's pile), so a hint
/// over a rug reads instead of burning underneath it.
const OVERLAY_LIFT: f32 = crate::rig::layer::HINT;

/// A laid luminous coat's honest brightness: a tinge on the neighbours
/// the sim already counts lit, an order under [`LAMP_LUMENS`] — ambiance,
/// not gallery lighting; bloom does the halo.
const COAT_LUMENS: f32 = 6_000.0;

/// The coat light's reach, metres: about one bay cell.
const COAT_RANGE: f32 = 1.2;

/// The coat quad's emissive ceiling as a `glow::set_lamp` level: paint
/// that glows, never a lamp pretending to be flat.
const COAT_GLOW: f32 = 0.45;

/// The painting's little canvas in sim units: 24×16 texels at the
/// shared rasterizer's own density.
const ART_W: f32 = 48.0;
const ART_H: f32 = 32.0;

/// The artwork's emissive multiplier: barely a glow. Paint, not a screen.
const ART_GLOW: f32 = 0.5;

/// The cabinet carcass's depth in sim units (~0.2 world at bay scale):
/// slim enough to read wardrobe, deep enough to shelve a shrunken rig.
const CABINET_DEPTH: f32 = 14.0;

/// Violation glyph arm length, sim units — the 2D console's `s = 12`.
const GLYPH_S: f32 = 12.0;

/// Most bars any one violation glyph spends; the pool is this deep.
const GLYPH_BARS: u8 = 5;

pub struct PiecesPlugin;

impl Plugin for PiecesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PieceIndex>()
            .init_resource::<HeldMemo>()
            .init_resource::<PendingSettle>()
            .init_resource::<CarryState>()
            .init_resource::<FlashState>()
            .init_resource::<RatState>()
            .add_systems(PostStartup, spawn_overlays)
            .add_systems(
                Update,
                (
                    latch_cues,
                    sync_pieces,
                    sync_fixtures,
                    sync_dressings,
                    xray_focus,
                    hover_glint,
                    carry_held,
                    placement_hints,
                    invite_glows,
                    violation_flash,
                    rat_watch,
                    breathe_pulses,
                    eta_needles,
                )
                    .chain()
                    .in_set(Phase::View),
            );
    }
}

// -------------------------------------------------------------- bookkeeping --

/// Piece id → rig root entity, the diffing map for [`sync_pieces`].
#[derive(Resource, Default)]
struct PieceIndex(HashMap<u32, Entity>);

/// Last frame's held piece. `Place` and `Reject` fire after the sim already
/// let go, so the cues need yesterday's grip to know what was carried —
/// the same trick the 2D juice's `held_was` plays.
#[derive(Resource, Default)]
struct HeldMemo(Option<(u32, Kind)>);

/// A `Cue::Place` waiting for [`sync_pieces`] to wind the settle on its rig.
#[derive(Resource, Default)]
struct PendingSettle(Option<u32>);

/// The carry: which piece rides the pointer, and where it last hovered so a
/// parked pointer keeps the piece in hand instead of dropping it visually.
#[derive(Resource, Default)]
struct CarryState {
    carrying: Option<u32>,
    last: Option<(Vec3, Quat)>,
}

/// The hard-reject flash: the refused footprint in sim coordinates, the
/// rule that refused it (for the glyph and the suspicious violet), and
/// how long the frame keeps burning.
#[derive(Resource, Default)]
struct FlashState {
    left: f32,
    area: Option<Rect>,
    rule: Option<Violation>,
}

/// The stowaway's entity and the way its nose points.
#[derive(Resource, Default)]
struct RatState {
    entity: Option<Entity>,
    yaw: f32,
}

/// One spawned piece rig: its sim identity, the eased transform tween, and
/// the child entities the view systems toggle.
#[derive(Component)]
struct PieceRig {
    from: Vec3,
    goal: Vec3,
    rot_from: Quat,
    rot_goal: Quat,
    scale_from: Vec3,
    scale_goal: Vec3,
    /// Seconds since the last berth change, saturating at [`EASE_LEN`].
    ease: f32,
    /// Seconds of settle left after a `Cue::Place`.
    settle: f32,
    gnawed_shown: bool,
    bite: Entity,
    /// Every visible part of the piece itself lives under this child, so
    /// the focus x-ray can drop the body wholesale while the frame stays.
    body_root: Entity,
    frame_root: Entity,
    slash: Entity,
    frame_mat: Handle<StandardMaterial>,
}

/// Marker: this rig is ghosted by the focus x-ray — body hidden, its
/// footprint frame lit dim as the "something stands here" outline.
#[derive(Component)]
struct XRayed;

/// A decoration emissive that breathes on the idle clock (the suspicious
/// hum, the very-mysterious chord). Own material instance, always.
#[derive(Component)]
struct Pulse {
    color: Color,
    base: f32,
    amp: f32,
    freq: f32,
    phase: f32,
}

/// One hold-cell hint quad, with its refusal slash alongside.
#[derive(Component)]
struct HintCell {
    x: u8,
    y: u8,
    slash: Entity,
}

/// Which invitation a barter overlay quad answers. The rail row sits on
/// `SHELF_SLOTS == FLOTSAM_SLOTS`: shelf re-slotting while a barter is
/// open, the outboard net while none is — one set of quads, two meanings,
/// never both (the layouts are exclusive by design).
#[derive(Clone, Copy)]
enum Row {
    Rail,
    Give,
    Take,
    Received,
}

/// A drop-target glow quad on the barter counter.
#[derive(Component)]
struct RowGlow {
    row: Row,
    phase: f32,
}

/// One edge bar of the violation flash frame, `0..8`: bars 0–3 frame the
/// footprint's share of the wall band, 4–7 its share of the deck strip —
/// a refused footprint may straddle the fold.
#[derive(Component)]
struct VioBar(u8);

/// One bar of the violation glyph pool, `0..GLYPH_BARS` — the 2D
/// console's per-rule icons (weight, bracket, hazard, snowflake, and the
/// cabinet's full box) restated as emissive hardware over the flash.
#[derive(Component)]
struct GlyphBar(u8);

/// One inviting glow quad in a cabinet cubby's mouth, keyed by the host
/// piece and its slot; wakes while the sim's drop matrix invites a stow.
#[derive(Component)]
struct CubbyGlow {
    piece: u32,
    slot: u8,
    phase: f32,
}

/// The ETA gauge piece's needle: `reach` is the pivot-to-centre arm in
/// rig-local sim units; [`eta_needles`] sweeps it with the live leg.
#[derive(Component)]
struct EtaNeedle {
    reach: f32,
}

/// The rat rig's root.
#[derive(Component)]
struct RatRoot;

/// The rat's tail, remembering its resting pose so the sway composes.
#[derive(Component)]
struct RatTail {
    base: Quat,
}

/// A lamp rig's living parts: `level` eases lit/dark over [`LAMP_WAKE`]
/// seconds and feeds both the point light's [`Dimmable`] base — fx.rs
/// keeps the per-frame omen math — and the bulb glass, which is its own
/// material instance per the shared-handle rule. Lamps burn only while
/// the sim's `lamp_lit` says so: berthed in the hold, nowhere else — a
/// lamp on the counter or boxed in a cubby is dark glass.
#[derive(Component)]
struct LampGlow {
    piece: u32,
    color: Color,
    mat: Handle<StandardMaterial>,
    /// Eased lit level, `0..=1`.
    level: f32,
}

/// The wall lamp's bracket sub-root. Built reaching +X (the right
/// stile); when the piece's footprint sits on wall column 0 a π turn
/// about Z mirrors it left — every part sits at local y = 0, so the
/// turn is a clean side flip, not an upside-down sconce.
#[derive(Component)]
struct WallArm {
    piece: u32,
}

/// One blossom on a Seedlings rig, visible only while some footprint
/// cell sits in lamplight (`lit_adjacent`) — presentation only, the 3D
/// reading of the 2D bloom.
#[derive(Component)]
struct Blossom {
    piece: u32,
}

/// One of a covering rig's two bodies: the laid coat or rug versus the
/// rolled/canned shelf form. [`sync_dressings`] shows exactly one, by
/// the sim's berth class — laid only while the piece lies `Loc::Laid`
/// with no hand on it; a carried covering rides packed, roll or tin.
#[derive(Component)]
struct DressForm {
    piece: u32,
    laid: bool,
}

/// A laid luminous coat's living glow: `level` eases with the berth
/// class the way lamp bulbs ease with `lamp_lit`, feeding the same two
/// sinks — the quad's own emissive (its own material instance, per the
/// shared-handle rule) and a faint real point light whose [`Dimmable`]
/// base the omen dims through fx.rs, no special case. Canned, the tin
/// is blacked out and the level falls to dark.
#[derive(Component)]
struct CoatGlow {
    piece: u32,
    color: Color,
    mat: Handle<StandardMaterial>,
    /// Eased laid level, `0..=1`.
    level: f32,
}

/// Handles shared by the overlay systems: the static refusal-slash
/// phosphor, the one violation-flash material every frame bar burns
/// through, and the glyph pool's ink.
#[derive(Resource)]
struct SharedBits {
    slash: Handle<StandardMaterial>,
    flash: Handle<StandardMaterial>,
    glyph: Handle<StandardMaterial>,
}

// ------------------------------------------------------------------ helpers --

/// The panel with `want`'s station tag, if the rig spawned it.
fn surface_of(surfaces: &Query<(&Station, &SimSurface)>, want: Station) -> Option<SimSurface> {
    surfaces
        .iter()
        .find(|(station, _)| **station == want)
        .map(|(_, surface)| *surface)
}

/// The net chart a sim point reads through, as its cabin station and
/// mapped surface — the old wall-band/deck-strip pair generalized to
/// the room net's six charts. `None` off the net or on a hole.
fn chart_of(
    surfaces: &Query<(&Station, &SimSurface)>,
    sim: SimVec2,
) -> Option<(Station, SimSurface)> {
    let (x, y) = layout::cell_at(sim)?;
    let station = match layout::surface_of(x, y)? {
        Surf::Aft => Station::BayWall,
        Surf::Floor => Station::BayFloor,
        Surf::Port => Station::BayPort,
        Surf::Starboard => Station::BayStarboard,
        Surf::Front => Station::BayFront,
        Surf::Ceiling => Station::BayCeiling,
    };
    Some((station, surface_of(surfaces, station)?))
}

/// Where a hold footprint (its `layout::piece_rect`) sits in the room,
/// as the rig root's (translation, rotation, scale). On the floor chart
/// a rig STANDS: feet on its plan rect, upright, turned by the backing
/// rule ([`floor_facing`]) and keeping its bas-relief height (the
/// re-authored 3D extents are deferred; BAY.md). On a wall or the
/// ceiling it hangs flat against the chart. Sizes derive from the
/// surface scales, so retuning `rig::BAY_CELL` re-scales every rig.
fn net_site(surfaces: &Query<(&Station, &SimSurface)>, rect: Rect) -> Option<(Vec3, Quat, Vec3)> {
    let (station, surface) = chart_of(surfaces, rect_center(rect))?;
    let aft = surface_of(surfaces, Station::BayWall)?;
    Some(site_on(station, &surface, &aft, rect))
}

/// The backing rule (BAY.md): where a standing rig's orientation cannot
/// be read off its cells, its footprint decides. A footprint against a
/// wall seam turns its BACK to that wall (the couch against the wall);
/// mid-floor it faces the front of the room, toward the user. Only the
/// yaws that keep the visual plan on its logical cells are candidates:
/// a half turn is always compatible, quarter turns only for one-column
/// footprints (a 2-wide couch cannot lie along the port wall without
/// leaving its cells). Seam priority aft, front, then the sides —
/// stable, and the always-compatible flips first.
fn floor_facing(surface: &SimSurface, aft: &SimSurface, rect: Rect) -> Quat {
    let base = Station::BayWall.face(aft);
    let (fx, fy, fw, fh) = layout::FLOOR;
    // Clamped non-negative before the cast: rects live on the net.
    #[allow(clippy::cast_sign_loss)]
    let cell = |units: f32| (units / layout::CELL).round().max(0.0) as u8;
    let cx = cell(rect.x - layout::GRID_ORIGIN.x);
    let cy = cell(rect.y - layout::GRID_ORIGIN.y);
    let cw = cell(rect.w).max(1);
    let ch = cell(rect.h).max(1);
    // Sim axes on the floor chart, in world: `u` port -> starboard,
    // `v` aft -> front (the floor's y3 row lies at the aft seam).
    let u = surface.half_u.normalize();
    let v = surface.half_v.normalize();
    let want = if cy == fy {
        v
    } else if cy + ch == fy + fh {
        -v
    } else if cx == fx && cw == 1 {
        u
    } else if cx + cw == fx + fw && cw == 1 {
        -u
    } else {
        v
    };
    let ahead = base * Vec3::Z;
    let yaw = ahead.cross(want).dot(Vec3::Y).atan2(ahead.dot(want));
    Quat::from_rotation_y(yaw) * base
}

/// [`net_site`]'s pure core, for a known chart — shared with the unit
/// tests, which build charts straight from `rig::bay()`.
fn site_on(
    station: Station,
    surface: &SimSurface,
    aft: &SimSurface,
    rect: Rect,
) -> (Vec3, Quat, Vec3) {
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    let scale = Vec3::new(su, sv, su.min(sv)) * BAY_FIT;
    match station {
        Station::BayFloor => {
            let base = surface.to_world(rect_center(rect));
            (
                base + Vec3::Y * (rect.h * 0.5 * scale.y),
                floor_facing(surface, aft, rect),
                scale,
            )
        }
        // Ceiling cargo hangs PENDANT: upright like a floor rig, author
        // up staying world up — the lamp's cord meets the ceiling and
        // its shade swings below — rather than pasted flat against the
        // plane, which is what the playtest called out.
        Station::BayCeiling => {
            let base = surface.to_world(rect_center(rect));
            (
                base - Vec3::Y * (rect.h * 0.5 * scale.y),
                Station::BayWall.face(aft),
                scale,
            )
        }
        _ => (
            surface.to_world(rect_center(rect)),
            wall_upright(station, surface, rect),
            scale,
        ),
    }
}

/// The upright rule for wall cargo: the side charts' columns run up the
/// wall (the seam law pins them), so a rig hung there inherits a
/// quarter turn — the playtest's sideways star chart. A SQUARE
/// footprint rolls back upright about the wall normal (facing is
/// untouched); a non-square footprint's cells genuinely lie that way
/// — portrait on a side wall — so it keeps the chart's orientation
/// rather than pull its body off its cells. Aft and front, whose
/// columns already run level, compute a zero roll and pass through.
fn wall_upright(station: Station, surface: &SimSurface, rect: Rect) -> Quat {
    let base = station.face(surface);
    if !station.chart_flipped() {
        return base;
    }
    let square = ((rect.w - rect.h) / layout::CELL).abs() < 0.5;
    if !square {
        return base;
    }
    let n = station.inward(surface);
    let up = base * Vec3::Y;
    let want = (Vec3::Y - n * Vec3::Y.dot(n)).normalize_or_zero();
    if want.length_squared() < 0.5 {
        return base;
    }
    let roll = up.cross(want).dot(n).atan2(up.dot(want));
    Quat::from_axis_angle(n, roll) * base
}

/// Where a laid footprint lies: flat AGAINST its chart, lifted
/// [`LAID_LIFT`] proud of the quad so the coat clears the socket plates
/// yet stays under everything standing on the same cells. No
/// [`BAY_FIT`] margin — a covering covers; its own geometry insets
/// where the berth edge should still read.
fn net_laid(surfaces: &Query<(&Station, &SimSurface)>, rect: Rect) -> Option<(Vec3, Quat, Vec3)> {
    let (station, surface) = chart_of(surfaces, rect_center(rect))?;
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    Some((
        surface.to_world(rect_center(rect)) + station.inward(&surface) * LAID_LIFT,
        station.face(&surface),
        Vec3::new(su, sv, su.min(sv)),
    ))
}

/// Cubby anchor centres in the cabinet rig's local space, sim units.
/// Slot order matches `layout::cubby_rect`: row-major from the top-left
/// facing the open front — local +X is sim +x, local +Y is up, so slot 0
/// sits up-left of the rig's centre. `z` recesses the cargo into the
/// carcass so it reads shelved, not stuck on.
fn cubby_anchor(slot: u8) -> Vec3 {
    let sx = if slot.is_multiple_of(2) { -1.0 } else { 1.0 };
    let sy = if slot / 2 == 0 { 1.0 } else { -1.0 };
    Vec3::new(sx * layout::CELL * 0.22, sy * layout::CELL * 0.47, 3.4)
}

/// The berth transform for a piece: the bay for hold cargo, flat into
/// the bay surface for laid dressings, a cubby anchor inside the host's
/// standing rig for stowed cargo, the desk mapping for everything on
/// the counter. `None` only for a stow whose
/// cabinet is missing — impossible by the sim's rules; the caller hides
/// the rig rather than guess.
fn berth_site(
    pieces: &[Piece],
    piece: &Piece,
    surfaces: &Query<(&Station, &SimSurface)>,
) -> Option<(Vec3, Quat, Vec3)> {
    match piece.loc {
        Loc::Hold { .. } => net_site(surfaces, layout::piece_rect(pieces, piece)),
        Loc::Laid { .. } => net_laid(surfaces, layout::piece_rect(pieces, piece)),
        Loc::Flotsam { slot } => {
            // Rail cargo stands on its hopper tile at bay scale, facing
            // the doorway — staged for the fire, not shelved.
            let (pos, rot) = crate::airlock::site(slot);
            let chart = surface_of(surfaces, Station::BayFloor)?;
            let s = chart.scale_u().min(chart.scale_v());
            let scale = Vec3::splat(s) * BAY_FIT;
            let (_, h) = piece.kind.cells();
            let lift = f32::from(h) * layout::CELL * 0.5 * scale.y;
            Some((pos + Vec3::Y * lift, rot, scale))
        }
        Loc::Stow { cabinet, slot } => {
            // An occupied cabinet cannot leave the hold, so the host is a
            // standing floor rig whenever this berth exists at all.
            let host = pieces
                .iter()
                .find(|other| other.id == cabinet && matches!(other.loc, Loc::Hold { .. }))?;
            let (pos, rot, scale) = net_site(surfaces, layout::piece_rect(pieces, host))?;
            Some((
                pos + rot * (cubby_anchor(slot) * scale),
                rot,
                Vec3::splat(scale.min_element() * STOW_FIT),
            ))
        }
        _ => {
            let barter = surface_of(surfaces, Station::Barter)?;
            let rect = layout::piece_rect(pieces, piece);
            let (w, h) = piece.kind.cells();
            let fw = f32::from(w) * layout::CELL;
            let fh = f32::from(h) * layout::CELL;
            // Slot rects are smaller than big footprints; fit like the 2D
            // glyph box, aspect kept.
            let fit = (rect.w / fw).min(rect.h / fh) * FIT;
            let (su, sv) = (barter.scale_u(), barter.scale_v());
            Some((
                barter.to_world(rect_center(rect)),
                barter.orientation(),
                Vec3::new(su, sv, su.min(sv)) * fit,
            ))
        }
    }
}

/// Cubic ease-out, the module's one easing curve.
fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t;
    u.mul_add(-u * u, 1.0)
}

/// Centre of a sim rect.
const fn rect_center(rect: Rect) -> SimVec2 {
    SimVec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y))
}

/// A deterministic decoration phase from a piece id, `0..TAU`.
const fn phase_of(id: u32, salt: u64) -> f32 {
    (splitmix(id as u64, salt) % 1000) as f32 / 1000.0 * TAU
}

/// Where the rat sits in a hold cell: low and left of centre, matching the
/// 2D perch so it stays out of the cargo silhouettes.
fn perch((x, y): (u8, u8)) -> SimVec2 {
    let cell = layout::cell_rect(x, y);
    SimVec2::new(cell.w.mul_add(0.42, cell.x), cell.h.mul_add(0.74, cell.y))
}

/// A chunky low-poly sphere.
fn ico(radius: f32) -> Mesh {
    Sphere::new(radius)
        .mesh()
        .ico(1)
        .expect("icosphere subdivisions in range")
}

// ----------------------------------------------------------------- overlays --

/// Pre-spawn everything that waits dark for a sim state to light it: a
/// hint quad per bay cell (rows 0–2 on the wall band, row 3 on the deck
/// strip) with its refusal slash, the barter row glow quads, the
/// violation frame bars (four per bay surface — a refused footprint may
/// straddle the fold), and the glyph bar pool.
fn spawn_overlays(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
) {
    let slash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 3.0);
    let flash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 0.0);
    let glyph_mat = glow::phosphor(&mut materials, palette::GLINT, 0.0);
    commands.insert_resource(SharedBits {
        slash: slash_mat.clone(),
        flash: flash_mat.clone(),
        glyph: glyph_mat.clone(),
    });
    let Some(barter) = surface_of(&surfaces, Station::Barter) else {
        return;
    };

    // Net cell hints: a thin quad per cell on whichever chart holds it,
    // its refusal slash floating just above (shape channel — illegality
    // never rides hue alone). The socket plates themselves are rig
    // furniture; these are the glow layer over them — lifted past
    // [`OVERLAY_LIFT`] so a hint over a laid rug burns over the pile,
    // not inside it. Holes get no hint; nothing can land there.
    for y in 0..layout::GRID_ROWS {
        for x in 0..layout::GRID_COLS {
            let cell = layout::cell_rect(x, y);
            let Some((station, surface)) = chart_of(&surfaces, rect_center(cell)) else {
                continue;
            };
            let (su, sv) = (surface.scale_u(), surface.scale_v());
            let rot = station.face(&surface);
            let normal = station.inward(&surface);
            let center = surface.to_world(rect_center(cell));
            let slash = commands
                .spawn((
                    Mesh3d(skin.cube.clone()),
                    MeshMaterial3d(slash_mat.clone()),
                    Transform::from_translation(center + normal * crate::rig::layer::SLASH)
                        .with_rotation(rot * Quat::from_rotation_z((cell.h / cell.w).atan()))
                        .with_scale(Vec3::new(
                            cell.w.hypot(cell.h) * 0.82 * su,
                            2.6 * sv,
                            0.0015,
                        )),
                    Visibility::Hidden,
                ))
                .id();
            let mat = glow::phosphor(&mut materials, palette::LAMP_OK, 0.0);
            commands.spawn((
                Mesh3d(skin.cube.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(center + normal * OVERLAY_LIFT)
                    .with_rotation(rot)
                    .with_scale(Vec3::new((cell.w - 4.0) * su, (cell.h - 4.0) * sv, 0.0015)),
                Visibility::Hidden,
                HintCell { x, y, slash },
            ));
        }
    }

    // Barter row glows. FLOTSAM_SLOTS == SHELF_SLOTS, so the Rail quads
    // serve both the station shelf and the outboard net.
    let (bu, bv) = (barter.scale_u(), barter.scale_v());
    let brot = barter.orientation();
    let bnormal = barter.normal();
    for (row, slots) in [
        (Row::Rail, &layout::SHELF_SLOTS),
        (Row::Received, &layout::RECEIVED_SLOTS),
        (Row::Give, &layout::GIVE_SLOTS),
        (Row::Take, &layout::TAKE_SLOTS),
    ] {
        for (i, slot) in slots.iter().enumerate() {
            let mat = glow::phosphor(&mut materials, palette::AMBER, 0.0);
            commands.spawn((
                Mesh3d(skin.cube.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(barter.to_world(rect_center(*slot)) + bnormal * 0.002)
                    .with_rotation(brot)
                    .with_scale(Vec3::new((slot.w + 4.0) * bu, (slot.h + 4.0) * bv, 0.0015)),
                Visibility::Hidden,
                RowGlow {
                    row,
                    phase: i as f32 * 1.7,
                },
            ));
        }
    }

    // The violation flash's frame bars — four per bay surface — and the
    // glyph pool, all aimed when a hard reject lands. The gantry that
    // used to spawn here is `rig::spawn`'s furniture now: the bay owns
    // its own frame.
    for i in 0..8_u8 {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(flash_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            VioBar(i),
        ));
    }
    for i in 0..GLYPH_BARS {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(glyph_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            GlyphBar(i),
        ));
    }
}

// -------------------------------------------------------------------- cues --

/// Latch what this frame's cues mean for cargo before the view systems run:
/// the settle target, the violation flash, and yesterday's grip.
fn latch_cues(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    mut memo: ResMut<HeldMemo>,
    mut settle: ResMut<PendingSettle>,
    mut flash: ResMut<FlashState>,
) {
    let sim = &shell.bridge.sim;
    for cue in sim.cues() {
        match cue {
            Cue::Place => settle.0 = memo.0.map(|(id, _)| id),
            Cue::Reject { hard: true } => {
                if let Some(rule) = sim.last_violation() {
                    // The footprint the drop would have covered, anchored at
                    // the pointer's cell this frame — the 2D juice's aim.
                    // (An Occupied reject can fire from a bare grab, so an
                    // empty memo means a one-cell flash under the hand.)
                    let (w, h) = memo.0.map_or((1, 1), |(_, kind)| kind.cells());
                    let (x, y) = layout::cell_at(pointer.sim).unwrap_or((0, 0));
                    let anchor = layout::cell_rect(x, y);
                    flash.left = FLASH_LEN;
                    flash.area = Some(Rect::new(
                        anchor.x,
                        anchor.y,
                        f32::from(w) * layout::CELL,
                        f32::from(h) * layout::CELL,
                    ));
                    // The rule picks the glyph, and Suspicious the violet.
                    flash.rule = Some(rule);
                }
            }
            Cue::Reseed => {
                flash.left = 0.0;
                flash.area = None;
                flash.rule = None;
                settle.0 = None;
            }
            _ => {}
        }
    }
    memo.0 = sim.held(0).and_then(|held| {
        sim.pieces()
            .iter()
            .find(|piece| piece.id == held.piece)
            .map(|piece| (piece.id, piece.kind))
    });
}

// -------------------------------------------------------------------- sync --

/// Diff the sim's pieces against the spawned rigs: spawn the new, despawn
/// the gone (everything on `Cue::Reseed`), re-aim each rig at its berth,
/// and run the glide/settle tweens. The tween interpolates scale along
/// with position, so a piece changing worlds — desk model to bay
/// furniture or into a cubby — grows or shrinks across the same glide.
#[allow(clippy::too_many_arguments)]
fn sync_pieces(
    mut commands: Commands,
    time: Res<Time>,
    shell: Res<Shell>,
    skin: Res<Skin>,
    shared: Res<SharedBits>,
    screens: Option<Res<crate::crt::Screens>>,
    pane: Option<Res<crate::viewport::Pane>>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut index: ResMut<PieceIndex>,
    mut settle: ResMut<PendingSettle>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rigs: Query<(&mut PieceRig, &mut Transform, &mut Visibility)>,
) {
    let sim = &shell.bridge.sim;

    // A new world means new cargo: clear everything and respawn below.
    if sim.cues().iter().any(|cue| matches!(cue, Cue::Reseed)) {
        for (_, entity) in index.0.drain() {
            commands.entity(entity).despawn();
        }
    }

    for piece in sim.pieces() {
        let Some((goal, rot, scale)) = berth_site(sim.pieces(), piece, &surfaces) else {
            // A stow with no cabinet under it this frame: hide, never
            // crash — the sim's rules say this cannot happen, and the
            // view's job is to stay standing if it somehow does.
            if let Some(&entity) = index.0.get(&piece.id)
                && let Ok((_, _, mut vis)) = rigs.get_mut(entity)
            {
                vis.set_if_neq(Visibility::Hidden);
            }
            continue;
        };
        if let Some(&entity) = index.0.get(&piece.id) {
            let Ok((mut rig, transform, mut vis)) = rigs.get_mut(entity) else {
                continue;
            };
            vis.set_if_neq(Visibility::Visible);
            if (goal - rig.goal).length_squared() > 1e-8 {
                rig.from = transform.translation;
                rig.rot_from = transform.rotation;
                rig.scale_from = transform.scale;
                rig.goal = goal;
                rig.rot_goal = rot;
                rig.scale_goal = scale;
                rig.ease = 0.0;
            }
            if piece.gnawed && !rig.gnawed_shown {
                rig.gnawed_shown = true;
                commands.entity(rig.bite).insert(Visibility::Visible);
            }
            if settle.0 == Some(piece.id) {
                rig.settle = SETTLE_LEN;
                settle.0 = None;
            }
        } else {
            let place = Transform::from_translation(goal)
                .with_rotation(rot)
                .with_scale(scale);
            let glasses = ScreenGlasses {
                map: screens.as_ref().map(|s| s.map.clone()),
                preview: screens.as_ref().map(|s| s.preview.clone()),
                sky: pane.as_ref().map(|p| p.image.clone()),
            };
            let entity = spawn_rig(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut images,
                &skin,
                &shared,
                &glasses,
                piece,
                place,
            );
            index.0.insert(piece.id, entity);
        }
    }

    // Despawn whatever the sim no longer knows.
    index.0.retain(|id, entity| {
        let live = sim.pieces().iter().any(|piece| piece.id == *id);
        if !live {
            commands.entity(*entity).despawn();
        }
        live
    });

    // The tweens: glide to the berth, settle after a place.
    let dt = time.delta_secs();
    for (mut rig, mut transform, _) in &mut rigs {
        rig.ease = (rig.ease + dt).min(EASE_LEN);
        rig.settle = (rig.settle - dt).max(0.0);
        let eased = ease_out(rig.ease / EASE_LEN);
        let heat = rig.settle / SETTLE_LEN;
        let settle_scale = (heat * heat).mul_add(0.1, 1.0);
        transform.translation = rig.from.lerp(rig.goal, eased);
        transform.rotation = rig.rot_from.slerp(rig.rot_goal, eased);
        transform.scale = rig.scale_from.lerp(rig.scale_goal, eased) * settle_scale;
    }
}

// ---------------------------------------------------------------- fixtures --

/// The fixtures' live state, read fresh from the sim's own predicates
/// each frame: lamp bulbs and their point lights ease between lit and
/// dark glass over [`LAMP_WAKE`] seconds (`lamp_lit` — hold only; a lamp
/// riding a shelf or pad is dark), wall-lamp arms reach for whichever
/// stile their wall column touches, and seedlings blossom exactly where
/// `lit_adjacent` says the lamplight falls.
///
/// The lights themselves are gated through [`Dimmable`]'s base intensity:
/// fx.rs's `dim_cabin` overwrites `PointLight::intensity` from it every
/// frame, so the omen keeps dimming fixture light with no special case.
fn sync_fixtures(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lamps: Query<(&mut LampGlow, &mut Dimmable)>,
    mut arms: Query<(&WallArm, &mut Transform)>,
    mut blossoms: Query<(&Blossom, &mut Visibility)>,
) {
    let pieces = shell.bridge.sim.pieces();
    let step = time.delta_secs() / LAMP_WAKE;
    for (mut lamp, mut dimmable) in &mut lamps {
        let piece = pieces.iter().find(|piece| piece.id == lamp.piece);
        let lit = piece.is_some_and(lamp_lit);
        lamp.level = if lit {
            (lamp.level + step).min(1.0)
        } else {
            (lamp.level - step).max(0.0)
        };
        let lumens = if piece.is_some_and(|piece| piece.kind == Kind::CeilingLamp) {
            CEILING_LUMENS
        } else {
            LAMP_LUMENS
        };
        dimmable.intensity = lumens * lamp.level;
        if let Some(mut mat) = materials.get_mut(&lamp.mat) {
            glow::set_lamp(&mut mat, lamp.color, lamp.level);
        }
    }
    for (arm, mut transform) in &mut arms {
        let left = pieces
            .iter()
            .any(|piece| piece.id == arm.piece && matches!(piece.loc, Loc::Hold { x: 0, .. }));
        transform.rotation = if left {
            Quat::from_rotation_z(PI)
        } else {
            Quat::IDENTITY
        };
    }
    for (blossom, mut visibility) in &mut blossoms {
        let blooming = pieces.iter().any(|piece| {
            piece.id == blossom.piece
                && matches!(piece.loc, Loc::Hold { x, y } if {
                    let (w, h) = piece.kind.cells();
                    (0..w).any(|dx| (0..h).any(|dy| lit_adjacent(pieces, x + dx, y + dy)))
                })
        });
        *visibility = if blooming {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Show each covering's berth-true body — laid flat versus rolled or
/// canned — and ease every luminous coat between dark and burning. A
/// carried covering rides packed even though the sim parks its `Loc` at
/// the origin mid-drag, so the hand never holds a flattened coat. The
/// glow feeds the same two sinks a lamp does: its own glass instance and
/// the tinge light's [`Dimmable`] base (fx.rs owns the omen math).
fn sync_dressings(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut forms: Query<(&DressForm, &mut Visibility)>,
    mut coats: Query<(&mut CoatGlow, &mut Dimmable)>,
) {
    let sim = &shell.bridge.sim;
    let pieces = sim.pieces();
    let in_hand = |id: u32| sim.held(0).is_some_and(|held| held.piece == id);
    let lies = |id: u32| {
        pieces
            .iter()
            .find(|piece| piece.id == id)
            .is_some_and(|piece| matches!(piece.loc, Loc::Laid { .. }))
            && !in_hand(id)
    };
    for (form, mut vis) in &mut forms {
        vis.set_if_neq(if lies(form.piece) == form.laid {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let step = time.delta_secs() / LAMP_WAKE;
    for (mut coat, mut dimmable) in &mut coats {
        let burning = lies(coat.piece);
        coat.level = if burning {
            (coat.level + step).min(1.0)
        } else {
            (coat.level - step).max(0.0)
        };
        dimmable.intensity = COAT_LUMENS * coat.level;
        if let Some(mut mat) = materials.get_mut(&coat.mat) {
            glow::set_lamp(&mut mat, coat.color, coat.level * COAT_GLOW);
        }
    }
}

// ------------------------------------------------------------------- carry --

/// The held piece rides the hand, wearing its legality frame — `LAMP_OK`
/// glow for a drop that would land, `LAMP_NO` plus a diagonal slash for
/// one that would not. Two grips, one carry:
///
/// - **Focused** (the desk): glued to the pointer exactly as the 2D drag
///   was — lifted off the struck panel, a tenth larger.
/// - **Roaming** (the bay): pinned upright at the crosshair's aim point
///   on the bay surfaces; aimed at nothing — the pointer parks off the
///   bay constantly mid-walk — it floats low-center ahead of the camera,
///   carried in both arms rather than visually dropped.
#[allow(clippy::too_many_arguments)]
fn carry_held(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    camera_rig: Res<crate::rig::CameraRig>,
    camera: Single<&Transform, With<crate::rig::CabinCamera>>,
    surfaces: Query<(&Station, &SimSurface)>,
    index: Res<PieceIndex>,
    mut carry: ResMut<CarryState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rigs: Query<(&mut PieceRig, &mut Transform), Without<crate::rig::CabinCamera>>,
    mut vis: Query<&mut Visibility, Without<PieceRig>>,
) {
    let sim = &shell.bridge.sim;
    let held = sim.held(0);

    // The previous carry ended (or swapped): put its tell away.
    if let Some(prev) = carry.carrying
        && held.map(|held| held.piece) != Some(prev)
        && let Some(&entity) = index.0.get(&prev)
        && let Ok((rig, _)) = rigs.get_mut(entity)
    {
        if let Ok(mut v) = vis.get_mut(rig.frame_root) {
            *v = Visibility::Hidden;
        }
        if let Ok(mut v) = vis.get_mut(rig.slash) {
            *v = Visibility::Hidden;
        }
    }

    let Some(held) = held else {
        carry.carrying = None;
        carry.last = None;
        return;
    };
    carry.carrying = Some(held.piece);
    let Some(&entity) = index.0.get(&held.piece) else {
        return;
    };
    let Ok((mut rig, mut transform)) = rigs.get_mut(entity) else {
        return;
    };

    let (pos, rot, fit) = if camera_rig.roaming() {
        if let (Some(world), Some(station)) = (pointer.world, pointer.station)
            && let Some(surface) = surface_of(&surfaces, station)
        {
            // Aimed at the room: hover the piece at the hit, standing
            // the way it would land — upright over the floor and the
            // hopper tiles, flat against a wall or ceiling chart.
            let rot = if matches!(station, Station::BayFloor | Station::Airlock) {
                surface_of(&surfaces, Station::BayWall).map_or_else(
                    || station.face(&surface),
                    |band| Station::BayWall.face(&band),
                )
            } else {
                station.face(&surface)
            };
            (world + station.inward(&surface) * CARRY_LIFT, rot, 1.1)
        } else {
            // Aimed at nothing: hitched low on one arm, off center and
            // compact, its open face turned back toward the carrier —
            // the carrier keeps their view (occlusion, BAY.md).
            let forward = *camera.forward();
            let level = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::NEG_Z);
            (
                camera.translation + forward * CARRY_AHEAD + *camera.right() * CARRY_SIDE
                    - Vec3::Y * CARRY_DOWN,
                Quat::from_rotation_y((-level.x).atan2(-level.z)),
                CARRY_COMPACT,
            )
        }
    } else {
        // The focus drag: the ray's hit lifted off that panel, or —
        // parked pointer — simply wherever it last hovered, falling back
        // to floating over the counter the drag must have started from.
        if let Some(world) = pointer.world
            && let Some(surface) = pointer.station.and_then(|s| surface_of(&surfaces, s))
        {
            carry.last = Some((world + surface.normal() * CARRY_LIFT, surface.orientation()));
        }
        let Some((pos, rot)) = carry.last.or_else(|| {
            surface_of(&surfaces, Station::Barter)
                .map(|desk| (desk.center + desk.normal() * 0.25, desk.orientation()))
        }) else {
            return;
        };
        (pos, rot, 1.1)
    };
    carry.last = Some((pos, rot));

    transform.translation = pos;
    transform.rotation = rot;
    transform.scale = rig.scale_goal * fit;
    // Keep the tween anchored to the hand, so the eventual drop glides
    // from here to the berth instead of teleporting — growing back to
    // full size out of a compact carry.
    rig.from = pos;
    rig.rot_from = rot;
    rig.scale_from = rig.scale_goal * fit;
    rig.ease = 0.0;

    if let Some(mut mat) = materials.get_mut(&rig.frame_mat) {
        let col = if held.legal {
            palette::LAMP_OK
        } else {
            palette::LAMP_NO
        };
        glow::set_lamp(&mut mat, col, 1.0);
    }
    if let Ok(mut v) = vis.get_mut(rig.frame_root) {
        *v = Visibility::Visible;
    }
    if let Ok(mut v) = vis.get_mut(rig.slash) {
        *v = if held.legal {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

// -------------------------------------------------------------------- x-ray --

/// Distance from `p` to the segment `a..b`, metres.
fn segment_distance(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (p - ab.mul_add(Vec3::splat(t), a)).length()
}

/// The focus x-ray (the occlusion defect class, BAY.md): while the
/// camera glides to or parks at a focus, any room cargo standing
/// between the eye and the focused panels goes see-through — body
/// hidden, its footprint frame lit dim glint as the "something stands
/// here" outline. Placement is never refused for camera reasons; the
/// renderer copes, and the outline keeps the ghost honest.
///
/// Desk rows are exempt (they are the focused content), coverings lie
/// flat and cannot blind, the held piece is the player's own hand, and
/// a ghosted cabinet keeps its stowed minis visible — x-ray showing
/// the contents is the point.
#[allow(clippy::too_many_arguments)]
fn xray_focus(
    mut commands: Commands,
    shell: Res<Shell>,
    camera_rig: Res<crate::rig::CameraRig>,
    camera: Single<&Transform, With<crate::rig::CabinCamera>>,
    surfaces: Query<(&Station, &SimSurface)>,
    index: Res<PieceIndex>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rigs: Query<(&PieceRig, &Transform, Option<&XRayed>), Without<crate::rig::CabinCamera>>,
    mut vis: Query<&mut Visibility, Without<PieceRig>>,
) {
    use crate::rig::{Focus, Mode};
    let focus = match camera_rig.mode {
        Mode::ToFocus { focus, .. } | Mode::Focused { focus } => Some(focus),
        Mode::Roam | Mode::ToRoam { .. } => None,
    };
    // Sightline targets: the focused panel group's centers and corners.
    let mut targets: Vec<Vec3> = Vec::new();
    if let Some(focus) = focus {
        for (station, surface) in &surfaces {
            if Focus::of(*station) == Some(focus) {
                targets.push(surface.center);
                for (su, sv) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                    targets.push(surface.center + surface.half_u * su + surface.half_v * sv);
                }
            }
        }
    }
    let sim = &shell.bridge.sim;
    let held = sim.held(0).map(|held| held.piece);
    let eye = camera.translation;
    for piece in sim.pieces() {
        let Some(&entity) = index.0.get(&piece.id) else {
            continue;
        };
        let Ok((rig, transform, xrayed)) = rigs.get(entity) else {
            continue;
        };
        let candidate =
            matches!(piece.loc, Loc::Hold { .. } | Loc::Flotsam { .. }) && held != Some(piece.id);
        let occludes = candidate && !targets.is_empty() && {
            let (w, h) = piece.kind.cells();
            let radius = Vec2::new(
                f32::from(w) * layout::CELL * transform.scale.x,
                f32::from(h) * layout::CELL * transform.scale.y,
            )
            .length()
            .mul_add(0.5, XRAY_MARGIN);
            targets
                .iter()
                .any(|&target| segment_distance(eye, target, transform.translation) < radius)
        };
        if occludes {
            // Asserted every ghosted frame, not just on the transition:
            // `carry_held` may hide the frame the same frame a drop
            // lands, and idempotent writes cost nothing.
            if xrayed.is_none() {
                commands.entity(entity).insert(XRayed);
            }
            if let Ok(mut v) = vis.get_mut(rig.body_root) {
                *v = Visibility::Hidden;
            }
            // The outline shows only during the glide ("something
            // stands here, you are flying through it"); parked at the
            // focus, the ghost goes fully clear — nothing draws over
            // the interface being worked (playtest call-out).
            let outline = matches!(camera_rig.mode, Mode::ToFocus { .. });
            if let Ok(mut v) = vis.get_mut(rig.frame_root) {
                *v = if outline {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            if outline && let Some(mut mat) = materials.get_mut(&rig.frame_mat) {
                glow::set_lamp(&mut mat, palette::ICON_LIT, XRAY_GLOW);
            }
        } else if xrayed.is_some() {
            commands.entity(entity).remove::<XRayed>();
            if let Ok(mut v) = vis.get_mut(rig.body_root) {
                *v = Visibility::Visible;
            }
            if let Ok(mut v) = vis.get_mut(rig.frame_root) {
                *v = Visibility::Hidden;
            }
        }
    }
}

/// The hover frame's lamp level: an aim tell, dimmer than the carry's
/// legality glow and warmer than the x-ray outline's duty.
const HOVER_GLOW: f32 = 0.25;

/// Roam-mode hover feedback: with empty hands, the piece the crosshair
/// would grab wears a faint glint frame before any click — and where a
/// rig's silhouette overflows its cells, the frame says honestly which
/// cells ARE the hitbox (the playtest's mismatch, told instead of
/// hidden).
#[allow(clippy::too_many_arguments)]
fn hover_glint(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    camera_rig: Res<crate::rig::CameraRig>,
    index: Res<PieceIndex>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rigs: Query<&PieceRig>,
    mut vis: Query<&mut Visibility, Without<PieceRig>>,
    mut prev: Local<Option<u32>>,
) {
    let sim = &shell.bridge.sim;
    let hovered = (camera_rig.roaming() && sim.held(0).is_none())
        .then(|| layout::piece_at(sim.pieces(), pointer.sim).map(|piece| piece.id))
        .flatten();
    if *prev != hovered
        && let Some(old) = *prev
        && let Some(&entity) = index.0.get(&old)
        && let Ok(rig) = rigs.get(entity)
        && let Ok(mut v) = vis.get_mut(rig.frame_root)
    {
        *v = Visibility::Hidden;
    }
    *prev = hovered;
    if let Some(id) = hovered
        && let Some(&entity) = index.0.get(&id)
        && let Ok(rig) = rigs.get(entity)
    {
        if let Ok(mut v) = vis.get_mut(rig.frame_root) {
            *v = Visibility::Visible;
        }
        // The handle rule's hover half: over a click-functional piece
        // the frame reads AMBER on the carry handle (a click moves the
        // cargo) and the ordinary glint elsewhere (a click will be the
        // focus interaction) — the split told before it is spent.
        let on_handle = sim
            .pieces()
            .iter()
            .find(|piece| piece.id == id)
            .and_then(|piece| {
                carry_handle_rect(piece.kind, layout::piece_rect(sim.pieces(), piece))
            })
            .is_some_and(|handle| handle.contains(pointer.sim));
        if let Some(mut mat) = materials.get_mut(&rig.frame_mat) {
            let hue = if on_handle {
                palette::AMBER
            } else {
                palette::ICON_LIT
            };
            glow::set_lamp(&mut mat, hue, HOVER_GLOW);
        }
    }
}

// -------------------------------------------------------------------- hints --

/// While a player-owned (or flotsam) piece is held and the pointer maps
/// into the grid, light the footprint cells the drop would cover — the
/// sim's `placement_check` picks the color, a slash marks every refused
/// cell so the ruling survives without hue.
fn placement_hints(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hints: Query<(
        &HintCell,
        &MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
    mut slashes: Query<&mut Visibility, Without<HintCell>>,
) {
    let sim = &shell.bridge.sim;
    let plan = sim.held(0).and_then(|held| {
        let ours = player_owned(held.origin) || matches!(held.origin, Loc::Flotsam { .. });
        if !ours {
            return None;
        }
        let piece = sim.pieces().iter().find(|piece| piece.id == held.piece)?;
        let (ax, ay) = layout::cell_at(pointer.sim)?;
        // The hint must consult the SAME arbiter the drop will: a
        // covering answers to the dressing rules (a tin coats any
        // chart), everything else to placement. The playtest's
        // green-frame-over-red-hint contradiction was this line using
        // one arbiter for both.
        let legal = if piece.kind.covering() {
            space_trucking::sim::cargo::dressing_check(sim.pieces(), piece.id, piece.kind, ax, ay)
                .is_ok()
        } else {
            placement_check(sim.pieces(), piece.id, piece.kind, ax, ay).is_ok()
        };
        let (w, h) = piece.kind.cells();
        Some((ax, ay, w, h, legal))
    });
    for (cell, material, mut visibility) in &mut hints {
        let lit = plan.filter(|&(ax, ay, w, h, _)| {
            cell.x >= ax && cell.x < ax + w && cell.y >= ay && cell.y < ay + h
        });
        if let Some((_, _, _, _, legal)) = lit {
            *visibility = Visibility::Visible;
            if let Some(mut mat) = materials.get_mut(&material.0) {
                let col = if legal {
                    palette::LAMP_OK
                } else {
                    palette::LAMP_NO
                };
                glow::set_lamp(&mut mat, col, 0.8);
            }
            if let Ok(mut slash) = slashes.get_mut(cell.slash) {
                *slash = if legal {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                };
            }
        } else {
            *visibility = Visibility::Hidden;
            if let Ok(mut slash) = slashes.get_mut(cell.slash) {
                *slash = Visibility::Hidden;
            }
        }
    }
}

// ------------------------------------------------------------ drop targets --

/// Breathe amber over exactly what the sim's drop matrix invites. The
/// rail quads answer for the shelf while a barter is open and for the
/// outboard net while none is — the two lives of one row of sockets —
/// and the `stow` flag wakes the cabinets: empty cubby mouths breathe a
/// gentler amber while the carried piece could box up somewhere.
fn invite_glows(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glows: Query<
        (&RowGlow, &MeshMaterial3d<StandardMaterial>, &mut Visibility),
        Without<CubbyGlow>,
    >,
    mut cubbies: Query<
        (
            &CubbyGlow,
            &MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        Without<RowGlow>,
    >,
) {
    let sim = &shell.bridge.sim;
    let targets = sim.drop_targets(0);
    let docked = sim.barter().is_some();
    let t = time.elapsed_secs();
    for (row_glow, material, mut visibility) in &mut glows {
        let invited = targets.is_some_and(|targets| match row_glow.row {
            Row::Give => targets.give,
            Row::Take => targets.take,
            Row::Received => targets.received,
            Row::Rail => {
                if docked {
                    targets.shelf
                } else {
                    targets.net
                }
            }
        });
        if invited {
            *visibility = Visibility::Visible;
            if let Some(mut mat) = materials.get_mut(&material.0) {
                let level = glow::breathe(t, 2.0, row_glow.phase).mul_add(0.3, 0.45);
                glow::set_lamp(&mut mat, palette::AMBER, level);
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    // Which cubbies to light is sim *state*, never a re-derived rule: the
    // invitation itself is `targets.stow`; a cubby answers it when its
    // host stands in the hold (a shelved cabinet stores nothing) and no
    // piece already rides that slot.
    let inviting = targets.is_some_and(|targets| targets.stow);
    for (cubby, material, mut visibility) in &mut cubbies {
        let hosted = sim
            .pieces()
            .iter()
            .any(|piece| piece.id == cubby.piece && matches!(piece.loc, Loc::Hold { .. }));
        let empty = !sim.pieces().iter().any(|piece| {
            matches!(
                piece.loc,
                Loc::Stow { cabinet, slot } if cabinet == cubby.piece && slot == cubby.slot
            )
        });
        if inviting && hosted && empty {
            *visibility = Visibility::Visible;
            if let Some(mut mat) = materials.get_mut(&material.0) {
                let level = glow::breathe(t, 2.0, cubby.phase).mul_add(0.2, 0.3);
                glow::set_lamp(&mut mat, palette::AMBER, level);
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

// -------------------------------------------------------------------- flash --

/// A glyph bar spanning `a` to `b` with `girth`, as the (centre, size,
/// tilt) triple the flash system turns into a cube transform. Endpoints
/// are panel-local sim units, +y up.
fn bar_between(a: Vec2, b: Vec2, girth: f32) -> (Vec2, Vec2, f32) {
    let d = b - a;
    ((a + b) * 0.5, Vec2::new(girth, d.length()), -d.x.atan2(d.y))
}

/// The violation glyphs, one small bar-built icon per refused rule — the
/// 2D console's hand-drawn set (the kettlebell, the mount bracket, the
/// hazard triangle, the snowflake) carried over, plus the cabinet's full
/// box. Bounds, overlap, and the suspicious objection stay glyphless:
/// the frame — and its violet — already says everything those rules
/// mean. Offsets are panel-local sim units, +y up; `rect` steers the
/// affix bracket toward the surface the footprint missed.
fn glyph_spec(rule: Option<Violation>, rect: Rect) -> Vec<(Vec2, Vec2, f32)> {
    let s = GLYPH_S;
    match rule {
        // The hazard triangle, chevron nosing down its middle.
        Some(Violation::Volatile) => vec![
            bar_between(
                Vec2::new(-s * 0.9, -s * 0.7),
                Vec2::new(s * 0.9, -s * 0.7),
                s * 0.18,
            ),
            bar_between(Vec2::new(-s * 0.9, -s * 0.7), Vec2::new(0.0, s), s * 0.18),
            bar_between(Vec2::new(s * 0.9, -s * 0.7), Vec2::new(0.0, s), s * 0.18),
            bar_between(
                Vec2::new(-s * 0.35, s * 0.35),
                Vec2::new(0.0, -s * 0.1),
                s * 0.16,
            ),
            bar_between(
                Vec2::new(0.0, -s * 0.1),
                Vec2::new(s * 0.35, s * 0.35),
                s * 0.16,
            ),
        ],
        // The snowflake: three arms through the centre.
        Some(Violation::Cryo) => (0..3_u8)
            .map(|i| {
                let angle = f32::from(i).mul_add(PI / 3.0, FRAC_PI_2);
                let tip = Vec2::new(angle.cos(), angle.sin()) * s;
                bar_between(-tip, tip, s * 0.16)
            })
            .collect(),
        // The bracket the fixture failed to reach, turned toward the
        // missed surface, bolts just inside the stub.
        Some(Violation::Affix(mount)) => {
            let grid_mid_x =
                f32::from(layout::GRID_COLS).mul_add(layout::CELL * 0.5, layout::GRID_ORIGIN.x);
            // `out` points off the mount surface into the room.
            let out = match mount {
                Mount::Ceiling => Vec2::new(0.0, -1.0),
                Mount::Floor => Vec2::new(0.0, 1.0),
                Mount::Wall => {
                    let left = rect.w.mul_add(0.5, rect.x) < grid_mid_x;
                    Vec2::new(if left { 1.0 } else { -1.0 }, 0.0)
                }
            };
            let along = out.perp();
            let base = -out * (s * 0.6);
            let mut bars = vec![bar_between(base - along * s, base + along * s, s * 0.22)];
            for bolt in [-0.5_f32, 0.5] {
                bars.push((
                    base + along * (s * bolt) + out * (s * 0.3),
                    Vec2::splat(s * 0.3),
                    0.0,
                ));
            }
            bars
        }
        // The full box: a crate packed past its rim, lid floating off.
        Some(Violation::Occupied) => vec![
            bar_between(
                Vec2::new(-s * 0.7, -s * 0.75),
                Vec2::new(-s * 0.7, s * 0.35),
                s * 0.2,
            ),
            bar_between(
                Vec2::new(s * 0.7, -s * 0.75),
                Vec2::new(s * 0.7, s * 0.35),
                s * 0.2,
            ),
            bar_between(
                Vec2::new(-s * 0.8, -s * 0.75),
                Vec2::new(s * 0.8, -s * 0.75),
                s * 0.2,
            ),
            (Vec2::new(0.0, -s * 0.2), Vec2::new(s * 1.1, s * 0.55), 0.0),
            (Vec2::new(0.0, s * 0.62), Vec2::new(s * 1.9, s * 0.22), 0.0),
        ],
        // Off the net, onto a piece (or its standing shadow), the violet
        // objection, the doormat, the sealed floor, and the last vital
        // instrument refusing its exit: the frame alone. (Aisle, Sealed,
        // and Vital are rules still owed their own glyphs — the frame
        // and the buzz carry them meanwhile.)
        Some(
            Violation::Bounds
            | Violation::Overlap
            | Violation::Suspicious
            | Violation::Aisle
            | Violation::Sealed
            | Violation::Vital,
        )
        | None => vec![],
    }
}

/// Glyph ink per rule, the 2D icons' own colors: GLINT hardware, AMBER
/// hazard, frost in the cryo core's hue.
const fn glyph_color(rule: Option<Violation>) -> Color {
    match rule {
        Some(Violation::Volatile) => palette::AMBER,
        Some(Violation::Cryo) => palette::kind_color(Kind::CryoCore),
        _ => palette::GLINT,
    }
}

/// The hard-reject flash: a frame burning over the attempted footprint
/// for just under half a second — `LAMP_NO`, or `EERIE` when the hold
/// itself objected to a second suspicious crate — with the refused
/// rule's glyph over its middle. A bay footprint may straddle the fold,
/// so the frame draws per surface: bars 0–3 take the wall band's share,
/// 4–7 the deck strip's, and at the seam the two frames kiss because the
/// fold is watertight.
fn violation_flash(
    time: Res<Time>,
    shared: Res<SharedBits>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut flash: ResMut<FlashState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bars: Query<(&VioBar, &mut Transform, &mut Visibility), Without<GlyphBar>>,
    mut glyphs: Query<(&GlyphBar, &mut Transform, &mut Visibility), Without<VioBar>>,
) {
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    let live = flash.left > 0.0;
    let Some(rect) = flash.area.filter(|_| live) else {
        for (_, _, mut visibility) in &mut bars {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut glyphs {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    // The room-grid law says a footprint lies wholly in one chart, so
    // the frame no longer splits over the fold: four bars on the one
    // chart, four spare bars idle. A refused drop with its anchor off
    // the net (a hole, dead space) shows nothing — the buzz carries it.
    let Some((station, surface)) = chart_of(&surfaces, rect_center(rect)) else {
        for (_, _, mut visibility) in &mut bars {
            *visibility = Visibility::Hidden;
        }
        for (_, _, mut visibility) in &mut glyphs {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let heat = flash.left / FLASH_LEN;
    let color = if matches!(flash.rule, Some(Violation::Suspicious)) {
        palette::EERIE
    } else {
        palette::LAMP_NO
    };
    if let Some(mut mat) = materials.get_mut(&shared.flash) {
        glow::set_lamp(&mut mat, color, heat);
    }
    let inward = station.inward(&surface);
    let face = station.face(&surface);
    for (bar, mut transform, mut visibility) in &mut bars {
        if bar.0 >= 4 {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let (su, sv) = (surface.scale_u(), surface.scale_v());
        let across = Vec3::new((rect.w + 6.0) * su, 3.0 * sv, 0.003);
        let down = Vec3::new(3.0 * su, (rect.h + 6.0) * sv, 0.003);
        let (mid, scale) = match bar.0 % 4 {
            0 => (SimVec2::new(rect.w.mul_add(0.5, rect.x), rect.y), across),
            1 => (
                SimVec2::new(rect.w.mul_add(0.5, rect.x), rect.y + rect.h),
                across,
            ),
            2 => (SimVec2::new(rect.x, rect.h.mul_add(0.5, rect.y)), down),
            _ => (
                SimVec2::new(rect.x + rect.w, rect.h.mul_add(0.5, rect.y)),
                down,
            ),
        };
        transform.translation = surface.to_world(mid) + inward * crate::rig::layer::FLASH;
        transform.rotation = face;
        transform.scale = scale;
    }

    let spec = glyph_spec(flash.rule, rect);
    if let Some(mut mat) = materials.get_mut(&shared.glyph) {
        glow::set_lamp(&mut mat, glyph_color(flash.rule), heat);
    }
    let mid = rect_center(rect);
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    let rot = face;
    let anchor = surface.to_world(mid) + inward * crate::rig::layer::GLYPH;
    for (bar, mut transform, mut visibility) in &mut glyphs {
        let Some(&(offset, size, tilt)) = spec.get(usize::from(bar.0)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Visible;
        transform.translation = anchor + rot * Vec3::new(offset.x * su, offset.y * sv, 0.0);
        transform.rotation = rot * Quat::from_rotation_z(tilt);
        transform.scale = Vec3::new(size.x * su, size.y * sv, 0.003);
    }
}

// ---------------------------------------------------------------------- rat --

/// The stowaway: spawned while `sim.rat()` says one is aboard, hopping
/// between bay cells on the sim's own tween (tick, `moved_at`, alpha —
/// replays exactly), nose along its travel. The wall rows are climbable
/// — it is a ship rat, flat against the band with its nose where it is
/// going — and the deck row is ordinary floor; the watertight fold hands
/// one to the other mid-hop without a gap.
#[allow(clippy::too_many_arguments)]
fn rat_watch(
    mut commands: Commands,
    time: Res<Time>,
    shell: Res<Shell>,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<RatState>,
    mut roots: Query<&mut Transform, With<RatRoot>>,
    mut tails: Query<(&RatTail, &mut Transform), Without<RatRoot>>,
) {
    let sim = &shell.bridge.sim;
    let Some(rat) = sim.rat() else {
        if let Some(entity) = state.entity.take() {
            commands.entity(entity).despawn();
        }
        state.yaw = 0.0;
        return;
    };
    let Some(floor) = surface_of(&surfaces, Station::BayFloor) else {
        return;
    };

    // The hop is sim-driven feedback: position interpolates between the
    // previous and current perch with a shallow arc off the surface.
    let age = sim.tick().saturating_sub(rat.moved_at) as f32 + sim.alpha();
    let t = (age / RAT_HOP_TICKS).clamp(0.0, 1.0);
    let from = perch(rat.prev_cell);
    let to = perch(rat.cell);
    let at = from.lerp(to, ease_out(t));
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx.mul_add(dx, dy * dy) > 1.0 {
        // Panel-up is sim -y, so the yaw flips the sim's vertical.
        state.yaw = (-dy).atan2(dx);
    }
    // A couch under its paws is bedtime. The sim already stretches the
    // hop cadence NAP_LAZE-wide; the pose says so too — flattened along
    // the cushions, nothing fidgeting — so it reads asleep in a still.
    let napping = t >= 1.0
        && sim.pieces().iter().any(|piece| {
            let Loc::Hold { x, y } = piece.loc else {
                return false;
            };
            let (w, h) = piece.kind.cells();
            piece.kind == Kind::Couch
                && (x..x + w).contains(&rat.cell.0)
                && (y..y + h).contains(&rat.cell.1)
        });
    let unit = (floor.scale_u() + floor.scale_v()) * 0.5 * RAT_FIT;
    let hop = (PI * t).sin() * 5.0 * unit;
    // Asleep it settles to its cell's centre and lies ON the standing
    // couch's cushions — their crowns sit 0.60 footprint-heights over
    // the plates (centre lifted 0.5, cushion tops at +0.10; see the
    // couch rig) — instead of hiding inside the upholstery.
    let (at, scale, lift) = if napping {
        let cell = layout::cell_rect(rat.cell.0, rat.cell.1);
        (
            rect_center(cell),
            // Long and low: nose splayed out, belly in the upholstery.
            Vec3::new(unit * 1.18, unit * 1.06, unit * 0.6),
            0.60 * layout::CELL * floor.scale_v() * BAY_FIT,
        )
    } else {
        (at, Vec3::splat(unit), 0.0)
    };
    // A mid-hop position between two charts reads through whichever
    // chart holds the interpolated point; the fold seams are watertight
    // so the handover never opens a gap. Off-chart interpolants (a hop
    // whose midpoint crosses a fold corner) fall back to the nearer
    // perch's chart.
    let Some((station, surface)) = chart_of(&surfaces, at)
        .or_else(|| chart_of(&surfaces, to))
        .or_else(|| chart_of(&surfaces, from))
    else {
        return;
    };
    let inward = station.inward(&surface);
    let place = Transform::from_translation(surface.to_world(at) + inward * (hop + lift))
        .with_rotation(station.face(&surface) * Quat::from_rotation_z(state.yaw))
        .with_scale(scale);
    if let Some(entity) = state.entity {
        if let Ok(mut transform) = roots.get_mut(entity) {
            *transform = place;
        }
    } else {
        state.entity = Some(spawn_rat(&mut commands, &mut meshes, &skin, place));
    }

    // The tail: idle-clock sway awake; curled tight and still asleep.
    let sway = (glow::breathe(time.elapsed_secs(), 3.0, 0.0) - 0.5) * 0.9;
    for (tail, mut transform) in &mut tails {
        if napping {
            transform.rotation = Quat::from_rotation_z(1.2) * tail.base;
            transform.scale = Vec3::new(1.0, 0.6, 1.0);
        } else {
            transform.rotation = Quat::from_rotation_z(sway) * tail.base;
            transform.scale = Vec3::ONE;
        }
    }
}

/// Build the rat: metal-family grays only (`RIVET` and friends), per the
/// art direction — nothing about the stowaway is hue-coded. Local axes:
/// +X nose, +Y left flank, +Z off the panel.
fn spawn_rat(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    skin: &Skin,
    place: Transform,
) -> Entity {
    let root = commands.spawn((place, Visibility::default(), RatRoot)).id();
    let ball = meshes.add(ico(1.0));
    commands.spawn((
        Mesh3d(ball.clone()),
        MeshMaterial3d(skin.rivet.clone()),
        Transform::from_xyz(0.0, 0.0, 2.6).with_scale(Vec3::new(5.0, 3.2, 2.8)),
        ChildOf(root),
    ));
    commands.spawn((
        Mesh3d(ball),
        MeshMaterial3d(skin.rivet.clone()),
        Transform::from_xyz(4.8, 0.0, 3.4).with_scale(Vec3::splat(2.4)),
        ChildOf(root),
    ));
    let ear = meshes.add(Mesh::from(Cone {
        radius: 1.1,
        height: 2.2,
    }));
    for side in [-1.0, 1.0] {
        commands.spawn((
            Mesh3d(ear.clone()),
            MeshMaterial3d(skin.rivet.clone()),
            Transform::from_xyz(4.2, 1.3 * side, 5.6)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ChildOf(root),
        ));
    }
    let base = Quat::from_rotation_z(FRAC_PI_2);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.45, 7.5))),
        MeshMaterial3d(skin.plate_shade.clone()),
        Transform::from_xyz(-6.2, 0.0, 2.0).with_rotation(base),
        RatTail { base },
        ChildOf(root),
    ));
    root
}

// -------------------------------------------------------------- decoration --

/// Run every breathing emissive: the suspicious hum (~1 Hz, the audio's
/// beat), the very-mysterious chord. Each Pulse owns its material.
fn breathe_pulses(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pulses: Query<(&Pulse, &MeshMaterial3d<StandardMaterial>)>,
) {
    let t = time.elapsed_secs();
    for (pulse, material) in &pulses {
        if let Some(mut mat) = materials.get_mut(&material.0) {
            mat.emissive = pulse.color.to_linear()
                * glow::breathe(t, pulse.freq, pulse.phase).mul_add(pulse.amp, pulse.base);
        }
    }
}

/// Sweep from full (a leg just started) to empty across the ETA dial —
/// ±this much rotation around twelve o'clock.
const NEEDLE_SWEEP: f32 = 2.4;

/// The ETA gauge pieces read the leg: needle at the top of its sweep
/// when a course is armed at the dock, draining as the leg completes,
/// resting at empty otherwise — the console arc's reading, carried by
/// the instrument that owns it now.
fn eta_needles(shell: Res<Shell>, mut needles: Query<(&EtaNeedle, &mut Transform)>) {
    let sim = &shell.bridge.sim;
    let remaining = match sim.ship().state {
        ShipState::Traveling {
            progress,
            leg_ticks,
            ..
        } => 1.0 - ((progress as f32 + sim.alpha()) / leg_ticks as f32).clamp(0.0, 1.0),
        ShipState::Docked(_) if sim.ship().selected.is_some() => 1.0,
        ShipState::Docked(_) => 0.0,
    };
    let angle = remaining.mul_add(NEEDLE_SWEEP, -NEEDLE_SWEEP * 0.5);
    for (needle, mut transform) in &mut needles {
        let spin = Quat::from_rotation_z(angle);
        transform.rotation = spin;
        transform.translation = Vec3::new(0.0, 0.0, 7.2) + spin * Vec3::new(0.0, needle.reach, 0.0);
    }
}

// ------------------------------------------------------------------- rigs --

/// Everything a kind builder needs in one grip.
struct RigParts<'w, 's, 'a> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    images: &'a mut Assets<Image>,
    skin: &'a Skin,
    /// The live screen textures the instrument pieces wear: the chart
    /// tank's map, the destination preview's glass, the window's sky.
    /// `None` in headless paths; the builders fall back to phosphor.
    map_image: Option<Handle<Image>>,
    preview_image: Option<Handle<Image>>,
    sky_image: Option<Handle<Image>>,
    root: Entity,
}

impl RigParts<'_, '_, '_> {
    /// Add one mesh part under the rig root.
    fn part(
        &mut self,
        mesh: impl Into<Mesh>,
        material: Handle<StandardMaterial>,
        transform: Transform,
    ) -> Entity {
        let mesh = self.meshes.add(mesh.into());
        self.spawn(mesh, material, transform)
    }

    /// Add a part reusing an existing mesh handle.
    fn spawn(
        &mut self,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
        transform: Transform,
    ) -> Entity {
        self.commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                transform,
                ChildOf(self.root),
            ))
            .id()
    }

    /// An enamel (lit painted metal) material for an accent.
    fn tint(&mut self, color: Color) -> Handle<StandardMaterial> {
        glow::enamel(self.materials, color)
    }
}

/// The carry-handle law (BAY.md, "The handle rule"): a click-functional
/// kind declares the sub-rect of its footprint that grabs as cargo, as
/// fractions of the piece rect in sim orientation (+y down). A press
/// inside routes to carry; anywhere else on the piece, to focus. The
/// rig draws the amber grab from THIS declaration ([`carry_grab`]), so
/// hitbox and geometry cannot drift apart. `None` = passive cargo:
/// nothing to guard, the whole body grabs.
pub const fn carry_handle(kind: Kind) -> Option<Rect> {
    match kind {
        Kind::ChartTank | Kind::LaunchLever => Some(Rect::new(0.25, 0.80, 0.50, 0.16)),
        _ => None,
    }
}

/// [`carry_handle`] in sim units over a berthed piece's rect.
pub fn carry_handle_rect(kind: Kind, rect: Rect) -> Option<Rect> {
    carry_handle(kind).map(|frac| {
        Rect::new(
            frac.x.mul_add(rect.w, rect.x),
            frac.y.mul_add(rect.h, rect.y),
            frac.w * rect.w,
            frac.h * rect.h,
        )
    })
}

/// Draw the amber carry grab exactly over the declared handle sub-rect:
/// a glowing crossbar in two brass stanchions, the one shape every
/// movable instrument shares — grab semantics read by form, not hue
/// alone. `z` is the local depth the bar rides at.
fn carry_grab(rig: &mut RigParts<'_, '_, '_>, kind: Kind, fw: f32, fh: f32, z: f32) {
    let Some(frac) = carry_handle(kind) else {
        return;
    };
    // Fractions (+y down) to rig-local (+y up), about the rig centre.
    let cx = frac.w.mul_add(0.5, frac.x) - 0.5;
    let cy = 0.5 - frac.h.mul_add(0.5, frac.y);
    let (hx, hy) = (cx * fw, cy * fh);
    let (hw, hh) = (frac.w * fw, frac.h * fh);
    let bar = glow::phosphor(rig.materials, palette::AMBER, 1.2);
    rig.part(
        Cuboid::new(hw * 0.9, hh * 0.55, 2.6),
        bar,
        Transform::from_xyz(hx, hy, z),
    );
    let post = rig.meshes.add(Cuboid::new(hh * 0.4, hh * 0.4, z - 0.5));
    for sx in [-1.0f32, 1.0] {
        rig.spawn(
            post.clone(),
            rig.skin.brass.clone(),
            Transform::from_xyz(sx.mul_add(hw * 0.45, hx), hy, (z - 0.5).mul_add(0.5, 0.5)),
        );
    }
}

/// The live screen textures handed down to the instrument builders.
#[derive(Default)]
struct ScreenGlasses {
    map: Option<Handle<Image>>,
    preview: Option<Handle<Image>>,
    sky: Option<Handle<Image>>,
}

/// Spawn one piece's whole rig at `place`: the kind's silhouette in local
/// sim units (footprint `w*CELL × h*CELL` in X/Y, thickness up +Z off the
/// panel), the hidden bite wedge, and the hidden carry-legality frame.
#[allow(clippy::too_many_arguments)]
fn spawn_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    skin: &Skin,
    shared: &SharedBits,
    glasses: &ScreenGlasses,
    piece: &Piece,
    place: Transform,
) -> Entity {
    let root = commands.spawn((place, Visibility::default())).id();
    // The body layer: everything that *is* the piece parents here, so
    // the focus x-ray can hide the whole silhouette in one write while
    // the frame (a sibling) stays showable.
    let body_root = commands
        .spawn((Transform::default(), Visibility::default(), ChildOf(root)))
        .id();
    let color = palette::variant_tint(palette::kind_color(piece.kind), piece.variant);
    let (w, h) = piece.kind.cells();
    let fw = f32::from(w) * layout::CELL;
    let fh = f32::from(h) * layout::CELL;
    let mut rig = RigParts {
        commands: &mut *commands,
        meshes: &mut *meshes,
        materials: &mut *materials,
        images: &mut *images,
        skin,
        map_image: glasses.map.clone(),
        preview_image: glasses.preview.clone(),
        sky_image: glasses.sky.clone(),
        root: body_root,
    };
    build_kind(&mut rig, piece, color, fw, fh);

    // The rat's mark: a socket-dark wedge biting past the right flank —
    // it changes the silhouette, so it reads in any palette.
    let turn = (splitmix(u64::from(piece.id), SALT_BITE) % 628) as f32 / 100.0;
    let bite = rig.part(
        Cylinder::new(8.0, 26.0).mesh().resolution(3).build(),
        skin.socket.clone(),
        Transform::from_xyz(fw * 0.46, fh * 0.30, 13.0)
            .with_rotation(Quat::from_rotation_z(turn) * Quat::from_rotation_x(FRAC_PI_2)),
    );
    rig.commands.entity(bite).insert(if piece.gnawed {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });

    // The carry tell: an emissive frame around the footprint plus a slash
    // bar, both dark until the carry system wakes them.
    let frame_mat = glow::phosphor(rig.materials, palette::LAMP_OK, 0.0);
    let frame_root = rig
        .commands
        .spawn((Transform::default(), Visibility::Hidden, ChildOf(root)))
        .id();
    // A wireframe BOX, not a flat rectangle: the tell wraps the body's
    // volume (playtest: a fixed-plane rectangle around a 3D object read
    // as UI debris). Twelve edges around the footprint and the rigs'
    // common depth.
    let (hx, hy) = (fw.mul_add(0.5, 3.0), fh.mul_add(0.5, 3.0));
    let (z0, z1) = (-2.0, 30.0);
    let rail_x = rig.meshes.add(Cuboid::new(hx.mul_add(2.0, 2.6), 2.6, 2.6));
    let rail_y = rig.meshes.add(Cuboid::new(2.6, hy.mul_add(2.0, 2.6), 2.6));
    let rail_z = rig.meshes.add(Cuboid::new(2.6, 2.6, z1 - z0 + 2.6));
    let zm = f32::midpoint(z0, z1);
    let mut edges: Vec<(Handle<Mesh>, Vec3)> = Vec::new();
    for z in [z0, z1] {
        for sy in [-1.0, 1.0] {
            edges.push((rail_x.clone(), Vec3::new(0.0, sy * hy, z)));
        }
        for sx in [-1.0, 1.0] {
            edges.push((rail_y.clone(), Vec3::new(sx * hx, 0.0, z)));
        }
    }
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            edges.push((rail_z.clone(), Vec3::new(sx * hx, sy * hy, zm)));
        }
    }
    for (mesh, at) in edges {
        rig.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_translation(at),
            ChildOf(frame_root),
        ));
    }
    let slash = rig
        .commands
        .spawn((
            Mesh3d(rig.meshes.add(Cuboid::new(fw.hypot(fh) + 4.0, 3.0, 3.0))),
            MeshMaterial3d(shared.slash.clone()),
            Transform::from_xyz(0.0, 0.0, 34.0)
                .with_rotation(Quat::from_rotation_z((fh / fw).atan())),
            Visibility::Hidden,
            ChildOf(frame_root),
        ))
        .id();

    commands.entity(root).insert(PieceRig {
        from: place.translation,
        goal: place.translation,
        rot_from: place.rotation,
        rot_goal: place.rotation,
        scale_from: place.scale,
        scale_goal: place.scale,
        ease: EASE_LEN,
        settle: 0.0,
        gnawed_shown: piece.gnawed,
        bite,
        body_root,
        frame_root,
        slash,
        frame_mat,
    });
    root
}

/// One silhouette per cargo kind, the 2D glyph identities restated as
/// primitives. Variants ride the tint; ids seed the decoration phases.
#[allow(clippy::too_many_lines)]
fn build_kind(rig: &mut RigParts, piece: &Piece, color: Color, fw: f32, fh: f32) {
    let body = rig.tint(color);
    match piece.kind {
        // A pink rhombus with a sparkle: a cube on its corner, one glint.
        Kind::PerfumeVial => {
            rig.part(
                Cuboid::new(fw * 0.52, fh * 0.52, 15.0),
                body,
                Transform::from_xyz(0.0, 0.0, 9.0).with_rotation(Quat::from_rotation_z(FRAC_PI_4)),
            );
            let sparkle = glow::phosphor(rig.materials, palette::GLINT, 2.5);
            rig.part(
                ico(2.2),
                sparkle,
                Transform::from_xyz(fw * 0.3, fh * 0.3, 16.0),
            );
        }
        // A gold slab, a darker belt, a sphere head. Unimaginably tacky.
        Kind::GildedIdol => {
            let belt = rig.tint(palette::mix(color, palette::SHADOW, 0.45));
            rig.part(
                Cuboid::new(fw * 0.58, fh * 0.52, 18.0),
                body.clone(),
                Transform::from_xyz(0.0, -fh * 0.1, 9.0),
            );
            rig.part(
                Cuboid::new(fw * 0.62, fh * 0.07, 19.0),
                belt,
                Transform::from_xyz(0.0, -fh * 0.02, 9.5),
            );
            rig.part(
                ico(fw * 0.26),
                body,
                Transform::from_xyz(0.0, fh * 0.28, 12.0),
            );
        }
        // A 2×2 sub-grid of identical government flavour.
        Kind::RationBricks => {
            let brick = rig.meshes.add(Cuboid::new(26.0, 26.0, 16.0));
            for (ix, iy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                rig.spawn(
                    brick.clone(),
                    body.clone(),
                    Transform::from_xyz(15.5 * ix, 15.5 * iy, 8.0),
                );
            }
        }
        // Two rust bars, stacked askew.
        Kind::ScrapAlloy => {
            let under = rig.tint(palette::mix(color, palette::SHADOW, 0.25));
            rig.part(
                Cuboid::new(fw * 0.92, fh * 0.36, 10.0),
                under,
                Transform::from_xyz(-fw * 0.02, -fh * 0.16, 5.0),
            );
            rig.part(
                Cuboid::new(fw * 0.88, fh * 0.34, 10.0),
                body,
                Transform::from_xyz(fw * 0.02, fh * 0.14, 15.0),
            );
        }
        // A pot with a sprout on top. Under lamplight it blooms: three
        // PerfumeVial-pink buds, hidden until `lit_adjacent` says the
        // footprint sits in a lit lamp's halo (presentation only, the
        // 2D bloom's reading).
        Kind::Seedlings => {
            let pot = rig.tint(palette::mix(color, palette::SHADOW, 0.35));
            rig.part(
                Cylinder::new(fw * 0.3, 12.0),
                pot,
                Transform::from_xyz(0.0, -fh * 0.1, 6.0)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            rig.part(
                Cone {
                    radius: fw * 0.2,
                    height: 18.0,
                },
                body,
                Transform::from_xyz(0.0, -fh * 0.1, 21.0)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let bud_mat = rig.tint(palette::kind_color(Kind::PerfumeVial));
            let bud = rig.meshes.add(ico(2.4));
            for (bx, by, bz) in [(-6.0, -3.5, 15.0), (5.5, -2.5, 17.0), (0.8, -4.5, 27.0)] {
                rig.commands.spawn((
                    Mesh3d(bud.clone()),
                    MeshMaterial3d(bud_mat.clone()),
                    Transform::from_xyz(bx, by, bz),
                    Visibility::Hidden,
                    Blossom { piece: piece.id },
                    ChildOf(rig.root),
                ));
            }
        }
        // A horizontal capsule wearing hazard chevrons.
        Kind::GasCanister => {
            rig.part(
                Capsule3d::new(10.0, 34.0),
                body,
                Transform::from_xyz(0.0, 0.0, 10.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            );
            let warn = rig.tint(palette::mix(color, palette::SHADOW, 0.5));
            let leg = rig.meshes.add(Cuboid::new(9.0, 3.0, 3.0));
            for cx in [-8.0, 8.0] {
                rig.spawn(
                    leg.clone(),
                    warn.clone(),
                    Transform::from_xyz(cx - 2.5, 3.2, 19.0)
                        .with_rotation(Quat::from_rotation_z(-FRAC_PI_4)),
                );
                rig.spawn(
                    leg.clone(),
                    warn.clone(),
                    Transform::from_xyz(cx - 2.5, -3.2, 19.0)
                        .with_rotation(Quat::from_rotation_z(FRAC_PI_4)),
                );
            }
        }
        // A hexagonal prism in a frost ring.
        Kind::CryoCore => {
            rig.part(
                Cylinder::new(fw * 0.36, 18.0).mesh().resolution(6).build(),
                body,
                Transform::from_xyz(0.0, 0.0, 9.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let frost = rig.tint(palette::mix(palette::GLINT, color, 0.4));
            let r = fw * 0.44;
            rig.part(
                Torus::new(r - 1.4, r + 1.4),
                frost,
                Transform::from_xyz(0.0, 0.0, 9.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
        }
        // Three stacked pearls, the middle a shade wetter.
        Kind::BrinePearls => {
            let mid = rig.tint(palette::mix(color, palette::SHADOW, 0.15));
            let pearl = rig.meshes.add(ico(10.5));
            rig.spawn(
                pearl.clone(),
                body.clone(),
                Transform::from_xyz(0.0, 21.0, 10.0),
            );
            rig.spawn(pearl.clone(), mid, Transform::from_xyz(0.0, 0.0, 10.0));
            rig.spawn(pearl, body, Transform::from_xyz(0.0, -21.0, 10.0));
        }
        // Matte near-black, breathing an eerie edge frame at the audio
        // hum's ~1 Hz beat.
        Kind::SuspiciousCrate => {
            rig.part(
                Cuboid::new(fw * 0.84, fh * 0.84, 24.0),
                body,
                Transform::from_xyz(0.0, 0.0, 12.0),
            );
            let hum = glow::phosphor(rig.materials, palette::EERIE, 0.5);
            let along = rig.meshes.add(Cuboid::new(fw * 0.86, 2.6, 2.6));
            let across = rig.meshes.add(Cuboid::new(2.6, fh * 0.86, 2.6));
            let first = rig.spawn(
                along.clone(),
                hum.clone(),
                Transform::from_xyz(0.0, fh * 0.42, 24.0),
            );
            rig.commands.entity(first).insert(Pulse {
                color: palette::EERIE,
                base: 0.6,
                amp: 2.4,
                freq: TAU,
                phase: phase_of(piece.id, SALT_PULSE),
            });
            rig.spawn(
                along,
                hum.clone(),
                Transform::from_xyz(0.0, -fh * 0.42, 24.0),
            );
            rig.spawn(
                across.clone(),
                hum.clone(),
                Transform::from_xyz(fw * 0.42, 0.0, 24.0),
            );
            rig.spawn(across, hum, Transform::from_xyz(-fw * 0.42, 0.0, 24.0));
        }
        // A dun parcel lashed with twine, knot hand-tied off centre.
        Kind::MysteriousCrate => {
            rig.part(
                Cuboid::new(fw * 0.8, fh * 0.8, 18.0),
                body,
                Transform::from_xyz(0.0, 0.0, 9.0),
            );
            let twine = rig.tint(palette::mix(color, palette::SHADOW, 0.4));
            rig.part(
                Cuboid::new(fw * 0.84, 2.4, 2.0),
                twine.clone(),
                Transform::from_xyz(0.0, 0.0, 18.4),
            );
            rig.part(
                Cuboid::new(2.4, fh * 0.84, 2.0),
                twine.clone(),
                Transform::from_xyz(-fw * 0.06, 0.0, 18.4),
            );
            rig.part(ico(1.8), twine, Transform::from_xyz(-fw * 0.06, 0.0, 19.4));
        }
        // The big one. It hums a chord: a bright ring and core.
        Kind::VeryMysteriousCrate => {
            rig.part(
                Cuboid::new(fw * 0.88, fh * 0.88, 28.0),
                body,
                Transform::from_xyz(0.0, 0.0, 14.0),
            );
            let hum = glow::phosphor(rig.materials, palette::EERIE_BRIGHT, 0.8);
            let r = fw * 0.26;
            let halo = rig.part(
                Torus::new(r - 1.6, r + 1.6),
                hum.clone(),
                Transform::from_xyz(0.0, 0.0, 28.6).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            rig.commands.entity(halo).insert(Pulse {
                color: palette::EERIE_BRIGHT,
                base: 0.8,
                amp: 1.6,
                freq: 2.2,
                phase: phase_of(piece.id, SALT_PULSE),
            });
            rig.part(ico(fw * 0.09), hum, Transform::from_xyz(0.0, 0.0, 28.6));
        }
        // A shard chipped off the comet, one glint down its flank.
        Kind::CometIce => {
            rig.part(
                Cone {
                    radius: fw * 0.32,
                    height: 28.0,
                },
                body,
                Transform::from_xyz(0.0, 0.0, 14.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let shine = rig.tint(palette::GLINT);
            rig.part(
                Cuboid::new(1.6, 1.6, 12.0),
                shine,
                Transform::from_xyz(-fw * 0.12, fw * 0.06, 12.0),
            );
        }
        // A bottle of the dark between stars, corked, one star inside.
        Kind::BottledMidnight => {
            rig.part(
                Cylinder::new(fw * 0.24, 16.0),
                body.clone(),
                Transform::from_xyz(0.0, 0.0, 8.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            rig.part(
                Cylinder::new(fw * 0.1, 7.0),
                body,
                Transform::from_xyz(0.0, 0.0, 19.5).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            rig.part(
                Cylinder::new(fw * 0.13, 4.0),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, 0.0, 25.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let star = glow::phosphor(rig.materials, palette::GLINT, 4.0);
            let sx = (f32::from(piece.variant % 4) - 1.5) * 2.5;
            rig.part(ico(1.4), star, Transform::from_xyz(sx, 0.0, 9.0));
        }
        // Three overlapping cream spheres. It is looking at you.
        Kind::Fluff => {
            let shade = rig.tint(palette::mix(color, palette::SHADOW, 0.08));
            let r = fw * 0.28;
            rig.part(ico(r * 0.85), shade, Transform::from_xyz(-4.0, -2.0, 7.0));
            rig.part(
                ico(r * 0.75),
                body.clone(),
                Transform::from_xyz(4.5, -2.5, 6.5),
            );
            rig.part(ico(r), body, Transform::from_xyz(0.0, 1.5, 9.5));
            let eye = rig.meshes.add(ico(1.1));
            rig.spawn(
                eye.clone(),
                rig.skin.socket.clone(),
                Transform::from_xyz(-2.6, 5.0, 17.0),
            );
            rig.spawn(
                eye,
                rig.skin.socket.clone(),
                Transform::from_xyz(2.6, 5.0, 17.0),
            );
        }
        // Inner-ring transit papers: a flat card with the Guild's stripe.
        Kind::TransitChit => {
            rig.part(
                Cuboid::new(fw * 0.74, fh * 0.52, 5.0),
                body,
                Transform::from_xyz(0.0, 0.0, 2.5),
            );
            let stripe = rig.tint(palette::POI_GUILD);
            rig.part(
                Cuboid::new(fw * 0.12, fh * 0.52, 5.6),
                stripe,
                Transform::from_xyz(-fw * 0.2, 0.0, 2.8),
            );
        }
        // One priceless chip: a low cylinder, a rim, an inner ring.
        Kind::CasinoChip => {
            let r = fw * 0.36;
            rig.part(
                Cylinder::new(r, 9.0),
                body,
                Transform::from_xyz(0.0, 0.0, 4.5).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let rim = rig.tint(palette::mix(color, palette::SHADOW, 0.3));
            rig.part(
                Torus::new(r.mul_add(0.94, -1.3), r.mul_add(0.94, 1.3)),
                rim,
                Transform::from_xyz(0.0, 0.0, 9.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            let inner = rig.tint(palette::mix(palette::GLINT, color, 0.2));
            rig.part(
                Torus::new(r.mul_add(0.52, -1.0), r.mul_add(0.52, 1.0)),
                inner,
                Transform::from_xyz(0.0, 0.0, 9.2).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
        }
        // A hanging shade off the gantry's top rail: mount plate, stem,
        // a flattened cone shade, and the warm bulb beneath — the bulb
        // and its point light wake through `sync_fixtures`.
        Kind::CeilingLamp => {
            rig.part(
                Cuboid::new(9.0, 3.0, 5.0),
                rig.skin.plate_shade.clone(),
                Transform::from_xyz(0.0, fh * 0.44, 10.0),
            );
            rig.part(
                Cylinder::new(1.3, fh * 0.26),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, fh * 0.30, 10.0),
            );
            rig.part(
                Cone {
                    radius: fw * 0.28,
                    height: 12.0,
                },
                body,
                Transform::from_xyz(0.0, fh * 0.04, 10.0),
            );
            let root = rig.root;
            lamp_bulb(rig, piece, root, Vec3::new(0.0, -fh * 0.14, 10.0), 3.4);
        }
        // A sconce off a repossessed liner: bracket arm and mount pad
        // reaching for the nearer stile (the `WallArm` sub-root flips
        // sides with the piece's wall column), cup, bulb.
        Kind::WallLamp => {
            let arm_root = rig
                .commands
                .spawn((
                    Transform::default(),
                    Visibility::default(),
                    WallArm { piece: piece.id },
                    ChildOf(rig.root),
                ))
                .id();
            let bracket = rig.meshes.add(Cuboid::new(fw * 0.34, 3.0, 3.0));
            rig.commands.spawn((
                Mesh3d(bracket),
                MeshMaterial3d(rig.skin.plate_shade.clone()),
                Transform::from_xyz(fw * 0.24, 0.0, 10.0),
                ChildOf(arm_root),
            ));
            let pad = rig.meshes.add(Cuboid::new(3.4, 10.0, 6.0));
            rig.commands.spawn((
                Mesh3d(pad),
                MeshMaterial3d(rig.skin.plate_shade.clone()),
                Transform::from_xyz(fw * 0.42, 0.0, 10.0),
                ChildOf(arm_root),
            ));
            let cup = rig.meshes.add(Mesh::from(Cone {
                radius: fw * 0.20,
                height: 11.0,
            }));
            rig.commands.spawn((
                Mesh3d(cup),
                MeshMaterial3d(body),
                Transform::from_xyz(fw * 0.10, 0.0, 10.0)
                    .with_rotation(Quat::from_rotation_z(-FRAC_PI_2)),
                ChildOf(arm_root),
            ));
            lamp_bulb(rig, piece, arm_root, Vec3::new(-fw * 0.12, 0.0, 10.0), 3.2);
        }
        // A standing lamp bolted to the deck lip: base disc, pole, the
        // shade up top with its bulb tucked under.
        Kind::FloorLamp => {
            rig.part(
                Cylinder::new(fw * 0.26, 3.2),
                rig.skin.plate_shade.clone(),
                Transform::from_xyz(0.0, -fh * 0.41, 2.4)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
            rig.part(
                Cylinder::new(1.3, fh * 0.72),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, -fh * 0.04, 6.0),
            );
            rig.part(
                Cone {
                    radius: fw * 0.30,
                    height: 13.0,
                },
                body,
                Transform::from_xyz(0.0, fh * 0.33, 11.0),
            );
            let root = rig.root;
            lamp_bulb(rig, piece, root, Vec3::new(0.0, fh * 0.21, 11.0), 3.4);
        }
        // Somebody's living room, in transit: seat slab, back rest, arm
        // cubes, cushion bumps, stubby feet — upholstery hue, dim shading.
        // A NOTE ON DEPTH, learned from this couch: rigs began as
        // desk-era bas-reliefs, +Z meaning relief height — biggest
        // features closest to the viewer. Standing rigs re-purpose +Z
        // as ROOM depth (bay_site keeps local +Z toward the player), so
        // any furniture with an asymmetric depth story must be composed
        // truly: backs near z = 0 against the wall, seats and open
        // fronts reaching +Z. Symmetric rigs (lamps, tins, crates)
        // never notice; a couch authored as relief faces backwards.
        Kind::Couch => {
            let under = rig.tint(palette::mix(color, palette::SHADOW, 0.3));
            // The backrest stands at the wall side; the seat deck runs
            // out into the room, cushions on top, arms full depth.
            rig.part(
                Cuboid::new(fw * 0.74, fh * 0.56, 5.0),
                body.clone(),
                Transform::from_xyz(0.0, fh * 0.16, 2.5),
            );
            rig.part(
                Cuboid::new(fw * 0.74, fh * 0.30, 18.0),
                under,
                Transform::from_xyz(0.0, -fh * 0.20, 10.0),
            );
            let cushion = rig.meshes.add(ico(6.0));
            for side in [-1.0, 1.0] {
                rig.spawn(
                    cushion.clone(),
                    body.clone(),
                    Transform::from_xyz(fw * 0.17 * side, -fh * 0.02, 10.0)
                        .with_scale(Vec3::new(1.5, 0.7, 1.1)),
                );
            }
            let arm = rig.meshes.add(Cuboid::new(fw * 0.10, fh * 0.54, 16.0));
            for side in [-1.0, 1.0] {
                rig.spawn(
                    arm.clone(),
                    body.clone(),
                    Transform::from_xyz(fw * 0.42 * side, -fh * 0.04, 8.0),
                );
            }
            let foot = rig.meshes.add(Cuboid::new(4.0, 5.0, 4.0));
            for (side, fz) in [(-1.0, 3.0), (1.0, 3.0), (-1.0, 16.0), (1.0, 16.0)] {
                rig.spawn(
                    foot.clone(),
                    rig.skin.plate_shade.clone(),
                    Transform::from_xyz(fw * 0.36 * side, -fh * 0.44, fz),
                );
            }
        }
        // Gilt frame, subject debatable: a backing slab, raised frame
        // lips, and the canvas — one seeded artwork painted through the
        // shared rasterizer, emissive so low it reads as paint.
        Kind::Painting => {
            let backing = rig.tint(palette::mix(color, palette::SHADOW, 0.35));
            rig.part(
                Cuboid::new(fw * 0.82, fh * 0.74, 5.0),
                backing,
                Transform::from_xyz(0.0, 0.0, 2.5),
            );
            let lip_h = rig.meshes.add(Cuboid::new(fw * 0.78, 3.2, 4.0));
            let lip_v = rig.meshes.add(Cuboid::new(3.2, fh * 0.66, 4.0));
            for (mesh, at) in [
                (lip_h.clone(), Vec3::new(0.0, fh * 0.315, 5.4)),
                (lip_h, Vec3::new(0.0, -fh * 0.315, 5.4)),
                (lip_v.clone(), Vec3::new(fw * 0.35, 0.0, 5.4)),
                (lip_v, Vec3::new(-fw * 0.35, 0.0, 5.4)),
            ] {
                rig.spawn(mesh, body.clone(), Transform::from_translation(at));
            }
            let art = paint_artwork(rig.images, rig.materials, piece.id);
            rig.part(
                Rectangle::new(1.0, 1.0),
                art,
                Transform::from_xyz(0.0, 0.0, 5.15).with_scale(Vec3::new(
                    fw * 0.68,
                    fh * 0.58,
                    1.0,
                )),
            );
        }
        // Furniture that stores (docs/BAY.md): a slim wardrobe in oiled
        // oak, brass where hands go, its front open so the cubby rack —
        // and everything stowed in it — stays visible. The 2×2 rack's
        // interiors are socket-dark so the shrunken cargo reads against
        // them; a hidden amber quad in each mouth answers the sim's stow
        // invitation through `invite_glows`. Stowed pieces themselves are
        // ordinary rigs parked at [`cubby_anchor`]s by `sync_pieces`.
        Kind::Cabinet => {
            let deep = CABINET_DEPTH;
            let rack = rig.tint(palette::mix(color, palette::SHADOW, 0.25));
            // Carcass: the back sheet alone owns the rear plane, and
            // every part meeting it starts INSIDE it — joints between
            // rig solids interpenetrate, never kiss, because two faces
            // sharing a plane shimmer (the twice-caught cabinet: first
            // its rear, then the plane its "fixed" parts abutted at).
            rig.part(
                Cuboid::new(fw * 0.96, fh * 0.97, 2.0),
                body.clone(),
                Transform::from_xyz(0.0, 0.0, 1.0),
            );
            let side = rig.meshes.add(Cuboid::new(2.6, fh * 0.94, deep - 1.0));
            for sx in [-1.0, 1.0] {
                rig.spawn(
                    side.clone(),
                    body.clone(),
                    Transform::from_xyz(fw * 0.44 * sx, 0.0, (deep + 1.0) * 0.5),
                );
            }
            let cap = rig.meshes.add(Cuboid::new(fw * 0.92, 2.6, deep - 1.0));
            for sy in [-1.0, 1.0] {
                rig.spawn(
                    cap.clone(),
                    body.clone(),
                    Transform::from_xyz(0.0, fh * 0.465 * sy, (deep + 1.0) * 0.5),
                );
            }
            // The rack: mid shelf and centre stile, a shade darker,
            // rooted inside the back sheet like everything else.
            rig.part(
                Cuboid::new(fw * 0.88, 2.2, deep * 0.85),
                rack.clone(),
                Transform::from_xyz(0.0, 0.0, (deep * 0.85).mul_add(0.5, 1.5)),
            );
            rig.part(
                Cuboid::new(2.2, fh * 0.9, deep * 0.85),
                rack,
                Transform::from_xyz(0.0, 0.0, (deep * 0.85).mul_add(0.5, 1.5)),
            );
            // Brass fittings: a cornice over the opening, stubby feet.
            rig.part(
                Cuboid::new(fw * 0.96, 2.2, 2.2),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, fh * 0.475, deep),
            );
            let foot = rig.meshes.add(Cuboid::new(3.0, 3.4, 3.0));
            for (sx, fz) in [
                (-1.0, 3.0),
                (1.0, 3.0),
                (-1.0, deep - 3.0),
                (1.0, deep - 3.0),
            ] {
                rig.spawn(
                    foot.clone(),
                    rig.skin.brass.clone(),
                    Transform::from_xyz(fw * 0.36 * sx, fh.mul_add(-0.5, 1.2), fz),
                );
            }
            // The cubbies: dark interior backs, invite glows in front.
            let lining = rig.meshes.add(Cuboid::new(fw * 0.36, fh * 0.4, 1.2));
            let mouth = rig.meshes.add(Cuboid::new(fw * 0.33, fh * 0.37, 0.6));
            for slot in 0..CABINET_SLOTS {
                let at = cubby_anchor(slot);
                rig.spawn(
                    lining.clone(),
                    rig.skin.socket.clone(),
                    Transform::from_xyz(at.x, at.y, 2.2),
                );
                let invite = glow::phosphor(rig.materials, palette::AMBER, 0.0);
                rig.commands.spawn((
                    Mesh3d(mouth.clone()),
                    MeshMaterial3d(invite),
                    Transform::from_xyz(at.x, at.y, 3.0),
                    Visibility::Hidden,
                    CubbyGlow {
                        piece: piece.id,
                        slot,
                        phase: f32::from(slot) * 1.3,
                    },
                    ChildOf(rig.root),
                ));
            }
        }
        // The exterior window: a brass frame around the ship's one real
        // sky — the viewport's painted glass rides this piece wherever
        // it is rehung (the whimsy rule made physical; the void
        // follows). Headless paths fall back to a phosphor pane with a
        // stand-in star scatter seeded by the piece id.
        Kind::Window => {
            if let Some(image) = rig.sky_image.clone() {
                let glass = crate::crt::tube_glass(rig.materials, &image);
                rig.part(
                    Rectangle::new(1.0, 1.0),
                    glass,
                    Transform::from_xyz(0.0, 0.0, 2.4).with_scale(Vec3::new(
                        fw * 0.88,
                        fh * 0.78,
                        1.0,
                    )),
                );
            } else {
                let pane = glow::phosphor(
                    rig.materials,
                    palette::mix(color, palette::PHOSPHOR, 0.12),
                    0.35,
                );
                rig.part(
                    Cuboid::new(fw * 0.88, fh * 0.78, 3.0),
                    pane,
                    Transform::from_xyz(0.0, 0.0, 1.5),
                );
                let star = glow::phosphor(rig.materials, palette::GLINT, 2.2);
                let fleck = rig.meshes.add(ico(0.9));
                for i in 0..7_u32 {
                    let n = (piece.id.wrapping_mul(7).wrapping_add(i)) as f32;
                    let angle = n * 2.399;
                    let reach = (n * 0.517).fract().mul_add(0.36, 0.08);
                    rig.spawn(
                        fleck.clone(),
                        star.clone(),
                        Transform::from_xyz(
                            angle.cos() * fw * reach,
                            angle.sin() * fh * reach,
                            2.6,
                        ),
                    );
                }
            }
            let lip_h = rig.meshes.add(Cuboid::new(fw * 0.96, fh * 0.10, 6.0));
            let lip_v = rig.meshes.add(Cuboid::new(fw * 0.05, fh * 0.94, 6.0));
            for (mesh, at) in [
                (lip_h.clone(), Vec3::new(0.0, fh * 0.43, 3.0)),
                (lip_h, Vec3::new(0.0, -fh * 0.43, 3.0)),
                (lip_v.clone(), Vec3::new(fw * 0.455, 0.0, 3.0)),
                (lip_v, Vec3::new(-fw * 0.455, 0.0, 3.0)),
            ] {
                rig.spawn(
                    mesh,
                    rig.skin.brass.clone(),
                    Transform::from_translation(at),
                );
            }
        }
        // The chart tank: the star map's phosphor aquarium, off the
        // wall at last. Dark glass in a brass chassis over a plinth,
        // the chart glowing on its own (vital instruments must read
        // lights-out), and the amber carry grab at its base — the
        // handle rule's move affordance (BAY.md, "The handle rule").
        Kind::ChartTank => {
            rig.part(
                Cuboid::new(fw * 0.92, fh * 0.90, 3.0),
                rig.skin.plate_shade.clone(),
                Transform::from_xyz(0.0, 0.0, 1.5),
            );
            let void = rig.tint(palette::mix(color, palette::SHADOW, 0.82));
            rig.part(
                Cuboid::new(fw * 0.86, fh * 0.82, 9.0),
                void,
                Transform::from_xyz(0.0, 0.0, 6.0),
            );
            // The chart itself: the CRT's painted map rides the tank's
            // glass, proud of the void slab so it actually shows.
            if let Some(image) = rig.map_image.clone() {
                let glass = crate::crt::tube_glass(rig.materials, &image);
                rig.part(
                    Rectangle::new(1.0, 1.0),
                    glass,
                    Transform::from_xyz(0.0, 0.0, 11.0).with_scale(Vec3::new(
                        fw * 0.78,
                        fh * 0.72,
                        1.0,
                    )),
                );
            } else {
                let field = glow::phosphor(rig.materials, color, 1.4);
                rig.part(
                    Cuboid::new(fw * 0.78, fh * 0.72, 1.6),
                    field,
                    Transform::from_xyz(0.0, 0.0, 11.0),
                );
            }
            let post = rig.meshes.add(Cuboid::new(3.0, fh * 0.9, 11.0));
            for sx in [-1.0f32, 1.0] {
                rig.spawn(
                    post.clone(),
                    rig.skin.brass.clone(),
                    Transform::from_xyz(sx * fw * 0.44, 0.0, 6.0),
                );
            }
            let cap = rig.meshes.add(Cuboid::new(fw * 0.92, 3.0, 11.0));
            for sy in [-1.0f32, 1.0] {
                rig.spawn(
                    cap.clone(),
                    rig.skin.brass.clone(),
                    Transform::from_xyz(0.0, sy * fh * 0.43, 6.0),
                );
            }
            carry_grab(rig, piece.kind, fw, fh, 12.5);
        }
        // The ETA gauge: a brass drum with a dark dial, the phosphor
        // needle reading the live leg ([`eta_needles`] sweeps it).
        // Passive — it earns no amber handle.
        Kind::EtaGauge => {
            let flat = Quat::from_rotation_x(FRAC_PI_2);
            rig.part(
                Cylinder::new(fw * 0.42, 6.0),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, 0.0, 3.0).with_rotation(flat),
            );
            let dial = rig.tint(palette::mix(color, palette::SHADOW, 0.7));
            rig.part(
                Cylinder::new(fw * 0.35, 3.0),
                dial,
                Transform::from_xyz(0.0, 0.0, 5.5).with_rotation(flat),
            );
            let needle = glow::phosphor(rig.materials, color, 1.6);
            let arm = rig.part(
                Cuboid::new(2.0, fh * 0.28, 1.6),
                needle,
                Transform::from_xyz(0.0, fh * 0.14, 7.2),
            );
            rig.commands
                .entity(arm)
                .insert(EtaNeedle { reach: fh * 0.14 });
            let hub = glow::etched(rig.materials, palette::GLINT);
            rig.part(ico(1.8), hub, Transform::from_xyz(0.0, 0.0, 7.4));
        }
        // The destination preview: a square brass porthole wearing the
        // CRT's painted preview — the selected world's face rides the
        // piece now, not the console. Passive glass; headless paths
        // show a lone phosphor disc instead.
        Kind::DestPreview => {
            rig.part(
                Cuboid::new(fw * 0.84, fh * 0.84, 4.0),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, 0.0, 2.0),
            );
            if let Some(image) = rig.preview_image.clone() {
                let glass = crate::crt::tube_glass(rig.materials, &image);
                rig.part(
                    Rectangle::new(1.0, 1.0),
                    glass,
                    Transform::from_xyz(0.0, 0.0, 4.6).with_scale(Vec3::new(
                        fw * 0.68,
                        fh * 0.68,
                        1.0,
                    )),
                );
            } else {
                let glass = rig.tint(palette::mix(color, palette::SHADOW, 0.72));
                rig.part(
                    Cuboid::new(fw * 0.68, fh * 0.68, 3.0),
                    glass,
                    Transform::from_xyz(0.0, 0.0, 3.5),
                );
                let world = glow::phosphor(rig.materials, color, 1.5);
                rig.part(ico(fw * 0.14), world, Transform::from_xyz(0.0, 0.0, 5.6));
            }
        }
        // The launch handle: a shade plate, the brass quadrant slot,
        // and the pull arm reaching into the room, its knob wearing the
        // go-lamp green (the FUNCTION, like the console handle it will
        // absorb) — while the amber carry grab below is the MOVE
        // affordance, per the handle rule. Both glow enough that the
        // one lever that commits a course is findable in the dark.
        Kind::LaunchLever => {
            rig.part(
                Cuboid::new(fw * 0.72, fh * 0.88, 3.0),
                rig.skin.plate_shade.clone(),
                Transform::from_xyz(0.0, 0.0, 1.5),
            );
            rig.part(
                Cuboid::new(fw * 0.14, fh * 0.72, 4.0),
                rig.skin.brass.clone(),
                Transform::from_xyz(0.0, 0.0, 3.2),
            );
            let arm = rig.tint(palette::mix(color, palette::SHADOW, 0.35));
            rig.part(
                Cuboid::new(3.2, fh * 0.52, 3.2),
                arm,
                Transform::from_xyz(0.0, -fh * 0.06, 8.0)
                    .with_rotation(Quat::from_rotation_x(-0.5)),
            );
            let knob = glow::phosphor(rig.materials, palette::LAMP_OK, 0.9);
            rig.part(ico(3.4), knob, Transform::from_xyz(0.0, fh * 0.16, 11.5));
            carry_grab(rig, piece.kind, fw, fh, 6.0);
        }
        // The dressing kinds own two bodies each — laid into the room
        // versus rolled or canned for the counter — and
        // [`sync_dressings`] shows exactly one, by the sim's berth class.
        Kind::Rug => {
            let border = rig.tint(palette::mix(color, palette::SHADOW, 0.3));
            // [`RUG_THICK`] is a world measure; rigs build in sim units,
            // so the pile converts through the bay's cell scale.
            let pile = RUG_THICK / (crate::rig::BAY_CELL / layout::CELL);
            let home = rig.root;
            rig.root = dress_form(rig, piece, home, true);
            // The pile over a darker binding: the border reads woven at
            // a glance, and the fringe knots the short ends.
            rig.part(
                Cuboid::new(fw * 0.98, fh * 0.96, pile * 0.7),
                border.clone(),
                Transform::from_xyz(0.0, 0.0, pile * 0.35),
            );
            rig.part(
                Cuboid::new(fw * 0.90, fh * 0.86, pile),
                body.clone(),
                Transform::from_xyz(0.0, 0.0, pile * 0.75),
            );
            let tassel = rig.meshes.add(Cuboid::new(2.4, 3.6, 0.5));
            for sx in [-1.0f32, 1.0] {
                for i in 0..5 {
                    rig.spawn(
                        tassel.clone(),
                        border.clone(),
                        Transform::from_xyz(sx * fw * 0.465, (i as f32 - 2.0) * 6.0, 0.4),
                    );
                }
            }
            rig.root = dress_form(rig, piece, home, false);
            // Rolled for the counter: a tied bolt of weave, brass bands.
            let across = Quat::from_rotation_z(FRAC_PI_2);
            rig.part(
                Cylinder::new(fh * 0.26, fw * 0.88),
                body,
                Transform::from_xyz(0.0, 0.0, fh * 0.26).with_rotation(across),
            );
            let band = rig.meshes.add(Cylinder::new(fh * 0.28, 2.2));
            for sx in [-1.0f32, 1.0] {
                rig.spawn(
                    band.clone(),
                    rig.skin.brass.clone(),
                    Transform::from_xyz(sx * fw * 0.26, 0.0, fh * 0.26).with_rotation(across),
                );
            }
            rig.root = home;
        }
        Kind::PaintTin => {
            let coat = rig.tint(palette::enamel_color(piece.variant));
            let home = rig.root;
            rig.root = dress_form(rig, piece, home, true);
            // The coat: enamel a hair inside the cell so the berth edge
            // still reads, with one streak the painter didn't chase.
            rig.part(
                Cuboid::new(fw * 0.84, fh * 0.84, 0.4),
                coat.clone(),
                Transform::from_xyz(0.0, 0.0, 0.2),
            );
            let streak = rig.tint(palette::mix(
                palette::enamel_color(piece.variant),
                palette::GLINT,
                0.14,
            ));
            rig.part(
                Cuboid::new(fw * 0.6, 2.6, 0.3),
                streak,
                Transform::from_xyz(-1.5, 3.0, 0.45).with_rotation(Quat::from_rotation_z(0.16)),
            );
            rig.root = dress_form(rig, piece, home, false);
            // Canned: a squat battered tin, the lid wearing its color.
            tin(rig, body, coat);
            rig.root = home;
        }
        Kind::LuminousPaint => {
            let glow_hue = palette::mix(color, palette::PHOSPHOR, 0.35);
            let mat = glow::phosphor(rig.materials, glow_hue, 0.0);
            let home = rig.root;
            rig.root = dress_form(rig, piece, home, true);
            // The coat's glass, plus the real tinge beneath it — both fed
            // by [`sync_dressings`] exactly as the lamps are fed.
            rig.part(
                Cuboid::new(fw * 0.84, fh * 0.84, 0.4),
                mat.clone(),
                Transform::from_xyz(0.0, 0.0, 0.2),
            );
            rig.commands.spawn((
                PointLight {
                    color: glow_hue,
                    intensity: 0.0,
                    range: COAT_RANGE,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 6.0),
                Dimmable { intensity: 0.0 },
                CoatGlow {
                    piece: piece.id,
                    color: glow_hue,
                    mat: mat.clone(),
                    level: 0.0,
                },
                ChildOf(rig.root),
            ));
            rig.root = dress_form(rig, piece, home, false);
            // Canned: the blackout tin — dark body, the lid's glass dark
            // until laid (it shares the coat's instance, and the level
            // stays floored while packed).
            let blackout = rig.tint(palette::mix(color, palette::SHADOW, 0.55));
            tin(rig, blackout, mat);
            rig.root = home;
        }
    }
}

/// A sub-root for one of a covering's two bodies; [`sync_dressings`]
/// shows exactly one per piece.
fn dress_form(rig: &mut RigParts, piece: &Piece, home: Entity, laid: bool) -> Entity {
    rig.commands
        .spawn((
            Transform::IDENTITY,
            Visibility::Hidden,
            DressForm {
                piece: piece.id,
                laid,
            },
            ChildOf(home),
        ))
        .id()
}

/// A squat paint tin under the current rig root: `shell` for the body,
/// `lid` capping it — the one silhouette both paints share.
fn tin(rig: &mut RigParts, shell: Handle<StandardMaterial>, lid: Handle<StandardMaterial>) {
    let upright = Quat::from_rotation_x(FRAC_PI_2);
    rig.part(
        Cylinder::new(9.5, 11.0),
        shell,
        Transform::from_xyz(0.0, 0.0, 5.5).with_rotation(upright),
    );
    rig.part(
        Cylinder::new(9.7, 1.6),
        lid,
        Transform::from_xyz(0.0, 0.0, 11.4).with_rotation(upright),
    );
}

/// A lamp's live bulb: dark glass that wakes warm, plus the real point
/// light beneath it. The light spawns dark ([`Dimmable`] base 0) and
/// [`sync_fixtures`] eases both toward `lamp_lit` — no shadow maps, per
/// the art direction; the pool of light is the point.
fn lamp_bulb(rig: &mut RigParts, piece: &Piece, parent: Entity, at: Vec3, radius: f32) {
    let color = palette::mix(palette::kind_color(piece.kind), palette::GLINT, 0.35);
    let mat = glow::phosphor(rig.materials, color, 0.0);
    let bulb = rig.meshes.add(ico(radius));
    rig.commands.spawn((
        Mesh3d(bulb),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(at),
        ChildOf(parent),
    ));
    let range = if piece.kind == Kind::CeilingLamp {
        CEILING_RANGE
    } else {
        LAMP_RANGE
    };
    rig.commands.spawn((
        PointLight {
            color,
            intensity: 0.0,
            range,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(at),
        Dimmable { intensity: 0.0 },
        LampGlow {
            piece: piece.id,
            color,
            mat,
            level: 0.0,
        },
        ChildOf(parent),
    ));
}

/// The artwork half of a `Painting`: a 24×16-texel canvas painted once
/// through the shared [`canvas::Canvas`] at its own scale, then baked
/// into an emissive texture. Seeded by the piece id over [`SALT_ART`] —
/// the same picture every boot — choosing one of four archetypes, all
/// palette-derived inks, at [`ART_GLOW`] so gallery light (the lamps)
/// matters more than the paint's own faint glow.
#[allow(clippy::too_many_lines)] // four archetypes, one gallery
fn paint_artwork(
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    id: u32,
) -> Handle<StandardMaterial> {
    let field = Rect::new(0.0, 0.0, ART_W, ART_H);
    let mut cv = canvas::Canvas::new(field);
    let roll = splitmix(u64::from(id), SALT_ART);
    let bits = |shift: u64, span: u64| ((roll >> shift) % span) as f32;
    match roll % 4 {
        // A horizon under a low sun.
        0 => {
            cv.fill(
                field,
                canvas::mix(
                    canvas::ink(palette::POI_NEPTUNE),
                    canvas::ink(palette::SHADOW),
                    0.45,
                ),
            );
            let horizon = bits(8, 5).mul_add(2.0, 16.0);
            cv.fill(
                Rect::new(0.0, horizon, ART_W, ART_H - horizon),
                canvas::mix(
                    canvas::ink(palette::POI_HERMITAGE),
                    canvas::ink(palette::SHADOW),
                    0.35,
                ),
            );
            cv.fill(
                Rect::new(0.0, horizon, ART_W, 2.0),
                canvas::fade(canvas::ink(palette::AMBER), 0.4),
            );
            let sun = SimVec2::new(bits(16, 8).mul_add(3.0, 12.0), horizon - 6.0);
            cv.dot(sun, 4.0, canvas::ink(palette::AMBER));
            cv.dot(sun, 2.0, canvas::ink(palette::GLINT));
        }
        // Diagonal stripes, hues rolled per band.
        1 => {
            cv.fill(
                field,
                canvas::mix(
                    canvas::ink(palette::TRIM_GIVE),
                    canvas::ink(palette::SHADOW),
                    0.4,
                ),
            );
            let inks = [
                palette::AMBER,
                palette::EERIE,
                palette::POI_MARS,
                palette::GLINT,
            ];
            for i in 0..4_u64 {
                let col = canvas::ink(inks[((roll >> (8 + i * 4)) % 4) as usize]);
                let x = (i as f32).mul_add(12.0, bits(40, 4) - 6.0);
                cv.seg(
                    SimVec2::new(x, ART_H + 4.0),
                    SimVec2::new(x + 18.0, -4.0),
                    4.0,
                    canvas::fade(col, 0.85),
                );
            }
        }
        // An orb over bands.
        2 => {
            for (i, tone) in [
                palette::TRIM_TAKE,
                palette::TRIM_SHELF,
                palette::TRIM_RECEIVED,
                palette::TRIM_GIVE,
            ]
            .into_iter()
            .enumerate()
            {
                cv.fill(
                    Rect::new(0.0, i as f32 * 8.0, ART_W, 8.0),
                    canvas::mix(canvas::ink(tone), canvas::ink(palette::SHADOW), 0.3),
                );
            }
            let orb = SimVec2::new(bits(8, 10).mul_add(2.0, 14.0), 13.0);
            cv.dot(orb, 7.0, canvas::ink(palette::POI_URANUS));
            cv.ring(
                orb,
                9.0,
                1.0,
                canvas::fade(canvas::ink(palette::GLINT), 0.7),
            );
        }
        // A lone heptagon. The officially published schematics show six.
        _ => {
            cv.fill(
                field,
                canvas::mix(
                    canvas::ink(palette::EERIE),
                    canvas::ink(palette::SHADOW),
                    0.72,
                ),
            );
            let at = SimVec2::new(24.0, 16.0);
            let spin = bits(8, 90) * 4.0;
            cv.poly(at, 7, 10.0, spin, canvas::ink(palette::POI_GUILD));
            cv.poly_ring(
                at,
                7,
                10.0,
                spin,
                1.0,
                canvas::ink(palette::accent::GUILD_EDGE),
            );
            cv.dot(at, 1.5, canvas::fade(canvas::ink(palette::GLINT), 0.8));
        }
    }
    let (w, h) = (cv.w, cv.h);
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        cv.px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let image = images.add(image);
    materials.add(StandardMaterial {
        base_color: palette::GLINT,
        base_color_texture: Some(image.clone()),
        emissive: LinearRgba::WHITE * ART_GLOW,
        emissive_texture: Some(image),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig;

    fn chart(want: Station) -> SimSurface {
        rig::bay()
            .into_iter()
            .find(|(station, _)| *station == want)
            .map(|(_, surface)| surface)
            .expect("chart spawned")
    }

    fn rect_of(x: u8, y: u8, kind: Kind) -> Rect {
        let (w, h) = kind.cells();
        let anchor = layout::cell_rect(x, y);
        Rect::new(
            anchor.x,
            anchor.y,
            f32::from(w) * layout::CELL,
            f32::from(h) * layout::CELL,
        )
    }

    /// The net mapping's regimes: wall cells hang flat on their chart's
    /// plane, floor cargo stands upright at furniture scale keeping its
    /// bas-relief height, and the side walls place at their own planes.
    #[test]
    fn net_sites_hang_and_stand_per_chart() {
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        let port = chart(Station::BayPort);
        // A painting on the aft chart: flat against the aft wall plane,
        // facing into the room.
        let (pos, rot, _) = site_on(Station::BayWall, &aft, &aft, rect_of(4, 1, Kind::Painting));
        assert!(
            (pos.z - aft.center.z).abs() < 1e-4,
            "wall piece left the wall plane: {pos}"
        );
        assert!(
            (rot * Vec3::Z).z < -0.9,
            "aft cargo must face into the room"
        );
        // A couch on the floor: upright (local +Y is world up), feet on
        // the plates, spanning about its two 0.55 cells.
        let couch = rect_of(4, 4, Kind::Couch);
        let (pos, rot, scale) = site_on(Station::BayFloor, &floor, &aft, couch);
        assert!(
            (rot * Vec3::Y - Vec3::Y).length() < 1e-4,
            "standing rigs must be upright"
        );
        let base = couch.h.mul_add(-0.5 * scale.y, pos.y);
        assert!(base.abs() < 0.05, "the couch floats: base at {base}");
        let width = couch.w * scale.x;
        assert!((0.95..=1.15).contains(&width), "couch width {width}");
        // A wall lamp on the port chart: at the port plane, facing
        // starboard (+X is into the room from port).
        let (pos, rot, _) = site_on(Station::BayPort, &port, &aft, rect_of(1, 4, Kind::WallLamp));
        assert!(
            (pos.x - port.center.x).abs() < 1e-4,
            "port piece left its wall plane: {pos}"
        );
        assert!(
            (rot * Vec3::Z).x > 0.9,
            "port cargo must face into the room"
        );
    }

    /// The backing rule: seam contact turns a standing rig's back to
    /// the wall, quarter turns only where the plan allows them, and
    /// mid-floor cargo faces the front of the room.
    #[test]
    fn the_backing_rule_turns_floor_rigs() {
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        let facing = |x: u8, y: u8, kind: Kind| {
            let (_, rot, _) = site_on(Station::BayFloor, &floor, &aft, rect_of(x, y, kind));
            assert!(
                (rot * Vec3::Y - Vec3::Y).length() < 1e-4,
                "the backing rule must keep rigs upright"
            );
            rot * Vec3::Z
        };
        // Mid-floor: face the front of the room, toward the user.
        assert!(facing(5, 4, Kind::Couch).z < -0.9, "mid-floor faces front");
        // Against the front gutter: back to the front wall.
        assert!(
            facing(3, 7, Kind::Couch).z > 0.9,
            "front-row cargo turns its back to the front wall"
        );
        // A one-column piece against the port seam backs onto it.
        assert!(
            facing(3, 4, Kind::FloorLamp).x > 0.9,
            "a port-seam lamp faces starboard"
        );
        // A two-wide couch cannot lie along the port wall: the quarter
        // turn would leave its cells, so the default stands.
        assert!(
            facing(3, 5, Kind::Couch).z < -0.9,
            "an incompatible seam keeps the default facing"
        );
        // The aft seam wins the corner: back to the aft wall reads as
        // the flush default (facing front).
        assert!(
            facing(3, 3, Kind::FloorLamp).z < -0.9,
            "the aft corner backs onto the aft wall"
        );
    }

    /// The upright rule: square wall cargo reads up-is-up on every
    /// wall — the side charts' vertical columns must not turn the star
    /// chart sideways — while facing stays into the room and portrait
    /// footprints keep the chart's own lie.
    #[test]
    fn square_wall_cargo_hangs_upright() {
        let aft = chart(Station::BayWall);
        for (station, x, y) in [
            (Station::BayWall, 4, 0),
            (Station::BayPort, 0, 4),
            (Station::BayStarboard, 9, 5),
            (Station::BayFront, 4, 8),
        ] {
            let surface = chart(station);
            let (_, rot, _) = site_on(station, &surface, &aft, rect_of(x, y, Kind::ChartTank));
            assert!(
                (rot * Vec3::Y).y > 0.9,
                "{station:?}: the tank's up must be world up, got {:?}",
                rot * Vec3::Y
            );
            let inward = station.inward(&surface);
            assert!(
                (rot * Vec3::Z).dot(inward) > 0.9,
                "{station:?}: the tank must still face into the room"
            );
        }
        // A portrait footprint on a side wall lies as its cells lie.
        let port = chart(Station::BayPort);
        let (_, rot, _) = site_on(Station::BayPort, &port, &aft, rect_of(0, 4, Kind::Painting));
        assert!(
            (rot * Vec3::Y).y.abs() < 0.1,
            "a 2x1 painting on the port wall hangs portrait with its cells"
        );
    }

    /// Cubby anchors follow `layout::cubby_rect`'s row-major order from
    /// the top-left, seen facing the open front.
    #[test]
    fn cubby_anchors_match_the_sim_rack() {
        // Slot 0 top-left … 3 bottom-right; local +X is sim +x, +Y up.
        assert!(cubby_anchor(0).x < cubby_anchor(1).x);
        assert!(cubby_anchor(2).x < cubby_anchor(3).x);
        assert!(cubby_anchor(0).y > cubby_anchor(2).y);
        assert!(cubby_anchor(1).y > cubby_anchor(3).y);
        // And the sim agrees which sub-rect is which: cubby 0 sits
        // up-left of cubby 3 in sim coordinates (sim +y runs down).
        let body = Rect::new(0.0, 0.0, layout::CELL, 2.0 * layout::CELL);
        let c0 = layout::cubby_rect(body, 0);
        let c3 = layout::cubby_rect(body, 3);
        assert!(c0.x < c3.x && c0.y < c3.y);
    }

    /// A stowed rig fits its cubby: the widest 1×1 footprint at stow
    /// scale stays inside ~0.18 world units, without vanishing.
    #[test]
    fn stowed_pieces_shrink_to_the_cubby() {
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        let (_, _, scale) = site_on(
            Station::BayFloor,
            &floor,
            &aft,
            rect_of(4, 4, Kind::Cabinet),
        );
        let extent = layout::CELL * scale.min_element() * STOW_FIT;
        assert!(extent <= 0.19, "stowed extent {extent}");
        assert!(extent >= 0.12, "stowed cargo should stay visible: {extent}");
    }

    /// Every violation names its presentation: a glyph, or (bounds,
    /// overlap, the violet objection) the bare frame — and no glyph
    /// outgrows the bar pool.
    #[test]
    fn glyphs_cover_the_violation_ladder() {
        let rect = rect_of(5, 4, Kind::PerfumeVial);
        for rule in [
            Violation::Bounds,
            Violation::Overlap,
            Violation::Volatile,
            Violation::Cryo,
            Violation::Suspicious,
            Violation::Affix(Mount::Ceiling),
            Violation::Affix(Mount::Floor),
            Violation::Affix(Mount::Wall),
            Violation::Occupied,
            Violation::Aisle,
            Violation::Sealed,
            Violation::Vital,
        ] {
            let bars = glyph_spec(Some(rule), rect);
            assert!(
                bars.len() <= usize::from(GLYPH_BARS),
                "{rule:?} overflows the pool"
            );
            let frame_only = matches!(
                rule,
                Violation::Bounds
                    | Violation::Overlap
                    | Violation::Suspicious
                    | Violation::Aisle
                    | Violation::Sealed
                    | Violation::Vital
            );
            assert_eq!(bars.is_empty(), frame_only, "{rule:?}");
        }
        assert!(glyph_spec(None, rect).is_empty());
    }

    /// The z-fight guard: every occupied rung of the decal ladder —
    /// including the rug's pile top, which rides between LAID and HINT —
    /// steps at least `layer::STEP` from its neighbours, and the step
    /// itself clears two skins of mesh. A new decal gets a named rung
    /// and a row here, or it shimmers like the playtest doormat did.
    #[test]
    fn the_decal_ladder_never_z_fights() {
        use crate::rig::layer;
        let rungs = [
            // The nearest actual mesh under the ladder: the backer
            // slab's face, 0.004 behind the chart's mapping plane.
            ("backer face", -0.004),
            ("tile", layer::TILE),
            ("doormat", layer::DOORMAT),
            ("laid", layer::LAID),
            ("rug pile top", LAID_LIFT + RUG_THICK),
            ("hint", layer::HINT),
            ("slash", layer::SLASH),
            ("flash", layer::FLASH),
            ("glyph", layer::GLYPH),
        ];
        for pair in rungs.windows(2) {
            let ((below, lo), (above, hi)) = (pair[0], pair[1]);
            assert!(
                hi - lo >= layer::STEP - 1e-6,
                "{above} ({hi}) sits within a fight of {below} ({lo})"
            );
        }
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(
                layer::STEP >= 2.0 * layer::SKIN,
                "a ladder step must clear two skins of mesh"
            );
        }
    }
}
