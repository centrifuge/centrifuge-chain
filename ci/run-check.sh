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
    cargo test --workspace --release --features runtime-benchmarks,try-runtime --exclude runtime-integration-tests
    ;;

  test-integration)
    cargo test --release --package runtime-integration-tests --features fast-runtime
    ;;


  lint-fmt)
    cargo fmt -- --check
    ;;

  lint-taplo)
    # The recomended installation fails because an issue in taplo, issue:
    # https://github.com/tamasfe/taplo/issues/507
    # Should be fixed in the next taplo release.
    # Recomended command:
    #   cargo install taplo-cli --locked
    cargo install --git=https://github.com/tamasfe/taplo.git taplo-cli
    taplo fmt --check
    ;;

  lint-clippy)
    cargo clippy --workspace -- -D warnings -A clippy::unnecessary-cast -A clippy::bool-to-int-with-if
    ;;
  benchmark-check)
    ./scripts/check_benchmarks.sh $RUNTIME
    ;;
  docs-build)
    RUSTDOCFLAGS="-D warnings" cargo doc --all --no-deps
esac
