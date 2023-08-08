#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly-2023-02-07}"
SRTOOL_VERSION="${SRTOOL_VERSION:-1.66.1-0.9.25}"
PACKAGE="${PACKAGE:-centrifuge-runtime}" # Need to replicate job for all runtimes
RUNTIME="${RUNTIME:-centrifuge}"

# Reusing the same features for different builds will safe storage in CI jobs
FEATURES="--features runtime-benchmarks,try-runtime,fast-runtime"

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

  build-runtime-testnet)
    export RUSTC_VERSION=$RUST_TOOLCHAIN
    docker run --rm -e PACKAGE=$PACKAGE -e BUILD_OPTS="--features=testnet-runtime" -v $PWD:/build -v /tmp/cargo:/cargo-home paritytech/srtool:$SRTOOL_VERSION build
    ;;

  tests)
    cargo test --workspace $FEATURES --exclude runtime-integration-tests
    ;;

  integration)
    cargo test --package runtime-integration-tests $FEATURES
    ;;

  fmt)
    cargo fmt -- --check
    ;;

  taplo)
    taplo fmt --check
    ;;

  clippy)
    cargo clippy --workspace $FEATURES -- -D warnings -A clippy::unnecessary-cast -A clippy::bool-to-int-with-if
    ;;

  benchmark)
    ./scripts/runtime_benchmarks.sh $RUNTIME
    ;;

  benchmark-check)
    ./scripts/check_benchmarks.sh $RUNTIME debug $FEATURES

esac
