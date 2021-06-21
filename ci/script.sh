#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly-2021-03-15}"

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

./scripts/init.sh install-toolchain

rustc --version
rustup --version
cargo --version

case $TARGET in
	build-node)
		cargo build --release "$@"
		;;

  build-runtime)
    export RUSTC_VERSION=$RUST_TOOLCHAIN
    docker run --rm -e RUNTIME_DIR=./runtime -e PACKAGE=centrifuge-chain-runtime -v $PWD:/build -v /tmp/cargo:/cargo-home chevdor/srtool:$RUSTC_VERSION build
    ;;

  tests)
    cargo test -p pallet-bridge-mapping -p pallet-fees -p pallet-anchors -p pallet-claims -p proofs --release
    ;;

  lint)
    cargo fmt -- --check
esac
