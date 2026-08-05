//! Cues in, sound out. The audio half of the frontend.
//!
//! This is the counterpart to `draw`: [`space_trucking::sim`] says *what*
//! happened, and everything here decides what that sounds like. The waveforms
//! themselves come from [`space_trucking::synth`], which stays macroquad-free
//! so it can be unit-tested; this file is the part that needs a live audio
//! context.
//!
//! ## The autoplay problem
//!
//! Browsers start every page's audio context suspended and only resume it
//! inside a real user gesture. quad-snd's `audio.js` already hooks
//! mousedown/keydown/touch to do the resuming, but the four ambient loops
//! must not be started at load: against a suspended context they would run
//! silently and then arrive mid-note the instant the page woke up. So they
//! wait for the first press — started at volume zero and eased up from
//! there — and until then [`Audio::needs_gesture`] is true so the renderer
//! can pulse the speaker icon. One-shots wait for the same press: the sim
//! ticks (and creaks) before anyone touches anything, and a cue played into
//! a suspended context would come back from the dead on resume.

use macroquad::audio::{self, PlaySoundParams, Sound};
use macroquad::input::{MouseButton, get_last_key_pressed, is_mouse_button_pressed};
use space_trucking::sim::{Cue, ShipState, Sim};
use space_trucking::synth;

/// Gain for docking — the ship setting its weight down. Medium: it ends a
/// leg, so it may land with a little authority.
const ARRIVE_GAIN: f32 = 0.5;

/// Gain for undocking. A touch under [`ARRIVE_GAIN`]: departures are the
/// start of something, not the payoff.
const DEPART_GAIN: f32 = 0.45;

/// Gain for lifting a piece. Quiet — this fires on every pickup during
/// hold-tetris and anything bigger would nag.
const PICKUP_GAIN: f32 = 0.3;

/// Gain for a legal placement, the most tactile sound in the game. The
/// loudest of the cargo sounds on purpose: this is the juice.
const PLACE_GAIN: f32 = 0.55;

/// Gain for a soft reject (an ignored click or drop). Barely a shrug.
const REJECT_SOFT_GAIN: f32 = 0.22;

/// Gain for a hard reject (a placement rule refused an in-grid drop).
/// Louder than soft: the player tried something real and was told no.
const REJECT_HARD_GAIN: f32 = 0.45;

/// Gain for the station refusing a trade. Between the two rejects so it
/// reads as its own kind of no; the voice itself already feels longer.
const REFUSE_GAIN: f32 = 0.32;

/// Peak gain for an accepted trade, scaled by [`loudness`] of the
/// generosity overshoot — a generous deal sounds warmer.
const ACCEPT_GAIN: f32 = 0.5;

/// Gain for picking a destination. Nearly subliminal: an acknowledgement,
/// not an event.
const SELECT_GAIN: f32 = 0.12;

/// Gain for the suspicious jump — the biggest sound in the game, and still
/// moderate, because this all plays for hours in the background.
const JUMP_GAIN: f32 = 0.7;

/// Gain for the Guild hangar swallowing a delivered crate. It borrows the
/// jump's stinger at half strength: the hangar's take is kin to the crate's
/// own magic, and the shared voice says so — quietly, since it lands under
/// the arrival clunk and belongs to the Guild, not the player.
const DELIVERED_GAIN: f32 = 0.35;

/// Peak gain for a hull creak, scaled by [`loudness`] of its intensity.
/// Quiet: creaks are texture, not information.
const CREAK_GAIN: f32 = 0.25;

/// Gain for a rat stowing away: one soft creak off the round-robin bank —
/// something shifted in the hold as the ship cast off. Under [`CREAK_GAIN`]
/// so it hides among the ordinary hull noises; the rat is a discovery, not
/// an announcement.
const RAT_ABOARD_GAIN: f32 = 0.2;

/// Peak gain for a rat hop, scaled by [`loudness`] of its intensity. The
/// quietest recurring sound in the game — `tick_pick` at a whisper, every
/// ten seconds or so, deliberately ignorable.
const RAT_SKITTER_GAIN: f32 = 0.12;

/// Gain for a nibble: the buzz voice at very low gain, more felt than
/// heard. It recurs for as long as the rat is ignored, so anything louder
/// would nag — and nagging is exactly what this event must not do.
const RAT_NIBBLE_GAIN: f32 = 0.08;

/// Gain for a chase: `tick_pick` at pickup strength — the player acted and
/// gets the same tactile acknowledgement a lift does.
const RAT_CHASED_GAIN: f32 = 0.3;

/// Gain for the rat leaving: the latch voice, low — a small door closing
/// somewhere below decks. Under [`DEPART_GAIN`], since it usually lands
/// beside an arrival clunk.
const RAT_LEFT_GAIN: f32 = 0.25;

/// Gain for the pause and warp blips, which do not vary.
const UI_GAIN: f32 = 0.35;

/// Gain for the reseed chime.
const CHIME_GAIN: f32 = 0.4;

/// Comet ice thunking aboard, before the haul scaling.
const HARVEST_GAIN: f32 = 0.5;

/// The ??? exchange's quiet stinger.
const EXCHANGE_GAIN: f32 = 0.3;

/// Shutters coming down on a station out of patience.
const SHUTTER_GAIN: f32 = 0.6;

/// Encounter window opening and closing blips.
const ENCOUNTER_GAIN: f32 = 0.4;

/// The gas station's top-up clunk.
const GAS_GAIN: f32 = 0.4;

/// Casino payout and casino consolation.
const CASINO_WIN_GAIN: f32 = 0.55;
const CASINO_LOSS_GAIN: f32 = 0.35;

/// Peak gain for a whale verse, scaled by [`loudness`].
const WHALE_GAIN: f32 = 0.45;

/// The ad drone's arrival buzz and departure blip.
const AD_GAIN: f32 = 0.3;

/// A swat landing on the drone.
const AD_SWAT_GAIN: f32 = 0.5;

/// One fluff becoming two, barely audibly.
const FLUFF_GAIN: f32 = 0.15;

/// The Grand Parade's stinger — the biggest ceremony the game has.
const PARADE_GAIN: f32 = 0.6;

/// Cruising engine loop target while traveling out of warp.
const ENGINE_GAIN: f32 = 0.25;

/// Warp engine loop target while traveling in warp. Slightly above the
/// cruise target: the same machine, working harder.
const WARP_GAIN: f32 = 0.3;

/// How fast either engine loop rises and falls, in gain per second. A big
/// ship spins up faster than it coasts down; both engines share the pair,
/// so toggling warp crossfades them at matching speed.
const ENGINE_FADE_IN: f32 = 0.8;
const ENGINE_FADE_OUT: f32 = 0.5;

/// Hum loop target while the suspicious crate is merely aboard.
const HUM_BASE: f32 = 0.12;

/// How much the omen adds to the hum at full swell. `HUM_BASE` plus this is
/// the loudest standing sound in the game right before the jump — the
/// design doc's mystery hook, and it has to be felt.
const HUM_SWELL: f32 = 0.68;

/// Hum fade rates, in gain per second. Slower than the engines: the hum
/// creeps in and lingers, which is most of what makes it eerie.
const HUM_FADE_IN: f32 = 0.6;
const HUM_FADE_OUT: f32 = 0.35;

/// Station room-tone target while docked. Very quiet — air handlers you
/// only notice when they stop.
const AIR_GAIN: f32 = 0.08;

/// Room-tone fade rates, in gain per second. Eases in as the berth seals,
/// drops a little quicker when the ship lets go.
const AIR_FADE_IN: f32 = 0.4;
const AIR_FADE_OUT: f32 = 0.6;

/// One ambient loop: its sound, its current eased gain, and its fade rates.
struct Ambient {
    sound: Sound,
    /// Current gain, eased toward the frame's target every update.
    gain: f32,
    /// Gain per second while rising toward the target.
    fade_in: f32,
    /// Gain per second while falling toward the target.
    fade_out: f32,
}

impl Ambient {
    /// Start the loop silent. Called once, inside the first real gesture,
    /// per the autoplay rules in the module doc.
    fn start(&self) {
        audio::play_sound(
            &self.sound,
            PlaySoundParams {
                looped: true,
                volume: 0.0,
            },
        );
    }

    /// Move one frame's worth toward `target` and push the result to the
    /// mixer. Stepping straight to the target would click; these rates are
    /// slow enough to sound like machinery, fast enough to feel causal.
    fn ease(&mut self, target: f32, dt: f32) {
        let rate = if target > self.gain {
            self.fade_in
        } else {
            self.fade_out
        };
        let step = rate * dt;
        self.gain += (target - self.gain).clamp(-step, step);
        audio::set_sound_volume(&self.sound, self.gain);
    }
}

/// The loaded sound bank plus the state that shapes playback.
pub struct Audio {
    clunk: Sound,
    latch: Sound,
    tick_pick: Sound,
    thock: Sound,
    buzz: Sound,
    deal: Sound,
    /// The three hull-creak bakes, played round-robin.
    creaks: [Sound; 3],
    stinger: Sound,
    blip_up: Sound,
    blip_down: Sound,
    chime: Sound,
    engine: Ambient,
    engine_warp: Ambient,
    hum: Ambient,
    station_air: Ambient,
    /// Which creak plays next. Cosmetic variety, so it lives out here
    /// rather than in the deterministic sim.
    next_creak: usize,
    /// Whether a real user gesture has arrived. Browsers keep the audio
    /// context suspended until one does, so anything looping must wait.
    awake: bool,
    /// Whether the player has muted.
    muted: bool,
}

impl Audio {
    /// Synthesise and decode the whole bank.
    ///
    /// Async because on the web each buffer goes to the browser to decode
    /// and macroquad waits frames for the result. It is a handful of frames
    /// at startup for a few hundred kilobytes of samples.
    pub async fn load() -> Self {
        Self {
            clunk: bake(&synth::clunk()).await,
            latch: bake(&synth::latch()).await,
            tick_pick: bake(&synth::tick_pick()).await,
            thock: bake(&synth::thock()).await,
            buzz: bake(&synth::buzz()).await,
            deal: bake(&synth::deal()).await,
            creaks: [
                bake(&synth::creak_a()).await,
                bake(&synth::creak_b()).await,
                bake(&synth::creak_c()).await,
            ],
            stinger: bake(&synth::stinger()).await,
            blip_up: bake(&synth::blip(true)).await,
            blip_down: bake(&synth::blip(false)).await,
            chime: bake(&synth::chime()).await,
            engine: Ambient {
                sound: bake(&synth::engine()).await,
                gain: 0.0,
                fade_in: ENGINE_FADE_IN,
                fade_out: ENGINE_FADE_OUT,
            },
            engine_warp: Ambient {
                sound: bake(&synth::engine_warp()).await,
                gain: 0.0,
                fade_in: ENGINE_FADE_IN,
                fade_out: ENGINE_FADE_OUT,
            },
            hum: Ambient {
                sound: bake(&synth::hum()).await,
                gain: 0.0,
                fade_in: HUM_FADE_IN,
                fade_out: HUM_FADE_OUT,
            },
            station_air: Ambient {
                sound: bake(&synth::station_air()).await,
                gain: 0.0,
                fade_in: AIR_FADE_IN,
                fade_out: AIR_FADE_OUT,
            },
            next_creak: 0,
            awake: false,
            muted: false,
        }
    }

    /// One frame: handle the mute key, wake on the first gesture, play
    /// everything the sim cued, and ease the four loops toward whatever the
    /// sim's current state asks for. All sim state is read fresh here every
    /// frame — nothing is cached across frames, so a reseed (which drops
    /// warp, undocks, everything) simply retargets the loops.
    pub fn update(&mut self, dt: f32, sim: &Sim, toggle_mute: bool) {
        if toggle_mute {
            self.muted = !self.muted;
        }

        if !self.awake && (toggle_mute || pressed_this_frame()) {
            self.awake = true;
            self.engine.start();
            self.engine_warp.start();
            self.hum.start();
            self.station_air.start();
        }
        if !self.awake {
            // Nothing is playing yet; don't poke the mixer about it.
            return;
        }

        for cue in sim.cues() {
            self.play(*cue);
        }
        self.ease_loops(dt, sim);
    }

    /// Whether audio still waits on a first gesture, which is the one audio
    /// state worth signalling to the player.
    #[must_use]
    pub const fn needs_gesture(&self) -> bool {
        !self.awake
    }

    /// Whether the player has muted, for the speaker icon.
    #[must_use]
    pub const fn muted(&self) -> bool {
        self.muted
    }

    /// Fire one cue on its voice.
    fn play(&mut self, cue: Cue) {
        if self.muted {
            return;
        }
        let (sound, volume) = match cue {
            Cue::Select => (&self.tick_pick, SELECT_GAIN),
            Cue::Depart => (&self.latch, DEPART_GAIN),
            Cue::Arrive => (&self.clunk, ARRIVE_GAIN),
            Cue::Pickup => (&self.tick_pick, PICKUP_GAIN),
            Cue::Place => (&self.thock, PLACE_GAIN),
            Cue::Reject { hard: false } => (&self.buzz, REJECT_SOFT_GAIN),
            Cue::Reject { hard: true } => (&self.buzz, REJECT_HARD_GAIN),
            Cue::Refuse => (&self.buzz, REFUSE_GAIN),
            Cue::Accept { value } => (&self.deal, ACCEPT_GAIN * loudness(value)),
            Cue::Jump => (&self.stinger, JUMP_GAIN),
            Cue::Delivered => (&self.stinger, DELIVERED_GAIN),
            // No one-shot: the omen is the hum swelling, and marking its
            // edges with a sting would give the mystery away.
            Cue::OmenStart | Cue::OmenEnd => return,
            // Shutters slamming: the latch, hard. The message lands.
            Cue::Shutter => (&self.latch, SHUTTER_GAIN),
            // Something pulled alongside; something fell astern.
            Cue::EncounterStart => (&self.blip_up, ENCOUNTER_GAIN),
            Cue::EncounterEnd => (&self.blip_down, ENCOUNTER_GAIN),
            // The gas station's inexplicable generosity.
            Cue::GasBoost => (&self.latch, GAS_GAIN),
            // The casino's two moods.
            Cue::CasinoWin => (&self.deal, CASINO_WIN_GAIN),
            Cue::CasinoLoss => (&self.buzz, CASINO_LOSS_GAIN),
            // The whale, through the hull. The creak bank at whale scale.
            Cue::WhaleSong { intensity } => {
                let sound = &self.creaks[0];
                let volume = WHALE_GAIN * loudness(intensity);
                (sound, volume)
            }
            // Ads. Ads ads ads. Then, mercifully, not.
            Cue::AdStart => (&self.buzz, AD_GAIN),
            Cue::AdSwat => (&self.thock, AD_SWAT_GAIN),
            Cue::AdEnd => (&self.blip_down, AD_GAIN),
            // A very soft pop, like a second yawn.
            Cue::FluffBirth => (&self.tick_pick, FLUFF_GAIN),
            // The hangar opens. Whatever it was for, it is happening.
            Cue::ParadeStart => (&self.stinger, PARADE_GAIN),
            // Free cargo thunking into the hold, scaled by the haul.
            Cue::Harvest { intensity } => (&self.thock, HARVEST_GAIN * loudness(intensity)),
            // The exchange gets the stinger, quiet: ??? is not loud.
            Cue::Exchange => (&self.stinger, EXCHANGE_GAIN),
            Cue::Creak { intensity } => {
                let creak = &self.creaks[self.next_creak];
                self.next_creak = (self.next_creak + 1) % self.creaks.len();
                (creak, CREAK_GAIN * loudness(intensity))
            }
            Cue::RatAboard => {
                let creak = &self.creaks[self.next_creak];
                self.next_creak = (self.next_creak + 1) % self.creaks.len();
                (creak, RAT_ABOARD_GAIN)
            }
            Cue::RatSkitter { intensity } => {
                (&self.tick_pick, RAT_SKITTER_GAIN * loudness(intensity))
            }
            Cue::RatNibble => (&self.buzz, RAT_NIBBLE_GAIN),
            Cue::RatChased => (&self.tick_pick, RAT_CHASED_GAIN),
            Cue::RatLeft => (&self.latch, RAT_LEFT_GAIN),
            Cue::Pause { paused: false } | Cue::Warp { engaged: true } => (&self.blip_up, UI_GAIN),
            Cue::Pause { paused: true } | Cue::Warp { engaged: false } => {
                (&self.blip_down, UI_GAIN)
            }
            Cue::Reseed => (&self.chime, CHIME_GAIN),
        };
        audio::play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume,
            },
        );
    }

    /// Retarget and ease the four standing loops from this frame's sim
    /// state. The two engines each chase their own target, so a warp toggle
    /// plays as a crossfade without any crossfade code; mute pulls every
    /// target to zero, which fades the beds out instead of cutting them.
    fn ease_loops(&mut self, dt: f32, sim: &Sim) {
        let live = !self.muted;
        let traveling = matches!(sim.ship().state, ShipState::Traveling { .. });

        let engine = if live && traveling && !sim.is_warp() {
            ENGINE_GAIN
        } else {
            0.0
        };
        let warp = if live && traveling && sim.is_warp() {
            WARP_GAIN
        } else {
            0.0
        };
        // The suspicious crate hums from the moment it is stowed, and the
        // omen rides on top: at full omen the hum dominates the mix right
        // up until the jump snaps it away.
        let hum = if live && sim.suspicious_aboard() {
            HUM_SWELL.mul_add(sim.omen(), HUM_BASE)
        } else {
            0.0
        };
        let air = if live && sim.barter().is_some() {
            AIR_GAIN
        } else {
            0.0
        };

        self.engine.ease(engine, dt);
        self.engine_warp.ease(warp, dt);
        self.hum.ease(hum, dt);
        self.station_air.ease(air, dt);
    }
}

/// Whether a real press landed this frame.
///
/// This is the one place the audio module reads macroquad input directly:
/// browsers only resume the audio context inside a genuine key or mouse
/// gesture, so waking must be tied to exactly that. Movement does not
/// count — a browser will not resume audio for it, and neither do we. All
/// gameplay input still flows through `sim::InputFrame`.
fn pressed_this_frame() -> bool {
    is_mouse_button_pressed(MouseButton::Left) || get_last_key_pressed().is_some()
}

/// Map a cue intensity onto a gain multiplier.
///
/// Amplitude and perceived loudness are not the same thing: halving
/// amplitude is nothing like halving loudness. The square root pulls quiet
/// events up where they can still be heard, and the floor keeps the
/// smallest ones from vanishing entirely.
fn loudness(intensity: f32) -> f32 {
    intensity.clamp(0.0, 1.0).sqrt().mul_add(0.75, 0.25)
}

/// Hand one synthesised WAV to the audio backend.
async fn bake(wav: &[u8]) -> Sound {
    audio::load_sound_from_bytes(wav)
        .await
        .expect("synth output is a WAV the backend just accepted")
}
