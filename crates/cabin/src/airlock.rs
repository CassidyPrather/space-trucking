//! The burner: where cargo goes to push the ship.
//!
//! The incinerator is a **room** now (docs/ROOMS.md): an ordinary room
//! attached at the cabin's starboard door, its whole net hazard-class
//! `Consume` tiles, riding with the ship. This annex (`rig`'s `AIR_*`
//! constants carve the doorway and the chamber) is that room's stage-one
//! presentation: four hazard-bordered berth tiles, each bound through its
//! own [`SimSurface`] to one cell of the furnace room's own deck.
//! Drop cargo on a tile to stage it, snatch it back any time before the
//! shovel; underway, on the stoker's slow beat, the lowest occupied cell
//! goes into the firebox behind the outer hatch (`Cue::Burn`) — the hatch
//! glass flares with the feeding, the banked fire breathes ember behind
//! it while the stoke lasts, and the window's star-streaks stretch to
//! match the extra way.
//!
//! Fuel simply stays staged: docking has no reason to bank it, so the
//! `Cue::Jettison` strobe retired with the one ceremony that discarded.
//! The exclusivity hack retired with it — there is no shelf row to share,
//! so the tiles are live always. The chamber is sized so the largest
//! footprint in the game JUST fits, proven by `rig`'s tests.
//!
//! Stage two draws the furnace room as the room it is; until then only
//! [`HOPPER_CELLS`] of its deck have a face, and cargo set anywhere else
//! in it is legal, burnable, and invisible.

use bevy::prelude::*;

use space_trucking::sim::room::RoomId;
use space_trucking::sim::{Cue, Loc, STOKE_PER_FLAM, layout};

use crate::rig::{AIR_DOOR_H, AIR_DOOR_Z0, AIR_DOOR_Z1, AIR_X0, AIR_X1, Skin};
use crate::surface::{SimSurface, Station};
use crate::{Phase, Shell, glow, palette};

/// One berth tile's edge, world units: four fit the chamber two-by-two
/// with walking seams, and a piece bigger than its tile simply crams —
/// the chamber, not the tile, is the fit guarantee.
const TILE: f32 = 0.44;

/// Tile plane, just above the annex deck.
const TILE_Y: f32 = 0.012;

/// The beacon's answer to a room parting, seconds: feedback, inside the
/// half-second law.
const CYCLE_LEN: f32 = 0.45;

/// The firebox's answer to a feeding, seconds: a flare that decays back
/// into the banked-ember glow. A touch over the half-second law because
/// its tail hides inside the standing glow rather than ending on black.
const FLASH_LEN: f32 = 0.6;

/// Banked boost that reads as a full firebox, in stoke ticks: one
/// feeding at the flammability scale's top. More keeps burning longer,
/// but the glass has nowhere brighter to go.
const FIREBOX_FULL: u64 = 3 * STOKE_PER_FLAM;

/// The furnace room's dense id. It is attached at the yard and rides
/// with the ship, so it holds room 1 for the life of a run — and if the
/// crew ever sells the furnace, its tiles simply stop resolving.
pub const BURNER_ROOM: RoomId = 1;

/// The furnace-room cells this annex gives a face to, in tile order:
/// the near pair of its deck, then the pair behind them. The room's own
/// grid is larger; this is the stage-one window onto it.
pub const HOPPER_CELLS: [(u8, u8); 4] = [(3, 3), (4, 3), (3, 4), (4, 4)];

/// Which tile a furnace-room cell shows on, if any.
#[must_use]
pub fn hopper_slot(x: u8, y: u8) -> Option<u8> {
    HOPPER_CELLS
        .iter()
        .position(|&cell| cell == (x, y))
        .map(|slot| slot as u8)
}

/// The four tile centres, slot-indexed like [`HOPPER_CELLS`]:
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
/// the hopper is empty, breathing amber while fuel is staged, a hard
/// red strobe when a seam parts.
#[derive(Component)]
struct Beacon {
    mat: Handle<StandardMaterial>,
}

/// The firebox glass in the outer hatch: dark while the fire is out,
/// breathing ember in proportion to the banked stoke, flaring when the
/// stoker feeds it.
#[derive(Component)]
struct Firebox {
    mat: Handle<StandardMaterial>,
}

/// The latched clocks: the parting strobe, and the feeding flare with
/// the strength the last feeding bought.
#[derive(Resource, Default)]
struct AirlockFx {
    cycle: f32,
    flash: f32,
    /// Flare amplitude, from `Cue::Burn`'s intensity: slag flickers,
    /// upholstery lights the room.
    fed: f32,
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
    for slot in 0..HOPPER_CELLS.len() as u8 {
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
        // One surface per tile, bound to its own furnace-room cell: the
        // crosshair ray lands anywhere on the tile and the sim hears a
        // pointer inside that cell. Berth transitions stay the sim's.
        let (cx, cy) = HOPPER_CELLS[usize::from(slot)];
        let rect = layout::cell_rect(BURNER_ROOM, cx, cy);
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
    // The firebox door on the far wall: a dark dogged disk with a
    // hazard ring, its round glass the one place the fire shows. The
    // sync system writes the banked stoke onto the glass every frame.
    let mid_z = f32::midpoint(AIR_DOOR_Z0, AIR_DOOR_Z1);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.34, 0.02))),
        MeshMaterial3d(skin.hazard.clone()),
        Transform::from_xyz(AIR_X1 - 0.008, 0.95, mid_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    ));
    let fire = glow::phosphor(&mut materials, palette::EMBER, 0.0);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.30, 0.03))),
        MeshMaterial3d(fire.clone()),
        Transform::from_xyz(AIR_X1 - 0.012, 0.95, mid_z)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Firebox { mat: fire },
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

/// Drive the room from the sim: latch the cues, then read the rail and
/// the banked stoke. Staged fuel breathes amber on the beacon ("the
/// stoker's next beat takes this"); a feeding flares the firebox glass,
/// whose standing ember tracks the banked fire; a dock-overflow tip is
/// the beacon's hard red strobe, done inside half a second.
fn sync(
    time: Res<Time>,
    shell: Res<Shell>,
    mut fx: ResMut<AirlockFx>,
    beacons: Query<&Beacon>,
    fireboxes: Query<&Firebox>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    fx.cycle = (fx.cycle - dt).max(0.0);
    fx.flash = (fx.flash - dt).max(0.0);
    for cue in shell.bridge.sim.cues() {
        match cue {
            Cue::Parted => fx.cycle = CYCLE_LEN,
            Cue::Burn { intensity } => {
                fx.flash = FLASH_LEN;
                // Even slag (intensity 0) shows its disposal; real fuel
                // scales up from there.
                fx.fed = intensity.mul_add(0.7, 0.3);
            }
            _ => {}
        }
    }
    let staged = shell
        .bridge
        .sim
        .pieces()
        .iter()
        .any(|piece| matches!(piece.loc, Loc::Hold { room, .. } if room == BURNER_ROOM));
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
    // The firebox glass: the banked stoke sets the standing ember — the
    // same number the sim spends on extra way, normalized to one full
    // feeding — and a feeding's flare decays squared over it, fire-like.
    let heat = (shell.bridge.sim.stoke().min(FIREBOX_FULL) as f32) / (FIREBOX_FULL as f32);
    let flare = (fx.flash / FLASH_LEN).powi(2) * fx.fed;
    for firebox in &fireboxes {
        if let Some(mut mat) = materials.get_mut(&firebox.mat) {
            let ember = heat * glow::breathe(t, 3.4, 0.0).mul_add(0.30, 0.50);
            glow::set_lamp(&mut mat, palette::EMBER, (ember + flare).min(1.0));
        }
    }
}
