#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly}"

echo "*** Initializing WASM build environment"

rustup update $RUST_TOOLCHAIN
rustup update stable

rustup toolchain install $RUST_TOOLCHAIN
rustup default $RUST_TOOLCHAIN

rustup target add wasm32-unknown-unknown --toolchain $RUST_TOOLCHAIN
