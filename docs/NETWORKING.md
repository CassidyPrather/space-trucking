# Networking — deterministic lockstep and the guild server

Like [ART_DIRECTION.md](ART_DIRECTION.md), this file exists so the decisions
stay deliberate. The multiplayer promises in [DESIGN.md](../DESIGN.md) —
deterministic, tolerant of network failures, drop-in drop-out, six crew, a
central server that is not heavy-handed — are architecture, and this is the
architecture. Code follows this file or amends it in the same change.

## The one big decision

**Nobody ever transmits game state. Only inputs travel.**

The sim is a pure function of (seed, input schedule): every crew member runs
an identical replica, and agreement on the schedule *is* agreement on the
world. That single property buys almost everything the design doc asks for:
bandwidth is a handful of bytes per tick, any replica plus the input log is
the whole truth, a save string is a join ticket, and cheating is confined to
inputs (which we do not care about — see DESIGN.md on anti-cheat).

## Topology

```
 clients (≤6, one ship) ──► helm (host) ──► guild server (one, global)
        ▲                     │
        └── sealed schedule ──┘
```

- **Crew / cluster**: up to `MAX_CREW = 6` players crewing one ship. One
  client is the **helm** — authoritative over *input ordering only*, never
  over state. Clients send `Input(tick, player, frame)`; the helm seals each
  tick's canonical `CrewFrame` (absent players get the default frame) and
  broadcasts `Schedule(tick, frames)`. Replicas apply sealed ticks in order
  and never speculate: a gap means **stall, not diverge**.
- **Input delay**: local inputs are scheduled `INPUT_DELAY` ticks ahead, so
  the schedule usually arrives before the replica needs it and the game
  feels continuous at ambient-game latencies. This game is slow on purpose;
  generous delay is free.
- **Join**: `Hello` → `Welcome { save_string, next_tick }` — the same STV
  save the single-player game writes — then live schedules. **Leave**: the
  player's inputs become defaults; their held piece snaps home (the sim
  already does this for a vanished pointer). The helm's own departure is
  host migration: any replica can seal from the shared log (deferred, but
  nothing in the protocol prevents it).
- **Guild server**: one global process, star topology, one message pair.
  Cluster helms send `Report { cluster, deliveries }` where `deliveries` is
  the ship's own monotonic counter from the sim; the server merges with
  `max()` per cluster — **idempotent, commutative, duplicate-proof,
  reorder-proof** — and answers `Progress { total }`. The server persists
  its counters (`GLD1` line format) and restarts without loss. Nothing the
  server says enters the deterministic sim; global progress is display
  state, like the mute flag.

## Failure rules

- Messages may be delayed, dropped, duplicated, or reordered; every handler
  must be safe under all four. Sequenced schedules are re-requested by gap;
  reports and progress are idempotent by construction.
- A replica that cannot advance stalls silently (the ambient fiction: the
  ship hums along; your console catches up). Divergence is a bug by
  definition and the harness exists to prove it cannot happen.
- Determinism, pausing, saving, and fast-forward are *lockstep concerns
  too*: pause and warp toggles ride the schedule like any input; catch-up
  and fast-forward replay sealed ticks, so an offline crew's ship
  fast-forwards exactly like a solo one.

## The harness is the point

Real sockets are an adapter to write later; `net::Transport` is a trait so
that day is plumbing. What CI runs — and what any protocol change must keep
green — is the **seeded flaky harness**: N in-process endpoints joined by
links with deterministic latency, drop, duplication, and reordering drawn
from `splitmix` streams. The required properties, each a test:

1. **Lockstep determinism**: six clients, divergent local inputs, hostile
   links → bit-identical state at every sealed tick and identical final
   save strings.
2. **Pausing**: any player's pause lands at one canonical tick for all.
3. **Persistence**: join-by-snapshot mid-session converges bit-identically;
   the guild server restarts from disk without losing a delivery.
4. **Fast-forward**: sealed-schedule replay equals live stepwise play; warp
   in lockstep stays in lockstep.
5. **Drop-out**: a vanished client stalls nobody; a rejoin syncs clean.
6. **Server convergence**: duplicated, reordered delivery reports from many
   clusters sum exactly once.

## Module map

`src/net/` — pure, deterministic, macroquad-free, like `sim`:
`protocol.rs` (messages + line serialization, versioned like the save),
`session.rs` (Helm and Client state machines), `guild.rs` (the server +
`GLD1` persistence), `harness.rs` (FlakyLink + playout driver, test-facing).
`examples/convoy.rs` runs a six-client crew over hostile links and prints
the convergence trace — the "six client apps" in one command.

The shipped frontend consumes none of this yet: the slice's visible
mechanic (crates surfacing on far shelves, the Guild's hangar steal, the
delivery lamp plate) reads the sim's own counter. Wiring the console to a
live session is a later slice; the protocol it will speak is this one.
