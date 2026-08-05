//! Tactile levers: the two ceremony controls — launch and accept — stop
//! being buttons and become pulls. Press the handle, drag it along its
//! track, and let go past the threshold to throw it; let go early and it
//! springs back, nothing said. The sim's contract is untouched: it still
//! receives one plain press inside the lever's rect, synthesized on the
//! frame the pull completes, so tapes stay honest and the 2D console
//! could replay a cabin session none the wiser.
//!
//! Grip capture only happens with empty hands — while cargo is held, all
//! pointer input passes straight through to the sim's drag logic, and a
//! sloppy drag crossing a lever rect costs nothing.

use bevy::prelude::*;
use space_trucking::sim::layout::Rect as SimRect;
use space_trucking::sim::{Vec2 as SimVec2, layout};

use crate::rig::CameraRig;
use crate::surface::VirtualPointer;
use crate::{Phase, Shell};

/// Fraction of the track a pull must cover before release fires it.
const THROW: f32 = 0.7;

/// How much of the lever rect's width is a full pull, in sim units.
const TRACK_FRACTION: f32 = 0.55;

/// Spring-back rate once released, in travel per second.
const SPRING: f32 = 6.0;

/// One lever's grip state.
#[derive(Default, Clone, Copy)]
pub struct Grip {
    /// Visible handle throw, `0..=1` — the view modules read this.
    pub travel: f32,
    /// Where the pull started, while held.
    hold_from: Option<f32>,
    /// The pull completed this frame; `advance` consumes it.
    fired: bool,
}

impl Grip {
    fn update(
        &mut self,
        rect: SimRect,
        pointer: SimVec2,
        press: bool,
        held: bool,
        release: bool,
        dt: f32,
    ) {
        self.fired = false;
        match self.hold_from {
            None => {
                if press && rect.contains(pointer) {
                    self.hold_from = Some(pointer.x);
                }
                self.travel = SPRING.mul_add(-dt, self.travel).max(0.0);
            }
            Some(from) => {
                let track = rect.w * TRACK_FRACTION;
                self.travel = ((pointer.x - from) / track).clamp(0.0, 1.0);
                if release {
                    self.fired = self.travel >= THROW;
                    self.hold_from = None;
                } else if !held || !rect.contains(pointer) {
                    // Lost the handle (left the rect, left focus, lost the
                    // button): spring back, no ceremony.
                    self.hold_from = None;
                }
            }
        }
    }

    /// Whether a pull is in progress. Exercised by the tests; view
    /// modules may want it for grip-state styling later.
    #[allow(dead_code)]
    #[must_use]
    pub const fn gripped(&self) -> bool {
        self.hold_from.is_some()
    }
}

/// Both levers' grips, read by the view modules for handle positions and
/// by `advance` for input synthesis.
#[derive(Resource, Default)]
pub struct Grips {
    pub launch: Grip,
    pub accept: Grip,
}

impl Grips {
    /// Whether raw pointer input over `at` should be withheld from the
    /// sim this frame: empty-handed presses on lever rects belong to the
    /// gesture layer, which answers with a synthesized press on
    /// completion. With cargo in hand everything passes through — drops
    /// and drags are the sim's business.
    #[must_use]
    pub const fn masks(at: SimVec2, holding: bool) -> bool {
        !holding && (layout::LAUNCH_LEVER.contains(at) || layout::ACCEPT_LEVER.contains(at))
    }

    /// The synthesized press for this frame, if a pull just completed:
    /// a plain press at the lever's center, exactly what the 2D console
    /// would have sent.
    #[must_use]
    pub fn fired_press(&self) -> Option<SimVec2> {
        let center = |r: SimRect| SimVec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y));
        if self.launch.fired {
            Some(center(layout::LAUNCH_LEVER))
        } else if self.accept.fired {
            Some(center(layout::ACCEPT_LEVER))
        } else {
            None
        }
    }
}

/// Track both grips from this frame's pointer. Runs in `Phase::Input`
/// after the pointer, before `advance`.
pub fn grip(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<VirtualPointer>,
    rig: Res<CameraRig>,
    shell: Res<Shell>,
    mut grips: ResMut<Grips>,
) {
    let live = rig.interactive();
    let holding = shell.bridge.sim.held(0).is_some();
    let press = live && !holding && buttons.just_pressed(MouseButton::Left);
    let held = live && buttons.pressed(MouseButton::Left);
    let release = live && buttons.just_released(MouseButton::Left);
    let dt = time.delta_secs();
    grips
        .launch
        .update(layout::LAUNCH_LEVER, pointer.sim, press, held, release, dt);
    grips
        .accept
        .update(layout::ACCEPT_LEVER, pointer.sim, press, held, release, dt);
}

pub struct GesturePlugin;

impl Plugin for GesturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Grips>().add_systems(
            Update,
            grip.in_set(Phase::Input)
                .after(crate::surface::track_pointer),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mid(r: SimRect) -> SimVec2 {
        SimVec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y))
    }

    #[test]
    fn a_full_pull_fires_once() {
        let r = layout::LAUNCH_LEVER;
        let mut grip = Grip::default();
        let start = SimVec2::new(r.x + 10.0, r.y + 20.0);
        grip.update(r, start, true, true, false, 0.016);
        assert!(grip.gripped());
        // Drag right past the throw threshold, then release.
        let pulled = SimVec2::new(r.w.mul_add(TRACK_FRACTION, start.x), start.y);
        grip.update(r, pulled, false, true, false, 0.016);
        assert!(grip.travel >= THROW);
        grip.update(r, pulled, false, false, true, 0.016);
        assert!(grip.fired);
        // The next frame it is spent.
        grip.update(r, pulled, false, false, false, 0.016);
        assert!(!grip.fired && !grip.gripped());
    }

    #[test]
    fn a_timid_pull_springs_back_silently() {
        let r = layout::ACCEPT_LEVER;
        let mut grip = Grip::default();
        let start = mid(r);
        grip.update(r, start, true, true, false, 0.016);
        let nudged = SimVec2::new(start.x + 6.0, start.y);
        grip.update(r, nudged, false, true, false, 0.016);
        assert!(grip.travel < THROW);
        grip.update(r, nudged, false, false, true, 0.016);
        assert!(!grip.fired);
        // Travel decays toward rest.
        let was = grip.travel;
        grip.update(r, nudged, false, false, false, 0.1);
        assert!(grip.travel < was || grip.travel == 0.0);
    }

    #[test]
    fn leaving_the_rect_drops_the_handle() {
        let r = layout::LAUNCH_LEVER;
        let mut grip = Grip::default();
        grip.update(r, mid(r), true, true, false, 0.016);
        assert!(grip.gripped());
        grip.update(r, SimVec2::new(r.x - 50.0, r.y), false, true, false, 0.016);
        assert!(!grip.gripped());
        grip.update(r, mid(r), false, false, true, 0.016);
        assert!(
            !grip.fired,
            "a release after losing the handle is not a pull"
        );
    }

    #[test]
    fn masking_spares_cargo_drags() {
        let over_lever = mid(layout::LAUNCH_LEVER);
        assert!(Grips::masks(over_lever, false));
        assert!(!Grips::masks(over_lever, true), "held cargo passes through");
        assert!(!Grips::masks(SimVec2::new(100.0, 470.0), false));
    }
}
