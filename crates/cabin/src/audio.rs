//! Cues in, sound out. The audio half of the cabin.
//!
//! This is the counterpart to the view modules: [`space_trucking::sim`] says
//! *what* happened, and everything here decides what that sounds like. The
//! waveforms come from [`space_trucking::synth`], which stays engine-free so
//! it can be unit-tested; this file is the part that needs Bevy's mixer.
//!
//! A faithful port of the 2D console's `src/audio.rs`: the same voices, the
//! same gain table, the same loop targets and fade rates, read from the same
//! sim accessors. The one deliberate difference is the autoplay gate: the 2D
//! console runs in browsers, whose audio contexts start suspended until a
//! real user gesture, so it holds every sound until the first press. A
//! native window has no such rule, so the four ambient loops start looping
//! at volume zero on the first frame and simply ease up from there.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use space_trucking::sim::{Cue, ShipState};
use space_trucking::synth;

use crate::{Phase, Shell};

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

/// Peak gain for the burner taking a feeding, scaled by [`loudness`] of
/// the flame it buys: slag goes down as a quiet swallow, upholstery
/// lands like a furnace door. On the stoker's slow beat, so it may
/// carry a little weight without nagging.
const BURN_GAIN: f32 = 0.42;

/// The hopper overflow tipped over the side at dock.
const JETTISON_GAIN: f32 = 0.3;

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

/// Which of the four standing loops an [`Ambient`] entity is, so the ease
/// pass can match each entity to its frame target.
#[derive(Clone, Copy)]
enum Bed {
    Engine,
    EngineWarp,
    Hum,
    StationAir,
}

/// One ambient loop entity: which bed it is, its current eased gain, and
/// its fade rates. Bevy's `AudioSink` (added once playback starts) is the
/// mixer half; this component is the state half.
#[derive(Component)]
struct Ambient {
    bed: Bed,
    /// Current gain, eased toward the frame's target every update.
    gain: f32,
    /// Gain per second while rising toward the target.
    fade_in: f32,
    /// Gain per second while falling toward the target.
    fade_out: f32,
}

impl Ambient {
    /// Move one frame's worth toward `target`. Stepping straight to the
    /// target would click; these rates are slow enough to sound like
    /// machinery, fast enough to feel causal.
    fn ease(&mut self, target: f32, dt: f32) {
        let rate = if target > self.gain {
            self.fade_in
        } else {
            self.fade_out
        };
        let step = rate * dt;
        self.gain += (target - self.gain).clamp(-step, step);
    }
}

/// The synthesised one-shot bank, plus the creak cursor.
#[derive(Resource)]
struct SoundBank {
    clunk: Handle<AudioSource>,
    latch: Handle<AudioSource>,
    tick_pick: Handle<AudioSource>,
    thock: Handle<AudioSource>,
    buzz: Handle<AudioSource>,
    deal: Handle<AudioSource>,
    /// The three hull-creak bakes, played round-robin.
    creaks: [Handle<AudioSource>; 3],
    stinger: Handle<AudioSource>,
    blip_up: Handle<AudioSource>,
    blip_down: Handle<AudioSource>,
    chime: Handle<AudioSource>,
    /// Which creak plays next. Cosmetic variety, so it lives out here
    /// rather than in the deterministic sim.
    next_creak: usize,
}

/// The soundscape: synthesises the bank at startup, then plays cues and
/// eases the four standing loops every View phase.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, setup)
            .add_systems(Update, update.in_set(Phase::View));
    }
}

/// Synthesise the whole bank into audio assets and start the four loops.
fn setup(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(SoundBank {
        clunk: bake(&mut sources, synth::clunk()),
        latch: bake(&mut sources, synth::latch()),
        tick_pick: bake(&mut sources, synth::tick_pick()),
        thock: bake(&mut sources, synth::thock()),
        buzz: bake(&mut sources, synth::buzz()),
        deal: bake(&mut sources, synth::deal()),
        creaks: [
            bake(&mut sources, synth::creak_a()),
            bake(&mut sources, synth::creak_b()),
            bake(&mut sources, synth::creak_c()),
        ],
        stinger: bake(&mut sources, synth::stinger()),
        blip_up: bake(&mut sources, synth::blip(true)),
        blip_down: bake(&mut sources, synth::blip(false)),
        chime: bake(&mut sources, synth::chime()),
        next_creak: 0,
    });

    // The four beds loop from boot, silent until the ease pass raises
    // them. Native audio contexts are live immediately, so no gesture
    // gating here; web builds will need the first-gesture gate.
    let beds = [
        (
            synth::engine(),
            Bed::Engine,
            ENGINE_FADE_IN,
            ENGINE_FADE_OUT,
        ),
        (
            synth::engine_warp(),
            Bed::EngineWarp,
            ENGINE_FADE_IN,
            ENGINE_FADE_OUT,
        ),
        (synth::hum(), Bed::Hum, HUM_FADE_IN, HUM_FADE_OUT),
        (
            synth::station_air(),
            Bed::StationAir,
            AIR_FADE_IN,
            AIR_FADE_OUT,
        ),
    ];
    for (wav, bed, fade_in, fade_out) in beds {
        commands.spawn((
            AudioPlayer::new(bake(&mut sources, wav)),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                ..default()
            },
            Ambient {
                bed,
                gain: 0.0,
                fade_in,
                fade_out,
            },
        ));
    }
}

/// One frame, in the 2D console's order: mute, then cues, then the loops.
/// All sim state is read fresh here every frame — nothing is cached across
/// frames, so a reseed (which drops warp, undocks, everything) simply
/// retargets the loops.
fn update(
    mut commands: Commands,
    time: Res<Time>,
    shell: Res<Shell>,
    mut bank: ResMut<SoundBank>,
    mut beds: Query<(&mut Ambient, Option<&mut AudioSink>)>,
) {
    let sim = &shell.bridge.sim;

    // Mute suppresses one-shots entirely, same as the 2D `play`.
    if !shell.muted {
        for cue in sim.cues() {
            play(&mut commands, &mut bank, *cue);
        }
    }

    // Retarget and ease the four standing loops from this frame's sim
    // state. The two engines each chase their own target, so a warp toggle
    // plays as a crossfade without any crossfade code; mute pulls every
    // target to zero, which fades the beds out instead of cutting them.
    let live = !shell.muted;
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

    let dt = time.delta_secs();
    for (mut ambient, sink) in &mut beds {
        let target = match ambient.bed {
            Bed::Engine => engine,
            Bed::EngineWarp => warp,
            Bed::Hum => hum,
            Bed::StationAir => air,
        };
        // The sink only exists once playback has actually started; until
        // then there is no mixer to push to, so hold the gain where it is.
        if let Some(mut sink) = sink {
            ambient.ease(target, dt);
            sink.set_volume(Volume::Linear(ambient.gain));
        }
    }
}

/// Fire one cue on its voice, as a one-shot entity that despawns itself
/// when the sample ends.
fn play(commands: &mut Commands, bank: &mut SoundBank, cue: Cue) {
    let (sound, volume) = match cue {
        Cue::Select => (bank.tick_pick.clone(), SELECT_GAIN),
        Cue::Depart => (bank.latch.clone(), DEPART_GAIN),
        Cue::Arrive => (bank.clunk.clone(), ARRIVE_GAIN),
        Cue::Pickup => (bank.tick_pick.clone(), PICKUP_GAIN),
        Cue::Place => (bank.thock.clone(), PLACE_GAIN),
        Cue::Reject { hard: false } => (bank.buzz.clone(), REJECT_SOFT_GAIN),
        Cue::Reject { hard: true } => (bank.buzz.clone(), REJECT_HARD_GAIN),
        Cue::Refuse => (bank.buzz.clone(), REFUSE_GAIN),
        Cue::Accept { value } => (bank.deal.clone(), ACCEPT_GAIN * loudness(value)),
        Cue::Jump => (bank.stinger.clone(), JUMP_GAIN),
        Cue::Delivered => (bank.stinger.clone(), DELIVERED_GAIN),
        // No one-shot: the omen is the hum swelling, and marking its
        // edges with a sting would give the mystery away.
        Cue::OmenStart | Cue::OmenEnd => return,
        // Shutters slamming: the latch, hard. The message lands.
        Cue::Shutter => (bank.latch.clone(), SHUTTER_GAIN),
        // Something pulled alongside; something fell astern.
        Cue::EncounterStart => (bank.blip_up.clone(), ENCOUNTER_GAIN),
        Cue::EncounterEnd => (bank.blip_down.clone(), ENCOUNTER_GAIN),
        // The gas station's inexplicable generosity.
        Cue::GasBoost => (bank.latch.clone(), GAS_GAIN),
        // The casino's two moods.
        Cue::CasinoWin => (bank.deal.clone(), CASINO_WIN_GAIN),
        Cue::CasinoLoss => (bank.buzz.clone(), CASINO_LOSS_GAIN),
        // The whale, through the hull. The creak bank at whale scale.
        Cue::WhaleSong { intensity } => (bank.creaks[0].clone(), WHALE_GAIN * loudness(intensity)),
        // Ads. Ads ads ads. Then, mercifully, not.
        Cue::AdStart => (bank.buzz.clone(), AD_GAIN),
        Cue::AdSwat => (bank.thock.clone(), AD_SWAT_GAIN),
        Cue::AdEnd => (bank.blip_down.clone(), AD_GAIN),
        // A very soft pop, like a second yawn.
        Cue::FluffBirth => (bank.tick_pick.clone(), FLUFF_GAIN),
        // The furnace door: iron takes the piece, the fire answers in
        // the gain. Slag barely murmurs; a couch really goes.
        Cue::Burn { intensity } => (bank.clunk.clone(), BURN_GAIN * loudness(intensity)),
        // Dock overflow tipping over the side: a low latch, cargo going
        // its own way.
        Cue::Jettison => (bank.latch.clone(), JETTISON_GAIN),
        // The hangar opens. Whatever it was for, it is happening.
        Cue::ParadeStart => (bank.stinger.clone(), PARADE_GAIN),
        // Free cargo thunking into the hold, scaled by the haul.
        Cue::Harvest { intensity } => (bank.thock.clone(), HARVEST_GAIN * loudness(intensity)),
        // The exchange gets the stinger, quiet: ??? is not loud.
        Cue::Exchange => (bank.stinger.clone(), EXCHANGE_GAIN),
        Cue::Creak { intensity } => {
            let creak = bank.creaks[bank.next_creak].clone();
            bank.next_creak = (bank.next_creak + 1) % bank.creaks.len();
            (creak, CREAK_GAIN * loudness(intensity))
        }
        Cue::RatAboard => {
            let creak = bank.creaks[bank.next_creak].clone();
            bank.next_creak = (bank.next_creak + 1) % bank.creaks.len();
            (creak, RAT_ABOARD_GAIN)
        }
        Cue::RatSkitter { intensity } => (
            bank.tick_pick.clone(),
            RAT_SKITTER_GAIN * loudness(intensity),
        ),
        Cue::RatNibble => (bank.buzz.clone(), RAT_NIBBLE_GAIN),
        Cue::RatChased => (bank.tick_pick.clone(), RAT_CHASED_GAIN),
        Cue::RatLeft => (bank.latch.clone(), RAT_LEFT_GAIN),
        Cue::Pause { paused: false } | Cue::Warp { engaged: true } => {
            (bank.blip_up.clone(), UI_GAIN)
        }
        Cue::Pause { paused: true } | Cue::Warp { engaged: false } => {
            (bank.blip_down.clone(), UI_GAIN)
        }
        Cue::Reseed => (bank.chime.clone(), CHIME_GAIN),
    };
    commands.spawn((
        AudioPlayer::new(sound),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
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

/// Hand one synthesised WAV to the asset store as a playable source.
fn bake(sources: &mut Assets<AudioSource>, wav: Vec<u8>) -> Handle<AudioSource> {
    sources.add(AudioSource { bytes: wav.into() })
}
