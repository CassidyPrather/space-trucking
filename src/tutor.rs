//! The previous contractor's hand: wordless, Monument-style onboarding.
//!
//! When a fresh run sits idle long enough, a translucent ghost cursor — the
//! hand of whoever had this rig before you — demonstrates what to do, then
//! fades. Two lessons: *fly* (pick a port, pull the launch lever) and
//! *trade* (drag cargo to the give pad, pull the accept lever). The ghost
//! is an overlay, never an actor: it feeds the sim no input, calls nothing
//! on it, and fakes no cues. Because the real console will not react to a
//! ghost, it carries its own dim echoes of what WOULD happen — a faint
//! selection ring, a faint lever glow — so the demonstration reads without
//! lying loudly.
//!
//! Pure by design: real `dt` plus the idle clock plus sim state in,
//! [`Ghost`] out, no macroquad anywhere, so eligibility, interruption, and
//! cooldowns are all unit-testable. What the ghost looks like is the
//! renderer's business.
//!
//! Interruption is sacred: any real input (seen here as the idle clock
//! resetting) snuffs the ghost within [`SNUFF_LEN`] seconds, and it stays
//! away entirely while the sim is paused or the omen dims the world.
//!
//! Reduced-motion note: the ghost is instructional motion, not idle
//! decoration — a future accessibility slice that gates decorative motion
//! deliberately leaves it running, calm as it is.

use space_trucking::sim::{Cue, Loc, POIS, PoiId, ShipState, Sim, Vec2, layout};

/// Idle seconds before the fly lesson may begin.
pub const FLY_IDLE: f32 = 8.0;

/// Idle seconds before the trade lesson may begin.
pub const TRADE_IDLE: f32 = 10.0;

/// Seconds after a finished lesson before any lesson repeats.
pub const COOLDOWN: f32 = 25.0;

/// Fast fade when real input (or a paused/dimmed world) snuffs the ghost.
pub const SNUFF_LEN: f32 = 0.2;

/// Fade-in at the start of a lesson.
const FADE_IN: f32 = 0.7;

/// Fade-out folded into the end of a lesson's script.
const FADE_OUT: f32 = 0.8;

/// Seconds the fake press takes to sink in or lift off.
const PRESS_EASE: f32 = 0.08;

/// The port the fly lesson demonstrates picking: Mars reads nicely
/// mid-map…
const DEMO_POI: PoiId = 2;

/// …unless the run happens to be docked there, in which case Venus stands
/// in. Either way the position comes from [`POIS`], never a hardcoded point.
const DEMO_POI_ALT: PoiId = 0;

/// What the ghost's dim echo highlights. The real console cannot react to
/// a fake hand, so the demonstration names its own faint consequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Echo {
    None,
    /// A faint selection ring over the port the ghost picked.
    Poi(PoiId),
    /// A faint go-glow on the launch lever.
    LaunchLever,
    /// A faint outline on the hold piece the ghost is "holding".
    HoldPiece(u32),
    /// A faint invite on a give-pad slot.
    GiveSlot(u8),
    /// A faint go-glow on the accept lever.
    AcceptLever,
}

/// One frame of the ghost, for the renderer to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    /// Cursor position in world coordinates.
    pub pos: Vec2,
    /// Overall translucency envelope, `0..=1`; every stroke multiplies it.
    pub alpha: f32,
    /// Fake press depth, `0..=1`, eased so the pulses read as gentle.
    pub press: f32,
    /// What to faintly highlight alongside the hand.
    pub echo: Echo,
}

/// One keyframe: the ghost sits at `pos` at `t` seconds into the lesson,
/// and the segment from this key to the next carries this echo and press
/// state. Positions between keys ease with [`smoothstep`].
struct Key {
    t: f32,
    pos: Vec2,
    echo: Echo,
    pressing: bool,
}

const fn key(t: f32, pos: Vec2, echo: Echo, pressing: bool) -> Key {
    Key {
        t,
        pos,
        echo,
        pressing,
    }
}

/// The tutor: owned by `main`, fed real time plus the idle clock, read by
/// the renderer through [`Tutor::ghost`]. It never feeds anything back.
#[derive(Default)]
pub struct Tutor {
    state: State,
    /// Seconds before another lesson may begin after one completes.
    cooldown: f32,
    /// Last frame's idle clock; a decrease means real input happened.
    last_idle: f32,
    /// The player has concluded a trade this session. Session-scoped on
    /// purpose — the simplest honest persistence is none — so a returning
    /// player who idles docked sees a gentle reminder, which is fine.
    ever_accepted: bool,
}

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Playing {
        keys: Vec<Key>,
        t: f32,
        press: f32,
    },
    /// Real input (or a paused/dimmed world) snuffed the ghost mid-lesson;
    /// the snapshot fades out fast and the tutor goes back to waiting.
    Snuffed {
        ghost: Ghost,
        t: f32,
    },
}

impl Tutor {
    /// Advance by one real frame. `idle_seconds` is wall time since the
    /// player last pressed, keyed, or toggled anything (`main` tracks it);
    /// a decrease from last frame is the interruption signal.
    pub fn update(&mut self, dt: f32, idle_seconds: f32, sim: &Sim) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if sim
            .cues()
            .iter()
            .any(|cue| matches!(cue, Cue::Accept { .. }))
        {
            self.ever_accepted = true;
        }
        let interrupted = idle_seconds < self.last_idle;
        self.last_idle = idle_seconds;
        // The ghost stays away while the sim is paused or the omen dims the
        // world: those moments belong to the player and the event.
        let blocked = sim.is_paused() || sim.omen() > 0.0 || sim.light() < 0.999;

        match &mut self.state {
            State::Idle => {
                if interrupted || blocked || self.cooldown > 0.0 {
                    return;
                }
                if idle_seconds >= FLY_IDLE {
                    if let Some(at) = fly_eligible(sim) {
                        self.state = playing(fly_script(at));
                        return;
                    }
                }
                if idle_seconds >= TRADE_IDLE && !self.ever_accepted {
                    if let Some((piece, from)) = trade_eligible(sim) {
                        self.state = playing(trade_script(piece, from));
                    }
                }
            }
            State::Playing { keys, t, press } => {
                if interrupted || blocked {
                    let ghost = sample(keys, *t, *press);
                    self.state = State::Snuffed { ghost, t: 0.0 };
                    return;
                }
                *t += dt;
                if *t >= keys[keys.len() - 1].t {
                    self.state = State::Idle;
                    self.cooldown = COOLDOWN;
                    return;
                }
                let target = if pressing_at(keys, *t) { 1.0 } else { 0.0 };
                *press = step_toward(*press, target, dt / PRESS_EASE);
            }
            State::Snuffed { t, .. } => {
                *t += dt;
                if *t >= SNUFF_LEN {
                    self.state = State::Idle;
                }
            }
        }
    }

    /// The ghost to draw this frame, if any.
    #[must_use]
    pub fn ghost(&self) -> Option<Ghost> {
        match &self.state {
            State::Idle => None,
            State::Playing { keys, t, press } => Some(sample(keys, *t, *press)),
            State::Snuffed { ghost, t } => {
                let left = 1.0 - t / SNUFF_LEN;
                (left > 0.0).then_some(Ghost {
                    alpha: ghost.alpha * left,
                    ..*ghost
                })
            }
        }
    }
}

const fn playing(keys: Vec<Key>) -> State {
    State::Playing {
        keys,
        t: 0.0,
        press: 0.0,
    }
}

// ------------------------------------------------------------- eligibility --

/// Lesson one's preconditions: a run that has never departed, docked, with
/// every hand empty. Returns where the ship is docked so the script can
/// avoid demonstrating the port it is already at.
fn fly_eligible(sim: &Sim) -> Option<PoiId> {
    let ShipState::Docked(at) = sim.ship().state else {
        return None;
    };
    (sim.legs() == 0 && sim.all_held().next().is_none()).then_some(at)
}

/// Lesson two's preconditions: docked with a trade open, the give pad
/// untouched, at least one piece stowed to offer, and every hand empty.
/// Returns the piece the ghost will pretend to carry and where it sits.
fn trade_eligible(sim: &Sim) -> Option<(u32, Vec2)> {
    if sim.barter().is_none() || sim.all_held().next().is_some() {
        return None;
    }
    if sim
        .pieces()
        .iter()
        .any(|piece| matches!(piece.loc, Loc::GivePad { .. }))
    {
        return None;
    }
    let piece = sim
        .pieces()
        .iter()
        .find(|piece| matches!(piece.loc, Loc::Hold { .. }))?;
    Some((piece.id, rect_center(layout::piece_rect(piece))))
}

// ----------------------------------------------------------------- scripts --

/// Which port the fly lesson picks: never the one the ship is docked at.
const fn demo_poi(docked: PoiId) -> PoiId {
    if docked == DEMO_POI {
        DEMO_POI_ALT
    } else {
        DEMO_POI
    }
}

/// Lesson one: fade in near the hold, drift to a port, press (ring echo),
/// drift to the launch lever, press (glow echo), fade.
fn fly_script(docked: PoiId) -> Vec<Key> {
    let target = demo_poi(docked);
    let poi = POIS[usize::from(target)].pos;
    let lever = rect_center(layout::LAUNCH_LEVER);
    let start = hold_center();
    vec![
        key(0.0, start, Echo::None, false),
        key(1.0, start, Echo::None, false),
        key(3.0, poi, Echo::None, false),
        key(3.3, poi, Echo::Poi(target), true),
        key(3.9, poi, Echo::Poi(target), false),
        key(4.9, poi, Echo::Poi(target), false),
        key(6.4, lever, Echo::LaunchLever, false),
        key(6.7, lever, Echo::LaunchLever, true),
        key(7.3, lever, Echo::LaunchLever, false),
        key(8.2, lever, Echo::LaunchLever, false),
    ]
}

/// Lesson two: fade in over a stowed piece, press and hold, carry to the
/// first give slot (invite echo), release, drift to the accept lever,
/// press (glow echo), fade.
fn trade_script(piece: u32, from: Vec2) -> Vec<Key> {
    let give = rect_center(layout::GIVE_SLOTS[0]);
    let accept = rect_center(layout::ACCEPT_LEVER);
    vec![
        key(0.0, from, Echo::None, false),
        key(1.0, from, Echo::HoldPiece(piece), false),
        key(1.4, from, Echo::HoldPiece(piece), true),
        key(1.9, from, Echo::GiveSlot(0), true),
        key(3.6, give, Echo::GiveSlot(0), true),
        key(4.2, give, Echo::GiveSlot(0), false),
        key(5.2, give, Echo::GiveSlot(0), false),
        key(6.7, accept, Echo::AcceptLever, false),
        key(7.0, accept, Echo::AcceptLever, true),
        key(7.6, accept, Echo::AcceptLever, false),
        key(8.5, accept, Echo::AcceptLever, false),
    ]
}

// ---------------------------------------------------------------- sampling --

/// Ghost state at `t` seconds into a script, with the eased press depth.
fn sample(keys: &[Key], t: f32, press: f32) -> Ghost {
    debug_assert!(keys.len() >= 2, "a script needs at least two keys");
    let len = keys[keys.len() - 1].t;
    let index = segment(keys, t);
    let (from, to) = (&keys[index], &keys[index + 1]);
    let span = (to.t - from.t).max(f32::EPSILON);
    let eased = smoothstep((t - from.t) / span);
    Ghost {
        pos: from.pos.lerp(to.pos, eased),
        alpha: (t / FADE_IN).min((len - t) / FADE_OUT).clamp(0.0, 1.0),
        press,
        echo: from.echo,
    }
}

/// Whether the segment under `t` holds the button down.
fn pressing_at(keys: &[Key], t: f32) -> bool {
    keys[segment(keys, t)].pressing
}

/// Index of the key opening the segment `t` falls in.
fn segment(keys: &[Key], t: f32) -> usize {
    keys.iter()
        .rposition(|k| k.t <= t)
        .unwrap_or(0)
        .min(keys.len().saturating_sub(2))
}

// ------------------------------------------------------------- small math --

/// Centre of a layout rect.
fn rect_center(r: layout::Rect) -> Vec2 {
    Vec2::new(r.w.mul_add(0.5, r.x), r.h.mul_add(0.5, r.y))
}

/// Centre of the hold grid: where the fly lesson's hand fades in.
fn hold_center() -> Vec2 {
    Vec2::new(
        f32::from(layout::GRID_COLS).mul_add(layout::CELL * 0.5, layout::GRID_ORIGIN.x),
        f32::from(layout::GRID_ROWS).mul_add(layout::CELL * 0.5, layout::GRID_ORIGIN.y),
    )
}

/// Hermite smoothstep, clamped to `0..=1`: gentle starts and stops, per the
/// calm pacing the art doc asks of everything.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * 2.0f32.mul_add(-t, 3.0)
}

/// Move `value` toward `target` by at most `step`; lands exactly. The sim's
/// easing primitive, restated because the sim keeps its copy private.
const fn step_toward(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use space_trucking::sim::{GUILD, InputFrame, WORLD_H, WORLD_W};

    const DT: f32 = 1.0 / 60.0;

    /// Venus, the trade-lesson test destination.
    const VENUS: PoiId = 0;

    fn press_at(p: Vec2) -> InputFrame {
        InputFrame {
            pointer: p,
            press: true,
            held: true,
            ..InputFrame::default()
        }
    }

    /// Run the tutor for `seconds` with the idle clock growing from
    /// `idle0`; returns the final idle value.
    fn run_idle(tutor: &mut Tutor, sim: &Sim, seconds: f32, idle0: f32) -> f32 {
        let mut idle = idle0;
        for _ in 0..(seconds / DT) as i32 {
            idle += DT;
            tutor.update(DT, idle, sim);
        }
        idle
    }

    /// Select Venus, pull the lever, and fast-forward onto the dock.
    fn fly_to_venus(sim: &mut Sim) {
        sim.advance(0.0, &press_at(POIS[usize::from(VENUS)].pos));
        sim.advance(0.0, &press_at(rect_center(layout::LAUNCH_LEVER)));
        let ShipState::Traveling { leg_ticks, .. } = sim.ship().state else {
            panic!("lever pull must depart");
        };
        sim.fast_forward(leg_ticks + 10);
        assert_eq!(sim.ship().state, ShipState::Docked(VENUS));
    }

    #[test]
    fn no_ghost_before_the_idle_threshold() {
        let sim = Sim::new(7);
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, FLY_IDLE - 0.5, 0.0);
        assert!(tutor.ghost().is_none());
    }

    #[test]
    fn fly_ghost_appears_on_a_fresh_idle_rig() {
        let sim = Sim::new(7);
        assert_eq!(sim.legs(), 0);
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, FLY_IDLE + 1.0, 0.0);
        let ghost = tutor.ghost().expect("idle fresh rig must be haunted");
        assert!(ghost.alpha > 0.0);
    }

    #[test]
    fn fly_ghost_demonstrates_the_port_press() {
        // Docked at the Guild, the demo port is Mars: by mid-lesson the
        // hand is on it, pressing, with the selection-ring echo up.
        let sim = Sim::new(7);
        assert_eq!(sim.ship().state, ShipState::Docked(GUILD));
        let mut tutor = Tutor::default();
        let idle = run_idle(&mut tutor, &sim, FLY_IDLE + 0.1, 0.0);
        assert!(tutor.ghost().is_some(), "lesson should have started");
        run_idle(&mut tutor, &sim, 3.6, idle);
        let ghost = tutor.ghost().expect("mid-lesson ghost");
        assert_eq!(ghost.echo, Echo::Poi(DEMO_POI));
        assert!(ghost.press > 0.9, "press was {}", ghost.press);
        let poi = POIS[usize::from(DEMO_POI)].pos;
        assert!((ghost.pos - poi).length() < 1.0, "hand not on the port");
    }

    #[test]
    fn real_input_snuffs_the_ghost_fast() {
        let sim = Sim::new(7);
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, FLY_IDLE + 2.0, 0.0);
        assert!(tutor.ghost().is_some());
        // The idle clock resetting is the interruption signal.
        tutor.update(DT, 0.0, &sim);
        let ghost = tutor.ghost().expect("snuff fades, not pops");
        run_idle(&mut tutor, &sim, SNUFF_LEN + 0.1, 0.0);
        assert!(tutor.ghost().is_none(), "ghost outlived the snuff");
        assert!(ghost.alpha <= 1.0);
    }

    #[test]
    fn cooldown_gates_the_repeat() {
        let sim = Sim::new(7);
        let mut tutor = Tutor::default();
        // Through the threshold and the whole lesson; the cooldown starts
        // at the lesson's end, a bit before this phase's.
        let idle = run_idle(&mut tutor, &sim, FLY_IDLE + 10.0, 0.0);
        assert!(tutor.ghost().is_none(), "lesson should have ended");
        // Still idle, still eligible — but cooling down.
        let idle = run_idle(&mut tutor, &sim, COOLDOWN - 3.0, idle);
        assert!(tutor.ghost().is_none(), "cooldown ignored");
        run_idle(&mut tutor, &sim, 4.0, idle);
        assert!(tutor.ghost().is_some(), "eligible rig should re-haunt");
    }

    #[test]
    fn paused_world_keeps_the_ghost_away() {
        let mut sim = Sim::new(7);
        sim.advance(
            0.0,
            &InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
        assert!(sim.is_paused());
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, FLY_IDLE + 5.0, 0.0);
        assert!(tutor.ghost().is_none());
    }

    #[test]
    fn held_piece_blocks_lessons() {
        let mut sim = Sim::new(7);
        // Grab the scrap anchored at (0, 2) and keep holding it.
        let scrap = Vec2::new(
            layout::CELL.mul_add(0.5, layout::GRID_ORIGIN.x),
            layout::CELL.mul_add(2.5, layout::GRID_ORIGIN.y),
        );
        sim.advance(0.0, &press_at(scrap));
        assert!(sim.held(0).is_some(), "nothing grabbed at {scrap:?}");
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, TRADE_IDLE + 5.0, 0.0);
        assert!(tutor.ghost().is_none());
    }

    #[test]
    fn traveling_keeps_the_ghost_away() {
        let mut sim = Sim::new(7);
        sim.advance(0.0, &press_at(POIS[usize::from(VENUS)].pos));
        sim.advance(0.0, &press_at(rect_center(layout::LAUNCH_LEVER)));
        assert!(matches!(sim.ship().state, ShipState::Traveling { .. }));
        let mut tutor = Tutor::default();
        run_idle(&mut tutor, &sim, TRADE_IDLE + 5.0, 0.0);
        assert!(tutor.ghost().is_none());
    }

    #[test]
    fn trade_lesson_waits_for_its_own_idle() {
        // A flown run: the fly lesson is spent, so the trade lesson's
        // longer threshold is what gates the haunting.
        let mut sim = Sim::new(7);
        fly_to_venus(&mut sim);
        assert_eq!(sim.legs(), 1);
        let mut tutor = Tutor::default();
        let idle = run_idle(&mut tutor, &sim, TRADE_IDLE - 0.5, 0.0);
        assert!(tutor.ghost().is_none(), "trade lesson jumped its idle");
        run_idle(&mut tutor, &sim, 1.0, idle);
        assert!(tutor.ghost().is_some(), "idle docked rig should re-haunt");
    }

    #[test]
    fn an_accepted_trade_retires_the_trade_lesson() {
        let mut sim = Sim::new(7);
        fly_to_venus(&mut sim);
        let mut tutor = Tutor {
            ever_accepted: true,
            ..Tutor::default()
        };
        run_idle(&mut tutor, &sim, TRADE_IDLE + 5.0, 0.0);
        assert!(tutor.ghost().is_none(), "spent lesson still haunting");
    }

    #[test]
    fn scripts_stay_inside_the_world() {
        let in_world = |p: Vec2| p.x >= 0.0 && p.x <= WORLD_W && p.y >= 0.0 && p.y <= WORLD_H;
        // Every possible docked port, for the fly lesson.
        for docked in 0..POIS.len() as PoiId {
            for k in fly_script(docked) {
                assert!(in_world(k.pos), "fly key leaves the world: {:?}", k.pos);
            }
        }
        // Every hold cell a carried piece could start from.
        for y in 0..layout::GRID_ROWS {
            for x in 0..layout::GRID_COLS {
                let from = rect_center(layout::cell_rect(x, y));
                for k in trade_script(0, from) {
                    assert!(in_world(k.pos), "trade key leaves the world: {:?}", k.pos);
                }
            }
        }
    }

    #[test]
    fn scripts_keep_their_keys_in_order() {
        for keys in [fly_script(GUILD), trade_script(0, hold_center())] {
            assert!(keys.len() >= 2);
            for pair in keys.windows(2) {
                assert!(pair[0].t < pair[1].t, "keyframe times must ascend");
            }
        }
    }
}
