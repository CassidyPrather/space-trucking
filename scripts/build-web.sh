#!/usr/bin/env bash
# Builds the wasm binary and assembles the static site into dist/web/.
# CI runs this same script, so it stays the single source of truth for layout.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# Keep this in sync with the [[bin]] name in Cargo.toml
BIN="game-template"
OUT="dist/web"

cargo build --release --target wasm32-unknown-unknown

rm -rf "$OUT"
mkdir -p "$OUT"
cp web/* "$OUT/"
cp "target/wasm32-unknown-unknown/release/${BIN}.wasm" "$OUT/"

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
