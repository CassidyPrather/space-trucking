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
use space_trucking::sim::layout::{self, Rect};
use space_trucking::sim::room::{CABIN, RoomId, Rooms};
#[allow(unused_imports)]
use space_trucking::sim::{};
use space_trucking::sim::{
    Cue, Kind, Loc, Mount, Piece, ShipState, Vec2 as SimVec2, Violation, cargo, lamp_lit,
    lit_adjacent, placement_check, player_owned, splitmix,
};

use crate::poi::{Coat, Shape, Worn};
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

/// How much bigger than its berth the aim-anchored hover draws a carried
/// piece: it stands at the DESTINATION rather than at the face, so it
/// wears full scale and a shade over, which is what says "this is the
/// thing, and it is not landed yet".
const HOVER_FIT: f32 = 1.1;

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

/// Fraction of its cells a bay rig fills. Roomier than the desk's [`FIT`]:
/// furniture nearly fills its berth — a couch reads ~1.06 world units
/// wide over its two 0.55 cells.
pub const BAY_FIT: f32 = 0.96;

/// **How deep every rig is drawn**, in rig-local sim units: the near
/// face just behind the berth plane, the far face one CELL out from it.
/// Written down here because three things read it — the carry tell's
/// wireframe box, which wraps the body's volume; the gauntlet, which
/// asks how much air a berth actually spends so a station's furniture
/// can be told it is standing in one (`crate::gauntlet`); and the sim's
/// own `Kind::extent`, whose middle number is this said in cells.
///
/// **The band is one cell deep, and it is derived rather than chosen.**
/// It used to be a round 32 units, which came out at 0.497 m — nine
/// tenths of a cell, on a world built entirely of them (`BAY_CELL`,
/// rooms four cells tall, a cell of padding between rooms). A rig is
/// [`BAY_FIT`] of its cells across and [`BAY_FIT`] of them tall; it is
/// [`BAY_FIT`] of one cell deep now too, which is the same sentence
/// said on the third axis. The sim states the depth as one cell
/// (`cargo::Kind::extent`) and the drawing spends exactly that, so a
/// piece's plan on the deck and its body on the deck are one claim.
pub const RIG_NEAR: f32 = -2.0;
pub const RIG_FAR: f32 = RIG_NEAR + layout::CELL;

/// **The middle of the depth every rig is composed within**, in
/// rig-local sim units — where a rig's body sits along its own `+Z`,
/// whatever the kind.
///
/// Zero in a rig's own frame is the **berth plane**, and the band runs
/// from just behind it to one cell out into the room. On a wall that
/// plane is a real thing: the chart the rig is screwed to, with the
/// room in front of it, so the band lands where it belongs and this is
/// simply where the body's middle falls.
///
/// **A chart a body stands ON has no such plane**, and that is what this
/// exists to say. The rect a deck or deckhead berth is given spends the
/// DEPTH (`cargo::Kind::plan_on`), so the cells own the ground on both
/// sides of their own middle and there is nothing for a band hung off a
/// plane to hang from. [`site_on`] draws a standing rig back by exactly
/// this, which lands the band on the cells; without it every standing
/// body was composed this far out into the aisle, which is most of half
/// a cell and reads as cargo half a berth off the grid.
const fn rig_mid() -> f32 {
    f32::midpoint(RIG_NEAR, RIG_FAR)
}

/// **How much of the world one rig-local sim unit spends**, in metres.
///
/// A rig is composed in sim units and berthed at [`site_on`]'s scale,
/// which is a property of the chart it hangs on and not of the cell it
/// takes: every chart of every room is laid at `rig::BAY_CELL` to the
/// cell, so there is one number and it is derived rather than restated —
/// a retune of the bay's cell moves the rigs and this together
/// (`tests::a_rig_spends_the_same_metre_on_every_chart`).
///
/// [`BAY_FIT`] is in it because a standing or hanging rig wears it. A
/// laid covering does not, so it is drawn a twenty-fifth bigger than
/// this says; a measure that reads a shade small errs toward reporting,
/// which is the direction a detector is allowed to err in.
pub const RIG_UNIT: f32 = crate::rig::BAY_CELL / layout::CELL * BAY_FIT;

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

/// The launch handle's throw, in the lever rig's own local frame: the
/// pivot sits low in the brass slot, the arm reaches [`LEVER_ARM`] sim
/// units up out of it, and travel tips the whole assembly toward the
/// room about the pivot — a lever pulled, not a slider slid. Rest is a
/// hair off vertical so the handle reads grabbable at a glance.
const LEVER_PIVOT_Z: f32 = 5.0;
const LEVER_ARM: f32 = 14.0;
const LEVER_REST: f32 = 0.30;

/// How far the throw tips the arm past its rest, in radians. Public
/// because the gesture layer's axis is DERIVED from this swing rather
/// than written down beside it (`crate::gesture`): a handle that falls
/// through its slot is pulled by a drag that falls down the panel.
pub const LEVER_THROW: f32 = 0.80;

/// Launch-lever thunk travel time after a departure, and the rattle
/// after a refused pull — the 2D console's clocks, moved with the
/// hardware. Both inside the half-second law.
const THUNK_LEN: f32 = 0.35;
const SHAKE_LEN: f32 = 0.30;

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
            .init_resource::<LeverJuice>()
            .add_systems(PostStartup, spawn_overlays)
            // The surfaces that ride cargo — instrument stations, the
            // standing rigs' faces — move with their pieces, so they
            // are hung before anything reads a surface this frame: the
            // camera's aim, the focus poses, and the pointer all want a
            // surface that agrees with where the hardware actually is.
            //
            // And they are hung AFTER the rooms are built, because a
            // rider is derived from the chart its berth lies on: with
            // the two merely both "before steer" and unordered against
            // each other, the very first frame could run this one first,
            // find no charts, and hang nothing — which reads downstream
            // as "every instrument aboard was jettisoned" and drops a
            // booted focus (`--view tank`) on the floor before it ever
            // has a pose.
            .add_systems(
                Update,
                ride_pieces
                    .in_set(Phase::Input)
                    .after(crate::room::rebuild)
                    .before(crate::rig::steer),
            )
            .add_systems(
                Update,
                (
                    latch_cues,
                    sync_pieces,
                    sync_fixtures,
                    sync_dressings,
                    xray_focus,
                    hover_glint,
                    claim_outlines,
                    carry_held,
                    placement_hints,
                    invite_glows,
                    violation_flash,
                    rat_watch,
                    breathe_pulses,
                    eta_needles,
                    lever_motion,
                    lever_lamp,
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
    /// The amber carry grab's own emissive on the click-functional
    /// kinds, so the hover tell can flare the very bar it names.
    grab_mat: Option<Handle<StandardMaterial>>,
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

/// One hold-cell hint quad, with its refusal slash alongside. Every
/// attached room gets a set, spawned and retired with the room.
#[derive(Component)]
struct HintCell {
    room: RoomId,
    x: u8,
    y: u8,
    slash: Entity,
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

/// The mark at the empty end of an ETA gauge's sweep — where the needle
/// is going. [`eta_needles`] burns it up as the leg closes.
#[derive(Component)]
struct EtaArrival;

/// The launch handle's pivot on a `LaunchLever` rig: [`lever_motion`]
/// throws it, the go-lamp and halo ride it.
#[derive(Component)]
struct LeverHandle;

/// The go-lamp knob at the handle's tip.
#[derive(Component)]
struct LeverLamp;

/// The soft glow plate behind the knob, awake while a pull would work.
#[derive(Component)]
struct LeverHalo;

/// The launch handle's two feedback clocks, wound by cues. One set for
/// every lever aboard: the ship's ceremonies are the ship's, not a
/// particular piece of hardware's.
#[derive(Resource, Default)]
struct LeverJuice {
    thunk: f32,
    shake: f32,
}

/// A surface entity a piece carries, by piece id: an instrument's
/// station on its own glass (BAY.md, "Instruments as cargo") and a
/// standing rig's pick face on its own body. [`ride_pieces`] retires
/// either the moment the piece leaves that berth, so nothing
/// downstream has to ask where the hardware went.
#[derive(Component)]
pub struct Riding(pub u32);

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
pub struct SharedBits {
    pub slash: Handle<StandardMaterial>,
    flash: Handle<StandardMaterial>,
    glyph: Handle<StandardMaterial>,
}

// ------------------------------------------------------------------ helpers --

/// Which room's lane a chart's rect lies in. Lanes are fixed by id and
/// never overlap (docs/ROOMS.md, "Room grids and colored tiles"), so a
/// chart's own rect says whose room it belongs to — which is why nothing
/// in here has to be told, or to guess.
fn chart_room(surface: &SimSurface) -> Option<RoomId> {
    layout::cell_at(SimVec2::new(surface.rect.x + 1.0, surface.rect.y + 1.0))
        .map(|(room, _, _)| room)
}

/// The net chart a sim point reads through, whichever room's it is. The
/// old wall-band/deck-strip pair generalized first to the cabin's six
/// charts and now to every attached room's: lanes are disjoint, so the
/// rect that CONTAINS the point is the only possible answer. `None` off
/// every net.
fn chart_of(
    surfaces: &Query<(&Station, &SimSurface)>,
    sim: SimVec2,
) -> Option<(Station, SimSurface)> {
    surfaces
        .iter()
        .find(|(station, surface)| station.chart_flipped() && surface.rect.contains(sim))
        .map(|(station, surface)| (*station, *surface))
}

/// One named chart of one named room.
fn room_chart(
    surfaces: &Query<(&Station, &SimSurface)>,
    room: RoomId,
    want: Station,
) -> Option<SimSurface> {
    surfaces
        .iter()
        .find(|(station, surface)| {
            let mine = chart_room(surface) == Some(room);
            **station == want && mine
        })
        .map(|(_, surface)| *surface)
}

/// The aft chart of the same room as `surface` — the frame the backing
/// rule reads "room-forward" from, and the one thing a rig needs beyond
/// its own chart.
fn aft_for(surfaces: &Query<(&Station, &SimSurface)>, surface: &SimSurface) -> Option<SimSurface> {
    room_chart(surfaces, chart_room(surface)?, Station::BayWall)
}

/// Where a hold footprint (its `layout::piece_rect`) sits in the room,
/// as the rig root's (translation, rotation, scale). On the floor chart
/// a rig STANDS: feet on its plan rect, upright, turned by the backing
/// rule ([`floor_facing`]) and keeping its bas-relief height (the
/// re-authored 3D extents are deferred; BAY.md). On a wall or the
/// ceiling it hangs flat against the chart. Sizes derive from the
/// surface scales, so retuning `rig::BAY_CELL` re-scales every rig.
fn net_site(
    surfaces: &Query<(&Station, &SimSurface)>,
    kind: Kind,
    rect: Rect,
) -> Option<(Vec3, Quat, Vec3)> {
    let (station, surface) = chart_of(surfaces, rect_center(rect))?;
    let aft = aft_for(surfaces, &surface)?;
    Some(site_on(station, &surface, &aft, kind, rect))
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
    // The floor chart's OWN rect is the floor rect — of whichever room's
    // deck this is. Reading the seams off it rather than off a named room
    // is what lets the same rule stand a couch up in the cabin, in a
    // derelict's hold, and in a station's trade room.
    let plan = surface.rect;
    let seam = layout::CELL * 0.5;
    let one_column = (rect.w - layout::CELL).abs() < seam;
    // Sim axes on the floor chart, in world: `u` port -> starboard,
    // `v` aft -> front (the floor's aft row lies at the aft seam).
    let u = surface.half_u.normalize();
    let v = surface.half_v.normalize();
    let want = if (rect.y - plan.y).abs() < seam {
        v
    } else if ((rect.y + rect.h) - (plan.y + plan.h)).abs() < seam {
        -v
    } else if (rect.x - plan.x).abs() < seam && one_column {
        u
    } else if ((rect.x + rect.w) - (plan.x + plan.w)).abs() < seam && one_column {
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
///
/// **A flat chart's berth spends two axes and the chart fixes the
/// third**, and getting that backwards on one of them is what stood
/// every crate half a berth out into the aisle. A deck berth's rect is a
/// plan — across by deep — so the deck fixes the HEIGHT (the rig rises
/// half its own off the plane) and the cells own the depth. A wall
/// berth's rect is an elevation — across by tall — so the wall fixes the
/// DEPTH, and that is the one case where the band every rig is composed
/// within ([`RIG_NEAR`], [`RIG_FAR`]) already begins where it should.
/// Everywhere else it has to be drawn back onto the cells ([`rig_mid`]).
fn site_on(
    station: Station,
    surface: &SimSurface,
    aft: &SimSurface,
    kind: Kind,
    rect: Rect,
) -> (Vec3, Quat, Vec3) {
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    let scale = Vec3::new(su, sv, su.min(sv)) * BAY_FIT;
    // How tall the kind stands, in sim units. A deck berth's rect is a
    // PLAN — across by deep — so the height a rig rises through is the
    // kind's own and never the rect's (`cargo::Kind::extent`).
    let tall = f32::from(kind.upright().1) * layout::CELL;
    // Back onto the plan: the depth band's middle, laid off along the
    // way the rig is turned to look, so the band covers the cells the
    // rect spent on it instead of reaching out of them.
    let onto_plan = |rot: Quat| rot * (Vec3::Z * (rig_mid() * scale.z));
    match station {
        Station::BayFloor => {
            let base = surface.to_world(rect_center(rect));
            let rot = floor_facing(surface, aft, rect);
            (
                base + Vec3::Y * (tall * 0.5 * scale.y) - onto_plan(rot),
                rot,
                scale,
            )
        }
        // Ceiling cargo hangs PENDANT: upright like a floor rig, author
        // up staying world up — the lamp's cord meets the ceiling and
        // its shade swings below — rather than pasted flat against the
        // plane, which is what the playtest called out.
        //
        // **And it takes the backing rule, exactly as a floor rig does.**
        // It used to take one fixed turn, facing the front of the room
        // from every cell of the deckhead, which is the couch-facing-the-
        // wall defect stood on its head: a pendant hung on the front row
        // looked into the front wall a hand's breadth in front of it. A
        // deckhead is a chart a body stands on with gravity the other way
        // round, its seams are the same four seams, and there was never a
        // reason for it to be the one chart whose bodies do not turn. Mid-
        // room the rule's own default is the turn this used to hardcode,
        // so nothing away from a seam moves.
        Station::BayCeiling => {
            let base = surface.to_world(rect_center(rect));
            let rot = floor_facing(surface, aft, rect);
            (
                base - Vec3::Y * (tall * 0.5 * scale.y) - onto_plan(rot),
                rot,
                scale,
            )
        }
        _ => (
            surface.to_world(rect_center(rect)),
            wall_upright(station, surface),
            scale,
        ),
    }
}

/// The turn about a chart's inward normal that carries the chart's own
/// up onto the ROOM's up. The net is one sheet of paper folded into a
/// box, so the charts do not all lie the same way once they stand:
/// nothing to do on the aft wall, whose rows already run level; a HALF
/// turn on the front, which unfolds downward off the floor's front edge
/// so its +y climbs the wall; a QUARTER on the side walls, whose
/// columns run up the wall — the playtest's sideways star chart.
/// `None` where up has no direction in the plane at all: the floor and
/// the ceiling lie flat, and flat has no upright.
fn upright_roll(station: Station, surface: &SimSurface) -> Option<f32> {
    if !station.chart_flipped() {
        return None;
    }
    let n = station.inward(surface);
    let want = Vec3::Y - n * Vec3::Y.dot(n);
    if want.length_squared() < 0.5 {
        return None;
    }
    let want = want.normalize();
    let up = station.face(surface) * Vec3::Y;
    Some(up.cross(want).dot(n).atan2(up.dot(want)))
}

/// **The upright rule for wall cargo, and it no longer has a clause.**
/// A rig hung on a chart inherits that chart's lie, so it rolls back
/// upright about the wall normal (facing is untouched). Every wall, every
/// footprint, always.
///
/// It used to have to count cells first. A HALF turn maps any footprint
/// onto itself; a QUARTER turn trades width for height, so a non-square
/// footprint was refused one and hung sideways instead — the starting
/// window coming out a quarter turn from the window that left. What was
/// turning was the CELLS: the net's side flaps fold out sideways, and a
/// footprint stated in the sheet's frame therefore meant a different
/// shape on a flank than on an end. The footprint is stated in the
/// wall's own frame now (`cargo::Kind::plan_on`), which means the cells
/// a flank berth owns are already the rolled ones — so the roll a body
/// wants is always the one its cells have paid for, and there is nothing
/// left to refuse.
fn wall_upright(station: Station, surface: &SimSurface) -> Quat {
    let base = station.face(surface);
    upright_roll(station, surface).map_or(base, |roll| {
        Quat::from_axis_angle(station.inward(surface), roll) * base
    })
}

/// Whether the upright rule turned a berth's rig off its chart's own
/// lie. That is exactly when the rig's frame and the chart's frame stop
/// agreeing about where a sub-rect of the footprint lies — and so
/// exactly when the piece must carry its own face ([`standing_surface`]).
fn wall_rolled(station: Station, surface: &SimSurface) -> bool {
    let base = station.face(surface);
    (wall_upright(station, surface) * Vec3::Y).dot(base * Vec3::Y) < 0.999
}

/// Where a laid footprint lies: flat AGAINST its chart, lifted
/// [`LAID_LIFT`] proud of the quad so the coat clears the socket plates
/// yet stays under everything standing on the same cells. No
/// [`BAY_FIT`] margin — a covering covers; its own geometry insets
/// where the berth edge should still read.
fn net_laid(surfaces: &Query<(&Station, &SimSurface)>, rect: Rect) -> Option<(Vec3, Quat, Vec3)> {
    let (station, surface) = chart_of(surfaces, rect_center(rect))?;
    Some(laid_on(station, &surface, rect))
}

/// The chart a sim point reads through, among a plain list of them —
/// [`chart_of`] for callers holding a snapshot rather than a query.
fn chart_at(charts: &[(Station, SimSurface)], sim: SimVec2) -> Option<(Station, SimSurface)> {
    charts
        .iter()
        .copied()
        .find(|(station, surface)| station.chart_flipped() && surface.rect.contains(sim))
}

/// The aft chart of `surface`'s own room, among a plain list.
fn aft_in(charts: &[(Station, SimSurface)], surface: &SimSurface) -> Option<SimSurface> {
    let room = chart_room(surface)?;
    charts
        .iter()
        .find(|(station, other)| *station == Station::BayWall && chart_room(other) == Some(room))
        .map(|(_, other)| *other)
}

/// [`net_laid`]'s pure core, for a known chart — shared with the unit
/// tests. A coat takes the upright rule like any other thing drawn on a
/// wall: paint has a top, and wallpaper will have a pattern. The floor
/// and ceiling have no upright to take, and a rug's non-square
/// footprint keeps its cells, so today's dressings all lie exactly
/// where they always did.
fn laid_on(station: Station, surface: &SimSurface, rect: Rect) -> (Vec3, Quat, Vec3) {
    let (su, sv) = (surface.scale_u(), surface.scale_v());
    (
        surface.to_world(rect_center(rect)) + station.inward(surface) * LAID_LIFT,
        wall_upright(station, surface),
        Vec3::new(su, sv, su.min(sv)),
    )
}

/// **The pose a rig berthed on `rect` actually takes**, with the chart it
/// takes it on: [`site_on`]'s own answer, for a caller holding a
/// snapshot of the charts rather than a live world.
///
/// Pure, and it exists so the gauntlet can ask which way a berth turns a
/// body without spawning one (`crate::gauntlet`). A berth's TURN is the
/// half of its pose no box can carry: an axis-aligned box is the same
/// box after a half turn, and it is the same box after a quarter turn
/// whenever the footprint is square — so a rule handed only
/// [`berth_box`]'s corners cannot ask which way a couch is looking, and
/// for as long as that was all there was, nothing did.
#[must_use]
pub fn berth_pose(
    charts: &[(Station, SimSurface)],
    kind: Kind,
    rect: Rect,
) -> Option<(Station, SimSurface, Vec3, Quat, Vec3)> {
    let (station, surface) = chart_at(charts, rect_center(rect))?;
    let aft = aft_in(charts, &surface)?;
    let (pos, rot, scale) = site_on(station, &surface, &aft, kind, rect);
    Some((station, surface, pos, rot, scale))
}

/// **The world box a rig berthed on `rect` actually fills**, as an
/// axis-aligned `(lo, hi)` — [`site_on`]'s pose plus the common rig
/// depth ([`RIG_NEAR`], [`RIG_FAR`]), spun onto the world axes.
///
/// Pure, and it exists so the gauntlet can ask what a berth costs in air
/// without spawning a thing (`crate::gauntlet`). It goes through the very
/// function the runtime poses rigs with, so a retune of the berth pose
/// moves the question and the answer together.
#[must_use]
pub fn berth_box(charts: &[(Station, SimSurface)], kind: Kind, rect: Rect) -> Option<(Vec3, Vec3)> {
    let (_, _, pos, rot, scale) = berth_pose(charts, kind, rect)?;
    // The body, in rig-local sim units: the kind's OWN frame across and
    // up (`cargo::Kind::upright`, which no berth turns) and the common
    // rig depth along the local normal. Read off the kind rather than
    // off the rect, because a deck berth's rect is a plan and a plan
    // says nothing about how tall the thing standing on it is.
    let (a, t) = kind.upright();
    let half = Vec3::new(
        f32::from(a) * layout::CELL * 0.5,
        f32::from(t) * layout::CELL * 0.5,
        (RIG_FAR - RIG_NEAR) * 0.5,
    ) * scale;
    let centre = pos + rot * (Vec3::Z * (rig_mid() * scale.z));
    let m = Mat3::from_quat(rot);
    let reach = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
    Some((centre - reach, centre + reach))
}

/// **What a named feature of a rig claims about the way it points.**
///
/// The kind×chart sweep ([`tests::every_kind_hangs_true_on_every_legal_berth`])
/// asks whether a rig is turned the way its BERTH says; nothing asked
/// whether the rig's own parts point where their names say. A sconce
/// whose cup opens along the wall and a floor lamp whose base plate
/// stands on edge like a wheel both hang perfectly true by the first
/// question and are both wrong, and a screenshot of a lit lamp shows
/// neither.
///
/// **The turn is derived from the claim, never written beside it.** It
/// used to be a hand-authored quaternion sitting next to the direction
/// it was supposed to produce, which is a design that guarantees the
/// bug it then detects: two of the four disagreed, and they are the two
/// the playtest found by eye. A claim-bearing part is a body of
/// revolution about its own axis — a cone, a cylinder, a torus — so any
/// turn that carries [`Feature::axis`] onto [`Feature::want`] draws the
/// same body, and the shortest one is as good as any. Nothing is left
/// for a builder and a name to disagree about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    /// What the part is called in its builder.
    pub name: &'static str,
    /// The part's own meaningful axis in ITS body's frame — see
    /// [`MOUTH`], [`AXLE`].
    pub axis: Vec3,
    /// Where that axis has to end up, in the rig's local frame: local
    /// `+Z` is into the room on every berth the net has, local `+Y` is
    /// up off the deck.
    pub want: Vec3,
}

impl Feature {
    /// The turn the rig gives the part: the shortest one that carries
    /// its axis onto the direction its name claims.
    #[must_use]
    pub fn turn(&self) -> Quat {
        Quat::from_rotation_arc(self.axis.normalize(), self.want.normalize())
    }
}

/// **What a named part of a rig claims about what holds it up.**
///
/// [`Feature`]'s sibling, one question over. Every family the harness
/// had measured a part against the WORLD — the band it is composed in,
/// the plane it fights, the cells it draws inside, the direction its own
/// name claims — and not one of them measured a part against another
/// part of the same rig. A couch's foot standing under a couch it does
/// not touch satisfies all of them: it is inside the band, it shares no
/// plane, it draws well within its cells, and "foot" makes no claim
/// about a direction. What it does claim is a JOINT, and a joint with
/// daylight in it is furniture on stilts of air.
///
/// **The claim is declared and then checked, never guessed at.** A
/// sweep that tried to infer joints — "these two are close, they
/// probably meet" — would report every crate that happens to stand near
/// its own lid, and would say nothing about the one part whose name is
/// a promise. A part that is composition declares no seat and is asked
/// nothing, which is why `gauntlet::ALLOWED` needs no entry for this
/// family: there is nothing to forgive, only things nobody claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    /// What the part is called in its builder.
    pub name: &'static str,
    /// What holds it: another part of the SAME rig, by that part's own
    /// name. Several may answer to it — a pane glazed behind four lips
    /// meets whichever lip it reaches — and meeting any one of them is
    /// meeting the seat.
    pub on: &'static str,
}

/// **How far a part stands off the thing it is bolted to**, in rig-local
/// sim units: one fight-free step of the decal ladder
/// (`rig::layer::STEP`), said in the units a rig is composed in.
///
/// Two bodies meeting on one plane is a coin toss in the depth buffer,
/// so a joint that has to READ as a joint — a pane glazed into a bezel,
/// a stud proud of its ring — stands one step off instead of none. The
/// gauntlet's own tolerance is this plus a paint's thickness
/// (`gauntlet::SEAT_GAP`), so a builder spending exactly this is inside
/// the rule with room to spare and anything spending a body's width is
/// not.
pub const GLAZE: f32 = crate::rig::layer::STEP / RIG_UNIT;

/// **How far a rig's sole is buried in the chart it is berthed on**, in
/// rig-local sim units — [`GLAZE`], because it is the same joint one
/// plane down. A sole flush with the deck shares a plane with it and a
/// sole above it is furniture floating, so a foot meets a floor by going
/// a step into it, exactly as a pane meets the bezel it is glazed into.
///
/// **The sole is whichever face meets the chart**, and gravity is not in
/// the argument: a pendant's canopy meets a deckhead by going a step up
/// into it and a porthole's glass meets a wall by going a step back into
/// it. The gauntlet holds a rig to both sides of this — `SOLE_SINK`, a
/// centimetre, past which a foot is a body through the deck, and its
/// `rig-seated` family, which refuses a rig that never gets to its chart
/// at all.
pub const SOLE_BURY: f32 = GLAZE;

/// **Where a deck-berthed kind's sole lands, in its own frame** — the
/// chart plane its berth stands it on, [`SOLE_BURY`] into it.
///
/// [`site_on`] stands a deck berth's rig half its own height above the
/// chart, so the chart is at `-tall/2` of the rig's own upright frame
/// and a foot meets it by going a step past it. Every kind that stands
/// on a deck is composed off this rather than off a decimal, and that is
/// the whole difference between a kind DRAWN STANDING and a kind drawn
/// centred in its cell that happens to be near the floor — which is what
/// twenty of them were, because they began as glyphs on a flat console
/// and were given depth without ever being given a floor (docs/BAY.md).
fn sole_of(fh: f32) -> f32 {
    (-fh).mul_add(0.5, -SOLE_BURY)
}

/// A cone's open end: Bevy stands a `Cone` apex-up, so its mouth faces
/// its own `-Y`. A shade, a cup, a horn — the direction the light or the
/// hopper actually goes.
pub const MOUTH: Vec3 = Vec3::NEG_Y;

/// A cylinder's or a torus's axle: Bevy stands a `Cylinder` on its own
/// `+Y`. For a disc — a base plate, a chip — this is the way its FACE
/// looks.
pub const AXLE: Vec3 = Vec3::Y;

/// Every claim-bearing feature of one kind's rig, read back off the rig
/// itself.
///
/// Deliberately short and open: a part earns a claim when its
/// orientation carries a promise a viewer would notice being broken. The
/// rest of a rig is composition, and composition is what screenshots are
/// actually good at.
///
/// It is a **derivation**, not a second table. A claim used to be
/// written here and the turn taken from it by name; now the claim rides
/// the part that makes it ([`Part::pointing`]), so what a rig points and
/// what its list of promises says are one reading of one source.
#[must_use]
pub fn features(kind: Kind) -> Vec<Feature> {
    let piece = Piece {
        id: 0,
        kind,
        variant: 0,
        gnawed: false,
        loc: Loc::Hold {
            room: CABIN,
            x: 0,
            y: 0,
        },
    };
    parts(&piece, Screens::LIVE)
        .into_iter()
        .filter_map(|part| part.claim)
        .collect()
}

/// Every seat claim of one kind's rig, read back off the rig itself —
/// [`features`]'s sibling, and a derivation rather than a second table
/// for the same reason. Both of a screen's states are read, because a
/// pane that is glass in a played build and a phosphor slab in a
/// headless one is two bodies making one promise.
#[must_use]
pub fn seats(kind: Kind) -> Vec<Seat> {
    let piece = Piece {
        id: 0,
        kind,
        variant: 0,
        gnawed: false,
        loc: Loc::Hold {
            room: CABIN,
            x: 0,
            y: 0,
        },
    };
    Screens::BOTH
        .into_iter()
        .flat_map(|screens| parts(&piece, screens))
        .filter_map(|part| part.seat)
        .collect()
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

/// The berth transform for a piece: its own room's net for hold cargo,
/// flat into that net for laid dressings, a cubby anchor inside the
/// host's standing rig for stowed cargo. `None` only where the room is
/// not drawn — a stow whose cabinet is missing, or a lane no chart
/// stands in yet; the caller hides the rig rather than guess.
///
/// Every attached room is drawn now, so a crate on a station's offer
/// band, a couch in a derelict's hold, and fuel on the furnace's deck
/// all come through this one path — the same rigs, the same rules, one
/// room over.
fn berth_site(
    rooms: &Rooms,
    pieces: &[Piece],
    piece: &Piece,
    surfaces: &Query<(&Station, &SimSurface)>,
) -> Option<(Vec3, Quat, Vec3)> {
    match piece.loc {
        Loc::Hold { .. } => net_site(
            surfaces,
            piece.kind,
            layout::piece_rect(rooms, pieces, piece),
        ),
        Loc::Laid { .. } => net_laid(surfaces, layout::piece_rect(rooms, pieces, piece)),
        Loc::Stow { cabinet, slot } => {
            // An occupied cabinet cannot leave its room, so the host is a
            // standing floor rig whenever this berth exists at all.
            let host = pieces
                .iter()
                .find(|other| other.id == cabinet && matches!(other.loc, Loc::Hold { .. }))?;
            let (pos, rot, scale) =
                net_site(surfaces, host.kind, layout::piece_rect(rooms, pieces, host))?;
            Some((
                pos + rot * (cubby_anchor(slot) * scale),
                rot,
                Vec3::splat(scale.min_element() * STOW_FIT),
            ))
        }
    }
}

/// The footprint a drop at `sim` would cover: the aimed cell as the
/// anchor and `cargo::plan`'s answer for that cell's own chart — the
/// very plan [`placement_hints`] lights, so hint, ghost, and berth all
/// read one geometry.
fn aimed_rect(rooms: &Rooms, kind: Kind, sim: SimVec2) -> Option<Rect> {
    let (room, ax, ay) = layout::cell_at(sim)?;
    let (w, h) = cargo::plan(rooms.kind(room)?, kind, ax, ay)?;
    let anchor = layout::cell_rect(room, ax, ay);
    Some(Rect::new(
        anchor.x,
        anchor.y,
        f32::from(w) * layout::CELL,
        f32::from(h) * layout::CELL,
    ))
}

/// **The pose a drop at `sim` would settle the carried kind into**: the
/// turn [`site_on`] would give it, and where the same berth would stand
/// its origin relative to the middle of the cells it takes, in metres.
/// `None` where the aim is off the net (the caller falls back to the
/// chart's own facing and no stand-off at all).
///
/// So the carried ghost promises the pose the piece will actually take —
/// the upright rule on the side walls, the backing rule on the floor,
/// the hopper tile's turn toward the doorway.
///
/// **The stand-off is a whole offset and not a height**, because a
/// standing berth spends one ([`site_on`] draws a deck rig back onto its
/// own cells). A ghost that carried only the reach off the chart hovered
/// square over the cell and landed most of half a cell into the aisle,
/// which is the very defect the berth pose was cured of.
///
/// **The stand-off is the half the ghost used to get wrong**, and it did
/// not show while every kind was composed centred in its own cell: the
/// ghost hung its ORIGIN at the struck point, and a body centred on its
/// origin then sat half in the deck, which reads as a piece resting on
/// the floor if you do not look hard. A kind drawn STANDING has its body
/// wholly above its origin, so the same hover puts the whole of it under
/// the deck. Both numbers are read off `site_on` here rather than
/// restated, so the ghost and the berth move together.
fn hover_pose(
    rooms: &Rooms,
    station: Station,
    surface: &SimSurface,
    aft: Option<&SimSurface>,
    kind: Kind,
    sim: SimVec2,
) -> Option<(Quat, Vec3)> {
    let rect = aimed_rect(rooms, kind, sim)?;
    let (pos, rot, _) = site_on(station, surface, aft?, kind, rect);
    let chart = surface.to_world(rect_center(rect));
    Some((rot, pos - chart))
}

// -------------------------------------------------------- riding surfaces --

/// A quad riding a rig's own berth pose, bound to a sim rect: the frame
/// every rig is authored in — local +X is sim +x, local +Y is up-panel
/// (so the sim's downward y maps to `NEG_Y`), local +Z the depth parts
/// stand proud of the berth plane at. `at` is where the quad's centre
/// sits in that frame and `extent` its size, both in rig-local sim
/// units; `plane` is its depth. The mapping is built from the same
/// transform and the same local units the rig's parts are placed with,
/// so hitbox and geometry cannot drift apart — the fixture sweep's
/// lesson, kept by construction rather than by care.
#[must_use]
fn riding_face(
    rect: Rect,
    at: Vec2,
    extent: (f32, f32),
    plane: f32,
    site: (Vec3, Quat, Vec3),
) -> SimSurface {
    let (pos, rot, scale) = site;
    SimSurface {
        center: pos + rot * (Vec3::new(at.x, at.y, plane) * scale),
        half_u: rot * (Vec3::X * (extent.0 * 0.5 * scale.x)),
        half_v: rot * (Vec3::NEG_Y * (extent.1 * 0.5 * scale.y)),
        rect,
    }
}

/// The station surface an instrument carries at a berth pose: the face
/// the rig draws its glass on, bound to the instrument's logical rect.
/// The normal comes out facing the room because the berth already
/// turned the rig to face it.
#[must_use]
fn ride_surface(mount: &Instrument, kind: Kind, site: (Vec3, Quat, Vec3)) -> SimSurface {
    let (w, h) = kind.upright();
    riding_face(
        mount.rect,
        Vec2::ZERO,
        (
            f32::from(w) * layout::CELL * mount.face.0,
            f32::from(h) * layout::CELL * mount.face.1,
        ),
        mount.plane,
        site,
    )
}

/// **What a rig actually draws**, as an axis-aligned `(centre, half)` in
/// the rig's own local sim units: the union of every body [`parts`]
/// describes — both of a covering's forms, both screen states, each
/// sub-frame at rest — held to the cells the sim gave the kind.
///
/// A footprint and a silhouette are two claims about one object. The
/// footprint is the law, because it is what placement is checked
/// against; the silhouette is what a player can see and therefore what
/// they aim at, and it is derived here rather than declared so the two
/// cannot be authored apart. Held to the cells because a face that
/// reached past them would read a neighbour's berth: the sim answers
/// "which piece is at this point", and a point outside the rect is
/// somebody else's.
#[must_use]
pub fn silhouette(kind: Kind) -> (Vec2, Vec2) {
    let (mid, half) = drawn_box(kind);
    (mid.truncate(), half.truncate())
}

/// **The whole box a rig draws, depth and all** — [`silhouette`] with the
/// third axis it leaves out, as an axis-aligned `(centre, half)` in the
/// rig's own local sim units.
///
/// The silhouette is the two axes a chart is measured in, because it
/// answers a question asked in a chart's frame: which piece is at this
/// point. This answers the question a **tell** asks — what shape do I
/// draw around — and that one has three axes. The depth is the union of
/// what [`parts`] describes rather than the band a rig is composed in
/// ([`RIG_NEAR`], [`RIG_FAR`]): a painting is a finger thick and hangs on
/// a wall a whole cell deep, and a tell cut from the band would stand
/// half a metre out of the wall around a picture.
///
/// Across and up are still held to the kind's own cells, exactly as the
/// silhouette is and for the same reason. Depth is not: nothing else
/// owns the air in front of a berth, and a rig that leans out of its
/// band should be outlined where it actually is.
#[must_use]
pub fn drawn_box(kind: Kind) -> (Vec3, Vec3) {
    let piece = Piece {
        id: 0,
        kind,
        variant: 0,
        gnawed: false,
        loc: Loc::Hold {
            room: CABIN,
            x: 0,
            y: 0,
        },
    };
    let (w, h) = kind.upright();
    let cells = Vec2::new(f32::from(w) * layout::CELL, f32::from(h) * layout::CELL) * 0.5;
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for screens in Screens::BOTH {
        for part in parts(&piece, screens) {
            let Some(body) = part.body else { continue };
            let at = part.under.rest() * part.at;
            let half = body.half() * at.scale;
            let m = Mat3::from_quat(at.rotation);
            let reach = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
            lo = lo.min(at.translation - reach);
            hi = hi.max(at.translation + reach);
        }
    }
    // A rig that draws nothing answers for its cells and its band; there
    // is no such kind today, and a face of no size would be a piece
    // nothing could aim at.
    if !(lo.x < hi.x && lo.y < hi.y) {
        return (
            Vec3::new(0.0, 0.0, rig_mid()),
            Vec3::new(cells.x, cells.y, (RIG_FAR - RIG_NEAR) * 0.5),
        );
    }
    let lo = lo.max(Vec3::new(-cells.x, -cells.y, lo.z));
    let hi = hi.min(Vec3::new(cells.x, cells.y, hi.z));
    ((lo + hi) * 0.5, (hi - lo) * 0.5)
}

// ---------------------------------------------------------------- the tells --

/// **Which sentence a tell says about a piece.** Three readings, three
/// FORMS — and the forms are the whole of it, because a tell may not
/// signal on hue alone and a cabin with the lamps sold reads in one
/// colour anyway.
///
/// - [`Tell::Aim`] — *the crosshair is on this, or your hands are*: a
///   **bracket at every corner** of the body, at the body's own rim. The
///   lightest form for the reading that comes and goes with where you
///   are looking.
/// - [`Tell::Offered`] — *this pile is what the room is waiting on*: a
///   **closed ring**, one continuous line all the way round. The
///   strongest claim gets the most complete form, and it is the form the
///   claim has always had — it has only come off the floor.
/// - [`Tell::Marked`] — *I want that one*: a **dash across the middle of
///   every edge**, on the body's own rim inside the ring. A stub reads
///   as a mark ON a thing where a ring reads as a claim ROUND it, which
///   is the difference between the room noting your interest and the
///   room making an offer.
///
/// All three may be worn at once — the crosshair resting on a good the
/// room has offered and you have asked for — and
/// [`tests::no_two_tells_draw_one_bar_over_another`] holds them apart.
/// A dashed body inside a closed ring is the reading the old inset ticks
/// were reaching for, and the one a wall could hide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tell {
    Aim,
    Offered,
    Marked,
}

/// One bar of a tell, in the rig's own local frame and sim units: `at`
/// its centre and `size` its full extent, axis-aligned like everything
/// else a rig is composed of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub at: Vec3,
    pub size: Vec3,
}

/// **How far off the body each form stands, and how thick its bar is**,
/// in rig sim units — a unit is about a centimetre and a half of world.
///
/// The aim and the mark share the body's own rim, which they can because
/// their forms are complementary: brackets take the ends of every edge
/// and dashes take the middles, and the runs below are cut so the two
/// can never meet. The offer's ring stands off outside both, because a
/// closed line covers every edge end to end and has nowhere to hide.
///
/// Tight on purpose. A tell is an outline of a body and not a crate
/// around it: at these numbers the whole of a claimed one-cell good's
/// outline still stands inside the cell the sim gave it.
const AIM_OUT: f32 = 2.0;
const AIM_GIRTH: f32 = 2.0;
const MARK_OUT: f32 = 2.0;
const MARK_GIRTH: f32 = 1.6;
const OFFER_OUT: f32 = 5.5;
const OFFER_GIRTH: f32 = 2.4;

/// The longest a corner bracket runs, and the most of one edge it may
/// take. The cap is what keeps a bracket a bracket on a big body; the
/// fraction is what keeps two of them from meeting in the middle of a
/// small one, which would draw the offer's own closed ring by accident.
const BRACKET: f32 = 7.0;
const BRACKET_RUN: f32 = 0.3;

/// The same pair for a mark's dash: a quarter of an edge at most, so it
/// reads as one stub on a side and never as a ring with the corners
/// rubbed off.
const DASH: f32 = 8.0;
const DASH_RUN: f32 = 0.28;

/// **The bars one tell draws around a body**, in the body's own frame —
/// `mid` and `half` as [`drawn_box`] hands them over, and out comes the
/// outline, one axis-aligned bar at a time.
///
/// This is the whole of the change the tells needed for a purchased
/// asset to arrive: nothing here reads a mesh, a `Cuboid`, or a chart.
/// It reads a BOX, and the box is derived from whatever [`parts`]
/// describes — so a kind re-cut from bought geometry gets its outline
/// re-cut with it, and a tell drawn round a picture hanging flat on a
/// wall stands off that wall instead of being painted onto it.
#[must_use]
pub fn tell_bars(mid: Vec3, half: Vec3, tell: Tell) -> Vec<Bar> {
    let (out, girth) = match tell {
        Tell::Aim => (AIM_OUT, AIM_GIRTH),
        Tell::Offered => (OFFER_OUT, OFFER_GIRTH),
        Tell::Marked => (MARK_OUT, MARK_GIRTH),
    };
    let reach = half + Vec3::splat(out);
    let mut bars = Vec::new();
    for axis in 0..3 {
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        // The bar runs the whole inflated edge plus one girth, so the
        // twelve of a closed ring meet at the corners rather than
        // leaving eight square holes in it.
        let len = reach[axis].mul_add(2.0, girth);
        let runs: [(f32, f32); 2] = match tell {
            Tell::Offered => [(0.0, len), (0.0, 0.0)],
            Tell::Aim => {
                let run = BRACKET.min(len * BRACKET_RUN);
                [(-(len - run) * 0.5, run), ((len - run) * 0.5, run)]
            }
            Tell::Marked => [(0.0, DASH.min(len * DASH_RUN)), (0.0, 0.0)],
        };
        for sb in [-1.0_f32, 1.0] {
            for sc in [-1.0_f32, 1.0] {
                for (off, run) in runs.iter().filter(|(_, run)| *run > 0.0) {
                    let mut at = mid;
                    at[b] = sb.mul_add(reach[b], at[b]);
                    at[c] = sc.mul_add(reach[c], at[c]);
                    at[axis] += off;
                    let mut size = Vec3::splat(girth);
                    size[axis] = *run;
                    bars.push(Bar { at, size });
                }
            }
        }
    }
    bars
}

/// The pick face a rig carries over its OWN body: its whole drawn
/// [`silhouette`], bound to the sub-rect of its OWN rect that silhouette
/// covers, `plane` rig-local units off the berth plane. Whatever the sim
/// hit-tests inside the piece — the cabinet's cubby sub-rects, an
/// instrument's amber handle band — is then read in the very frame the
/// rig drew it in, so the aim lands on the cargo the player is looking
/// at.
///
/// **The quad is the body, not the footprint**, and the two are not the
/// same shape. It used to be the footprint, which meant the region that
/// answered for a piece was its plan rather than its picture: the brine
/// pearls are a thin column of three spheres filling 62% of their cells
/// across, and a third of a cell of air on either flank of them picked
/// them up. Binding the sub-rect the silhouette covers rather than the
/// whole rect keeps the mapping one-to-one in rig-local units, which is
/// what the handle band and the cubbies are declared in — the face gets
/// smaller, and nothing declared inside it moves.
#[must_use]
fn standing_face(kind: Kind, rect: Rect, plane: f32, site: (Vec3, Quat, Vec3)) -> SimSurface {
    let (at, half) = silhouette(kind);
    let (a, t) = kind.upright();
    // **Sim units of rect per sim unit of rig, on each axis.** A rig is
    // composed in its own upright frame ([`Kind::upright`]) and a berth
    // may hand it a rect of quite another shape: a deck berth's rect is
    // a PLAN, across by deep, where the rig is across by tall, and a
    // flank's is the wall frame transposed. The body is read onto the
    // rect it owns rather than assumed to fill it — which is the whole
    // of the deck apron defect on the pick side, since a wardrobe's
    // elevation laid straight onto its plan spilled a cell of bare deck
    // in front of it that answered for the wardrobe.
    let per = Vec2::new(
        rect.w / (f32::from(a) * layout::CELL),
        rect.h / (f32::from(t) * layout::CELL),
    );
    let mid = rect_center(rect);
    // The rig's +Y is up-panel and the sim's +y runs down, so the box
    // reads back onto the piece's rect with its vertical flipped.
    let bound = Rect::new(
        (at.x - half.x).mul_add(per.x, mid.x),
        (-at.y - half.y).mul_add(per.y, mid.y),
        half.x * 2.0 * per.x,
        half.y * 2.0 * per.y,
    );
    riding_face(bound, at, (half.x * 2.0, half.y * 2.0), plane, site)
}

/// How far a rolled wall piece's pick face stands off its chart, in
/// rig-local sim units, for a kind with no glass of its own: enough
/// that the ray settles on the FACE and never on the chart it would
/// otherwise share a plane with — coplanar quads answer by query order,
/// which is no answer at all — and shallow enough that the aim still
/// reads where the hardware is.
const WALL_FACE_PLANE: f32 = 6.0;

/// The depth a wall piece's pick face rides at: an instrument answers
/// on its own glass — the same plane the mount table hands the rig — so
/// the crosshair and the focused cursor read the same pane from either
/// side of the handle rule. Everything else takes the standoff.
fn face_plane(kind: Kind) -> f32 {
    instrument(kind).map_or(WALL_FACE_PLANE, |mount| mount.plane)
}

/// Where a berthed instrument's station hangs, from its hold cells
/// alone: the piece's BERTH pose — never the eased tween, since a
/// station that lagged its own housing would hand the sim stale
/// coordinates mid-glide — through the same [`site_on`] the rig lands
/// with. `None` for passive cargo, or off the net.
#[must_use]
pub fn instrument_surface(
    charts: &[(Station, SimSurface)],
    kind: Kind,
    rect: Rect,
) -> Option<(Station, SimSurface)> {
    let mount = instrument(kind)?;
    let (station, surface) = chart_at(charts, rect_center(rect))?;
    let aft = aft_in(charts, &surface)?;
    Some((
        mount.station,
        ride_surface(&mount, kind, site_on(station, &surface, &aft, kind, rect)),
    ))
}

/// The pick face a hold berth carries, wherever the rig's own frame
/// leaves its chart's. Two ways that happens, one answer.
///
/// A rig that STANDS is nowhere near the flat chart it berths on: floor
/// cargo rises off the deck, a pendant hangs under the ceiling slab, so
/// aiming at the top of a wardrobe and projecting that ray onto the
/// deck answers about a plate two steps behind it, skewed and mirrored
/// (the playtest's top-right cubby selecting the top-left one).
///
/// A rig the upright rule ROLLS lies in its chart's plane but a quarter
/// (or a half) turn off its lie, so a sub-rect declared in the chart's
/// own y — the amber handle band — is nowhere near the bar the rig
/// draws from those very numbers: the tank on a side wall wears its
/// grab across the bottom and routes carry down one flank. Same defect
/// class, other plane. The face stands the piece's own frame a standoff
/// off the wall, where it outranks the chart it hangs on.
///
/// Wall cargo the rule leaves alone still needs no face: there the
/// chart already IS the piece.
#[must_use]
pub fn standing_surface(
    charts: &[(Station, SimSurface)],
    kind: Kind,
    rect: Rect,
) -> Option<SimSurface> {
    let (station, surface) = chart_at(charts, rect_center(rect))?;
    let aft = aft_in(charts, &surface)?;
    let site = site_on(station, &surface, &aft, kind, rect);
    match station {
        Station::BayFloor | Station::BayCeiling => Some(standing_face(kind, rect, 0.0, site)),
        _ if wall_rolled(station, &surface) => {
            Some(standing_face(kind, rect, face_plane(kind), site))
        }
        _ => None,
    }
}

/// Hang, move, and retire the surfaces that ride the cargo: an
/// instrument's station on its own glass, a standing rig's pick face on
/// its own body. Runs before the pointer so the ray meets surfaces that
/// agree with the hardware they are painted on; a jettisoned (or
/// carried, or shelved) piece simply has none, and `aimed_station`, the
/// focus poses, and the pointer all skip what is not there.
fn ride_pieces(
    mut commands: Commands,
    shell: Res<Shell>,
    charts: Query<(&Station, &SimSurface), Without<Riding>>,
    mut riders: Query<(Entity, &Riding, &Station, &mut SimSurface)>,
) {
    let charts: Vec<(Station, SimSurface)> = charts
        .iter()
        .map(|(station, surface)| (*station, *surface))
        .collect();
    let sim = &shell.bridge.sim;
    let in_hand = sim.held(0).map(|held| held.piece);
    let mut live: Vec<(u32, Station, SimSurface)> = Vec::new();
    for piece in sim.pieces() {
        // A piece in hand rides the crosshair, not a berth: its surfaces
        // come down until it lands, so the carry never aims at itself.
        if in_hand == Some(piece.id) {
            continue;
        }
        let rect = layout::piece_rect(sim.rooms(), sim.pieces(), piece);
        // Whichever room the berth is in: an instrument carries its
        // station wherever it hangs, and a standing rig carries its own
        // pick face wherever it stands. A crate staged on the furnace's
        // deck is floor cargo one room over, and is grabbed by its body
        // for exactly the reason floor cargo always was.
        if matches!(piece.loc, Loc::Hold { .. }) {
            if let Some((station, surface)) = instrument_surface(&charts, piece.kind, rect) {
                live.push((piece.id, station, surface));
            }
            if let Some(face) = standing_surface(&charts, piece.kind, rect) {
                live.push((piece.id, Station::Standing, face));
            }
        }
    }
    // Matched by piece AND station: one piece may carry both a station
    // and a face, and neither may inherit the other's quad.
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
    let cell = layout::cell_rect(CABIN, x, y);
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

/// Pre-spawn everything that waits dark for a sim state to light it: the
/// violation frame bars (four per bay surface — a refused footprint may
/// straddle the fold), and the glyph bar pool. The per-cell hints belong
/// to their rooms and are spawned with them ([`hint_cells`]).
fn spawn_overlays(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
) {
    let slash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 3.0);
    let flash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 0.0);
    let glyph_mat = glow::phosphor(&mut materials, palette::GLINT, 0.0);
    commands.insert_resource(SharedBits {
        slash: slash_mat,
        flash: flash_mat.clone(),
        glyph: glyph_mat.clone(),
    });

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
    // And the standing tells' own pool, dark. What they say is derived
    // by the sim every frame and never stored, so the presentation keeps
    // a pool and aims it.
    let tell_mat = glow::phosphor(&mut materials, palette::AMBER, 2.0);
    for i in 0..TELL_BARS {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(tell_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            TellBar(i as u16),
        ));
    }
}

/// One room's net-cell hints: a thin quad per cell on whichever chart
/// holds it, its refusal slash floating just above (shape channel —
/// illegality never rides hue alone). The socket plates themselves are
/// the room's own furniture; these are the glow layer over them, lifted
/// past [`OVERLAY_LIFT`] so a hint over a laid rug burns over the pile
/// rather than inside it. Holes get no hint; nothing can land there.
///
/// Spawned with the room and retired with it, because a hint that
/// outlived its floor would light a cell in space.
pub fn hint_cells(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    shared: &SharedBits,
    placed: &crate::room::Placed,
) {
    let tag = crate::room::InRoom {
        room: placed.id,
        kind: placed.kind,
    };
    let room = placed.id;
    let (cols, rows) = placed.kind.grid();
    for y in 0..rows {
        for x in 0..cols {
            let cell = layout::cell_rect(room, x, y);
            let Some((station, surface)) = chart_at(&placed.charts, rect_center(cell)) else {
                continue;
            };
            let (su, sv) = (surface.scale_u(), surface.scale_v());
            let rot = station.face(&surface);
            let normal = station.inward(&surface);
            let center = surface.to_world(rect_center(cell));
            let slash = commands
                .spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(shared.slash.clone()),
                    Transform::from_translation(center + normal * crate::rig::layer::SLASH)
                        .with_rotation(rot * Quat::from_rotation_z((cell.h / cell.w).atan()))
                        .with_scale(Vec3::new(
                            cell.w.hypot(cell.h) * 0.82 * su,
                            2.6 * sv,
                            0.0015,
                        )),
                    Visibility::Hidden,
                    tag,
                ))
                .id();
            let mat = glow::phosphor(materials, palette::LAMP_OK, 0.0);
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(center + normal * OVERLAY_LIFT)
                    .with_rotation(rot)
                    .with_scale(Vec3::new((cell.w - 4.0) * su, (cell.h - 4.0) * sv, 0.0015)),
                Visibility::Hidden,
                HintCell { room, x, y, slash },
                tag,
            ));
        }
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
                    let (room, x, y) = layout::cell_at(pointer.sim).unwrap_or((CABIN, 0, 0));
                    let (w, h) = memo
                        .0
                        .and_then(|(_, kind)| cargo::plan(sim.rooms().kind(room)?, kind, x, y))
                        .unwrap_or((1, 1));
                    let anchor = layout::cell_rect(room, x, y);
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
    skies: Option<Res<crate::viewport::Skies>>,
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
        let Some((goal, rot, scale)) = berth_site(sim.rooms(), sim.pieces(), piece, &surfaces)
        else {
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
                sky: skies.is_some(),
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
    let rooms = shell.bridge.sim.rooms();
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
                && matches!(piece.loc, Loc::Hold { room, x, y } if {
                    rooms.kind(room).and_then(|host| {
                        let (w, h) = cargo::plan(host, piece.kind, x, y)?;
                        Some((0..w).any(|dx| {
                            (0..h).any(|dy| lit_adjacent(host, pieces, room, x + dx, y + dy))
                        }))
                    }) == Some(true)
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

    let kind = sim
        .pieces()
        .iter()
        .find(|piece| piece.id == held.piece)
        .map(|piece| piece.kind);

    let (pos, rot, fit) = if camera_rig.roaming() {
        if let (Some(world), Some(station), Some(surface), Some(kind)) =
            (pointer.world, pointer.station, pointer.surface, kind)
        {
            // Aimed at the room: hover the piece at the hit, standing
            // exactly the way it would land. The promise is kept by
            // deriving the rotation from the SAME berth maths the drop
            // will use ([`hover_pose`]) — a preview that computed its
            // own facing drifted from the berth (the playtest's
            // quarter-turned starboard chart hovering upright, then
            // landing sideways, and the reverse once the upright rule
            // landed). Only the lift off the surface is the preview's.
            //
            // The berth is whatever chart the aimed CELL belongs to,
            // which is not always the surface the ray struck: a
            // crosshair resting on a standing rig's own face reads that
            // piece's cells, and those cells are still the floor's.
            let (berth, plane) = chart_of(&surfaces, pointer.sim).unwrap_or((station, surface));
            let aft = aft_for(&surfaces, &plane);
            let (rot, stand) =
                hover_pose(sim.rooms(), berth, &plane, aft.as_ref(), kind, pointer.sim)
                    .unwrap_or_else(|| (station.face(&surface), Vec3::ZERO));
            (
                world + station.inward(&surface) * CARRY_LIFT + stand * HOVER_FIT,
                rot,
                HOVER_FIT,
            )
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
        // parked pointer — simply wherever it last hovered. There is no
        // console face left to float over as a last resort (the hull
        // owns no panels), so a drag with no history at all simply waits
        // for the pointer to land somewhere real.
        if let (Some(world), Some(surface)) = (pointer.world, pointer.surface) {
            carry.last = Some((world + surface.normal() * CARRY_LIFT, surface.orientation()));
        }
        let Some((pos, rot)) = carry.last else {
            return;
        };
        (pos, rot, HOVER_FIT)
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
        // The instrument the camera came to work is the focused
        // CONTENT, never an occluder of itself — the desk rows'
        // exemption, generalized now that a station can be cargo.
        let is_the_focus = matches!(piece.loc, Loc::Hold { .. })
            && instrument(piece.kind).is_some_and(|mount| Focus::of(mount.station) == focus);
        let candidate =
            matches!(piece.loc, Loc::Hold { .. }) && held != Some(piece.id) && !is_the_focus;
        let occludes = candidate && !targets.is_empty() && {
            let (w, h) = piece.kind.upright();
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
/// would grab wears a faint glint frame before any click. The frame is
/// cut from the rig's [`silhouette`], which is where its pick face is
/// cut from too, so what lights up is what answers: the tell used to
/// wrap the footprint and say honestly that the plan was the hitbox,
/// and the plan is no longer the hitbox.
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
        .then(|| layout::piece_at(sim.rooms(), sim.pieces(), pointer.sim).map(|piece| piece.id))
        .flatten();
    if *prev != hovered
        && let Some(old) = *prev
        && let Some(&entity) = index.0.get(&old)
        && let Ok(rig) = rigs.get(entity)
    {
        if let Ok(mut v) = vis.get_mut(rig.frame_root) {
            *v = Visibility::Hidden;
        }
        // The aim left: the grab bar falls back to its resting amber.
        if let Some(grab) = rig.grab_mat.as_ref()
            && let Some(mut mat) = materials.get_mut(grab)
        {
            mat.emissive = palette::AMBER.to_linear() * GRAB_GLOW;
        }
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
                carry_handle_rect(
                    piece.kind,
                    layout::piece_rect(sim.rooms(), sim.pieces(), piece),
                )
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
        // And the hardware itself answers: the amber bar the aim rests
        // on flares to the grab brightness. The tell is on the piece,
        // not only in the frame around it — brightness, not hue alone.
        if let Some(grab) = rig.grab_mat.as_ref()
            && let Some(mut mat) = materials.get_mut(grab)
        {
            let level = if on_handle { GRAB_FLARE } else { GRAB_GLOW };
            mat.emissive = palette::AMBER.to_linear() * level;
        }
    }
}

// ------------------------------------------------------ the standing tells --

/// One bar of the standing tells' pool, and which bar of the frame it is
/// this frame — the pool is aimed afresh every frame and the pieces
/// themselves never move.
#[derive(Component, Clone, Copy, Debug)]
struct TellBar(u16);

/// How many bars the pool shares out. A bracketed sentence spends
/// twenty-four and a dashed one twelve, so this is two dozen goods
/// claimed at once, or four dozen merely asked for.
///
/// The pool is sized to be **read**, not to be exhaustive, and that is a
/// deliberate limit rather than an unchecked one. A frame is a work
/// order the player clears one piece at a time, and the set is
/// re-derived every frame, so a twenty-fifth crate simply gets its
/// outline when the twenty-fourth is carried aboard. What the pool must
/// never do is show *nothing* while something is detained, and it
/// cannot: it fills from the front.
const TELL_BARS: usize = 576;

/// **The standing tells: what a room's business is about, outlined where
/// it stands.** Three sentences and two forms, aimed at the bodies
/// themselves — [`room::lit_footprints`] says which pieces and which
/// form, and this hangs [`tell_bars`] on each one's own [`drawn_box`],
/// in the rig's own pose.
///
/// - `Sim::composed` names the pile the room would hand over if the
///   handshake were worked right now, bracketed on the room's own stock —
///   *this is what's on offer for yours*;
/// - `Sim::detained_cargo` names every piece of the player's standing in
///   a room that will not ride out, bracketed where the player set it
///   down — *this is what the launch is waiting on*;
/// - `Sim::marks` names the room's goods the player has pointed at —
///   *I want that one* — and those wear the weaker form, a dash across
///   the middle of every edge rather than a bracket at every corner.
///
/// The second is **the whole reading of the staging law**, and it is why
/// a station's deck needs no paint of its own: an empty staging cell is
/// deck, and an occupied one wears the outline. Pull the lever with one
/// standing and the sim answers `Cue::Refit`, which strobes every jamb
/// red (`room::seam_fx`) — the same refusal the door's own latch gives,
/// at the other end of the same law. Not a word anywhere.
///
/// **The tell is on the body now, not on the chart under it.** It used
/// to be four bars ringing the piece's footprint, painted on whichever
/// chart the piece was berthed on and lifted a rung of the decal ladder
/// off it, and a mark's bars were drawn short and INSET — inside the
/// footprint, which on anything standing proud of its chart means inside
/// the piece. A painting hangs flat on a wall and fills its own
/// footprint exactly, so the mark on a picture for sale was drawn
/// behind the picture and the press that set it read as a dead click.
/// An outline round the body cannot be hidden by the body, and it needs
/// to know nothing about the body but its box — which is the property
/// the whole tell layer was moved for, with purchased geometry coming.
///
/// **Nothing draws it through what stands in FRONT of it, and the note
/// that used to say otherwise was wrong.** The frame carried a
/// `depth_bias` of a thousand, for the staging law's sake: a station's
/// dressing may stand in a staging cell and cargo may be set down inside
/// it, so the reading that says what the launch is waiting on had better
/// not be the one a bollard can hide. But a material's depth bias only
/// sorts this engine's transmissive and transparent phases — an opaque
/// emissive never sees one — and the whole picture moves by fifteen
/// pixels with it taken out. So it is out, and what keeps the reading
/// instead is that an outline SURROUNDS its body: a bollard takes a bar
/// or two out of a ring and leaves a ring.
fn claim_outlines(
    shell: Res<Shell>,
    index: Res<PieceIndex>,
    rigs: Query<&Transform, (With<PieceRig>, Without<TellBar>)>,
    mut bars: Query<(&TellBar, &mut Transform, &mut Visibility), Without<PieceRig>>,
) {
    let sim = &shell.bridge.sim;
    let mut aimed: Vec<(Vec3, Quat, Vec3)> = Vec::new();
    for (id, marked) in crate::room::lit_footprints(sim) {
        if aimed.len() >= TELL_BARS {
            break;
        }
        let Some(piece) = sim.pieces().iter().find(|piece| piece.id == id) else {
            continue;
        };
        let Some(&entity) = index.0.get(&id) else {
            continue;
        };
        // The rig's LIVE pose, tween and all, so an outline rides the
        // glide of the piece it is about instead of waiting at the berth
        // for it.
        let Ok(at) = rigs.get(entity) else {
            continue;
        };
        let (mid, half) = drawn_box(piece.kind);
        let tell = if marked { Tell::Marked } else { Tell::Offered };
        for bar in tell_bars(mid, half, tell) {
            aimed.push((
                at.translation + at.rotation * (bar.at * at.scale),
                at.rotation,
                bar.size * at.scale,
            ));
        }
    }
    for (bar, mut transform, mut visibility) in &mut bars {
        let Some(&(at, rot, size)) = aimed.get(usize::from(bar.0)) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        *transform = Transform::from_translation(at)
            .with_rotation(rot)
            .with_scale(size);
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
        let ours = player_owned(sim.rooms(), sim.pieces(), held.origin);
        if !ours {
            return None;
        }
        let piece = sim.pieces().iter().find(|piece| piece.id == held.piece)?;
        let (room, ax, ay) = layout::cell_at(pointer.sim)?;
        // The hint must consult the SAME arbiter the drop will: a
        // covering answers to the dressing rules (a tin coats any
        // chart), everything else to placement. The playtest's
        // green-frame-over-red-hint contradiction was this line using
        // one arbiter for both.
        let legal = if piece.kind.covering() {
            space_trucking::sim::cargo::dressing_check(
                sim.rooms(),
                sim.pieces(),
                piece.id,
                piece.kind,
                room,
                ax,
                ay,
            )
            .is_ok()
        } else {
            placement_check(
                sim.rooms(),
                sim.pieces(),
                piece.id,
                piece.kind,
                room,
                ax,
                ay,
            )
            .is_ok()
        };
        let (w, h) = cargo::plan(sim.rooms().kind(room)?, piece.kind, ax, ay)?;
        Some((room, ax, ay, w, h, legal))
    });
    for (cell, material, mut visibility) in &mut hints {
        let lit = plan.filter(|&(room, ax, ay, w, h, _)| {
            room == cell.room && cell.x >= ax && cell.x < ax + w && cell.y >= ay && cell.y < ay + h
        });
        if let Some((_, _, _, _, _, legal)) = lit {
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
/// counter's rows of sockets left with the counter (docs/ROOMS.md), so
/// what is left of this is the cabinets: empty cubby mouths breathe a
/// gentler amber while the carried piece could box up somewhere.
fn invite_glows(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cubbies: Query<(
        &CubbyGlow,
        &MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
) {
    let sim = &shell.bridge.sim;
    let targets = sim.drop_targets(0);
    let t = time.elapsed_secs();
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
        // The doormat: three stripes across the way through. An
        // aperture belongs to two rooms at once, so nothing berths on
        // it, and the refusal wears the stripes that say why.
        Some(Violation::Threshold) => vec![
            bar_between(
                Vec2::new(-s * 0.9, s * 0.55),
                Vec2::new(s * 0.9, s * 0.55),
                s * 0.2,
            ),
            bar_between(Vec2::new(-s * 0.9, 0.0), Vec2::new(s * 0.9, 0.0), s * 0.2),
            bar_between(
                Vec2::new(-s * 0.9, -s * 0.55),
                Vec2::new(s * 0.9, -s * 0.55),
                s * 0.2,
            ),
        ],
        // Off the net, onto a piece (or its standing shadow), the violet
        // objection, the last vital instrument refusing its exit, a cell
        // and a cell the room's own hardware already fills: the frame
        // alone. (Vital and Fixture are rules still owed their own
        // glyphs — the frame and the buzz carry them meanwhile.)
        Some(
            Violation::Bounds
            | Violation::Overlap
            | Violation::Suspicious
            | Violation::Vital
            | Violation::Fixture,
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
    // The frame bars are drawn in the chart's own axes and stay there —
    // a rectangle has no up. The GLYPH does: a hazard triangle points
    // somewhere and a crate has a lid, so the icon takes the upright
    // rule whole, on every wall the net folds up (charts whose columns
    // run sideways, and the front's rows that climb).
    let rot = wall_upright(station, &surface);
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
    // The stowaway is a cabin resident: `rats` only ever perches it on
    // room-zero cells, so its chart is the cabin's own deck.
    let Some(floor) = room_chart(&surfaces, CABIN, Station::BayFloor) else {
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
            let Loc::Hold { room: CABIN, x, y } = piece.loc else {
                return false;
            };
            let Some((w, h)) = sim
                .rooms()
                .kind(CABIN)
                .and_then(|host| cargo::plan(host, piece.kind, x, y))
            else {
                return false;
            };
            piece.kind == Kind::Couch
                && (x..x + w).contains(&rat.cell.0)
                && (y..y + h).contains(&rat.cell.1)
        });
    let unit = f32::midpoint(floor.scale_u(), floor.scale_v()) * RAT_FIT;
    let hop = (PI * t).sin() * 5.0 * unit;
    // Asleep it settles to its cell's centre and lies ON the standing
    // couch's cushions — their crowns sit 0.60 footprint-heights over
    // the plates (centre lifted 0.5, cushion tops at +0.10; see the
    // couch rig) — instead of hiding inside the upholstery.
    let (at, scale, lift) = if napping {
        let cell = layout::cell_rect(CABIN, rat.cell.0, rat.cell.1);
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

/// **Where the needle stands with `remaining` of the leg left.**
///
/// The one place the sweep becomes a bearing. The hand is turned by it
/// and so is every mark it is read against ([`parts`]), so a retune of
/// [`NEEDLE_SWEEP`] moves the scale and the hand together — a graduation
/// pointing at a reading the needle never reaches is a dial that lies,
/// and there is now no arithmetic left for the two to disagree in.
fn eta_bearing(remaining: f32) -> Quat {
    Quat::from_rotation_z(remaining.mul_add(NEEDLE_SWEEP, -NEEDLE_SWEEP * 0.5))
}

/// The graduated fractions of the leg the dial is pipped at. Arrival is
/// not among them: it carries a mark of its own, because the whole
/// question the scale exists to answer is which end it is.
const ETA_PIPS: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

/// The chapter ring the marks stand on, as a fraction of the gauge's own
/// cell. Outside the reach of the hand and inside the bezel's rim: a
/// scale the needle sweeps across is a scale you cannot read while it is
/// being read, and a mark past the bezel is a mark off the instrument.
const ETA_RING: f32 = 0.345;

/// The arrival mark's emissive with a whole leg still to run, and with
/// none of it left. It rests under the needle's own 1.6 — the hand is
/// the brightest thing on a face that is not being arrived at — and
/// wakes over it, because a gauge that has run out is the one thing on
/// the wall worth looking at.
const ARRIVAL_REST: f32 = 0.9;
const ARRIVAL_WAKE: f32 = 3.4;

/// How sharply the arrival mark wakes as the leg closes. Cubed: nearly
/// all of it in the last quarter and nearly none of it before, which is
/// what "nearly there" ought to look like on a leg whose middle is
/// uneventful.
const ARRIVAL_CURVE: i32 = 3;

/// The ETA gauge pieces read the leg: needle at the top of its sweep
/// when a course is armed at the dock, draining as the leg completes,
/// resting at empty otherwise — the console arc's reading, carried by
/// the instrument that owns it now.
///
/// And the arrival mark burns up as the hand closes on it, so the end of
/// the sweep says which end it is from across the room, before the
/// needle is near enough to read against a graduation.
fn eta_needles(
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut needles: Query<(&EtaNeedle, &mut Transform)>,
    marks: Query<&MeshMaterial3d<StandardMaterial>, With<EtaArrival>>,
) {
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
    let spin = eta_bearing(remaining);
    for (needle, mut transform) in &mut needles {
        transform.rotation = spin;
        transform.translation = Vec3::new(0.0, 0.0, 7.2) + spin * Vec3::new(0.0, needle.reach, 0.0);
    }
    let level = (1.0 - remaining)
        .powi(ARRIVAL_CURVE)
        .mul_add(ARRIVAL_WAKE - ARRIVAL_REST, ARRIVAL_REST);
    for mark in &marks {
        if let Some(mut mat) = materials.get_mut(&mark.0) {
            mat.emissive = palette::AMBER.to_linear() * level;
        }
    }
}

/// Every launch handle's ride: the thunk throw on departure, the
/// rattle on a refused pull, rest against the slot's stop otherwise —
/// the console face's lever motion, carried by the instrument that
/// owns it now. A live pull overrides both clocks: the handle is in
/// the hand, and the travel is the gesture layer's own (`gesture.rs`
/// never learned the lever moved; it still watches `LAUNCH_LEVER`).
fn lever_motion(
    time: Res<Time>,
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    grips: Res<crate::gesture::Grips>,
    mut juice: ResMut<LeverJuice>,
    mut handles: Query<&mut Transform, With<LeverHandle>>,
) {
    let dt = time.delta_secs();
    juice.thunk = (juice.thunk - dt).max(0.0);
    juice.shake = (juice.shake - dt).max(0.0);
    for cue in shell.bridge.sim.cues() {
        match cue {
            Cue::Depart => juice.thunk = THUNK_LEN,
            Cue::Reject { hard: false } if layout::LAUNCH_LEVER.contains(pointer.sim) => {
                juice.shake = SHAKE_LEN;
            }
            _ => {}
        }
    }
    // The thunk throws the handle over fast, then eases it home; the
    // rattle jitters it around its rest. The 2D lever's envelope.
    let heat = juice.thunk / THUNK_LEN;
    let pull = if heat > 0.65 {
        (1.0 - heat) / 0.35
    } else {
        heat / 0.65
    }
    .max(grips.launch.travel);
    let shake = (time.elapsed_secs() * 70.0).sin() * 0.05 * (juice.shake / SHAKE_LEN);
    let angle = pull.mul_add(LEVER_THROW, LEVER_REST) + shake;
    for mut handle in &mut handles {
        handle.rotation = Quat::from_rotation_x(angle);
    }
}

/// The go-lamps and their halos: lit and breathing while a pull would
/// depart, dark glass while the sim would refuse one.
fn lever_lamp(
    time: Res<Time>,
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lamps: Query<&MeshMaterial3d<StandardMaterial>, With<LeverLamp>>,
    halos: Query<&MeshMaterial3d<StandardMaterial>, With<LeverHalo>>,
) {
    let sim = &shell.bridge.sim;
    let ship = sim.ship();
    // The gangway law's own reading: nothing of the player's may rest in
    // a room that is only alongside, or the lever would strand it.
    let pullable = matches!(ship.state, ShipState::Docked(_))
        && ship.selected.is_some()
        && !sim.pieces().iter().any(|piece| {
            matches!(piece.loc, Loc::Hold { room, .. } | Loc::Laid { room, .. }
                if !sim.rooms().riding(room))
                && player_owned(sim.rooms(), sim.pieces(), piece.loc)
        });
    // Decoration: the go-glow breathes gently while a pull would work.
    // Hover feedback: pointing at the lever wakes its lamp faintly even
    // when the pull would refuse — "this is a thing", never "this is
    // ready". The lamps are the cabin's affordance language.
    let hovered = layout::LAUNCH_LEVER.contains(pointer.sim);
    let breath = glow::breathe(time.elapsed_secs(), 2.2, 0.0).mul_add(0.24, 0.66);
    let level = if pullable {
        if hovered { breath.max(0.95) } else { breath }
    } else if hovered {
        0.18
    } else {
        0.0
    };
    for lamp in &lamps {
        if let Some(mut mat) = materials.get_mut(&lamp.0) {
            glow::set_lamp(&mut mat, palette::LAMP_OK, level);
        }
    }
    let strength = if pullable { breath * 0.55 } else { 0.0 };
    for halo in &halos {
        if let Some(mut mat) = materials.get_mut(&halo.0) {
            mat.emissive = palette::LAMP_OK.to_linear() * strength;
        }
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
    /// tank's map and the destination preview's glass. `None` in
    /// headless paths; the builders fall back to phosphor.
    map_image: Option<Handle<Image>>,
    preview_image: Option<Handle<Image>>,
    /// Whether the void is standing. A window's glass is hung DARK and
    /// dressed by `viewport::aim_skies` every frame — which sky it reads
    /// and which rectangle of it is its own are facts about where the
    /// crew hung it, so the rig neither knows nor asks.
    sky: bool,
    root: Entity,
    /// The amber grab's own emissive, filled in by [`carry_grab`] on
    /// the kinds that wear one — [`hover_glint`] flares it.
    grab: Option<Handle<StandardMaterial>>,
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
}

/// The carry-handle law (BAY.md, "The handle rule"): a click-functional
/// kind declares the sub-rect of its footprint that grabs as cargo, as
/// fractions of the piece rect in sim orientation (+y down). A press
/// inside routes to carry; anywhere else on the piece, to focus. The
/// rig draws the amber grab from THIS declaration ([`grab_parts`]), so
/// hitbox and geometry cannot drift apart. `None` = passive cargo:
/// nothing to guard, the whole body grabs.
pub const fn carry_handle(kind: Kind) -> Option<Rect> {
    match kind {
        Kind::ChartTank | Kind::LaunchLever => Some(Rect::new(0.25, 0.80, 0.50, 0.16)),
        _ => None,
    }
}

/// The launch handle's own panel: [`layout::LAUNCH_LEVER`] with a
/// working margin around it, so the pull has room to start and the
/// gesture layer — which only ever hears about the lever rect — needs
/// no word of the move. The rect is the law; the binding travels.
const LEVER_PANEL: Rect = Rect::new(
    layout::LAUNCH_LEVER.x - LEVER_MARGIN,
    layout::LAUNCH_LEVER.y - LEVER_MARGIN,
    layout::LAUNCH_LEVER.w + 2.0 * LEVER_MARGIN,
    layout::LAUNCH_LEVER.h + 2.0 * LEVER_MARGIN,
);

/// The margin above, in sim units.
const LEVER_MARGIN: f32 = 20.0;

/// How an instrument piece carries its station (BAY.md, "Instruments
/// as cargo"): a mounted instrument hangs its `SimSurface` on its own
/// cells, so rulings, tapes, and `layout`'s rects never hear that the
/// hardware moved. The face fractions and the plane depth are the same
/// numbers the rig builds its glass from, so — like the carry handle —
/// the mapping and the geometry cannot drift apart.
#[derive(Clone, Copy)]
pub struct Instrument {
    /// Which station this face answers as.
    pub station: Station,
    /// The sim rect the face is bound to.
    pub rect: Rect,
    /// Fractions of the footprint (w, h) the face covers.
    pub face: (f32, f32),
    /// The face's depth in rig-local sim units, off the berth plane.
    pub plane: f32,
}

/// The instrument mount table: every click-functional kind and the
/// station it carries. Passive glass (window, gauge, preview) reads
/// without being worked, so it binds no station at all.
#[must_use]
pub const fn instrument(kind: Kind) -> Option<Instrument> {
    match kind {
        Kind::ChartTank => Some(Instrument {
            station: Station::Map,
            rect: layout::MAP_PANEL,
            face: (0.78, 0.72),
            plane: 11.0,
        }),
        Kind::LaunchLever => Some(Instrument {
            station: Station::Lever,
            rect: LEVER_PANEL,
            face: (0.72, 0.88),
            plane: 5.5,
        }),
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

/// The amber grab's resting emissive, and the brightness it flares to
/// while the crosshair actually rests on it. Two levels of the one
/// lamp: the handle says "movable" always and "movable NOW" under the
/// aim — a brightness signal, never hue alone.
const GRAB_GLOW: f32 = 1.2;
const GRAB_FLARE: f32 = 4.0;

/// How much of its declared band the drawn crossbar fills: a hair in
/// from the edges, so the amber reads as hardware bolted inside the
/// region rather than as a rectangle painted over it — and so every
/// texel of grab the player can see is inside the band that routes.
const GRAB_BAR_W: f32 = 0.9;
const GRAB_BAR_H: f32 = 0.55;

/// The declared handle band in RIG-LOCAL units — centre and size, +y up
/// about the rig's own middle. One derivation of the one declaration,
/// spent twice: the rig draws its amber from it and the sweep test
/// reads the drawing back off it, so bar and band cannot drift apart
/// unwatched.
fn grab_bar(kind: Kind, fw: f32, fh: f32) -> Option<(Vec2, Vec2)> {
    let frac = carry_handle(kind)?;
    // Fractions (+y down) to rig-local (+y up), about the rig centre.
    let cx = frac.w.mul_add(0.5, frac.x) - 0.5;
    let cy = 0.5 - frac.h.mul_add(0.5, frac.y);
    Some((
        Vec2::new(cx * fw, cy * fh),
        Vec2::new(frac.w * fw, frac.h * fh),
    ))
}

/// The live screen textures handed down to the instrument builders.
#[derive(Default)]
struct ScreenGlasses {
    map: Option<Handle<Image>>,
    preview: Option<Handle<Image>>,
    sky: bool,
}

/// Spawn one piece's whole rig at `place`: the kind's silhouette in local
/// **The carry tell**: the emissive wireframe the carry and the hover
/// light, plus the refusal slash across it. Returns
/// `(frame root, frame material, slash)`, all dark and hidden until
/// something wakes them.
///
/// A wireframe BOX, not a flat rectangle: the tell wraps the body's
/// volume (playtest: a fixed-plane rectangle around a 3D object read as
/// UI debris). It is [`Tell::Aim`]'s closed ring round the body
/// [`drawn_box`] describes — the same box the pick face is cut from
/// ([`standing_face`]), so what lights up is what answers.
///
/// **It wraps the body's own depth and not the rig band it is composed
/// in.** The band is a whole cell deep for every kind alike, so a ring
/// cut from it stood half a metre out of the wall around a painting a
/// finger thick — the same defect the standing tells had, one reading
/// over.
fn carry_tell(
    rig: &mut RigParts<'_, '_, '_>,
    piece: &Piece,
    root: Entity,
    shared: &SharedBits,
) -> (Entity, Handle<StandardMaterial>, Entity) {
    let frame_mat = glow::phosphor(rig.materials, palette::LAMP_OK, 0.0);
    let frame_root = rig
        .commands
        .spawn((Transform::default(), Visibility::Hidden, ChildOf(root)))
        .id();
    let (mid, half) = drawn_box(piece.kind);
    let cube = rig.skin.cube.clone();
    for bar in tell_bars(mid, half, Tell::Aim) {
        rig.commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_translation(bar.at).with_scale(bar.size),
            ChildOf(frame_root),
        ));
    }
    let (hx, hy) = (half.x + AIM_OUT, half.y + AIM_OUT);
    let slash = rig
        .commands
        .spawn((
            Mesh3d(cube),
            MeshMaterial3d(shared.slash.clone()),
            Transform::from_xyz(mid.x, mid.y, 34.0)
                .with_rotation(Quat::from_rotation_z((hy / hx).atan()))
                .with_scale(Vec3::new((hx * 2.0).hypot(hy * 2.0), 3.0, 3.0)),
            Visibility::Hidden,
            ChildOf(frame_root),
        ))
        .id();
    (frame_root, frame_mat, slash)
}

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
    let (w, h) = piece.kind.upright();
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
        sky: glasses.sky,
        root: body_root,
        grab: None,
    };
    build_kind(&mut rig, piece);
    let grab_mat = rig.grab.clone();

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
    let (frame_root, frame_mat, slash) = carry_tell(&mut rig, piece, root, shared);

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
        grab_mat,
    });
    root
}

/// How a window's glass is framed. It is the only thing that tells the
/// family's sizes apart from a pace back — the void behind all three is
/// the same void, and it had better be, because it is the same sky
/// (`viewport`, "One wall, one sky").
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bezel {
    /// A bolt ring around a round bore. The porthole, and the reason the
    /// aperture math never had to learn about circles: the glass behind
    /// the ring is still a rectangle, so the sky is still projected
    /// through a rectangle — the brass simply eats the corners. What the
    /// crew sees through a round hole is exactly what is out there,
    /// because occluding a correct picture leaves a correct picture.
    Ring,
    /// Four brass lips around a rectangle: the transit window, as it has
    /// always been.
    Lipped,
    /// Lips, plus the mullion four cells of glass cannot arrive
    /// without. Saturn ships the bay window in two crates, and the
    /// frame says so.
    Mullioned,
}

/// Which bezel a window wears. One arm per size, and adding a fourth
/// size is an arm HERE rather than a second copy of the glass.
const fn bezel(kind: Kind) -> Option<Bezel> {
    match kind {
        Kind::Porthole => Some(Bezel::Ring),
        Kind::Window => Some(Bezel::Lipped),
        Kind::BayWindow => Some(Bezel::Mullioned),
        _ => None,
    }
}

// -------------------------------------------------------------- the parts --

/// **The primitive bodies a rig is cut from**, in rig-local sim units.
///
/// A short list, and the same argument the stations' own
/// [`Shape`](crate::poi::Shape) list makes: cargo is told apart by
/// arrangement, not by modelling. What is here is what thirty-two kinds
/// actually spend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Body {
    /// A box, `x × y × z`.
    Box(Vec3),
    /// A cylinder standing on its own `+y`, `facets` sides round it —
    /// `None` where the mesher's own smooth default is wanted.
    Drum { r: f32, h: f32, facets: Option<u32> },
    /// A true cone: a disc at the bottom of its own `+y`, a point at the
    /// top.
    Horn { r: f32, h: f32 },
    /// A torus lying in its own `x/z` plane.
    Hoop { inner: f32, outer: f32 },
    /// A capsule standing on its own `+y`: `len` of barrel between two
    /// round caps. Nothing about it is flat.
    Pill { r: f32, len: f32 },
    /// A chunky low-poly sphere.
    Ball { r: f32 },
    /// A flat annulus in its own `x/y` plane, showing its own `+z`.
    Washer { bore: f32, brim: f32, facets: u32 },
    /// A single-sided unit quad in its own `x/y` plane, showing its own
    /// `+z`. Its size rides the transform's scale rather than its mesh,
    /// because a pane wearing a live texture is re-scaled, never re-cut.
    Pane,
}

impl Body {
    /// The body's half-extents in its own frame, before the transform's
    /// own scale. A sheet's is zero on the axis it has no thickness in,
    /// which is the truth about it: a sheet IS a face.
    #[must_use]
    pub fn half(self) -> Vec3 {
        match self {
            Self::Box(size) => size.abs() * 0.5,
            Self::Drum { r, h, .. } | Self::Horn { r, h } => Vec3::new(r, h * 0.5, r),
            Self::Hoop { inner, outer } => Vec3::new(outer, (outer - inner) * 0.5, outer),
            Self::Pill { r, len } => Vec3::new(r, len.mul_add(0.5, r), r),
            Self::Ball { r } => Vec3::splat(r),
            Self::Washer { brim, .. } => Vec3::new(brim, brim, 0.0),
            Self::Pane => Vec3::new(0.5, 0.5, 0.0),
        }
    }

    /// Whether the body is a single-sided sheet: one face, no volume,
    /// and the face it shows is its own `+z`.
    #[must_use]
    pub const fn sheet(self) -> bool {
        matches!(self, Self::Washer { .. } | Self::Pane)
    }

    /// Which of the shared silhouettes the body reads as, for a caller
    /// asking which of its box's sides the renderer actually draws.
    /// Solids only — a sheet answers [`Self::sheet`] instead, because a
    /// sheet's one face is not a side of any box.
    ///
    /// The drum's narrow cap reads wide here, the same way it does for a
    /// station's fittings: a true cone's top is a POINT and `Shape::Cone`
    /// is a frustum with a real disc up there, so a cone is described
    /// with one more flat side than it has. That errs toward reporting,
    /// which is the direction a detector is allowed to err in.
    #[must_use]
    pub const fn shape(self) -> Shape {
        match self {
            Self::Box(_) | Self::Washer { .. } | Self::Pane => Shape::Slab,
            Self::Drum { .. } => Shape::Post,
            Self::Horn { .. } => Shape::Cone,
            Self::Hoop { .. } | Self::Pill { .. } | Self::Ball { .. } => Shape::Dome,
        }
    }

    /// The mesh, cut once per distinct body per rig.
    fn mesh(self) -> Mesh {
        match self {
            Self::Box(size) => Cuboid::new(size.x, size.y, size.z).into(),
            Self::Drum {
                r,
                h,
                facets: Some(sides),
            } => Cylinder::new(r, h).mesh().resolution(sides).build(),
            Self::Drum { r, h, facets: None } => Cylinder::new(r, h).into(),
            Self::Horn { r, h } => Cone {
                radius: r,
                height: h,
            }
            .into(),
            Self::Hoop { inner, outer } => Torus::new(inner, outer).into(),
            Self::Pill { r, len } => Capsule3d::new(r, len).into(),
            Self::Ball { r } => ico(r),
            Self::Washer { bore, brim, facets } => {
                Annulus::new(bore, brim).mesh().resolution(facets).build()
            }
            Self::Pane => Rectangle::new(1.0, 1.0).into(),
        }
    }
}

/// Which of a rig's live screens have a picture to wear.
///
/// A headless build has no void to look into and no rasteriser running,
/// and the rigs that would wear one fall back to phosphor — with
/// different geometry, not merely a different coat. So the description
/// has to be told which of the two worlds it is describing, rather than
/// guess and be right in one of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Screens {
    pub sky: bool,
    pub map: bool,
    pub preview: bool,
}

impl Screens {
    /// Every screen lit: the game as it is played.
    pub const LIVE: Self = Self {
        sky: true,
        map: true,
        preview: true,
    };
    /// None of them: a headless boot, and the fallbacks it draws.
    pub const DARK: Self = Self {
        sky: false,
        map: false,
        preview: false,
    };
    /// Both worlds, for a sweep that has to judge each of them.
    pub const BOTH: [Self; 2] = [Self::LIVE, Self::DARK];
}

/// How a part is finished.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cut {
    /// One of the shared coats — enamel, etched, phosphor, or a worn
    /// metal off the ship's own [`Skin`].
    Coat(Coat),
    /// The chart tank's live map, painted by the CRT rasteriser.
    Map,
    /// The destination preview's live glass.
    Preview,
    /// The void, seen through a window's pane. Hung dark; `viewport`
    /// dresses it every frame.
    Sky,
    /// One seeded artwork, painted through the shared canvas.
    Art,
}

/// The turned sub-frames a rig hangs parts in, and what moves each one.
///
/// Most of a rig hangs straight off its own body. What does not hangs
/// off a sub-root some system owns: the runtime swings the sconce's arm,
/// throws the launch handle's pivot, and shows exactly one of a
/// covering's two bodies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Under {
    /// The rig's own body.
    Rig,
    /// The sconce's bracket arm, which `sync_fixtures` swings to
    /// whichever stile the piece's wall column touches.
    Arm,
    /// The launch handle's pivot, at the pose it rests in;
    /// `lever_motion` throws it with the gesture layer's travel.
    Pivot(Transform),
    /// A covering's laid body, shown while it lies on its chart.
    Laid,
    /// A covering's packed body, shown while it stands on a counter.
    Packed,
}

impl Under {
    /// Whether two parts hang in the same sub-frame. A pose is not an
    /// identity — the pivot is one frame whatever it rests at.
    fn same(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }

    /// The pose the sub-frame rests at.
    #[must_use]
    pub const fn rest(self) -> Transform {
        match self {
            Self::Pivot(at) => at,
            _ => Transform::IDENTITY,
        }
    }
}

/// What the runtime does with a part beyond drawing it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    /// Geometry, and nothing else.
    Plain,
    /// A breathing emissive. Whatever else wears the same coat breathes
    /// with it, which is how the eerie crates beat as one frame off one
    /// marked bar.
    Pulse {
        color: Color,
        base: f32,
        amp: f32,
        freq: f32,
        phase: f32,
    },
    /// A seedling's bud, hidden until the berth stands in lamplight.
    Bud,
    /// A cubby's amber invitation, hidden until the sim offers the slot.
    /// It breathes on a phase of its own, so it is the one role that
    /// insists on a material instance of its own.
    Cubby {
        slot: u8,
    },
    /// The ETA gauge's needle, swept by the live leg.
    Needle {
        reach: f32,
    },
    /// The empty end of the ETA gauge's sweep: the mark the needle
    /// arrives at, which burns up as it closes. Its own material, like
    /// the cubby's, because two gauges aboard read two different legs
    /// only if they are not sharing one amber.
    Arrival,
    /// The launch handle's go-lamp, and the halo behind it.
    Knob,
    Halo,
    /// A lamp's glass, with the room light under it at this reach.
    Bulb {
        range: f32,
    },
    /// A luminous coat's own tinge: a light, and no body at all.
    Tinge,
    /// The amber carry grab, which the aim flares.
    Grab,
}

impl Role {
    /// Whether the part must own its material outright rather than share
    /// the rig's instance of its coat. Only the cubby does: four mouths
    /// wearing one amber would breathe as one, and they are meant to
    /// breathe out of step (`invite_glows`).
    const fn alone(self) -> bool {
        matches!(self, Self::Cubby { .. } | Self::Arrival)
    }

    /// Whether the part is hung hidden for a system to show later.
    const fn dark(self) -> bool {
        matches!(self, Self::Bud | Self::Cubby { .. })
    }
}

/// **One part of one rig, described and not yet built**: what it is cut
/// from, how it is finished, where it stands in its own frame, and
/// whatever claim it makes about the way it points.
///
/// The reason it is data. `build_kind` used to compose a rig straight
/// into a live Bevy world, which meant nothing pure could enumerate a
/// rig's parts and no sweep could ask a question about one — the Guild's
/// chit cut its card and its stripe to the same height and the same
/// centre, so top edges shared a plane and bottoms shared another, along
/// the whole of a stripe held at arm's length, and the gauntlet could not
/// have caught it. The rooms went this way first (`room::charts`,
/// `room::sites`, `room::tiles`): a describer says what is there, and the
/// presentation layer stamps what it returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    /// What the part is called in its own rig.
    pub what: &'static str,
    /// Which of a repeated part this is, where a rig draws several.
    pub nth: Option<u8>,
    /// The silhouette — `None` for a part that lights and draws nothing.
    pub body: Option<Body>,
    /// How it is finished.
    pub cut: Cut,
    /// Where it stands in its frame.
    pub at: Transform,
    /// Which frame that is.
    pub under: Under,
    /// What the runtime does with it beyond drawing it.
    pub role: Role,
    /// What it claims about the way it points, if it claims anything.
    pub claim: Option<Feature>,
    /// What it claims holds it up, if it claims anything.
    pub seat: Option<Seat>,
}

impl Part {
    /// One part, spelled out.
    const fn new(what: &'static str, body: Body, coat: Coat, at: Transform) -> Self {
        Self {
            what,
            nth: None,
            body: Some(body),
            cut: Cut::Coat(coat),
            at,
            under: Under::Rig,
            role: Role::Plain,
            claim: None,
            seat: None,
        }
    }

    /// A light with no body of its own.
    const fn lamp(what: &'static str, coat: Coat, at: Transform, role: Role) -> Self {
        Self {
            what,
            nth: None,
            body: None,
            cut: Cut::Coat(coat),
            at,
            under: Under::Rig,
            role,
            claim: None,
            seat: None,
        }
    }

    /// Which of a repeated part this is.
    const fn nth(mut self, nth: u8) -> Self {
        self.nth = Some(nth);
        self
    }

    /// Finished by something other than a coat — a live screen, the
    /// void, or a painted canvas.
    const fn cut(mut self, cut: Cut) -> Self {
        self.cut = cut;
        self
    }

    /// Hung in a sub-frame rather than off the rig's own body.
    const fn under(mut self, under: Under) -> Self {
        self.under = under;
        self
    }

    /// Given a job beyond being drawn.
    const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Scaled — the one thing a pane's size rides, and the couch
    /// cushions' squash.
    const fn scaled(mut self, scale: Vec3) -> Self {
        self.at.scale = scale;
        self
    }

    /// **Turned by the claim its own name makes.** The turn is derived
    /// here and nowhere else: a claim-bearing part is a body of
    /// revolution about `axis`, so any turn carrying `axis` onto `want`
    /// draws the same body and the shortest one is as good as any. A
    /// builder and a name have nothing left to disagree about.
    fn pointing(mut self, axis: Vec3, want: Vec3) -> Self {
        let claim = Feature {
            name: self.what,
            axis,
            want,
        };
        self.at.rotation = claim.turn();
        self.claim = Some(claim);
        self
    }

    /// **Bolted to another part of the same rig**, named by that part's
    /// own `what` — see [`Seat`]. Declared rather than derived, because
    /// a joint is a promise the builder makes and not a distance the
    /// sweep can guess at.
    const fn seated(mut self, on: &'static str) -> Self {
        self.seat = Some(Seat {
            name: self.what,
            on,
        });
        self
    }

    /// The name a finding files the part under.
    #[must_use]
    pub fn label(&self) -> String {
        self.nth
            .map_or_else(|| self.what.to_owned(), |n| format!("{}[{n}]", self.what))
    }
}

/// The amber carry grab: a glowing crossbar in two brass stanchions,
/// drawn exactly over the declared handle sub-rect ([`grab_bar`]) so the
/// hitbox and the geometry cannot drift apart. `z` is the local depth
/// the bar rides at. Empty for a passive kind, which declares no handle.
fn grab_parts(kind: Kind, fw: f32, fh: f32, z: f32) -> Vec<Part> {
    let Some((at, size)) = grab_bar(kind, fw, fh) else {
        return Vec::new();
    };
    let (hx, hy) = (at.x, at.y);
    let (hw, hh) = (size.x, size.y);
    let mut out = vec![
        Part::new(
            "carry grab",
            Body::Box(Vec3::new(hw * GRAB_BAR_W, hh * GRAB_BAR_H, 2.6)),
            Coat::phosphor(palette::AMBER, GRAB_GLOW),
            Transform::from_xyz(hx, hy, z),
        )
        .role(Role::Grab),
    ];
    for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
        out.push(
            Part::new(
                "grab stanchion",
                Body::Box(Vec3::new(hh * 0.4, hh * 0.4, z - 0.5)),
                Coat::metal(Worn::Brass),
                Transform::from_xyz(
                    sx.mul_add(hw * GRAB_BAR_W * 0.5, hx),
                    hy,
                    (z - 0.5).mul_add(0.5, 0.5),
                ),
            )
            .nth(u8::try_from(i).unwrap_or(0)),
        );
    }
    out
}

/// A squat paint tin standing on its base at `sole`, `deep` into its
/// cell: `shell` for the body, `lid` capping it — the one silhouette
/// both paints share.
///
/// **It used to be drawn end-on**, a disc of lid in front of a disc of
/// shell, which is what a tin looks like as a glyph on a flat console
/// and is a tin lying on its side once there is a deck under it. Both
/// paints ride this, so both of them fell over together and both of them
/// stand up together.
fn tin_parts(shell: Coat, lid: Coat, sole: f32, deep: f32) -> Vec<Part> {
    let drum = 11.0;
    vec![
        Part::new(
            "tin shell",
            Body::Drum {
                r: 9.5,
                h: drum,
                facets: None,
            },
            shell,
            Transform::from_xyz(0.0, sole + drum * 0.5, deep),
        ),
        // Proud of the shell and straddling its rim, which is what a lid
        // pressed onto a tin looks like and keeps the two off one plane.
        Part::new(
            "tin lid",
            Body::Drum {
                r: 9.7,
                h: 1.6,
                facets: None,
            },
            lid,
            Transform::from_xyz(0.0, sole + drum + 0.4, deep),
        ),
    ]
}

/// A porthole's bolt head, and how far it stands out of the brass it is
/// torqued into. The ball's own radius less its proudness is how deep
/// the bolt sinks behind the ring, and that depth is the whole stand-off
/// the ring assembly hangs at ([`ring_z`]).
const STUD_R: f32 = 1.5;
const STUD_PROUD: f32 = 0.6;

/// How far a headless pane's stand-in stars ride in front of the glass
/// they are scattered on, in rig-local sim units. Stated off the glass
/// rather than at a depth of their own, so a bezel that moves takes its
/// own sky with it instead of leaving it hanging in front of the brass.
const STAR_PROUD: f32 = 0.2;

/// **Where a bolt ring's brass hangs**, in rig-local sim units: far
/// enough off the berth plane that the bolts sunk behind it just reach
/// it, and no further. A porthole is a bore cut in a hull with a ring
/// torqued over it, so the bolts are what touch the ship and the ring is
/// what they hold on.
///
/// Only the ring bezel has an answer to give: the lipped and mullioned
/// bezels are boxes six units deep that already stand on the plane.
const fn ring_z(bezel: Bezel) -> f32 {
    match bezel {
        Bezel::Ring => STUD_R - STUD_PROUD,
        Bezel::Lipped | Bezel::Mullioned => 0.0,
    }
}

/// The whole window family, described: a frame around a hole in the hull.
///
/// The glass carries `viewport::SkyPane` — the whole contract between
/// this furniture and the view — and it is hung DARK, because what is
/// out there depends on where the crew put it and the furniture is in no
/// position to know. `viewport::aim_skies` dresses it every frame: the
/// aperture the void is seen through IS this quad, wherever the crew
/// rehang it (the whimsy rule made physical; the void follows), and
/// panes sharing a wall share one render of it.
///
/// Headless paths, which have no void to look into, fall back to a
/// phosphor pane with a stand-in star scatter seeded by the piece id.
#[allow(clippy::too_many_lines)]
fn window_parts(piece: &Piece, color: Color, fw: f32, fh: f32, screens: Screens) -> Vec<Part> {
    let Some(bezel) = bezel(piece.kind) else {
        return Vec::new();
    };
    let brass = Coat::metal(Worn::Brass);
    // How much of the footprint is glass. A porthole's bore is round, so
    // its pane is square and small enough for the ring to cover its
    // corners; the rectangular bezels take almost the whole cell.
    let (gw, gh) = match bezel {
        Bezel::Ring => {
            let m = fw.min(fh) * 0.60;
            (m, m)
        }
        Bezel::Lipped => (fw * 0.88, fh * 0.78),
        Bezel::Mullioned => (fw * 0.90, fh * 0.84),
    };
    // Which member of this bezel the glass is glazed behind, where the
    // glass itself hangs, and where a headless slab's own back face
    // sits: a ring is a flat annulus at one plane, four lips are a box
    // the pane sits inside. The pane meets its frame either way
    // (`gauntlet`, `part-seated`) — the ring's used to stand nine
    // millimetres clear of the brass it is supposedly bolted into, and
    // only the played build drew it that way.
    //
    // **A bore is cut in a hull, so a porthole begins at one.** The
    // brass hung at the middle of the band every rig is composed in,
    // which put the whole fitting — glass, ring and bolts — a good
    // three centimetres out in the air in front of the wall it is
    // torqued down to, with the room's own paint visible under the
    // brim. Every other kind that hangs on a wall puts its backmost
    // body on the plane and this one now does too (`gauntlet`,
    // `rig-seated`): the assembly is carried back until the BOLTS reach
    // the hull, and every member keeps the depth it was drawn at
    // relative to them, so what moved is where the fitting begins and
    // not how it is put together. The ring's own annulus clears the
    // decal ladder's flat paint with room to spare, which a sheet of
    // brass on a wall has to.
    let (frame, glass_z, glass_back) = match bezel {
        Bezel::Ring => ("bolt ring", ring_z(bezel) - GLAZE, 0.0),
        // The deep bezels are boxes standing on the wall already; a
        // headless slab keeps clear of the plane their backmost member
        // begins at, since two opaque faces at one depth are a coin toss.
        Bezel::Lipped | Bezel::Mullioned => ("lip", 2.4, 0.4),
    };
    let mut out = Vec::new();
    if screens.sky {
        out.push(
            Part::new(
                "sky pane",
                Body::Pane,
                Coat::phosphor(palette::SHADOW, 0.0),
                Transform::from_xyz(0.0, 0.0, glass_z),
            )
            .cut(Cut::Sky)
            .scaled(Vec3::new(gw, gh, 1.0))
            .seated(frame),
        );
    } else {
        // Inside the bezel's own depth at both ends: cut to 0..3 it
        // shared the lips' back plane and the bolt ring's front one, so
        // a headless boot drew the frame and the glass at one depth. It
        // runs out to the same face the played pane hangs at, so the
        // slab and the glass make the same joint with the same frame.
        out.push(
            Part::new(
                "sky pane",
                Body::Box(Vec3::new(gw, gh, glass_z - glass_back)),
                Coat::phosphor(palette::mix(color, palette::PHOSPHOR, 0.12), 0.35),
                Transform::from_xyz(0.0, 0.0, f32::midpoint(glass_back, glass_z)),
            )
            .seated(frame),
        );
        let star = Coat::phosphor(palette::GLINT, 2.2);
        for i in 0..7_u32 {
            let n = (piece.id.wrapping_mul(7).wrapping_add(i)) as f32;
            let angle = n * 2.399;
            let reach = (n * 0.517).fract().mul_add(0.36, 0.08);
            out.push(
                Part::new(
                    "star",
                    Body::Ball { r: 0.9 },
                    star,
                    Transform::from_xyz(
                        angle.cos() * gw * reach,
                        angle.sin() * gh * reach,
                        glass_z + STAR_PROUD,
                    ),
                )
                .nth(u8::try_from(i).unwrap_or(0)),
            );
        }
    }
    match bezel {
        // The bolt ring: a flat brass annulus whose bore is the visible
        // glass, and the studs that say somebody torqued it down. Twelve
        // facets, not a smooth circle — the crunch does not do smooth.
        Bezel::Ring => {
            let bore = gw * 0.5;
            let brim = fw.min(fh) * 0.47;
            let ring_z = ring_z(bezel);
            out.push(Part::new(
                "bolt ring",
                Body::Washer {
                    bore,
                    brim,
                    facets: 12,
                },
                brass,
                Transform::from_xyz(0.0, 0.0, ring_z),
            ));
            for i in 0..6_u32 {
                let angle = (i as f32) * std::f32::consts::TAU / 6.0;
                let reach = f32::midpoint(bore, brim);
                out.push(
                    Part::new(
                        "stud",
                        Body::Ball { r: STUD_R },
                        brass,
                        Transform::from_xyz(
                            angle.cos() * reach,
                            angle.sin() * reach,
                            ring_z + STUD_PROUD,
                        ),
                    )
                    .nth(u8::try_from(i).unwrap_or(0))
                    .seated("bolt ring"),
                );
            }
        }
        // Four lips around the rectangle, and — where the glass is too
        // wide to have arrived in one sheet — the mullion between the
        // two panes that did.
        Bezel::Lipped | Bezel::Mullioned => {
            // The uprights sit inside the rails on both counts: a hair
            // in of the rails' outer edge and a hair shallower than
            // their depth, because four lips cut to one rectangle share
            // a plane at every corner they cross.
            let lip_h = Body::Box(Vec3::new(fw * 0.96, fh * 0.10, 6.0));
            let lip_v = Body::Box(Vec3::new(fw * 0.05, fh * 0.94, 5.4));
            for (i, (body, at)) in [
                (lip_h, Vec3::new(0.0, fh * 0.43, 3.0)),
                (lip_h, Vec3::new(0.0, -fh * 0.43, 3.0)),
                (lip_v, Vec3::new(fw * 0.45, 0.0, 3.0)),
                (lip_v, Vec3::new(-fw * 0.45, 0.0, 3.0)),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(
                    Part::new("lip", body, brass, Transform::from_translation(at))
                        .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            if bezel == Bezel::Mullioned {
                out.push(
                    Part::new(
                        "mullion",
                        Body::Box(Vec3::new(fw * 0.035, fh * 0.86, 5.0)),
                        brass,
                        Transform::from_xyz(0.0, 0.0, 3.0),
                    )
                    .seated("lip"),
                );
            }
        }
    }
    out
}

/// **One silhouette per cargo kind**, the 2D glyph identities restated
/// as primitives and described rather than built. Variants ride the
/// tint; ids seed the decoration phases.
///
/// Pure: it spawns nothing, reads no world, and answers off a `Piece`
/// and which screens are lit. [`build_kind`] stamps exactly what it
/// returns, and `crate::gauntlet` measures exactly what it returns, so
/// the sweep and the room see one geometry.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parts(piece: &Piece, screens: Screens) -> Vec<Part> {
    let color = palette::variant_tint(palette::kind_color(piece.kind), piece.variant);
    let (w, h) = piece.kind.upright();
    let fw = f32::from(w) * layout::CELL;
    let fh = f32::from(h) * layout::CELL;
    let body = Coat::enamel(color);
    let brass = Coat::metal(Worn::Brass);
    let plate = Coat::metal(Worn::PlateShade);
    let socket = Coat::metal(Worn::Socket);
    let shaded = |mix: f32| Coat::enamel(palette::mix(color, palette::SHADOW, mix));
    let flat = Quat::from_rotation_x(FRAC_PI_2);
    // Where a standing kind's sole meets its deck, and the middle of the
    // depth a rig is composed within: the two numbers a body that stands
    // on a floor is placed off, rather than the middle of a cell face a
    // bas-relief was centred on.
    let sole = sole_of(fh);
    let deep = f32::midpoint(RIG_NEAR, RIG_FAR);
    let mut out: Vec<Part> = Vec::new();
    match piece.kind {
        // A pink rhombus with a sparkle: cut crystal, corner-on, one
        // glint down the near arris.
        // **The corner it stands on is the one facing the room.** The
        // glyph turned the cube about its depth, which read as a diamond
        // on a flat console and reads as a flask balanced on a point the
        // moment it has a deck under it. The same quarter turn taken
        // about the UPRIGHT keeps the faceted silhouette and puts the
        // thing on its base.
        Kind::PerfumeVial => {
            let flask = Vec3::new(fw * 0.52, fh * 0.52, 15.0);
            out.push(Part::new(
                "flask",
                Body::Box(flask),
                body,
                Transform::from_xyz(0.0, flask.y.mul_add(0.5, sole), deep)
                    .with_rotation(Quat::from_rotation_y(FRAC_PI_4)),
            ));
            // Straddling the near arris, which the turn brings round to
            // face the room: a corner's reach out of the flask's own
            // centre is half its two girths on the diagonal.
            let reach = core::f32::consts::SQRT_2 * 0.25;
            out.push(Part::new(
                "sparkle",
                Body::Ball { r: 2.2 },
                Coat::phosphor(palette::GLINT, 2.5),
                Transform::from_xyz(
                    (flask.x - flask.z) * reach,
                    fh.mul_add(0.38, sole),
                    (flask.x + flask.z).mul_add(reach, deep),
                ),
            ));
        }
        // A gold slab, a darker belt, a sphere head. Unimaginably tacky.
        // **An idol stands on its own base**, so the torso's foot is the
        // sole and the belt and the head are measured off the torso.
        Kind::GildedIdol => {
            let torso = Vec3::new(fw * 0.58, fh * 0.52, 18.0);
            let stand = torso.y.mul_add(0.5, sole);
            out.push(Part::new(
                "torso",
                Body::Box(torso),
                body,
                Transform::from_xyz(0.0, stand, 9.0),
            ));
            out.push(Part::new(
                "belt",
                Body::Box(Vec3::new(fw * 0.62, fh * 0.07, 19.0)),
                shaded(0.45),
                Transform::from_xyz(0.0, fh.mul_add(0.08, stand), 10.0),
            ));
            out.push(Part::new(
                "head",
                Body::Ball { r: fw * 0.26 },
                body,
                Transform::from_xyz(0.0, fh.mul_add(0.38, stand), 12.0),
            ));
        }
        // A 2×2 sub-grid of identical government flavour, the bottom
        // course standing on the deck.
        Kind::RationBricks => {
            let brick = Vec3::new(26.0, 26.0, 16.0);
            let course = brick.y.mul_add(0.5, sole);
            for (i, (ix, iy)) in [(-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                out.push(
                    Part::new(
                        "brick",
                        Body::Box(brick),
                        body,
                        Transform::from_xyz(15.5 * ix, 31.0f32.mul_add(iy, course), 8.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
        }
        // Two rust bars, stacked askew on the deck.
        Kind::ScrapAlloy => {
            let under = fh.mul_add(0.18, sole);
            out.push(Part::new(
                "under bar",
                Body::Box(Vec3::new(fw * 0.92, fh * 0.36, 10.0)),
                shaded(0.25),
                Transform::from_xyz(-fw * 0.02, under, 5.0),
            ));
            out.push(Part::new(
                "top bar",
                Body::Box(Vec3::new(fw * 0.88, fh * 0.34, 10.0)),
                body,
                Transform::from_xyz(fw * 0.02, fh.mul_add(0.30, under), 15.0),
            ));
        }
        // A pot with a sprout on top. Under lamplight it blooms: three
        // PerfumeVial-pink buds, hidden until `lit_adjacent` says the
        // footprint sits in a lit lamp's halo (presentation only, the
        // 2D bloom's reading).
        // **A pot stands on the deck and a sprout grows up out of it.**
        // Both were laid on their sides — the glyph drew a drum and a
        // cone end-on, which is a circle and a disc on a flat console
        // and is a plant pot on its side once there is a floor.
        Kind::Seedlings => {
            let pot = 12.0;
            let pot_top = sole + pot;
            let sprout = 18.0_f32;
            out.push(Part::new(
                "pot",
                Body::Drum {
                    r: fw * 0.3,
                    h: pot,
                    facets: None,
                },
                shaded(0.35),
                Transform::from_xyz(0.0, sole + pot * 0.5, deep),
            ));
            // Rooted a finger inside the pot rather than balanced on its
            // rim, which is the joint every other body in this file makes.
            let stem = sprout.mul_add(0.5, pot_top - 2.0);
            out.push(Part::new(
                "sprout",
                Body::Horn {
                    r: fw * 0.2,
                    h: sprout,
                },
                body,
                Transform::from_xyz(0.0, stem, deep),
            ));
            let bud = Coat::enamel(palette::kind_color(Kind::PerfumeVial));
            for (i, (bx, by, bz)) in [
                (-4.2, stem - 2.0, deep + 1.5),
                (4.0, stem + 0.5, deep - 1.2),
                (0.5, stem + 5.0, deep + 2.6),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(
                    Part::new(
                        "bud",
                        Body::Ball { r: 2.4 },
                        bud,
                        Transform::from_xyz(bx, by, bz),
                    )
                    .nth(u8::try_from(i).unwrap_or(0))
                    .role(Role::Bud),
                );
            }
        }
        // A horizontal capsule wearing hazard chevrons, lying on the
        // deck. It lies rather than stands because it is two cells
        // across and one course tall: a bottle that long, stood up, is
        // a bottle through the deckhead.
        Kind::GasCanister => {
            let r = 10.0;
            let lying = sole + r;
            out.push(Part::new(
                "tank",
                Body::Pill { r, len: 34.0 },
                body,
                Transform::from_xyz(0.0, lying, 10.0)
                    .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
            ));
            let warn = shaded(0.5);
            // The lower leg is the shallower one: two legs cut to one
            // depth meet along a plane where they cross, and a chevron
            // is two legs that cross.
            let legs = [
                Body::Box(Vec3::new(9.0, 3.0, 3.0)),
                Body::Box(Vec3::new(9.0, 3.0, 2.2)),
            ];
            let mut nth = 0_u8;
            for cx in [-8.0f32, 8.0] {
                for (leg, (sy, turn)) in legs
                    .into_iter()
                    .zip([(3.2f32, -FRAC_PI_4), (-3.2, FRAC_PI_4)])
                {
                    out.push(
                        Part::new(
                            "chevron",
                            leg,
                            warn,
                            Transform::from_xyz(cx - 2.5, lying + sy, 19.0)
                                .with_rotation(Quat::from_rotation_z(turn)),
                        )
                        .nth(nth),
                    );
                    nth += 1;
                }
            }
        }
        // A hexagonal prism in a frost ring.
        // **A core is a canister and a canister stands.** Both bodies
        // were drawn end-on — a hexagon inside a circle, which is the
        // glyph — so the prism lay on its side and the ring stood on its
        // rim. Upright, the ring is a collar round the core's waist,
        // which is what a frost ring on a cryogenic flask is.
        Kind::CryoCore => {
            let core = 18.0;
            let waist = sole + core * 0.5;
            out.push(Part::new(
                "core",
                Body::Drum {
                    r: fw * 0.36,
                    h: core,
                    facets: Some(6),
                },
                body,
                Transform::from_xyz(0.0, waist, deep),
            ));
            let r = fw * 0.44;
            out.push(Part::new(
                "frost ring",
                Body::Hoop {
                    inner: r - 1.4,
                    outer: r + 1.4,
                },
                Coat::enamel(palette::mix(palette::GLINT, color, 0.4)),
                Transform::from_xyz(0.0, waist, deep),
            ));
        }
        // Three stacked pearls, the middle a shade wetter, the lowest
        // resting on the deck.
        Kind::BrinePearls => {
            let r = 10.5;
            let low = sole + r;
            for (i, (coat, y)) in [(body, low + 42.0), (shaded(0.15), low + 21.0), (body, low)]
                .into_iter()
                .enumerate()
            {
                out.push(
                    Part::new(
                        "pearl",
                        Body::Ball { r },
                        coat,
                        Transform::from_xyz(0.0, y, 10.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
        }
        // Matte near-black, breathing an eerie edge frame at the audio
        // hum's ~1 Hz beat.
        Kind::SuspiciousCrate => {
            let crate_body = Vec3::new(fw * 0.84, fh * 0.84, 24.0);
            let stand = crate_body.y.mul_add(0.5, sole);
            out.push(Part::new(
                "crate",
                Body::Box(crate_body),
                body,
                Transform::from_xyz(0.0, stand, 12.0),
            ));
            let hum = Coat::phosphor(palette::EERIE, 0.5);
            // **The frame is painted on the crate's face**, so it runs
            // INSIDE the edge it traces rather than straddling it. It
            // used to be centred on the crate's own top and bottom, and
            // a crate that stands on its deck then stands on a strip of
            // light with the box floating over it.
            let rail = 2.6;
            let along = Body::Box(Vec3::new(fw * 0.86, rail, rail));
            let across = Body::Box(Vec3::new(rail, fh * 0.82, 1.8));
            let up = crate_body.y.mul_add(0.5, -rail);
            for (i, (edge, at)) in [
                (along, Vec3::new(0.0, stand + up, 24.0)),
                (along, Vec3::new(0.0, stand - up, 24.0)),
                (across, Vec3::new(fw * 0.42, stand, 24.0)),
                (across, Vec3::new(-fw * 0.42, stand, 24.0)),
            ]
            .into_iter()
            .enumerate()
            {
                let mut part = Part::new("hum edge", edge, hum, Transform::from_translation(at))
                    .nth(u8::try_from(i).unwrap_or(0));
                if i == 0 {
                    part = part.role(Role::Pulse {
                        color: palette::EERIE,
                        base: 0.6,
                        amp: 2.4,
                        freq: TAU,
                        phase: phase_of(piece.id, SALT_PULSE),
                    });
                }
                out.push(part);
            }
        }
        // A dun parcel lashed with twine, knot hand-tied off centre.
        Kind::MysteriousCrate => {
            let parcel = Vec3::new(fw * 0.8, fh * 0.8, 18.0);
            let stand = parcel.y.mul_add(0.5, sole);
            out.push(Part::new(
                "parcel",
                Body::Box(parcel),
                body,
                Transform::from_xyz(0.0, stand, 9.0),
            ));
            let twine = shaded(0.4);
            out.push(
                Part::new(
                    "twine",
                    Body::Box(Vec3::new(fw * 0.84, 2.4, 2.0)),
                    twine,
                    Transform::from_xyz(0.0, stand, 18.4),
                )
                .nth(0),
            );
            // The upright lashing stops a hair short of the parcel's own
            // ends: run out past them it is what meets the deck, and a
            // parcel standing on its string is a parcel standing on air.
            out.push(
                Part::new(
                    "twine",
                    Body::Box(Vec3::new(2.4, fh * 0.78, 1.4)),
                    twine,
                    Transform::from_xyz(-fw * 0.06, stand, 18.4),
                )
                .nth(1),
            );
            out.push(Part::new(
                "knot",
                Body::Ball { r: 1.8 },
                twine,
                Transform::from_xyz(-fw * 0.06, stand, 19.4),
            ));
        }
        // The big one. It hums a chord: a bright ring and core.
        Kind::VeryMysteriousCrate => {
            let crate_body = Vec3::new(fw * 0.88, fh * 0.88, 28.0);
            let stand = crate_body.y.mul_add(0.5, sole);
            out.push(Part::new(
                "crate",
                Body::Box(crate_body),
                body,
                Transform::from_xyz(0.0, stand, 14.0),
            ));
            let hum = Coat::phosphor(palette::EERIE_BRIGHT, 0.8);
            let r = fw * 0.26;
            out.push(
                Part::new(
                    "halo",
                    Body::Hoop {
                        inner: r - 1.6,
                        outer: r + 1.6,
                    },
                    hum,
                    Transform::from_xyz(0.0, stand, 28.6).with_rotation(flat),
                )
                .role(Role::Pulse {
                    color: palette::EERIE_BRIGHT,
                    base: 0.8,
                    amp: 1.6,
                    freq: 2.2,
                    phase: phase_of(piece.id, SALT_PULSE),
                }),
            );
            // The core sits as deep as it is round: a crate 28 units
            // thick already fills all but two of the band the kind
            // builders compose in, and a ball six units across cannot
            // stand proud of that face and stay inside it. Sunk to its
            // own radius, its crown is the band's own front — a boss
            // bulging through the halo, rather than an orb hanging past
            // the box the carry tell wraps.
            out.push(Part::new(
                "core",
                Body::Ball { r: fw * 0.09 },
                hum,
                Transform::from_xyz(0.0, stand, fw.mul_add(-0.09, RIG_FAR)),
            ));
        }
        // A shard chipped off the comet, one glint down its flank.
        // **A shard stands on its broken base.** Point-first at the eye
        // is the glyph's reading of a cone; on a deck it is a spike
        // lying down, and the flank the glint runs down is vertical.
        Kind::CometIce => {
            let shard = 28.0;
            out.push(Part::new(
                "shard",
                Body::Horn {
                    r: fw * 0.32,
                    h: shard,
                },
                body,
                Transform::from_xyz(0.0, sole + shard * 0.5, deep),
            ));
            out.push(Part::new(
                "glint",
                Body::Box(Vec3::new(1.6, 12.0, 1.6)),
                Coat::enamel(palette::GLINT),
                Transform::from_xyz(-4.5, fh.mul_add(0.33, sole), deep + 4.7),
            ));
        }
        // A bottle of the dark between stars, corked, one star inside.
        // **A corked bottle stands on its base.** Body, neck and cork
        // were three drums drawn end-on and stacked in DEPTH, which is
        // the glyph's concentric circles; on a deck that is a bottle
        // lying down with its cork pointing at whoever walks in. The
        // stack is the same stack, stood up, each length rooted a finger
        // in the one below it.
        Kind::BottledMidnight => {
            let (barrel, neck, cork) = (16.0_f32, 7.0_f32, 4.0_f32);
            let barrel_y = barrel.mul_add(0.5, sole);
            let neck_y = neck.mul_add(0.5, sole + barrel - 1.5);
            let cork_y = cork.mul_add(0.5, neck.mul_add(0.5, neck_y) - 1.0);
            out.push(Part::new(
                "bottle",
                Body::Drum {
                    r: fw * 0.24,
                    h: barrel,
                    facets: None,
                },
                body,
                Transform::from_xyz(0.0, barrel_y, deep),
            ));
            out.push(Part::new(
                "neck",
                Body::Drum {
                    r: fw * 0.1,
                    h: neck,
                    facets: None,
                },
                body,
                Transform::from_xyz(0.0, neck_y, deep),
            ));
            out.push(Part::new(
                "cork",
                Body::Drum {
                    r: fw * 0.13,
                    h: cork,
                    facets: None,
                },
                brass,
                Transform::from_xyz(0.0, cork_y, deep),
            ));
            let sx = (f32::from(piece.variant % 4) - 1.5) * 2.5;
            out.push(Part::new(
                "star",
                Body::Ball { r: 1.4 },
                Coat::phosphor(palette::GLINT, 4.0),
                Transform::from_xyz(sx, barrel_y, deep),
            ));
        }
        // Three overlapping cream spheres. It is looking at you.
        Kind::Fluff => {
            let r = fw * 0.28;
            // It sits on the deck on the widest of its three tufts;
            // the other two and the eyes are measured off that one.
            let sit = sole + r * 0.85;
            for (i, (coat, ball, at)) in [
                (shaded(0.08), r * 0.85, Vec3::new(-4.0, sit, 7.0)),
                (body, r * 0.75, Vec3::new(4.5, sit - 0.5, 6.5)),
                (body, r, Vec3::new(0.0, sit + 3.5, 9.5)),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(
                    Part::new(
                        "tuft",
                        Body::Ball { r: ball },
                        coat,
                        Transform::from_translation(at),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            for (i, ex) in [-2.6f32, 2.6].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "eye",
                        Body::Ball { r: 1.1 },
                        socket,
                        Transform::from_xyz(ex, sit + 7.0, 17.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
        }
        // Inner-ring transit papers: a flat card with the Guild's stripe.
        // The stripe stands proud of the card on every edge it has — a
        // hair deeper and a hair short of the card's own top and bottom.
        // Cut to the card's full height it shared both those planes with
        // it, which is two faces at one depth along the whole of a stripe
        // somebody is holding up to their eye.
        //
        // **A card lies down.** It is a card: five units thick and two
        // thirds of a cell tall, and the only way to settle a thing like
        // that onto a deck standing is to balance it on its edge, which
        // is a worse picture than the one it replaces. So the whole
        // composition takes a quarter turn onto its back — same card,
        // same stripe, same proudness, printed side up — and the card's
        // own face is the sole.
        Kind::TransitChit => {
            let laid = Quat::from_rotation_x(-FRAC_PI_2);
            out.push(Part::new(
                "card",
                Body::Box(Vec3::new(fw * 0.74, fh * 0.52, 5.0)),
                body,
                Transform::from_xyz(0.0, sole + 2.5, deep).with_rotation(laid),
            ));
            out.push(Part::new(
                "stripe",
                Body::Box(Vec3::new(fw * 0.12, fh * 0.46, 5.6)),
                Coat::enamel(palette::POI_GUILD),
                Transform::from_xyz(-fw * 0.2, sole + 3.0, deep).with_rotation(laid),
            ));
        }
        // One priceless chip: a low cylinder, a rim, an inner ring.
        // **A chip lies face up.** Nine units thick and three quarters
        // of a cell across, stood on a deck it is a coin balanced on its
        // edge; the same three bodies about an upright axle are a chip
        // on a table, which is where a chip is. The face's own claim
        // turns with it, so what says "this side looks at you" and what
        // draws it are still one reading.
        Kind::CasinoChip => {
            let r = fw * 0.36;
            let chip = 9.0;
            out.push(
                Part::new(
                    "chip face",
                    Body::Drum {
                        r,
                        h: chip,
                        facets: None,
                    },
                    body,
                    Transform::from_xyz(0.0, sole + chip * 0.5, deep),
                )
                .pointing(AXLE, Vec3::Y),
            );
            out.push(Part::new(
                "rim",
                Body::Hoop {
                    inner: r.mul_add(0.94, -1.3),
                    outer: r.mul_add(0.94, 1.3),
                },
                shaded(0.3),
                Transform::from_xyz(0.0, sole + chip, deep),
            ));
            out.push(Part::new(
                "inner ring",
                Body::Hoop {
                    inner: r.mul_add(0.52, -1.0),
                    outer: r.mul_add(0.52, 1.0),
                },
                Coat::enamel(palette::mix(palette::GLINT, color, 0.2)),
                Transform::from_xyz(0.0, sole + chip + 0.2, deep),
            ));
        }
        // A hanging shade off the gantry's top rail: mount plate, stem,
        // a flattened cone shade, and the warm bulb beneath — the bulb
        // and its point light wake through `sync_fixtures`.
        Kind::CeilingLamp => {
            // **The canopy meets the deckhead.** A pendant's plate is
            // the one part of it that touches the ship, and it used to
            // stop eight millimetres under the plane it is screwed to —
            // which is the same defect as a foot over a deck, upside
            // down, and just as invisible in a photograph. Its top face
            // is buried [`SOLE_BURY`] into the chart now, derived off
            // the plate's own girth rather than set by a decimal, so a
            // thicker plate stays screwed to the same ceiling.
            let cap = Body::Box(Vec3::new(9.0, 3.0, 5.0));
            out.push(Part::new(
                "mount plate",
                cap,
                plate,
                Transform::from_xyz(0.0, fh.mul_add(0.5, SOLE_BURY) - cap.half().y, 10.0),
            ));
            out.push(
                Part::new(
                    "stem",
                    Body::Drum {
                        r: 1.3,
                        h: fh * 0.26,
                        facets: None,
                    },
                    brass,
                    Transform::from_xyz(0.0, fh * 0.30, 10.0),
                )
                .seated("mount plate"),
            );
            let shade = Part::new(
                "shade",
                Body::Horn {
                    r: fw * 0.28,
                    h: 12.0,
                },
                body,
                Transform::from_xyz(0.0, fh * 0.04, 10.0),
            )
            .pointing(MOUTH, Vec3::NEG_Y)
            .seated("stem");
            let bulb = bulb_part(piece.kind, &shade, 3.4);
            out.push(shade);
            out.push(bulb);
        }
        // A sconce off a repossessed liner: bracket arm and mount pad
        // reaching for the nearer stile (the `WallArm` sub-root flips
        // sides with the piece's wall column), cup, bulb.
        //
        // **A sconce is bolted to a wall, so its pad begins at one.**
        // The whole fitting used to be composed at z = 10 — the middle
        // of the band a rig is composed in — which on a wall berth is a
        // hand's breadth of daylight between the pad and the plane it
        // is screwed to. Every other kind that hangs on a wall puts its
        // backmost body at z = 0 and this one did not, so the arm read
        // as a plank floating in the air. The pad owns the wall now and
        // the bracket runs inside its depth at both ends, which is the
        // same joint the cabinet's carcass makes: parts of one fitting
        // interpenetrate rather than share a face.
        Kind::WallLamp => {
            let pad = 6.0;
            out.push(
                Part::new(
                    "bracket",
                    Body::Box(Vec3::new(fw * 0.34, 3.0, 3.0)),
                    plate,
                    Transform::from_xyz(fw * 0.24, 0.0, pad - 2.0),
                )
                .under(Under::Arm)
                .seated("mount pad"),
            );
            out.push(
                Part::new(
                    "mount pad",
                    Body::Box(Vec3::new(3.4, 10.0, pad)),
                    plate,
                    Transform::from_xyz(fw * 0.42, 0.0, pad * 0.5),
                )
                .under(Under::Arm),
            );
            let cup = Part::new(
                "sconce cup",
                Body::Horn {
                    r: fw * 0.20,
                    h: 11.0,
                },
                body,
                Transform::from_xyz(fw * 0.10, 0.0, 10.0),
            )
            .under(Under::Arm)
            .pointing(MOUTH, Vec3::Z)
            .seated("bracket");
            let bulb = bulb_part(piece.kind, &cup, 3.2).under(Under::Arm);
            out.push(cup);
            out.push(bulb);
        }
        // A standing lamp bolted to the deck lip: base disc, pole, the
        // shade up top with its bulb tucked under.
        // **A floor lamp is one column, and it stands on one axle.** Its
        // plate, its pole, its shade and its bulb stood on three, a
        // relief's depth apart, because a rig's +Z began life as relief
        // height and the biggest feature was composed nearest the eye.
        // On a deck +Z is room depth, so that leant the lamp backwards —
        // and the base disc, the widest thing on the column and the
        // lowest, was the one composed furthest back: it reached a hand's
        // breadth behind the plane [`RIG_NEAR`] begins at, out of the box
        // the carry tell wraps a body in. The axle is where the shade and
        // the bulb already stood, so the disc that carries the pole is
        // under the pole, and the pole is under the shade.
        Kind::FloorLamp => {
            let axle = 11.0;
            // **The plate stands on the deck and the pole stands in the
            // plate.** Both ends of the pole are derived from what they
            // meet, so settling the plate onto the floor cannot leave
            // the column above it hanging.
            let disc = 3.2_f32;
            let plate_y = disc.mul_add(0.5, sole);
            let pole_top = fh * 0.32;
            out.push(
                Part::new(
                    "base plate",
                    Body::Drum {
                        r: fw * 0.26,
                        h: disc,
                        facets: None,
                    },
                    plate,
                    Transform::from_xyz(0.0, plate_y, axle),
                )
                .pointing(AXLE, Vec3::Y),
            );
            out.push(
                Part::new(
                    "pole",
                    Body::Drum {
                        r: 1.3,
                        h: pole_top - plate_y,
                        facets: None,
                    },
                    brass,
                    Transform::from_xyz(0.0, f32::midpoint(plate_y, pole_top), axle),
                )
                .seated("base plate"),
            );
            let shade = Part::new(
                "shade",
                Body::Horn {
                    r: fw * 0.30,
                    h: 13.0,
                },
                body,
                Transform::from_xyz(0.0, fh * 0.33, axle),
            )
            .pointing(MOUTH, Vec3::NEG_Y)
            .seated("pole");
            let bulb = bulb_part(piece.kind, &shade, 3.4);
            out.push(shade);
            out.push(bulb);
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
            // The backrest stands at the wall side; the seat deck runs
            // out into the room, cushions on top, arms full depth.
            out.push(Part::new(
                "backrest",
                Body::Box(Vec3::new(fw * 0.74, fh * 0.56, 5.0)),
                body,
                Transform::from_xyz(0.0, fh * 0.16, 2.5),
            ));
            let seat_y = -fh * 0.20;
            let seat_h = fh * 0.30;
            out.push(Part::new(
                "seat",
                Body::Box(Vec3::new(fw * 0.76, seat_h, 18.0)),
                shaded(0.3),
                Transform::from_xyz(0.0, seat_y, 10.0),
            ));
            for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "cushion",
                        Body::Ball { r: 6.0 },
                        body,
                        Transform::from_xyz(fw * 0.17 * side, -fh * 0.02, 10.0),
                    )
                    .scaled(Vec3::new(1.5, 0.7, 1.1))
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "arm",
                        Body::Box(Vec3::new(fw * 0.10, fh * 0.54, 16.0)),
                        body,
                        Transform::from_xyz(fw * 0.42 * side, -fh * 0.04, 8.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            // **The feet run from the sole up INTO the seat they
            // carry.** A sole flush with the deck shares a plane with
            // it, so the bottom is buried a hair; the top used to stop
            // nine millimetres short of the seat, which is a couch
            // standing on four stilts of air (`gauntlet`, `part-seated`).
            // Both ends are derived from the things they meet.
            let head = seat_h.mul_add(-0.5, seat_y) + GLAZE;
            for (i, (side, fz)) in [(-1.0f32, 3.0), (1.0, 3.0), (-1.0, 16.0), (1.0, 16.0)]
                .into_iter()
                .enumerate()
            {
                out.push(
                    Part::new(
                        "foot",
                        Body::Box(Vec3::new(4.0, head - sole, 4.0)),
                        plate,
                        Transform::from_xyz(fw * 0.36 * side, f32::midpoint(sole, head), fz),
                    )
                    .nth(u8::try_from(i).unwrap_or(0))
                    .seated("seat"),
                );
            }
        }
        // Gilt frame, subject debatable: a backing slab, raised frame
        // lips, and the canvas — one seeded artwork painted through the
        // shared rasterizer, emissive so low it reads as paint.
        Kind::Painting => {
            out.push(Part::new(
                "backing",
                Body::Box(Vec3::new(fw * 0.82, fh * 0.74, 5.0)),
                shaded(0.35),
                Transform::from_xyz(0.0, 0.0, 2.5),
            ));
            let lip_h = Body::Box(Vec3::new(fw * 0.78, 3.2, 4.0));
            let lip_v = Body::Box(Vec3::new(3.2, fh * 0.66, 3.2));
            for (i, (lip, at)) in [
                (lip_h, Vec3::new(0.0, fh * 0.315, 5.4)),
                (lip_h, Vec3::new(0.0, -fh * 0.315, 5.4)),
                (lip_v, Vec3::new(fw * 0.35, 0.0, 5.4)),
                (lip_v, Vec3::new(-fw * 0.35, 0.0, 5.4)),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(
                    Part::new("frame lip", lip, body, Transform::from_translation(at))
                        .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            out.push(
                Part::new(
                    "artwork",
                    Body::Pane,
                    Coat::enamel(palette::GLINT),
                    Transform::from_xyz(0.0, 0.0, 5.15),
                )
                .cut(Cut::Art)
                .scaled(Vec3::new(fw * 0.68, fh * 0.58, 1.0)),
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
            let rack = shaded(0.25);
            // Carcass: the back sheet alone owns the rear plane, and
            // every part meeting it starts INSIDE it — joints between
            // rig solids interpenetrate, never kiss, because two faces
            // sharing a plane shimmer (the twice-caught cabinet: first
            // its rear, then the plane its "fixed" parts abutted at).
            out.push(Part::new(
                "back sheet",
                Body::Box(Vec3::new(fw * 0.96, fh * 0.97, 2.0)),
                body,
                Transform::from_xyz(0.0, 0.0, 1.0),
            ));
            for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "side",
                        Body::Box(Vec3::new(2.6, fh * 0.94, deep - 1.0)),
                        body,
                        Transform::from_xyz(fw * 0.43 * sx, 0.0, f32::midpoint(deep, 1.0)),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            for (i, sy) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "cap",
                        Body::Box(Vec3::new(fw * 0.92, 2.6, deep - 1.8)),
                        body,
                        Transform::from_xyz(0.0, fh * 0.455 * sy, f32::midpoint(deep, 1.0)),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            // The rack: mid shelf and centre stile, a shade darker,
            // rooted inside the back sheet like everything else.
            out.push(Part::new(
                "shelf",
                Body::Box(Vec3::new(fw * 0.88, 2.2, deep * 0.85)),
                rack,
                Transform::from_xyz(0.0, 0.0, (deep * 0.85).mul_add(0.5, 1.5)),
            ));
            out.push(Part::new(
                "stile",
                Body::Box(Vec3::new(2.2, fh * 0.9, deep * 0.79)),
                rack,
                Transform::from_xyz(0.0, 0.0, (deep * 0.85).mul_add(0.5, 1.5)),
            ));
            // Brass fittings: a cornice over the opening, stubby feet.
            out.push(Part::new(
                "cornice",
                Body::Box(Vec3::new(fw * 0.96, 2.2, 2.2)),
                brass,
                Transform::from_xyz(0.0, fh * 0.475, deep),
            ));
            // The feet stand a step INTO the deck, like every other
            // sole (`SOLE_BURY`), and reach up inside the carcass sides
            // they are screwed to.
            let foot_y = sole + 1.7;
            for (i, (sx, fz)) in [
                (-1.0f32, 3.0),
                (1.0, 3.0),
                (-1.0, deep - 3.0),
                (1.0, deep - 3.0),
            ]
            .into_iter()
            .enumerate()
            {
                out.push(
                    Part::new(
                        "foot",
                        Body::Box(Vec3::new(3.0, 3.4, 3.0)),
                        brass,
                        Transform::from_xyz(fw * 0.36 * sx, foot_y, fz),
                    )
                    .nth(u8::try_from(i).unwrap_or(0))
                    .seated("side"),
                );
            }
            // The cubbies: dark interior backs, invite glows in front.
            for slot in 0..CABINET_SLOTS {
                let anchor = cubby_anchor(slot);
                out.push(
                    Part::new(
                        "cubby lining",
                        Body::Box(Vec3::new(fw * 0.36, fh * 0.4, 1.2)),
                        socket,
                        Transform::from_xyz(anchor.x, anchor.y, 2.2),
                    )
                    .nth(slot),
                );
                out.push(
                    Part::new(
                        "cubby mouth",
                        Body::Box(Vec3::new(fw * 0.33, fh * 0.37, 0.6)),
                        Coat::phosphor(palette::AMBER, 0.0),
                        Transform::from_xyz(anchor.x, anchor.y, 3.0),
                    )
                    .nth(slot)
                    .role(Role::Cubby { slot }),
                );
            }
        }
        // The whole window family, ONE construction: the porthole, the
        // transit window, and Saturn's bay pane are the same hole in the
        // hull at three sizes, so they are the same rig with three
        // bezels ([`window_parts`]). Adding a fourth size is an arm in
        // `bezel`, never a second copy of the glass.
        Kind::Window | Kind::Porthole | Kind::BayWindow => {
            out.extend(window_parts(piece, color, fw, fh, screens));
        }
        // The chart tank: the star map's phosphor aquarium, off the
        // wall at last. Dark glass in a brass chassis over a plinth,
        // the chart glowing on its own (vital instruments must read
        // lights-out), and the amber carry grab at its base — the
        // handle rule's move affordance (BAY.md, "The handle rule").
        Kind::ChartTank => {
            out.push(Part::new(
                "plinth",
                Body::Box(Vec3::new(fw * 0.92, fh * 0.90, 3.0)),
                plate,
                Transform::from_xyz(0.0, 0.0, 1.5),
            ));
            out.push(Part::new(
                "void slab",
                Body::Box(Vec3::new(fw * 0.86, fh * 0.82, 9.0)),
                shaded(0.82),
                Transform::from_xyz(0.0, 0.0, 6.0),
            ));
            // The chart itself: the CRT's painted map rides the tank's
            // glass, proud of the void slab so it actually shows. The
            // pane's size and depth come from the mount table, which is
            // also what the Map station's surface is derived from — the
            // picture and the pointer cannot land on different glass.
            let mount = instrument(piece.kind).expect("the tank mounts the map");
            let (gw, gh, gz) = (fw * mount.face.0, fh * mount.face.1, mount.plane);
            if screens.map {
                out.push(
                    Part::new(
                        "chart glass",
                        Body::Pane,
                        Coat::phosphor(color, 1.4),
                        Transform::from_xyz(0.0, 0.0, gz),
                    )
                    .cut(Cut::Map)
                    .scaled(Vec3::new(gw, gh, 1.0)),
                );
            } else {
                out.push(Part::new(
                    "chart glass",
                    Body::Box(Vec3::new(gw, gh, 1.6)),
                    Coat::phosphor(color, 1.4),
                    Transform::from_xyz(0.0, 0.0, gz),
                ));
            }
            for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "post",
                        Body::Box(Vec3::new(3.0, fh * 0.86, 11.0)),
                        brass,
                        Transform::from_xyz(sx * fw * 0.44, 0.0, 6.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            for (i, sy) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "cap",
                        Body::Box(Vec3::new(fw * 0.88, 3.0, 10.2)),
                        brass,
                        Transform::from_xyz(0.0, sy * fh * 0.43, 6.0),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            out.extend(grab_parts(piece.kind, fw, fh, 12.5));
        }
        // The ETA gauge: a brass drum with a dark dial, the phosphor
        // needle reading the live leg ([`eta_needles`] sweeps it), and
        // the scale it is read against. Passive — it earns no amber
        // handle.
        //
        // **A hand with no dial under it is not a reading.** The gauge
        // shipped as a bare face and a needle that wandered across it,
        // and the playtest asked the only question left to ask: when is
        // arrival? So the sweep is graduated — a pip at each quarter of
        // the leg — and its empty end carries a mark of its own, longer
        // and wider than a pip and reaching in far enough for the
        // needle to seat against it, which lights as the leg closes
        // ([`eta_needles`] again). Not a word and not a number: a
        // notch, a size, a colour and a motion, three of them still
        // there with the hue taken away.
        //
        // Every mark is turned by [`eta_bearing`], the same function
        // that turns the needle, so the scale cannot come to mean a
        // sweep the hand does not walk.
        Kind::EtaGauge => {
            out.push(Part::new(
                "drum",
                Body::Drum {
                    r: fw * 0.42,
                    h: 6.0,
                    facets: None,
                },
                brass,
                Transform::from_xyz(0.0, 0.0, 3.0).with_rotation(flat),
            ));
            out.push(Part::new(
                "dial",
                Body::Drum {
                    r: fw * 0.35,
                    h: 3.0,
                    facets: None,
                },
                shaded(0.7),
                Transform::from_xyz(0.0, 0.0, 5.5).with_rotation(flat),
            ));
            // The graduations, on a chapter ring outboard of where the
            // needle's tip reaches — a hand that draws over the scale
            // it is read against hides the reading at the moment the
            // reading is being taken.
            let ring = fw * ETA_RING;
            for (i, left) in ETA_PIPS.into_iter().enumerate() {
                out.push(
                    Part::new(
                        "pip",
                        Body::Box(Vec3::new(2.0, 2.2, 1.6)),
                        Coat::etched(palette::ICON),
                        Transform::from_translation(
                            eta_bearing(left) * Vec3::new(0.0, ring, 0.0) + Vec3::Z * 7.4,
                        )
                        .with_rotation(eta_bearing(left)),
                    )
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
            out.push(
                Part::new(
                    "arrival mark",
                    Body::Box(Vec3::new(3.6, 3.4, 1.6)),
                    Coat::phosphor(palette::AMBER, ARRIVAL_REST),
                    Transform::from_translation(
                        eta_bearing(0.0) * Vec3::new(0.0, ring, 0.0) + Vec3::Z * 7.4,
                    )
                    .with_rotation(eta_bearing(0.0)),
                )
                .role(Role::Arrival),
            );
            out.push(
                Part::new(
                    "needle",
                    Body::Box(Vec3::new(2.0, fh * 0.28, 1.6)),
                    Coat::phosphor(color, 1.6),
                    Transform::from_xyz(0.0, fh * 0.14, 7.2),
                )
                .role(Role::Needle { reach: fh * 0.14 }),
            );
            out.push(Part::new(
                "hub",
                Body::Ball { r: 1.8 },
                Coat::etched(palette::GLINT),
                Transform::from_xyz(0.0, 0.0, 7.4),
            ));
        }
        // The destination preview: a square brass porthole wearing the
        // CRT's painted preview — the selected world's face rides the
        // piece now, not the console. Passive glass; headless paths
        // show a lone phosphor disc instead.
        Kind::DestPreview => {
            // The glass is GLAZED into the bezel, so it sits on the
            // bezel's own face and not a finger's breadth in front of
            // it. The headless slab always overlapped the brass and the
            // played pane never touched it, so only the build people
            // look at showed the gap.
            let bezel_face = 4.0;
            out.push(Part::new(
                "bezel",
                Body::Box(Vec3::new(fw * 0.84, fh * 0.84, bezel_face)),
                brass,
                Transform::from_xyz(0.0, 0.0, bezel_face * 0.5),
            ));
            if screens.preview {
                out.push(
                    Part::new(
                        "preview glass",
                        Body::Pane,
                        shaded(0.72),
                        Transform::from_xyz(0.0, 0.0, bezel_face + GLAZE),
                    )
                    .cut(Cut::Preview)
                    .scaled(Vec3::new(fw * 0.68, fh * 0.68, 1.0))
                    .seated("bezel"),
                );
            } else {
                out.push(
                    Part::new(
                        "preview glass",
                        Body::Box(Vec3::new(fw * 0.68, fh * 0.68, 3.0)),
                        shaded(0.72),
                        Transform::from_xyz(0.0, 0.0, 3.5),
                    )
                    .seated("bezel"),
                );
                out.push(Part::new(
                    "world",
                    Body::Ball { r: fw * 0.14 },
                    Coat::phosphor(color, 1.5),
                    Transform::from_xyz(0.0, 0.0, 5.6),
                ));
            }
        }
        // The launch handle: a shade plate, the brass quadrant slot,
        // and the pull arm reaching into the room, its knob wearing the
        // go-lamp green (the FUNCTION — the console face's handle moved
        // in here whole) while the amber carry grab below is the MOVE
        // affordance, per the handle rule. Both glow enough that the
        // one lever that commits a course is findable in the dark.
        // The arm hangs off a [`LeverHandle`] pivot so [`lever_motion`]
        // can throw it with the gesture layer's own travel.
        Kind::LaunchLever => {
            let mount = instrument(piece.kind).expect("the lever mounts its panel");
            out.push(Part::new(
                "panel",
                Body::Box(Vec3::new(fw * mount.face.0, fh * mount.face.1, 3.0)),
                plate,
                Transform::from_xyz(0.0, 0.0, 1.5),
            ));
            out.push(Part::new(
                "quadrant slot",
                Body::Box(Vec3::new(fw * 0.14, fh * 0.72, 4.0)),
                brass,
                Transform::from_xyz(0.0, 0.0, 3.2),
            ));
            let pivot = Under::Pivot(
                Transform::from_xyz(0.0, -fh * 0.26, LEVER_PIVOT_Z)
                    .with_rotation(Quat::from_rotation_x(LEVER_REST)),
            );
            out.push(
                Part::new(
                    "pull arm",
                    Body::Box(Vec3::new(3.2, LEVER_ARM, 3.2)),
                    shaded(0.35),
                    Transform::from_xyz(0.0, LEVER_ARM * 0.5, 0.0),
                )
                .under(pivot),
            );
            // The halo sits behind the knob, a soft plate that wakes
            // only while a pull would actually depart.
            out.push(
                Part::new(
                    "halo",
                    Body::Drum {
                        r: 5.4,
                        h: 0.6,
                        facets: None,
                    },
                    Coat::phosphor(palette::LAMP_OK, 0.0),
                    Transform::from_xyz(0.0, LEVER_ARM, -1.2).with_rotation(flat),
                )
                .under(pivot)
                .role(Role::Halo),
            );
            out.push(
                Part::new(
                    "knob",
                    Body::Ball { r: 3.4 },
                    Coat::phosphor(palette::LAMP_OK, 0.9),
                    Transform::from_xyz(0.0, LEVER_ARM, 0.0),
                )
                .under(pivot)
                .role(Role::Knob),
            );
            out.extend(grab_parts(piece.kind, fw, fh, 6.0));
        }
        // The dressing kinds own two bodies each — laid into the room
        // versus rolled or canned for the counter — and
        // [`sync_dressings`] shows exactly one, by the sim's berth class.
        Kind::Rug => {
            let border = shaded(0.3);
            // [`RUG_THICK`] is a world measure; rigs build in sim units,
            // so the pile converts through the bay's cell scale.
            let pile = RUG_THICK / (crate::rig::BAY_CELL / layout::CELL);
            // The pile over a darker binding: the border reads woven at
            // a glance, and the fringe knots the short ends.
            out.push(
                Part::new(
                    "binding",
                    Body::Box(Vec3::new(fw * 0.98, fh * 0.96, pile * 0.7)),
                    border,
                    Transform::from_xyz(0.0, 0.0, pile * 0.35),
                )
                .under(Under::Laid),
            );
            out.push(
                Part::new(
                    "pile",
                    Body::Box(Vec3::new(fw * 0.90, fh * 0.86, pile)),
                    body,
                    Transform::from_xyz(0.0, 0.0, pile * 0.75),
                )
                .under(Under::Laid),
            );
            let mut nth = 0_u8;
            for sx in [-1.0f32, 1.0] {
                for i in 0..5_u8 {
                    out.push(
                        Part::new(
                            "tassel",
                            Body::Box(Vec3::new(2.4, 3.6, 0.5)),
                            border,
                            Transform::from_xyz(sx * fw * 0.465, (f32::from(i) - 2.0) * 6.0, 0.4),
                        )
                        .under(Under::Laid)
                        .nth(nth),
                    );
                    nth += 1;
                }
            }
            // Rolled for the counter: a tied bolt of weave, brass bands.
            // A rolled rug lies on the deck on the bands that tie it,
            // because the bands stand proud of the weave — so the axle
            // the whole roll turns about is a band's own radius up.
            let across = Quat::from_rotation_z(FRAC_PI_2);
            let axle = fh.mul_add(0.28, sole);
            out.push(
                Part::new(
                    "bolt",
                    Body::Drum {
                        r: fh * 0.26,
                        h: fw * 0.88,
                        facets: None,
                    },
                    body,
                    Transform::from_xyz(0.0, axle, fh * 0.26).with_rotation(across),
                )
                .under(Under::Packed),
            );
            for (i, sx) in [-1.0f32, 1.0].into_iter().enumerate() {
                out.push(
                    Part::new(
                        "band",
                        Body::Drum {
                            r: fh * 0.28,
                            h: 2.2,
                            facets: None,
                        },
                        brass,
                        Transform::from_xyz(sx * fw * 0.26, axle, fh * 0.26).with_rotation(across),
                    )
                    .under(Under::Packed)
                    .nth(u8::try_from(i).unwrap_or(0)),
                );
            }
        }
        Kind::PaintTin => {
            let coat = Coat::enamel(palette::enamel_color(piece.variant));
            // The coat: enamel a hair inside the cell so the berth edge
            // still reads, with one streak the painter didn't chase.
            out.push(
                Part::new(
                    "coat",
                    Body::Box(Vec3::new(fw * 0.84, fh * 0.84, 0.4)),
                    coat,
                    Transform::from_xyz(0.0, 0.0, 0.2),
                )
                .under(Under::Laid),
            );
            out.push(
                Part::new(
                    "streak",
                    Body::Box(Vec3::new(fw * 0.6, 2.6, 0.3)),
                    Coat::enamel(palette::mix(
                        palette::enamel_color(piece.variant),
                        palette::GLINT,
                        0.14,
                    )),
                    Transform::from_xyz(-1.5, 3.0, 0.45).with_rotation(Quat::from_rotation_z(0.16)),
                )
                .under(Under::Laid),
            );
            // Canned: a squat battered tin, the lid wearing its color.
            out.extend(
                tin_parts(body, coat, sole, deep)
                    .into_iter()
                    .map(|part| part.under(Under::Packed)),
            );
        }
        Kind::LuminousPaint => {
            let glow_hue = palette::mix(color, palette::PHOSPHOR, 0.35);
            let glass = Coat::phosphor(glow_hue, 0.0);
            // The coat's glass, plus the real tinge beneath it — both fed
            // by [`sync_dressings`] exactly as the lamps are fed.
            out.push(
                Part::new(
                    "coat",
                    Body::Box(Vec3::new(fw * 0.84, fh * 0.84, 0.4)),
                    glass,
                    Transform::from_xyz(0.0, 0.0, 0.2),
                )
                .under(Under::Laid),
            );
            out.push(
                Part::lamp(
                    "tinge",
                    glass,
                    Transform::from_xyz(0.0, 0.0, 6.0),
                    Role::Tinge,
                )
                .under(Under::Laid),
            );
            // Canned: the blackout tin — dark body, the lid's glass dark
            // until laid (it shares the coat's instance, and the level
            // stays floored while packed).
            out.extend(
                tin_parts(shaded(0.55), glass, sole, deep)
                    .into_iter()
                    .map(|part| part.under(Under::Packed)),
            );
        }
    }
    out
}

/// A lamp's live bulb: dark glass that wakes warm, with the real point
/// light under it. The light rests dark ([`Dimmable`] base 0) and
/// `sync_fixtures` eases both toward `lamp_lit` — no shadow maps, per
/// the art direction; the pool of light is the point.
///
/// **Seated in the mouth of the shade that shades it, and nowhere
/// else.** Where the bulb stands used to be a literal written beside the
/// shade, which meant the two agreed only for as long as nobody turned
/// the shade — and the sconce is what happens when somebody does. Its
/// cup was hand-turned to open along `-X`, its bulb was placed by hand
/// in front of that opening, and when [`Part::pointing`] began deriving
/// the turn from the claim the cup swung into the room and left the bulb
/// where the old turn had put it: a bare glowing ball hanging off the
/// end of a bracket, lighting nothing, with the cup it belonged in
/// pointing the other way. A shade is a cone, its mouth is the disc at
/// the low end of its own axis, and reading that mouth off the shade's
/// own transform is what makes the two move together for good.
fn bulb_part(kind: Kind, shade: &Part, radius: f32) -> Part {
    let color = palette::mix(palette::kind_color(kind), palette::GLINT, 0.35);
    let range = if kind == Kind::CeilingLamp {
        CEILING_RANGE
    } else {
        LAMP_RANGE
    };
    let Some(Body::Horn { h, .. }) = shade.body else {
        panic!("a lamp's bulb is seated in a shade, and a shade is a cone")
    };
    Part::new(
        "bulb",
        Body::Ball { r: radius },
        Coat::phosphor(color, 0.0),
        Transform::from_translation(shade.at.transform_point(Vec3::new(0.0, -h * 0.5, 0.0))),
    )
    .role(Role::Bulb { range })
}

// ------------------------------------------------------------ the stamping --

/// **Stamp one piece's rig into the world**: every part [`parts`]
/// describes, spawned in the frame it names, in the order it is
/// described.
///
/// Meshes and coats are cut once each per rig and handed round, which is
/// what the hand-written builders did by hoisting a shared handle above
/// a loop — the same drawing, one place to get it right. A part whose
/// role writes its own material is the exception and says so
/// ([`Role::alone`]).
fn build_kind(rig: &mut RigParts, piece: &Piece) {
    let screens = Screens {
        sky: rig.sky,
        map: rig.map_image.is_some(),
        preview: rig.preview_image.is_some(),
    };
    let home = rig.root;
    let mut bodies: Vec<(Body, Handle<Mesh>)> = Vec::new();
    let mut coats: Vec<(Coat, Handle<StandardMaterial>)> = Vec::new();
    let mut frames: Vec<(Under, Entity)> = Vec::new();
    for part in parts(piece, screens) {
        rig.root = frame_of(rig, piece, home, &mut frames, part.under);
        stamp(rig, piece, &part, &mut bodies, &mut coats);
    }
    rig.root = home;
}

/// The entity a part's frame hangs on, spawned the first time that frame
/// is asked for — so the sub-roots appear exactly where the description
/// first needs them.
fn frame_of(
    rig: &mut RigParts<'_, '_, '_>,
    piece: &Piece,
    home: Entity,
    frames: &mut Vec<(Under, Entity)>,
    under: Under,
) -> Entity {
    if matches!(under, Under::Rig) {
        return home;
    }
    if let Some((_, entity)) = frames.iter().find(|(seen, _)| seen.same(under)) {
        return *entity;
    }
    let entity = match under {
        Under::Rig => home,
        Under::Arm => rig
            .commands
            .spawn((
                under.rest(),
                Visibility::default(),
                WallArm { piece: piece.id },
                ChildOf(home),
            ))
            .id(),
        Under::Pivot(_) => rig
            .commands
            .spawn((
                under.rest(),
                Visibility::default(),
                ChildOf(home),
                LeverHandle,
            ))
            .id(),
        Under::Laid => dress_form(rig, piece, home, true),
        Under::Packed => dress_form(rig, piece, home, false),
    };
    frames.push((under, entity));
    entity
}

/// One described part, made real.
#[allow(clippy::too_many_lines)]
fn stamp(
    rig: &mut RigParts<'_, '_, '_>,
    piece: &Piece,
    part: &Part,
    bodies: &mut Vec<(Body, Handle<Mesh>)>,
    coats: &mut Vec<(Coat, Handle<StandardMaterial>)>,
) {
    let material = match part.cut {
        Cut::Coat(coat) => {
            if let Some((_, handle)) = coats.iter().find(|(seen, _)| *seen == coat)
                && !part.role.alone()
            {
                handle.clone()
            } else {
                let handle = coat.material(rig.materials, Some(rig.skin));
                if !part.role.alone() {
                    coats.push((coat, handle.clone()));
                }
                handle
            }
        }
        Cut::Map => rig
            .map_image
            .clone()
            .map(|image| crate::crt::tube_glass(rig.materials, &image))
            .expect("a mapped screen has its picture"),
        Cut::Preview => rig
            .preview_image
            .clone()
            .map(|image| crate::crt::tube_glass(rig.materials, &image))
            .expect("a mapped screen has its picture"),
        Cut::Sky => crate::viewport::pane_glass(rig.materials),
        Cut::Art => paint_artwork(rig.images, rig.materials, piece.id),
    };
    let Some(body) = part.body else {
        // A light and no body at all: the luminous coat's own tinge.
        if let Cut::Coat(coat) = part.cut {
            rig.commands.spawn((
                PointLight {
                    color: coat.color,
                    intensity: 0.0,
                    range: COAT_RANGE,
                    shadow_maps_enabled: false,
                    ..default()
                },
                part.at,
                Dimmable { intensity: 0.0 },
                CoatGlow {
                    piece: piece.id,
                    color: coat.color,
                    mat: material,
                    level: 0.0,
                },
                ChildOf(rig.root),
            ));
        }
        return;
    };
    let mesh = if let Some((_, handle)) = bodies.iter().find(|(seen, _)| *seen == body) {
        handle.clone()
    } else {
        let handle = rig.meshes.add(body.mesh());
        bodies.push((body, handle.clone()));
        handle
    };
    let entity = rig
        .commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            part.at,
            ChildOf(rig.root),
        ))
        .id();
    if part.role.dark() {
        rig.commands.entity(entity).insert(Visibility::Hidden);
    }
    if matches!(part.cut, Cut::Sky) {
        rig.commands.entity(entity).insert(crate::viewport::SkyPane);
    }
    match part.role {
        Role::Plain | Role::Tinge => {}
        Role::Bud => {
            rig.commands
                .entity(entity)
                .insert(Blossom { piece: piece.id });
        }
        Role::Pulse {
            color,
            base,
            amp,
            freq,
            phase,
        } => {
            rig.commands.entity(entity).insert(Pulse {
                color,
                base,
                amp,
                freq,
                phase,
            });
        }
        Role::Cubby { slot } => {
            rig.commands.entity(entity).insert(CubbyGlow {
                piece: piece.id,
                slot,
                phase: f32::from(slot) * 1.3,
            });
        }
        Role::Needle { reach } => {
            rig.commands.entity(entity).insert(EtaNeedle { reach });
        }
        Role::Arrival => {
            rig.commands.entity(entity).insert(EtaArrival);
        }
        Role::Knob => {
            rig.commands.entity(entity).insert(LeverLamp);
        }
        Role::Halo => {
            rig.commands.entity(entity).insert(LeverHalo);
        }
        Role::Grab => rig.grab = Some(material),
        Role::Bulb { range } => {
            let color = match part.cut {
                Cut::Coat(coat) => coat.color,
                _ => palette::GLINT,
            };
            rig.commands.spawn((
                PointLight {
                    color,
                    intensity: 0.0,
                    range,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(part.at.translation),
                Dimmable { intensity: 0.0 },
                LampGlow {
                    piece: piece.id,
                    color,
                    mat: material,
                    level: 0.0,
                },
                ChildOf(rig.root),
            ));
        }
    }
}

// The rat's mark, and everything else that is not a kind's own body,
// stays with `spawn_rig`: it belongs to every rig alike.

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

    /// Whether `sub` lies wholly inside `whole` — the claim a pick face
    /// makes about the cells it may answer for.
    fn within(sub: Rect, whole: Rect) -> bool {
        const SLACK: f32 = 1e-3;
        sub.x >= whole.x - SLACK
            && sub.y >= whole.y - SLACK
            && sub.x + sub.w <= whole.x + whole.w + SLACK
            && sub.y + sub.h <= whole.y + whole.h + SLACK
    }

    fn rect_of(x: u8, y: u8, kind: Kind) -> Rect {
        let (w, h) = cargo::plan(space_trucking::sim::room::RoomKind::Cabin, kind, x, y)
            .expect("the tests berth on real cells");
        let anchor = layout::cell_rect(CABIN, x, y);
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
        let (pos, rot, _) = site_on(
            Station::BayWall,
            &aft,
            &aft,
            Kind::Painting,
            rect_of(4, 1, Kind::Painting),
        );
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
        let (pos, rot, scale) = site_on(Station::BayFloor, &floor, &aft, Kind::Couch, couch);
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
        let (pos, rot, _) = site_on(
            Station::BayPort,
            &port,
            &aft,
            Kind::WallLamp,
            rect_of(1, 4, Kind::WallLamp),
        );
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
            let (_, rot, _) = site_on(Station::BayFloor, &floor, &aft, kind, rect_of(x, y, kind));
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
            facing(3, 9, Kind::Couch).z > 0.9,
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
        // **And a deckhead takes the same rule.** It used to take one
        // fixed turn from every cell of every ceiling in the game, so a
        // pendant hung on the front row looked into the front wall a
        // hand's breadth in front of it — the couch-facing-the-wall
        // defect stood on its head, and invisible to every family in the
        // gauntlet until one of them learned to ask which way a berth
        // turns a body.
        let ceiling = chart(Station::BayCeiling);
        let hung = |x: u8, y: u8, kind: Kind| {
            let (_, rot, _) = site_on(
                Station::BayCeiling,
                &ceiling,
                &aft,
                kind,
                rect_of(x, y, kind),
            );
            assert!(
                (rot * Vec3::Y - Vec3::Y).length() < 1e-4,
                "a pendant hangs the author's up world up"
            );
            rot * Vec3::Z
        };
        assert!(
            hung(16, 6, Kind::CeilingLamp).z < -0.9,
            "mid-ceiling faces front, exactly as the deck under it does"
        );
        assert!(
            hung(16, 9, Kind::CeilingLamp).z > 0.9,
            "a pendant on the front row turns its back to the front wall"
        );
    }

    /// **A berth owns the ground its body stands on, and not a cell
    /// more.**
    ///
    /// The deck apron, said once. A kind used to state one pair of
    /// numbers for all three axes, and the second of them was an
    /// ELEVATION: on a wall it meant courses, and the deck read the same
    /// number as depth. So a 1×2 wardrobe claimed 1.06 m of deck for a
    /// body that reaches 0.53 m into the room, and the half-metre of
    /// bare deck in front of it answered for the wardrobe — the sim
    /// says "which piece is at this point", and the point was inside
    /// its rect. Aiming at that deck picked the wardrobe up; aiming at
    /// it while carrying read a berth two cells deep.
    ///
    /// The claim is two-sided on purpose. A body smaller than its cells
    /// is the apron; a body bigger is the overhang `face-fits` catches
    /// from the other side. Both are the same equality, and the number
    /// it holds to is [`BAY_FIT`] — the one margin a rig wears, on all
    /// three axes now.
    #[test]
    fn a_berth_owns_the_ground_its_body_stands_on() {
        let ship = Rooms::new();
        let charts = rig::bay();
        let (cols, rows) = space_trucking::sim::RoomKind::Cabin.grid();
        let mut swept = 0_u32;
        let mut floors = 0_u32;
        for kind in Kind::ALL {
            if kind.covering() {
                continue;
            }
            for y in 0..rows {
                for x in 0..cols {
                    if placement_check(&ship, &[], 0, kind, CABIN, x, y).is_err() {
                        continue;
                    }
                    let rect = rect_of(x, y, kind);
                    let (station, surface) =
                        chart_at(&charts, rect_center(rect)).expect("a legal berth is charted");
                    let (lo, hi) = berth_box(&charts, kind, rect).expect("and so is its box");
                    let body = hi - lo;
                    for (axis, cells, scale, along) in [
                        (surface.half_u.normalize(), rect.w, surface.scale_u(), "u"),
                        (surface.half_v.normalize(), rect.h, surface.scale_v(), "v"),
                    ] {
                        let drawn = body.dot(axis.abs());
                        let owned = cells * scale;
                        assert!(
                            (drawn / owned - BAY_FIT).abs() < 1e-3,
                            "{kind:?} at ({x}, {y}) on {station:?} draws {drawn} m along \
                             {along} where its cells own {owned} m",
                        );
                    }
                    swept += 1;
                    floors += u32::from(station == Station::BayFloor);
                }
            }
        }
        assert!(swept > 500, "the sweep went thin: {swept} berths");
        assert!(floors > 0, "no deck berth was measured at all");
    }

    /// **The deck in front of a wardrobe is deck.**
    ///
    /// The apron again, this time as the player meets it: a crosshair
    /// aimed at bare deck a step in front of a standing piece used to
    /// read that piece, and reading a cabinet reaches into its cubbies —
    /// so a click on the floor came back holding a transit chit. Driven
    /// through [`crate::surface::pick`] from a standing eye, because
    /// that is the path the aim actually takes.
    #[test]
    fn the_deck_in_front_of_a_standing_piece_is_deck() {
        use crate::room::InRoom;
        use crate::surface::{Aimable, pick};

        let sim = space_trucking::sim::Sim::from_save(crate::fixture::SAVE)
            .expect("the fixture board reads");
        let (rooms, pieces) = (sim.rooms(), sim.pieces());
        let charts = rig::bay();
        let aims: Vec<Aimable> = charts
            .iter()
            .map(|(station, surface)| Aimable {
                station: *station,
                surface: *surface,
                riding: false,
                in_room: Some(InRoom {
                    room: CABIN,
                    kind: space_trucking::sim::RoomKind::Cabin,
                }),
            })
            .chain(pieces.iter().filter_map(|piece| {
                let rect = layout::piece_rect(rooms, pieces, piece);
                standing_surface(&charts, piece.kind, rect).map(|surface| Aimable {
                    station: Station::Standing,
                    surface,
                    riding: true,
                    in_room: None,
                })
            }))
            .collect();
        // The starter fixture stands a cabinet at (6, 4) of the cabin's
        // net: one cell of deck, two courses tall. The cell in front of
        // it — one row toward the front wall — is bare deck.
        let cabinet = pieces
            .iter()
            .find(|piece| piece.kind == Kind::Cabinet)
            .expect("the fixture ships a wardrobe");
        let Loc::Hold { x, y, .. } = cabinet.loc else {
            panic!("it stands on the deck");
        };
        let floor = chart(Station::BayFloor);
        let eye = Vec3::new(0.0, crate::rig::EYE_HEIGHT, 0.0);
        let read_at = |cell: (u8, u8)| {
            let at = rect_center(layout::cell_rect(CABIN, cell.0, cell.1));
            let target = floor.to_world(at);
            let dir = Dir3::new(target - eye).expect("the eye is not on the deck");
            let hit = pick(
                Ray3d::new(eye, dir),
                true,
                f32::INFINITY,
                aims.iter().copied(),
            );
            layout::piece_at(rooms, pieces, hit.sim).copied()
        };
        // Its own cell answers with the wardrobe, or with whatever is
        // shelved inside it — a cubby is a berth of the cabinet's.
        let on_it = read_at((x, y)).expect("the wardrobe answers for its own cell");
        assert!(
            on_it.id == cabinet.id
                || matches!(on_it.loc, Loc::Stow { cabinet: host, .. } if host == cabinet.id),
            "aiming at the wardrobe read {on_it:?}",
        );
        // The next cell toward the front wall is bare deck, and bare
        // deck answers for nobody.
        assert_eq!(
            read_at((x, y + 1)).map(|piece| piece.id),
            None,
            "the deck a step in front of the wardrobe answered for something",
        );
    }

    /// **The upright rule, and it applies to everything now.** Wall
    /// cargo reads up-is-up on every wall — the side charts' vertical
    /// columns must not turn the star chart sideways — while facing
    /// stays into the room.
    ///
    /// The 2×1 painting is the case that used to be the exception: its
    /// footprint could not afford a quarter turn, so it hung sideways
    /// down a flank, cells and body together. Its cells are stated in
    /// the wall's own frame now, so they are already the rolled ones and
    /// the roll costs nothing.
    #[test]
    fn wall_cargo_hangs_upright() {
        let aft = chart(Station::BayWall);
        for (station, x, y, kind) in [
            (Station::BayWall, 4, 0, Kind::ChartTank),
            (Station::BayPort, 0, 4, Kind::ChartTank),
            (Station::BayStarboard, 11, 5, Kind::ChartTank),
            (Station::BayFront, 4, 10, Kind::ChartTank),
            (Station::BayWall, 4, 1, Kind::Painting),
            (Station::BayPort, 0, 4, Kind::Painting),
            (Station::BayStarboard, 11, 5, Kind::Painting),
        ] {
            let surface = chart(station);
            let (_, rot, _) = site_on(station, &surface, &aft, kind, rect_of(x, y, kind));
            assert!(
                (rot * Vec3::Y).y > 0.9,
                "{station:?}: the {kind:?}'s up must be world up, got {:?}",
                rot * Vec3::Y
            );
            let inward = station.inward(&surface);
            assert!(
                (rot * Vec3::Z).dot(inward) > 0.9,
                "{station:?}: the {kind:?} must still face into the room"
            );
        }
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

    /// The standing rule: a rig that stands OFF its chart is picked on
    /// its own face, so what the aim lands on is what the player is
    /// looking at. The hard case is the cabinet the backing rule turns —
    /// berthed against the port seam it yaws to face starboard, and the
    /// deck chart behind it can only answer about a plate two steps
    /// away, skewed and mirrored (the playtest's top-right cubby
    /// selecting the top-left one).
    #[test]
    fn a_yawed_cabinets_cubbies_are_picked_on_its_face() {
        let charts = rig::bay();
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        // One column wide against the port seam: the backing rule spends
        // its quarter turn and the cabinet faces starboard.
        let rect = rect_of(3, 4, Kind::Cabinet);
        let (pos, rot, scale) = site_on(Station::BayFloor, &floor, &aft, Kind::Cabinet, rect);
        assert!(
            (rot * Vec3::Z).x > 0.9,
            "the backing rule must turn this cabinet to starboard: {:?}",
            rot * Vec3::Z
        );
        let face =
            standing_surface(&charts, Kind::Cabinet, rect).expect("a standing rig carries a face");
        assert!(
            within(face.rect, rect),
            "the face binds the silhouette's own cells, {:?} inside {rect:?}",
            face.rect
        );
        assert!(
            face.normal().dot(rot * Vec3::Z) > 0.99,
            "the face must look the way the rig looks"
        );
        // Every cubby the rig DRAWS is the cubby the sim reads there.
        for slot in 0..CABINET_SLOTS {
            let drawn = pos + rot * (cubby_anchor(slot) * scale);
            let n = face.normal();
            let ray = Ray3d::new(drawn + n * 0.6, Dir3::new(-n).expect("a unit normal"));
            let (_, sim, _) = face.project(ray).expect("the face takes the aim");
            assert!(
                layout::cubby_rect(rect, slot).contains(sim),
                "cubby {slot} is drawn where the sim reads {sim:?}"
            );
        }
        // And the report itself, put to the test: standing in front of
        // the cabinet and aiming at the TOP-RIGHT of what is on show
        // selects the top-right cubby, slot 1 — not its mirror.
        let n = face.normal();
        let eye = face.center + n * 0.9 + Vec3::Y * 0.25;
        let right = (-n).cross(Vec3::Y).normalize();
        let target = face.center
            + right * (face.half_u.length() * 0.5)
            + Vec3::Y * (face.half_v.length() * 0.5);
        let ray = Ray3d::new(eye, Dir3::new(target - eye).expect("a look direction"));
        let (_, sim, _) = face.project(ray).expect("the aim meets the face");
        assert!(
            layout::cubby_rect(rect, 1).contains(sim),
            "the top-right quadrant read {sim:?}"
        );
        // The defect it retires: that same ray, read through the flat
        // deck chart, cannot name the cubby the player is looking at.
        let flat = floor.project(ray).map(|(_, sim, _)| sim);
        assert!(
            flat.is_none_or(|sim| !layout::cubby_rect(rect, 1).contains(sim)),
            "the deck chart must not be able to answer for a standing rig: {flat:?}"
        );
    }

    /// Which berths carry a face: the ones whose rig leaves its chart's
    /// lie. Standing cargo does it bodily — a pendant hangs clear of
    /// the ceiling exactly as floor cargo stands clear of the deck —
    /// and rolled wall cargo does it by the turn the upright rule
    /// spends. What is left flat and level on its chart needs none: the
    /// chart is already the piece, and a second surface would only
    /// fight it.
    #[test]
    fn a_face_hangs_wherever_the_rig_leaves_its_chart() {
        let charts = rig::bay();
        // A pendant hangs clear of the ceiling slab; a square sconce and
        // the tank spend the side charts' quarter turn; and the window,
        // 2×1, spends the front chart's half turn — whose rows climb the
        // wall, so every footprint there can afford the roll.
        for (x, y, kind) in [
            (18, 6, Kind::CeilingLamp),
            (0, 4, Kind::WallLamp),
            (12, 5, Kind::ChartTank),
            (4, 12, Kind::Window),
            // The painting down a flank: a berth the athwart rule used
            // to refuse, upright now and carrying its own face for it.
            (0, 4, Kind::Painting),
        ] {
            let rect = rect_of(x, y, kind);
            let face = standing_surface(&charts, kind, rect)
                .unwrap_or_else(|| panic!("{kind:?} at ({x}, {y}) leaves its chart's lie"));
            assert!(
                within(face.rect, rect),
                "{kind:?}: a face binds a sub-rect of its own cells, got {:?}",
                face.rect
            );
        }
        // The aft chart already stands level, so nothing hung there is
        // turned at all and nothing hung there needs a face of its own.
        for (x, y, kind) in [(4, 1, Kind::Painting), (4, 1, Kind::ChartTank)] {
            assert!(
                standing_surface(&charts, kind, rect_of(x, y, kind)).is_none(),
                "{kind:?} at ({x}, {y}) lies with its chart and needs no face"
            );
        }
    }

    /// A rolled wall piece's face stands PROUD of the wall it hangs on:
    /// coplanar quads are settled by query order, which is not an
    /// answer, so the standoff is what makes the face outrank the chart
    /// for a crosshair coming out of the room.
    #[test]
    fn a_wall_face_outranks_the_chart_it_hangs_on() {
        let charts = rig::bay();
        let starboard = chart(Station::BayStarboard);
        let rect = rect_of(12, 5, Kind::ChartTank);
        let face = standing_surface(&charts, Kind::ChartTank, rect).expect("the tank rolls");
        let inward = Station::BayStarboard.inward(&starboard);
        let off = (face.center - starboard.to_world(rect_center(rect))).dot(inward);
        assert!(off > 0.02, "the face sits in the wall: {off}");
        assert!(
            face.normal().dot(inward) > 0.99,
            "the face must look into the room"
        );
        // A ray in from the room meets the face first, every time.
        let eye = face.center + inward * 1.2 + Vec3::Y * 0.2;
        let ray = Ray3d::new(eye, Dir3::new(face.center - eye).expect("a look direction"));
        let near = face.project(ray).expect("the face takes the aim").0;
        let far = starboard.project(ray).expect("the chart is behind it").0;
        assert!(near < far, "the chart answered first: {near} vs {far}");
    }

    /// Selection follows the drawing, end to end: with all four cubbies
    /// of a yawed cabinet full, aiming at a boxed mini picks THAT mini
    /// out of the sim's own hit test — the law the fixture sweep set,
    /// now that a standing rig maps its own body.
    #[test]
    fn stowed_cargo_is_grabbed_where_it_is_drawn() {
        let charts = rig::bay();
        let aft = chart(Station::BayWall);
        let floor = chart(Station::BayFloor);
        let mut pieces = vec![Piece {
            id: 1,
            kind: Kind::Cabinet,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x: 3,
                y: 4,
            },
        }];
        // Three cubbies boxed, one left bare: the sim's rack tiles the
        // whole body, so an empty mouth is the only place the furniture
        // itself answers — which is exactly where it should.
        let bare = 2;
        pieces.extend(
            (0..CABINET_SLOTS)
                .filter(|slot| *slot != bare)
                .map(|slot| Piece {
                    id: 10 + u32::from(slot),
                    kind: Kind::PerfumeVial,
                    variant: 0,
                    gnawed: false,
                    loc: Loc::Stow { cabinet: 1, slot },
                }),
        );
        let rooms = space_trucking::sim::Sim::new(1).rooms().clone();
        let rect = layout::piece_rect(&rooms, &pieces, &pieces[0]);
        let (pos, rot, scale) = site_on(Station::BayFloor, &floor, &aft, Kind::Cabinet, rect);
        let face = standing_surface(&charts, Kind::Cabinet, rect).expect("the cabinet stands");
        for slot in 0..CABINET_SLOTS {
            let drawn = pos + rot * (cubby_anchor(slot) * scale);
            let n = face.normal();
            let ray = Ray3d::new(drawn + n * 0.6, Dir3::new(-n).expect("a unit normal"));
            let (_, sim, _) = face.project(ray).expect("the face takes the aim");
            let want = if slot == bare {
                1
            } else {
                10 + u32::from(slot)
            };
            assert_eq!(
                layout::piece_at(&rooms, &pieces, sim).map(|piece| piece.id),
                Some(want),
                "aiming at cubby {slot} must grab piece {want}"
            );
        }
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
            Kind::Cabinet,
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
            Violation::Vital,
        ] {
            let bars = glyph_spec(Some(rule), rect);
            assert!(
                bars.len() <= usize::from(GLYPH_BARS),
                "{rule:?} overflows the pool"
            );
            let frame_only = matches!(
                rule,
                Violation::Bounds | Violation::Overlap | Violation::Suspicious | Violation::Vital
            );
            assert_eq!(bars.is_empty(), frame_only, "{rule:?}");
        }
        assert!(glyph_spec(None, rect).is_empty());
    }

    /// The instrument mount, mechanised: the chart tank's station lands
    /// on the tank's OWN glass — the pane the rig draws at the mount's
    /// plane — facing into the room, and a ray fired down that normal
    /// reads `MAP_PANEL` coordinates back. Derived from the starter
    /// berth through the same call the runtime hangs it with.
    #[test]
    fn the_map_rides_the_tanks_glass() {
        let charts = rig::bay();
        let rect = rect_of(12, 5, Kind::ChartTank);
        let (station, surface) =
            instrument_surface(&charts, Kind::ChartTank, rect).expect("the tank mounts the map");
        assert_eq!(station, Station::Map);
        assert_eq!(surface.rect, layout::MAP_PANEL);
        // The berth the rig itself takes, and the glass depth it draws
        // at: the surface must sit exactly there, not on the wall.
        let starboard = chart(Station::BayStarboard);
        let aft = chart(Station::BayWall);
        let (pos, rot, scale) = site_on(
            Station::BayStarboard,
            &starboard,
            &aft,
            Kind::ChartTank,
            rect,
        );
        let mount = instrument(Kind::ChartTank).expect("mounted");
        let want = pos + rot * (Vec3::Z * (mount.plane * scale.z));
        assert!(
            (surface.center - want).length() < 1e-5,
            "the map sits at {} instead of the tank's glass at {want}",
            surface.center
        );
        let inward = Station::BayStarboard.inward(&starboard);
        let off = (surface.center - pos).dot(inward);
        assert!(
            off > 0.05,
            "the glass must stand proud of the wall, not in it: {off}"
        );
        assert!(
            surface.normal().dot(inward) > 0.99,
            "the chart must read into the room"
        );
        // And the mapping round-trips: aim at the middle of the glass,
        // land in the middle of the map.
        let n = surface.normal();
        let ray = Ray3d::new(
            surface.center + n * 0.4,
            Dir3::new(-n).expect("a unit normal"),
        );
        let (_, sim, _) = surface.project(ray).expect("the ray meets the glass");
        let middle = rect_center(layout::MAP_PANEL);
        assert!(
            (sim.x - middle.x).abs() < 1.0 && (sim.y - middle.y).abs() < 1.0,
            "the glass centre reads {sim:?}, not {middle:?}"
        );
        // The launch handle hangs the same way, on its own front-wall
        // berth, bound to the rect the gesture layer still watches.
        let (station, lever) = instrument_surface(
            &charts,
            Kind::LaunchLever,
            rect_of(5, 10, Kind::LaunchLever),
        )
        .expect("the handle mounts its panel");
        assert_eq!(station, Station::Lever);
        assert!(
            lever.rect.contains(rect_center(layout::LAUNCH_LEVER))
                && lever.rect.w > layout::LAUNCH_LEVER.w,
            "the lever panel must contain the lever rect with room to pull"
        );
    }

    /// **The ETA dial marks the end the needle arrives at.**
    ///
    /// The gauge shipped as a bare face with a hand wandering over it,
    /// and the playtest asked the question a bare face leaves open:
    /// which end is arrival? So this is the player's question and not
    /// the drawing's — four things have to hold for a scale to answer
    /// it, and each of them was breakable by a retune of a number in
    /// another paragraph:
    ///
    /// 1. **The hand never draws over the scale.** Every mark stands
    ///    outboard of the radius the needle's tip reaches, which settles
    ///    it for every reading at once, because the hand is radial.
    /// 2. **Every mark is on the instrument** — inside the bezel's own
    ///    rim, since a mark past the rim is a mark on the wall.
    /// 3. **The arrival mark is where the hand ends up.** Its bearing is
    ///    the one [`eta_needles`] turns the needle to with none of the
    ///    leg left, and no graduation shares that bearing.
    /// 4. **It is told from a graduation without hue**: bigger across
    ///    and bigger along, so the end of the scale reads as the end in
    ///    a picture with the colour taken out.
    #[test]
    fn the_eta_dial_marks_the_end_the_needle_arrives_at() {
        let rig = rig_of(Kind::EtaGauge, Screens::LIVE);
        let solid = |what: &str| {
            rig.iter()
                .filter(|part| part.what == what)
                .map(|part| match part.body {
                    Some(Body::Box(size)) => (part, size),
                    other => panic!("{what} is cut from {other:?}, not a bar"),
                })
                .collect::<Vec<_>>()
        };
        let (needle, hand) = solid("needle")[0];
        let Role::Needle { reach } = needle.role else {
            panic!("the needle must carry the role the sweep drives it by")
        };
        // How far the tip gets, and how far the face goes.
        let tip = hand.y.mul_add(0.5, reach);
        let rim = rig
            .iter()
            .find(|part| part.what == "drum")
            .and_then(|part| match part.body {
                Some(Body::Drum { r, .. }) => Some(r),
                _ => None,
            })
            .expect("the gauge wears a bezel");
        let pips = solid("pip");
        let arrival = solid("arrival mark");
        assert_eq!(pips.len(), ETA_PIPS.len(), "one pip per graduation");
        assert_eq!(arrival.len(), 1, "one end is the end");
        for (mark, size) in pips.iter().chain(&arrival) {
            let (at, along) = (mark.at.translation.truncate().length(), size.y * 0.5);
            assert!(
                at - along > tip,
                "{} reaches in to {:.2}, inside the {tip:.2} the hand sweeps: \
                 the scale is hidden by the reading",
                mark.label(),
                at - along
            );
            assert!(
                at + along <= rim,
                "{} reaches out to {:.2}, past the {rim:.2} bezel: it is a mark \
                 on the wall, not on the gauge",
                mark.label(),
                at + along
            );
        }
        // Where the hand comes to rest with the leg run out.
        let done = (eta_bearing(0.0) * Vec3::Y).truncate();
        let bearing = |part: &Part| part.at.translation.truncate().normalize_or_zero().dot(done);
        assert!(
            bearing(arrival[0].0) > 0.9999,
            "the arrival mark stands off the bearing the needle ends on"
        );
        for (pip, _) in &pips {
            assert!(
                bearing(pip) < 0.999,
                "{} sits on top of arrival: a graduation there says the \
                 sweep has two ends",
                pip.label()
            );
        }
        let (_, big) = arrival[0];
        for (pip, size) in &pips {
            assert!(
                big.x > size.x && big.y > size.y,
                "{} is as large as the arrival mark, so the two ends of the \
                 sweep are told apart by hue alone",
                pip.label()
            );
        }
    }

    /// The handle rule's click routing, decided (BAY.md): amber handle
    /// means carry, the rest of a click-functional piece means focus,
    /// and passive cargo is all grab — plus the answer stops applying
    /// the moment the instrument leaves its wall.
    #[test]
    fn the_handle_decides_carry_or_focus() {
        let sim = space_trucking::sim::Sim::new(1);
        let (rooms, pieces) = (sim.rooms(), sim.pieces());
        let of_kind = |kind: Kind| {
            pieces
                .iter()
                .find(|piece| piece.kind == kind)
                .expect("the starter board hangs it")
        };
        for (kind, focus) in [
            (Kind::ChartTank, crate::rig::Focus::Tank),
            (Kind::LaunchLever, crate::rig::Focus::Lever),
        ] {
            let piece = of_kind(kind);
            let rect = layout::piece_rect(rooms, pieces, piece);
            let handle = carry_handle_rect(kind, rect).expect("click-functional cargo wears one");
            assert_eq!(
                crate::rig::handle_route(rooms, pieces, rect_center(handle)),
                None,
                "{kind:?}: the grab must reach the sim untouched"
            );
            // A hair above the handle band is still the instrument.
            let body = SimVec2::new(rect_center(rect).x, rect.h.mul_add(0.25, rect.y));
            assert!(!handle.contains(body));
            assert_eq!(
                crate::rig::handle_route(rooms, pieces, body),
                Some(focus),
                "{kind:?}: the body must answer with its station"
            );
        }
        // Passive cargo has no function to guard: every point grabs.
        let lamp = of_kind(Kind::CeilingLamp);
        let at = rect_center(layout::piece_rect(rooms, pieces, lamp));
        assert_eq!(crate::rig::handle_route(rooms, pieces, at), None);
        // Off the net entirely — a parked pointer — routes nowhere.
        assert_eq!(
            crate::rig::handle_route(rooms, pieces, crate::bridge::POINTER_PARKED),
            None
        );
    }

    /// The carried ghost promises the berth it would take: the preview
    /// rotation is [`site_on`]'s, so a piece hovering over a starboard
    /// cell stands upright (the side charts' quarter turn rolled out)
    /// and one hovering over a front-row floor cell has already turned
    /// its back to the wall it would stand against.
    #[test]
    fn the_carry_preview_promises_the_berth() {
        let aft = chart(Station::BayWall);
        let starboard = chart(Station::BayStarboard);
        let floor = chart(Station::BayFloor);
        let rooms = space_trucking::sim::Sim::new(1).rooms().clone();
        let hover = |station: Station, surface: &SimSurface, kind: Kind, x: u8, y: u8| {
            let cell = layout::cell_rect(CABIN, x, y);
            let at = SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
            hover_pose(&rooms, station, surface, Some(&aft), kind, at)
                .expect("the aim is on the net")
                .0
        };
        let up = hover(Station::BayStarboard, &starboard, Kind::ChartTank, 12, 5);
        assert!(
            (up * Vec3::Y).y > 0.9,
            "a hovered tank must stand up, not lie on its side: {:?}",
            up * Vec3::Y
        );
        assert!(
            (up * Vec3::Z).dot(Station::BayStarboard.inward(&starboard)) > 0.9,
            "and still face into the room"
        );
        // The placed transform is the same one, to the last bit.
        let placed = site_on(
            Station::BayStarboard,
            &starboard,
            &aft,
            Kind::ChartTank,
            rect_of(12, 5, Kind::ChartTank),
        )
        .1;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(
                (up * axis - placed * axis).length() < 1e-5,
                "the preview's {axis:?} ({:?}) drifted from the berth's ({:?})",
                up * axis,
                placed * axis
            );
        }
        // The floor's backing rule applies to the ghost too.
        let backed = hover(Station::BayFloor, &floor, Kind::Couch, 3, 9);
        assert!(
            (backed * Vec3::Z).z > 0.9,
            "a couch hovered on the front row must already have its back to the wall"
        );
    }

    /// One berth under test: a kind hung at one cell of one chart, with
    /// everything the three claims below need to interrogate it.
    struct Berth {
        kind: Kind,
        cell: (u8, u8),
        rooms: Rooms,
        station: Station,
        surface: SimSurface,
        aft: SimSurface,
        rect: Rect,
        /// A covering LIES into its chart instead of standing on it.
        laid: bool,
        site: (Vec3, Quat, Vec3),
        name: String,
    }

    /// Whether the body a rig DRAWS covers exactly the plan the sim
    /// ruled on: the four corners of its `w × h` silhouette land on the
    /// four corners its rect owns on the chart, in whatever order the
    /// turn left them. A half turn always passes; a quarter turn passes
    /// only for a square footprint — which is the whole reason the
    /// upright rule counts cells before it spends a roll.
    fn lies_on_its_cells(
        surface: &SimSurface,
        kind: Kind,
        rect: Rect,
        rot: Quat,
        scale: Vec3,
    ) -> bool {
        // The body, in the kind's OWN upright frame, and the cells the
        // sim gave it, in the CHART's — two different shapes on a flank,
        // where the sheet's columns climb the wall, and the turn is what
        // has to carry one onto the other.
        let (a, t) = kind.upright();
        let hw = f32::from(a) * layout::CELL * 0.5 * scale.x;
        let hh = f32::from(t) * layout::CELL * 0.5 * scale.y;
        let (cu, cv) = (rect.w * 0.5 * scale.x, rect.h * 0.5 * scale.y);
        let (u, v) = (surface.half_u.normalize(), surface.half_v.normalize());
        let quadrants = [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
        quadrants.iter().all(|&(a, b)| {
            let drawn = rot * Vec3::new(a * hw, b * hh, 0.0);
            quadrants
                .iter()
                .any(|&(c, d)| (drawn - (u * (c * cu) + v * (d * cv))).length() < 1e-4)
        })
    }

    /// Claim one: the berth stands its cargo the way the cargo is drawn.
    /// A rig that STANDS answers for its own body, so all its chart owes
    /// is which plates it rises from; anything flat must face into the
    /// room, lie ON its own cells, and read up-is-up wherever its
    /// footprint can afford the turn — the side charts' columns run up
    /// the wall, so their quarter turn would cost a non-square its cells
    /// and it keeps the chart's own lie instead.
    ///
    /// **Which way a body on a FLAT chart is looking is asked next door**,
    /// and it is asked there because it cannot be asked here: this sweep
    /// walks the cabin's own six charts, and a deck rig's turn is a
    /// question about the seams of whatever room it is standing in.
    /// `gauntlet::berth_turned` asks it of every room the game has —
    /// whether the deck a standing rig faces is deck of the same room —
    /// and it found the deckhead taking one fixed turn from every cell of
    /// every ceiling in the game.
    fn the_body_hangs_true(b: &Berth) {
        let (_, rot, scale) = b.site;
        let (name, station) = (&b.name, b.station);
        let flat = matches!(station, Station::BayFloor | Station::BayCeiling);
        if !b.laid && flat {
            assert!(
                (rot * Vec3::Y - Vec3::Y).length() < 1e-4,
                "{name}: a standing rig must rise world-up, got {:?}",
                rot * Vec3::Y
            );
            assert!(
                (rot * Vec3::Z).y.abs() < 1e-3,
                "{name}: a standing rig looks across the room, not at the plate"
            );
            return;
        }
        assert!(
            (rot * Vec3::Z).dot(station.inward(&b.surface)) > 0.999,
            "{name}: flat cargo must face into the room"
        );
        assert!(
            lies_on_its_cells(&b.surface, b.kind, b.rect, rot, scale),
            "{name}: the drawn body left its own cells"
        );
        if flat {
            return;
        }
        // **Up is up on every wall a body may hang on**, and this is the
        // sentence the sweep used to be missing.
        //
        // It used to read the other way round: up on the aft and front
        // charts, *sideways* on the flanks, because that is what
        // [`wall_upright`] does with a non-square footprint. A test that
        // restates the branch it is testing can only fail when the
        // implementation contradicts itself, so this one passed on every
        // one of its two thousand berths while the starting window came
        // out a quarter turn from where it went in. The defect was never
        // in the roll: the CELLS turn, because the net's side flaps fold
        // out sideways, and the body lies on its cells.
        //
        // A footprint is stated in the wall's own frame now
        // (`cargo::Kind::plan_on`), so the cells a flank berth owns are
        // already the rolled ones and the roll is always affordable. The
        // claim can be the player's rather than the rule's.
        assert!(
            (rot * Vec3::Y).y > 0.999,
            "{name}: a hung body reads up-is-up on every wall it may take, got {:?}",
            rot * Vec3::Y
        );
    }

    /// Claim two: the carried ghost promises the berth it would take, to
    /// the last bit — the turn AND the stand-off. Preview and berth
    /// share [`site_on`] today; the claim is here so no refactor can
    /// quietly split them again.
    ///
    /// **The stand-off half of it is the half that was missing**, and it
    /// was invisible while every kind was drawn centred in its own cell:
    /// the ghost hung a rig's ORIGIN at the point the crosshair struck,
    /// so a body centred on its origin sat half in the deck and read as
    /// a piece more or less resting on it. Every kind that stands is
    /// drawn wholly above its origin now, and the same hover would have
    /// put the whole of it under the floor.
    ///
    /// Coverings are the one exemption, and honestly so: a carried rug
    /// is ROLLED UP — a different body of the same rig — and its ghost
    /// promises the berth THAT body takes.
    fn the_ghost_promises_the_berth(b: &Berth) {
        if b.laid {
            return;
        }
        // Aimed at the ANCHOR cell: a ghost's footprint hangs off the
        // cell under the crosshair, so that is the aim this berth would
        // ever be reached by.
        let aim = rect_center(layout::cell_rect(CABIN, b.cell.0, b.cell.1));
        let (preview, stand) =
            hover_pose(&b.rooms, b.station, &b.surface, Some(&b.aft), b.kind, aim)
                .expect("the aim is on the net");
        let (name, rot) = (&b.name, b.site.1);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(
                (preview * axis - rot * axis).length() < 1e-5,
                "{name}: the preview's {axis:?} ({:?}) drifted from the berth's ({:?})",
                preview * axis,
                rot * axis
            );
        }
        // And the hover the runtime builds out of that stand-off stands
        // the piece exactly where the berth will, one lift proud of it.
        //
        // **The whole offset, not merely the reach off the chart.** A
        // standing berth draws its rig back onto its own cells
        // ([`site_on`]), so a ghost carrying only the height hovered
        // square over the cell and promised a landing most of half a
        // cell out into the aisle. What the crosshair is allowed to move
        // is where the ghost hangs, not how the berth is composed: the
        // aim is the ANCHOR cell and a footprint hangs off it, so the
        // ghost and the berth differ by exactly that cell's own offset
        // from the middle of the rect, and by nothing else.
        let inward = b.station.inward(&b.surface);
        let hovered = b.surface.to_world(aim) + inward * CARRY_LIFT + stand;
        let promised = b.site.0 + inward * CARRY_LIFT;
        let anchored = b.surface.to_world(aim) - b.surface.to_world(rect_center(b.rect));
        assert!(
            (hovered - promised - anchored).length() < 1e-4,
            "{name}: the ghost hovers at {hovered:?}, the berth stands it at {promised:?}, \
             and the aimed cell is only {anchored:?} off the plan's own middle"
        );
    }

    /// Claim three, for a kind that wears one: every texel of amber the
    /// rig DRAWS routes as carry, and the body around it routes to the
    /// instrument's focus — asked through the surface the crosshair
    /// actually meets at this berth, face or chart. `false` where the
    /// kind wears no handle at all.
    fn the_amber_is_the_routing_region(b: &Berth, charts: &[(Station, SimSurface)]) -> bool {
        let (w, h) = b.kind.upright();
        let (fw, fh) = (f32::from(w) * layout::CELL, f32::from(h) * layout::CELL);
        let (Some(handle), Some((at, size))) =
            (carry_handle_rect(b.kind, b.rect), grab_bar(b.kind, fw, fh))
        else {
            return false;
        };
        // The board this berth would make, so the routing is asked the
        // way the runtime asks it.
        let board = vec![Piece {
            id: 1,
            kind: b.kind,
            variant: 0,
            gnawed: false,
            loc: Loc::Hold {
                room: CABIN,
                x: b.cell.0,
                y: b.cell.1,
            },
        }];
        let face = standing_surface(charts, b.kind, b.rect);
        let quad = face.unwrap_or(b.surface);
        let n = face.map_or_else(|| b.station.inward(&b.surface), |f| f.normal());
        let (pos, rot, scale) = b.site;
        let name = &b.name;
        // Where the crosshair lands, aiming square at a point of the
        // rig's own body.
        let aim_at = |local: Vec3| {
            let drawn = pos + rot * (local * scale);
            let ray = Ray3d::new(drawn + n * 0.6, Dir3::new(-n).expect("a unit normal"));
            quad.project(ray).expect("the aim meets the piece").1
        };
        let bar = Vec2::new(size.x * GRAB_BAR_W, size.y * GRAB_BAR_H) * 0.5;
        for (dx, dy) in [
            (0.0f32, 0.0f32),
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ] {
            let local = Vec3::new(dx.mul_add(bar.x, at.x), dy.mul_add(bar.y, at.y), 0.0);
            let sim = aim_at(local);
            assert!(
                handle.contains(sim),
                "{name}: amber drawn at {local:?} reads {sim:?}, outside {handle:?}"
            );
            assert_eq!(
                crate::rig::handle_route(&b.rooms, &board, sim),
                None,
                "{name}: the grab must reach the sim as a carry"
            );
        }
        // And the rest of the body is NOT the handle: a click there is
        // the instrument's focus, which is the whole point of declaring
        // a band at all.
        let sim = aim_at(Vec3::ZERO);
        assert!(
            !handle.contains(sim),
            "{name}: the piece's middle reads as grab at {sim:?}"
        );
        assert_eq!(
            crate::rig::handle_route(&b.rooms, &board, sim),
            instrument(b.kind).and_then(|mount| crate::rig::Focus::of(mount.station)),
            "{name}: the body must answer with its own station"
        );
        true
    }

    /// **Claim four: the face a berth carries is the body it draws.**
    ///
    /// A footprint and a silhouette are two claims about one object, and
    /// the pick face used to be cut from the first: a piece answered
    /// over its whole plan, air included. The brine pearls are three
    /// spheres in a column filling 62% of their cells across, so a third
    /// of a cell of nothing on either flank of them picked them up —
    /// which is the playtest's "the hitbox is horizontal and the item is
    /// vertical", the plan being the shape that lies flat.
    ///
    /// So the face is measured against [`silhouette`] here, and the
    /// corners of the drawn body are aimed at through it: what a player
    /// can see is what answers, and what answers stays inside the cells
    /// the sim ruled on. `false` where the berth carries no face at all.
    fn the_face_is_the_body_it_draws(b: &Berth, charts: &[(Station, SimSurface)]) -> bool {
        // A covering has no hold form aboard, so no berth of one ever
        // carries a face: it lies INTO its chart, and the chart is the
        // piece there as surely as it is for level wall cargo.
        if b.laid {
            return false;
        }
        let Some(face) = standing_surface(charts, b.kind, b.rect) else {
            return false;
        };
        let (mid, half) = silhouette(b.kind);
        let (pos, rot, scale) = b.site;
        let name = &b.name;
        for (axis, quad, drawn, unit) in [
            ("across", face.half_u.length(), half.x, scale.x),
            ("up", face.half_v.length(), half.y, scale.y),
        ] {
            let want = drawn * unit;
            assert!(
                (quad - want).abs() < 1e-4,
                "{name}: the face measures {quad} {axis}, the body {want}"
            );
        }
        // Every corner of the drawn body reads a point of the piece's
        // own cells, and the aim a hand's breadth beyond it reads
        // nothing at all: the face is the picture, edge included.
        let n = face.normal();
        let aim = |local: Vec2| {
            let at = pos + rot * (Vec3::new(local.x, local.y, 0.0) * scale);
            face.project(Ray3d::new(
                at + n * 0.6,
                Dir3::new(-n).expect("a unit normal"),
            ))
        };
        for (a, b_) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = Vec2::new(a.mul_add(half.x, mid.x), b_.mul_add(half.y, mid.y));
            // A hair inside the corner, drawn back toward the body's own
            // middle. It used to be drawn back toward the rig's ORIGIN,
            // which is the same point only while every kind is composed
            // centred in its cell — and none of the ones that stand on a
            // deck is, now that they are drawn standing on it.
            let inside = mid + (corner - mid) * 0.999;
            let sim = aim(inside).expect("the aim meets the body").1;
            assert!(
                b.rect.contains(sim),
                "{name}: the body's corner {corner:?} reads {sim:?}, off its own cells"
            );
            let past = Vec2::new(
                a.mul_add(half.x + 4.0, mid.x),
                b_.mul_add(half.y + 4.0, mid.y),
            );
            assert!(
                aim(past).is_none(),
                "{name}: the face answers {past:?}, which is air beside the body"
            );
        }
        true
    }

    /// The orientation defect class, closed by sweep: every kind, at
    /// every placement the sim's own arbiter allows, put to all four
    /// claims above. Each of them held on the wall it was written
    /// against and nowhere else at some point in this class's history —
    /// the sideways star chart, the front wall's upside-down sky, the
    /// grab bar a quarter turn off the band that routes it — so a sweep
    /// is the only shape of test that can say "on every wall" and mean
    /// it.
    #[test]
    fn every_kind_hangs_true_on_every_legal_berth() {
        use space_trucking::sim::cargo::{dressing_check, placement_check};
        use space_trucking::sim::room::Rooms;
        let ship = Rooms::new();
        let charts = rig::bay();
        let aft = chart(Station::BayWall);
        let mut swept = 0_u32;
        let mut handled = 0_u32;
        let mut faced = 0_u32;
        let mut walls_handled: Vec<Station> = Vec::new();
        for kind in Kind::ALL {
            let (cols, rows) = space_trucking::sim::RoomKind::Cabin.grid();
            for y in 0..rows {
                for x in 0..cols {
                    // The sim rules the board; the cabin only draws it.
                    // A covering answers to the dressing arbiter — it
                    // has no hold form aboard at all — and everything
                    // else to the placement ladder, on an empty board so
                    // the sweep asks about charts, not about neighbours.
                    let laid = kind.covering();
                    let legal = if laid {
                        dressing_check(&ship, &[], 0, kind, CABIN, x, y).is_ok()
                    } else {
                        placement_check(&ship, &[], 0, kind, CABIN, x, y).is_ok()
                    };
                    if !legal {
                        continue;
                    }
                    let rect = rect_of(x, y, kind);
                    let (station, surface) =
                        chart_at(&charts, rect_center(rect)).expect("a legal berth is on a chart");
                    let berth = Berth {
                        kind,
                        cell: (x, y),
                        rooms: ship.clone(),
                        station,
                        surface,
                        aft,
                        rect,
                        laid,
                        site: if laid {
                            laid_on(station, &surface, rect)
                        } else {
                            site_on(station, &surface, &aft, kind, rect)
                        },
                        name: format!("{kind:?} at ({x}, {y}) on {station:?}"),
                    };
                    swept += 1;
                    the_body_hangs_true(&berth);
                    the_ghost_promises_the_berth(&berth);
                    faced += u32::from(the_face_is_the_body_it_draws(&berth, &charts));
                    if the_amber_is_the_routing_region(&berth, &charts) {
                        handled += 1;
                        if !walls_handled.contains(&station) {
                            walls_handled.push(station);
                        }
                    }
                }
            }
        }
        assert!(
            swept > 1000,
            "the sweep should cover the whole net: {swept}"
        );
        assert!(faced > 500, "the faces went unswept: {faced} of {swept}");
        // "On every wall" is the claim, so the sweep proves it reached
        // every wall: a handle checked on the aft chart alone is the
        // very mistake this test exists to catch.
        for wall in [
            Station::BayWall,
            Station::BayPort,
            Station::BayStarboard,
            Station::BayFront,
        ] {
            assert!(
                walls_handled.contains(&wall),
                "no handled kind was swept on {wall:?} ({handled} berths in all)"
            );
        }
    }

    /// One kind's rig, described.
    fn rig_of(kind: Kind, screens: Screens) -> Vec<Part> {
        parts(
            &Piece {
                id: 0,
                kind,
                variant: 0,
                gnawed: false,
                loc: Loc::Hold {
                    room: CABIN,
                    x: 0,
                    y: 0,
                },
            },
            screens,
        )
    }

    /// **A rig spends the same metre on every chart.** [`RIG_UNIT`] is
    /// one number because every chart of every room is laid at
    /// `rig::BAY_CELL` to the cell — and the gauntlet measures a rig's
    /// own parts against world thresholds through it, so a chart that
    /// scaled differently would have the sweep judging one rig by
    /// another's ruler. Derived from [`site_on`] rather than compared to
    /// a written-down number, so a retune of the bay moves both.
    #[test]
    fn a_rig_spends_the_same_metre_on_every_chart() {
        let charts = rig::bay();
        let aft = chart(Station::BayWall);
        let mut seen = 0_u32;
        for (station, surface) in charts {
            if !station.chart_flipped() {
                continue;
            }
            let (_, _, scale) = site_on(
                station,
                &surface,
                &aft,
                Kind::Painting,
                rect_of(4, 1, Kind::Painting),
            );
            for spent in [scale.x, scale.y, scale.z] {
                assert!(
                    (spent - RIG_UNIT).abs() < 1e-6,
                    "{station:?} spends {spent} m of world on a sim unit, not {RIG_UNIT}"
                );
            }
            seen += 1;
        }
        assert!(seen >= 6, "the cabin's six charts went missing: {seen}");
    }

    /// **Every cargo kind describes a body.** The describer is the only
    /// account of what a rig is now, so a kind that says nothing is a
    /// kind that draws nothing — the empty-room defect, one crate down.
    #[test]
    fn every_kind_describes_a_body() {
        for kind in Kind::ALL {
            let rig = rig_of(kind, Screens::LIVE);
            assert!(!rig.is_empty(), "{kind:?} describes no parts at all");
            assert!(
                rig.iter().any(|part| part.body.is_some()),
                "{kind:?} describes only lights, and lights are not a silhouette"
            );
        }
    }

    /// **The promise a part makes is the turn the rig gives it.** A
    /// claim used to be written in one table and the turn taken from it
    /// by name, which is a design that guarantees the bug it detects:
    /// two of four disagreed, and they are the two the playtest found by
    /// eye. The claim rides the part now, and the part's own rotation is
    /// derived from it, so this asserts a thing that cannot come apart —
    /// which is the point of asserting it, because the shape that COULD
    /// come apart is what a later hand would reach for.
    #[test]
    fn the_promise_a_part_makes_is_the_turn_the_rig_gives_it() {
        let mut claimed = 0_u32;
        for kind in Kind::ALL {
            for part in rig_of(kind, Screens::LIVE) {
                let Some(claim) = part.claim else { continue };
                claimed += 1;
                assert_eq!(claim.name, part.what, "{kind:?}: a claim under two names");
                let got = (part.at.rotation * claim.axis).normalize_or_zero();
                assert!(
                    got.dot(claim.want.normalize_or_zero()) > 0.999,
                    "{kind:?}'s {} points {got} and its name says {}",
                    part.what,
                    claim.want
                );
            }
            // And the list of promises is that same reading, not a
            // second table beside it.
            let named: Vec<Feature> = rig_of(kind, Screens::LIVE)
                .into_iter()
                .filter_map(|part| part.claim)
                .collect();
            assert_eq!(named, features(kind), "{kind:?}'s promises are restated");
        }
        assert!(claimed >= 4, "the claim-bearing parts went missing");
    }

    /// **A part hangs in a frame its own kind is entitled to.** The
    /// sub-roots are not decoration: something moves each of them, and a
    /// part in the wrong one is a part some system will swing, throw, or
    /// hide for reasons that have nothing to do with it.
    #[test]
    fn a_part_hangs_in_a_frame_its_kind_is_entitled_to() {
        for kind in Kind::ALL {
            for part in rig_of(kind, Screens::LIVE) {
                let allowed = match part.under {
                    Under::Rig => true,
                    Under::Arm => kind == Kind::WallLamp,
                    Under::Pivot(_) => kind == Kind::LaunchLever,
                    Under::Laid | Under::Packed => kind.covering(),
                };
                assert!(
                    allowed,
                    "{kind:?}'s {} hangs in {:?}, which nothing on it owns",
                    part.what, part.under
                );
            }
        }
    }

    /// **A covering owns two bodies and nothing else owns any.** The
    /// dressing regime's whole shape: laid into the room versus rolled
    /// or canned on a counter, with `sync_dressings` showing exactly one.
    #[test]
    fn a_covering_owns_two_bodies_and_nothing_else_owns_either() {
        for kind in Kind::ALL {
            let rig = rig_of(kind, Screens::LIVE);
            let laid = rig
                .iter()
                .filter(|part| part.under == Under::Laid && part.body.is_some())
                .count();
            let packed = rig
                .iter()
                .filter(|part| part.under == Under::Packed && part.body.is_some())
                .count();
            if kind.covering() {
                assert!(laid > 0 && packed > 0, "{kind:?} is missing a body");
            } else {
                assert_eq!(
                    (laid, packed),
                    (0, 0),
                    "{kind:?} is not a covering and keeps two bodies"
                );
            }
        }
    }

    /// **Only glass reads differently in the dark.** A headless boot has
    /// no void and no rasteriser, so the kinds that wear a live screen
    /// fall back to phosphor with geometry of their own — and every
    /// other kind must describe the identical rig either way, or the
    /// sweep would be judging a game nobody plays.
    #[test]
    fn only_glass_reads_differently_in_a_headless_boot() {
        for kind in Kind::ALL {
            let lit = rig_of(kind, Screens::LIVE);
            let dark = rig_of(kind, Screens::default());
            let glazed = matches!(
                kind,
                Kind::Window
                    | Kind::Porthole
                    | Kind::BayWindow
                    | Kind::ChartTank
                    | Kind::DestPreview
            );
            assert_eq!(
                lit != dark,
                glazed,
                "{kind:?} wears live glass: {glazed}, and reads differently dark: {}",
                lit != dark
            );
        }
    }

    /// **A tell is never drawn inside the piece it is about.**
    ///
    /// The defect this whole layer was rebuilt for, stated as a law. A
    /// mark used to be four short bars set 62% of the way in from its
    /// footprint's rim, which is *inside the footprint* — and a painting
    /// hangs flat on a wall and fills its footprint, so the mark on a
    /// picture for sale was drawn behind the picture. Press a good, and
    /// nothing on screen changed.
    ///
    /// The wall does not appear in this test and that is the point: a
    /// berth is a rigid motion, so a bar that stands clear of the body
    /// in the rig's own frame stands clear of it on every chart in the
    /// game. Which is the property an outline has and a decal painted on
    /// the ground under a thing does not.
    #[test]
    fn a_tell_never_draws_inside_the_body_it_is_about() {
        for kind in Kind::ALL {
            let (mid, half) = drawn_box(kind);
            for tell in [Tell::Aim, Tell::Offered, Tell::Marked] {
                for bar in tell_bars(mid, half, tell) {
                    let gap = (bar.at - mid).abs() - (bar.size * 0.5 + half);
                    assert!(
                        gap.max_element() > 0.0,
                        "{kind:?}: a {tell:?} bar at {:?} of {:?} is buried in a body of {half:?}",
                        bar.at - mid,
                        bar.size
                    );
                }
            }
        }
    }

    /// **No two tells draw one bar over another.**
    ///
    /// Three readings may be worn at once — the crosshair rests on a
    /// good the room has offered and you have asked for — so the forms
    /// have to be able to share a body. Each has a stand-off of its own,
    /// and the girths are cut to leave daylight between them.
    ///
    /// It is the coplanar question asked where a tell can answer it. Two
    /// amber bars meeting on one line at one depth is the shimmer the
    /// old inset was bought to avoid, and the inset is what buried a
    /// mark inside the picture it was about.
    #[test]
    fn no_two_tells_draw_one_bar_over_another() {
        let overlap = |a: &Bar, b: &Bar| {
            let gap = (a.at - b.at).abs() - (a.size + b.size) * 0.5;
            gap.max_element() <= 0.0
        };
        for kind in Kind::ALL {
            let (mid, half) = drawn_box(kind);
            let forms =
                [Tell::Aim, Tell::Offered, Tell::Marked].map(|tell| tell_bars(mid, half, tell));
            for (i, one) in forms.iter().enumerate() {
                for other in forms.iter().skip(i + 1) {
                    for a in one {
                        for b in other {
                            assert!(
                                !overlap(a, b),
                                "{kind:?}: a bar at {:?} of {:?} lies over one at {:?} of {:?}",
                                a.at,
                                a.size,
                                b.at,
                                b.size
                            );
                        }
                    }
                }
            }
        }
    }

    /// The z-fight guard: every occupied rung of the decal ladder —
    /// including the rug's pile top, which rides between LAID and HINT —
    /// steps at least `layer::STEP` from its neighbours, and the step
    /// itself clears two skins of mesh. A new decal gets a named rung
    /// and a row here, or it shimmers like the playtest doormat did.
    ///
    /// **Every rung in `rig::layer` must appear below.** A rung declared
    /// and left out of this list is a rung nobody is checking, so the
    /// count is asserted too: adding one to the ladder without spacing
    /// it fails the build rather than the eye.
    #[test]
    fn the_decal_ladder_never_z_fights() {
        use crate::rig::layer;
        let rungs = [
            // The ladder's own basement: the backer slab's face, which
            // is the only thing on the ladder standing BEHIND the
            // mapping plane. Its BACK is not a rung — it is the same
            // slab — and it answers to the hull check below instead.
            ("backer face", -layer::BACKER),
            // A colored tile carries up to three readings, and they get
            // three rungs, room after room, because a room's tiles are
            // the cabin's tiles one lane over: the class's FIELD under
            // the berth wells, the class's own MARK on its region's rim,
            // and the TREAD of any doorway crossing the same deck. All
            // three landed on one square metre of the Guild's floor in
            // the playtest, two of them sharing a rung, and shimmered.
            ("tile field / berth well", layer::TILE),
            ("tile mark", layer::MARK),
            ("threshold tread", layer::TREAD),
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
        // **Nothing is declared a rung and then left unchecked.** The
        // ladder is read back out of its own source and counted against
        // the list above, the way the palette reads the crate back for
        // raw colors. The three constants that are not rungs — the
        // backer's thickness, the step, and the skin limit — name
        // themselves here, so a new rung has nowhere to hide.
        let ladder = include_str!("rig.rs")
            .split_once("pub mod layer {")
            .and_then(|(_, rest)| rest.split_once("\n}"))
            .expect("the decal ladder is a module in rig.rs")
            .0;
        let declared: Vec<&str> = ladder
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub const "))
            .filter_map(|line| line.split_once(':').map(|(name, _)| name))
            .filter(|name| !matches!(*name, "BACKER_T" | "STEP" | "SKIN"))
            .collect();
        assert_eq!(
            declared.len(),
            // Every named rung, plus the rug's pile top, which is a
            // height rather than a constant.
            rungs.len() - 1,
            "the ladder declares {declared:?} but only {} rungs are spaced",
            rungs.len() - 1
        );
        // **And the ladder stands clear of the hull it is painted on.**
        // A backer thick enough to reach the slab behind it is sliced at
        // that slab's own plane by the aperture punch, and the remainder
        // and the hull then present two opaque faces on one plane — the
        // playtest's flickering deck, which the tiles ON it never showed
        // because they ride rungs and the deck did not.
        let slabs = rig::structure();
        for (station, surface) in rig::bay() {
            if !matches!(station, Station::BayWall | Station::BayFloor) {
                continue;
            }
            let n = station.inward(&surface);
            let axis = if n.y.abs() > 0.5 { 1 } else { 2 };
            let plate = [
                surface.center - n * layer::BACKER,
                surface.center - n * (layer::BACKER + layer::BACKER_T),
            ];
            for slab in &slabs {
                let lo = slab.center - slab.size * 0.5;
                let hi = slab.center + slab.size * 0.5;
                // Only slabs this plate actually covers can fight it.
                let spans = (0..3).filter(|k| *k != axis).all(|k| {
                    let half = (surface.orientation()
                        * Vec3::new(surface.half_u.length(), surface.half_v.length(), 0.0))
                    .abs()[k];
                    hi[k].min(surface.center[k] + half) - lo[k].max(surface.center[k] - half) > 0.05
                });
                if !spans {
                    continue;
                }
                let (near, far) = (
                    plate[0][axis].min(plate[1][axis]),
                    plate[0][axis].max(plate[1][axis]),
                );
                for face in [lo[axis], hi[axis]] {
                    assert!(
                        face <= near - layer::STEP + 1e-6 || face >= far + layer::STEP - 1e-6,
                        "{station:?}'s backer spans {near}..{far} across a hull face at \
                         {face}: the aperture punch will slice it there and leave two \
                         opaque faces on one plane"
                    );
                }
            }
        }
    }

    /// **A lamp's bulb burns inside the shade that shades it.** Three
    /// kinds hang a light and every one of them is a cone with a glass
    /// in its mouth, which is a promise about two parts of one rig and
    /// not about either part alone: a bulb outside its shade lights the
    /// room from a bare ball hanging in the air, and the shade it left
    /// behind reads as an empty dark cup.
    ///
    /// Nothing in the harness could ask this. The seven families all
    /// measure a part against the WORLD — the band it is composed in,
    /// the plane it fights, the cells it draws inside, the direction its
    /// own name claims — and a stranded bulb satisfies all of them. It
    /// is inside `RIG_NEAR..RIG_FAR`, it shares no plane with anything,
    /// it draws well within its cells, and it makes no claim of its own
    /// to break. The sconce carried one for a month with a green build.
    ///
    /// Stated as a band rather than a point on purpose. The builder
    /// seats the bulb AT the mouth; what a viewer needs is only that it
    /// is in the opening — within the mouth's own circle, and no further
    /// out of it than its own glass — so a lamp may be retuned without
    /// this becoming a copy of the arithmetic that placed it.
    #[test]
    fn a_lamps_bulb_burns_inside_the_shade_that_shades_it() {
        let mut lit = 0_u32;
        for kind in Kind::ALL {
            for screens in Screens::BOTH {
                let rig = rig_of(kind, screens);
                for bulb in rig.iter().filter(|p| matches!(p.role, Role::Bulb { .. })) {
                    let Some(Body::Ball { r }) = bulb.body else {
                        panic!("{kind:?}'s bulb is not a glass")
                    };
                    // The shade it belongs to: the cone hung in the same
                    // sub-frame, which is the only thing on any of these
                    // rigs that has a mouth to seat a glass in.
                    let shade = rig
                        .iter()
                        .find(|p| {
                            p.under.same(bulb.under) && matches!(p.body, Some(Body::Horn { .. }))
                        })
                        .unwrap_or_else(|| panic!("{kind:?} lights a bulb under no shade"));
                    let Some(Body::Horn { r: mouth, h }) = shade.body else {
                        unreachable!("the shade was found by being a cone")
                    };
                    // The bulb's centre, in the shade's own frame, where
                    // the mouth is the disc at -h/2 about the axis.
                    let seat =
                        shade.at.rotation.inverse() * (bulb.at.translation - shade.at.translation);
                    let off = seat.x.hypot(seat.z);
                    let past = h.mul_add(0.5, seat.y).abs();
                    assert!(
                        off <= mouth,
                        "{kind:?}'s bulb stands {off:.2} off the axis of the shade whose \
                         mouth is {mouth:.2} across: it burns beside the cup, not in it"
                    );
                    assert!(
                        past <= r,
                        "{kind:?}'s bulb sits {past:.2} from its shade's mouth, further \
                         than the {r:.2} of its own glass"
                    );
                    lit += 1;
                }
            }
        }
        assert!(lit >= 6, "the lamps went dark: {lit} bulbs swept");
    }
}

#[cfg(test)]
mod band {
    use super::*;

    /// **A rig is drawn one cell deep.**
    ///
    /// The world is built of `rig::BAY_CELL` cubes — rooms four cells
    /// tall, walls three courses, a cell of padding between rooms — and
    /// the band every kind is composed within was the last length in it
    /// that was not. It was 0.497 m, nine tenths of a cell, which is a
    /// number off no line in particular; it is one cell wearing the same
    /// [`BAY_FIT`] the width and the height wear, so a rig fills the same
    /// fraction of its berth on all three axes.
    ///
    /// Asked of the metres rather than of the constant, because what has
    /// to land on the grid is the depth a berth actually spends.
    #[test]
    fn a_rig_is_drawn_one_cell_deep() {
        let (near, far) = (RIG_NEAR * RIG_UNIT, RIG_FAR * RIG_UNIT);
        let depth = far - near;
        let cell = crate::rig::BAY_CELL * BAY_FIT;
        assert!(
            (depth - cell).abs() < 1e-5,
            "a rig is drawn {depth} m deep where a berth is {cell} m of cell",
        );
        // And the near face still stands just BEHIND the berth plane, so
        // a body flush with its own chart is inside the box that wraps it.
        assert!(
            near < 0.0,
            "the near face at {near} m has to clear the chart"
        );
    }
}
