#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly}"

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

# Install rustup and the specified rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Load cargo environment. Specifically, put cargo into PATH.
source ~/.cargo/env

sudo apt-get -y update
sudo apt-get install -y cmake pkg-config libssl-dev

./scripts/init.sh

rustc --version
rustup --version
cargo --version

case $TARGET in
	"build-client")
		cargo build --release --locked "$@"
		;;

	"runtime-test")
		cargo test -p centrifuge-chain-runtime
		;;
esac
