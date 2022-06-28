#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly-2022-05-09}"
PACKAGE="${PACKAGE:-centrifuge-runtime}" #Need to replicate job for all runtimes

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
    docker run --rm -e PACKAGE=$PACKAGE -v $PWD:/build -v /tmp/cargo:/cargo-home paritytech/srtool:$RUSTC_VERSION build
    ;;

  tests)
    RUST_MIN_STACK=8388608 cargo test --workspace --release --features test-benchmarks,try-runtime
    ;;

  integration)
    RUST_MIN_STACK=8388608 cargo test --release --package runtime-integration-tests
    ;;

  lint)
    cargo fmt -- --check
esac
