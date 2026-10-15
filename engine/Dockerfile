# The engine's build image: Rust 1.97 with the armv7 (MPC Gen1 / Force) and aarch64 (Gen2) targets and rustfmt.
FROM rust:1.97-bookworm
RUN rustup target add armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu && rustup component add rustfmt \
 && apt-get update -qq && apt-get install -y -qq gcc-arm-linux-gnueabihf gcc-aarch64-linux-gnu >/dev/null \
 && rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
