# Telemetry — measurement with consent

DESIGN.md says to prioritize telemetry *and* to figure out informed consent
before letting people play. This file is the contract for both, in the
house pattern: code follows it or amends it in the same change. The rules
exist to be over-cautious — an ambient game for hangout nights lives or
dies on trust.

## Principles

1. **Opt-in, default off.** No consent recorded means no telemetry: nothing
   aggregated, nothing written, and any previously stored buffer is
   deleted on the next boot. Declining is a first-class choice, visually
   equal to accepting, and the game is identical either way.
2. **Consent is meta-UI.** The game itself stays wordless; the consent card
   lives in the HTML shell (`web/index.html`), where translatable text is
   legitimate (`lang`-tagged, minimal, plain). It appears once, before
   first play; the choice persists; clearing site data re-asks. Native
   builds have no card and default to off.
3. **No PII, ever.** Nothing identifying, nothing free-form: no names, no
   IPs stored by us, no user agents, no pointer coordinates, no timings
   precise enough to fingerprint. Aggregates and counts only.
4. **Derived from cues, never from the sim.** The aggregator consumes the
   same `Cue` stream and coarse public state the audio does. It cannot
   touch determinism: no telemetry code is reachable from `src/sim/` or
   `src/net/`.
5. **Local-first and inspectable.** The buffer is a human-readable `TLM1`
   string under one storage key. Anyone can read exactly what would be
   shared, because it is sitting in their own storage in plain lines.
6. **No endpoint exists.** Nothing is transmitted anywhere today. When an
   upload lands, it must be: same-origin, batched, fire-and-forget,
   contents byte-identical to the local buffer, and documented here first
   — in that order.

## What is counted (the whole schema)

Session aggregates, all of them coarse:

| Field | Meaning |
| --- | --- |
| `sessions` | boots with consent present |
| `seconds`, `travel_seconds`, `docked_seconds`, `warp_seconds`, `paused_seconds` | coarse time in each state |
| `legs` | departures |
| `arrivals` | dockings |
| `places`, `pickups` | cargo handling volume |
| `rejects_soft`, `rejects_hard` | friction signals (hard = a stowage rule) |
| `trades`, `gifts` | accepts, split by whether the room answered with goods |
| `trade_value[4]` | accepts bucketed by generosity quartile |
| `refusals` | lever pulls the station declined |
| `deliveries` | hangar deliveries |
| `omens`, `jumps` | event exposure |
| `reseeds` | fresh runs started |
| `catch_up_seconds` | absence replayed at boot |

Anything not in this table is not collected. Adding a row is a design
review question ("what decision would this number change?"), answered in
this file.

## Storage

- `space-trucking/telemetry-consent` — `"yes"` / `"no"`.
- `space-trucking/telemetry` — the `TLM1` buffer, merged across sessions
  (plain summed counters; merging is associative and commutative so a
  future upload can batch buffers without double-count anxiety).
