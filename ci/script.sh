#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

# Install rustup and the specified rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Load cargo environment. Specifically, put cargo into PATH.
source ~/.cargo/env

rustup toolchain install $RUST_TOOLCHAIN
rustup default $RUST_TOOLCHAIN

rustc --version
rustup --version
cargo --version

sudo apt-get -y update
sudo apt-get install -y cmake pkg-config libssl-dev

./scripts/init.sh

rustup target add wasm32-unknown-unknown --toolchain $RUST_TOOLCHAIN

case $TARGET in
	"build-client")
		cargo build --release --locked "$@"
		;;

	"runtime-test")
		cargo test -p centrifuge-chain-runtime
		;;
esac
