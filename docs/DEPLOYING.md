Releases ship native binaries: publishing a GitHub Release triggers CI's
`build-and-publish` job, which builds the cabin (`-p cabin`, binary name
`space-trucking`) for Linux x86_64, macOS aarch64, and Windows x86_64 and
attaches the archives to the release.

The retired 2D console's web build (wasm + GitHub Pages) left with it —
see docs/BAY.md for the decision. If a web target returns (Bevy compiles
to wasm), the old `build-web.sh` + Pages pipeline in git history is the
reference for the shape: a folder of files, no external requests.

## The browser target

The cabin compiles for wasm32 today, and CI checks that it keeps doing
so:

```bash
cargo check --target wasm32-unknown-unknown -p cabin
```

Nothing goes on the command line: `crates/cabin/Cargo.toml` carries a
wasm-only dependency table (Bevy's `web` and `webgl2` features, chrono's
`wasmbind`) that cargo unions in for that target and ignores for every
other, so a native build's graph is untouched. The sim library was the
retired console's wasm payload and never stopped being one.

That is the compile. Between it and a page someone can open:

- **A loader.** Bevy drives the page through wasm-bindgen's generated
  JavaScript, so the build needs `wasm-bindgen` (the CLI's version must
  match the crate the lock file pins) and `wasm-opt`, and a page that
  calls what they emit. The retired console's `web/index.html` is a
  macroquad shell and does not carry over.
- **Persistence.** Saves and the replay tape are `std::fs` writes, which
  fail silently in a browser: the game runs, nothing survives a reload.
- **The art seam.** The index is read through `std::fs` too, so a
  browser build draws the whitebox even with converted meshes beside it.
  Routing the index and the meshes through the asset server is what a
  dressed web build needs, under the rule the resolver already keeps —
  nothing derived from a pack in the repository — and with one product
  file, rather than a folder of loose meshes, as the shape to ship.
- **A size budget.** The old one was set for a 2D toy; a Bevy 3D payload
  is an order of magnitude larger and gets its own number when the build
  job returns.
