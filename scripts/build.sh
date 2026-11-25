#!/bin/sh
# Builds the browser game in the pinned image and prints its sha256.
# The canonical build host is linux/amd64 (cargo mixes the host into crate
# identity), so ARM machines build under emulation. Same commit + same image
# + same path (/src) = same hash on every machine. CI prints the same hash.
set -eu
cd "$(dirname "$0")/.."
docker build -q --platform linux/amd64 -t lastgame-build-amd64 . >/dev/null
docker run --rm --platform linux/amd64 --network none -v "$PWD:/src" lastgame-build-amd64 sh -c \
  'CARGO_TARGET_DIR=target/amd64 cargo build -q --release --locked --offline --target wasm32-unknown-unknown -p engine && cp target/amd64/wasm32-unknown-unknown/release/engine.wasm web/game.wasm && sha256sum web/game.wasm'
