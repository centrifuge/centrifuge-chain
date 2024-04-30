#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
# export RUSTFLAGS=" -W unused-extern-crates"
rustc --version
rustup --version
cargo --version

case $TARGET in
 cargo-build)
   SCCACHE_RECACHE=true cargo build -p centrifuge-chain --release "$@"
   ;;
#
#  test-general)
#    cargo test --workspace --release --features runtime-benchmarks,try-runtime --exclude runtime-integration-tests
#    ;;

  test-integration)
    cargo build --release --package runtime-integration-tests -vv

    echo "NOW TESTING"
    cargo test --release --package runtime-integration-tests --features fast-runtime &
    CARGO_PID=$!
    BUILD_DISCOVERED=false
    DEPS_DISCOVERED=false
    while true; do
      if [ -d "./target/" ]; then
        if ls ./target/release/build/runtime-integration* 1> /dev/null 2>&1; then
          ls -l ./target/release/build/runtime-integration*/out/
          echo "release build directory exists."
          BUILD_DISCOVERED=true
        fi
        if [ -d "./target/release/deps" ] && ls ./target/release/deps/runtime_integration* 1> /dev/null 2>&1; then
          ls -l ./target/release/deps/runtime_integration*/out/
          echo "Deps build directory exists."
          DEPS_DISCOVERED=true
        fi        
        if [ "$BUILD_DISCOVERED" = true ] && [ "$DEPS_DISCOVERED" = true ]; then
          echo "Both build and dependency directories have been discovered."
          break
        fi
        sleep 30
      else
        echo "Folder ./target not found"
        sleep 5
      fi
    if ! ps -p $CARGO_PID > /dev/null; then
      echo "Cargo process has finished."
      break
    fi
    done
    wait $CARGO_PID
    exit $?
    ;;

  lint-fmt)
    cargo fmt -- --check
    ;;

  lint-taplo)
    cargo install taplo-cli
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
    ;;

  try-runtime)
    cargo build -p centrifuge-chain --release --features try-runtime
    if [ "$1" == "altair" ]; then
      echo "Running try-runtime for altair"
      RUST_LOG=runtime=trace,try-runtime::cli=trace,executor=trace \
      try-runtime \
      --runtime target/release/wbuild/altair-runtime/altair_runtime.wasm \
      on-runtime-upgrade live --uri wss://fullnode.altair.centrifuge.io:443
    elif [ "$1" == "centrifuge" ]; then
      echo "Running try-runtime for centrifuge"
      RUST_LOG=runtime=trace,try-runtime::cli=trace,executor=trace \
      try-runtime \
      --runtime target/release/wbuild/centrifuge-runtime/centrifuge_runtime.wasm \
      on-runtime-upgrade live --uri wss://fullnode.centrifuge.io:443
    else
      echo "Invalid argument. Please specify 'altair' or 'centrifuge'."
      exit 1
    fi
    ;;
  subalfred)
    # Find all child directories containing Cargo.toml files
    # TODO: Filter by crates found in the workspace
    #   HINT: Use `cargo workspaces list -l" and filter by the paths
    dirs=$(find . -name Cargo.toml -print0 | xargs -0 -n1 dirname | sort -u)

    # Execute the command "subalfred check" on each directory
    for dir in $dirs; do
      # Avoiding cargo workspace
      if [[ "$dir" == "." ]]; then
        continue
      fi
      subalfred check features $dir
    done
esac
