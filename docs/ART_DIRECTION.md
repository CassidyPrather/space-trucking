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
  Ambient, not attention-seeking — this game is a background hum.
- **Vignette**: screen corners darken slightly; the map is a tube, not a
  viewport.
- The omen dims the *whole panel* (`sim.light()`), and adds an `EERIE` cast
  on top; screens flicker slightly before the jump. The dimming multiplies
  the palette — no element opts out.

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
