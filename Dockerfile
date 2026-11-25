# The one build environment. Same image (linux/amd64) + same path (/src) =
# same game.wasm bytes on every machine, which is what the build quorum
# compares. Node only runs the playtests inside the wasm, which gets no imports.
FROM rust:1.100.0-slim@sha256:3b199fd6db7d709274df56565b8f4003f9e30d7bfcff9d53148c86053b7855e2
RUN rustup target add wasm32-unknown-unknown \
 && apt-get update && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
