//! The lockstep session: two transport-agnostic state machines.
//!
//! [`Helm`] owns the canonical input log — authoritative over *input
//! ordering only*, never over state — and [`Client`] owns a replica that
//! applies sealed ticks strictly in order. Neither knows a socket or a
//! clock: both are driven entirely by [`super::protocol::Message`]s plus an
//! explicit [`Helm::on_tick_elapsed`] / [`Client::on_tick_elapsed`] pulse
//! from whatever loop hosts them, so the harness, a future websocket
//! adapter, and a replay file all exercise exactly the same code.
//!
//! Every handler is safe under delay, drop, duplication, and reordering by
//! construction: inputs are last-wins per (tick, player) and default when
//! absent, schedules are idempotent (an already-applied tick is ignored)
//! and re-orderable (future ticks park until the gap fills), and gaps are
//! repaired by request. A replica that cannot advance stalls; it never
//! speculates.

use std::collections::{BTreeMap, VecDeque};

use super::protocol::Message;
use crate::sim::{CrewFrame, InputFrame, MAX_CREW, PlayerId, Sim, splitmix};

/// How many ticks ahead of its session clock a client schedules its own
/// inputs.
///
/// At 60 Hz this is ~133 ms of slack — generous, and free in a game this
/// slow: the schedule usually arrives before the replica needs it, so
/// play feels continuous at ambient-game latencies (the harness proves
/// continuity for one-way latencies up to 8 ticks).
pub const INPUT_DELAY: u64 = 8;

/// Sealed schedules the helm keeps for gap repair, about 4 s of session
/// time.
///
/// Enough to cover any honest re-request round trip — even a warp burst
/// (16 ticks per pulse) times the harness's worst latency — while
/// bounding memory; anyone further behind gets a fresh snapshot instead.
pub const HELM_WINDOW: usize = 256;

/// How far into the future the helm accepts an input. Honest clients aim
/// `INPUT_DELAY` ahead; anything far past that is garbage or madness and
/// is dropped rather than hoarded.
pub const INPUT_HORIZON: u64 = 64;

/// How far ahead of its next needed tick a client parks schedules. Bounds
/// memory against insane senders; honest reordering never comes close.
const PARK_HORIZON: u64 = 1024;

/// Pulses of schedule silence before a client re-requests from its exact
/// gap. Resends are free (idempotent); silence is a stall.
const REPAIR_NAG: u64 = 16;

/// How far past a hole a parked schedule must sit before the hole counts
/// as a drop. Reordering alone never spreads arrivals wider than the wire
/// jitter [`INPUT_DELAY`] is sized for, so anything staler is genuinely
/// missing — requesting sooner would spam repairs for ticks still in
/// flight, requesting only on the starve nag would stall longer than
/// needed.
const GAP_EVIDENCE: u64 = INPUT_DELAY;

/// Pulses between a joiner's `Hello` knocks until a `Welcome` lands.
const HELLO_NAG: u64 = 16;

/// Applied ticks between a client's state-hash checkpoints. Cheap enough
/// to leave always on; the harness compares them across replicas.
pub const CHECKPOINT_EVERY: u64 = 64;

/// A sealed tick where every player is absent.
fn default_frames() -> CrewFrame {
    [InputFrame::default(); MAX_CREW]
}

/// Order-sensitive fold of a save string through [`splitmix`], for cheap
/// replica-agreement checks. Divergence detection only, nothing
/// cryptographic.
#[must_use]
pub fn state_hash(save: &str) -> u64 {
    let mut hash = 0x5EED_C0DE_u64;
    for chunk in save.as_bytes().chunks(8) {
        let mut word = [0_u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        hash = splitmix(hash, u64::from_le_bytes(word));
    }
    splitmix(hash, save.len() as u64)
}

/// Where a helm-produced message should go. The state machine names the
/// intent; the driver, which knows the topology, does the addressing.
#[derive(Clone, Debug)]
pub enum Outbound {
    /// To every connected endpoint (the sealed schedule stream).
    Broadcast(Message),
    /// Back to whichever endpoint sent the message being handled.
    Reply(Message),
}

/// The input sequencer. One per cluster, co-located with an ordinary
/// [`Client`] — the helm is also a player, and its own inputs travel the
/// same `Input` → seal → `Schedule` path as everyone else's.
///
/// The helm holds no replica of its own; when a `Welcome` snapshot is
/// needed it asks the driver via the `snapshot` provider, which hands back
/// a save string plus the schedule index that save has applied through.
#[derive(Debug, Default)]
pub struct Helm {
    /// Next tick to seal; equivalently, sealed ticks so far.
    sealed: u64,
    /// Rolling window of sealed schedules, oldest first; covers ticks
    /// `sealed - log.len() .. sealed`.
    log: VecDeque<CrewFrame>,
    /// Future inputs awaiting their seal. Last-wins per (tick, player), so
    /// duplicated or re-sent inputs are harmless by construction.
    pending: BTreeMap<u64, CrewFrame>,
    /// Inputs that arrived for an already-sealed tick. Their players got
    /// the default frame instead; the count is the harness's meter for
    /// whether `INPUT_DELAY` really covers the latency in play.
    late_inputs: u64,
}

impl Helm {
    /// A helm that has sealed nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Next tick to seal — every tick below this is canonical history.
    #[must_use]
    pub const fn sealed(&self) -> u64 {
        self.sealed
    }

    /// Oldest tick still answerable from the log.
    #[must_use]
    pub fn window_start(&self) -> u64 {
        self.sealed - self.log.len() as u64
    }

    /// Inputs that arrived too late to make their tick.
    #[must_use]
    pub const fn late_inputs(&self) -> u64 {
        self.late_inputs
    }

    /// Handle one inbound message. `snapshot` is consulted only when a
    /// `Welcome` must be issued (a `Hello`, or a repair request from
    /// before the window).
    pub fn on_message(
        &mut self,
        msg: &Message,
        snapshot: &dyn Fn() -> (String, u64),
    ) -> Vec<Outbound> {
        match msg {
            Message::Hello { .. } => vec![Outbound::Reply(welcome(snapshot))],
            Message::Input {
                tick,
                player,
                frame,
            } => {
                self.collect(*tick, *player, frame);
                Vec::new()
            }
            Message::ScheduleRequest { from_tick } => self.repair(*from_tick, snapshot),
            // The schedule stream is its own output, and the guild pair is
            // the driver's business; both are ignored without fuss.
            _ => Vec::new(),
        }
    }

    /// Seal the next tick at its deadline: whatever inputs have arrived,
    /// absent players default, one `Schedule` out. The driver pulses this
    /// once per session tick — and [`crate::sim::WARP_FACTOR`] times per
    /// tick while the crew warps, which is how lockstep realises warp.
    pub fn on_tick_elapsed(&mut self) -> Vec<Outbound> {
        let tick = self.sealed;
        let frames = self.pending.remove(&tick).unwrap_or_else(default_frames);
        self.sealed += 1;
        self.log.push_back(frames);
        while self.log.len() > HELM_WINDOW {
            self.log.pop_front();
        }
        vec![Outbound::Broadcast(Message::Schedule { tick, frames })]
    }

    /// File one player's future input. Sealed ticks are immutable — a late
    /// input is counted and dropped, never spliced into history — and the
    /// far future is refused so a confused sender cannot balloon memory.
    fn collect(&mut self, tick: u64, player: PlayerId, frame: &InputFrame) {
        let slot = usize::from(player);
        if slot >= MAX_CREW {
            return;
        }
        if tick < self.sealed {
            self.late_inputs += 1;
            return;
        }
        if tick >= self.sealed + INPUT_HORIZON {
            return;
        }
        self.pending.entry(tick).or_insert_with(default_frames)[slot] = *frame;
    }

    /// Answer a gap re-request: re-issue schedules from the log, or — when
    /// the gap predates the window — a fresh snapshot, which is the honest
    /// repair for someone that far behind. Requests for the future mean
    /// the requester is merely early; the tick will broadcast when sealed.
    fn repair(&self, from_tick: u64, snapshot: &dyn Fn() -> (String, u64)) -> Vec<Outbound> {
        if from_tick >= self.sealed {
            return Vec::new();
        }
        let start = self.window_start();
        if from_tick < start {
            return vec![Outbound::Reply(welcome(snapshot))];
        }
        (from_tick..self.sealed)
            .map(|tick| {
                let frames = self.log[(tick - start) as usize];
                Outbound::Reply(Message::Schedule { tick, frames })
            })
            .collect()
    }
}

/// A `Welcome` from the driver's snapshot provider.
fn welcome(snapshot: &dyn Fn() -> (String, u64)) -> Message {
    let (save, next_tick) = snapshot();
    Message::Welcome { save, next_tick }
}

/// One crew member's session: a replica of the sim plus the discipline to
/// only ever feed it canonical, in-order sealed ticks.
#[derive(Debug)]
pub struct Client {
    player: PlayerId,
    /// The replica. `None` while joining: schedules mean nothing until a
    /// `Welcome` provides the world they apply to.
    sim: Option<Sim>,
    /// Next schedule index to apply. Deliberately not [`Sim::tick`]: a
    /// paused sealed tick applies without stepping the sim, so the
    /// schedule index runs ahead of the sim clock across pauses.
    next_tick: u64,
    /// Session clock in pulses. The driver pulses every endpoint once per
    /// sealed tick, so this tracks the helm's seal count; local inputs are
    /// scheduled [`INPUT_DELAY`] past it.
    clock: u64,
    /// Sealed ticks that arrived ahead of a gap, parked until it fills.
    parked: BTreeMap<u64, CrewFrame>,
    /// Pulses since the last applied tick, for the repair nag.
    starve: u64,
    /// One repair request per pulse, however many gaps reordering shows.
    requested_this_pulse: bool,
    /// The schedule index our first adopted `Welcome` landed us at, if we
    /// joined by snapshot rather than from the session's genesis.
    joined_at: Option<u64>,
    /// Lifetime `ScheduleRequest`s sent — the continuity test's meter.
    repairs: u64,
    /// Latest guild total heard. Display state, never enters the sim; kept
    /// with `max` because reordered `Progress` may arrive stale.
    progress: u64,
    /// (applied tick, state hash) every [`CHECKPOINT_EVERY`] ticks, for
    /// cross-replica agreement checks.
    checkpoints: Vec<(u64, u64)>,
}

impl Client {
    /// A founding crew member: present from tick 0 with the genesis sim.
    #[must_use]
    pub fn founding(player: PlayerId, sim: Sim) -> Self {
        Self {
            sim: Some(sim),
            ..Self::joining(player)
        }
    }

    /// A joiner: no replica yet; knocks with `Hello` until a `Welcome`
    /// brings the world.
    #[must_use]
    pub const fn joining(player: PlayerId) -> Self {
        Self {
            player,
            sim: None,
            next_tick: 0,
            clock: 0,
            parked: BTreeMap::new(),
            starve: 0,
            requested_this_pulse: false,
            joined_at: None,
            repairs: 0,
            progress: 0,
            checkpoints: Vec::new(),
        }
    }

    /// Which crew slot this client plays.
    #[must_use]
    pub const fn player(&self) -> PlayerId {
        self.player
    }

    /// The session clock, in pulses.
    #[must_use]
    pub const fn clock(&self) -> u64 {
        self.clock
    }

    /// Sealed ticks applied so far (also the next index to apply).
    #[must_use]
    pub const fn applied(&self) -> u64 {
        self.next_tick
    }

    /// Whether a replica exists — i.e. the join handshake is done.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.sim.is_some()
    }

    /// The replica, once live.
    #[must_use]
    pub const fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    /// Where our first `Welcome` landed us, if we joined by snapshot.
    #[must_use]
    pub const fn joined_at(&self) -> Option<u64> {
        self.joined_at
    }

    /// Lifetime `ScheduleRequest`s sent.
    #[must_use]
    pub const fn repairs(&self) -> u64 {
        self.repairs
    }

    /// Latest guild total heard (display state).
    #[must_use]
    pub const fn progress(&self) -> u64 {
        self.progress
    }

    /// (tick, state hash) checkpoints recorded so far.
    #[must_use]
    pub fn checkpoints(&self) -> &[(u64, u64)] {
        &self.checkpoints
    }

    /// Schedule this player's own input [`INPUT_DELAY`] ticks ahead, as an
    /// `Input` for the helm. Not yet live means not yet a player.
    pub fn local_input(&mut self, frame: InputFrame) -> Vec<Message> {
        if self.sim.is_none() {
            return Vec::new();
        }
        vec![Message::Input {
            tick: self.clock + INPUT_DELAY,
            player: self.player,
            frame,
        }]
    }

    /// Handle one inbound message; any returned messages go to the helm.
    pub fn on_message(&mut self, msg: &Message) -> Vec<Message> {
        match msg {
            Message::Welcome { save, next_tick } => {
                self.on_welcome(save, *next_tick);
                Vec::new()
            }
            Message::Schedule { tick, frames } => self.on_schedule(*tick, frames),
            Message::Progress { total } => {
                self.progress = self.progress.max(*total);
                Vec::new()
            }
            // Inputs, requests, hellos, and reports are helm/guild-bound;
            // a client hearing one (misrouting, malice) ignores it.
            _ => Vec::new(),
        }
    }

    /// One session pulse: advance the clock, knock if still joining, nag
    /// for repair if the schedule stream has gone quiet past honest jitter.
    pub fn on_tick_elapsed(&mut self) -> Vec<Message> {
        self.clock += 1;
        self.requested_this_pulse = false;
        if self.sim.is_none() {
            if self.clock % HELLO_NAG == 1 {
                return vec![Message::Hello {
                    player: self.player,
                }];
            }
            return Vec::new();
        }
        self.starve += 1;
        if self.starve % REPAIR_NAG == 0 {
            return self.request_repair();
        }
        Vec::new()
    }

    /// Adopt a join ticket. Idempotent and reorder-safe: a ticket that
    /// lands at or behind our replica is a duplicate and ignored; a
    /// garbled save is refused (the `Hello` nag will fetch another).
    fn on_welcome(&mut self, save: &str, next_tick: u64) {
        if self.sim.is_some() && next_tick <= self.next_tick {
            return;
        }
        let Ok(sim) = Sim::from_save(save) else {
            return;
        };
        self.sim = Some(sim);
        self.next_tick = next_tick;
        self.joined_at.get_or_insert(next_tick);
        // Re-align the input clock with the session. The ticket rode the
        // wire for an unknowable number of ticks, so aiming a full
        // `INPUT_DELAY` past its snapshot errs early: an early input waits
        // in the helm's pending window, a late one is thrown away.
        self.clock = self.clock.max(next_tick + INPUT_DELAY);
        self.parked = self.parked.split_off(&next_tick);
        self.starve = 0;
    }

    /// File one sealed tick and apply everything now contiguous. Strictly
    /// in order: an already-applied tick is a duplicate (ignored), a
    /// future tick parks until the hole in front of it fills. A parked
    /// tick more than [`GAP_EVIDENCE`] past the hole is proof of a drop —
    /// reordering never spreads wider than that — and earns one repair
    /// request per pulse. Stall, never speculate.
    fn on_schedule(&mut self, tick: u64, frames: &CrewFrame) -> Vec<Message> {
        if self.sim.is_none() || tick < self.next_tick || tick >= self.next_tick + PARK_HORIZON {
            return Vec::new();
        }
        self.parked.insert(tick, *frames);
        self.drain();
        let dropped = self
            .parked
            .last_key_value()
            .is_some_and(|(&last, _)| last >= self.next_tick + GAP_EVIDENCE);
        if dropped {
            self.request_repair()
        } else {
            Vec::new()
        }
    }

    /// Apply parked ticks while the next needed one is present.
    fn drain(&mut self) {
        let Some(sim) = self.sim.as_mut() else {
            return;
        };
        while let Some(frames) = self.parked.remove(&self.next_tick) {
            sim.crew_tick(&frames);
            self.next_tick += 1;
            self.starve = 0;
            if self.next_tick % CHECKPOINT_EVERY == 0 {
                self.checkpoints
                    .push((self.next_tick, state_hash(&sim.save_string())));
            }
        }
    }

    /// Ask the helm for everything from our gap onward, at most once per
    /// pulse — reordering can reveal the same gap many times in one burst.
    fn request_repair(&mut self) -> Vec<Message> {
        if self.requested_this_pulse {
            return Vec::new();
        }
        self.requested_this_pulse = true;
        self.repairs += 1;
        vec![Message::ScheduleRequest {
            from_tick: self.next_tick,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::TICK_DT;

    /// A frame with a recognisable fingerprint.
    fn marked(x: f32) -> InputFrame {
        InputFrame {
            pointer: crate::sim::Vec2::new(x, 1.0),
            held: true,
            ..InputFrame::default()
        }
    }

    /// The wire form is the honest equality for frames (no `PartialEq`).
    fn wire_of(tick: u64, frames: CrewFrame) -> String {
        Message::Schedule { tick, frames }.to_wire()
    }

    fn no_snapshot() -> (String, u64) {
        unreachable!("this path must not need a snapshot")
    }

    #[test]
    fn the_helm_seals_defaults_for_absent_players_and_last_wins_dupes() {
        let mut helm = Helm::new();
        let input = |tick, player, frame| Message::Input {
            tick,
            player,
            frame,
        };
        assert!(
            helm.on_message(&input(0, 2, marked(7.0)), &no_snapshot)
                .is_empty()
        );
        // A duplicate resend overwrites with itself: harmless.
        assert!(
            helm.on_message(&input(0, 2, marked(7.0)), &no_snapshot)
                .is_empty()
        );
        let outs = helm.on_tick_elapsed();
        assert_eq!(outs.len(), 1);
        let Outbound::Broadcast(Message::Schedule { tick, frames }) = &outs[0] else {
            panic!("sealing must broadcast a schedule")
        };
        assert_eq!(*tick, 0);
        let mut expected = default_frames();
        expected[2] = marked(7.0);
        assert_eq!(wire_of(0, *frames), wire_of(0, expected));
        assert_eq!(helm.sealed(), 1);
    }

    #[test]
    fn the_helm_refuses_late_and_absurd_inputs() {
        let mut helm = Helm::new();
        for _ in 0..10 {
            helm.on_tick_elapsed();
        }
        let late = Message::Input {
            tick: 4,
            player: 0,
            frame: marked(1.0),
        };
        helm.on_message(&late, &no_snapshot);
        assert_eq!(helm.late_inputs(), 1, "history is immutable");
        let absurd = Message::Input {
            tick: 10 + INPUT_HORIZON,
            player: 0,
            frame: marked(1.0),
        };
        helm.on_message(&absurd, &no_snapshot);
        assert!(helm.pending.is_empty(), "the far future is refused");
        // In-window input lands on its tick.
        let fine = Message::Input {
            tick: 12,
            player: 1,
            frame: marked(2.0),
        };
        helm.on_message(&fine, &no_snapshot);
        helm.on_tick_elapsed(); // seals 10
        helm.on_tick_elapsed(); // seals 11
        let outs = helm.on_tick_elapsed(); // seals 12
        let Outbound::Broadcast(Message::Schedule { frames, .. }) = &outs[0] else {
            panic!("sealing must broadcast")
        };
        let mut expected = default_frames();
        expected[1] = marked(2.0);
        assert_eq!(wire_of(12, *frames), wire_of(12, expected));
    }

    #[test]
    fn the_helm_repairs_from_its_window_and_snapshots_past_it() {
        let mut helm = Helm::new();
        let sealed = HELM_WINDOW as u64 + 100;
        for _ in 0..sealed {
            helm.on_tick_elapsed();
        }
        assert_eq!(helm.window_start(), sealed - HELM_WINDOW as u64);
        let snapshot = || (Sim::new(1).save_string(), sealed);
        // Inside the window: the exact missing run, oldest first.
        let request = Message::ScheduleRequest {
            from_tick: sealed - 5,
        };
        let outs = helm.on_message(&request, &snapshot);
        assert_eq!(outs.len(), 5);
        for (i, out) in outs.iter().enumerate() {
            let Outbound::Reply(Message::Schedule { tick, .. }) = out else {
                panic!("repairs are replies")
            };
            assert_eq!(*tick, sealed - 5 + i as u64);
        }
        // Before the window: a snapshot is the honest repair.
        let ancient = Message::ScheduleRequest { from_tick: 3 };
        let outs = helm.on_message(&ancient, &snapshot);
        assert_eq!(outs.len(), 1);
        assert!(
            matches!(
                &outs[0],
                Outbound::Reply(Message::Welcome { next_tick, .. }) if *next_tick == sealed
            ),
            "a pre-window gap earns a Welcome"
        );
        // The future: the requester is merely early; nothing to say.
        let early = Message::ScheduleRequest {
            from_tick: sealed + 1,
        };
        assert!(helm.on_message(&early, &snapshot).is_empty());
        // Hello earns the same ticket.
        let outs = helm.on_message(&Message::Hello { player: 3 }, &snapshot);
        assert!(matches!(&outs[0], Outbound::Reply(Message::Welcome { .. })));
    }

    #[test]
    fn a_client_applies_in_order_parks_reorders_and_requests_gaps() {
        let mut client = Client::founding(0, Sim::new(9));
        let schedule = |tick| Message::Schedule {
            tick,
            frames: default_frames(),
        };
        assert!(client.on_message(&schedule(0)).is_empty());
        assert_eq!(client.applied(), 1);
        // Tick 2 ahead of tick 1: parked in patience — a hole this young
        // is honest reordering, the missing tick is still in flight.
        assert!(client.on_message(&schedule(2)).is_empty());
        assert_eq!(client.applied(), 1, "never speculate");
        // The gap fills: both apply, strictly in order.
        assert!(client.on_message(&schedule(1)).is_empty());
        assert_eq!(client.applied(), 3);
        // Duplicates of applied history are ignored.
        assert!(client.on_message(&schedule(0)).is_empty());
        assert_eq!(client.applied(), 3);
        // A parked tick a whole GAP_EVIDENCE past the hole is proof of a
        // drop, worth exactly one request per pulse.
        let replies = client.on_message(&schedule(3 + GAP_EVIDENCE));
        assert!(matches!(
            replies.as_slice(),
            [Message::ScheduleRequest { from_tick: 3 }]
        ));
        assert!(client.on_message(&schedule(4 + GAP_EVIDENCE)).is_empty());
        // Next pulse the request may repeat...
        let _ = client.on_tick_elapsed();
        let replies = client.on_message(&schedule(5 + GAP_EVIDENCE));
        assert!(matches!(
            replies.as_slice(),
            [Message::ScheduleRequest { from_tick: 3 }]
        ));
        assert_eq!(client.repairs(), 2);
        // ...until the repair arrives and everything drains in order.
        for tick in 3..3 + GAP_EVIDENCE {
            assert!(client.on_message(&schedule(tick)).is_empty());
        }
        assert_eq!(client.applied(), 6 + GAP_EVIDENCE);
        // The sim really ran those ticks.
        assert_eq!(client.sim().map(Sim::tick), Some(6 + GAP_EVIDENCE));
    }

    #[test]
    fn a_starving_client_nags_and_a_joiner_knocks() {
        let mut client = Client::founding(0, Sim::new(9));
        let mut nags = 0;
        for _ in 0..(REPAIR_NAG * 3) {
            for msg in client.on_tick_elapsed() {
                assert!(matches!(msg, Message::ScheduleRequest { from_tick: 0 }));
                nags += 1;
            }
        }
        assert_eq!(nags, 3, "one nag per silent {REPAIR_NAG} pulses");

        let mut joiner = Client::joining(4);
        assert!(
            joiner.local_input(marked(1.0)).is_empty(),
            "not a player yet"
        );
        let mut hellos = 0;
        for _ in 0..(HELLO_NAG * 2) {
            for msg in joiner.on_tick_elapsed() {
                assert!(matches!(msg, Message::Hello { player: 4 }));
                hellos += 1;
            }
        }
        assert_eq!(hellos, 2);
    }

    #[test]
    fn a_welcome_adopts_once_and_refuses_garbage_and_stale_tickets() {
        // Build a real mid-session save to join from.
        let mut source = Sim::new(0xFACE);
        for _ in 0..10 {
            source.advance(TICK_DT, &InputFrame::default());
        }
        let save = source.save_string();

        let mut joiner = Client::joining(2);
        joiner.on_message(&Message::Welcome {
            save: "STV2\ngarbage".to_owned(),
            next_tick: 10,
        });
        assert!(!joiner.is_live(), "a garbled ticket is refused");
        joiner.on_message(&Message::Welcome {
            save: save.clone(),
            next_tick: 10,
        });
        assert!(joiner.is_live());
        assert_eq!(joiner.applied(), 10);
        assert_eq!(joiner.joined_at(), Some(10));
        // The clock re-aligns a full INPUT_DELAY past the snapshot: the
        // ticket's own travel time is unknowable, and early inputs merely
        // wait in the helm's pending window while late ones are lost.
        assert_eq!(
            joiner.clock(),
            10 + INPUT_DELAY,
            "the input clock re-aligns"
        );
        // A stale duplicate ticket cannot rewind the replica.
        joiner.on_message(&Message::Welcome { save, next_tick: 5 });
        assert_eq!(joiner.applied(), 10);
        assert_eq!(joiner.joined_at(), Some(10));
        // And it schedules inputs like anyone else now.
        let msgs = joiner.local_input(marked(3.0));
        assert!(matches!(
            msgs.as_slice(),
            [Message::Input { tick, player: 2, .. }] if *tick == 10 + 2 * INPUT_DELAY
        ));
    }

    #[test]
    fn state_hash_separates_and_respects_content_order() {
        let a = state_hash("tick 1\nseed 2\n");
        let b = state_hash("tick 2\nseed 1\n");
        let c = state_hash("tick 1\nseed 2\n");
        assert_eq!(a, c);
        assert_ne!(a, b);
        assert_ne!(state_hash(""), state_hash("\0"));
    }
}
