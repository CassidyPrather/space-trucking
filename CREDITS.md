# Credits

Third-party and vendored assets. The project's own code is AGPL-3.0-or-later;
see [LICENSE](LICENSE). This file tracks everything that came from somewhere
else.

**The convention:** every asset gets a line here at intake — name, source URL,
author, license, date added. Art, audio, fonts, shaders, vendored js, all of
it. Thirty seconds now versus archaeology later.

| Asset | Source | Author | License | Added |
| --- | --- | --- | --- | --- |
| `web/gl.js` | miniquad's wasm loader, vendored from the miniquad 0.4.10 crate — <https://github.com/not-fl3/miniquad> | not-fl3 and contributors | MIT OR Apache-2.0 | 2026-07 |
| `web/audio.js` | quad-snd's miniquad audio plugin, vendored unmodified from the quad-snd 0.2.8 crate (`js/audio.js`) — <https://github.com/not-fl3/quad-snd> | not-fl3 and contributors | MIT OR Apache-2.0 | 2026-07 |
| `web/sapp_jsutils.js` | sapp-jsutils' miniquad plugin, vendored unmodified from the sapp-jsutils 0.1.7 crate (`js/sapp_jsutils.js`) — <https://github.com/not-fl3/sapp-jsutils> | not-fl3 and contributors | MIT OR Apache-2.0 | 2026-08 |
| `web/quad-storage.js` | quad-storage's miniquad plugin, vendored unmodified from the upstream repo's `js/quad-storage.js` at the quad-storage 0.1.3 release commit (`3760b95`; the published crate ships no js) — <https://github.com/optozorax/quad-storage> | Ilya Sheprut (optozorax) | MIT OR Apache-2.0 | 2026-08 |

The js files are copied verbatim so refreshing them after a `cargo update` is
a straight `cp` out of `~/.cargo/registry/src/*/<crate>/` — except
`web/quad-storage.js`, which the published crate omits, so it comes from the
upstream git repo at the tag-equivalent commit recorded in the crate's
`.cargo_vcs_info.json`. Upstream `audio.js` logs `"fix"` to the console on the
first click and builds one throwaway `AudioContext` while it is at it; that is
theirs, left alone deliberately. Upstream `quad-storage.js` registers plugin
version `"0.1.2"` while the quad-storage-sys 0.1.0 crate reports its version
as a packed integer (65536), so gl.js logs a harmless version-mismatch
`console.error` at startup; also theirs, also left alone.

**Bought art is referenced, not carried.** Synty's licence lets their meshes
ship inside a built game and forbids redistributing them as source, so no pack
is in this repository and none ever will be — not in git, not in LFS. What is
here is `art/manifest.toml`: a stable id, the pack it came out of, the path
inside that pack, and the digest of the bytes the line was written against, one
table per asset. That file is the intake record this convention asks for, and
it is the place to look when somebody needs to know which licences a build
carried. `docs/ART_PIPELINE.md` explains how the payload reaches a build.

The cube and checker texture under `xtask/tests/fixtures/` are this project's
own, written to prove the converter on a machine with no packs on it, and are
covered by [LICENSE](LICENSE) like the rest of the code.

The sound effects themselves need no credit line — `src/synth.rs` generates
them from arithmetic at startup, so there are no audio assets to track.
