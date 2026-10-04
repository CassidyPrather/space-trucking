//! Travel encounters and the ad drone: things to meet over a leg.
//!
//! DESIGN.md asks events to be ignorable ("disengagement is
//! participation") and never wholly arbitrary. An encounter is a window
//! `[start, end)` in leg progress, scheduled at departure from
//! `splitmix(seed, legs)` exactly like the omen's jump tick — so it saves,
//! replays, and fast-forwards bit-identically. Inside the window something
//! sits alongside the ship: a derelict with a hold, an inexplicable gas
//! station, an inexplicable casino, a meteor shower, or a whale. The
//! three with a counterparty or a place bring a **room** alongside
//! (docs/ROOMS.md, "Events as rooms"); the meteor shower and the whale
//! are weather and stay schedules. Ignore any of them and the window
//! closes — but a room that came alongside stays until somebody shuts
//! the door, which is why an unresolved event blocks the next takeoff.
//!
//! The omen interaction is the subtle part, and it is tested from the
//! outside: a suspicious jump moves `progress` forward discontinuously,
//! which can skip a window entirely (it never opens) or land past the end
//! of an open one (it closes on the next travel tick). Both fall out of
//! comparing `progress` against the window bounds instead of counting
//! elapsed ticks.
//!
//! The ad drone is the second, smaller machine in this file: a billboard
//! that latches onto the ship mid-leg and plays unintelligible ads over
//! the console's screens until swatted twice or bored. Structurally both
//! follow the event-sibling convention (`on_depart` / `travel_tick` /
//! `on_dock` hooks, own save lines, own cues) that `event.rs` and
//! `rats.rs` established.

use super::cargo::Kind;
use super::{Cue, splitmix};

/// One leg in this many carries an encounter.
const ENCOUNTER_CHANCE: u64 = 3;

/// One leg in this many attracts an ad drone (independent of encounters).
const AD_CHANCE: u64 = 4;

/// Longest an encounter window runs, in ticks (two minutes alongside).
const WINDOW_CAP: u64 = 7200;

/// Swats needed to knock the ad drone off the hull.
pub const AD_SWATS: u8 = 2;

/// Ticks between the meteor shower's extra creak volleys.
const METEOR_CREAK_EVERY: u64 = 90;

/// Ticks between whale verses while one is alongside.
const WHALE_VERSE_EVERY: u64 = 600;

/// Stream salts, disjoint from every other derived stream in the sim.
const SALT_ENCOUNTER: u64 = 0xE4C0_0117;
const SALT_AD: u64 = 0xAD_D12073;
const SALT_CASINO: u64 = 0xCA_51F0;

/// What is out there this leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterKind {
    /// A dead ship. Its cargo is on its own floor; taking it is a carry
    /// through a doorway.
    Derelict,
    /// Fuel, snacks, flickering sign. Nobody knows who runs it. Working
    /// its pump bay's handshake tops the tanks up: a little of the
    /// remaining leg is skipped, once.
    GasStation,
    /// Neon heptagram, no visible doors. Stake cargo on the parlor's
    /// offer area and work its handshake: double or a commemorative chip.
    Casino,
    /// Gravel weather. Loud, then profitable: something always ends up
    /// embedded in the hull.
    MeteorShower,
    /// It is very large and it is singing. It likes gardens.
    Whale,
}

impl EncounterKind {
    /// Stable save token.
    pub(crate) const fn token(self) -> u8 {
        match self {
            Self::Derelict => 0,
            Self::GasStation => 1,
            Self::Casino => 2,
            Self::MeteorShower => 3,
            Self::Whale => 4,
        }
    }

    /// Inverse of [`EncounterKind::token`].
    pub(crate) const fn from_token(token: u8) -> Option<Self> {
        match token {
            0 => Some(Self::Derelict),
            1 => Some(Self::GasStation),
            2 => Some(Self::Casino),
            3 => Some(Self::MeteorShower),
            4 => Some(Self::Whale),
            _ => None,
        }
    }
}

/// One scheduled (or running) encounter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encounter {
    pub kind: EncounterKind,
    /// Window bounds in leg progress ticks.
    pub start: u64,
    pub end: u64,
    /// The window has opened (the thing is alongside).
    pub opened: bool,
    /// The window has closed (it fell astern, or the leg ended).
    pub closed: bool,
    /// The one-shot interaction is spent (gas top-up taken, whale's
    /// garden verse sung).
    pub used: bool,
}

impl Encounter {
    /// Whether the thing is alongside right now.
    #[must_use]
    pub const fn open(&self) -> bool {
        self.opened && !self.closed
    }
}

/// The encounter machine: at most one per leg.
#[derive(Clone, Debug, Default)]
pub struct Encounters {
    pub current: Option<Encounter>,
}

impl Encounters {
    pub const fn new() -> Self {
        Self { current: None }
    }

    /// Roll this leg's encounter, if any. Called at departure.
    pub fn on_depart(&mut self, seed: u64, legs: u64, leg_ticks: u64) {
        self.current = None;
        let h = splitmix(seed ^ SALT_ENCOUNTER, legs);
        if h % ENCOUNTER_CHANCE != 0 || leg_ticks < 3600 {
            // Short hops meet nothing; there is barely time to wave.
            return;
        }
        let kind = match (h >> 8) % 5 {
            0 => EncounterKind::Derelict,
            1 => EncounterKind::GasStation,
            2 => EncounterKind::Casino,
            3 => EncounterKind::MeteorShower,
            _ => EncounterKind::Whale,
        };
        // Somewhere in the 20%..70% belly of the leg, never brushing the
        // dock, capped so even a monster leg meets it for minutes only.
        let start = leg_ticks / 5 + (h >> 16) % (leg_ticks / 2).max(1);
        let duration = (leg_ticks / 4).clamp(1800, WINDOW_CAP);
        let end = (start + duration)
            .min(leg_ticks.saturating_sub(600))
            .max(start + 1);
        self.current = Some(Encounter {
            kind,
            start,
            end,
            opened: false,
            closed: false,
            used: false,
        });
    }

    /// One travel tick: open or close the window against `progress`. The
    /// omen jump moves `progress` in leaps; comparing against bounds (not
    /// counting ticks) is what makes a leap skip or close a window
    /// correctly. Returns flotsam to spawn when a window just opened.
    pub fn travel_tick(
        &mut self,
        seed: u64,
        legs: u64,
        progress: u64,
        tick: u64,
        seedlings_aboard: bool,
        cues: &mut Vec<Cue>,
    ) -> Vec<Kind> {
        let Some(enc) = &mut self.current else {
            return Vec::new();
        };
        if enc.closed {
            return Vec::new();
        }
        if !enc.opened {
            if progress >= enc.end {
                // A jump carried the ship clean past it: never met.
                enc.closed = true;
                return Vec::new();
            }
            if progress >= enc.start {
                enc.opened = true;
                cues.push(Cue::EncounterStart);
                let h = splitmix(seed ^ SALT_ENCOUNTER, legs ^ 0xF107);
                return match enc.kind {
                    EncounterKind::Derelict => {
                        // A hold's worth of somebody's story, adrift. One
                        // piece odd, one piece hummed over.
                        let odd = match h % 3 {
                            0 => Kind::GildedIdol,
                            1 => Kind::CryoCore,
                            _ => Kind::BrinePearls,
                        };
                        if (h >> 32) % 3 == 0 {
                            vec![odd, Kind::MysteriousCrate]
                        } else {
                            vec![odd, Kind::ScrapAlloy]
                        }
                    }
                    _ => Vec::new(),
                };
            }
            return Vec::new();
        }
        // Open: ambience per kind, then the closing bound.
        if progress >= enc.end {
            enc.closed = true;
            cues.push(Cue::EncounterEnd);
            if enc.kind == EncounterKind::MeteorShower {
                // The souvenir: something is embedded in the hull, and it
                // is yours now. Reported via the return value's sibling
                // path (the caller spawns hull salvage into flotsam).
                return vec![Kind::ScrapAlloy];
            }
            return Vec::new();
        }
        match enc.kind {
            EncounterKind::MeteorShower => {
                if progress % METEOR_CREAK_EVERY == 0 {
                    let h = splitmix(seed ^ SALT_ENCOUNTER, tick);
                    let intensity = ((h >> 20) % 700) as f32 / 1000.0 + 0.3;
                    cues.push(Cue::Creak { intensity });
                }
            }
            EncounterKind::Whale => {
                if progress % WHALE_VERSE_EVERY == 0 {
                    let h = splitmix(seed ^ SALT_ENCOUNTER, tick);
                    let intensity = ((h >> 12) % 500) as f32 / 1000.0 + 0.3;
                    cues.push(Cue::WhaleSong { intensity });
                }
                if seedlings_aboard && !enc.used && progress % WHALE_VERSE_EVERY == 300 {
                    // It noticed the garden. It sings back, once, closer.
                    enc.used = true;
                    cues.push(Cue::WhaleSong { intensity: 1.0 });
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Docked: whatever was alongside falls astern for good.
    pub fn on_dock(&mut self, cues: &mut Vec<Cue>) {
        if let Some(enc) = &mut self.current {
            if enc.open() {
                cues.push(Cue::EncounterEnd);
            }
            enc.closed = true;
        }
        self.current = None;
    }

    /// The casino's coin, flipped once per wager. `true` is a win.
    #[must_use]
    pub const fn casino_coin(seed: u64, tick: u64) -> bool {
        splitmix(seed ^ SALT_CASINO, tick) % 2 == 0
    }
}

/// The ad drone: a flying billboard that latches on mid-leg and plays
/// ads in a language nobody reads over every screen on the console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drone {
    /// Window bounds in leg progress ticks.
    pub start: u64,
    pub end: u64,
    /// It is currently attached and advertising.
    pub attached: bool,
    /// It has left (swatted off or bored), never to return this leg.
    pub gone: bool,
    /// Swats landed so far, `0..AD_SWATS`.
    pub swats: u8,
}

/// The drone machine: at most one per leg.
#[derive(Clone, Debug, Default)]
pub struct Drones {
    pub drone: Option<Drone>,
}

impl Drones {
    pub const fn new() -> Self {
        Self { drone: None }
    }

    /// Roll this leg's drone, if any. Independent of the encounter roll.
    pub fn on_depart(&mut self, seed: u64, legs: u64, leg_ticks: u64) {
        self.drone = None;
        let h = splitmix(seed ^ SALT_AD, legs);
        if h % AD_CHANCE != 0 || leg_ticks < 3600 {
            return;
        }
        let start = leg_ticks / 6 + (h >> 16) % (leg_ticks / 2).max(1);
        let end = (start + (leg_ticks / 3).min(WINDOW_CAP)).min(leg_ticks.saturating_sub(300));
        if end <= start {
            return;
        }
        self.drone = Some(Drone {
            start,
            end,
            attached: false,
            gone: false,
            swats: 0,
        });
    }

    /// One travel tick: latch on or get bored, against `progress`.
    pub fn travel_tick(&mut self, progress: u64, cues: &mut Vec<Cue>) {
        let Some(drone) = &mut self.drone else {
            return;
        };
        if drone.gone {
            return;
        }
        if !drone.attached {
            if progress >= drone.end {
                drone.gone = true;
            } else if progress >= drone.start {
                drone.attached = true;
                cues.push(Cue::AdStart);
            }
            return;
        }
        if progress >= drone.end {
            drone.attached = false;
            drone.gone = true;
            cues.push(Cue::AdEnd);
        }
    }

    /// A press landed on the drone: one swat. The second knocks it off.
    /// Returns whether the press was consumed.
    pub fn on_press(&mut self, cues: &mut Vec<Cue>) -> bool {
        let Some(drone) = &mut self.drone else {
            return false;
        };
        if !drone.attached {
            return false;
        }
        drone.swats += 1;
        cues.push(Cue::AdSwat);
        if drone.swats >= AD_SWATS {
            drone.attached = false;
            drone.gone = true;
            cues.push(Cue::AdEnd);
        }
        true
    }

    /// Docked: any surviving drone peels away to bother someone else.
    pub fn on_dock(&mut self, cues: &mut Vec<Cue>) {
        if let Some(drone) = &self.drone {
            if drone.attached {
                cues.push(Cue::AdEnd);
            }
        }
        self.drone = None;
    }

    /// Whether the ads are playing right now.
    #[must_use]
    pub fn advertising(&self) -> bool {
        self.drone.is_some_and(|drone| drone.attached)
    }
}
