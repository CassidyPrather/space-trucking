#!/usr/bin/env bash
# Builds the wasm binary and assembles the static site into dist/web/.
# CI runs this same script, so it stays the single source of truth for layout.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# Keep this in sync with the [[bin]] name in Cargo.toml
BIN="space-trucking"
OUT="dist/web"

# -p: the workspace also carries the native-only Bevy cabin crate
cargo build --release --target wasm32-unknown-unknown -p space-trucking

rm -rf "$OUT"
mkdir -p "$OUT"
cp web/* "$OUT/"
cp "target/wasm32-unknown-unknown/release/${BIN}.wasm" "$OUT/"

# Cache-bust every asset reference with the commit hash. The filenames never
# change across deploys, so without this a browser (and Pages' ~10 minute
# cache) happily keeps serving last week's game after a deploy; with it,
# every deploy is a fresh URL and a plain reload gets the new build.
STAMP=$(git rev-parse --short HEAD 2>/dev/null || date +%s)
sed -i.bak \
    -e "s|src=\"gl.js\"|src=\"gl.js?v=${STAMP}\"|" \
    -e "s|src=\"audio.js\"|src=\"audio.js?v=${STAMP}\"|" \
    -e "s|src=\"sapp_jsutils.js\"|src=\"sapp_jsutils.js?v=${STAMP}\"|" \
    -e "s|src=\"quad-storage.js\"|src=\"quad-storage.js?v=${STAMP}\"|" \
    -e "s|load(\"${BIN}.wasm\")|load(\"${BIN}.wasm?v=${STAMP}\")|" \
    "$OUT/index.html"
rm -f "$OUT/index.html.bak"
grep -q "v=${STAMP}" "$OUT/index.html" || {
    echo "cache-bust stamp failed to apply; index.html references changed?" >&2
    exit 1
}

# rustc's wasm32-unknown-unknown baseline emits these six post-MVP features
# (`rustc --print cfg --target wasm32-unknown-unknown` lists them). wasm-opt
# only enables them by default in newer binaryen, and refuses to validate its
# input without them — so name them explicitly and work with any version.
WASM_FEATURES=(
    --enable-bulk-memory
    --enable-multivalue
    --enable-mutable-globals
    --enable-nontrapping-float-to-int
    --enable-reference-types
    --enable-sign-ext
)

wasm="${OUT}/${BIN}.wasm"
before=$(wc -c <"$wasm")
if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz --strip-debug --strip-producers "${WASM_FEATURES[@]}" "$wasm" -o "$wasm"
    echo "wasm-opt: ${before} -> $(wc -c <"$wasm") bytes"
else
    # binaryen provides wasm-opt; CI always runs it, so this only costs you locally
    echo "wasm-opt not found; shipping unoptimized wasm (${before} bytes)"
fi

echo "Bundle ready in ${OUT}/"
echo "Serve locally: python3 -m http.server --directory ${OUT} 8080"
