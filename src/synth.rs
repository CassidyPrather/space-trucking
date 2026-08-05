//! Procedural sound effects, as WAV bytes.
//!
//! The whole soundscape is synthesised at startup, so the game ships no audio
//! assets: the wasm carries the arithmetic below instead of a few hundred
//! kilobytes of samples, and there is nothing to credit in `CREDITS.md`.
//!
//! The design rule (see `DESIGN.md`) is ambience, not music: nothing here is
//! more melodic than a two-note gesture, and the register aims where Cogmind's
//! soundscape lives — deep, textural, mechanical. Four seamless one-second
//! loops carry the standing states (engine, warp engine, the suspicious hum,
//! station air) and the one-shots mark the sim's cues.
//!
//! Two constraints shape what is here. macroquad's audio API offers volume
//! and looping and nothing else — no pitch control — so anything that should
//! vary by pitch is baked into its own buffer (that is why the hull creak
//! exists three times), and only gain varies at runtime. And on the web these
//! bytes go through the browser's `decodeAudioData`, which never reports
//! failure back to us in a form macroquad surfaces, so a malformed header
//! hangs the loader instead of erroring. The tests at the bottom guard the
//! header for that reason.

use std::f32::consts::TAU;

/// Sample rate of every buffer.
///
/// quad-snd's native mixer runs at 44.1 kHz and nearest-neighbour resamples
/// anything else on load; matching it means that never happens.
pub const SAMPLE_RATE: u32 = 44_100;

/// Length of every ambient loop. Exactly one second, which is what lets the
/// integer-hertz partials below meet cleanly at the loop point.
const LOOP_SECS: f32 = 1.0;

/// Fade applied to the head and tail of every one-shot, in seconds. A buffer
/// that starts or stops at a nonzero sample clicks.
const FADE: f32 = 0.004;

/// Fixed seed for the noise voices, so a given build always sounds the same.
const NOISE_SEED: u64 = 0x50FA_5EED;

/// The traveling drone: a big old freighter heard from inside.
///
/// Every partial is an exact integer number of hertz over an exactly
/// one-second buffer, so each one completes a whole number of cycles and the
/// end of the buffer lines up with its start — the whole trick to a seamless
/// loop, no crossfading required. The 41/42 Hz pair beats once a second (also
/// seamless), which reads as machinery slowly cycling rather than a held
/// tone; the upper partials sit on the 41 Hz series and taper fast, keeping
/// the sound warm and low enough that you stop hearing it after a minute.
#[must_use]
pub fn engine() -> Vec<u8> {
    const PARTIALS: [(f32, f32); 6] = [
        (41.0, 0.50),
        (42.0, 0.38),
        (82.0, 0.24),
        (123.0, 0.11),
        (164.0, 0.06),
        (205.0, 0.03),
    ];
    wav(&render_exact(LOOP_SECS, |t| {
        // 2 Hz is slow enough to feel like idling, and an integer, so the
        // tremolo loops as cleanly as the partials do.
        let tremolo = 0.12_f32.mul_add((TAU * 2.0 * t).sin(), 0.88);
        PARTIALS
            .iter()
            .map(|(freq, gain)| (TAU * freq * t).sin() * gain)
            .sum::<f32>()
            * tremolo
            * 0.5
    }))
}

/// The engine at 16x fast-forward: the same machine, working.
///
/// Same 41/42 Hz fundamentals as [`engine`] so the ear hears one engine in
/// two states, but the energy moves up the harmonic series — the high
/// partials that idle barely touched now carry real weight — and the tremolo
/// quickens to 5 Hz (still an integer, still seamless). Brightness and strain
/// stand in for the pitch bend macroquad cannot do.
#[must_use]
pub fn engine_warp() -> Vec<u8> {
    const PARTIALS: [(f32, f32); 8] = [
        (41.0, 0.40),
        (42.0, 0.30),
        (82.0, 0.34),
        (123.0, 0.26),
        (164.0, 0.20),
        (205.0, 0.14),
        (246.0, 0.09),
        (287.0, 0.05),
    ];
    wav(&render_exact(LOOP_SECS, |t| {
        let tremolo = 0.18_f32.mul_add((TAU * 5.0 * t).sin(), 0.82);
        PARTIALS
            .iter()
            .map(|(freq, gain)| (TAU * freq * t).sin() * gain)
            .sum::<f32>()
            * tremolo
            * 0.42
    }))
}

/// The Suspicious Crate. Eerie on purpose, and pointedly not the engine.
///
/// A 66/67 Hz beat pair a musical fifth-ish above the engine's, so the two
/// drones never fuse. The partial set is the unsettling part: 132 and 198 are
/// true harmonics of 66, but 99 is not — it is the bare fifth wedged between
/// them — so the tone never resolves into a single pitch, just hangs there.
/// The 1 Hz swell keeps it breathing; the frontend turns the gain up when the
/// omen event wants it oppressive.
#[must_use]
pub fn hum() -> Vec<u8> {
    const PARTIALS: [(f32, f32); 5] = [
        (66.0, 0.50),
        (67.0, 0.34),
        (99.0, 0.20),
        (132.0, 0.12),
        (198.0, 0.08),
    ];
    wav(&render_exact(LOOP_SECS, |t| {
        let swell = 0.30_f32.mul_add((TAU * t).sin(), 0.70);
        PARTIALS
            .iter()
            .map(|(freq, gain)| (TAU * freq * t).sin() * gain)
            .sum::<f32>()
            * swell
            * 0.45
    }))
}

/// Docked room tone: air handlers plus the building's electricity.
///
/// Double-lowpassed noise (the [`tick_pick`] filter idiom, closed further
/// down) for the air, with a faint 60 Hz partial buried in it for the mains.
/// Seeded noise does not land back where it started after a second, so the
/// loop is forced: the render runs 50 ms long and the overhang is crossfaded
/// into the head, which makes the loop seam an ordinary interior step of the
/// filtered stream. The 60 Hz partial is integer-hertz and added after the
/// crossfade, so it loops on its own like every other partial in this file.
#[must_use]
pub fn station_air() -> Vec<u8> {
    const XFADE: f32 = 0.05;
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut lp = (0.0_f32, 0.0_f32);
    let mut stream = render_exact(LOOP_SECS + XFADE, |_| {
        let noise = rng.f32().mul_add(2.0, -1.0);
        lp.0 = (noise - lp.0).mul_add(0.06, lp.0);
        lp.1 = (lp.0 - lp.1).mul_add(0.06, lp.1);
        lp.1 * 2.4
    });

    let len = sample_count(LOOP_SECS);
    let (head, overhang) = stream.split_at_mut(len);
    let fade = overhang.len() as f32;
    for (i, (out, tail)) in head.iter_mut().zip(overhang.iter()).enumerate() {
        // At i = 0 this is entirely the overhang — the sample the filter
        // produced right after the loop's last one — so the seam step is a
        // step the stream really took.
        let w = i as f32 / fade;
        *out = tail.mul_add(1.0 - w, *out * w);
    }
    stream.truncate(len);

    for (i, sample) in stream.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        *sample = (TAU * 60.0 * t).sin().mul_add(0.05, *sample);
    }
    wav(&stream)
}

/// Docking: the ship setting its weight down.
///
/// A low swept-sine thump with a single damped high partial ringing out —
/// 1970 Hz is deliberately on no harmonic series in this file, which is what
/// makes it read as struck metal rather than a note.
#[must_use]
pub fn clunk() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut phase = 0.0_f32;
    wav(&render(0.4, |t| {
        // Sweeping a sine means integrating frequency; evaluating
        // `sin(TAU * f(t) * t)` with a moving `f` warps the phase and chirps.
        let freq = 105.0_f32.mul_add((-t * 15.0).exp(), 36.0);
        phase += TAU * freq / SAMPLE_RATE as f32;
        let body = phase.sin() * (-t * 8.0).exp();
        let metal = (TAU * 1970.0 * t).sin() * (-t * 16.0).exp() * 0.14;
        let transient = rng.f32().mul_add(2.0, -1.0) * (-t * 110.0).exp() * 0.3;
        (body + metal + transient) * 0.65
    }))
}

/// Undock: clamps release and air escapes.
///
/// A mechanical clack — two detuned mid partials chosen off any shared series
/// so they sound like parts, not a chord — then a short lowpassed-noise hiss
/// whose envelope opens just after the click, the way a seal lets go.
#[must_use]
pub fn latch() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut lp = (0.0_f32, 0.0_f32);
    wav(&render(0.3, |t| {
        let clack = ((TAU * 1327.0 * t)
            .sin()
            .mul_add(0.35, (TAU * 861.0 * t).sin()))
            * (-t * 70.0).exp();
        let noise = rng.f32().mul_add(2.0, -1.0);
        lp.0 = (noise - lp.0).mul_add(0.30, lp.0);
        lp.1 = (lp.0 - lp.1).mul_add(0.30, lp.1);
        let hiss = lp.1 * (-t * 12.0).exp() * (1.0 - (-t * 90.0).exp());
        clack.mul_add(0.6, hiss * 1.3)
    }))
}

/// Cargo lifted: a tiny dry tick.
///
/// Just a grain of brightish filtered noise, no tail — this fires on every
/// pickup during hold-tetris and anything bigger would nag.
#[must_use]
pub fn tick_pick() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut lp = (0.0_f32, 0.0_f32);
    wav(&render(0.06, |t| {
        let noise = rng.f32().mul_add(2.0, -1.0);
        lp.0 = (noise - lp.0).mul_add(0.45, lp.0);
        lp.1 = (lp.0 - lp.1).mul_add(0.45, lp.1);
        lp.1 * (-t * 160.0).exp() * 0.8
    }))
}

/// Cargo placed: the most tactile sound in the game, so it gets the most
/// layers.
///
/// A swept mid-frequency knock for the contact, a quiet 82 Hz body under it
/// so the crate has mass, and a noise transient so the first millisecond
/// reads as impact. This is the juice; everything else defers to it.
#[must_use]
pub fn thock() -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut phase = 0.0_f32;
    wav(&render(0.12, |t| {
        let freq = 190.0_f32.mul_add((-t * 40.0).exp(), 165.0);
        phase += TAU * freq / SAMPLE_RATE as f32;
        let knock = phase.sin() * (-t * 34.0).exp();
        let body = (TAU * 82.0 * t).sin() * (-t * 26.0).exp() * 0.5;
        let transient = rng.f32().mul_add(2.0, -1.0) * (-t * 170.0).exp() * 0.25;
        (knock + body + transient) * 0.6
    }))
}

/// Refusal: a dull low buzz.
///
/// Odd partials over 92 Hz approximate a square wave — the classic "wrong"
/// timbre — but the series stops at the seventh harmonic so it stays unlovely
/// without being harsh. The envelope snaps on and sags, like a solenoid that
/// did not engage.
#[must_use]
pub fn buzz() -> Vec<u8> {
    const PARTIALS: [(f32, f32); 4] = [(92.0, 1.0), (276.0, 0.42), (460.0, 0.24), (644.0, 0.13)];
    wav(&render(0.15, |t| {
        let env = (1.0 - (-t * 160.0).exp()) * (-t * 12.0).exp();
        PARTIALS
            .iter()
            .map(|(freq, gain)| (TAU * freq * t).sin() * gain)
            .sum::<f32>()
            * env
            * 0.4
    }))
}

/// Trade accepted: a warm swell that glides up a fifth.
///
/// One fundamental with a quiet second harmonic riding the same phase, so the
/// glide stays a single warming tone — a gesture of assent, deliberately
/// short of a melody.
#[must_use]
pub fn deal() -> Vec<u8> {
    const SECS: f32 = 0.6;
    let mut phase = 0.0_f32;
    wav(&render(SECS, |t| {
        let u = t / SECS;
        // Smoothstep easing: the glide starts and lands gently instead of
        // lurching between pitches.
        let ease = u * u * 2.0_f32.mul_add(-u, 3.0);
        let freq = 110.0_f32.mul_add(ease, 165.0);
        phase += TAU * freq / SAMPLE_RATE as f32;
        let tone = 0.35_f32.mul_add((2.0 * phase).sin(), phase.sin());
        // An arch envelope: swells in, fades out, no attack to mistake for
        // an impact.
        tone * (0.5 * TAU * u).sin() * 0.5
    }))
}

/// First hull creak: mid groan, moderate wobble. See [`groan`] for the
/// recipe; the three creaks are separate bakes because round-robin variety
/// has to be baked when the playback API has no pitch control.
#[must_use]
pub fn creak_a() -> Vec<u8> {
    groan(0.55, 1, 0.10, 0.015, 5.0, 1.0, 1.1)
}

/// Second hull creak: longer, darker, slower wobble, and a narrowed envelope
/// so it wells up from nothing mid-grain.
#[must_use]
pub fn creak_b() -> Vec<u8> {
    groan(0.7, 2, 0.05, 0.008, 3.0, 1.8, 1.6)
}

/// Third hull creak: the longest and brightest, with a fast shudder — closer
/// to straining plating than settling timber.
#[must_use]
pub fn creak_c() -> Vec<u8> {
    groan(0.8, 3, 0.16, 0.030, 7.0, 0.7, 0.9)
}

/// Shared body of the hull creaks: a stick-slip groan.
///
/// Two double-lowpass filters at different widths run over the same noise;
/// their difference is a crude bandpass, which gives the groan a center
/// without giving it a pitch. The wobble is amplitude tremolo standing in
/// for the juddering of stick-slip friction, and `shape` bends the arch
/// envelope so the three creaks rise and die differently. `salt` decorrelates
/// the noise between creaks so they do not share a fingerprint.
fn groan(
    secs: f32,
    salt: u64,
    wide: f32,
    narrow: f32,
    wobble_hz: f32,
    shape: f32,
    gain: f32,
) -> Vec<u8> {
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED ^ salt);
    let mut lo = (0.0_f32, 0.0_f32);
    let mut hi = (0.0_f32, 0.0_f32);
    wav(&render(secs, |t| {
        let u = t / secs;
        let noise = rng.f32().mul_add(2.0, -1.0);
        hi.0 = (noise - hi.0).mul_add(wide, hi.0);
        hi.1 = (hi.0 - hi.1).mul_add(wide, hi.1);
        lo.0 = (noise - lo.0).mul_add(narrow, lo.0);
        lo.1 = (lo.0 - lo.1).mul_add(narrow, lo.1);
        let band = hi.1 - lo.1;
        let wobble = 0.45_f32.mul_add((TAU * wobble_hz * t).sin(), 0.55);
        band * wobble * (0.5 * TAU * u).sin().powf(shape) * gain
    }))
}

/// The suspicious jump: the biggest sound in the game, still not loud.
///
/// A whoomph — the swept-sine idiom from [`clunk`], pitched to fall below
/// hearing rather than land on a thud — under an airy highpassed-noise swell
/// that peaks about two-thirds through, so the sound is displacement first
/// and rushing air after, like the room being somewhere else now.
#[must_use]
pub fn stinger() -> Vec<u8> {
    const SECS: f32 = 0.9;
    let mut rng = fastrand::Rng::with_seed(NOISE_SEED);
    let mut lp = 0.0_f32;
    let mut phase = 0.0_f32;
    wav(&render(SECS, |t| {
        let u = t / SECS;
        let freq = 90.0_f32.mul_add((-t * 9.0).exp(), 27.0);
        phase += TAU * freq / SAMPLE_RATE as f32;
        let body = phase.sin() * (-t * 5.0).exp();
        // Noise minus its own lowpass leaves only the airy top.
        let noise = rng.f32().mul_add(2.0, -1.0);
        lp = (noise - lp).mul_add(0.15, lp);
        // u^2(1-u) peaks at u = 2/3; the 6.75 normalizes that peak to 1.
        let air = (noise - lp) * (u * u * (1.0 - u) * 6.75) * 0.16;
        (body + air) * 0.85
    }))
}

/// Pause and warp toggles. `rising` picks the direction the pitch slides, so
/// the two states are distinguishable without looking at the screen.
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
            ("clunk", clunk()),
            ("latch", latch()),
            ("tick_pick", tick_pick()),
            ("thock", thock()),
            ("buzz", buzz()),
            ("deal", deal()),
            ("creak_a", creak_a()),
            ("creak_b", creak_b()),
            ("creak_c", creak_c()),
            ("stinger", stinger()),
            ("blip up", blip(true)),
            ("blip down", blip(false)),
            ("chime", chime()),
        ]
    }

    /// Every ambient loop, by name, for the checks that apply to all of them.
    fn loops() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("engine", engine()),
            ("engine_warp", engine_warp()),
            ("hum", hum()),
            ("station_air", station_air()),
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
        for (name, bytes) in one_shots().into_iter().chain(loops()) {
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
        for (name, bytes) in one_shots().into_iter().chain(loops()) {
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
    fn loops_close_without_a_seam() {
        for (name, bytes) in loops() {
            let samples = decode(&bytes);
            assert_eq!(
                samples.len(),
                SAMPLE_RATE as usize,
                "{name} is not exactly 1s"
            );

            // Integer-hertz partials over a one-second buffer (and, for the
            // noise loop, the head crossfade) should hand off from the last
            // sample to the first as smoothly as any interior pair does.
            let seam = i32::from(samples[0]) - i32::from(samples[samples.len() - 1]);
            let widest_interior = samples
                .windows(2)
                .map(|pair| (i32::from(pair[1]) - i32::from(pair[0])).abs())
                .max()
                .unwrap();
            assert!(
                seam.abs() <= widest_interior,
                "{name} loop point jumps {}, more than the {widest_interior} of any real step",
                seam.abs()
            );
        }
    }

    #[test]
    fn generation_is_reproducible() {
        // The noise voices seed a fixed RNG; two builds must agree.
        assert_eq!(clunk(), clunk());
        assert_eq!(latch(), latch());
        assert_eq!(tick_pick(), tick_pick());
        assert_eq!(thock(), thock());
        assert_eq!(stinger(), stinger());
        assert_eq!(station_air(), station_air());
        assert_eq!(creak_a(), creak_a());
        assert_eq!(creak_b(), creak_b());
        assert_eq!(creak_c(), creak_c());
    }
}
