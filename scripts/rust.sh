#!/usr/bin/env bash
# Build the Rust engine (engine/ca72-mpc) as static libraries: build/rust/aarch64 (the Move) and build/rust/x86_64 (scripts/test.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
IMAGE=model72-schwung-build
docker image inspect "$IMAGE" &>/dev/null || docker build -q -t "$IMAGE" engine >/dev/null
mkdir -p build/rust/aarch64 build/rust/x86_64 build/cargo-home build/target
docker run --rm -u "$(id -u):$(id -g)" -e CARGO_HOME=/w/build/cargo-home -e CARGO_TARGET_DIR=/w/build/target -v "$PWD":/w -w /w/engine "$IMAGE" sh -euc '
  cargo build -q --release -p ca72-mpc --target aarch64-unknown-linux-gnu
  cargo build -q --release -p ca72-mpc
  cp /w/build/target/aarch64-unknown-linux-gnu/release/libca72_mpc.a /w/build/rust/aarch64/
  cp /w/build/target/release/libca72_mpc.a /w/build/rust/x86_64/'
