#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

# Install rustup and the specified rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain=$RUST_TOOLCHAIN -y

# Load cargo environment. Specifically, put cargo into PATH.
source ~/.cargo/env

rustc --version
rustup --version
cargo --version

case $TARGET in
	"build-client")
		sudo apt-get -y update
		sudo apt-get install -y cmake pkg-config libssl-dev

		./scripts/init.sh
		./scripts/build.sh --locked "$@"

		cargo build --release --locked "$@"
		;;

	"wasm-build")
		# Install prerequisites and build all wasm projects
		./scripts/init.sh
		./scripts/build.sh --locked "$@"
		;;
		
	"runtime-test")
		cargo test -p centrifuge-chain-runtime
		;;		
esac