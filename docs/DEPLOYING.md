Releases ship native binaries: publishing a GitHub Release triggers CI's
`build-and-publish` job, which builds the cabin (`-p cabin`, binary name
`space-trucking`) for Linux x86_64, macOS aarch64, and Windows x86_64 and
attaches the archives to the release.

The retired 2D console's web build (wasm + GitHub Pages) left with it —
see docs/BAY.md for the decision. If a web target returns (Bevy compiles
to wasm), the old `build-web.sh` + Pages pipeline in git history is the
reference for the shape: a folder of files, no external requests.
