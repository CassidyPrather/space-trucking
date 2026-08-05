# Art Direction — the 2D prototype

The purpose of this document is the same as [DESIGN_REVIEW.md](DESIGN_REVIEW.md)'s:
keep decisions deliberate. Simple graphics are fine; *undirected* graphics are
not. Everything drawn should be traceable to a rule here, and anything that
wants a different rule amends this file in the same change — silently diverging
from it is the aesthetic version of restating the ownership matrix.

The 3D/VRChat pass will not inherit these pixels, but it should inherit these
decisions: the conceit, the palette discipline, and the wear philosophy all
translate.

## The conceit

**The screen is not a UI. It is the instrument panel of an old freighter.**

Every element is a physical thing bolted to that panel: plates are riveted
metal with bevels, indicators are lamps behind glass, the levers are levers.
The star map and the destination preview are *screens* — phosphor CRTs set
into the panel — and they are the only things that get screen treatments
(glow, scanlines, sweep). Everything else is metal, worn by someone else's
decade of shipping before the player ever sat down.

Two material families, and each element belongs to exactly one:

| Family | Elements | Treatments |
| --- | --- | --- |
| **Phosphor screens** | star map, destination preview | glow, scanlines, slow sonar sweep, vignette; drawn in phosphor colors only |
| **Worn metal** | plates, hold bay, barter furniture, levers, buttons, dial | bevels (light from top-left), rivets, deterministic wear, lamp indicators |

## Pixel crunch

The whole console renders to a **400×300 target and upscales nearest-neighbor**
into the letterbox — hard pixel edges everywhere, the design doc's
"smoothing off" translated to 2D. One knob (`CRUNCH` in the renderer), remove
it and everything still draws. The version string draws outside the crunch at
native resolution: it is dev information, not part of the fiction.

## Palette

All color lives in `src/palette.rs`. **No raw `Color::new` anywhere else in
the frontend** — a unit test greps the frontend sources and fails the build
otherwise. Naming is by role, not by hue, so retuning the palette never
touches the renderer.

Starting values (tune by screenshot, in one place):

| Role | Value | Notes |
| --- | --- | --- |
| `VOID` | `#0a0d10` | space, behind everything |
| `HULL` | `#1a1f1d` | panel background between plates |
| `PLATE` | `#242b27` | riveted plate faces, green-gray metal |
| `PLATE_LIT` | `#3a443d` | bevel edge facing the light (top/left) |
| `PLATE_SHADE` | `#121614` | bevel edge away from it (bottom/right) |
| `RIVET` | `#465049` | plus a `PLATE_SHADE` pixel under each, top-left light |
| `SOCKET` | `#151a17` | inset wells: hold cells, shelf/pad slots |
| `PHOSPHOR` | `#7fd962` | CRT foreground: POI rings, routes, sweep |
| `PHOSPHOR_DIM` | `#2c4a2a` | CRT furniture: grid hints, trails, scanline tint |
| `AMBER` | `#e8a33d` | invitation and warning lamps, ETA arc, want pips |
| `BRASS` | `#b08d57` | lever handles and hardware accents |
| `LAMP_OK` | `#59c135` | ready/go lamps (accept lever, launch lever) |
| `LAMP_NO` | `#d84a35` | refusal flashes, violation glyphs |
| `EERIE` | `#8f5fd6` | the suspicious violet: crate glow, omen cast |

Cargo keeps one saturated identity hue per kind (cargo tells the story and
must stay legible at socket size), harmonized toward the palette's warmth and
listed in `palette.rs` alongside everything else.

Planet glyphs on the map are phosphor-tinted, not full-color: the CRT shows
you a *reading* of Venus, not Venus. Their identity comes from silhouette and
detail (halo, smog ring, wedge, bands), which survives monochrome — and the
destination preview may afford a slightly richer tint as the "big screen."

## Light, depth, and wear

- Light comes from the **top-left**, always. Raised things (plates, buttons,
  lever handles) are lit on top/left and shaded bottom/right; inset things
  (sockets, screens) are the reverse. No other shading exists.
- **Wear is deterministic**: scratches, scuffs, and smudges are placed by
  `splitmix` from a fixed render seed — never `rand`, never hand-placed
  per-element. Every ship is scuffed the same way every boot, and wear
  density is one constant. Subtle: alpha ≤ 0.15, or it reads as dirt on the
  player's own monitor.
- Indicators are **lamps**: a bright core pixel-dot with a soft halo, not a
  recolored rectangle. Off-lamps stay visible as dark glass.
- State changes ease; nothing teleports except the pixel-snap of the crunch
  itself. Existing juice timings stay under half a second.

## Screens (phosphor treatments)

- **Scanlines**: every other target-pixel row inside screen rects, very low
  alpha, drawn last.
- **Sweep**: one slow sonar line rotating about the map center (~20 s per
  revolution) with a short fading trail; POIs it passes brighten briefly.
  Ambient, not attention-seeking — this game is a background hum. Under
  reduced motion it parks (see Motion) and the afterglow stays off.
- **Vignette**: screen corners darken slightly; the map is a tube, not a
  viewport.
- The omen dims the *whole panel* (`sim.light()`), and adds an `EERIE` cast
  on top; screens flicker slightly before the jump. The dimming multiplies
  the palette — no element opts out.

## Motion

Every animation is one of three things, and each must know which:

- **Feedback** answers a player action or a sim event and communicates a
  change: the juice tweens, the rat's sim-driven hop, a lamp waking, the
  dial's needle easing, shakes and flashes, ship travel itself. Feedback
  finishes inside half a second (the catch-up dock pulse is the one
  sanctioned exception) and **always runs**.
- **Decoration** loops while nothing changes: the sonar sweep and its
  afterglow, star twinkle, Venus's orbiting sparkles, the Guild hexagon's
  pulse, the lit-lamp shimmer, the crate's violet breathing, the invite
  glow's and go-glows' breathing, engine flicker, the rat's tail sway, the
  crew-ghost's breathing frame, the omen's screen flicker. Decoration
  **gates on the reduced-motion flag** and freezes to a legible static
  state — the sweep parks at its zero bearing, shimmers and pulses hold
  their calm midpoint — and every gated element must still read as itself,
  just motionless. In the renderer that means idle loops take their time
  from `Scene::idle_clock` (zero under the flag); feedback reads `Juice`'s
  real clocks and never comes through it.
- **Instruction** is the onboarding ghost demonstrating an interaction:
  motion *as* meaning, a third category that runs regardless of the flag.

The flag: the web shell mirrors `prefers-reduced-motion` into
`localStorage["space-trucking/reduced-motion"]` (`"1"` while reduced,
removed otherwise), and the game reads it once at boot — honestly, a
mid-session OS toggle applies on the next load. Native builds have no shell
writing the key, so they always run full motion.

## No hue alone

No meaning may be carried by hue alone: every signal needs a second channel
— geometry, brightness, or position. The dial has its needle and break-even
notch under the colour ramp, lamps carry lit-versus-dark-glass brightness,
violation flashes carry rule glyphs, the bite mark and the mute stroke are
geometry, the row trims sit at fixed positions. The refused half of the drag
placement hint therefore wears a diagonal slash across each refused
footprint cell — and across the held piece itself — `LAMP_NO`-coloured but
shape-carried; the legal state stays a plain fill.

## Do / Don't

- **Do** draw new elements from palette roles + the two material families.
- **Do** add wear to any new metal; add scanlines to any new screen.
- **Don't** introduce text (version corner excepted), gradients that imply a
  third light source, pure black or pure white, or unweathered surfaces.
- **Don't** hand-tune a one-off color; add a palette role or reuse one.
- **Don't** animate anything faster than the juice conventions allow; the
  console must be glanceable-away-from for minutes at a time.

## Guards

- Palette purity is enforced by test (`palette.rs` scans the frontend
  sources for raw color constructors).
- Wear determinism falls under the sim's rule: hashes, not RNG state.
- The design-review checklist asks whether new visuals follow this file or
  amend it — those are the only two options.
