//! The burner: where cargo goes to push the ship.
//!
//! The incinerator is a **room** (docs/ROOMS.md): an ordinary room
//! attached at the cabin's starboard door, its whole net hazard-class
//! `Consume` tiles, riding with the ship. Stage two draws it as the room
//! it is — `crate::room` folds its net onto its own lattice box, punches
//! its doorway out of the cabin's wall, and paints its ember-edged deck —
//! so the hand-measured annex, its four rail tiles, and the `AIR_*`
//! constants that sized them all retired together. Cargo staged in the
//! furnace stands on its floor exactly as cargo stands on the cabin's,
//! through the same rigs, because it is the same rule one room over.
//!
//! What is left here is the furnace's own **flavour**, and it hangs on
//! the room's derived geometry rather than on numbers of its own: the
//! firebox glass in the outer wall, whose banked ember tracks the stoke
//! the sim spends on extra way and flares when the stoker feeds it
//! (`Cue::Burn`), and the warning beacon over the doorway, which breathes
//! amber while fuel is staged and strobes when a seam parts.
//!
//! Fuel simply stays staged: docking has no reason to bank it, so the
//! `Cue::Jettison` strobe retired with the one ceremony that discarded.

use bevy::prelude::*;

use space_trucking::sim::room::RoomKind;
use space_trucking::sim::{Cue, Loc, STOKE_PER_FLAM};

use crate::room::{InRoom, Placed};
use crate::surface::Station;
use crate::{Phase, Shell, glow, palette};

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

/// The warning beacon over the doorway (cabin side): dark glass while
/// the hopper is empty, breathing amber while fuel is staged, a hard
/// red strobe when a seam parts.
#[derive(Component)]
struct Beacon {
    mat: Handle<StandardMaterial>,
}

/// The firebox glass in the outer wall: dark while the fire is out,
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
            .add_systems(Update, sync.in_set(Phase::View));
    }
}

/// The furnace room's own fittings, hung on its derived box: the firebox
/// door in the outer wall and the beacon over the doorway. Called by
/// `room::rebuild` while it builds the burner, so the fire arrives and
/// leaves with the room it belongs to — sell the furnace and its glass
/// goes with it.
pub fn fittings(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    skin: &crate::rig::Skin,
    placed: &Placed,
    tag: InRoom,
) {
    if placed.kind != RoomKind::Burner {
        return;
    }
    // The firebox lives in the outer wall — the one opposite the door the
    // room came in by — and its plane is that chart's, not a number.
    let Some(outer) = placed.chart(Station::BayStarboard) else {
        return;
    };
    let inward = Station::BayStarboard.inward(outer);
    let hearth = outer.center - inward * 0.02;
    let face = Quat::from_rotation_arc(Vec3::Z, inward);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.30, 0.02))),
        MeshMaterial3d(skin.hazard.clone()),
        Transform::from_translation(hearth + inward * 0.02)
            .with_rotation(face * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        tag,
    ));
    let fire = glow::phosphor(materials, palette::EMBER, 0.0);
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.26, 0.03))),
        MeshMaterial3d(fire.clone()),
        Transform::from_translation(hearth + inward * 0.035)
            .with_rotation(face * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        Firebox { mat: fire },
        tag,
    ));
    // The beacon: a housing hung just under the doorway's lintel, INSIDE
    // the furnace — the room whose business it reports — and low enough
    // in the opening to be read from the cabin as well. It used to lean
    // across the seam and sit above the lintel on the cabin's side,
    // which is a room reaching into a room it does not own; the eye that
    // stages fuel walks under it either way.
    let Some(door) = placed.ports.iter().find(|site| site.mate.is_some()) else {
        return;
    };
    let over = door.leaf - door.out * 0.03 + Vec3::Y * (door.half_b.length() - 0.08);
    let dir_a = door.half_a.normalize_or_zero().abs();
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(skin.plate_shade.clone()),
        Transform::from_translation(over)
            .with_scale(door.out.abs() * 0.03 + dir_a * 0.22 + Vec3::Y * 0.10),
        tag,
    ));
    let lamp = glow::phosphor(materials, palette::AMBER, 0.0);
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(lamp.clone()),
        Transform::from_translation(over - door.out * 0.025)
            .with_scale(door.out.abs() * 0.02 + dir_a * 0.14 + Vec3::Y * 0.06),
        Beacon { mat: lamp },
        tag,
    ));
}

/// Drive the room from the sim: latch the cues, then read the furnace's
/// own deck and the banked stoke. Staged fuel breathes amber on the
/// beacon ("the stoker's next beat takes this"); a feeding flares the
/// firebox glass, whose standing ember tracks the banked fire; a seam
/// parting is the beacon's hard red strobe, done inside half a second.
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
    // Which room the furnace is is the graph's answer, never a constant:
    // the burner takes whatever dense id the yard gave it, and if the
    // crew ever sells it there is simply no room to read.
    let sim = &shell.bridge.sim;
    let furnace = sim.rooms().find(RoomKind::Burner);
    let staged = furnace.is_some_and(|room| {
        sim.pieces()
            .iter()
            .any(|piece| matches!(piece.loc, Loc::Hold { room: at, .. } if at == room))
    });
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
    let heat = (sim.stoke().min(FIREBOX_FULL) as f32) / (FIREBOX_FULL as f32);
    let flare = (fx.flash / FLASH_LEN).powi(2) * fx.fed;
    for firebox in &fireboxes {
        if let Some(mut mat) = materials.get_mut(&firebox.mat) {
            let ember = heat * glow::breathe(t, 3.4, 0.0).mul_add(0.30, 0.50);
            glow::set_lamp(&mut mat, palette::EMBER, (ember + flare).min(1.0));
        }
    }
}
