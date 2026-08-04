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
