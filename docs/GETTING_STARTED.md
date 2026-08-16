# Getting Started

Where to look when starting a new toy. Roughly three genres of toy, three
frameworks: things that move (macroquad), things that are a spreadsheet with
delusions (egui), things that are really a shader (wgpu). Pick early.

## Frameworks

- [macroquad](https://macroquad.rs/) — the default; the raylib of Rust, clean wasm story, and what this template uses
- [egui / eframe](https://github.com/emilk/egui) — for spreadsheet-with-delusions toys (Aurora-likes); starter: [eframe_template](https://github.com/emilk/eframe_template)
- [wgpu + WGSL](https://wgpu.rs/) — the "real" graphics path; learn it via [Learn wgpu](https://sotrh.github.io/learn-wgpu/)
- [Bevy](https://bevy.org/) — the community's center of gravity; heavy for toys, but the port target if one outgrows macroquad
- [bracket-lib](https://github.com/amethyst/bracket-lib) — if a toy turns out roguelike-shaped; [tutorial](https://bfnightly.bracketproductions.com/)
- [awesome-quads](https://github.com/ozkriff/awesome-quads) — ecosystem index for the macroquad/miniquad family

## 2D Assets

- [Kenney](https://kenney.nl/) — CC0, cohesive, the single best answer
- [OpenGameArt](https://opengameart.org/) — filter by license *before* browsing
- [Lospec](https://lospec.com/palette-list) — palettes
- [itch.io free assets](https://itch.io/game-assets/free) — check licenses per pack

## 3D Assets

- [Poly Haven](https://polyhaven.com/) — CC0 textures, HDRIs, models
- [ambientCG](https://ambientcg.com/) — CC0 materials
- [Quaternius](https://quaternius.com/) — CC0 low-poly

## Sounds

The template starts with none of these: `src/synth.rs` generates its effects
from arithmetic, which costs no payload and no credits line. Worth outgrowing
the moment a toy wants real sound design.

- [freesound](https://freesound.org/) — filter to CC0
- [Sonniss GDC bundles](https://sonniss.com/gameaudiogdc) — royalty-free, enormous
- [jsfxr](https://sfxr.me/) — generate retro SFX; perfect at toy scale
- [musicdsp.org](https://www.musicdsp.org/) — the old archive of DSP snippets, if you keep synthesising instead

## Fonts

- [Google Fonts](https://fonts.google.com/) — OFL

## Build Times

A Bevy toy is a five-hundred-crate graph, and two of its defaults cost more
than they look. Worth doing early on any new one, because both are one line
and neither changes what is on screen.

- **`debug = "line-tables-only"` in `[profile.dev]`.** The dev default is
  full DWARF across the whole graph, and a per-package `opt-level`
  override does not touch it. Here it was two thirds of the binary, and
  the linker read all of it again on every edit.
- **Name the engine features.** Bevy's `default` is four umbrellas — 2d,
  3d, ui, audio — so a game that cuts its geometry in code still builds a
  glTF loader, an animation player, a scene serialiser and every image
  decoder. Turn defaults off and list what the game actually draws.
  Keep `png` even with no assets: screenshot saving goes through the same
  format registry, and without it `--shot` writes an empty file and
  exits 0.

Measure before reaching for a linker. On this project the link is about
1.9 s and the stock GNU ld beat both lld and gold, so the usual advice to
install one bought nothing. `cargo build --timings` names the crates that
actually cost; the two items above were worth more than any of them.

## Verification

- [GAUNTLET.md](GAUNTLET.md) — this project's own answer to "how do you
  check a 3D scene without a human looking at it". Worth reading for the
  shape of the idea before the specifics: describe the geometry purely,
  sweep the descriptions for defect classes a screenshot cannot show, and
  keep the findings in a file the build asserts equality against. The
  timestep link below is the other half of it: a picture only reproduces
  if the clock does.

## Shaders and Techniques

- [Shadertoy](https://www.shadertoy.com/) — the ideal zero-setup shader-fidget environment
- [The Book of Shaders](https://thebookofshaders.com/) — gentler on-ramp
- [Red Blob Games](https://www.redblobgames.com/) — interactive explainers (pathfinding, hexes, procgen); also a model of the form
- [Game Programming Patterns](https://gameprogrammingpatterns.com/) — the game loop and update method chapters are this template's theory
- [Fix Your Timestep](https://gafferongames.com/post/fix_your_timestep/) — why `sim.advance()` looks the way it does

## Design Reference

- [Juice it or lose it](https://www.youtube.com/watch?v=Fy0aCDmgnxg) — why fidgets feel good
- [The Art of Screenshake](https://www.youtube.com/watch?v=AJdEqssNZ-U) — ditto, angrier
- [Grid Sage Games blog](https://www.gridsagegames.com/blog/) — Josh Ge; the open-devlog ethos
- [RogueBasin](https://www.roguebasin.com/) — the deluge of scrappy shared knowledge
- [r/roguelikedev](https://www.reddit.com/r/roguelikedev/) — Sharing Saturday and the annual tutorial event (has Rust tracks)

## Ecosystem Hooks

Not in the template. Add per toy.

- [quad-storage](https://github.com/optozorax/quad-storage) — localStorage persistence; background-tick toys compute elapsed time on load, no server needed
- [quad-url](https://github.com/optozorax/quad-url) — URL params, so seeds become shareable links
- [binaryen / wasm-opt](https://github.com/WebAssembly/binaryen) — what shrinks the wasm; CI assumes it. Name the wasm features explicitly (`build-web.sh` does) — rustc emits post-MVP instructions that older wasm-opt refuses to validate without being told they are allowed
- [Are we game yet](https://arewegameyet.rs/) — ecosystem index

## Licensing

- [CC0](https://creativecommons.org/public-domain/cc0/) — default to it and the bookkeeping mostly disappears
- Every asset gets a [CREDITS.md](../CREDITS.md) line at intake. No exceptions, including generated assets' tools where the tool requires it.
