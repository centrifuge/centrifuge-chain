#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly-2022-08-05}"
SRTOOL_VERSION="${SRTOOL_VERSION:-1.64.0}"
PACKAGE="${PACKAGE:-centrifuge-runtime}" # Need to replicate job for all runtimes
RUNTIME="${RUNTIME:-centrifuge}"

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
    docker run --rm -e PACKAGE=$PACKAGE -v $PWD:/build -v /tmp/cargo:/cargo-home paritytech/srtool:$SRTOOL_VERSION build
    ;;

  build-runtime-fast)
    export RUSTC_VERSION=$RUST_TOOLCHAIN
    docker run --rm -e PACKAGE=$PACKAGE -e BUILD_OPTS="--features=fast-runtime" -v $PWD:/build -v /tmp/cargo:/cargo-home paritytech/srtool:$SRTOOL_VERSION build
    ;;

  tests)
    RUST_MIN_STACK=8388608 cargo test --workspace --release --features test-benchmarks,try-runtime --exclude runtime-integration-tests
    ;;

  integration)
    RUST_MIN_STACK=8388608 cargo test --release --package runtime-integration-tests --features fast-runtime
    ;;

  fmt)
    cargo fmt -- --check
    ;;

  taplo)
    taplo fmt --check
    ;;

  clippy)
    cargo clippy --workspace -- -D warnings -A clippy::unnecessary-cast
    ;;

  benchmark)
    ./scripts/runtime_benchmarks.sh $RUNTIME

esac
