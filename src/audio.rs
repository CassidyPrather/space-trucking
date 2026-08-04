//! Cues in, sound out — currently a silent shell.
//!
//! The real audio half of the frontend maps [`space_trucking::sim::Cue`]s
//! onto synthesised voices and eases the ambient loops; that lands with the
//! audio stage. This placeholder keeps the frontend plumbing shaped right —
//! mute key, per-frame update, and the browser's first-gesture rule — while
//! playing nothing at all. It deliberately does not touch `synth`, which is
//! being rewritten in parallel.

use space_trucking::sim::Cue;

/// Placeholder audio state: tracks mute and wakefulness, plays nothing.
pub struct Audio {
    /// Whether the player has muted.
    #[allow(dead_code)] // Read again once the audio stage lands.
    muted: bool,
    /// Whether a real user gesture has arrived. Browsers keep the audio
    /// context suspended until one does, so anything looping must wait.
    awake: bool,
}

impl Audio {
    /// Build the (empty) sound bank. The real loader awaits browser-side
    /// decodes, so the signature stays async.
    #[allow(clippy::unused_async)] // Signature contract with the audio stage.
    pub async fn load() -> Self {
        Self {
            muted: false,
            awake: false,
        }
    }

    /// One frame: track the mute key and wakefulness. Cue playback and loop
    /// easing arrive with the audio stage.
    #[allow(clippy::missing_const_for_fn)] // The real update will play sounds.
    pub fn update(&mut self, _dt: f32, _cues: &[Cue], toggle_mute: bool) {
        if toggle_mute {
            self.muted = !self.muted;
            self.awake = true;
        }
    }

    /// Whether audio still waits on a first gesture, which is the one audio
    /// state worth signalling to the player.
    #[must_use]
    pub const fn needs_gesture(&self) -> bool {
        !self.awake
    }
}
