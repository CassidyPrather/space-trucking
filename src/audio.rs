//! Cues in, sound out. The audio half of the frontend.
//!
//! This is the counterpart to `draw`: [`game_template::sim`] says *what*
//! happened, and everything here decides what that sounds like. The waveforms
//! themselves come from [`game_template::synth`], which stays macroquad-free
//! so it can be unit-tested; this file is the part that needs a live audio
//! context.
//!
//! ## The autoplay problem
//!
//! Browsers start every page's audio context suspended and only resume it
//! inside a real user gesture. quad-snd's `audio.js` already hooks
//! mousedown/keydown/touch to do the resuming, so one-shots fired by a click
//! are fine on their own. The looping drone is not: started at load it would
//! run silently against a suspended context and then arrive mid-note the
//! instant the page woke up. So it waits for the first press, and until then
//! the HUD says so.

use game_template::sim::{Cue, InputFrame};
use game_template::synth;
use macroquad::audio::{self, PlaySoundParams, Sound};

/// Peak gain for a burst, at full intensity.
const BURST_GAIN: f32 = 0.85;

/// Peak gain for a wall-impact grain, at full intensity.
const IMPACT_GAIN: f32 = 0.55;

/// Gain for the pause and reseed cues, which do not vary.
const UI_GAIN: f32 = 0.4;

/// Gain the attract drone rises to while the pointer is pulling.
const DRONE_GAIN: f32 = 0.3;

/// How fast the drone fades in and out, in gain per second. Stepping it
/// straight to the target would click; these are slow enough to sound like a
/// swell and fast enough to feel like a response.
const DRONE_FADE_IN: f32 = 2.5;
const DRONE_FADE_OUT: f32 = 1.2;

/// The loaded sound bank plus the state that shapes playback.
pub struct Audio {
    thump: Sound,
    patter: Sound,
    chime: Sound,
    blip_up: Sound,
    blip_down: Sound,
    drone: Sound,
    /// Current drone gain, eased toward its target every frame.
    drone_gain: f32,
    /// Whether the user has pressed something yet, so the drone may start.
    awake: bool,
    muted: bool,
}

impl Audio {
    /// Synthesise and decode the whole bank.
    ///
    /// Async because on the web each buffer goes to the browser to decode and
    /// macroquad waits frames for the result. It is a handful of frames at
    /// startup for a few tens of kilobytes of samples.
    pub async fn load() -> Self {
        Self {
            thump: bake(&synth::thump()).await,
            patter: bake(&synth::patter()).await,
            chime: bake(&synth::chime()).await,
            blip_up: bake(&synth::blip(true)).await,
            blip_down: bake(&synth::blip(false)).await,
            drone: bake(&synth::drone()).await,
            drone_gain: 0.0,
            awake: false,
            muted: false,
        }
    }

    /// One frame: handle the mute key, play everything the sim cued, and ease
    /// the drone toward whatever the pointer is asking for.
    pub fn update(&mut self, dt: f32, cues: &[Cue], input: &InputFrame, toggle_mute: bool) {
        if toggle_mute {
            self.muted = !self.muted;
        }

        // Only presses count; a browser will not resume audio for mouse
        // movement, and neither will we.
        if !self.awake
            && (toggle_mute || input.burst || input.toggle_pause || input.reseed.is_some())
        {
            self.awake = true;
            audio::play_sound(
                &self.drone,
                PlaySoundParams {
                    looped: true,
                    volume: 0.0,
                },
            );
        }

        for cue in cues {
            self.play(*cue);
        }

        let target = if self.awake && !self.muted && input.attract {
            DRONE_GAIN
        } else {
            0.0
        };
        let rate = if target > self.drone_gain {
            DRONE_FADE_IN
        } else {
            DRONE_FADE_OUT
        };
        self.drone_gain += (target - self.drone_gain).clamp(-rate * dt, rate * dt);
        audio::set_sound_volume(&self.drone, self.drone_gain);
    }

    /// Whether the drone is still waiting on a first press, which is the one
    /// audio state worth nagging the user about.
    #[must_use]
    pub const fn needs_gesture(&self) -> bool {
        !self.awake
    }

    /// HUD text for the current state.
    #[must_use]
    pub const fn status(&self) -> &'static str {
        if self.muted {
            "muted - M to unmute"
        } else if self.awake {
            "on - M to mute"
        } else {
            "press or click to start"
        }
    }

    /// Fire one cue.
    fn play(&self, cue: Cue) {
        if self.muted {
            return;
        }
        let (sound, volume) = match cue {
            Cue::Burst { intensity } => (&self.thump, BURST_GAIN * loudness(intensity)),
            Cue::Impact { intensity } => (&self.patter, IMPACT_GAIN * loudness(intensity)),
            Cue::Pause { paused: true } => (&self.blip_down, UI_GAIN),
            Cue::Pause { paused: false } => (&self.blip_up, UI_GAIN),
            Cue::Reseed => (&self.chime, UI_GAIN),
        };
        audio::play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume,
            },
        );
    }
}

/// Map a cue intensity onto a gain multiplier.
///
/// Amplitude and perceived loudness are not the same thing: halving amplitude
/// is nothing like halving loudness. The square root pulls quiet events up
/// where they can still be heard, and the floor keeps the smallest ones from
/// vanishing entirely.
fn loudness(intensity: f32) -> f32 {
    intensity.clamp(0.0, 1.0).sqrt().mul_add(0.75, 0.25)
}

/// Hand one synthesised WAV to the audio backend.
async fn bake(wav: &[u8]) -> Sound {
    audio::load_sound_from_bytes(wav)
        .await
        .expect("synth output is a WAV the backend just accepted")
}
