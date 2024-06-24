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
    cargo test --workspace --release --features runtime-benchmarks,try-runtime --exclude runtime-integration-tests
    ;;

  test-integration)
    cargo test --release --package runtime-integration-tests
    ;;

  lint-fmt)
    # We need the `+nightly` as long as the used toolchain is the stable version
    # in order to allow using the formatter unstable options
    cargo +nightly fmt -- --check
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
    echo "Contents of target/release/wbuild/:"
    ls -l target/release/wbuild/
    echo "Contents of subdirectories in target/release/wbuild/:"
    ls -l target/release/wbuild/*/
    # Check if try-runtime is available
    if ! command try-runtime --version &> /dev/null
    then
        echo "try-runtime could not be found, trying 'cargo install try-runtime-cli'"
        curl -sL https://github.com/paritytech/try-runtime-cli/releases/download/v0.6.1/try-runtime-x86_64-unknown-linux-musl -o try-runtime
        chmod +x ./try-runtime
    fi    
    if [ "$1" == "altair" ]; then
      echo "Running try-runtime for altair"
      RUST_LOG=runtime=trace,try-runtime::cli=trace,executor=trace \
      ./try-runtime \
      --runtime target/release/wbuild/altair-runtime/altair_runtime.wasm \
      on-runtime-upgrade live --uri wss://fullnode.altair.centrifuge.io:443
    elif [ "$1" == "centrifuge" ]; then
      echo "Running try-runtime for centrifuge"
      RUST_LOG=runtime=trace,try-runtime::cli=trace,executor=trace \
      ./try-runtime \
      --runtime target/release/wbuild/centrifuge-runtime/centrifuge_runtime.wasm \
      on-runtime-upgrade live --uri wss://fullnode.centrifuge.io:443
    elif [ "$1" == "demo" ]; then
      echo "Running try-runtime for demo env"
      RUST_LOG=runtime=trace,try-runtime::cli=trace,executor=trace \
      ./try-runtime \
      --runtime target/release/wbuild/development-runtime/development_runtime.wasm \
      on-runtime-upgrade live --uri  wss://fullnode-query.demo.k-f.dev:443 # fullnode.demo.k-f.dev is throtlled and thus it might fail
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
