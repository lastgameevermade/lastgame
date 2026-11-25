#!/bin/sh
# Proves the checker rejects every known attack (checker/suite) and accepts a harmless change.
set -eu
cd "$(dirname "$0")/.."
docker build -q -t lastgame-build . >/dev/null
exec docker run --rm --network none -e SUITE_ONLY="${SUITE_ONLY:-}" -v "$PWD:/src" lastgame-build sh checker/suite/run.sh
