//! Cosmetic juice: short-lived tweens and flashes keyed off sim cues.
//!
//! Everything in here is presentation-only. The struct listens to
//! [`Sim::cues`] once per frame and runs little countdown clocks on real
//! frame time; the renderer reads them as normalized "heat" values (1.0 the
//! instant something happened, cooling to 0.0). Nothing gameplay-relevant
//! lives here — deleting this module would change how the game feels, never
//! what it does.

use space_trucking::sim::{Cue, Kind, Loc, Sim, Vec2, Violation, layout};

/// Shake length for the hold grid, lever, and station glyph, seconds.
const SHAKE_LEN: f32 = 0.30;

/// How long the violation glyph stays over the offending cells.
const FLASH_LEN: f32 = 0.45;

/// Launch-lever thunk travel time.
const THUNK_LEN: f32 = 0.35;

/// Dock-ring pop after an arrival.
const POP_LEN: f32 = 0.40;

/// Long dock-ring pulse after an offline catch-up arrival: the one tween
/// allowed to run past half a second, so a returning player cannot miss it.
const PULSE_LEN: f32 = 3.5;

/// Ring blip on the freshly selected destination.
const BLIP_LEN: f32 = 0.35;

/// Held-piece scale-in after a pickup.
const PICKUP_LEN: f32 = 0.15;

/// Snap-settle overshoot after a legal placement.
const SETTLE_LEN: f32 = 0.18;

/// Received goods sliding from the take pad to the received shelf.
const SLIDE_LEN: f32 = 0.30;

/// Radial celebration flash from the dial after an accepted trade.
const ACCEPT_LEN: f32 = 0.45;

/// Screen-space violet flash when the suspicious jump fires.
const JUMP_LEN: f32 = 0.45;

/// Red dial flash after a refused trade.
const DIAL_LEN: f32 = 0.35;

/// Normalized cooldown: 1.0 the instant a timer is wound, 0.0 once spent.
const fn heat(remaining: f32, length: f32) -> f32 {
    (remaining / length).clamp(0.0, 1.0)
}

/// All the renderer's cosmetic clocks. Default is everything cold.
#[derive(Default)]
pub struct Juice {
    /// Free-running cosmetic clock, real seconds since startup. Drives
    /// twinkles, orbits, and pulses; never touches the sim.
    clock: f32,
    grid_shake: f32,
    lever_shake: f32,
    lever_thunk: f32,
    dial_flash: f32,
    station_shake: f32,
    accept_flash: f32,
    accept_value: f32,
    slide: f32,
    /// Pieces mid-slide from the take pad to the received shelf.
    slide_ids: Vec<u32>,
    violation: f32,
    /// The refused rule and the world rect of the attempted footprint.
    violation_at: Option<(Violation, layout::Rect)>,
    jump_flash: f32,
    dock_pop: f32,
    dock_pulse: f32,
    select_blip: f32,
    pickup: f32,
    settle: f32,
    settle_id: Option<u32>,
    /// Last frame's held piece, so Place/Reject cues (which fire after the
    /// sim already dropped it) still know what was being carried.
    held_was: Option<(u32, Kind)>,
}

impl Juice {
    /// Advance every clock by real `dt` and wind up whatever this frame's
    /// cues call for. `pointer` and `pressed` are this frame's input, used
    /// only to aim flashes (which cell, which lever).
    pub fn update(&mut self, dt: f32, sim: &Sim, pointer: Vec2, pressed: bool) {
        self.clock += dt;
        for timer in [
            &mut self.grid_shake,
            &mut self.lever_shake,
            &mut self.lever_thunk,
            &mut self.dial_flash,
            &mut self.station_shake,
            &mut self.accept_flash,
            &mut self.slide,
            &mut self.violation,
            &mut self.jump_flash,
            &mut self.dock_pop,
            &mut self.dock_pulse,
            &mut self.select_blip,
            &mut self.pickup,
            &mut self.settle,
        ] {
            *timer = (*timer - dt).max(0.0);
        }

        for cue in sim.cues() {
            match *cue {
                Cue::Select => self.select_blip = BLIP_LEN,
                Cue::Depart => self.lever_thunk = THUNK_LEN,
                Cue::Arrive => self.dock_pop = POP_LEN,
                Cue::Pickup => self.pickup = PICKUP_LEN,
                Cue::Place => {
                    self.settle = SETTLE_LEN;
                    self.settle_id = self.held_was.map(|(id, _)| id);
                }
                Cue::Reject { hard: true } => self.hard_reject(sim, pointer),
                Cue::Reject { hard: false } => {
                    if pressed && layout::LAUNCH_LEVER.contains(pointer) {
                        self.lever_shake = SHAKE_LEN;
                    }
                }
                Cue::Accept { value } => self.accept(sim, value),
                Cue::Refuse => {
                    self.dial_flash = DIAL_LEN;
                    self.station_shake = SHAKE_LEN;
                }
                Cue::Jump => self.jump_flash = JUMP_LEN,
                _ => {}
            }
        }

        self.held_was = sim
            .held()
            .and_then(|held| sim.pieces().iter().find(|piece| piece.id == held.piece))
            .map(|piece| (piece.id, piece.kind));
    }

    /// An offline catch-up docked the ship: pulse the dock ring long enough
    /// for a returning player to notice where they ended up.
    pub const fn catch_up_arrival(&mut self) {
        self.dock_pulse = PULSE_LEN;
    }

    /// A stowage rule refused a drop: shake the grid and aim the rule glyph
    /// at the footprint the drop would have covered.
    fn hard_reject(&mut self, sim: &Sim, pointer: Vec2) {
        self.grid_shake = SHAKE_LEN;
        let Some(rule) = sim.last_violation() else {
            return;
        };
        let (w, h) = self.held_was.map_or((1, 1), |(_, kind)| kind.cells());
        let (x, y) = layout::cell_at(pointer).unwrap_or((0, 0));
        let anchor = layout::cell_rect(x, y);
        self.violation = FLASH_LEN;
        self.violation_at = Some((
            rule,
            layout::Rect::new(
                anchor.x,
                anchor.y,
                f32::from(w) * layout::CELL,
                f32::from(h) * layout::CELL,
            ),
        ));
    }

    /// A trade concluded: flash the dial (scaled by generosity) and start
    /// the received goods sliding from the take pad to their shelf.
    fn accept(&mut self, sim: &Sim, value: f32) {
        self.accept_flash = ACCEPT_LEN;
        self.accept_value = value;
        self.slide = SLIDE_LEN;
        self.slide_ids.clear();
        self.slide_ids.extend(
            sim.pieces()
                .iter()
                .filter(|piece| matches!(piece.loc, Loc::ReceivedShelf { .. }))
                .map(|piece| piece.id),
        );
    }

    /// Free-running cosmetic clock, seconds.
    #[must_use]
    pub const fn clock(&self) -> f32 {
        self.clock
    }

    /// Hold-grid shake heat after a hard reject.
    #[must_use]
    pub const fn grid_shake(&self) -> f32 {
        heat(self.grid_shake, SHAKE_LEN)
    }

    /// Launch-lever shake heat after a refused pull.
    #[must_use]
    pub const fn lever_shake(&self) -> f32 {
        heat(self.lever_shake, SHAKE_LEN)
    }

    /// Launch-lever thunk heat after a departure.
    #[must_use]
    pub const fn lever_thunk(&self) -> f32 {
        heat(self.lever_thunk, THUNK_LEN)
    }

    /// Red dial-flash heat after a refused trade.
    #[must_use]
    pub const fn dial_flash(&self) -> f32 {
        heat(self.dial_flash, DIAL_LEN)
    }

    /// Station-glyph shake heat after a refused trade.
    #[must_use]
    pub const fn station_shake(&self) -> f32 {
        heat(self.station_shake, SHAKE_LEN)
    }

    /// Accept celebration: `(heat, generosity)` while the flash lives.
    #[must_use]
    pub fn accept_flash(&self) -> Option<(f32, f32)> {
        (self.accept_flash > 0.0)
            .then_some((heat(self.accept_flash, ACCEPT_LEN), self.accept_value))
    }

    /// Where a freshly received piece is along its take-pad-to-shelf slide,
    /// `0..=1`, if it is mid-slide at all.
    #[must_use]
    pub fn received_slide(&self, id: u32) -> Option<f32> {
        (self.slide > 0.0 && self.slide_ids.contains(&id))
            .then_some(1.0 - heat(self.slide, SLIDE_LEN))
    }

    /// The refused stowage rule, the world rect it refused over, and the
    /// flash heat, while the flash lives.
    #[must_use]
    pub fn violation(&self) -> Option<(Violation, layout::Rect, f32)> {
        let (rule, rect) = self.violation_at?;
        (self.violation > 0.0).then_some((rule, rect, heat(self.violation, FLASH_LEN)))
    }

    /// Screen-space violet flash heat as the suspicious jump fires.
    #[must_use]
    pub const fn jump_flash(&self) -> f32 {
        heat(self.jump_flash, JUMP_LEN)
    }

    /// Dock-ring pop heat right after an arrival.
    #[must_use]
    pub const fn dock_pop(&self) -> f32 {
        heat(self.dock_pop, POP_LEN)
    }

    /// Long dock-ring pulse heat after an offline catch-up arrival.
    #[must_use]
    pub const fn dock_pulse(&self) -> f32 {
        heat(self.dock_pulse, PULSE_LEN)
    }

    /// Selection blip heat on the freshly picked destination.
    #[must_use]
    pub const fn select_blip(&self) -> f32 {
        heat(self.select_blip, BLIP_LEN)
    }

    /// Pickup scale-in heat: 1.0 the instant a piece is lifted.
    #[must_use]
    pub const fn pickup_pop(&self) -> f32 {
        heat(self.pickup, PICKUP_LEN)
    }

    /// Snap-settle heat for the piece just placed, if it is `id`.
    #[must_use]
    pub fn settling(&self, id: u32) -> Option<f32> {
        (self.settle > 0.0 && self.settle_id == Some(id)).then_some(heat(self.settle, SETTLE_LEN))
    }
}
