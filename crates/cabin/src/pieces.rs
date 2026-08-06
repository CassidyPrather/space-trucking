//! Cargo pieces and the rat, made physical: every [`Piece`] the sim knows
//! becomes a small low-poly rig standing proud of its panel like an object
//! in a tray — hold pieces on the hold rack, everything else on the barter
//! counter — plus the held piece glued to the pointer, per-cell placement
//! hints, drop-target invitations, the hard-reject flash, and the stowaway.
//!
//! Semantics mirror the 2D console's `render.rs` (`draw_pieces`,
//! `piece_glyph`, `draw_held`, `draw_drop_hints`, `draw_rat`,
//! `draw_violation_flash`): the sim stays the only arbiter — footprints
//! come from `layout::piece_rect`, legality from `placement_check`, invites
//! from `drop_targets` — and no refusal rides on hue alone: illegality
//! always carries a slash, gnawing carries a wedge, shapes over colors.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use bevy::prelude::*;

use space_trucking::sim::layout::{self, Rect};
use space_trucking::sim::{
    Cue, Kind, Loc, Piece, Vec2 as SimVec2, Violation, placement_check, player_owned, splitmix,
};

use crate::rig::Skin;
use crate::surface::{SimSurface, Station, VirtualPointer};
use crate::{Phase, Shell, glow, palette};

/// How long a piece takes to glide to a new berth, seconds.
const EASE_LEN: f32 = 0.15;

/// Scale-settle (1.1 → 1.0) after `Cue::Place`, seconds.
const SETTLE_LEN: f32 = 0.18;

/// Violation flash length — the 2D juice's clock, kept.
const FLASH_LEN: f32 = 0.45;

/// How far a carried piece hovers off the struck surface, meters.
const CARRY_LIFT: f32 = 0.05;

/// Fraction of its rect a rig fills, so tray neighbours never touch.
const FIT: f32 = 0.88;

/// Rat hop tween length in ticks (0.35 s), same as the 2D renderer.
const RAT_HOP_TICKS: f32 = 21.0;

/// Salt for emissive pulse phases, off every sim stream.
const SALT_PULSE: u64 = 0x91EC_E501;

/// Salt for the bite wedge's spin.
const SALT_BITE: u64 = 0x91EC_B17E;

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
                    carry_held,
                    placement_hints,
                    invite_glows,
                    violation_flash,
                    rat_watch,
                    breathe_pulses,
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

/// The hard-reject flash: the refused footprint in sim coordinates and how
/// long the frame keeps burning.
#[derive(Resource, Default)]
struct FlashState {
    left: f32,
    area: Option<Rect>,
    eerie: bool,
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
    frame_root: Entity,
    slash: Entity,
    frame_mat: Handle<StandardMaterial>,
}

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

/// One edge bar of the violation flash frame, `0..4`.
#[derive(Component)]
struct VioBar(u8);

/// The rat rig's root.
#[derive(Component)]
struct RatRoot;

/// The rat's tail, remembering its resting pose so the sway composes.
#[derive(Component)]
struct RatTail {
    base: Quat,
}

/// Handles shared by the overlay systems: the static refusal-slash phosphor
/// and the one violation-flash material all four bars burn through.
#[derive(Resource)]
struct SharedBits {
    slash_mat: Handle<StandardMaterial>,
    flash_mat: Handle<StandardMaterial>,
}

// ------------------------------------------------------------------ helpers --

/// The panel with `want`'s station tag, if the rig spawned it.
fn surface_of(surfaces: &Query<(&Station, &SimSurface)>, want: Station) -> Option<SimSurface> {
    surfaces
        .iter()
        .find(|(station, _)| **station == want)
        .map(|(_, surface)| *surface)
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

/// Pre-spawn everything that waits dark for a sim state to light it: the
/// 6×4 hold hint quads with their slashes, the barter row glow quads, and
/// the four violation-flash bars.
fn spawn_overlays(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
    surfaces: Query<(&Station, &SimSurface)>,
) {
    let slash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 3.0);
    let flash_mat = glow::phosphor(&mut materials, palette::LAMP_NO, 0.0);
    commands.insert_resource(SharedBits {
        slash_mat: slash_mat.clone(),
        flash_mat: flash_mat.clone(),
    });
    let Some(hold) = surface_of(&surfaces, Station::Hold) else {
        return;
    };
    let Some(barter) = surface_of(&surfaces, Station::Barter) else {
        return;
    };

    // Hold cell hints: a thin quad per cell, its refusal slash floating
    // just above it (shape channel — illegality never rides hue alone).
    let (su, sv) = (hold.scale_u(), hold.scale_v());
    let rot = hold.orientation();
    let normal = hold.normal();
    for y in 0..layout::GRID_ROWS {
        for x in 0..layout::GRID_COLS {
            let cell = layout::cell_rect(x, y);
            let center = hold.to_world(rect_center(cell));
            let slash = commands
                .spawn((
                    Mesh3d(skin.cube.clone()),
                    MeshMaterial3d(slash_mat.clone()),
                    Transform::from_translation(center + normal * 0.004)
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
                Transform::from_translation(center + normal * 0.002)
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

    // The violation flash's frame bars, aimed when a hard reject lands.
    for i in 0..4_u8 {
        commands.spawn((
            Mesh3d(skin.cube.clone()),
            MeshMaterial3d(flash_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            VioBar(i),
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
                    // A second crate aboard: the hold objects in violet.
                    flash.eerie = matches!(rule, Violation::Suspicious);
                    // rule glyphs deferred — the 2D per-rule icons
                    // (weight, hazard, snowflake) are not ported yet.
                }
            }
            Cue::Reseed => {
                flash.left = 0.0;
                flash.area = None;
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
/// and run the glide/settle tweens.
#[allow(clippy::too_many_arguments)]
fn sync_pieces(
    mut commands: Commands,
    time: Res<Time>,
    shell: Res<Shell>,
    skin: Res<Skin>,
    shared: Res<SharedBits>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut index: ResMut<PieceIndex>,
    mut settle: ResMut<PendingSettle>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rigs: Query<(&mut PieceRig, &mut Transform)>,
) {
    let sim = &shell.bridge.sim;
    let Some(hold) = surface_of(&surfaces, Station::Hold) else {
        return;
    };
    let Some(barter) = surface_of(&surfaces, Station::Barter) else {
        return;
    };

    // A new world means new cargo: clear everything and respawn below.
    if sim.cues().iter().any(|cue| matches!(cue, Cue::Reseed)) {
        for (_, entity) in index.0.drain() {
            commands.entity(entity).despawn();
        }
    }

    for piece in sim.pieces() {
        // Hold pieces live on the hold rack; every other Loc is barter
        // furniture (all its rects sit inside BARTER_PANEL).
        let surface = if matches!(piece.loc, Loc::Hold { .. }) {
            &hold
        } else {
            &barter
        };
        let rect = layout::piece_rect(piece);
        let (w, h) = piece.kind.cells();
        let fw = f32::from(w) * layout::CELL;
        let fh = f32::from(h) * layout::CELL;
        // Slot rects are smaller than big footprints; fit like the 2D
        // glyph box, aspect kept.
        let fit = (rect.w / fw).min(rect.h / fh) * FIT;
        let (su, sv) = (surface.scale_u(), surface.scale_v());
        let scale = Vec3::new(su, sv, su.min(sv)) * fit;
        let goal = surface.to_world(rect_center(rect));
        let rot = surface.orientation();
        if let Some(&entity) = index.0.get(&piece.id) {
            let Ok((mut rig, transform)) = rigs.get_mut(entity) else {
                continue;
            };
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
            let entity = spawn_rig(
                &mut commands,
                &mut meshes,
                &mut materials,
                &skin,
                &shared,
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
    for (mut rig, mut transform) in &mut rigs {
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

// ------------------------------------------------------------------- carry --

/// The held piece rides the pointer: lifted off the struck surface, a
/// tenth larger, wearing its legality frame — `LAMP_OK` glow for a drop
/// that would land, `LAMP_NO` plus a diagonal slash for one that would not.
#[allow(clippy::too_many_arguments)]
fn carry_held(
    shell: Res<Shell>,
    pointer: Res<VirtualPointer>,
    surfaces: Query<(&Station, &SimSurface)>,
    index: Res<PieceIndex>,
    mut carry: ResMut<CarryState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rigs: Query<(&mut PieceRig, &mut Transform)>,
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

    // Where the hand is: the ray's hit lifted off that panel, or — parked
    // pointer — simply wherever it last hovered.
    if let Some(world) = pointer.world
        && let Some(surface) = pointer.station.and_then(|s| surface_of(&surfaces, s))
    {
        carry.last = Some((world + surface.normal() * CARRY_LIFT, surface.orientation()));
    }
    let Some((pos, rot)) = carry.last.or_else(|| {
        surface_of(&surfaces, Station::Hold)
            .map(|hold| (hold.center + hold.normal() * 0.25, hold.orientation()))
    }) else {
        return;
    };
    carry.last = Some((pos, rot));

    transform.translation = pos;
    transform.rotation = rot;
    transform.scale = rig.scale_goal * 1.1;
    // Keep the tween anchored to the hand, so the eventual drop glides
    // from here to the berth instead of teleporting.
    rig.from = pos;
    rig.rot_from = rot;
    rig.scale_from = rig.scale_goal * 1.1;
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
        let legal = placement_check(sim.pieces(), piece.id, piece.kind, ax, ay).is_ok();
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

/// Breathe amber over exactly the rows the sim's drop matrix invites. The
/// rail quads answer for the shelf while a barter is open and for the
/// outboard net while none is — the two lives of one row of sockets.
fn invite_glows(
    time: Res<Time>,
    shell: Res<Shell>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut glows: Query<(&RowGlow, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
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
}

// -------------------------------------------------------------------- flash --

/// The hard-reject flash: a frame burning over the attempted footprint for
/// just under half a second — `LAMP_NO`, or `EERIE` when the hold itself
/// objected to a second suspicious crate.
fn violation_flash(
    time: Res<Time>,
    shared: Res<SharedBits>,
    surfaces: Query<(&Station, &SimSurface)>,
    mut flash: ResMut<FlashState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bars: Query<(&VioBar, &mut Transform, &mut Visibility)>,
) {
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    let live = flash.left > 0.0;
    let Some(rect) = flash.area.filter(|_| live) else {
        for (_, _, mut visibility) in &mut bars {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(hold) = surface_of(&surfaces, Station::Hold) else {
        return;
    };
    let heat = flash.left / FLASH_LEN;
    let color = if flash.eerie {
        palette::EERIE
    } else {
        palette::LAMP_NO
    };
    if let Some(mut mat) = materials.get_mut(&shared.flash_mat) {
        glow::set_lamp(&mut mat, color, heat);
    }
    let (su, sv) = (hold.scale_u(), hold.scale_v());
    let rot = hold.orientation();
    let normal = hold.normal();
    for (bar, mut transform, mut visibility) in &mut bars {
        *visibility = Visibility::Visible;
        let across = Vec3::new((rect.w + 6.0) * su, 3.0 * sv, 0.003);
        let down = Vec3::new(3.0 * su, (rect.h + 6.0) * sv, 0.003);
        let (mid, scale) = match bar.0 {
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
        transform.translation = hold.to_world(mid) + normal * 0.006;
        transform.rotation = rot;
        transform.scale = scale;
    }
}

// ---------------------------------------------------------------------- rat --

/// The stowaway: spawned while `sim.rat()` says one is aboard, perched on
/// the hold rack, hopping between cells on the sim's own tween (tick,
/// `moved_at`, alpha — replays exactly), nose along its travel.
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
    let Some(hold) = surface_of(&surfaces, Station::Hold) else {
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
    let unit = (hold.scale_u() + hold.scale_v()) * 0.5;
    let hop = (PI * t).sin() * 5.0 * unit;
    let place = Transform::from_translation(hold.to_world(at) + hold.normal() * hop)
        .with_rotation(hold.orientation() * Quat::from_rotation_z(state.yaw))
        .with_scale(Vec3::splat(unit));
    if let Some(entity) = state.entity {
        if let Ok(mut transform) = roots.get_mut(entity) {
            *transform = place;
        }
    } else {
        state.entity = Some(spawn_rat(&mut commands, &mut meshes, &skin, place));
    }

    // The tail sway is decoration, on the idle clock.
    let sway = (glow::breathe(time.elapsed_secs(), 3.0, 0.0) - 0.5) * 0.9;
    for (tail, mut transform) in &mut tails {
        transform.rotation = Quat::from_rotation_z(sway) * tail.base;
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

// ------------------------------------------------------------------- rigs --

/// Everything a kind builder needs in one grip.
struct RigParts<'w, 's, 'a> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    skin: &'a Skin,
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

/// Spawn one piece's whole rig at `place`: the kind's silhouette in local
/// sim units (footprint `w*CELL × h*CELL` in X/Y, thickness up +Z off the
/// panel), the hidden bite wedge, and the hidden carry-legality frame.
fn spawn_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &Skin,
    shared: &SharedBits,
    piece: &Piece,
    place: Transform,
) -> Entity {
    let root = commands.spawn((place, Visibility::default())).id();
    let color = palette::variant_tint(palette::kind_color(piece.kind), piece.variant);
    let (w, h) = piece.kind.cells();
    let fw = f32::from(w) * layout::CELL;
    let fh = f32::from(h) * layout::CELL;
    let mut rig = RigParts {
        commands: &mut *commands,
        meshes: &mut *meshes,
        materials: &mut *materials,
        skin,
        root,
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
    let (hx, hy) = (fw * 0.5, fh * 0.5);
    let rail_h = rig.meshes.add(Cuboid::new(fw + 6.0, 2.6, 2.6));
    let rail_v = rig.meshes.add(Cuboid::new(2.6, fh + 6.0, 2.6));
    for (mesh, at) in [
        (rail_h.clone(), Vec3::new(0.0, hy + 3.0, 12.0)),
        (rail_h, Vec3::new(0.0, -(hy + 3.0), 12.0)),
        (rail_v.clone(), Vec3::new(hx + 3.0, 0.0, 12.0)),
        (rail_v, Vec3::new(-(hx + 3.0), 0.0, 12.0)),
    ] {
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
            MeshMaterial3d(shared.slash_mat.clone()),
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
        // A pot with a sprout on top.
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
        // The five fixtures ride a plain tinted slab until their real
        // rigs land. // fixture pass pending
        Kind::CeilingLamp | Kind::WallLamp | Kind::FloorLamp | Kind::Couch | Kind::Painting => {
            rig.part(
                Cuboid::new(fw * 0.62, fh * 0.62, 14.0),
                body,
                Transform::from_xyz(0.0, 0.0, 7.0),
            );
        }
    }
}
