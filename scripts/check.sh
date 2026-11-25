#!/bin/sh
# Runs the checker exactly as CI does: pinned image, networking off.
#   scripts/check.sh [--changed FILE]
set -eu
cd "$(dirname "$0")/.."
docker build -q -t lastgame-build . >/dev/null
exec docker run --rm --network none -v "$PWD:/src" lastgame-build sh -c \
  'cargo build -q --release --locked --offline -p checker && target/release/checker --out verdict.json "$@"' sh "$@"
