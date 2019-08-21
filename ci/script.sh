#!/usr/bin/env bash

set -eux

# Enable warnings about unused extern crates
export RUSTFLAGS=" -W unused-extern-crates"

# Install rustup and the specified rust toolchain.
curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain=$RUST_TOOLCHAIN -y

# Load cargo environment. Specifically, put cargo into PATH.
source ~/.cargo/env

rustc --version
rustup --version
cargo --version

sudo apt-get -y update
sudo apt-get install -y cmake pkg-config libssl-dev

./scripts/init.sh

case $TARGET in
	"build-client")
		cargo build --release --locked "$@"
		;;
		
	"runtime-test")
		cargo test -p centrifuge-chain-runtime
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
esac
