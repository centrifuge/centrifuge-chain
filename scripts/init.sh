#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup install nightly-2020-02-17 # workaround as described in https://matrix.to/#/!HzySYSaIhtyWrwiwEV:matrix.org/$158222010481650qmZYp:matrix.parity.io?via=matrix.parity.io&via=matrix.org&via=web3.foundation
   # rustup update nightly
   rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2020-02-17 # workaround as described in https://matrix.to/#/!HzySYSaIhtyWrwiwEV:matrix.org/$158222010481650qmZYp:matrix.parity.io?via=matrix.parity.io&via=matrix.org&via=web3.foundation
# rustup target add wasm32-unknown-unknown --toolchain nightly
