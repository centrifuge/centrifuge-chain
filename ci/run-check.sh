#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
# export RUSTFLAGS=" -W unused-extern-crates"
rustc --version
rustup --version
cargo --version

case $TARGET in
  cargo-build)
    cargo build -p centrifuge-chain --release "$@"
    ;;

  test-general)
    cargo test --release --features runtime-benchmarks,try-runtime,fast-runtime --exclude runtime-integration-tests
    ;;

  test-integration)
    cargo test --release --package runtime-integration-tests --features fast-runtime
    ;;

  lint-fmt)
    cargo fmt -- --check
    ;;

  lint-taplo)
    taplo fmt --check
    ;;

  lint-clippy)
    cargo clippy -- -D warnings -A clippy::unnecessary-cast -A clippy::bool-to-int-with-if
    ;;
  benchmark-check)
    ./scripts/check_benchmarks.sh $RUNTIME
    ;;
  docs-build)
    RUSTDOCFLAGS="-D warnings" cargo doc --all --no-deps
esac
