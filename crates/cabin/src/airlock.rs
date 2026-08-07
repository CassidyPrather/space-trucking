//! The airlock: where cargo waits for the void.
//!
//! The sim's outboard rail — four `Loc::Flotsam` slots, swept by the
//! next docking, cast-off, or encounter close — gets a room of its own:
//! an annex off the starboard wall (`rig`'s `AIR_*` constants carve the
//! doorway and the chamber) with four hazard-bordered berth tiles, each
//! bound to one rail slot through its own [`SimSurface`]. Drop cargo on
//! a tile to stage it, take it back any time before the sweep; when the
//! sweep fires (`Cue::Jettison`) the lock "cycles" — the beacon over
//! the door strobes red for a breath, and the goods are gone.
//!
//! The tiles project only while the sim's rail rule holds (no barter
//! open — the rail IS the shelf row, contexts exclusive by
//! construction); `surface::track_pointer` owns that gate. The chamber
//! is sized so the largest footprint in the game JUST fits, proven by
//! `rig`'s tests, and the annex stands ready to be reused for whatever
//! else wants a door to space.

use bevy::prelude::*;

use space_trucking::sim::layout;
use space_trucking::sim::{Cue, Loc};

use crate::rig::{AIR_DOOR_H, AIR_DOOR_Z0, AIR_DOOR_Z1, AIR_X0, AIR_X1, Skin};
use crate::surface::{SimSurface, Station};
use crate::{Phase, Shell, glow, palette};

/// One berth tile's edge, world units: four fit the chamber two-by-two
/// with walking seams, and a piece bigger than its tile simply crams —
/// the chamber, not the tile, is the fit guarantee.
const TILE: f32 = 0.44;

/// Tile plane, just above the annex deck.
const TILE_Y: f32 = 0.012;

/// The beacon's answer to a sweep, seconds: feedback, inside the
/// half-second law.
const CYCLE_LEN: f32 = 0.45;

/// The four tile centres, slot-indexed like `layout::FLOTSAM_SLOTS`:
/// near pair first, row-major from the doorway looking outboard. The
/// grid crowds toward the door — every tile must sit inside `REACH`
/// from the walk envelope (rig's workability test holds it there), so
/// the slack in the chamber pools at the outer hatch, and a big crate
/// on a near tile pokes into the doorway: "just about fits" on show.
fn tile_center(slot: u8) -> Vec3 {
    let x = if slot.is_multiple_of(2) {
        TILE.mul_add(0.5, AIR_X0 + 0.02)
    } else {
        TILE.mul_add(1.5, AIR_X0 + 0.06)
    };
    let z = if slot / 2 == 0 {
        TILE.mul_add(0.5, AIR_DOOR_Z0 + 0.15)
    } else {
        TILE.mul_add(-0.5, AIR_DOOR_Z1 - 0.15)
    };
    Vec3::new(x, TILE_Y, z)
}

/// Where a rail piece stands: its tile's centre, facing the doorway.
/// The scale math lives with the other berths in `pieces::berth_site`;
/// this module only owns the room.
#[must_use]
pub fn site(slot: u8) -> (Vec3, Quat) {
    (
        tile_center(slot),
        Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
    )
}

/// The warning beacon over the doorway (cabin side): dark glass while
/// the chamber is empty, breathing amber while cargo is staged, a hard
/// red strobe for the moment the lock cycles.
#[derive(Component)]
struct Beacon {
    mat: Handle<StandardMaterial>,
}

/// The one latched clock: seconds left of the cycle strobe.
#[derive(Resource, Default)]
struct AirlockFx {
    cycle: f32,
}

pub struct AirlockPlugin;

impl Plugin for AirlockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AirlockFx>()
            .add_systems(PostStartup, spawn)
            .add_systems(Update, sync.in_set(Phase::View));
    }
}

/// Build the chamber dressing: berth tiles with hazard borders, their
/// slot-bound surfaces, the doorway's hazard jambs, the outer hatch,
/// and the beacon. The walls themselves come from `rig::structure`.
fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    skin: Res<Skin>,
) {
    // Berth tiles: a hazard-striped ground with a socket-dark centre —
    // patterned floor that says "this square goes outside".
    let border = meshes.add(Cuboid::new(TILE, 0.006, TILE));
    let well = meshes.add(Cuboid::new(TILE - 0.09, 0.007, TILE - 0.09));
    for slot in 0..layout::FLOTSAM_SLOTS.len() as u8 {
        let at = tile_center(slot);
        commands.spawn((
            Mesh3d(border.clone()),
            MeshMaterial3d(skin.hazard.clone()),
            Transform::from_translation(at),
        ));
        commands.spawn((
            Mesh3d(well.clone()),
            MeshMaterial3d(skin.socket.clone()),
            Transform::from_translation(at + Vec3::Y * 0.002),
        ));
        // One surface per tile, bound to its own rail slot's rect: the
        // crosshair ray lands anywhere on the tile and the sim hears a
        // pointer inside that slot. Berth transitions stay the sim's.
        let rect = layout::FLOTSAM_SLOTS[usize::from(slot)];
        commands.spawn((
            Station::Airlock,
            SimSurface {
                center: at + Vec3::Y * 0.004,
                half_u: Vec3::NEG_X * (TILE * 0.5),
                half_v: Vec3::NEG_Z * (TILE * 0.5),
                rect,
            },
        ));
    }
    // Doorway jambs and header: hazard paint where the room ends.
    let jamb = meshes.add(Cuboid::new(0.012, AIR_DOOR_H, 0.05));
    for z in [AIR_DOOR_Z0 - 0.025, AIR_DOOR_Z1 + 0.025] {
        commands.spawn((
            Mesh3d(jamb.clone()),
            MeshMaterial3d(skin.hazard.clone()),
            Transform::from_translation(Vec3::new(1.664, AIR_DOOR_H * 0.5, z)),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.012, 0.05, AIR_DOOR_Z1 - AIR_DOOR_Z0 + 0.1))),
        MeshMaterial3d(skin.hazard.clone()),
        Transform::from_translation(Vec3::new(
            1.664,
            AIR_DOOR_H + 0.025,
            f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1),
        )),
    ));
    // The outer hatch on the far wall: a dark dogged disk with a hazard
    // ring — the side of the door nobody opens from in here, yet.
    let mid_z = f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.34, 0.02))),
        MeshMaterial3d(skin.hazard.clone()),
        Transform::from_xyz(AIR_X1 - 0.008, 0.95, mid_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.30, 0.03))),
        MeshMaterial3d(skin.glass.clone()),
        Transform::from_xyz(AIR_X1 - 0.012, 0.95, mid_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    ));
    // The beacon: housing on the cabin face over the door, its lamp an
    // own-instance glass the sync system drives.
    let lamp = glow::phosphor(&mut materials, palette::AMBER, 0.0);
    commands.spawn((
        Mesh3d(skin.cube.clone()),
        MeshMaterial3d(skin.plate_shade.clone()),
        Transform::from_xyz(1.655, AIR_DOOR_H + 0.14, mid_z)
            .with_scale(Vec3::new(0.03, 0.10, 0.22)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.045, 0.02))),
        MeshMaterial3d(lamp.clone()),
        Transform::from_xyz(1.638, AIR_DOOR_H + 0.14, mid_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Beacon { mat: lamp },
    ));
}

/// Drive the beacon from the sim: latch the sweep cue, then read the
/// rail. Staged cargo breathes amber ("the next port call takes this");
/// the cycle itself is a hard red strobe, done inside half a second.
fn sync(
    time: Res<Time>,
    shell: Res<Shell>,
    mut fx: ResMut<AirlockFx>,
    beacons: Query<&Beacon>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    fx.cycle = (fx.cycle - time.delta_secs()).max(0.0);
    if shell
        .bridge
        .sim
        .cues()
        .iter()
        .any(|cue| matches!(cue, Cue::Jettison))
    {
        fx.cycle = CYCLE_LEN;
    }
    let staged = shell
        .bridge
        .sim
        .pieces()
        .iter()
        .any(|piece| matches!(piece.loc, Loc::Flotsam { .. }));
    let t = time.elapsed_secs();
    for beacon in &beacons {
        if let Some(mut mat) = materials.get_mut(&beacon.mat) {
            if fx.cycle > 0.0 {
                // Hard on/off at strobe cadence: motion and brightness
                // carry the alarm, not hue alone.
                let on = (t * 22.0).sin() > 0.0;
                glow::set_lamp(&mut mat, palette::LAMP_NO, if on { 1.0 } else { 0.1 });
            } else if staged {
                glow::set_lamp(
                    &mut mat,
                    palette::AMBER,
                    glow::breathe(t, 2.2, 0.0).mul_add(0.35, 0.45),
                );
            } else {
                glow::set_lamp(&mut mat, palette::AMBER, 0.0);
            }
        }
    }
}
