//! Procedural sound effects, as WAV bytes.
//!
//! Every voice is generated at startup, so the toy ships no audio assets: the
//! wasm carries the few hundred bytes of arithmetic below instead of a few
//! hundred kilobytes of samples, and there is nothing to credit in
//! `CREDITS.md`. Swap in `load_sound("thump.wav")` the moment you have real
//! sound design; this exists so the template has something to play.
//!
//! Two constraints shape what is here. macroquad's audio API offers volume
//! and looping and nothing else — no pitch control — so anything that should
//! vary by pitch is baked into its own buffer, and only gain varies at
//! runtime. And on the web these bytes go through the browser's
//! `decodeAudioData`, which never reports failure back to us in a form
//! macroquad surfaces, so a malformed header hangs the loader instead of
//! erroring. The tests at the bottom guard the header for that reason.

use std::f32::consts::TAU;

/// Sample rate of every buffer.
///
/// quad-snd's native mixer runs at 44.1 kHz and nearest-neighbour resamples
/// anything else on load; matching it means that never happens.
pub const SAMPLE_RATE: u32 = 44_100;

/// Length of the looping drone. Exactly one second, which is what lets the
/// integer-hertz partials below meet cleanly at the loop point.
const DRONE_SECS: f32 = 1.0;

/// Fade applied to the head and tail of every one-shot, in seconds. A buffer
/// that starts or stops at a nonzero sample clicks.
const FADE: f32 = 0.004;

/// Fixed seed for the noise voices, so a given build always sounds the same.
const NOISE_SEED: u64 = 0x50FA_5EED;

/// Burst: a low sine dropping through its own decay, with a noise transient
/// at the front so it reads as an impact rather than a note.
#[must_use]
pub fn thump() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut phase = 0.0;
    wav(&render(0.30, |t| {
        // Sweeping a sine means integrating frequency; evaluating
        // `sin(TAU * f(t) * t)` with a moving `f` warps the phase and chirps.
        let freq = 145.0_f32.mul_add((-t * 18.0).exp(), 45.0);
        phase += TAU * freq / SAMPLE_RATE as f32;
        let body = phase.sin() * (-t * 11.0).exp();
        let transient = rng.f32().mul_add(2.0, -1.0) * (-t * 130.0).exp() * 0.35;
        (body + transient) * 0.9
    }))
}

/// Wall impacts: a short grain of lowpassed noise. Deliberately dry — this
/// one plays in bursts, and anything with a tail would smear into mush.
#[must_use]
pub fn patter() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    // Two one-pole lowpasses in series; enough to take the fizz off white
    // noise without needing a real filter design.
    let mut lp = (0.0, 0.0);
    wav(&render(0.09, |t| {
        let noise = rng.f32().mul_add(2.0, -1.0);
        lp.0 = (noise - lp.0).mul_add(0.25, lp.0);
        lp.1 = (lp.0 - lp.1).mul_add(0.25, lp.1);
        lp.1 * (-t * 55.0).exp() * 2.2
    }))
}

/// Reseed: a three-note arpeggio, one note per fifth of a second, to mark
/// "new world" rather than "something was hit".
#[must_use]
pub fn chime() -> Vec<u8> {
    // D5, A5, D6 — an open-sounding stack that needs no chord context.
    const NOTES: [(f32, f32); 3] = [(587.33, 0.0), (880.0, 0.08), (1174.66, 0.16)];
    wav(&render(0.75, |t| {
        NOTES
            .iter()
            .filter(|(_, start)| t >= *start)
            .map(|(freq, start)| {
                let age = t - start;
                // A quiet second harmonic keeps it from sounding like a test tone.
                let tone =
                    0.25_f32.mul_add((TAU * freq * 2.0 * age).sin(), (TAU * freq * age).sin());
                tone * (-age * 7.0).exp()
            })
            .sum::<f32>()
            * 0.33
    }))
}

/// Pause and unpause. `rising` picks the direction the pitch slides, so the
/// two states are distinguishable without looking at the screen.
#[must_use]
pub fn blip(rising: bool) -> Vec<u8> {
    const SECS: f32 = 0.08;
    let (from, to): (f32, f32) = if rising {
        (520.0, 880.0)
    } else {
        (880.0, 520.0)
    };
    let mut phase = 0.0;
    wav(&render(SECS, |t| {
        let freq = (to - from).mul_add(t / SECS, from);
        phase += TAU * freq / SAMPLE_RATE as f32;
        phase.sin() * (-t * 16.0).exp() * 0.5
    }))
}

/// The attract drone: a quiet held tone the frontend fades in while the
/// pointer is pulling.
///
/// Every partial is an exact integer number of hertz over an exactly
/// one-second buffer, so each one completes a whole number of cycles and the
/// end of the buffer lines up with its start. That is the whole trick to a
/// seamless loop — no crossfading required. The 55/56 Hz pair beats against
/// itself once a second, which is also seamless, and keeps the tone alive.
#[must_use]
pub fn drone() -> Vec<u8> {
    const PARTIALS: [(f32, f32); 5] = [
        (55.0, 0.50),
        (56.0, 0.38),
        (110.0, 0.22),
        (165.0, 0.10),
        (221.0, 0.05),
    ];
    wav(&render_exact(DRONE_SECS, |t| {
        let tremolo = 0.15_f32.mul_add((TAU * 3.0 * t).sin(), 0.85);
        PARTIALS
            .iter()
            .map(|(freq, gain)| (TAU * freq * t).sin() * gain)
            .sum::<f32>()
            * tremolo
            * 0.5
    }))
}

/// Render `secs` of mono audio, then fade both ends so it starts and stops
/// silently. For one-shots.
fn render(secs: f32, voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    let mut buffer = render_exact(secs, voice);
    let fade = sample_count(FADE);
    let len = buffer.len();
    for i in 0..fade.min(len / 2) {
        let gain = i as f32 / fade as f32;
        buffer[i] *= gain;
        buffer[len - 1 - i] *= gain;
    }
    buffer
}

/// Render `secs` of mono audio verbatim. For anything that loops, where a
/// fade would be an audible dip once per cycle.
fn render_exact(secs: f32, mut voice: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..sample_count(secs))
        .map(|i| voice(i as f32 / SAMPLE_RATE as f32))
        .collect()
}

/// Seconds to a whole number of samples.
// Every duration fed to this is a positive literal in this file.
#[allow(clippy::cast_sign_loss)]
fn sample_count(secs: f32) -> usize {
    (secs * SAMPLE_RATE as f32) as usize
}

/// Wrap mono `f32` samples as a 16-bit PCM WAV.
///
/// Hand-rolled because the format's uncompressed case is a 44-byte header and
/// pulling a crate in for it would be sillier than writing it out.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);

    // RIFF header: the size counts everything after this field, so the 44-byte
    // header minus the 8 bytes of "RIFF" and the size itself, plus the samples.
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes()); // rest of this chunk
    out.extend_from_slice(&1_u16.to_le_bytes()); // uncompressed PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2_u16.to_le_bytes()); // bytes per frame
    out.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in samples {
        // Clamp rather than let the cast wrap: a voice that overshoots should
        // sound squashed, not inverted.
        let scaled = sample.clamp(-1.0, 1.0) * f32::from(i16::MAX);
        out.extend_from_slice(&(scaled as i16).to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every one-shot voice, by name, for the checks that apply to all of them.
    fn one_shots() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("thump", thump()),
            ("patter", patter()),
            ("chime", chime()),
            ("blip up", blip(true)),
            ("blip down", blip(false)),
        ]
    }

    /// Pull the samples back out of a WAV the way a decoder would.
    fn decode(bytes: &[u8]) -> Vec<i16> {
        bytes[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn header_is_a_well_formed_wav() {
        // A bad header does not fail loudly on the web -- decodeAudioData just
        // never calls back, and macroquad waits for it forever. Hence a test.
        for (name, bytes) in one_shots().into_iter().chain([("drone", drone())]) {
            assert_eq!(&bytes[0..4], b"RIFF", "{name}");
            assert_eq!(&bytes[8..12], b"WAVE", "{name}");
            assert_eq!(&bytes[12..16], b"fmt ", "{name}");
            assert_eq!(&bytes[36..40], b"data", "{name}");

            let data_len = u32_at(&bytes, 40) as usize;
            assert_eq!(data_len, bytes.len() - 44, "{name} data size");
            assert_eq!(
                u32_at(&bytes, 4) as usize,
                bytes.len() - 8,
                "{name} riff size"
            );
            assert_eq!(data_len % 2, 0, "{name} truncated 16-bit sample");
            assert_eq!(u32_at(&bytes, 24), SAMPLE_RATE, "{name} sample rate");
        }
    }

    #[test]
    fn voices_are_audible_but_not_clipped_flat() {
        for (name, bytes) in one_shots().into_iter().chain([("drone", drone())]) {
            let samples = decode(&bytes);
            let peak = samples.iter().map(|s| i32::from(s.abs())).max().unwrap();
            assert!(peak > 3000, "{name} is nearly silent (peak {peak})");

            // Clipping is clamped rather than wrapped, so a voice that runs
            // too hot shows up as a pile of samples pinned at the rail.
            let pinned = samples.iter().filter(|s| s.abs() > 32_700).count();
            assert!(
                pinned * 100 < samples.len(),
                "{name} clips for {pinned} of {} samples",
                samples.len()
            );
        }
    }

    #[test]
    fn one_shots_start_and_end_silently() {
        // Anything else is an audible click at each end.
        for (name, bytes) in one_shots() {
            let samples = decode(&bytes);
            assert_eq!(samples.first(), Some(&0), "{name} starts mid-waveform");
            assert!(
                samples.last().unwrap().abs() < 32,
                "{name} ends at {}",
                samples.last().unwrap()
            );
        }
    }

    #[test]
    fn drone_loops_without_a_seam() {
        let samples = decode(&drone());
        assert_eq!(
            samples.len(),
            SAMPLE_RATE as usize,
            "drone is not exactly 1s"
        );

        // Integer-hertz partials over a one-second buffer should hand off from
        // the last sample to the first as smoothly as any interior pair does.
        let seam = i32::from(samples[0]) - i32::from(samples[samples.len() - 1]);
        let widest_interior = samples
            .windows(2)
            .map(|pair| (i32::from(pair[1]) - i32::from(pair[0])).abs())
            .max()
            .unwrap();
        assert!(
            seam.abs() <= widest_interior,
            "loop point jumps {}, more than the {widest_interior} of any real step",
            seam.abs()
        );
    }

    #[test]
    fn generation_is_reproducible() {
        // The noise voices seed a fixed RNG; two builds must agree.
        assert_eq!(thump(), thump());
        assert_eq!(patter(), patter());
    }
}
