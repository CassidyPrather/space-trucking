//! The deterministic hostile network: seeded links, scripted crews, and a
//! convoy driver that plays whole voyages under fire.
//!
//! This is the testing precedent NETWORKING.md promises: real sockets are
//! a later adapter, but what CI proves — and what any protocol change must
//! keep green — lives here. A [`FlakyLink`] delays, drops, duplicates, and
//! reorders messages with every decision drawn from [`splitmix`] streams
//! off a link seed, so a run is bit-reproducible however hostile it gets.
//! A [`Convoy`] wires N [`Client`]s, one [`Helm`], and optionally a guild
//! server through those links and advances the whole fleet one virtual
//! tick per [`Convoy::step`]. [`Script`]s drive real drags to real layout
//! coordinates, so the replicas do real work while the network misbehaves.

use std::collections::BTreeMap;
use std::ops::Range;

use super::guild::{ClusterReporter, GuildServer};
use super::protocol::Message;
use super::session::{Client, Helm, INPUT_DELAY, Outbound};
use crate::sim::{CrewFrame, InputFrame, MAX_CREW, PlayerId, Sim, Vec2, layout, map, splitmix};

/// Sealed ticks per virtual step while the crew warps. Lockstep realises
/// warp by sealing faster, not by multiplying the tick — this must equal
/// [`crate::sim::WARP_FACTOR`], which a test pins.
pub const WARP_ROUNDS: u64 = 16;

/// Virtual ticks between guild report resends. Resends are free
/// (idempotent), and this is what stands between a dropped final report
/// and a lost delivery.
const REPORT_RESEND: u64 = 64;

/// Saturn: the canonical far station for the voyage (the inner ring
/// needs a transit chit, so the crate run charts outward).
const SATURN: map::PoiId = 7;

/// Seed for the canonical delivery voyage, found by search.
///
/// Saturn's first shelf offers a suspicious crate and the three starter
/// pieces cover its asking price, so [`delivery_voyage`] ends with a
/// Guild delivery.
pub const CONVOY_SEED: u64 = 0;

// ------------------------------------------------------------------ link --

/// One direction of a hostile connection. Deterministic: every decision is
/// `splitmix(seed, draw#)`, and delivery is ordered by `(deliver_at, seq)`,
/// so runs are bit-reproducible.
#[derive(Clone, Debug)]
pub struct FlakyLink {
    latency_ticks: Range<u64>,
    drop_permille: u32,
    dup_permille: u32,
    reorder: bool,
    seed: u64,
    /// Decision counter: draw `n` is `splitmix(seed, n)`, one stream for
    /// the link's whole life.
    draws: u64,
    /// `(deliver_at, seq)` → message. The map's order is delivery order.
    queue: BTreeMap<(u64, u64), Message>,
    seq: u64,
    /// Latest `deliver_at` handed out, for the FIFO clamp when `!reorder`.
    horizon: u64,
}

impl FlakyLink {
    /// A link with the given weather. `latency_ticks` is half-open, like
    /// any range; an empty range means instant delivery.
    #[must_use]
    pub const fn new(
        seed: u64,
        latency_ticks: Range<u64>,
        drop_permille: u32,
        dup_permille: u32,
        reorder: bool,
    ) -> Self {
        Self {
            latency_ticks,
            drop_permille,
            dup_permille,
            reorder,
            seed,
            draws: 0,
            queue: BTreeMap::new(),
            seq: 0,
            horizon: 0,
        }
    }

    /// A link that delivers everything, instantly, in order — the helm's
    /// loopback to its own client.
    #[must_use]
    pub const fn perfect() -> Self {
        Self::new(0, 0..1, 0, 0, false)
    }

    /// Put one message on the wire at virtual time `now`.
    pub fn send(&mut self, now: u64, msg: &Message) {
        let copies = 1 + u64::from(self.roll(1000) < u64::from(self.dup_permille));
        for _ in 0..copies {
            if self.roll(1000) < u64::from(self.drop_permille) {
                continue;
            }
            let mut at = now + self.latency();
            if !self.reorder {
                // FIFO discipline: nothing overtakes what was sent first.
                at = at.max(self.horizon);
            }
            self.horizon = self.horizon.max(at);
            self.queue.insert((at, self.seq), msg.clone());
            self.seq += 1;
        }
    }

    /// Everything due by virtual time `now`, in `(deliver_at, seq)` order.
    pub fn deliver(&mut self, now: u64) -> Vec<Message> {
        let later = self.queue.split_off(&(now + 1, 0));
        std::mem::replace(&mut self.queue, later)
            .into_values()
            .collect()
    }

    /// Messages still in flight.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether the wire is quiet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Drop everything in flight (an endpoint vanished).
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// One draw from the link's decision stream, reduced mod `bound`.
    const fn roll(&mut self, bound: u64) -> u64 {
        let draw = splitmix(self.seed, self.draws);
        self.draws += 1;
        draw % bound
    }

    /// One latency draw from the configured range.
    fn latency(&mut self) -> u64 {
        let span = self
            .latency_ticks
            .end
            .saturating_sub(self.latency_ticks.start)
            .max(1);
        self.latency_ticks.start + self.roll(span)
    }
}

/// The weather to stamp onto a set of links.
#[derive(Clone, Debug)]
pub struct LinkProfile {
    pub latency_ticks: Range<u64>,
    pub drop_permille: u32,
    pub dup_permille: u32,
    pub reorder: bool,
}

impl LinkProfile {
    /// The NETWORKING.md fixture: latency 0..8 ticks, 5% drop, 3% dup,
    /// reordering on.
    #[must_use]
    pub const fn hostile() -> Self {
        Self {
            latency_ticks: 0..8,
            drop_permille: 50,
            dup_permille: 30,
            reorder: true,
        }
    }

    /// Unpleasant but gentler weather, for tests whose choreography must
    /// survive with margin to spare.
    #[must_use]
    pub const fn mild() -> Self {
        Self {
            latency_ticks: 0..4,
            drop_permille: 20,
            dup_permille: 20,
            reorder: true,
        }
    }

    /// A flawless network (still a real link, still a queue).
    #[must_use]
    pub const fn perfect() -> Self {
        Self {
            latency_ticks: 0..1,
            drop_permille: 0,
            dup_permille: 0,
            reorder: false,
        }
    }

    /// One link under this weather, seeded for reproducibility.
    #[must_use]
    pub fn link(&self, seed: u64) -> FlakyLink {
        FlakyLink::new(
            seed,
            self.latency_ticks.clone(),
            self.drop_permille,
            self.dup_permille,
            self.reorder,
        )
    }
}

// ---------------------------------------------------------------- script --

/// A frame that presses at `p` (and holds, as a real pointer would).
#[must_use]
pub fn press_at(p: Vec2) -> InputFrame {
    InputFrame {
        pointer: p,
        press: true,
        held: true,
        ..InputFrame::default()
    }
}

/// A mid-drag frame holding at `p`.
#[must_use]
pub fn held_at(p: Vec2) -> InputFrame {
    InputFrame {
        pointer: p,
        held: true,
        ..InputFrame::default()
    }
}

/// A frame that releases at `p`.
#[must_use]
pub fn release_at(p: Vec2) -> InputFrame {
    InputFrame {
        pointer: p,
        release: true,
        ..InputFrame::default()
    }
}

/// Centre of a layout rect, for aiming at levers and slots.
#[must_use]
pub fn rect_center(rect: layout::Rect) -> Vec2 {
    Vec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y))
}

/// Centre of a hold grid cell.
#[must_use]
pub fn cell_center(x: u8, y: u8) -> Vec2 {
    rect_center(layout::cell_rect(x, y))
}

/// Centre of a barter slot.
#[must_use]
pub fn slot_center(slots: &[layout::Rect; 4], slot: usize) -> Vec2 {
    rect_center(slots[slot])
}

/// Scripted player inputs: sealed tick → per-player frames.
///
/// Ticks with no entry are the default frame, exactly like absent crew;
/// drags therefore occupy consecutive ticks (press, held, release), or
/// the sim's phantom-pointer rule snaps them home mid-flight.
#[derive(Clone, Debug, Default)]
pub struct Script {
    entries: BTreeMap<(u64, PlayerId), InputFrame>,
}

impl Script {
    /// An empty script: six crew members coasting.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set one player's frame for one sealed tick (last write wins).
    pub fn put(&mut self, tick: u64, player: PlayerId, frame: InputFrame) {
        self.entries.insert((tick, player), frame);
    }

    /// A press at `p` on `tick`.
    pub fn press(&mut self, tick: u64, player: PlayerId, p: Vec2) {
        self.put(tick, player, press_at(p));
    }

    /// A mid-drag hold at `p` on `tick`.
    pub fn hold(&mut self, tick: u64, player: PlayerId, p: Vec2) {
        self.put(tick, player, held_at(p));
    }

    /// A three-tick drag: press at `from`, hold and release at `to`.
    pub fn drag(&mut self, tick: u64, player: PlayerId, from: Vec2, to: Vec2) {
        self.put(tick, player, press_at(from));
        self.put(tick + 1, player, held_at(to));
        self.put(tick + 2, player, release_at(to));
    }

    /// A warp toggle on `tick`.
    pub fn toggle_warp(&mut self, tick: u64, player: PlayerId) {
        self.put(
            tick,
            player,
            InputFrame {
                toggle_warp: true,
                ..InputFrame::default()
            },
        );
    }

    /// A pause toggle on `tick`.
    pub fn toggle_pause(&mut self, tick: u64, player: PlayerId) {
        self.put(
            tick,
            player,
            InputFrame {
                toggle_pause: true,
                ..InputFrame::default()
            },
        );
    }

    /// One player's scripted frame for one tick, if any.
    #[must_use]
    pub fn get(&self, tick: u64, player: PlayerId) -> Option<InputFrame> {
        self.entries.get(&(tick, player)).copied()
    }

    /// Every scripted entry, in (tick, player) order.
    pub fn iter(&self) -> impl Iterator<Item = (u64, PlayerId, &InputFrame)> {
        self.entries
            .iter()
            .map(|(&(tick, player), frame)| (tick, player, frame))
    }

    /// The whole crew's frames for one tick, absent players default —
    /// exactly what the helm would seal if every input arrived on time.
    #[must_use]
    pub fn crew_frame(&self, tick: u64) -> CrewFrame {
        let mut frames = [InputFrame::default(); MAX_CREW];
        let last = (MAX_CREW - 1) as PlayerId;
        for (&(_, player), frame) in self.entries.range((tick, 0)..=(tick, last)) {
            frames[usize::from(player)] = *frame;
        }
        frames
    }

    /// Play the script straight into a sim, one [`Sim::crew_tick`] per
    /// tick — the ideal zero-latency run the lockstep session must equal.
    pub fn playout(&self, sim: &mut Sim, ticks: u64) {
        for tick in 0..ticks {
            sim.crew_tick(&self.crew_frame(tick));
        }
    }
}

/// The canonical scripted voyage for [`CONVOY_SEED`], spread across a
/// crew of `crew` players (at least 3).
///
/// Select Saturn and launch, warp through the cruise, trade the starter
/// cargo for Saturn's suspicious crate with parallel drags, stow it, fly
/// home, and let the Guild seize it. Returns the script and the sealed
/// tick by which the delivery is done.
#[must_use]
pub fn delivery_voyage(crew: usize) -> (Script, u64) {
    assert!(
        (3..=MAX_CREW).contains(&crew),
        "the voyage choreography needs 3..=6 crew"
    );
    // Roles are spread over the crew; parallel drags use role numbers that
    // stay distinct mod any allowed crew size.
    let role = |k: usize| (k % crew) as PlayerId;
    let launch = rect_center(layout::LAUNCH_LEVER);
    let accept = rect_center(layout::ACCEPT_LEVER);

    let mut script = Script::new();
    // Outbound: pick Saturn, pull the lever, warp most of the cruise. The
    // sky moves, so press coordinates and leg lengths are sampled at the
    // exact sealed ticks the presses land on — sealed tick N is applied
    // while the sim's clock reads N.
    let start = 10;
    let leg = map::leg_ticks(map::GUILD, SATURN, start + 2);
    script.press(start, role(1), map::poi_pos(SATURN, start));
    script.press(start + 2, role(0), launch);
    script.toggle_warp(start + 6, role(4));
    script.toggle_warp(start + 2 + leg - 8, role(4));
    // Docked at Saturn: crate to the take pad, the three starter pieces to
    // the give pads (two players at a time), accept, stow the crate.
    let a = start + 2 + leg + 4;
    script.drag(
        a,
        role(1),
        slot_center(&layout::SHELF_SLOTS, 0),
        slot_center(&layout::TAKE_SLOTS, 0),
    );
    script.drag(
        a,
        role(2),
        cell_center(0, 0),
        slot_center(&layout::GIVE_SLOTS, 0),
    );
    script.drag(
        a + 4,
        role(3),
        cell_center(0, 2),
        slot_center(&layout::GIVE_SLOTS, 1),
    );
    script.drag(
        a + 4,
        role(4),
        cell_center(2, 0),
        slot_center(&layout::GIVE_SLOTS, 2),
    );
    script.press(a + 8, role(0), accept);
    script.drag(
        a + 10,
        role(5),
        slot_center(&layout::RECEIVED_SLOTS, 0),
        cell_center(0, 2),
    );
    // Home: pick the Guild, launch, warp the return leg (its own length —
    // both worlds moved while the crew was trading).
    let leg_home = map::leg_ticks(SATURN, map::GUILD, a + 16);
    script.press(a + 14, role(1), map::poi_pos(map::GUILD, a + 14));
    script.press(a + 16, role(0), launch);
    script.toggle_warp(a + 20, role(4));
    script.toggle_warp(a + 16 + leg_home - 8, role(4));
    let end = a + 16 + leg_home + 6;
    (script, end)
}

// ---------------------------------------------------------------- convoy --

/// One crew member's seat in the convoy: a session client plus its two
/// link directions. Everything is public so tests can poke at it.
#[derive(Debug)]
pub struct Endpoint {
    pub client: Client,
    /// Client → helm.
    pub up: FlakyLink,
    /// Helm → client.
    pub down: FlakyLink,
    /// A vanished endpoint neither sends nor receives.
    pub connected: bool,
}

/// The whole cluster in one deterministic box.
///
/// N clients, the helm (whose own client rides a perfect loopback — the
/// helm is also a player), the scripted inputs, and the links toward one
/// guild server. [`Convoy::step`] advances one virtual tick; everything
/// stays indexable for assertions.
#[derive(Debug)]
pub struct Convoy {
    pub helm: Helm,
    pub endpoints: Vec<Endpoint>,
    pub script: Script,
    /// Complete sealed history, unwindowed — the harness's memory is
    /// unbounded on purpose so replay tests can check the whole log; the
    /// [`Helm`] itself keeps only its rolling window.
    pub sealed_log: Vec<CrewFrame>,
    /// Helm → guild server.
    pub guild_up: FlakyLink,
    /// Guild server → helm.
    pub guild_down: FlakyLink,
    /// Latest guild total heard at the helm. Display state only.
    pub progress: u64,
    reporter: ClusterReporter,
    link_seed: u64,
    /// Distinct link-seed salt for joiners and rejoiners.
    uniq: u64,
    now: u64,
}

impl Convoy {
    /// A convoy of `crew` founding clients (player 0 is the helm's own,
    /// on a perfect loopback; the rest ride `profile` links seeded from
    /// `link_seed`), playing `script` on the world `sim_seed` builds.
    #[must_use]
    pub fn new(
        sim_seed: u64,
        link_seed: u64,
        crew: usize,
        cluster: u64,
        profile: &LinkProfile,
        script: Script,
    ) -> Self {
        assert!((1..=MAX_CREW).contains(&crew), "1..=6 crew per ship");
        let genesis = Sim::new(sim_seed);
        let endpoints = (0..crew)
            .map(|i| {
                let client = Client::founding(i as PlayerId, genesis.clone());
                let (up, down) = if i == 0 {
                    (FlakyLink::perfect(), FlakyLink::perfect())
                } else {
                    (
                        profile.link(splitmix(link_seed, i as u64 * 2)),
                        profile.link(splitmix(link_seed, i as u64 * 2 + 1)),
                    )
                };
                Endpoint {
                    client,
                    up,
                    down,
                    connected: true,
                }
            })
            .collect();
        Self {
            helm: Helm::new(),
            endpoints,
            script,
            sealed_log: Vec::new(),
            guild_up: profile.link(splitmix(link_seed, 0x60)),
            guild_down: profile.link(splitmix(link_seed, 0x61)),
            progress: 0,
            reporter: ClusterReporter::new(cluster),
            link_seed,
            uniq: 0x100,
            now: 0,
        }
    }

    /// The virtual clock, in ticks.
    #[must_use]
    pub const fn now(&self) -> u64 {
        self.now
    }

    /// Ticks the helm has sealed.
    #[must_use]
    pub const fn sealed(&self) -> u64 {
        self.helm.sealed()
    }

    /// Endpoint `index`'s replica, if live.
    #[must_use]
    pub fn sim(&self, index: usize) -> Option<&Sim> {
        self.endpoints.get(index)?.client.sim()
    }

    /// Whether every connected endpoint has applied everything sealed.
    #[must_use]
    pub fn converged(&self) -> bool {
        self.endpoints
            .iter()
            .filter(|ep| ep.connected)
            .all(|ep| ep.client.is_live() && ep.client.applied() == self.helm.sealed())
    }

    /// One virtual tick: sample scripted inputs, pump the links, deadline
    /// the helm, pulse the clients. While the helm's replica warps, this
    /// seals [`WARP_ROUNDS`] ticks — warp in lockstep is sealing faster.
    pub fn step(&mut self) {
        let rounds = if self.sim(0).is_some_and(Sim::is_warp) {
            WARP_ROUNDS
        } else {
            1
        };
        for _ in 0..rounds {
            self.round(true);
        }
    }

    /// Advance to a sealed-tick target (inclusive lower bound).
    pub fn run_until_sealed(&mut self, target: u64) {
        while self.sealed() < target {
            self.step();
        }
    }

    /// Post-session catch-up: keep the links and pulses moving without
    /// sealing anything new, until everyone has applied everything or
    /// `max_rounds` runs out. Returns whether convergence was reached.
    pub fn drain(&mut self, max_rounds: u64) -> bool {
        for _ in 0..max_rounds {
            if self.converged() {
                return true;
            }
            self.round(false);
        }
        self.converged()
    }

    /// Wire a mid-session joiner for `player`; returns its endpoint index.
    /// It will knock with `Hello` and sync from a `Welcome` snapshot.
    pub fn add_joiner(&mut self, player: PlayerId, profile: &LinkProfile) -> usize {
        let index = self.endpoints.len();
        let up = profile.link(self.next_link_seed());
        let down = profile.link(self.next_link_seed());
        self.endpoints.push(Endpoint {
            client: Client::joining(player),
            up,
            down,
            connected: true,
        });
        index
    }

    /// Endpoint `index` vanishes: no goodbye, queues to and from it die
    /// with it. The helm seals defaults for its player from here on.
    pub fn vanish(&mut self, index: usize) {
        let ep = &mut self.endpoints[index];
        ep.connected = false;
        ep.up.clear();
        ep.down.clear();
    }

    /// The vanished endpoint comes back as a fresh joiner on the same
    /// player slot, with fresh links: the full snapshot sync, no state
    /// carried over.
    pub fn rejoin(&mut self, index: usize, profile: &LinkProfile) {
        let player = self.endpoints[index].client.player();
        let up = profile.link(self.next_link_seed());
        let down = profile.link(self.next_link_seed());
        self.endpoints[index] = Endpoint {
            client: Client::joining(player),
            up,
            down,
            connected: true,
        };
    }

    /// The star-topology leg: watch the helm replica's delivery tally,
    /// report growth (with a resend timer — idempotence makes resends
    /// free), and exchange messages with the guild server. Call once per
    /// [`Convoy::step`] when a guild is in play; the server itself lives
    /// outside so many convoys can share it.
    pub fn exchange_guild(&mut self, guild: &mut GuildServer) {
        let now = self.now;
        let deliveries = self.sim(0).map_or(0, Sim::deliveries);
        let report = self.reporter.watch(deliveries).or_else(|| {
            (now % REPORT_RESEND == 0)
                .then(|| self.reporter.last())
                .flatten()
        });
        if let Some(report) = report {
            // The same two-copy habit as inputs: dup-proof upstream, so
            // redundancy costs nothing and survives drops.
            self.guild_up.send(now, &report);
            self.guild_up.send(now, &report);
        }
        for msg in self.guild_up.deliver(now) {
            if let Some(progress) = guild.on_message(&msg) {
                self.guild_down.send(now, &progress);
            }
        }
        for msg in self.guild_down.deliver(now) {
            if let Message::Progress { total } = msg {
                self.progress = self.progress.max(total);
            }
        }
    }

    /// One round of the virtual tick. `seal` distinguishes live play from
    /// [`Convoy::drain`]'s catch-up rounds.
    fn round(&mut self, seal: bool) {
        if seal {
            self.sample_inputs();
        }
        self.pump();
        if seal {
            let outs = self.helm.on_tick_elapsed();
            self.route(outs, None);
            self.pump();
        }
        self.pulse_clients();
        self.pump();
        self.now += 1;
    }

    /// Each connected, live client samples the script for the tick its
    /// input would land on and schedules it `INPUT_DELAY` ahead.
    fn sample_inputs(&mut self) {
        let now = self.now;
        let script = &self.script;
        for ep in &mut self.endpoints {
            if !ep.connected {
                continue;
            }
            let target = ep.client.clock() + INPUT_DELAY;
            let Some(frame) = script.get(target, ep.client.player()) else {
                continue;
            };
            for msg in ep.client.local_input(frame) {
                // Three copies per input: the helm last-wins duplicates,
                // so redundancy is free and scripted drags survive
                // drop-happy links.
                ep.up.send(now, &msg);
                ep.up.send(now, &msg);
                ep.up.send(now, &msg);
            }
        }
    }

    /// Deliver everything due on every link, letting each delivery's
    /// replies ride the wire in the same instant, until quiet.
    fn pump(&mut self) {
        let now = self.now;
        loop {
            let mut moved = false;
            // Client → helm, via an inbox so the snapshot provider can
            // borrow the loopback replica while the helm handles mail.
            let mut inbox: Vec<(usize, Message)> = Vec::new();
            for (i, ep) in self.endpoints.iter_mut().enumerate() {
                if !ep.connected {
                    continue;
                }
                for msg in ep.up.deliver(now) {
                    inbox.push((i, msg));
                }
            }
            for (from, msg) in inbox {
                moved = true;
                let outs = {
                    // Welcome tickets come from the helm's own replica —
                    // the freshest in the fleet, thanks to the loopback.
                    let source = &self.endpoints[0].client;
                    let snapshot = || {
                        (
                            source.sim().map_or_else(String::new, Sim::save_string),
                            source.applied(),
                        )
                    };
                    self.helm.on_message(&msg, &snapshot)
                };
                self.route(outs, Some(from));
            }
            // Helm → clients.
            for ep in &mut self.endpoints {
                if !ep.connected {
                    continue;
                }
                for msg in ep.down.deliver(now) {
                    moved = true;
                    for reply in ep.client.on_message(&msg) {
                        ep.up.send(now, &reply);
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// Address the helm's outputs: broadcasts fan out to every connected
    /// endpoint (and sealed schedules enter the permanent log), replies go
    /// back where the inbound message came from.
    fn route(&mut self, outs: Vec<Outbound>, reply_to: Option<usize>) {
        let now = self.now;
        for out in outs {
            match out {
                Outbound::Broadcast(msg) => {
                    if let Message::Schedule { tick, frames } = &msg {
                        debug_assert_eq!(
                            *tick as usize,
                            self.sealed_log.len(),
                            "broadcasts happen in seal order"
                        );
                        self.sealed_log.push(*frames);
                    }
                    for ep in &mut self.endpoints {
                        if ep.connected {
                            ep.down.send(now, &msg);
                        }
                    }
                }
                Outbound::Reply(msg) => {
                    if let Some(index) = reply_to {
                        let ep = &mut self.endpoints[index];
                        if ep.connected {
                            ep.down.send(now, &msg);
                        }
                    }
                }
            }
        }
    }

    /// Everyone's session pulse; joiners knock, the starving nag.
    fn pulse_clients(&mut self) {
        let now = self.now;
        for ep in &mut self.endpoints {
            if !ep.connected {
                continue;
            }
            for msg in ep.client.on_tick_elapsed() {
                ep.up.send(now, &msg);
            }
        }
    }

    /// A fresh, never-repeating link seed for joiners and rejoiners.
    const fn next_link_seed(&mut self) -> u64 {
        self.uniq += 1;
        splitmix(self.link_seed, self.uniq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Loc, WARP_FACTOR};

    /// The wire form is the honest frame equality (no `PartialEq`).
    fn frame_wire(frame: &InputFrame) -> String {
        Message::Input {
            tick: 0,
            player: 0,
            frame: *frame,
        }
        .to_wire()
    }

    /// Cross-check every replica against endpoint 0: checkpoints agree
    /// wherever they overlap, everyone applied everything, and the final
    /// save strings are bit-identical.
    fn assert_replicas_agree(convoy: &Convoy) {
        let reference: BTreeMap<u64, u64> = convoy.endpoints[0]
            .client
            .checkpoints()
            .iter()
            .copied()
            .collect();
        assert!(
            !reference.is_empty(),
            "the run was long enough to checkpoint"
        );
        for (i, ep) in convoy.endpoints.iter().enumerate() {
            let mut last = 0;
            for &(tick, hash) in ep.client.checkpoints() {
                assert!(tick > last, "client {i} applied out of order");
                last = tick;
                assert_eq!(
                    reference.get(&tick),
                    Some(&hash),
                    "client {i} diverged at sealed tick {tick}"
                );
            }
        }
        let save = convoy.sim(0).expect("helm client is live").save_string();
        for (i, ep) in convoy.endpoints.iter().enumerate() {
            if !ep.connected {
                continue;
            }
            assert_eq!(ep.client.applied(), convoy.sealed(), "client {i} behind");
            assert_eq!(
                ep.client.sim().expect("live").save_string(),
                save,
                "client {i} final state diverged"
            );
        }
    }

    #[test]
    fn warp_rounds_pin_the_sim_factor() {
        assert!((WARP_ROUNDS as f32 - WARP_FACTOR).abs() < f32::EPSILON);
    }

    #[test]
    fn flaky_links_are_reproducible_and_fifo_without_reorder() {
        let run = |mut link: FlakyLink| {
            let mut out = Vec::new();
            for now in 0_u64..60 {
                if now < 30 {
                    link.send(now, &Message::Progress { total: now });
                }
                for msg in link.deliver(now) {
                    if let Message::Progress { total } = msg {
                        out.push(total);
                    }
                }
            }
            out
        };
        let stormy = || FlakyLink::new(42, 0..6, 100, 100, true);
        assert_eq!(run(stormy()), run(stormy()), "same seed, same weather");
        assert_ne!(
            run(stormy()),
            run(FlakyLink::new(43, 0..6, 100, 100, true)),
            "different seed, different weather"
        );
        // Without reorder, delivery order is send order even with jitter.
        let calm = run(FlakyLink::new(7, 0..6, 0, 0, false));
        assert_eq!(calm, (0..30).collect::<Vec<_>>());
        // The loopback delivers instantly and loses nothing.
        let mut loopback = FlakyLink::perfect();
        loopback.send(5, &Message::Progress { total: 1 });
        assert_eq!(loopback.len(), 1);
        assert_eq!(loopback.deliver(5).len(), 1);
        assert!(loopback.is_empty());
    }

    #[test]
    fn drop_and_dup_rates_land_near_their_dials() {
        let mut link = FlakyLink::new(1, 0..1, 200, 300, true);
        let mut got = 0_usize;
        for now in 0_u64..2000 {
            link.send(now, &Message::Progress { total: now });
            got += link.deliver(now).len();
        }
        // 1.3 copies each surviving 80%: about 1.04 per send.
        assert!(
            (1900..=2250).contains(&got),
            "{got} deliveries is far from the dials"
        );
    }

    /// The ideal playout really performs the voyage this module's tests
    /// lean on: a crate comes home and the Guild seizes it.
    #[test]
    fn the_delivery_voyage_script_delivers_solo() {
        for crew in [3, 6] {
            let (script, end) = delivery_voyage(crew);
            let mut sim = Sim::new(CONVOY_SEED);
            script.playout(&mut sim, end);
            assert_eq!(sim.deliveries(), 1, "crew of {crew}");
            assert_eq!(sim.ship().state, crate::sim::ShipState::Docked(map::GUILD));
        }
    }

    /// Property 1: six clients, divergent scripted inputs, hostile links —
    /// bit-identical state at every checkpoint and identical final saves.
    /// The delivery landing under fire is part of the fixture: inputs ride
    /// redundant copies, so the choreography survives the drop rate.
    #[test]
    fn lockstep_replicas_agree_bit_identically_under_hostile_links() {
        let (script, end) = delivery_voyage(6);
        let mut convoy = Convoy::new(CONVOY_SEED, 0x11CC, 6, 1, &LinkProfile::hostile(), script);
        convoy.run_until_sealed(end);
        assert!(convoy.drain(600), "everyone catches up once sealing stops");
        assert_replicas_agree(&convoy);
        assert_eq!(convoy.sim(0).expect("live").deliveries(), 1);
    }

    /// Property 2: any player's pause toggle lands at one canonical tick
    /// for every replica, and the sim clock freezes for exactly the span.
    #[test]
    fn a_pause_toggle_lands_on_one_canonical_tick_for_everyone() {
        let mut script = Script::new();
        script.drag(20, 2, cell_center(2, 0), cell_center(4, 2));
        script.toggle_pause(40, 3);
        script.toggle_pause(60, 1);
        script.drag(70, 1, cell_center(4, 2), cell_center(2, 0));
        let mut convoy = Convoy::new(7, 0x9A5E, 6, 1, &LinkProfile::hostile(), script);
        while convoy.sealed() < 120 {
            convoy.step();
            for (i, ep) in convoy.endpoints.iter().enumerate() {
                let applied = ep.client.applied();
                let paused = ep.client.sim().expect("founding").is_paused();
                assert_eq!(
                    paused,
                    (41..=60).contains(&applied),
                    "client {i}: pause not canonical at applied {applied}"
                );
            }
        }
        assert!(convoy.drain(400));
        assert_replicas_agree(&convoy);
        // The sealed log replays with the flips at exactly 40 and 60.
        let mut replay = Sim::new(7);
        for (tick, frames) in convoy.sealed_log.iter().enumerate() {
            replay.crew_tick(frames);
            assert_eq!(
                replay.is_paused(),
                (40..60).contains(&(tick as u64)),
                "replay: pause wrong after applying tick {tick}"
            );
        }
        // Twenty sealed ticks passed while the sim clock stood still.
        assert_eq!(replay.tick(), convoy.sealed() - 20);
        assert_eq!(
            replay.save_string(),
            convoy.sim(0).expect("live").save_string()
        );
    }

    /// Property 3 (session half): joining mid-session via Hello → Welcome
    /// snapshot converges bit-identically, and the joiner then plays.
    #[test]
    fn a_late_joiner_syncs_from_snapshot_and_converges() {
        let mut script = Script::new();
        script.drag(20, 1, cell_center(0, 0), cell_center(5, 0));
        script.drag(30, 2, cell_center(2, 0), cell_center(4, 2));
        // The joiner's own drag, well after it has synced.
        script.drag(300, 4, cell_center(5, 0), cell_center(0, 0));
        let mut convoy = Convoy::new(0xADD0, 0x70AD, 4, 1, &LinkProfile::hostile(), script);
        convoy.run_until_sealed(200);
        let joiner = convoy.add_joiner(4, &LinkProfile::hostile());
        convoy.run_until_sealed(420);
        assert!(convoy.drain(600));
        let client = &convoy.endpoints[joiner].client;
        assert!(client.is_live());
        assert!(
            client.joined_at() >= Some(200),
            "the joiner synced from a mid-session snapshot, not genesis"
        );
        assert_replicas_agree(&convoy);
        // Its drag really happened: the vial is back at (0, 0).
        let sim = convoy.sim(0).expect("live");
        assert!(
            sim.pieces()
                .iter()
                .any(|p| p.kind == crate::sim::Kind::PerfumeVial
                    && p.loc == Loc::Hold { x: 0, y: 0 }),
            "the joiner's drag must land"
        );
    }

    /// Property 4: replaying the sealed log through a fresh sim equals the
    /// live replicas, warp bursts included — warp seals in bursts of
    /// [`WARP_ROUNDS`] and stays in lockstep.
    #[test]
    fn sealed_schedule_replay_equals_live_play() {
        let (script, _) = delivery_voyage(6);
        let mut convoy = Convoy::new(CONVOY_SEED, 0x4E91, 6, 1, &LinkProfile::hostile(), script);
        let leg = map::leg_ticks(map::GUILD, SATURN, 12);
        let mut warp_bursts = 0_u64;
        while convoy.sealed() < 12 + leg {
            let before = convoy.sealed();
            convoy.step();
            let advanced = convoy.sealed() - before;
            assert!(
                advanced == 1 || advanced == WARP_ROUNDS,
                "a step seals 1 tick, or {WARP_ROUNDS} under warp"
            );
            warp_bursts += u64::from(advanced == WARP_ROUNDS);
        }
        assert!(warp_bursts > 100, "the cruise must actually seal in bursts");
        assert!(convoy.drain(600));
        assert_replicas_agree(&convoy);
        let mut replay = Sim::new(CONVOY_SEED);
        for frames in &convoy.sealed_log {
            replay.crew_tick(frames);
        }
        assert_eq!(replay.tick(), convoy.sealed());
        assert_eq!(
            replay.save_string(),
            convoy.sim(0).expect("live").save_string(),
            "sealed replay must equal live play"
        );
    }

    /// Property 5: a vanished client stalls nobody — its drag snaps home,
    /// the rest keep sealing and applying — and a rejoin syncs clean.
    #[test]
    fn a_vanished_client_stalls_nobody_and_can_rejoin() {
        let mut script = Script::new();
        script.press(20, 2, cell_center(2, 0));
        for tick in 21..=45 {
            script.hold(tick, 2, cell_center(3, 1));
        }
        let mut convoy = Convoy::new(5, 0x0FF, 6, 1, &LinkProfile::mild(), script);
        convoy.run_until_sealed(32);
        assert!(
            convoy
                .sim(0)
                .expect("live")
                .all_held()
                .any(|(player, _)| player == 2),
            "the drag is in flight when its player vanishes"
        );
        let before: Vec<u64> = convoy
            .endpoints
            .iter()
            .map(|ep| ep.client.applied())
            .collect();
        convoy.vanish(2);
        convoy.run_until_sealed(64);
        for (i, ep) in convoy.endpoints.iter().enumerate() {
            if i == 2 {
                continue;
            }
            assert!(
                ep.client.applied() > before[i] + 20,
                "client {i} stalled behind the vanished one"
            );
        }
        let sim = convoy.sim(0).expect("live");
        assert!(sim.all_held().next().is_none(), "the phantom drag lingers");
        assert!(
            sim.pieces()
                .iter()
                .any(|p| p.loc == (Loc::Hold { x: 2, y: 0 })),
            "the held piece must snap home"
        );
        convoy.rejoin(2, &LinkProfile::mild());
        convoy.run_until_sealed(140);
        assert!(convoy.drain(400));
        let back = &convoy.endpoints[2].client;
        assert!(back.is_live() && back.joined_at().is_some());
        assert_replicas_agree(&convoy);
    }

    /// The `INPUT_DELAY` continuity property: when one-way latency never
    /// exceeds the input delay and nothing is dropped, no input misses its
    /// seal, no replica ever needs a repair, and nobody falls further
    /// behind than the wire itself.
    #[test]
    fn input_delay_covers_ambient_latency_without_stalls() {
        let mut script = Script::new();
        script.drag(15, 1, cell_center(0, 0), cell_center(5, 0));
        script.drag(40, 3, cell_center(2, 0), cell_center(4, 2));
        script.drag(80, 5, cell_center(5, 0), cell_center(0, 0));
        let profile = LinkProfile {
            latency_ticks: 0..INPUT_DELAY,
            drop_permille: 0,
            dup_permille: 0,
            reorder: true,
        };
        let mut convoy = Convoy::new(3, 0xC0C0, 6, 1, &profile, script);
        while convoy.sealed() < 300 {
            convoy.step();
            for (i, ep) in convoy.endpoints.iter().enumerate() {
                assert!(
                    ep.client.applied() + INPUT_DELAY + 1 >= convoy.sealed(),
                    "client {i} fell further behind than the wire allows"
                );
            }
        }
        assert!(convoy.drain(50), "convergence within the latency bound");
        assert_eq!(convoy.helm.late_inputs(), 0, "every input made its seal");
        for (i, ep) in convoy.endpoints.iter().enumerate() {
            assert_eq!(ep.client.repairs(), 0, "client {i} needed a repair");
        }
        // Every scripted input landed verbatim on its sealed tick.
        for (tick, player, frame) in convoy.script.iter() {
            let sealed =
                convoy.sealed_log[usize::try_from(tick).expect("small")][usize::from(player)];
            assert_eq!(
                frame_wire(&sealed),
                frame_wire(frame),
                "input for tick {tick} player {player} did not land"
            );
        }
        assert_replicas_agree(&convoy);
    }
}
