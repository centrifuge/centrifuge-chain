#!/usr/bin/env bash

set -eux

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-nightly}"

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

./scripts/init.sh install-toolchain

rustc --version
rustup --version
cargo --version

case $TARGET in
	build-client)
		cargo build --release "$@"
		;;

	runtime-test)
		cargo test -p centrifuge-chain-runtime --release
		wget https://github.com/SimonKagstrom/kcov/archive/master.tar.gz &&
        tar xzf master.tar.gz &&
        cd kcov-master &&
        mkdir build &&
        cd build &&
        cmake .. &&
        make &&
        make install DESTDIR=../../kcov-build &&
        cd ../.. &&
        rm -rf kcov-master &&
        for file in target/debug/centrifuge_chain*; do [ -x "${file}" ] || continue; mkdir -p "target/cov/$(basename $file)"; kcov-build/usr/local/bin/kcov --exclude-pattern=/.cargo,/usr/lib --verify "target/cov/$(basename $file)" "$file"; done &&
        bash <(curl -s https://codecov.io/bash) &&
        echo "Uploaded code coverage"
		;;

  build-runtime)
    export RUSTC_VERSION=$RUST_TOOLCHAIN
    docker run --rm -e RUNTIME_DIR=./runtime -v $PWD:/build -v $TMPDIR/cargo:/cargo-home chevdor/srtool:$RUSTC_VERSION build
esac
