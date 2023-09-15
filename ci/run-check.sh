#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
# export RUSTFLAGS=" -W unused-extern-crates"
rustc --version
rustup --version
cargo --version

case $TARGET in
  cargo-build)
    cargo build --release "$@"
    ;;

  test-general)
    RUST_MIN_STACK=8388608 cargo test --workspace --release --features runtime-benchmarks,try-runtime --exclude runtime-integration-tests
    ;;

  test-integration)
    RUST_MIN_STACK=8388608 cargo test --release --package runtime-integration-tests --features fast-runtime
    ;;

  lint-fmt)
    cargo fmt -- --check
    ;;

  lint-taplo)
    taplo fmt --check
    ;;

  lint-clippy)
    cargo clippy --workspace -- -D warnings -A clippy::unnecessary-cast -A clippy::bool-to-int-with-if
    ;;
  benchmark-check)
    ./scripts/check_benchmarks.sh $RUNTIME
esac
