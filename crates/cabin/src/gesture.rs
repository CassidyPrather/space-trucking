//! Tactile levers: the two ceremony controls — launch and accept — stop
//! being buttons and become pulls. Press the handle, drag it along its
//! track, and the throw fires **the moment it reaches the end** — no
//! release ceremony, the way a physical detent feels. Let go early and
//! it springs back, nothing said. Once gripped, only the button ending
//! (or the throw completing) ends the pull: wandering off the rect or
//! even off the panel mid-drag does not drop the handle.
//!
//! The sim's contract is untouched: it still receives one plain press
//! inside the lever's rect, synthesized on the frame the pull completes,
//! so tapes stay honest and the 2D console could replay a cabin session
//! none the wiser. [`synthesize`] is the one place raw input and the
//! gesture layer merge into the sim's view of the frame — `advance`
//! calls it, and the gesture monkey test batters it.
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
    /// The pull completed this frame; `synthesize` consumes it.
    fired: bool,
}

impl Grip {
    fn update(&mut self, rect: SimRect, pointer: SimVec2, press: bool, held: bool, dt: f32) {
        self.fired = false;
        match self.hold_from {
            None => {
                if press && rect.contains(pointer) {
                    self.hold_from = Some(pointer.x);
                    self.travel = 0.0;
                } else {
                    self.travel = SPRING.mul_add(-dt, self.travel).max(0.0);
                }
            }
            Some(from) => {
                if !held {
                    // Button gone (released early, or focus left): spring
                    // back, no ceremony.
                    self.hold_from = None;
                    return;
                }
                let track = rect.w * TRACK_FRACTION;
                self.travel = ((pointer.x - from) / track).clamp(0.0, 1.0);
                if self.travel >= 1.0 {
                    // The detent: reaching the end IS the throw.
                    self.fired = true;
                    self.hold_from = None;
                }
            }
        }
    }

    /// Whether a pull is in progress.
    #[must_use]
    pub const fn gripped(&self) -> bool {
        self.hold_from.is_some()
    }
}

/// Both levers' grips, read by the view modules for handle positions and
/// by [`synthesize`] for input synthesis.
#[derive(Resource, Default)]
pub struct Grips {
    pub launch: Grip,
    pub accept: Grip,
}

impl Grips {
    /// Whether raw pointer input over `at` should be withheld from the
    /// sim this frame: empty-handed input on lever rects (or any live
    /// grip) belongs to the gesture layer, which answers with a
    /// synthesized press on completion. With cargo in hand everything
    /// passes through — drops and drags are the sim's business.
    #[must_use]
    pub const fn masks(&self, at: SimVec2, holding: bool) -> bool {
        if holding {
            return false;
        }
        self.launch.gripped()
            || self.accept.gripped()
            || layout::LAUNCH_LEVER.contains(at)
            || layout::ACCEPT_LEVER.contains(at)
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

/// The frame's pointer input as the sim should see it: raw input merged
/// with the gesture layer. Returns `(pointer, press, held, release)`.
/// One function so `advance` and the gesture monkey cannot drift apart.
#[allow(clippy::fn_params_excessive_bools)] // mirrors InputFrame's own shape
#[must_use]
pub fn synthesize(
    grips: &Grips,
    pointer: SimVec2,
    holding: bool,
    live: bool,
    press: bool,
    held: bool,
    release: bool,
) -> (SimVec2, bool, bool, bool) {
    if let Some(at) = grips.fired_press() {
        return (at, true, true, false);
    }
    let raw = live && !grips.masks(pointer, holding);
    (pointer, raw && press, raw && held, raw && release)
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
    let dt = time.delta_secs();
    grips
        .launch
        .update(layout::LAUNCH_LEVER, pointer.sim, press, held, dt);
    grips
        .accept
        .update(layout::ACCEPT_LEVER, pointer.sim, press, held, dt);
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
    use space_trucking::sim::{Cue, InputFrame, ShipState, Sim, splitmix};

    fn mid(r: SimRect) -> SimVec2 {
        SimVec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y))
    }

    #[test]
    fn a_full_pull_fires_at_the_detent_without_release() {
        let r = layout::LAUNCH_LEVER;
        let mut grip = Grip::default();
        let start = SimVec2::new(r.x + 10.0, r.y + 20.0);
        grip.update(r, start, true, true, 0.016);
        assert!(grip.gripped());
        // Drag right to the end of the track — button still down.
        let pulled = SimVec2::new(r.w.mul_add(TRACK_FRACTION, start.x), start.y);
        grip.update(r, pulled, false, true, 0.016);
        assert!(grip.fired, "the detent fires, no release needed");
        // The next frame it is spent, even with the button still down.
        grip.update(r, pulled, false, true, 0.016);
        assert!(!grip.fired && !grip.gripped());
    }

    #[test]
    fn a_timid_pull_springs_back_silently() {
        let r = layout::ACCEPT_LEVER;
        let mut grip = Grip::default();
        let start = mid(r);
        grip.update(r, start, true, true, 0.016);
        let nudged = SimVec2::new(start.x + 6.0, start.y);
        grip.update(r, nudged, false, true, 0.016);
        assert!(!grip.fired);
        // Early release: sprung, silent.
        grip.update(r, nudged, false, false, 0.016);
        assert!(!grip.fired && !grip.gripped());
        let was = grip.travel;
        grip.update(r, nudged, false, false, 0.1);
        assert!(grip.travel < was || grip.travel == 0.0);
    }

    #[test]
    fn wandering_off_the_rect_keeps_the_grip() {
        let r = layout::LAUNCH_LEVER;
        let mut grip = Grip::default();
        let start = SimVec2::new(r.x + 8.0, r.y + 30.0);
        grip.update(r, start, true, true, 0.016);
        // Drift below the rect mid-pull — a sloppy but honest drag.
        let wandered = SimVec2::new(r.w.mul_add(0.3, start.x), r.y + r.h + 25.0);
        grip.update(r, wandered, false, true, 0.016);
        assert!(grip.gripped(), "the hand is on the handle, not the rect");
        // Finish the pull from out there.
        let done = SimVec2::new(r.w.mul_add(TRACK_FRACTION, start.x), r.y + r.h + 25.0);
        grip.update(r, done, false, true, 0.016);
        assert!(grip.fired);
    }

    #[test]
    fn masking_spares_cargo_drags() {
        let grips = Grips::default();
        let over_lever = mid(layout::LAUNCH_LEVER);
        assert!(grips.masks(over_lever, false));
        assert!(!grips.masks(over_lever, true), "held cargo passes through");
        assert!(!grips.masks(SimVec2::new(100.0, 470.0), false));
    }

    /// Arm a fresh sim with a chartable destination, the honest way.
    fn armed_sim(seed: u64) -> Sim {
        let mut sim = Sim::new(seed);
        let target = (0..12u8)
            .find(|&id| {
                !matches!(sim.ship().state, ShipState::Docked(at) if at == id)
                    && sim.poi_chartable(id)
            })
            .expect("some POI is chartable");
        let arm = InputFrame {
            pointer: sim.poi_pos(target),
            press: true,
            held: true,
            ..InputFrame::default()
        };
        sim.advance(1.0 / 60.0, &arm);
        assert_eq!(sim.ship().selected, Some(target));
        sim
    }

    /// A deliberate, scripted full pull must launch the ship — the direct
    /// harness for the "pulled it to the end and nothing happened" defect.
    #[test]
    fn a_deliberate_pull_departs() {
        let mut sim = armed_sim(11);
        let mut grips = Grips::default();
        let r = layout::LAUNCH_LEVER;
        let start = SimVec2::new(r.x + 12.0, r.h.mul_add(0.5, r.y));
        let steps = 30;
        let mut departed = false;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let pointer = SimVec2::new(
                r.w.mul_add(TRACK_FRACTION, 4.0).mul_add(t, start.x),
                start.y,
            );
            let press = i == 0;
            grips
                .launch
                .update(layout::LAUNCH_LEVER, pointer, press, true, 1.0 / 60.0);
            grips
                .accept
                .update(layout::ACCEPT_LEVER, pointer, press, true, 1.0 / 60.0);
            let (at, s_press, s_held, s_release) = synthesize(
                &grips,
                pointer,
                sim.held(0).is_some(),
                true,
                press,
                true,
                false,
            );
            let frame = InputFrame {
                pointer: at,
                press: s_press,
                held: s_held,
                release: s_release,
                ..InputFrame::default()
            };
            sim.advance(1.0 / 60.0, &frame);
            departed |= sim.cues().iter().any(|c| matches!(c, Cue::Depart));
        }
        assert!(departed, "a full deliberate pull must launch the ship");
    }

    /// The gesture monkey, per the 2D prototype's drag-monkey tradition:
    /// batter [`synthesize`] + the grips with seeded pseudo-random input
    /// against a real docked sim. The invariant is mask integrity, not
    /// pacifism — a chaotic pointer may legitimately complete a pull
    /// (fast real mice do too), but the sim must NEVER receive a press
    /// inside a lever rect except as a completed pull's synthesized
    /// press, and every departure must trace to one.
    #[test]
    fn gesture_monkey_mask_integrity() {
        let mut sim = armed_sim(11);
        let mut grips = Grips::default();
        let mut held_down = false;
        for i in 0..4000u64 {
            let h = splitmix(0xC0FF_EE00, i);
            let x = ((h & 0x3FF) as f32).mul_add(0.35, 500.0); // 500..858 sweeps the console
            let y = (((h >> 10) & 0x1FF) as f32).mul_add(0.25, 260.0); // 260..388
            let pointer = SimVec2::new(x, y);
            let press = !held_down && (h >> 20).trailing_zeros() >= 3;
            if press {
                held_down = true;
            } else if held_down && (h >> 23).trailing_zeros() >= 3 {
                held_down = false;
            }
            let held = held_down;
            let release = !held_down && (h >> 26) & 0x1 == 1;
            let holding = sim.held(0).is_some();
            grips
                .launch
                .update(layout::LAUNCH_LEVER, pointer, press, held, 1.0 / 60.0);
            grips
                .accept
                .update(layout::ACCEPT_LEVER, pointer, press, held, 1.0 / 60.0);
            let fired = grips.fired_press().is_some();
            let (at, s_press, s_held, s_release) =
                synthesize(&grips, pointer, holding, true, press, held, release);
            // Mask integrity: any press the sim sees inside a lever rect
            // must be a completed pull's synthesized press.
            if s_press
                && !holding
                && (layout::LAUNCH_LEVER.contains(at) || layout::ACCEPT_LEVER.contains(at))
            {
                assert!(fired, "frame {i}: raw press leaked through a lever mask");
            }
            let frame = InputFrame {
                pointer: at,
                press: s_press,
                held: s_held,
                release: s_release,
                ..InputFrame::default()
            };
            sim.advance(1.0 / 60.0, &frame);
            let departed = sim.cues().iter().any(|c| matches!(c, Cue::Depart));
            assert!(
                !departed || fired,
                "frame {i}: departed without a completed pull"
            );
        }
    }
}
