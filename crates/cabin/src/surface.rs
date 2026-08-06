//! Sim surfaces: the trick that makes the 3D cabin the same game.
//!
//! The sim's whole interaction model is a pointer in its 800×600 console
//! world (`sim::layout`). Each interactive surface in the cabin — the nav
//! tank's glass, the console face, the barter counter, the bay's wall
//! band and deck strip — is a [`SimSurface`]: an oriented quad in 3D
//! space bound to one sim rect.
//! Each frame the cursor ray is cast against every surface; the nearest
//! hit maps to sim coordinates and becomes the virtual pointer the sim
//! reads. The inverse mapping places sim things (POIs, crates, the rat)
//! back onto cabin geometry. Hit-testing thus stays where it always was:
//! inside the sim, where the rules live.

use bevy::prelude::*;
use space_trucking::sim::Vec2 as SimVec2;
use space_trucking::sim::layout::Rect as SimRect;

/// Which mapped surface (or view system) is being talked about.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Station {
    /// The star map tank — `layout::MAP_PANEL`.
    Map,
    /// The console face: preview, ETA, launch lever, icon buttons.
    Console,
    /// The barter counter — slots, dial, accept lever, badge.
    Barter,
    /// The bay's aft wall band — hold grid rows 0–2, unfolded upright.
    BayWall,
    /// The bay's deck strip — hold grid row 3, folded flat.
    BayFloor,
}

impl Station {
    /// Whether this surface is part of the walkable bay — worked from
    /// roam with the crosshair rather than from a focus pose.
    #[must_use]
    pub const fn in_bay(self) -> bool {
        matches!(self, Self::BayWall | Self::BayFloor)
    }
}

/// An oriented quad bound to a sim rect. `half_u` spans sim +x (half the
/// panel's width), `half_v` spans sim +y — which is *down* in the sim's
/// world, so `half_v` points down-panel in 3D as well.
#[derive(Component, Clone, Copy, Debug)]
pub struct SimSurface {
    pub center: Vec3,
    pub half_u: Vec3,
    pub half_v: Vec3,
    pub rect: SimRect,
}

impl SimSurface {
    /// A panel standing near-vertical, facing +Z (toward the seat), tilted
    /// `tilt` radians about X: positive tilt reclines the top away and
    /// raises the face toward the viewer — desk-like at large angles.
    #[must_use]
    pub fn panel(center: Vec3, width: f32, height: f32, tilt: f32, rect: SimRect) -> Self {
        Self::panel_yawed(center, width, height, tilt, 0.0, rect)
    }

    /// [`Self::panel`] rotated `yaw` radians about Y afterwards, for
    /// panels mounted on other walls: `FRAC_PI_2` faces +X (the left
    /// wall's inward normal), `-FRAC_PI_2` faces -X.
    #[must_use]
    pub fn panel_yawed(
        center: Vec3,
        width: f32,
        height: f32,
        tilt: f32,
        yaw: f32,
        rect: SimRect,
    ) -> Self {
        let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-tilt);
        Self {
            center,
            half_u: rot * (Vec3::X * (width * 0.5)),
            half_v: rot * (Vec3::NEG_Y * (height * 0.5)),
            rect,
        }
    }

    /// The quad's outward normal.
    #[must_use]
    pub fn normal(&self) -> Vec3 {
        self.half_v.cross(self.half_u).normalize()
    }

    /// The rotation carrying local `+X`/`+Y`/`+Z` onto the panel's
    /// u / up-panel / normal frame — what furniture meshes orient by.
    /// (Sim v runs *down* the panel, so panel-up is `-v̂`.)
    #[must_use]
    pub fn orientation(&self) -> Quat {
        let u = self.half_u.normalize();
        let v = self.half_v.normalize();
        Quat::from_mat3(&Mat3::from_cols(u, -v, self.normal()))
    }

    /// Ray → (distance, sim position, world position), if the ray crosses
    /// the quad within its bounds.
    #[must_use]
    pub fn project(&self, ray: Ray3d) -> Option<(f32, SimVec2, Vec3)> {
        let normal = self.normal();
        let denom = ray.direction.dot(normal);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = (self.center - ray.origin).dot(normal) / denom;
        if t <= 0.0 {
            return None;
        }
        let world = ray.origin + ray.direction * t;
        let local = world - self.center;
        let a = local.dot(self.half_u) / self.half_u.length_squared();
        let b = local.dot(self.half_v) / self.half_v.length_squared();
        if !(-1.0..=1.0).contains(&a) || !(-1.0..=1.0).contains(&b) {
            return None;
        }
        let sim = SimVec2::new(
            ((a + 1.0) * 0.5).mul_add(self.rect.w, self.rect.x),
            ((b + 1.0) * 0.5).mul_add(self.rect.h, self.rect.y),
        );
        Some((t, sim, world))
    }

    /// Sim position → world position on the quad's plane. Positions
    /// outside the bound rect extrapolate — callers clamp if they care.
    #[must_use]
    pub fn to_world(self, sim: SimVec2) -> Vec3 {
        let a = ((sim.x - self.rect.x) / self.rect.w).mul_add(2.0, -1.0);
        let b = ((sim.y - self.rect.y) / self.rect.h).mul_add(2.0, -1.0);
        self.center + self.half_u * a + self.half_v * b
    }

    /// World length of one sim unit along the panel's u axis.
    #[must_use]
    pub fn scale_u(&self) -> f32 {
        self.half_u.length() * 2.0 / self.rect.w
    }

    /// World length of one sim unit along the panel's v axis.
    #[must_use]
    pub fn scale_v(&self) -> f32 {
        self.half_v.length() * 2.0 / self.rect.h
    }
}

/// The virtual pointer: where the cursor ray landed in sim terms this
/// frame, plus the 3D point for carrying pieces along the ray.
#[derive(Resource, Clone, Copy, Debug)]
pub struct VirtualPointer {
    /// Sim-space pointer; [`crate::bridge::POINTER_PARKED`] when the
    /// cursor touches nothing mapped.
    pub sim: SimVec2,
    /// The 3D hit point, if any surface was struck.
    pub world: Option<Vec3>,
    /// The station struck, for views that care where attention rests.
    pub station: Option<Station>,
}

impl Default for VirtualPointer {
    fn default() -> Self {
        Self {
            sim: crate::bridge::POINTER_PARKED,
            world: None,
            station: None,
        }
    }
}

/// Cast this frame's pointer ray onto the mapped surfaces; the nearest
/// hit becomes the virtual pointer. Two regimes, one mapping:
///
/// - **Focused**: the freed cursor ray, against every surface — precise
///   panel work, exactly as in the 2D console.
/// - **Roaming**: the crosshair ray straight out of the camera, against
///   the bay surfaces only, and only within [`crate::rig::REACH`] — the
///   carry's aim. Stations need focus; the bay needs proximity.
#[allow(clippy::needless_pass_by_value)]
pub fn track_pointer(
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::rig::CabinCamera>>,
    surfaces: Query<(&Station, &SimSurface)>,
    rig: Res<crate::rig::CameraRig>,
    mut pointer: ResMut<VirtualPointer>,
) {
    *pointer = VirtualPointer::default();
    let (camera, camera_tf) = *camera;
    let (ray, bay_only, reach) = if rig.interactive() {
        let Some(cursor) = window.cursor_position() else {
            return;
        };
        // The camera renders into the crunch target, so its viewport
        // speaks crunch pixels; rescale the window cursor into that space.
        let size = window.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let scaled =
            cursor / size * Vec2::new(crate::rig::CRUNCH_W as f32, crate::rig::CRUNCH_H as f32);
        let Ok(ray) = camera.viewport_to_world(camera_tf, scaled) else {
            return;
        };
        (ray, false, f32::INFINITY)
    } else if rig.roaming() {
        let forward = camera_tf.forward();
        let Ok(dir) = Dir3::new(forward.into()) else {
            return;
        };
        (
            Ray3d::new(camera_tf.translation(), dir),
            true,
            crate::rig::REACH,
        )
    } else {
        // Glides and parked cursors keep the pointer parked.
        return;
    };
    let mut nearest = f32::INFINITY;
    for (station, surface) in &surfaces {
        if bay_only && !station.in_bay() {
            continue;
        }
        if let Some((t, sim, world)) = surface.project(ray)
            && t < nearest
            && t <= reach
        {
            nearest = t;
            *pointer = VirtualPointer {
                sim,
                world: Some(world),
                station: Some(*station),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use space_trucking::sim::layout;

    fn map_panel() -> SimSurface {
        SimSurface::panel(Vec3::new(0.0, 1.5, -1.0), 1.0, 0.84, 0.0, layout::MAP_PANEL)
    }

    #[test]
    fn round_trip_center_and_corners() {
        let s = map_panel();
        let r = s.rect;
        for (sx, sy) in [
            (r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y)),
            (r.x, r.y),
            (r.x + r.w, r.y + r.h),
            (r.w.mul_add(0.25, r.x), r.h.mul_add(0.75, r.y)),
        ] {
            let world = s.to_world(SimVec2::new(sx, sy));
            // Cast from straight in front of the found point.
            let ray = Ray3d::new(world + Vec3::Z, Dir3::NEG_Z);
            let (_, sim, hit) = s.project(ray).expect("hit");
            assert!((sim.x - sx).abs() < 1e-3, "x {} vs {}", sim.x, sx);
            assert!((sim.y - sy).abs() < 1e-3, "y {} vs {}", sim.y, sy);
            assert!((hit - world).length() < 1e-4);
        }
    }

    #[test]
    fn misses_outside_the_quad() {
        let s = map_panel();
        let ray = Ray3d::new(Vec3::new(2.0, 1.5, 0.0), Dir3::NEG_Z);
        assert!(s.project(ray).is_none());
    }

    #[test]
    fn tilt_preserves_the_mapping() {
        let s = SimSurface::panel(
            Vec3::new(-0.6, 0.9, -0.9),
            0.78,
            0.52,
            55f32.to_radians(),
            layout::Rect::new(
                layout::GRID_ORIGIN.x,
                layout::GRID_ORIGIN.y,
                f32::from(layout::GRID_COLS) * layout::CELL,
                f32::from(layout::GRID_ROWS) * layout::CELL,
            ),
        );
        // The center of hold cell (2, 1) should round-trip through a ray
        // fired along the panel normal.
        let cell = layout::cell_rect(2, 1);
        let target = SimVec2::new(cell.w.mul_add(0.5, cell.x), cell.h.mul_add(0.5, cell.y));
        let world = s.to_world(target);
        let n = s.normal();
        let ray = Ray3d::new(world + n * 0.5, Dir3::new(-n).expect("unit"));
        let (_, sim, _) = s.project(ray).expect("hit");
        assert!(layout::cell_at(sim) == Some((2, 1)), "landed at {sim:?}");
        // And the normal faces the +Z hemisphere (toward the seat).
        assert!(n.z > 0.3, "normal {n:?} should face the seat");
    }

    #[test]
    fn sim_y_runs_down_the_panel() {
        let s = map_panel();
        let top = s.to_world(SimVec2::new(260.0, s.rect.y));
        let bottom = s.to_world(SimVec2::new(260.0, s.rect.y + s.rect.h));
        assert!(
            top.y > bottom.y,
            "sim +y must map downward: top {top:?} bottom {bottom:?}"
        );
    }
}
