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

Both js files are copied verbatim so refreshing them after a `cargo update` is
a straight `cp` out of `~/.cargo/registry/src/*/<crate>/`. Upstream `audio.js`
logs `"fix"` to the console on the first click and builds one throwaway
`AudioContext` while it is at it; that is theirs, left alone deliberately.

The sound effects themselves need no credit line — `src/synth.rs` generates
them from arithmetic at startup, so there are no audio assets to track.
