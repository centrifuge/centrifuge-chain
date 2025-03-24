#!/usr/bin/env bash

# set -eux

RUST_TOOLCHAIN=$(grep 'channel =' rust-toolchain.toml | awk -F'"' '{print $2}')

echo "Using rust toolchain: ${RUST_TOOLCHAIN}"

echo "*** Initializing WASM build environment"

rustup update $RUST_TOOLCHAIN

rustup toolchain install $RUST_TOOLCHAIN --locked
rustup toolchain install nightly --locked
rustup default $RUST_TOOLCHAIN
rustup component add rustfmt
rustup component add rust-src
rustup component add --toolchain nightly rustfmt

rustup target add wasm32-unknown-unknown --toolchain $RUST_TOOLCHAIN
